use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::db::{AccountRecord, Database};
use crate::error::AppError;

pub fn current_time_secs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenRefreshOutput {
    pub access_token: String,
    pub expires_in_secs: u64,
}

#[axum::async_trait]
pub trait TokenRefresher: Send + Sync + std::fmt::Debug {
    async fn refresh_token(&self, refresh_token: &str) -> Result<TokenRefreshOutput, String>;
}

#[derive(Debug, Default)]
pub struct MockTokenRefresher {
    pub custom_token: RwLock<Option<String>>,
    pub custom_expires_in: AtomicU64,
    pub should_fail: AtomicBool,
}

impl MockTokenRefresher {
    pub fn new() -> Self {
        Self {
            custom_token: RwLock::new(None),
            custom_expires_in: AtomicU64::new(3600),
            should_fail: AtomicBool::new(false),
        }
    }

    pub fn with_token(token: &str) -> Self {
        Self {
            custom_token: RwLock::new(Some(token.to_string())),
            custom_expires_in: AtomicU64::new(3600),
            should_fail: AtomicBool::new(false),
        }
    }

    pub fn set_failing(&self, fail: bool) {
        self.should_fail.store(fail, Ordering::SeqCst);
    }
}

#[axum::async_trait]
impl TokenRefresher for MockTokenRefresher {
    async fn refresh_token(&self, refresh_token: &str) -> Result<TokenRefreshOutput, String> {
        if self.should_fail.load(Ordering::SeqCst) {
            return Err("Mock token refresh failed".to_string());
        }
        let token = self
            .custom_token
            .read()
            .unwrap()
            .clone()
            .unwrap_or_else(|| {
                let safe_sub: String = refresh_token.chars().take(8).collect();
                format!("mock-token-for-{safe_sub}")
            });
        let expires_in = self.custom_expires_in.load(Ordering::SeqCst);
        Ok(TokenRefreshOutput {
            access_token: token,
            expires_in_secs: expires_in,
        })
    }
}

pub const AG_CLIENT_ID: &str =
    "1071006060591-tmhssin2h21lcre235vtolojh4g403ep.apps.googleusercontent.com";
/// OAuth client secret must be supplied at runtime; never commit credentials.
pub const AG_CLIENT_SECRET: &str = "";

#[allow(clippy::bind_instead_of_map)]
fn google_client_secret() -> Result<String, String> {
    std::env::var("AG_GOOGLE_CLIENT_SECRET")
        .or_else(|_| std::env::var("GOOGLE_CLIENT_SECRET"))
        .or_else(|_| {
            #[cfg(test)]
            {
                Ok::<String, std::env::VarError>("test-google-client-secret".to_string())
            }
            #[cfg(not(test))]
            {
                Err(std::env::VarError::NotPresent)
            }
        })
        .map_err(|_| {
            "Google OAuth client secret is missing; set AG_GOOGLE_CLIENT_SECRET".to_string()
        })
}
pub const AG_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
pub const AG_BASE_URL: &str = "https://daily-cloudcode-pa.googleapis.com";
pub const AG_QUOTA_URL: &str =
    "https://daily-cloudcode-pa.googleapis.com/v1internal:retrieveUserQuotaSummary";
pub const AG_USER_AGENT: &str = "antigravity/ide/2.11.0 darwin/arm64";
pub const AG_AUTHORIZE_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
pub const AG_USERINFO_URL: &str = "https://www.googleapis.com/oauth2/v1/userinfo";
pub const AG_SCOPES: &[&str] = &[
    "https://www.googleapis.com/auth/cloud-platform",
    "https://www.googleapis.com/auth/userinfo.email",
    "https://www.googleapis.com/auth/userinfo.profile",
    "https://www.googleapis.com/auth/cclog",
    "https://www.googleapis.com/auth/experimentsandconfigs",
];

pub fn urlencoding_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push_str(&format!("%{:02X}", b));
            }
        }
    }
    out
}

pub fn build_google_authorize_url(redirect_uri: &str, state: &str) -> String {
    let scope_str = AG_SCOPES.join(" ");
    let params = [
        ("client_id", AG_CLIENT_ID),
        ("redirect_uri", redirect_uri),
        ("response_type", "code"),
        ("scope", &scope_str),
        ("access_type", "offline"),
        ("prompt", "select_account consent"),
        ("state", state),
    ];
    let query = params
        .iter()
        .map(|(k, v)| format!("{k}={}", urlencoding_encode(v)))
        .collect::<Vec<_>>()
        .join("&");
    format!("{AG_AUTHORIZE_URL}?{query}")
}

pub const GOOGLE_OAUTH_TICKET_TTL_SECS: f64 = 600.0; // 10 minutes

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleOAuthTicket {
    pub state: String,
    pub redirect_uri: String,
    pub status: String,
    pub email: Option<String>,
    pub error: Option<String>,
    pub created_at: f64,
    pub expires_at: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleOAuthStartResponse {
    pub ok: bool,
    pub state: String,
    pub authorize_url: String,
    pub authorization_url: String,
    pub redirect_uri: String,
    pub expires_in: u64,
}

#[derive(Debug, Clone)]
pub struct GoogleTokenRefresher {
    client: reqwest::Client,
    token_url: String,
}

impl Default for GoogleTokenRefresher {
    fn default() -> Self {
        Self::new()
    }
}

impl GoogleTokenRefresher {
    pub fn new() -> Self {
        let token_url = std::env::var("AG_TOKEN_URL").unwrap_or_else(|_| AG_TOKEN_URL.to_string());
        Self::with_url(token_url)
    }

    pub fn with_url(token_url: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to build GoogleTokenRefresher reqwest client"),
            token_url: token_url.into(),
        }
    }

    pub fn with_client_and_url(client: reqwest::Client, token_url: impl Into<String>) -> Self {
        Self {
            client,
            token_url: token_url.into(),
        }
    }

    pub fn with_token_url(mut self, url: &str) -> Self {
        self.token_url = url.to_string();
        self
    }

    pub fn token_url(&self) -> &str {
        &self.token_url
    }
}

#[axum::async_trait]
impl TokenRefresher for GoogleTokenRefresher {
    async fn refresh_token(&self, refresh_token: &str) -> Result<TokenRefreshOutput, String> {
        let client_secret = google_client_secret()?;
        let params = [
            ("client_id", AG_CLIENT_ID),
            ("client_secret", client_secret.as_str()),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
        ];

        let res = self
            .client
            .post(&self.token_url)
            .form(&params)
            .send()
            .await
            .map_err(|e| format!("Token refresh network error: {e}"))?;

        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            return Err(format!("Token refresh failed {status}: {text}"));
        }

        let json: serde_json::Value = res
            .json()
            .await
            .map_err(|e| format!("Failed to parse token refresh JSON: {e}"))?;

        let access_token = json
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing access_token in refresh response".to_string())?
            .to_string();

        let expires_in_secs = json
            .get("expires_in")
            .and_then(|v| v.as_u64())
            .unwrap_or(3600);

        Ok(TokenRefreshOutput {
            access_token,
            expires_in_secs,
        })
    }
}

pub const QUOTA_CACHE_TTL_SECS: f64 = 120.0;

#[axum::async_trait]
pub trait GoogleQuotaFetcher: Send + Sync + std::fmt::Debug {
    async fn fetch_quota(&self, access_token: &str) -> Result<serde_json::Value, String>;
}

#[derive(Debug, Clone)]
pub struct DefaultGoogleQuotaFetcher {
    client: reqwest::Client,
    quota_url: String,
}

impl Default for DefaultGoogleQuotaFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl DefaultGoogleQuotaFetcher {
    pub fn new() -> Self {
        let quota_url = std::env::var("AG_QUOTA_URL").unwrap_or_else(|_| AG_QUOTA_URL.to_string());
        Self::with_url(quota_url)
    }

    pub fn with_url(quota_url: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .expect("Failed to build DefaultGoogleQuotaFetcher client"),
            quota_url: quota_url.into(),
        }
    }
}

pub fn parse_google_quota_summary(payload: &serde_json::Value) -> serde_json::Value {
    let now = current_time_secs();
    if payload.get("gemini_5h").is_some() || payload.get("claude_5h").is_some() {
        let mut cloned = payload.clone();
        if let Some(obj) = cloned.as_object_mut() {
            if !obj.contains_key("fetched_at") {
                obj.insert("fetched_at".to_string(), serde_json::json!(now));
            }
        }
        return cloned;
    }

    let mut normalized = serde_json::json!({
        "fetched_at": now
    });

    if let Some(groups) = payload.get("groups").and_then(|g| g.as_array()) {
        if let Some(map) = normalized.as_object_mut() {
            for group in groups {
                if let Some(buckets) = group.get("buckets").and_then(|b| b.as_array()) {
                    for bucket in buckets {
                        let bucket_id = bucket
                            .get("bucketId")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let mapping = match bucket_id {
                            "gemini-5h" | "gemini_5h" => Some(("gemini_5h", "Gemini 5h")),
                            "gemini-weekly" | "gemini_weekly" => {
                                Some(("gemini_weekly", "Gemini Weekly"))
                            }
                            "3p-5h" | "3p_5h" => Some(("claude_5h", "Claude/GPT 5h")),
                            "3p-weekly" | "3p_weekly" => {
                                Some(("claude_weekly", "Claude/GPT Weekly"))
                            }
                            _ => None,
                        };

                        if let Some((key, display_name)) = mapping {
                            let fraction = bucket
                                .get("remainingFraction")
                                .or_else(|| bucket.get("remaining_fraction"))
                                .and_then(|v| v.as_f64())
                                .unwrap_or(0.0);
                            let clamped = fraction.clamp(0.0, 1.0);
                            let remaining_percent = ((clamped * 100.0) * 10.0).round() / 10.0;
                            let reset_time = bucket
                                .get("resetTime")
                                .or_else(|| bucket.get("reset_time"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("");

                            map.insert(
                                key.to_string(),
                                serde_json::json!({
                                    "remaining_percent": remaining_percent,
                                    "reset_time": reset_time,
                                    "display_name": display_name
                                }),
                            );
                        }
                    }
                }
            }
        }
    }

    normalized
}

#[axum::async_trait]
impl GoogleQuotaFetcher for DefaultGoogleQuotaFetcher {
    async fn fetch_quota(&self, access_token: &str) -> Result<serde_json::Value, String> {
        let res = self
            .client
            .post(&self.quota_url)
            .header(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {access_token}"),
            )
            .header(reqwest::header::USER_AGENT, AG_USER_AGENT)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header("X-Client-Name", "antigravity")
            .header("X-Client-Version", "2.11.0")
            .json(&serde_json::json!({}))
            .send()
            .await
            .map_err(|e| format!("Google quota network error: {e}"))?;

        let status = res.status();
        let body = res
            .text()
            .await
            .map_err(|e| format!("Failed to read Google quota response: {e}"))?;

        if !status.is_success() {
            return Err(format!("Google quota upstream error ({status}): {body}"));
        }

        let raw: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| format!("Failed to parse Google quota response JSON: {e}"))?;

        Ok(parse_google_quota_summary(&raw))
    }
}

#[derive(Debug, Clone)]
pub struct MockGoogleQuotaFetcher {
    payload: serde_json::Value,
}

impl Default for MockGoogleQuotaFetcher {
    fn default() -> Self {
        Self {
            payload: serde_json::json!({
                "groups": [
                    {
                        "displayName": "Gemini Models",
                        "buckets": [
                            {
                                "bucketId": "gemini-5h",
                                "displayName": "Five Hour Limit Remaining",
                                "window": "5h",
                                "remainingFraction": 0.85,
                                "resetTime": "2026-09-23T05:00:00Z"
                            },
                            {
                                "bucketId": "gemini-weekly",
                                "displayName": "Weekly Limit Remaining",
                                "window": "weekly",
                                "remainingFraction": 0.72,
                                "resetTime": "2026-09-27T12:00:00Z"
                            }
                        ]
                    },
                    {
                        "displayName": "Claude and GPT models",
                        "buckets": [
                            {
                                "bucketId": "3p-5h",
                                "displayName": "Five Hour Limit Remaining",
                                "window": "5h",
                                "remainingFraction": 0.95,
                                "resetTime": "2026-09-23T06:00:00Z"
                            },
                            {
                                "bucketId": "3p-weekly",
                                "displayName": "Weekly Limit Remaining",
                                "window": "weekly",
                                "remainingFraction": 0.60,
                                "resetTime": "2026-09-28T18:00:00Z"
                            }
                        ]
                    }
                ]
            }),
        }
    }
}

impl MockGoogleQuotaFetcher {
    pub fn new(payload: serde_json::Value) -> Self {
        Self { payload }
    }
}

#[axum::async_trait]
impl GoogleQuotaFetcher for MockGoogleQuotaFetcher {
    async fn fetch_quota(&self, _access_token: &str) -> Result<serde_json::Value, String> {
        Ok(parse_google_quota_summary(&self.payload))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AccountResponse {
    pub id: String,
    pub email: String,
    pub is_active: bool,
    pub cooldown_remaining: f64,
    pub last_used_ago: Option<f64>,
    pub total_requests: i64,
    pub error_count: i64,
    pub last_error: Option<String>,
    pub token_valid: bool,
    pub recent_rpm: usize,
    pub quota: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct CreateAccountRequest {
    pub email: Option<String>,
    pub refresh_token: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreateAccountResponse {
    pub id: String,
    pub email: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct ListAccountsResponse {
    pub accounts: Vec<AccountResponse>,
    pub total: usize,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccountActionResponse {
    pub ok: bool,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct AccountQuotaResponse {
    pub account_id: String,
    pub quota: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RefreshAllAccountsQuotaResponse {
    pub ok: bool,
    pub total: usize,
    pub refreshed: usize,
}

#[derive(Debug, Clone)]
pub struct AccountItem {
    pub record: AccountRecord,
    pub recent_requests: VecDeque<f64>,
    pub quota_cache: serde_json::Value,
    pub quota_last_attempt: f64,
    pub quota_lock: Arc<tokio::sync::Mutex<()>>,
    pub in_flight: Arc<AtomicUsize>,
}

pub struct GoogleAccountLease {
    pool: Arc<AccountPool>,
    pub account: AccountRecord,
    in_flight: Arc<AtomicUsize>,
    committed: bool,
}

impl GoogleAccountLease {
    pub fn new(
        pool: Arc<AccountPool>,
        account: AccountRecord,
        in_flight: Arc<AtomicUsize>,
    ) -> Self {
        Self {
            pool,
            account,
            in_flight,
            committed: false,
        }
    }

    pub fn commit_success(&mut self) {
        if !self.committed {
            self.committed = true;
            let _ = self.pool.mark_used(&self.account.id);
            self.in_flight.fetch_sub(1, Ordering::SeqCst);
        }
    }

    pub fn commit_error(&mut self, error: &str, is_ban: bool) {
        if !self.committed {
            self.committed = true;
            let _ = self.pool.mark_error(&self.account.id, error, is_ban);
            self.in_flight.fetch_sub(1, Ordering::SeqCst);
        }
    }

    pub fn commit_error_with_cooldown(&mut self, error: &str, cooldown_secs: f64) {
        if !self.committed {
            self.committed = true;
            let _ = self
                .pool
                .mark_error_with_cooldown(&self.account.id, error, cooldown_secs);
            self.in_flight.fetch_sub(1, Ordering::SeqCst);
        }
    }

    pub fn release(&mut self) {
        if !self.committed {
            self.committed = true;
            self.in_flight.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

impl Drop for GoogleAccountLease {
    fn drop(&mut self) {
        if !self.committed {
            self.committed = true;
            self.in_flight.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

pub struct AccountPool {
    db: Arc<Database>,
    accounts: RwLock<HashMap<String, AccountItem>>,
    refresher: Arc<dyn TokenRefresher>,
    quota_fetcher: Arc<dyn GoogleQuotaFetcher>,
    pub min_gap_secs: f64,
    pub max_rpm: usize,
    pub max_concurrency: usize,
    oauth_tickets: RwLock<HashMap<String, GoogleOAuthTicket>>,
}

impl std::fmt::Debug for AccountPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let count = self.accounts.read().map(|m| m.len()).unwrap_or(0);
        f.debug_struct("AccountPool")
            .field("account_count", &count)
            .field("min_gap_secs", &self.min_gap_secs)
            .field("max_rpm", &self.max_rpm)
            .field("max_concurrency", &self.max_concurrency)
            .finish()
    }
}

impl AccountPool {
    pub fn new(db: Arc<Database>) -> Self {
        let min_gap_secs = std::env::var("AG_GAP_S")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.0);
        let max_concurrency = std::env::var("AG_MAX_CONCURRENCY")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(2);
        Self {
            db,
            accounts: RwLock::new(HashMap::new()),
            refresher: Arc::new(MockTokenRefresher::new()),
            quota_fetcher: Arc::new(MockGoogleQuotaFetcher::default()),
            min_gap_secs,
            max_rpm: 60,
            max_concurrency,
            oauth_tickets: RwLock::new(HashMap::new()),
        }
    }

    pub fn with_refresher(db: Arc<Database>, refresher: Arc<dyn TokenRefresher>) -> Self {
        let mut pool = Self::new(db);
        pool.refresher = refresher;
        pool
    }

    pub fn with_quota_fetcher(mut self, quota_fetcher: Arc<dyn GoogleQuotaFetcher>) -> Self {
        self.quota_fetcher = quota_fetcher;
        self
    }

    pub fn with_components(
        db: Arc<Database>,
        refresher: Arc<dyn TokenRefresher>,
        quota_fetcher: Arc<dyn GoogleQuotaFetcher>,
        min_gap_secs: f64,
        max_rpm: usize,
    ) -> Self {
        let max_concurrency = std::env::var("AG_MAX_CONCURRENCY")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(2);
        Self {
            db,
            accounts: RwLock::new(HashMap::new()),
            refresher,
            quota_fetcher,
            min_gap_secs,
            max_rpm,
            max_concurrency,
            oauth_tickets: RwLock::new(HashMap::new()),
        }
    }

    pub fn with_config(
        db: Arc<Database>,
        refresher: Arc<dyn TokenRefresher>,
        min_gap_secs: f64,
        max_rpm: usize,
    ) -> Self {
        let max_concurrency = std::env::var("AG_MAX_CONCURRENCY")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(2);
        Self {
            db,
            accounts: RwLock::new(HashMap::new()),
            refresher,
            quota_fetcher: Arc::new(MockGoogleQuotaFetcher::default()),
            min_gap_secs,
            max_rpm,
            max_concurrency,
            oauth_tickets: RwLock::new(HashMap::new()),
        }
    }

    pub fn clean_expired_tickets(&self) {
        let now = current_time_secs();
        if let Ok(mut tickets) = self.oauth_tickets.write() {
            tickets.retain(|_, t| t.expires_at > now);
        }
    }

    pub fn start_oauth(
        &self,
        redirect_uri: &str,
        origin: Option<&str>,
    ) -> GoogleOAuthStartResponse {
        self.clean_expired_tickets();
        let now = current_time_secs();
        let state_id = if let Some(orig) = origin.filter(|o| !o.trim().is_empty()) {
            format!("{}|{}", orig.trim(), uuid::Uuid::new_v4().simple())
        } else {
            uuid::Uuid::new_v4().simple().to_string()
        };

        let ticket = GoogleOAuthTicket {
            state: state_id.clone(),
            redirect_uri: redirect_uri.to_string(),
            status: "pending".to_string(),
            email: None,
            error: None,
            created_at: now,
            expires_at: now + GOOGLE_OAUTH_TICKET_TTL_SECS,
        };

        if let Ok(mut tickets) = self.oauth_tickets.write() {
            tickets.insert(state_id.clone(), ticket);
        }

        let auth_url = build_google_authorize_url(redirect_uri, &state_id);

        GoogleOAuthStartResponse {
            ok: true,
            state: state_id,
            authorize_url: auth_url.clone(),
            authorization_url: auth_url,
            redirect_uri: redirect_uri.to_string(),
            expires_in: GOOGLE_OAUTH_TICKET_TTL_SECS as u64,
        }
    }

    pub fn validate_and_consume_state(&self, state: Option<&str>) -> Result<String, AppError> {
        self.clean_expired_tickets();
        let now = current_time_secs();
        let mut tickets = self
            .oauth_tickets
            .write()
            .map_err(|e| AppError::Internal(format!("OAuth lock error: {e}")))?;

        let state_key = if let Some(s) = state.filter(|s| !s.trim().is_empty()) {
            let s_trimmed = s.trim();
            if let Some(t) = tickets.get(s_trimmed) {
                if t.expires_at > now {
                    s_trimmed.to_string()
                } else {
                    return Err(AppError::BadRequest(
                        "Phiên đăng nhập Google OAuth đã hết hạn. Vui lòng thử lại.".to_string(),
                    ));
                }
            } else if let Some(matching_key) = tickets
                .keys()
                .find(|k| {
                    *k == s_trimmed
                        || k.ends_with(&format!("|{s_trimmed}"))
                        || s_trimmed.ends_with(&format!("|{k}"))
                })
                .cloned()
            {
                let t = &tickets[&matching_key];
                if t.expires_at > now {
                    matching_key
                } else {
                    return Err(AppError::BadRequest(
                        "Phiên đăng nhập Google OAuth đã hết hạn. Vui lòng thử lại.".to_string(),
                    ));
                }
            } else {
                return Err(AppError::BadRequest(
                    "Mã state Google OAuth không hợp lệ hoặc không tìm thấy. Vui lòng thử lại."
                        .to_string(),
                ));
            }
        } else {
            let found = tickets
                .iter()
                .filter(|(_, t)| t.status == "pending" && t.expires_at > now)
                .max_by(|a, b| {
                    a.1.created_at
                        .partial_cmp(&b.1.created_at)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(k, _)| k.clone());

            found.ok_or_else(|| {
                AppError::BadRequest(
                    "Không tìm thấy phiên Google OAuth đang chờ xác thực. Vui lòng bắt đầu lại."
                        .to_string(),
                )
            })?
        };

        let ticket = tickets.remove(&state_key).unwrap();
        Ok(ticket.redirect_uri)
    }

    pub fn load_from_db(&self) -> Result<usize, rusqlite::Error> {
        let now = current_time_secs();
        let _ = self.db.cleanup_stale_errors(now)?;
        let records = self.db.list_accounts()?;
        let mut map = self.accounts.write().unwrap();
        map.clear();
        let count = records.len();
        for rec in records {
            if rec.is_active {
                map.insert(
                    rec.id.clone(),
                    AccountItem {
                        record: rec,
                        recent_requests: VecDeque::new(),
                        quota_cache: serde_json::json!({}),
                        quota_last_attempt: 0.0,
                        quota_lock: Arc::new(tokio::sync::Mutex::new(())),
                        in_flight: Arc::new(AtomicUsize::new(0)),
                    },
                );
            }
        }
        Ok(count)
    }

    pub fn account_count(&self) -> usize {
        self.accounts.read().unwrap().len()
    }

    pub fn db(&self) -> &Arc<Database> {
        &self.db
    }

    pub fn refresher(&self) -> &Arc<dyn TokenRefresher> {
        &self.refresher
    }

    pub fn quota_fetcher(&self) -> &Arc<dyn GoogleQuotaFetcher> {
        &self.quota_fetcher
    }

    pub async fn refresh_account_token(&self, account_id: &str) -> Result<String, String> {
        let raw_acc = self
            .db
            .get_account_by_id(account_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Account '{account_id}' not found"))?;
        let now = current_time_secs();
        let out = self.refresher.refresh_token(&raw_acc.refresh_token).await?;
        let _ = self.db.update_account_tokens(
            account_id,
            &out.access_token,
            now + out.expires_in_secs as f64,
        );
        let mut map = self.accounts.write().unwrap();
        if let Some(item) = map.get_mut(account_id) {
            item.record.access_token = Some(out.access_token.clone());
            item.record.expires_at = now + out.expires_in_secs as f64;
            item.record.error_count = 0;
        }
        Ok(out.access_token)
    }

    pub fn list_accounts(&self) -> Vec<AccountResponse> {
        let now = current_time_secs();
        let map = self.accounts.read().unwrap();
        let mut result: Vec<AccountResponse> = map
            .values()
            .map(|item| {
                let recent_rpm = item
                    .recent_requests
                    .iter()
                    .filter(|&&t| t > now - 60.0)
                    .count();
                AccountResponse {
                    id: item.record.id.clone(),
                    email: item.record.email.clone(),
                    is_active: item.record.is_active,
                    cooldown_remaining: (item.record.cooldown_until - now).max(0.0),
                    last_used_ago: if item.record.last_used_at > 0.0 {
                        Some((now - item.record.last_used_at).max(0.0))
                    } else {
                        None
                    },
                    total_requests: item.record.total_requests,
                    error_count: item.record.error_count,
                    last_error: item.record.last_error.clone(),
                    token_valid: item.record.expires_at > now,
                    recent_rpm,
                    quota: item.quota_cache.clone(),
                }
            })
            .collect();
        result.sort_by(|a, b| a.email.cmp(&b.email).then_with(|| a.id.cmp(&b.id)));
        result
    }

    pub fn get_account(&self, account_id: &str) -> Option<AccountResponse> {
        let now = current_time_secs();
        let map = self.accounts.read().unwrap();
        map.get(account_id).map(|item| {
            let recent_rpm = item
                .recent_requests
                .iter()
                .filter(|&&t| t > now - 60.0)
                .count();
            AccountResponse {
                id: item.record.id.clone(),
                email: item.record.email.clone(),
                is_active: item.record.is_active,
                cooldown_remaining: (item.record.cooldown_until - now).max(0.0),
                last_used_ago: if item.record.last_used_at > 0.0 {
                    Some((now - item.record.last_used_at).max(0.0))
                } else {
                    None
                },
                total_requests: item.record.total_requests,
                error_count: item.record.error_count,
                last_error: item.record.last_error.clone(),
                token_valid: item.record.expires_at > now,
                recent_rpm,
                quota: item.quota_cache.clone(),
            }
        })
    }

    pub fn add_account(
        &self,
        email: &str,
        refresh_token: &str,
    ) -> Result<AccountRecord, rusqlite::Error> {
        let record = self.db.create_account(None, email, refresh_token)?;
        let item = AccountItem {
            record: record.clone(),
            recent_requests: VecDeque::new(),
            quota_cache: serde_json::json!({}),
            quota_last_attempt: 0.0,
            quota_lock: Arc::new(tokio::sync::Mutex::new(())),
            in_flight: Arc::new(AtomicUsize::new(0)),
        };
        let mut map = self.accounts.write().unwrap();
        map.insert(record.id.clone(), item);
        Ok(record)
    }

    pub fn upsert_account(
        &self,
        email: &str,
        refresh_token: &str,
    ) -> Result<(AccountRecord, bool), AppError> {
        let existing = self.db.get_account_by_email(email)?;
        if let Some(mut acc) = existing {
            acc.refresh_token = refresh_token.to_string();
            acc.is_active = true;
            acc.cooldown_until = 0.0;
            acc.error_count = 0;
            acc.last_error = None;
            self.db.save_account(&acc)?;
            let mut map = self.accounts.write().unwrap();
            if let Some(item) = map.get_mut(&acc.id) {
                item.record = acc.clone();
            } else {
                map.insert(
                    acc.id.clone(),
                    AccountItem {
                        record: acc.clone(),
                        recent_requests: VecDeque::new(),
                        quota_cache: serde_json::json!({}),
                        quota_last_attempt: 0.0,
                        quota_lock: Arc::new(tokio::sync::Mutex::new(())),
                        in_flight: Arc::new(AtomicUsize::new(0)),
                    },
                );
            }
            Ok((acc, false))
        } else {
            let record = self.add_account(email, refresh_token)?;
            Ok((record, true))
        }
    }

    pub async fn exchange_oauth_code(
        &self,
        code: &str,
        redirect_uri: &str,
        token_url_override: Option<&str>,
        userinfo_url_override: Option<&str>,
    ) -> Result<(String, String, bool), AppError> {
        let token_url = token_url_override
            .map(|s| s.to_string())
            .or_else(|| std::env::var("AG_TOKEN_URL").ok())
            .unwrap_or_else(|| AG_TOKEN_URL.to_string());

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let client_secret = google_client_secret().map_err(AppError::Internal)?;
        let params = [
            ("client_id", AG_CLIENT_ID),
            ("client_secret", client_secret.as_str()),
            ("code", code),
            ("grant_type", "authorization_code"),
            ("redirect_uri", redirect_uri),
        ];

        let res = client
            .post(&token_url)
            .form(&params)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Google token exchange network error: {e}")))?;

        if !res.status().is_success() {
            let text = res.text().await.unwrap_or_default();
            let sanitized = if let Ok(err_val) = serde_json::from_str::<serde_json::Value>(&text) {
                let err_name = err_val["error"].as_str().unwrap_or("unknown_error");
                let err_desc = err_val["error_description"].as_str().unwrap_or("");
                if !err_desc.is_empty() {
                    format!("{err_name}: {err_desc}")
                } else {
                    err_name.to_string()
                }
            } else {
                "Google OAuth authorization exchange failed".to_string()
            };
            return Err(AppError::BadRequest(format!(
                "Google token exchange failed: {sanitized}"
            )));
        }

        let token_data: serde_json::Value = res
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to parse Google token JSON: {e}")))?;

        let refresh_token = token_data["refresh_token"].as_str().ok_or_else(|| {
            AppError::BadRequest("No refresh_token received from Google".to_string())
        })?;

        let access_token = token_data["access_token"].as_str();

        let mut email = "unknown@gmail.com".to_string();
        if let Some(at) = access_token {
            let userinfo_url = userinfo_url_override
                .map(|s| s.to_string())
                .or_else(|| std::env::var("AG_USERINFO_URL").ok())
                .unwrap_or_else(|| AG_USERINFO_URL.to_string());

            if let Ok(u_res) = client
                .get(&userinfo_url)
                .header("Authorization", format!("Bearer {at}"))
                .send()
                .await
            {
                if u_res.status().is_success() {
                    if let Ok(u_json) = u_res.json::<serde_json::Value>().await {
                        if let Some(em) = u_json["email"].as_str() {
                            email = em.to_string();
                        }
                    }
                }
            }
        }

        let (record, is_new) = self.upsert_account(&email, refresh_token)?;
        Ok((record.id, email, is_new))
    }

    pub fn remove_account(&self, account_id: &str) -> Result<bool, rusqlite::Error> {
        let deleted = self.db.delete_account(account_id)?;
        let mut map = self.accounts.write().unwrap();
        map.remove(account_id);
        Ok(deleted)
    }

    pub fn reset_cooldown(&self, account_id: &str) -> Result<bool, rusqlite::Error> {
        let updated = self.db.reset_account_cooldown(account_id)?;
        if updated {
            let mut map = self.accounts.write().unwrap();
            if let Some(item) = map.get_mut(account_id) {
                item.record.cooldown_until = 0.0;
                item.record.error_count = 0;
                item.record.last_error = None;
            }
        }
        Ok(updated)
    }

    pub fn set_quota_cache(&self, account_id: &str, quota: serde_json::Value) {
        let mut map = self.accounts.write().unwrap();
        if let Some(item) = map.get_mut(account_id) {
            item.quota_cache = quota;
            item.quota_last_attempt = current_time_secs();
        }
    }

    pub fn get_quota_last_attempt(&self, account_id: &str) -> Option<f64> {
        let map = self.accounts.read().unwrap();
        map.get(account_id).map(|item| item.quota_last_attempt)
    }

    pub fn set_quota_last_attempt(&self, account_id: &str, timestamp: f64) {
        let mut map = self.accounts.write().unwrap();
        if let Some(item) = map.get_mut(account_id) {
            item.quota_last_attempt = timestamp;
        }
    }

    pub fn set_account_active(&self, account_id: &str, is_active: bool) {
        let mut map = self.accounts.write().unwrap();
        if let Some(item) = map.get_mut(account_id) {
            item.record.is_active = is_active;
        }
    }

    pub fn set_account_cooldown(&self, account_id: &str, cooldown_until: f64) {
        let mut map = self.accounts.write().unwrap();
        if let Some(item) = map.get_mut(account_id) {
            item.record.cooldown_until = cooldown_until;
        }
    }

    pub async fn ensure_access_token_for_id(&self, account_id: &str) -> Result<String, String> {
        let now = current_time_secs();
        {
            let map = self.accounts.read().unwrap();
            if let Some(item) = map.get(account_id) {
                if let Some(ref tok) = item.record.access_token {
                    if item.record.expires_at > now + 300.0 && !tok.is_empty() {
                        return Ok(tok.clone());
                    }
                }
            } else {
                return Err(format!("Account '{account_id}' not found"));
            }
        }
        self.refresh_account_token(account_id).await
    }

    pub fn get_cached_quota(&self, account_id: &str) -> serde_json::Value {
        let map = self.accounts.read().unwrap();
        map.get(account_id)
            .map(|item| item.quota_cache.clone())
            .unwrap_or_else(|| serde_json::json!({}))
    }

    pub async fn refresh_quota(
        &self,
        account_id: &str,
        force: bool,
    ) -> Result<serde_json::Value, AppError> {
        let now = current_time_secs();
        let (quota_lock, cached_quota, last_attempt) = {
            let map = self.accounts.read().unwrap();
            let item = map
                .get(account_id)
                .ok_or_else(|| AppError::NotFound(format!("Account '{account_id}' not found")))?;
            (
                item.quota_lock.clone(),
                item.quota_cache.clone(),
                item.quota_last_attempt,
            )
        };

        if !force {
            let fetched_at = cached_quota
                .get("fetched_at")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            if fetched_at > 0.0 && now - fetched_at < QUOTA_CACHE_TTL_SECS {
                return Ok(cached_quota);
            }
            if last_attempt > 0.0 && now - last_attempt < QUOTA_CACHE_TTL_SECS {
                return Ok(cached_quota);
            }
        }

        let _guard = quota_lock.lock().await;

        let now = current_time_secs();
        if !force {
            let map = self.accounts.read().unwrap();
            if let Some(item) = map.get(account_id) {
                let fetched_at = item
                    .quota_cache
                    .get("fetched_at")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                if fetched_at > 0.0 && now - fetched_at < QUOTA_CACHE_TTL_SECS {
                    return Ok(item.quota_cache.clone());
                }
                if item.quota_last_attempt > 0.0
                    && now - item.quota_last_attempt < QUOTA_CACHE_TTL_SECS
                {
                    return Ok(item.quota_cache.clone());
                }
            }
        }

        self.set_quota_last_attempt(account_id, now);

        let access_token = match self.ensure_access_token_for_id(account_id).await {
            Ok(tok) => tok,
            Err(e) => {
                tracing::warn!(
                    "Failed to obtain access token for Google account '{account_id}': {e}"
                );
                let cached = self.get_cached_quota(account_id);
                if !cached.is_null() && cached.as_object().map(|o| !o.is_empty()).unwrap_or(false) {
                    return Ok(cached);
                }
                return Err(AppError::BadGateway(format!("Token refresh failed: {e}")));
            }
        };

        match self.quota_fetcher.fetch_quota(&access_token).await {
            Ok(quota) => {
                self.set_quota_cache(account_id, quota.clone());
                Ok(quota)
            }
            Err(e) => {
                let masked = crate::quota_refresh::sanitize_error_message(&e);
                tracing::warn!("Google quota fetch failed for account '{account_id}': {masked}");
                let _ = self.mark_error(account_id, &masked, false);
                let cached = self.get_cached_quota(account_id);
                if !cached.is_null() && cached.as_object().map(|o| !o.is_empty()).unwrap_or(false) {
                    return Ok(cached);
                }
                Err(AppError::BadGateway(format!(
                    "Google quota fetch failed: {masked}"
                )))
            }
        }
    }

    pub async fn test_account(
        &self,
        account_id: &str,
        _model_id: Option<&str>,
    ) -> Result<(bool, u64, String), AppError> {
        let _ = self
            .get_account(account_id)
            .ok_or_else(|| AppError::NotFound(format!("Account '{account_id}' not found")))?;

        let start = std::time::Instant::now();
        let raw_acc = self
            .db
            .get_account_by_id(account_id)?
            .ok_or_else(|| AppError::NotFound(format!("Account '{account_id}' not found")))?;

        let now = current_time_secs();
        if raw_acc.expires_at <= now + 300.0 {
            match self.refresher.refresh_token(&raw_acc.refresh_token).await {
                Ok(out) => {
                    let _ = self.db.update_account_tokens(
                        account_id,
                        &out.access_token,
                        now + out.expires_in_secs as f64,
                    );
                    let mut map = self.accounts.write().unwrap();
                    if let Some(item) = map.get_mut(account_id) {
                        item.record.access_token = Some(out.access_token);
                        item.record.expires_at = now + out.expires_in_secs as f64;
                        item.record.error_count = 0;
                    }
                }
                Err(err) => {
                    let _ = self.mark_error(account_id, &err, false);
                    return Err(AppError::BadGateway(format!(
                        "Không thể refresh OAuth token: {err}"
                    )));
                }
            }
        }

        let latency_ms = (start.elapsed().as_millis() as u64).max(1);
        let _ = self.mark_used(account_id);
        Ok((true, latency_ms, "Account hoạt động tốt".to_string()))
    }

    pub fn import_from_9router(
        &self,
        custom_path: Option<&std::path::Path>,
    ) -> Result<(usize, usize), AppError> {
        let db_path = match custom_path {
            Some(p) => p.to_path_buf(),
            None => {
                if let Ok(p) = std::env::var("NROUTER_DB_PATH") {
                    std::path::PathBuf::from(p)
                } else {
                    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                    std::path::PathBuf::from(home)
                        .join(".9router")
                        .join("db")
                        .join("data.sqlite")
                }
            }
        };

        if !db_path.exists() {
            return Ok((0, self.account_count()));
        }

        let conn = match rusqlite::Connection::open_with_flags(
            &db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        ) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Failed to open 9router sqlite db: {e}");
                return Ok((0, self.account_count()));
            }
        };

        let table_exists: bool = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='providerConnections'",
                [],
                |_| Ok(true),
            )
            .unwrap_or(false);

        if !table_exists {
            return Ok((0, self.account_count()));
        }

        let mut stmt = match conn.prepare(
            "SELECT email, data FROM providerConnections WHERE provider = 'antigravity' AND isActive = 1",
        ) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("Failed to prepare 9router query: {e}");
                return Ok((0, self.account_count()));
            }
        };

        let rows = stmt
            .query_map([], |row| -> rusqlite::Result<(String, String)> {
                let email: String = row.get(0)?;
                let raw_data: String = row.get(1)?;
                Ok((email, raw_data))
            })
            .map_err(|e| AppError::Internal(format!("Failed to query 9router rows: {e}")))?;

        let mut added = 0;
        for (email, raw_data) in rows.flatten() {
            let email = email.trim();
            if email.is_empty() {
                continue;
            }
            // Check if in memory
            {
                let map = self.accounts.read().unwrap();
                if map.values().any(|a| a.record.email == email) {
                    continue;
                }
            }
            // Check if in DB
            if let Ok(Some(_)) = self.db.get_account_by_email(email) {
                continue;
            }

            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw_data) {
                if let Some(rt) = val.get("refreshToken").and_then(|v| v.as_str()) {
                    let rt = rt.trim();
                    if !rt.is_empty() && self.add_account(email, rt).is_ok() {
                        added += 1;
                    }
                }
            }
        }

        Ok((added, self.account_count()))
    }

    pub fn mark_used(&self, account_id: &str) -> Result<(), rusqlite::Error> {
        let now = current_time_secs();
        {
            let mut map = self.accounts.write().unwrap();
            if let Some(item) = map.get_mut(account_id) {
                item.record.last_used_at = now;
                item.record.total_requests += 1;
                item.record.last_error = None;
                item.record.error_count = 0;
                item.recent_requests.push_back(now);
                while let Some(&front) = item.recent_requests.front() {
                    if front <= now - 60.0 {
                        item.recent_requests.pop_front();
                    } else {
                        break;
                    }
                }
            }
        }
        self.db.record_account_success(account_id, now)
    }

    pub fn mark_error(
        &self,
        account_id: &str,
        error: &str,
        is_ban: bool,
    ) -> Result<(), rusqlite::Error> {
        let cooldown_secs = if is_ban {
            3600.0
        } else if error.contains("429") || error.to_lowercase().contains("rate") {
            120.0
        } else {
            60.0
        };
        self.mark_error_with_cooldown(account_id, error, cooldown_secs)
    }

    pub fn mark_error_with_cooldown(
        &self,
        account_id: &str,
        error: &str,
        cooldown_secs: f64,
    ) -> Result<(), rusqlite::Error> {
        let now = current_time_secs();
        let cooldown_until = now + cooldown_secs;

        {
            let mut map = self.accounts.write().unwrap();
            if let Some(item) = map.get_mut(account_id) {
                item.record.error_count += 1;
                item.record.last_error = Some(error.to_string());
                item.record.cooldown_until = cooldown_until;
            }
        }
        self.db
            .record_account_error(account_id, error, cooldown_until)
    }

    pub fn get_stats_summary(&self, now: f64) -> (usize, usize, usize, f64, usize) {
        let map = self.accounts.read().unwrap();
        let total = map.len();
        let mut active = 0;
        let mut cooldown = 0;
        let mut live_rpm = 0;

        for item in map.values() {
            if item.record.is_active && item.record.cooldown_until <= now {
                active += 1;
            }
            if item.record.cooldown_until > now {
                cooldown += 1;
            }
            live_rpm += item
                .recent_requests
                .iter()
                .filter(|&&t| t > now - 60.0)
                .count();
        }

        let active_rate = if total > 0 {
            ((active as f64 / total as f64 * 100.0) * 10.0).round() / 10.0
        } else {
            0.0
        };

        (total, active, cooldown, active_rate, live_rpm)
    }

    pub fn peek_account(&self, model_id: Option<&str>) -> Option<AccountRecord> {
        let now = current_time_secs();
        let map = self.accounts.read().unwrap();
        let mut candidates: Vec<AccountItem> = map
            .values()
            .filter(|item| item.record.is_active && item.record.cooldown_until <= now)
            .cloned()
            .collect();
        if candidates.is_empty() {
            return None;
        }

        if let Some(model) = model_id {
            let m_lower = model.to_lowercase();
            if m_lower.contains("claude") || m_lower.contains("gpt") {
                let mut with_quota = Vec::new();
                for item in &candidates {
                    if let Some(claude) = item.quota_cache.get("claude_5h") {
                        let remaining = claude
                            .get("remaining_percent")
                            .and_then(|v| v.as_f64())
                            .unwrap_or(0.0);
                        if remaining > 0.0 {
                            with_quota.push(item.clone());
                        }
                    }
                }
                if !with_quota.is_empty() {
                    candidates = with_quota;
                }
            }
        }

        candidates.sort_by(|a, b| {
            a.record
                .last_used_at
                .partial_cmp(&b.record.last_used_at)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.record.id.cmp(&b.record.id))
        });

        candidates.first().map(|c| c.record.clone())
    }

    pub async fn acquire_lease(
        self: &Arc<Self>,
        model_id: Option<&str>,
    ) -> Option<GoogleAccountLease> {
        let now = current_time_secs();
        let candidates: Vec<AccountItem> = {
            let map = self.accounts.read().unwrap();
            map.values()
                .filter(|item| {
                    if !item.record.is_active {
                        return false;
                    }
                    if item.record.cooldown_until > now {
                        return false;
                    }
                    if item.in_flight.load(Ordering::SeqCst) >= self.max_concurrency {
                        return false;
                    }
                    let rpm = item
                        .recent_requests
                        .iter()
                        .filter(|&&t| t > now - 60.0)
                        .count();
                    if rpm >= self.max_rpm {
                        return false;
                    }
                    if self.min_gap_secs > 0.0
                        && (now - item.record.last_used_at) < self.min_gap_secs
                    {
                        return false;
                    }
                    true
                })
                .cloned()
                .collect()
        };

        if candidates.is_empty() {
            return None;
        }

        let mut sorted = candidates;

        if let Some(model) = model_id {
            let m_lower = model.to_lowercase();
            if m_lower.contains("claude") || m_lower.contains("gpt") {
                let mut with_quota = Vec::new();
                let mut without_quota = Vec::new();

                for item in &sorted {
                    if let Some(claude) = item.quota_cache.get("claude_5h") {
                        let remaining = claude
                            .get("remaining_percent")
                            .and_then(|v| v.as_f64())
                            .unwrap_or(0.0);
                        if remaining > 0.0 {
                            with_quota.push(item.clone());
                        }
                    } else {
                        without_quota.push(item.clone());
                    }
                }

                sorted = if !with_quota.is_empty() {
                    with_quota
                } else {
                    without_quota
                };

                if sorted.is_empty() {
                    return None;
                }
            }
        }

        sorted.sort_by(|a, b| {
            a.record
                .last_used_at
                .partial_cmp(&b.record.last_used_at)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.record.id.cmp(&b.record.id))
        });

        const TOKEN_REFRESH_LEAD_SECS: f64 = 300.0;
        for candidate in sorted {
            if candidate.in_flight.load(Ordering::SeqCst) >= self.max_concurrency {
                continue;
            }

            if candidate.record.access_token.is_some()
                && candidate.record.expires_at > now + TOKEN_REFRESH_LEAD_SECS
            {
                candidate.in_flight.fetch_add(1, Ordering::SeqCst);
                return Some(GoogleAccountLease::new(
                    self.clone(),
                    candidate.record,
                    candidate.in_flight.clone(),
                ));
            }

            match self
                .refresher
                .refresh_token(&candidate.record.refresh_token)
                .await
            {
                Ok(out) => {
                    let mut updated = candidate.record.clone();
                    updated.access_token = Some(out.access_token);
                    updated.expires_at = now + out.expires_in_secs as f64;
                    updated.error_count = 0;
                    let _ = self.db.save_account(&updated);
                    {
                        let mut map = self.accounts.write().unwrap();
                        if let Some(item) = map.get_mut(&updated.id) {
                            item.record = updated.clone();
                        }
                    }
                    candidate.in_flight.fetch_add(1, Ordering::SeqCst);
                    return Some(GoogleAccountLease::new(
                        self.clone(),
                        updated,
                        candidate.in_flight.clone(),
                    ));
                }
                Err(err) => {
                    let _ = self.mark_error(&candidate.record.id, &err, false);
                    continue;
                }
            }
        }

        None
    }

    pub async fn acquire_lease_with_pacing(
        self: &Arc<Self>,
        model_id: Option<&str>,
    ) -> Option<GoogleAccountLease> {
        if let Some(lease) = self.acquire_lease(model_id).await {
            return Some(lease);
        }

        if self.min_gap_secs > 0.0 {
            let now = current_time_secs();
            let shortest_wait = {
                let map = self.accounts.read().unwrap();
                let mut min_w: Option<f64> = None;
                for item in map.values() {
                    if !item.record.is_active || item.record.cooldown_until > now {
                        continue;
                    }
                    if item.in_flight.load(Ordering::SeqCst) >= self.max_concurrency {
                        continue;
                    }
                    let gap_rem = self.min_gap_secs - (now - item.record.last_used_at);
                    if gap_rem > 0.0 && gap_rem <= self.min_gap_secs.max(2.0) {
                        min_w = Some(min_w.map(|w: f64| w.min(gap_rem)).unwrap_or(gap_rem));
                    }
                }
                min_w
            };

            if let Some(wait_secs) = shortest_wait {
                tokio::time::sleep(tokio::time::Duration::from_secs_f64(wait_secs)).await;
                return self.acquire_lease(model_id).await;
            }
        }

        None
    }

    pub async fn pick_account(&self, model_id: Option<&str>) -> Option<AccountRecord> {
        let now = current_time_secs();
        let candidates: Vec<AccountItem> = {
            let map = self.accounts.read().unwrap();
            map.values()
                .filter(|item| {
                    if !item.record.is_active {
                        return false;
                    }
                    if item.record.cooldown_until > now {
                        return false;
                    }
                    let rpm = item
                        .recent_requests
                        .iter()
                        .filter(|&&t| t > now - 60.0)
                        .count();
                    if rpm >= self.max_rpm {
                        return false;
                    }
                    if self.min_gap_secs > 0.0
                        && (now - item.record.last_used_at) < self.min_gap_secs
                    {
                        return false;
                    }
                    true
                })
                .cloned()
                .collect()
        };

        if candidates.is_empty() {
            return None;
        }

        let mut sorted = candidates;

        // Model-specific quota filtering (e.g. claude / gpt)
        if let Some(model) = model_id {
            let m_lower = model.to_lowercase();
            if m_lower.contains("claude") || m_lower.contains("gpt") {
                let mut with_quota = Vec::new();
                let mut without_quota = Vec::new();

                for item in &sorted {
                    if let Some(claude) = item.quota_cache.get("claude_5h") {
                        let remaining = claude
                            .get("remaining_percent")
                            .and_then(|v| v.as_f64())
                            .unwrap_or(0.0);
                        if remaining > 0.0 {
                            with_quota.push(item.clone());
                        }
                    } else {
                        without_quota.push(item.clone());
                    }
                }

                sorted = if !with_quota.is_empty() {
                    with_quota
                } else {
                    without_quota
                };

                if sorted.is_empty() {
                    return None;
                }
            }
        }

        // Deterministic LRU / Round-Robin ordering:
        // Primary: last_used_at ASC
        // Secondary: id ASC
        sorted.sort_by(|a, b| {
            a.record
                .last_used_at
                .partial_cmp(&b.record.last_used_at)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.record.id.cmp(&b.record.id))
        });

        const TOKEN_REFRESH_LEAD_SECS: f64 = 300.0;
        for candidate in sorted {
            if candidate.record.access_token.is_some()
                && candidate.record.expires_at > now + TOKEN_REFRESH_LEAD_SECS
            {
                return Some(candidate.record);
            }

            // Needs token refresh
            match self
                .refresher
                .refresh_token(&candidate.record.refresh_token)
                .await
            {
                Ok(out) => {
                    let mut updated = candidate.record.clone();
                    updated.access_token = Some(out.access_token);
                    updated.expires_at = now + out.expires_in_secs as f64;
                    updated.error_count = 0;
                    let _ = self.db.save_account(&updated);
                    {
                        let mut map = self.accounts.write().unwrap();
                        if let Some(item) = map.get_mut(&updated.id) {
                            item.record = updated.clone();
                        }
                    }
                    return Some(updated);
                }
                Err(err) => {
                    let _ = self.mark_error(&candidate.record.id, &err, false);
                    continue;
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pool_deterministic_rotation_and_lru() {
        let db =
            Arc::new(Database::open_or_create(std::path::Path::new(":memory:"), None).unwrap());
        let pool = AccountPool::new(db.clone());

        let acc_a = pool.add_account("a@example.com", "tok-a").unwrap();
        let acc_b = pool.add_account("b@example.com", "tok-b").unwrap();

        // Initially both have last_used_at = 0. Deterministic tie-breaker selects smaller ID
        let first = pool
            .pick_account(None)
            .await
            .expect("should pick an account");
        let expected_first_id = if acc_a.id < acc_b.id {
            &acc_a.id
        } else {
            &acc_b.id
        };
        assert_eq!(&first.id, expected_first_id);

        // Mark first used
        pool.mark_used(&first.id).unwrap();

        // Second pick must be the other account (LRU)
        let second = pool
            .pick_account(None)
            .await
            .expect("should pick second account");
        let expected_second_id = if acc_a.id < acc_b.id {
            &acc_b.id
        } else {
            &acc_a.id
        };
        assert_eq!(&second.id, expected_second_id);

        // Mark second used
        pool.mark_used(&second.id).unwrap();

        // Next pick returns the first account again (since first last_used_at < second last_used_at)
        let third = pool.pick_account(None).await.expect("should rotate back");
        assert_eq!(&third.id, expected_first_id);
    }

    #[tokio::test]
    async fn test_cooldown_exclusion_and_reset() {
        let db =
            Arc::new(Database::open_or_create(std::path::Path::new(":memory:"), None).unwrap());
        let pool = AccountPool::new(db.clone());

        let acc_a = pool.add_account("a@example.com", "tok-a").unwrap();
        let acc_b = pool.add_account("b@example.com", "tok-b").unwrap();

        // Put A into cooldown
        pool.mark_error(&acc_a.id, "HTTP 429 Rate Limit", false)
            .unwrap();

        // Only B is candidate
        let picked = pool
            .pick_account(None)
            .await
            .expect("should pick non-cooldown account");
        assert_eq!(picked.id, acc_b.id);

        // Put B into cooldown too
        pool.mark_error(&acc_b.id, "Banned", true).unwrap();

        // Now both are in cooldown -> returns None
        assert!(pool.pick_account(None).await.is_none());

        // Reset cooldown on A
        let reset = pool.reset_cooldown(&acc_a.id).unwrap();
        assert!(reset);

        // A is eligible again
        let picked_after_reset = pool
            .pick_account(None)
            .await
            .expect("A is eligible after reset");
        assert_eq!(picked_after_reset.id, acc_a.id);
        assert_eq!(picked_after_reset.cooldown_until, 0.0);
        assert_eq!(picked_after_reset.error_count, 0);
    }

    #[tokio::test]
    async fn test_token_refresh_hook() {
        let db =
            Arc::new(Database::open_or_create(std::path::Path::new(":memory:"), None).unwrap());
        let mock_refresher = Arc::new(MockTokenRefresher::with_token("refreshed-token-123"));
        let pool = AccountPool::with_refresher(db.clone(), mock_refresher.clone());

        let acc = pool
            .add_account("user@example.com", "refresh-token-val")
            .unwrap();
        assert_eq!(acc.access_token, None);

        // Pick account triggers refresh
        let picked = pool
            .pick_account(None)
            .await
            .expect("should pick and refresh");
        assert_eq!(picked.access_token, Some("refreshed-token-123".to_string()));
        assert!(picked.expires_at > current_time_secs());

        // Test refresh failure puts account in cooldown
        mock_refresher.set_failing(true);
        // Force expiry
        let mut expired = picked.clone();
        expired.expires_at = 0.0;
        db.save_account(&expired).unwrap();
        pool.load_from_db().unwrap();

        assert!(pool.pick_account(None).await.is_none());
    }

    #[tokio::test]
    async fn test_quota_filtering_claude() {
        let db =
            Arc::new(Database::open_or_create(std::path::Path::new(":memory:"), None).unwrap());
        let pool = AccountPool::new(db.clone());

        let acc_a = pool.add_account("a@example.com", "tok-a").unwrap();
        let acc_b = pool.add_account("b@example.com", "tok-b").unwrap();

        // A has 0% claude quota, B has 50%
        pool.set_quota_cache(
            &acc_a.id,
            serde_json::json!({
                "claude_5h": { "remaining_percent": 0.0 }
            }),
        );
        pool.set_quota_cache(
            &acc_b.id,
            serde_json::json!({
                "claude_5h": { "remaining_percent": 50.0 }
            }),
        );

        // Requesting claude model should pick B
        let picked = pool
            .pick_account(Some("ag/claude-3-5-sonnet"))
            .await
            .expect("should pick B");
        assert_eq!(picked.id, acc_b.id);
    }

    #[tokio::test]
    async fn test_google_token_refresher_mock_http() {
        use axum::http::StatusCode;
        use axum::response::IntoResponse;
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = axum::Router::new().route(
            "/token",
            axum::routing::post(|body: String| async move {
                assert!(body.contains("grant_type=refresh_token"));
                assert!(body.contains("client_id="));
                assert!(body.contains("refresh_token=valid-rt"));
                (
                    StatusCode::OK,
                    [("content-type", "application/json")],
                    r#"{"access_token":"ya29.test-access-token","expires_in":1800}"#,
                )
                    .into_response()
            }),
        );
        tokio::spawn(async move {
            axum::serve(listener, server).await.unwrap();
        });

        let refresher = GoogleTokenRefresher::with_url(format!("http://127.0.0.1:{port}/token"));
        let res = refresher.refresh_token("valid-rt").await.unwrap();
        assert_eq!(res.access_token, "ya29.test-access-token");
        assert_eq!(res.expires_in_secs, 1800);
    }

    #[tokio::test]
    async fn test_google_token_refresher_oauth_error() {
        use axum::http::StatusCode;
        use axum::response::IntoResponse;
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = axum::Router::new().route(
            "/token",
            axum::routing::post(|| async move {
                (
                    StatusCode::BAD_REQUEST,
                    [("content-type", "application/json")],
                    r#"{"error":"invalid_grant","error_description":"Token has been expired or revoked."}"#,
                )
                    .into_response()
            }),
        );
        tokio::spawn(async move {
            axum::serve(listener, server).await.unwrap();
        });

        let refresher = GoogleTokenRefresher::with_url(format!("http://127.0.0.1:{port}/token"));
        let err = refresher.refresh_token("expired-rt").await.unwrap_err();
        assert!(err.contains("Token refresh failed 400"));
        assert!(err.contains("invalid_grant"));
    }

    #[test]
    fn test_build_google_authorize_url() {
        let url =
            build_google_authorize_url("http://localhost:20229/auth/callback", "test-state-123");
        assert!(url.starts_with(AG_AUTHORIZE_URL));
        assert!(url.contains("client_id="));
        assert!(url.contains("redirect_uri=http%3A%2F%2Flocalhost%3A20229%2Fauth%2Fcallback"));
        assert!(url.contains("state=test-state-123"));
        assert!(
            url.contains("prompt=select_account%20consent")
                || url.contains("prompt=select_account+consent")
        );
        assert!(url.contains("access_type=offline"));
    }

    #[test]
    fn test_account_upsert_and_isolation() {
        let db =
            Arc::new(Database::open_or_create(std::path::Path::new(":memory:"), None).unwrap());
        let pool = AccountPool::new(db);

        // First insert
        let (a1, is_new1) = pool
            .upsert_account("user@example.com", "rt-initial")
            .unwrap();
        assert_eq!(a1.email, "user@example.com");
        assert_eq!(a1.refresh_token, "rt-initial");
        assert!(is_new1);

        // Second upsert same email updates token
        let (a2, is_new2) = pool
            .upsert_account("user@example.com", "rt-updated")
            .unwrap();
        assert_eq!(a1.id, a2.id);
        assert_eq!(a2.refresh_token, "rt-updated");
        assert!(!is_new2);

        let list = pool.list_accounts();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].email, "user@example.com");
    }

    #[tokio::test]
    async fn test_google_oauth_exchange_mock_http() {
        use axum::http::StatusCode;
        use axum::response::IntoResponse;
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let server = axum::Router::new()
            .route(
                "/token",
                axum::routing::post(|| async move {
                    (
                        StatusCode::OK,
                        [("content-type", "application/json")],
                        r#"{"access_token":"mock-google-at","refresh_token":"mock-google-rt","expires_in":3600}"#,
                    )
                        .into_response()
                }),
            )
            .route(
                "/userinfo",
                axum::routing::get(|| async move {
                    (
                        StatusCode::OK,
                        [("content-type", "application/json")],
                        r#"{"email":"mock-authed@gmail.com","id":"123456"}"#,
                    )
                        .into_response()
                }),
            );

        tokio::spawn(async move {
            axum::serve(listener, server).await.unwrap();
        });

        let db =
            Arc::new(Database::open_or_create(std::path::Path::new(":memory:"), None).unwrap());
        let pool = AccountPool::new(db);

        let token_url = format!("http://127.0.0.1:{port}/token");
        let userinfo_url = format!("http://127.0.0.1:{port}/userinfo");

        let (aid, email, is_new) = pool
            .exchange_oauth_code(
                "mock-code-xyz",
                "http://localhost/auth/callback",
                Some(&token_url),
                Some(&userinfo_url),
            )
            .await
            .unwrap();

        assert_eq!(email, "mock-authed@gmail.com");
        assert!(is_new);
        let acc = pool.get_account(&aid).expect("Account saved in pool");
        assert_eq!(acc.email, "mock-authed@gmail.com");
    }

    #[test]
    fn test_parse_google_quota_summary_realistic() {
        let fixture = serde_json::json!({
            "groups": [
                {
                    "displayName": "Gemini Models",
                    "buckets": [
                        {
                            "bucketId": "gemini-weekly",
                            "displayName": "Weekly Limit Remaining",
                            "window": "weekly",
                            "resetTime": "2026-09-23T02:34:07Z",
                            "remainingFraction": 0.16550596
                        },
                        {
                            "bucketId": "gemini-5h",
                            "displayName": "Five Hour Limit Remaining",
                            "window": "5h",
                            "resetTime": "2026-09-23T04:14:53Z",
                            "remainingFraction": 0.4401904
                        }
                    ]
                },
                {
                    "displayName": "Claude and GPT models",
                    "buckets": [
                        {
                            "bucketId": "3p-weekly",
                            "displayName": "Weekly Limit Remaining",
                            "window": "weekly",
                            "resetTime": "2026-09-27T15:16:05Z",
                            "remainingFraction": 0.5391183
                        },
                        {
                            "bucketId": "3p-5h",
                            "displayName": "Five Hour Limit Remaining",
                            "window": "5h",
                            "resetTime": "2026-09-23T07:01:53Z",
                            "remainingFraction": 0.9997848
                        }
                    ]
                }
            ]
        });

        let parsed = parse_google_quota_summary(&fixture);
        assert!(parsed.get("fetched_at").is_some());

        let gem_5h = parsed.get("gemini_5h").unwrap();
        assert_eq!(gem_5h["remaining_percent"], 44.0);
        assert_eq!(gem_5h["reset_time"], "2026-09-23T04:14:53Z");
        assert_eq!(gem_5h["display_name"], "Gemini 5h");

        let gem_weekly = parsed.get("gemini_weekly").unwrap();
        assert_eq!(gem_weekly["remaining_percent"], 16.6);
        assert_eq!(gem_weekly["reset_time"], "2026-09-23T02:34:07Z");

        let claude_5h = parsed.get("claude_5h").unwrap();
        assert_eq!(claude_5h["remaining_percent"], 100.0);
        assert_eq!(claude_5h["reset_time"], "2026-09-23T07:01:53Z");

        let claude_weekly = parsed.get("claude_weekly").unwrap();
        assert_eq!(claude_weekly["remaining_percent"], 53.9);
        assert_eq!(claude_weekly["reset_time"], "2026-09-27T15:16:05Z");
    }

    #[tokio::test]
    async fn test_refresh_quota_caching_and_ttl() {
        let db =
            Arc::new(Database::open_or_create(std::path::Path::new(":memory:"), None).unwrap());
        let pool = AccountPool::new(db);
        let acc = pool.add_account("quota-ttl@example.com", "rt-ttl").unwrap();

        // 1. Initial refresh fetches quota
        let q1 = pool.refresh_quota(&acc.id, false).await.unwrap();
        let fetched_at1 = q1["fetched_at"].as_f64().unwrap();
        assert!(fetched_at1 > 0.0);

        // 2. Immediate second call without force should hit cache
        let q2 = pool.refresh_quota(&acc.id, false).await.unwrap();
        assert_eq!(q2["fetched_at"].as_f64().unwrap(), fetched_at1);

        // 3. Simulated expired cache
        let mut stale_q = q1.clone();
        stale_q["fetched_at"] = serde_json::json!(fetched_at1 - 300.0);
        pool.set_quota_cache(&acc.id, stale_q);
        pool.set_quota_last_attempt(&acc.id, 0.0);

        let q3 = pool.refresh_quota(&acc.id, false).await.unwrap();
        assert!(q3["fetched_at"].as_f64().unwrap() > fetched_at1 - 300.0);
    }

    #[tokio::test]
    async fn test_refresh_quota_multiple_accounts_distinct() {
        let db =
            Arc::new(Database::open_or_create(std::path::Path::new(":memory:"), None).unwrap());
        let pool = AccountPool::new(db);

        let acc1 = pool.add_account("acc1@example.com", "rt-1").unwrap();
        let acc2 = pool.add_account("acc2@example.com", "rt-2").unwrap();

        pool.set_quota_cache(
            &acc1.id,
            serde_json::json!({
                "gemini_5h": { "remaining_percent": 15.0, "reset_time": "2026-09-23T03:00:00Z" },
                "claude_5h": { "remaining_percent": 50.0, "reset_time": "2026-09-23T04:00:00Z" },
                "fetched_at": current_time_secs()
            }),
        );

        pool.set_quota_cache(
            &acc2.id,
            serde_json::json!({
                "gemini_5h": { "remaining_percent": 85.0, "reset_time": "2026-09-23T05:00:00Z" },
                "claude_5h": { "remaining_percent": 100.0, "reset_time": "2026-09-23T06:00:00Z" },
                "fetched_at": current_time_secs()
            }),
        );

        let q1 = pool.get_account(&acc1.id).unwrap().quota;
        let q2 = pool.get_account(&acc2.id).unwrap().quota;

        assert_eq!(q1["gemini_5h"]["remaining_percent"], 15.0);
        assert_eq!(q2["gemini_5h"]["remaining_percent"], 85.0);
    }

    #[tokio::test]
    async fn test_google_account_lease_concurrency_and_cancellation_safety() {
        let db =
            Arc::new(Database::open_or_create(std::path::Path::new(":memory:"), None).unwrap());
        let mut pool_inner = AccountPool::with_components(
            db,
            Arc::new(MockTokenRefresher::with_token("test-tok")),
            Arc::new(MockGoogleQuotaFetcher::default()),
            0.0,
            60,
        );
        pool_inner.max_concurrency = 1;
        let pool = Arc::new(pool_inner);
        let _acc = pool.add_account("user@example.com", "rt").unwrap();

        // 1. Acquire lease 1
        let lease1 = pool
            .acquire_lease(None)
            .await
            .expect("lease 1 should succeed");
        assert_eq!(lease1.account.email, "user@example.com");

        // 2. Second concurrent acquire must fail due to max_concurrency = 1
        assert!(pool.acquire_lease(None).await.is_none());

        // 3. Drop lease1 (simulates client disconnect / cancellation)
        drop(lease1);

        // 4. Concurrency slot is immediately freed, account not put in cooldown
        let acc_record = pool.get_account(&_acc.id).unwrap();
        assert_eq!(acc_record.error_count, 0);
        assert_eq!(acc_record.cooldown_remaining, 0.0);

        // 5. Subsequent acquire succeeds
        let mut lease2 = pool
            .acquire_lease(None)
            .await
            .expect("lease 2 should succeed");
        lease2.commit_success();

        let acc_record_after = pool.get_account(&_acc.id).unwrap();
        assert_eq!(acc_record_after.total_requests, 1);
    }

    #[tokio::test]
    async fn test_google_account_rate_limit_rpm_guardrail() {
        let db =
            Arc::new(Database::open_or_create(std::path::Path::new(":memory:"), None).unwrap());
        let pool = Arc::new(AccountPool::with_components(
            db,
            Arc::new(MockTokenRefresher::with_token("test-tok")),
            Arc::new(MockGoogleQuotaFetcher::default()),
            0.0,
            2, // max_rpm = 2
        ));
        let _acc = pool.add_account("rpm@example.com", "rt").unwrap();

        let mut l1 = pool.acquire_lease(None).await.expect("l1");
        l1.commit_success();
        let mut l2 = pool.acquire_lease(None).await.expect("l2");
        l2.commit_success();

        // 3rd request in same minute should be rejected by max_rpm = 2
        assert!(pool.acquire_lease(None).await.is_none());

        let (total, active, cooldown, _, live_rpm) = pool.get_stats_summary(current_time_secs());
        assert_eq!(total, 1);
        assert_eq!(active, 1);
        assert_eq!(cooldown, 0);
        assert_eq!(live_rpm, 2);
    }
}
