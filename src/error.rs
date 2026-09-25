use axum::{
    extract::rejection::JsonRejection,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

#[derive(Debug)]
pub enum AppError {
    Unauthorized(String),
    Forbidden(String),
    NotFound(String),
    BadRequest(String),
    StreamNotSupported(String),
    Internal(String),
    BadGateway(String),
    GatewayTimeout(String),
    ServiceUnavailable(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, err_type, code, message) = match self {
            AppError::Unauthorized(msg) => (
                StatusCode::UNAUTHORIZED,
                "invalid_request_error",
                "invalid_api_key",
                msg,
            ),
            AppError::Forbidden(msg) => (
                StatusCode::FORBIDDEN,
                "invalid_request_error",
                "forbidden",
                msg,
            ),
            AppError::NotFound(msg) => (
                StatusCode::NOT_FOUND,
                "invalid_request_error",
                "model_not_found",
                msg,
            ),
            AppError::BadRequest(msg) => (
                StatusCode::BAD_REQUEST,
                "invalid_request_error",
                "invalid_request_error",
                msg,
            ),
            AppError::StreamNotSupported(msg) => (
                StatusCode::BAD_REQUEST,
                "invalid_request_error",
                "stream_not_supported",
                msg,
            ),
            AppError::Internal(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "api_error",
                "internal_error",
                msg,
            ),
            AppError::BadGateway(msg) => (StatusCode::BAD_GATEWAY, "api_error", "bad_gateway", msg),
            AppError::GatewayTimeout(msg) => (
                StatusCode::GATEWAY_TIMEOUT,
                "api_error",
                "gateway_timeout",
                msg,
            ),
            AppError::ServiceUnavailable(msg) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "api_error",
                "service_unavailable",
                msg,
            ),
        };

        let body = Json(json!({
            "error": {
                "message": message,
                "type": err_type,
                "param": serde_json::Value::Null,
                "code": code,
            },
            "detail": message,
        }));

        (status, body).into_response()
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::Unauthorized(msg) => write!(f, "Unauthorized: {msg}"),
            AppError::Forbidden(msg) => write!(f, "Forbidden: {msg}"),
            AppError::NotFound(msg) => write!(f, "Not Found: {msg}"),
            AppError::BadRequest(msg) => write!(f, "Bad Request: {msg}"),
            AppError::StreamNotSupported(msg) => write!(f, "Stream Not Supported: {msg}"),
            AppError::Internal(msg) => write!(f, "Internal Error: {msg}"),
            AppError::BadGateway(msg) => write!(f, "Bad Gateway: {msg}"),
            AppError::GatewayTimeout(msg) => write!(f, "Gateway Timeout: {msg}"),
            AppError::ServiceUnavailable(msg) => write!(f, "Service Unavailable: {msg}"),
        }
    }
}

impl std::error::Error for AppError {}

impl From<JsonRejection> for AppError {
    fn from(rejection: JsonRejection) -> Self {
        AppError::BadRequest(rejection.to_string())
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(err: rusqlite::Error) -> Self {
        AppError::Internal(format!("Database error: {err}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_variants() {
        let err = AppError::BadRequest("bad payload".to_string());
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        let err = AppError::StreamNotSupported("no stream".to_string());
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        let err = AppError::NotFound("not found".to_string());
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        let err = AppError::Unauthorized("no key".to_string());
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        let err = AppError::Forbidden("admin only".to_string());
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);

        let err = AppError::Internal("server boom".to_string());
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let err = AppError::BadGateway("upstream failed".to_string());
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::BAD_GATEWAY);

        let err = AppError::GatewayTimeout("upstream timed out".to_string());
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::GATEWAY_TIMEOUT);

        let err = AppError::ServiceUnavailable("overloaded".to_string());
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}
