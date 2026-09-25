use axum::{
    extract::{FromRef, FromRequestParts},
    http::{header::AUTHORIZATION, request::Parts, HeaderValue},
};

use crate::db::Database;
use crate::error::AppError;
use crate::state::AppState;

pub fn constant_time_eq(a: &str, b: &str) -> bool {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    if a_bytes.len() != b_bytes.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a_bytes.iter().zip(b_bytes.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

pub fn extract_token(auth_header: Option<&HeaderValue>) -> Result<&str, AppError> {
    let header_val = match auth_header {
        Some(h) => h
            .to_str()
            .map_err(|_| AppError::Unauthorized("Invalid API key".to_string()))?,
        None => return Err(AppError::Unauthorized("Invalid API key".to_string())),
    };

    let mut parts = header_val.splitn(2, ' ');
    let scheme = parts.next().unwrap_or("");
    let token = parts.next().unwrap_or("").trim();

    if !scheme.eq_ignore_ascii_case("bearer") || token.is_empty() {
        return Err(AppError::Unauthorized("Invalid API key".to_string()));
    }

    Ok(token)
}

pub fn extract_and_verify_token(
    auth_header: Option<&HeaderValue>,
    expected_key: &str,
) -> Result<(), AppError> {
    let token = extract_token(auth_header)?;
    if !constant_time_eq(token, expected_key) {
        return Err(AppError::Unauthorized("Invalid API key".to_string()));
    }
    Ok(())
}

pub fn verify_token_with_db(
    auth_header: Option<&HeaderValue>,
    db: &Database,
    fallback_key: &str,
) -> Result<AuthenticatedUser, AppError> {
    let token = extract_token(auth_header)?;

    match db.get_key(token) {
        Ok(Some(api_key)) => {
            if !api_key.is_active {
                return Err(AppError::Unauthorized("API key is inactive".to_string()));
            }
            Ok(AuthenticatedUser {
                key_id: api_key.id,
                key: api_key.key,
                name: api_key.name,
                role: api_key.role,
            })
        }
        Ok(None) => {
            if constant_time_eq(token, fallback_key) {
                Ok(AuthenticatedUser {
                    key_id: "default".to_string(),
                    key: fallback_key.to_string(),
                    name: "Default Key".to_string(),
                    role: "admin".to_string(),
                })
            } else {
                Err(AppError::Unauthorized("Invalid API key".to_string()))
            }
        }
        Err(err) => {
            tracing::error!("Database lookup error during auth: {err}");
            if constant_time_eq(token, fallback_key) {
                Ok(AuthenticatedUser {
                    key_id: "default".to_string(),
                    key: fallback_key.to_string(),
                    name: "Default Key".to_string(),
                    role: "admin".to_string(),
                })
            } else {
                Err(AppError::Unauthorized("Invalid API key".to_string()))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub key_id: String,
    pub key: String,
    pub name: String,
    pub role: String,
}

impl AuthenticatedUser {
    pub fn is_admin(&self) -> bool {
        self.role == "admin"
    }
}

#[derive(Debug, Clone)]
pub struct AdminUser(pub AuthenticatedUser);

impl std::ops::Deref for AdminUser {
    type Target = AuthenticatedUser;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[axum::async_trait]
impl<S> FromRequestParts<S> for AdminUser
where
    S: Send + Sync,
    AppState: FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let user = AuthenticatedUser::from_request_parts(parts, state).await?;
        if user.role != "admin" {
            return Err(AppError::Forbidden("Admin role required".to_string()));
        }
        Ok(AdminUser(user))
    }
}

#[axum::async_trait]
impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
    AppState: FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app_state = AppState::from_ref(state);
        let auth_header = parts.headers.get(AUTHORIZATION);
        if auth_header.is_some() {
            verify_token_with_db(auth_header, &app_state.db, &app_state.config.api_key)
        } else {
            let path = parts.uri.path();
            let is_sse_allowed = (path == "/admin/active-requests"
                || path == "/admin/active-requests/stream")
                && parts.method == axum::http::Method::GET;

            if is_sse_allowed {
                if let Some(query) = parts.uri.query() {
                    let token = query.split('&').find_map(|pair| {
                        let mut kv = pair.split('=');
                        if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
                            if k == "token" || k == "key" {
                                return Some(v);
                            }
                        }
                        None
                    });
                    if let Some(t) = token {
                        let header_val =
                            axum::http::HeaderValue::from_str(&format!("Bearer {}", t)).ok();
                        return verify_token_with_db(
                            header_val.as_ref(),
                            &app_state.db,
                            &app_state.config.api_key,
                        );
                    }
                }
            }
            verify_token_with_db(None, &app_state.db, &app_state.config.api_key)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_time_eq() {
        assert!(constant_time_eq("secret123", "secret123"));
        assert!(!constant_time_eq("secret123", "secret124"));
        assert!(!constant_time_eq("secret123", "secret12"));
        assert!(!constant_time_eq("", "secret"));
        assert!(constant_time_eq("", ""));
    }

    #[test]
    fn test_extract_and_verify_token() {
        let expected = "valid-key-xyz";

        // Valid bearer
        let header = HeaderValue::from_static("Bearer valid-key-xyz");
        assert!(extract_and_verify_token(Some(&header), expected).is_ok());

        // Valid bearer case-insensitive scheme
        let header = HeaderValue::from_static("bearer valid-key-xyz");
        assert!(extract_and_verify_token(Some(&header), expected).is_ok());

        // Invalid key
        let header = HeaderValue::from_static("Bearer wrong-key");
        assert!(extract_and_verify_token(Some(&header), expected).is_err());

        // Missing bearer prefix
        let header = HeaderValue::from_static("valid-key-xyz");
        assert!(extract_and_verify_token(Some(&header), expected).is_err());

        // Basic auth prefix
        let header = HeaderValue::from_static("Basic valid-key-xyz");
        assert!(extract_and_verify_token(Some(&header), expected).is_err());

        // Missing header
        assert!(extract_and_verify_token(None, expected).is_err());
    }

    #[test]
    fn test_verify_token_with_db_and_fallback() {
        let db = Database::open_in_memory(Some("fallback-key")).unwrap();

        // 1. Initial seeded key
        let header = HeaderValue::from_static("Bearer fallback-key");
        let user = verify_token_with_db(Some(&header), &db, "fallback-key").unwrap();
        assert_eq!(user.key, "fallback-key");

        // 2. Dynamic key in DB
        let dyn_key = db
            .create_key("Dynamic Key", Some("dynamic-secret-key"))
            .unwrap();
        let header = HeaderValue::from_static("Bearer dynamic-secret-key");
        let user = verify_token_with_db(Some(&header), &db, "fallback-key").unwrap();
        assert_eq!(user.key_id, dyn_key.id);
        assert_eq!(user.key, "dynamic-secret-key");

        // 3. Deactivated dynamic key
        db.set_key_active(&dyn_key.id, false).unwrap();
        let err = verify_token_with_db(Some(&header), &db, "fallback-key").unwrap_err();
        assert!(matches!(err, AppError::Unauthorized(msg) if msg.contains("inactive")));

        // 4. Unknown key
        let header = HeaderValue::from_static("Bearer non-existent-key");
        let err = verify_token_with_db(Some(&header), &db, "fallback-key").unwrap_err();
        assert!(matches!(err, AppError::Unauthorized(msg) if msg.contains("Invalid API key")));

        // 5. Role check
        assert_eq!(user.role, "client");
        assert!(!user.is_admin());

        let admin_key = db
            .create_key_with_role("Admin Key", Some("admin-secret-key"), "admin")
            .unwrap();
        let header = HeaderValue::from_static("Bearer admin-secret-key");
        let admin_user = verify_token_with_db(Some(&header), &db, "fallback-key").unwrap();
        assert_eq!(admin_user.key_id, admin_key.id);
        assert_eq!(admin_user.role, "admin");
        assert!(admin_user.is_admin());
    }
}
