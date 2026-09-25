use std::collections::{HashMap, VecDeque};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::task::{Context, Poll};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use bytes::Bytes;
use chrono::Utc;
use futures_util::stream::Stream;
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE, USER_AGENT};
use serde::{Deserialize, Serialize};
use tracing::{error, warn};

use crate::db::{CodexAccountRecord, Database};
use crate::error::AppError;
use crate::provider::{
    parse_retry_after, BoxChatStream, ChatChoice, ChatChoiceMessage, ChatCompletionRequest,
    ChatCompletionResponse, ChatMessage, MessageContent, Provider, UsageInfo, MAX_SSE_BUFFER_BYTES,
};

pub const CODEX_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
pub const CODEX_TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
pub const CODEX_BASE_URL: &str = "https://chatgpt.com/backend-api";
pub const CODEX_RESPONSES_URL: &str = "https://chatgpt.com/backend-api/codex/responses";
pub const CODEX_USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";
pub const CODEX_AUTHORIZE_URL: &str = "https://auth.openai.com/oauth/authorize";
pub const CODEX_DEFAULT_REDIRECT_URI: &str = "http://localhost:1455/auth/callback";
pub const CODEX_OAUTH_REDIRECT_URI: &str = CODEX_DEFAULT_REDIRECT_URI;
pub const CODEX_OAUTH_SCOPES: &str = "openid profile email offline_access";
pub const CODEX_USER_AGENT: &str = "codex_cli_rs/0.148.0";
pub const CODEX_ORIGINATOR: &str = "codex_cli_rs";
pub const CODEX_MIN_GAP_SECS: f64 = 2.0;
pub const CODEX_MIN_GAP_S: f64 = CODEX_MIN_GAP_SECS;
pub const CODEX_MAX_RPM: usize = 15;
pub const CODEX_QUOTA_CACHE_TTL_SECS: f64 = 60.0;
pub const CODEX_QUOTA_STOP_PERCENT: f64 = 98.0;
pub const CODEX_TOKEN_REFRESH_LEAD_SECS: f64 = 300.0;
pub const CODEX_TOKEN_REFRESH_LEAD_S: f64 = CODEX_TOKEN_REFRESH_LEAD_SECS;
pub const CODEX_OAUTH_TICKET_TTL_SECS: f64 = 600.0;
pub const CODEX_OAUTH_TICKET_TTL_S: f64 = CODEX_OAUTH_TICKET_TTL_SECS;
pub const COOLDOWN_AFTER_ERROR_SECS: f64 = 60.0;
pub const CODEX_NATIVE_NON_STREAM_ENABLED: bool = false; // Phase 2 flag, OFF by default

fn current_time_secs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

pub fn mask_codex_error(value: &str) -> String {
    let text = value.replace(['\r', '\n'], " ").trim().to_string();
    let re_bearer = match Regex::new(r"(?i)\bbearer\s+([a-zA-Z0-9_\-\.]{6,})") {
        Ok(r) => r,
        Err(_) => return text,
    };
    let text = re_bearer.replace_all(&text, "bearer: ••••");
    let re = match Regex::new(r"(?i)\b(bearer|token|secret|code)\s*[:=]\s*\S+") {
        Ok(r) => r,
        Err(_) => return text.to_string(),
    };
    let replaced = re.replace_all(&text, "$1: ••••");
    let trimmed = replaced.trim();
    if trimmed.chars().count() > 180 {
        trimmed.chars().take(180).collect()
    } else {
        trimmed.to_string()
    }
}

pub fn generate_pkce_pair() -> (String, String) {
    let mut random_bytes = [0u8; 32];
    use ring::rand::SecureRandom;
    let rng = ring::rand::SystemRandom::new();
    if rng.fill(&mut random_bytes).is_err() {
        for (idx, b) in random_bytes.iter_mut().enumerate() {
            *b = ((current_time_secs() * 1000.0) as u64 ^ (idx as u64 * 0x5851f42d4c957f2d)) as u8;
        }
    }
    let verifier = URL_SAFE_NO_PAD.encode(random_bytes);
    let digest = ring::digest::digest(&ring::digest::SHA256, verifier.as_bytes());
    let challenge = URL_SAFE_NO_PAD.encode(digest.as_ref());
    (verifier, challenge)
}

pub fn jwt_claims_unverified(token: &str) -> serde_json::Value {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() < 2 {
        return serde_json::json!({});
    }
    let payload = parts[1].trim_end_matches('=');
    if let Ok(bytes) = URL_SAFE_NO_PAD.decode(payload.as_bytes()) {
        if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&bytes) {
            if val.is_object() {
                return val;
            }
        }
    }
    serde_json::json!({})
}

pub fn extract_codex_token_identity(tokens: &serde_json::Value) -> (String, String) {
    let id_token = tokens
        .get("id_token")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let access_token = tokens
        .get("access_token")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let id_claims = jwt_claims_unverified(id_token);
    let access_claims = jwt_claims_unverified(access_token);

    let profile = access_claims
        .get("https://api.openai.com/profile")
        .cloned()
        .unwrap_or(serde_json::json!({}));
    let auth = id_claims
        .get("https://api.openai.com/auth")
        .or_else(|| access_claims.get("https://api.openai.com/auth"))
        .cloned()
        .unwrap_or(serde_json::json!({}));

    let email = id_claims
        .get("email")
        .or_else(|| profile.get("email"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let account_id = tokens
        .get("account_id")
        .or_else(|| auth.get("chatgpt_account_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    (email, account_id)
}

pub use extract_codex_token_identity as extract_token_identity;

pub fn atomic_write_private_json(
    path: &Path,
    data: &serde_json::Value,
) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
        #[cfg(unix)]
        {
            let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
        }
    }
    let temp_name = format!(
        ".{}.tmp.{}",
        path.file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| "auth".to_string()),
        uuid::Uuid::new_v4()
    );
    let temp_path = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(temp_name);
    let content = serde_json::to_string_pretty(data)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

    std::fs::write(&temp_path, content)?;
    #[cfg(unix)]
    {
        let _ = std::fs::set_permissions(&temp_path, std::fs::Permissions::from_mode(0o600));
    }
    std::fs::rename(&temp_path, path)?;
    #[cfg(unix)]
    {
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexTokenRefreshOutput {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub id_token: Option<String>,
    pub account_id: Option<String>,
    pub expires_in_secs: u64,
}

#[axum::async_trait]
pub trait CodexTokenRefresher: Send + Sync + std::fmt::Debug {
    async fn refresh_token(&self, refresh_token: &str) -> Result<CodexTokenRefreshOutput, String>;
}

#[derive(Debug, Clone)]
pub struct DefaultCodexTokenRefresher {
    client: reqwest::Client,
    token_url: String,
}

impl Default for DefaultCodexTokenRefresher {
    fn default() -> Self {
        Self::new()
    }
}

pub type OpenAiCodexTokenRefresher = DefaultCodexTokenRefresher;

impl DefaultCodexTokenRefresher {
    pub fn new() -> Self {
        let token_url =
            std::env::var("CODEX_TOKEN_URL").unwrap_or_else(|_| CODEX_TOKEN_URL.to_string());
        Self::with_url(token_url)
    }

    pub fn with_url(token_url: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("Failed to build DefaultCodexTokenRefresher client"),
            token_url: token_url.into(),
        }
    }

    pub fn with_client_and_url(client: reqwest::Client, token_url: impl Into<String>) -> Self {
        Self {
            client,
            token_url: token_url.into(),
        }
    }
}

#[axum::async_trait]
impl CodexTokenRefresher for DefaultCodexTokenRefresher {
    async fn refresh_token(&self, refresh_token: &str) -> Result<CodexTokenRefreshOutput, String> {
        let params = [
            ("client_id", CODEX_CLIENT_ID),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ];
        let resp = self
            .client
            .post(&self.token_url)
            .form(&params)
            .send()
            .await
            .map_err(|e| format!("Codex token refresh network error: {e}"))?;

        let status = resp.status();
        let body = resp
            .text()
            .await
            .map_err(|e| format!("Failed to read Codex token refresh response: {e}"))?;

        if !status.is_success() {
            return Err(format!("Codex token refresh failed ({status}): {body}"));
        }

        let parsed: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| format!("Failed to parse Codex token refresh JSON: {e}"))?;

        let access_token = parsed
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Token refresh returned no access_token".to_string())?
            .to_string();

        let refresh_token = parsed
            .get("refresh_token")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let id_token = parsed
            .get("id_token")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let account_id = parsed
            .get("account_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let expires_in_secs = parsed
            .get("expires_in")
            .and_then(|v| v.as_u64())
            .unwrap_or(3600);

        Ok(CodexTokenRefreshOutput {
            access_token,
            refresh_token,
            id_token,
            account_id,
            expires_in_secs,
        })
    }
}

#[derive(Debug, Default)]
pub struct MockCodexTokenRefresher {
    pub custom_token: RwLock<Option<String>>,
    pub custom_expires_in: std::sync::atomic::AtomicU64,
    pub should_fail: AtomicBool,
}

impl MockCodexTokenRefresher {
    pub fn new() -> Self {
        Self {
            custom_token: RwLock::new(None),
            custom_expires_in: std::sync::atomic::AtomicU64::new(3600),
            should_fail: AtomicBool::new(false),
        }
    }

    pub fn with_token(token: &str) -> Self {
        Self {
            custom_token: RwLock::new(Some(token.to_string())),
            custom_expires_in: std::sync::atomic::AtomicU64::new(3600),
            should_fail: AtomicBool::new(false),
        }
    }

    pub fn set_failing(&self, fail: bool) {
        self.should_fail.store(fail, Ordering::SeqCst);
    }
}

#[axum::async_trait]
impl CodexTokenRefresher for MockCodexTokenRefresher {
    async fn refresh_token(&self, refresh_token: &str) -> Result<CodexTokenRefreshOutput, String> {
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
                format!("mock-codex-token-for-{safe_sub}")
            });
        let expires_in = self.custom_expires_in.load(Ordering::SeqCst);
        Ok(CodexTokenRefreshOutput {
            access_token: token,
            refresh_token: None,
            id_token: None,
            account_id: None,
            expires_in_secs: expires_in,
        })
    }
}

#[axum::async_trait]
pub trait CodexQuotaFetcher: Send + Sync + std::fmt::Debug {
    async fn fetch_quota(
        &self,
        access_token: &str,
        account_id: &str,
    ) -> Result<serde_json::Value, String>;
}

#[derive(Debug, Clone)]
pub struct DefaultCodexQuotaFetcher {
    client: reqwest::Client,
    usage_url: String,
}

impl Default for DefaultCodexQuotaFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl DefaultCodexQuotaFetcher {
    pub fn new() -> Self {
        let usage_url =
            std::env::var("CODEX_USAGE_URL").unwrap_or_else(|_| CODEX_USAGE_URL.to_string());
        Self::with_url(usage_url)
    }

    pub fn with_url(usage_url: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(15))
                .build()
                .expect("Failed to build DefaultCodexQuotaFetcher client"),
            usage_url: usage_url.into(),
        }
    }
}

fn find_quota_window(payload: &serde_json::Value, name: &str) -> serde_json::Value {
    if let Some(direct) = payload.get(name) {
        if direct.is_object() {
            return direct.clone();
        }
    }
    for key in &["rate_limit", "rate_limits", "usage", "limits"] {
        if let Some(nested) = payload.get(key) {
            if nested.is_object() {
                let found = find_quota_window(nested, name);
                if !found.is_null() && found.as_object().map(|o| !o.is_empty()).unwrap_or(false) {
                    return found;
                }
            }
        }
    }
    serde_json::json!({})
}

fn normalize_quota_window(mut window: serde_json::Value) -> serde_json::Value {
    if !window.is_object() {
        return serde_json::json!({});
    }
    let used_percent = window
        .get("used_percent")
        .or_else(|| window.get("usedPercent"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let clamped = used_percent.clamp(0.0, 100.0);
    if let Some(obj) = window.as_object_mut() {
        obj.insert("used_percent".to_string(), serde_json::json!(clamped));
        if !obj.contains_key("reset_at") {
            if let Some(r) = obj.get("resetAt").or_else(|| obj.get("reset_time")) {
                obj.insert("reset_at".to_string(), r.clone());
            }
        }
    }
    window
}

#[axum::async_trait]
impl CodexQuotaFetcher for DefaultCodexQuotaFetcher {
    async fn fetch_quota(
        &self,
        access_token: &str,
        account_id: &str,
    ) -> Result<serde_json::Value, String> {
        let resp = self
            .client
            .get(&self.usage_url)
            .header(AUTHORIZATION, format!("Bearer {access_token}"))
            .header("ChatGPT-Account-Id", account_id)
            .header(USER_AGENT, CODEX_USER_AGENT)
            .header("originator", CODEX_ORIGINATOR)
            .header(ACCEPT, "application/json")
            .send()
            .await
            .map_err(|e| format!("Codex quota fetch network error: {e}"))?;

        let status = resp.status();
        let body = resp
            .text()
            .await
            .map_err(|e| format!("Failed to read Codex quota response: {e}"))?;

        if !status.is_success() {
            return Err(format!("Codex quota fetch failed ({status}): {body}"));
        }

        let raw: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| format!("Failed to parse Codex quota JSON: {e}"))?;

        let primary = normalize_quota_window(find_quota_window(&raw, "primary_window"));
        let secondary = {
            let s = find_quota_window(&raw, "secondary_window");
            if !s.is_null() && s.as_object().map(|o| !o.is_empty()).unwrap_or(false) {
                normalize_quota_window(s)
            } else {
                normalize_quota_window(find_quota_window(&raw, "weekly_window"))
            }
        };

        let email = raw
            .get("email")
            .or_else(|| raw.get("account").and_then(|a| a.get("email")))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        Ok(serde_json::json!({
            "primary_window": primary,
            "weekly_window": secondary,
            "email": email,
            "fetched_at": current_time_secs(),
        }))
    }
}

#[derive(Debug, Default)]
pub struct MockCodexQuotaFetcher {
    pub custom_quota: RwLock<Option<serde_json::Value>>,
    pub should_fail: AtomicBool,
}

impl MockCodexQuotaFetcher {
    pub fn new() -> Self {
        Self {
            custom_quota: RwLock::new(None),
            should_fail: AtomicBool::new(false),
        }
    }

    pub fn with_quota(quota: serde_json::Value) -> Self {
        Self {
            custom_quota: RwLock::new(Some(quota)),
            should_fail: AtomicBool::new(false),
        }
    }

    pub fn set_failing(&self, fail: bool) {
        self.should_fail.store(fail, Ordering::SeqCst);
    }
}

#[axum::async_trait]
impl CodexQuotaFetcher for MockCodexQuotaFetcher {
    async fn fetch_quota(
        &self,
        _access_token: &str,
        _account_id: &str,
    ) -> Result<serde_json::Value, String> {
        if self.should_fail.load(Ordering::SeqCst) {
            return Err("Mock quota fetch failed".to_string());
        }
        let q = self
            .custom_quota
            .read()
            .unwrap()
            .clone()
            .unwrap_or_else(|| {
                serde_json::json!({
                    "primary_window": {
                        "used_percent": 10.0,
                        "reset_at": current_time_secs() + 18000.0,
                    },
                    "weekly_window": {
                        "used_percent": 5.0,
                    },
                    "fetched_at": current_time_secs(),
                })
            });
        Ok(q)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexGuardrails {
    pub max_concurrency: usize,
    pub min_gap_seconds: f64,
    pub max_rpm: usize,
}

impl Default for CodexGuardrails {
    fn default() -> Self {
        Self {
            max_concurrency: 1,
            min_gap_seconds: CODEX_MIN_GAP_SECS,
            max_rpm: CODEX_MAX_RPM,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexAccountStatus {
    pub id: String,
    pub email: String,
    pub account_id_suffix: String,
    pub is_active: bool,
    pub active: bool,
    pub cooldown_remaining: f64,
    pub recent_rpm: usize,
    pub total_requests: i64,
    pub last_error: String,
    pub quota: serde_json::Value,
    pub guardrails: CodexGuardrails,
    pub is_legacy: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexPoolStatus {
    pub enabled: bool,
    pub active: bool,
    pub accounts: Vec<CodexAccountStatus>,
    pub total_accounts: usize,
    pub active_accounts: usize,
    pub rotation_enabled: bool,
    pub email: String,
    pub account_id_suffix: String,
    pub cooldown_remaining: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexOAuthStartResponse {
    pub ok: bool,
    pub state: String,
    #[serde(default)]
    pub ticket_id: String,
    pub authorization_url: String,
    #[serde(default)]
    pub authorize_url: String,
    pub redirect_uri: String,
    pub expires_in: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexOAuthStatusResponse {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CodexOAuthTicket {
    pub verifier: String,
    pub status: String,
    pub email: Option<String>,
    pub error: Option<String>,
    pub expires_at: f64,
}

pub struct CodexAccount {
    pub id: String,
    pub auth_path: PathBuf,
    pub is_active: AtomicBool,
    pub email: RwLock<String>,
    pub account_id: RwLock<String>,
    pub access_token: RwLock<String>,
    pub refresh_token_value: RwLock<String>,
    pub expires_at: RwLock<f64>,
    pub cooldown_until: RwLock<f64>,
    pub last_started_at: RwLock<f64>,
    pub last_used_at: RwLock<f64>,
    pub total_requests: AtomicI64,
    pub error_count: AtomicI64,
    pub last_error: RwLock<String>,
    pub recent_requests: RwLock<VecDeque<f64>>,
    pub quota_cache: RwLock<serde_json::Value>,
    pub quota_last_attempt: RwLock<f64>,
    pub generation_lock: Arc<tokio::sync::Mutex<()>>,
    pub refresh_lock: Arc<tokio::sync::Mutex<()>>,
    pub quota_lock: Arc<tokio::sync::Mutex<()>>,
    pub in_flight: AtomicUsize,
}

pub struct CodexLease {
    pub pool: Arc<CodexPool>,
    pub account: Arc<CodexAccount>,
    committed: bool,
}

impl CodexLease {
    pub fn new(pool: Arc<CodexPool>, account: Arc<CodexAccount>) -> Self {
        Self {
            pool,
            account,
            committed: false,
        }
    }

    pub fn commit_success(&mut self) {
        if !self.committed {
            self.committed = true;
            self.pool.mark_used(&self.account);
            self.account.in_flight.fetch_sub(1, Ordering::SeqCst);
        }
    }

    pub fn commit_error(&mut self, error: &str, is_rate_limit: bool) {
        if !self.committed {
            self.committed = true;
            self.pool.mark_error(&self.account, error, is_rate_limit);
            self.account.in_flight.fetch_sub(1, Ordering::SeqCst);
        }
    }

    pub fn commit_error_with_cooldown(&mut self, error: &str, cooldown_secs: f64) {
        if !self.committed {
            self.committed = true;
            self.pool
                .mark_error_with_cooldown(&self.account, error, cooldown_secs);
            self.account.in_flight.fetch_sub(1, Ordering::SeqCst);
        }
    }

    pub fn release(&mut self) {
        if !self.committed {
            self.committed = true;
            self.account.in_flight.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

impl Drop for CodexLease {
    fn drop(&mut self) {
        if !self.committed {
            self.committed = true;
            self.account.in_flight.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

impl std::fmt::Debug for CodexAccount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodexAccount")
            .field("id", &self.id)
            .field("email", &*self.email.read().unwrap())
            .field("auth_path", &self.auth_path)
            .field("is_active", &self.is_active.load(Ordering::SeqCst))
            .field("access_token", &"[REDACTED]")
            .field("refresh_token", &"[REDACTED]")
            .field("cooldown_until", &*self.cooldown_until.read().unwrap())
            .finish()
    }
}

impl CodexAccount {
    pub fn new(id: impl Into<String>, auth_path: PathBuf, is_active: bool) -> Self {
        Self {
            id: id.into(),
            auth_path,
            is_active: AtomicBool::new(is_active),
            email: RwLock::new(String::new()),
            account_id: RwLock::new(String::new()),
            access_token: RwLock::new(String::new()),
            refresh_token_value: RwLock::new(String::new()),
            expires_at: RwLock::new(0.0),
            cooldown_until: RwLock::new(0.0),
            last_started_at: RwLock::new(0.0),
            last_used_at: RwLock::new(0.0),
            total_requests: AtomicI64::new(0),
            error_count: AtomicI64::new(0),
            last_error: RwLock::new(String::new()),
            recent_requests: RwLock::new(VecDeque::new()),
            quota_cache: RwLock::new(serde_json::json!({})),
            quota_last_attempt: RwLock::new(0.0),
            generation_lock: Arc::new(tokio::sync::Mutex::new(())),
            refresh_lock: Arc::new(tokio::sync::Mutex::new(())),
            quota_lock: Arc::new(tokio::sync::Mutex::new(())),
            in_flight: AtomicUsize::new(0),
        }
    }

    pub fn load_auth(&self) -> bool {
        if !self.auth_path.exists() {
            *self.last_error.write().unwrap() =
                format!("Auth file not found: {}", self.auth_path.display());
            return false;
        }
        match std::fs::read_to_string(&self.auth_path) {
            Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
                Ok(data) => {
                    let tokens = data.get("tokens").cloned().unwrap_or(serde_json::json!({}));
                    let access = tokens
                        .get("access_token")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let refresh = tokens
                        .get("refresh_token")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let (identity_email, identity_account_id) =
                        extract_codex_token_identity(&tokens);
                    let acc_id = tokens
                        .get("account_id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or(identity_account_id);

                    let access_claims = jwt_claims_unverified(&access);
                    let exp = access_claims
                        .get("exp")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);

                    if !identity_email.is_empty() {
                        *self.email.write().unwrap() = identity_email;
                    }
                    *self.account_id.write().unwrap() = acc_id.clone();
                    *self.access_token.write().unwrap() = access.clone();
                    *self.refresh_token_value.write().unwrap() = refresh.clone();
                    *self.expires_at.write().unwrap() = exp;

                    let ok = !refresh.is_empty() && !acc_id.is_empty();
                    if !ok {
                        *self.last_error.write().unwrap() =
                            "auth.json is missing required tokens".to_string();
                    }
                    ok
                }
                Err(e) => {
                    *self.last_error.write().unwrap() = format!("Could not parse auth JSON: {e}");
                    false
                }
            },
            Err(e) => {
                *self.last_error.write().unwrap() = format!("Could not read auth file: {e}");
                false
            }
        }
    }

    pub fn trim_rate_window(&self, now: f64) {
        let cutoff = now - 60.0;
        let mut reqs = self.recent_requests.write().unwrap();
        while let Some(&front) = reqs.front() {
            if front <= cutoff {
                reqs.pop_front();
            } else {
                break;
            }
        }
    }

    pub fn headers(&self, accept: &str, content_type: bool) -> HeaderMap {
        let mut map = HeaderMap::new();
        let token = self.access_token.read().unwrap().clone();
        if let Ok(val) = HeaderValue::from_str(&format!("Bearer {token}")) {
            map.insert(AUTHORIZATION, val);
        }
        if let Ok(val) = HeaderValue::from_str(CODEX_USER_AGENT) {
            map.insert(USER_AGENT, val);
        }
        if let Ok(val) = HeaderValue::from_str(CODEX_ORIGINATOR) {
            map.insert(reqwest::header::HeaderName::from_static("originator"), val);
        }
        let acc_id = self.account_id.read().unwrap().clone();
        if let Ok(val) = HeaderValue::from_str(&acc_id) {
            map.insert(
                reqwest::header::HeaderName::from_static("chatgpt-account-id"),
                val,
            );
        }
        if let Ok(val) = HeaderValue::from_str(accept) {
            map.insert(ACCEPT, val);
        }
        if content_type {
            map.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        }
        map
    }
}

pub struct CodexPool {
    pub db: Arc<Database>,
    pub refresher: Arc<dyn CodexTokenRefresher>,
    pub quota_fetcher: Arc<dyn CodexQuotaFetcher>,
    pub accounts: RwLock<HashMap<String, Arc<CodexAccount>>>,
    pub oauth_tickets: RwLock<HashMap<String, CodexOAuthTicket>>,
    pub accounts_dir: PathBuf,
    pub default_auth_path: PathBuf,
    pub base_url: String,
    pub token_url: RwLock<String>,
    pub guardrails: CodexGuardrails,
}

pub type CodexAccountPool = CodexPool;

impl std::fmt::Debug for CodexPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodexAccountPool")
            .field("accounts_dir", &self.accounts_dir)
            .field("default_auth_path", &self.default_auth_path)
            .field("base_url", &self.base_url)
            .field("accounts_count", &self.accounts.read().unwrap().len())
            .finish()
    }
}

impl CodexPool {
    pub fn new(db: Arc<Database>) -> Self {
        let data_dir = std::env::var("AG_DATA_DIR")
            .or_else(|_| std::env::var("EZ_DATA_DIR"))
            .unwrap_or_else(|_| crate::config::DEFAULT_DATA_DIR.to_string());
        let base_path = PathBuf::from(data_dir);
        let accounts_dir = base_path.join("codex-accounts");
        let default_auth_path = base_path.join("codex-auth.json");
        Self::with_components(
            db,
            Arc::new(DefaultCodexTokenRefresher::new()),
            Arc::new(DefaultCodexQuotaFetcher::new()),
            accounts_dir,
            default_auth_path,
            CODEX_BASE_URL.to_string(),
        )
    }

    pub fn with_refresher(db: Arc<Database>, refresher: Arc<dyn CodexTokenRefresher>) -> Self {
        let data_dir = std::env::var("AG_DATA_DIR")
            .or_else(|_| std::env::var("EZ_DATA_DIR"))
            .unwrap_or_else(|_| crate::config::DEFAULT_DATA_DIR.to_string());
        let base_path = PathBuf::from(data_dir);
        let accounts_dir = base_path.join("codex-accounts");
        let default_auth_path = base_path.join("codex-auth.json");
        Self::with_components(
            db,
            refresher,
            Arc::new(DefaultCodexQuotaFetcher::new()),
            accounts_dir,
            default_auth_path,
            CODEX_BASE_URL.to_string(),
        )
    }

    pub fn with_paths(
        db: Arc<Database>,
        accounts_dir: PathBuf,
        default_auth_path: PathBuf,
    ) -> Self {
        Self::with_components(
            db,
            Arc::new(DefaultCodexTokenRefresher::new()),
            Arc::new(DefaultCodexQuotaFetcher::new()),
            accounts_dir,
            default_auth_path,
            CODEX_BASE_URL.to_string(),
        )
    }

    pub fn with_components(
        db: Arc<Database>,
        refresher: Arc<dyn CodexTokenRefresher>,
        quota_fetcher: Arc<dyn CodexQuotaFetcher>,
        accounts_dir: PathBuf,
        default_auth_path: PathBuf,
        base_url: String,
    ) -> Self {
        let token_url =
            std::env::var("CODEX_TOKEN_URL").unwrap_or_else(|_| CODEX_TOKEN_URL.to_string());
        Self {
            db,
            refresher,
            quota_fetcher,
            accounts: RwLock::new(HashMap::new()),
            oauth_tickets: RwLock::new(HashMap::new()),
            accounts_dir,
            default_auth_path,
            base_url: base_url.trim_end_matches('/').to_string(),
            token_url: RwLock::new(token_url),
            guardrails: CodexGuardrails::default(),
        }
    }

    pub fn with_token_url(self, token_url: impl Into<String>) -> Self {
        *self.token_url.write().unwrap() = token_url.into();
        self
    }

    pub fn db(&self) -> &Arc<Database> {
        &self.db
    }

    pub fn load_from_db(&self) -> Result<(), String> {
        let _ = self
            .db
            .ensure_legacy_codex_account(&self.default_auth_path, None);

        let rows = self
            .db
            .list_codex_accounts()
            .map_err(|e| format!("Failed to list codex accounts from db: {e}"))?;

        let mut current_map = self.accounts.write().unwrap();
        let db_ids: std::collections::HashSet<String> = rows.iter().map(|r| r.id.clone()).collect();
        current_map.retain(|id, _| db_ids.contains(id));

        for r in rows {
            let auth_path = PathBuf::from(&r.auth_path);
            if let Some(existing) = current_map.get(&r.id) {
                existing.is_active.store(r.is_active, Ordering::SeqCst);
                if let Some(ref em) = r.email {
                    *existing.email.write().unwrap() = em.clone();
                }
                existing.load_auth();
            } else {
                let acc = Arc::new(CodexAccount::new(&r.id, auth_path, r.is_active));
                if let Some(ref em) = r.email {
                    *acc.email.write().unwrap() = em.clone();
                }
                acc.load_auth();
                current_map.insert(r.id.clone(), acc);
            }
        }
        Ok(())
    }

    pub fn pick_account(&self) -> Option<Arc<CodexAccount>> {
        let _ = self.load_from_db();
        let now = current_time_secs();
        let map = self.accounts.read().unwrap();

        let mut candidates = Vec::new();
        for acc in map.values() {
            if !acc.is_active.load(Ordering::SeqCst) {
                continue;
            }
            if !acc.load_auth() {
                continue;
            }
            if *acc.cooldown_until.read().unwrap() > now {
                continue;
            }
            let quota = acc.quota_cache.read().unwrap();
            if let Some(primary) = quota.get("primary_window").and_then(|w| w.as_object()) {
                if let Some(used) = primary.get("used_percent").and_then(|u| u.as_f64()) {
                    if used >= CODEX_QUOTA_STOP_PERCENT {
                        continue;
                    }
                }
            }
            candidates.push(acc.clone());
        }

        if candidates.is_empty() {
            return None;
        }

        candidates.sort_by(|a, b| {
            let a_used = *a.last_used_at.read().unwrap();
            let b_used = *b.last_used_at.read().unwrap();
            a_used
                .partial_cmp(&b_used)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.id.cmp(&b.id))
        });

        Some(candidates[0].clone())
    }

    pub fn acquire_lease(self: &Arc<Self>) -> Option<CodexLease> {
        let _ = self.load_from_db();
        let now = current_time_secs();
        let map = self.accounts.read().unwrap();

        let mut candidates = Vec::new();
        for acc in map.values() {
            if !acc.is_active.load(Ordering::SeqCst) {
                continue;
            }
            if !acc.load_auth() {
                continue;
            }
            if *acc.cooldown_until.read().unwrap() > now {
                continue;
            }
            if acc.in_flight.load(Ordering::SeqCst) >= self.guardrails.max_concurrency {
                continue;
            }
            let last_used = *acc.last_used_at.read().unwrap();
            if self.guardrails.min_gap_seconds > 0.0
                && (now - last_used) < self.guardrails.min_gap_seconds
            {
                continue;
            }
            let rpm = {
                let reqs = acc.recent_requests.read().unwrap();
                reqs.iter().filter(|&&t| t > now - 60.0).count()
            };
            if rpm >= self.guardrails.max_rpm {
                continue;
            }
            let quota = acc.quota_cache.read().unwrap();
            if let Some(primary) = quota.get("primary_window").and_then(|w| w.as_object()) {
                if let Some(used) = primary.get("used_percent").and_then(|u| u.as_f64()) {
                    if used >= CODEX_QUOTA_STOP_PERCENT {
                        continue;
                    }
                }
            }
            candidates.push(acc.clone());
        }

        if candidates.is_empty() {
            return None;
        }

        candidates.sort_by(|a, b| {
            let a_used = *a.last_used_at.read().unwrap();
            let b_used = *b.last_used_at.read().unwrap();
            a_used
                .partial_cmp(&b_used)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.id.cmp(&b.id))
        });

        let chosen = candidates[0].clone();
        chosen.in_flight.fetch_add(1, Ordering::SeqCst);
        let now = current_time_secs();
        *chosen.last_started_at.write().unwrap() = now;

        Some(CodexLease::new(self.clone(), chosen))
    }

    pub async fn acquire_lease_with_pacing(self: &Arc<Self>) -> Option<CodexLease> {
        if let Some(lease) = self.acquire_lease() {
            return Some(lease);
        }

        let now = current_time_secs();
        let shortest_wait = {
            let map = self.accounts.read().unwrap();
            let mut min_w: Option<f64> = None;
            for acc in map.values() {
                if !acc.is_active.load(Ordering::SeqCst)
                    || *acc.cooldown_until.read().unwrap() > now
                {
                    continue;
                }
                if acc.in_flight.load(Ordering::SeqCst) >= self.guardrails.max_concurrency {
                    continue;
                }
                let last_used = *acc.last_used_at.read().unwrap();
                let gap_rem = self.guardrails.min_gap_seconds - (now - last_used);
                if gap_rem > 0.0 && gap_rem <= self.guardrails.min_gap_seconds.max(2.0) {
                    min_w = Some(min_w.map(|w: f64| w.min(gap_rem)).unwrap_or(gap_rem));
                }
            }
            min_w
        };

        if let Some(wait_secs) = shortest_wait {
            tokio::time::sleep(tokio::time::Duration::from_secs_f64(wait_secs)).await;
            return self.acquire_lease();
        }

        None
    }

    pub fn peek_account(&self) -> Option<Arc<CodexAccount>> {
        let now = current_time_secs();
        let map = self.accounts.read().unwrap();
        let mut candidates = Vec::new();
        for acc in map.values() {
            if acc.is_active.load(Ordering::SeqCst) && *acc.cooldown_until.read().unwrap() <= now {
                candidates.push(acc.clone());
            }
        }
        if candidates.is_empty() {
            return None;
        }
        candidates.sort_by(|a, b| {
            let a_used = *a.last_used_at.read().unwrap();
            let b_used = *b.last_used_at.read().unwrap();
            a_used
                .partial_cmp(&b_used)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.id.cmp(&b.id))
        });
        Some(candidates[0].clone())
    }

    pub fn mark_used(&self, account: &Arc<CodexAccount>) {
        let now = current_time_secs();
        *account.last_used_at.write().unwrap() = now;
        account.total_requests.fetch_add(1, Ordering::SeqCst);
        let mut reqs = account.recent_requests.write().unwrap();
        reqs.push_back(now);
        while let Some(&front) = reqs.front() {
            if front <= now - 60.0 {
                reqs.pop_front();
            } else {
                break;
            }
        }
    }

    pub fn mark_error_with_cooldown(
        &self,
        account: &Arc<CodexAccount>,
        error: &str,
        cooldown_secs: f64,
    ) {
        let now = current_time_secs();
        *account.cooldown_until.write().unwrap() = now + cooldown_secs;
        account.error_count.fetch_add(1, Ordering::SeqCst);
        let masked = mask_codex_error(error);
        *account.last_error.write().unwrap() = masked.clone();
        let _ = self
            .db
            .update_codex_account_error(&account.id, Some(&masked));
    }

    pub fn mark_error(&self, account: &Arc<CodexAccount>, error: &str, is_rate_limit: bool) {
        let cooldown = if is_rate_limit {
            COOLDOWN_AFTER_ERROR_SECS
        } else {
            30.0
        };
        self.mark_error_with_cooldown(account, error, cooldown);
    }

    pub async fn ensure_token(&self, account: &Arc<CodexAccount>) -> Result<String, String> {
        if !account.load_auth() {
            return Err(account.last_error.read().unwrap().clone());
        }
        let now = current_time_secs();
        let (exp, current_token) = {
            let exp = *account.expires_at.read().unwrap();
            let tok = account.access_token.read().unwrap().clone();
            (exp, tok)
        };
        if exp > now + CODEX_TOKEN_REFRESH_LEAD_SECS && !current_token.is_empty() {
            return Ok(current_token);
        }
        self.refresh_token(account, false).await?;
        let fresh = account.access_token.read().unwrap().clone();
        if fresh.is_empty() {
            Err("No access token available after refresh".to_string())
        } else {
            Ok(fresh)
        }
    }

    pub async fn refresh_token(
        &self,
        account: &Arc<CodexAccount>,
        force: bool,
    ) -> Result<bool, String> {
        let _lock = account.refresh_lock.lock().await;
        account.load_auth();
        let now = current_time_secs();
        let exp = *account.expires_at.read().unwrap();
        if !force && exp > now + CODEX_TOKEN_REFRESH_LEAD_SECS {
            return Ok(true);
        }

        let refresh_val = account.refresh_token_value.read().unwrap().clone();
        if refresh_val.is_empty() {
            let err = "Cannot refresh: refresh_token is empty".to_string();
            *account.last_error.write().unwrap() = err.clone();
            return Err(err);
        }

        match self.refresher.refresh_token(&refresh_val).await {
            Ok(output) => {
                if account.auth_path.exists() {
                    if let Ok(content) = std::fs::read_to_string(&account.auth_path) {
                        if let Ok(mut data) = serde_json::from_str::<serde_json::Value>(&content) {
                            let tokens = data.as_object_mut().and_then(|obj| {
                                if !obj.contains_key("tokens") {
                                    obj.insert("tokens".to_string(), serde_json::json!({}));
                                }
                                obj.get_mut("tokens").and_then(|t| t.as_object_mut())
                            });
                            if let Some(t_obj) = tokens {
                                t_obj.insert(
                                    "access_token".to_string(),
                                    serde_json::json!(output.access_token),
                                );
                                if let Some(ref rt) = output.refresh_token {
                                    t_obj
                                        .insert("refresh_token".to_string(), serde_json::json!(rt));
                                }
                                if let Some(ref it) = output.id_token {
                                    t_obj.insert("id_token".to_string(), serde_json::json!(it));
                                }
                                if let Some(ref aid) = output.account_id {
                                    t_obj.insert("account_id".to_string(), serde_json::json!(aid));
                                }
                            }
                            if let Some(obj) = data.as_object_mut() {
                                obj.insert(
                                    "last_refresh".to_string(),
                                    serde_json::json!(Utc::now().to_rfc3339()),
                                );
                            }
                            let _ = atomic_write_private_json(&account.auth_path, &data);
                        }
                    }
                }
                account.load_auth();
                if *account.expires_at.read().unwrap() <= 0.0 {
                    *account.expires_at.write().unwrap() =
                        current_time_secs() + output.expires_in_secs as f64;
                }
                *account.last_error.write().unwrap() = String::new();
                let _ = self.db.update_codex_account_error(&account.id, None);
                Ok(true)
            }
            Err(e) => {
                let masked = mask_codex_error(&e);
                *account.last_error.write().unwrap() = masked.clone();
                let _ = self
                    .db
                    .update_codex_account_error(&account.id, Some(&masked));
                Err(masked)
            }
        }
    }

    pub async fn fetch_account_quota(
        &self,
        account: &Arc<CodexAccount>,
        force: bool,
    ) -> Result<serde_json::Value, String> {
        let now = current_time_secs();
        if !account.is_active.load(Ordering::SeqCst) {
            return Ok(account.quota_cache.read().unwrap().clone());
        }
        if !force && now - *account.quota_last_attempt.read().unwrap() < CODEX_QUOTA_CACHE_TTL_SECS
        {
            return Ok(account.quota_cache.read().unwrap().clone());
        }

        let _lock = account.quota_lock.lock().await;
        let now = current_time_secs();
        if !force && now - *account.quota_last_attempt.read().unwrap() < CODEX_QUOTA_CACHE_TTL_SECS
        {
            return Ok(account.quota_cache.read().unwrap().clone());
        }
        *account.quota_last_attempt.write().unwrap() = now;

        let token = match self.ensure_token(account).await {
            Ok(t) => t,
            Err(_) => return Ok(account.quota_cache.read().unwrap().clone()),
        };
        let acc_id = account.account_id.read().unwrap().clone();

        match self.quota_fetcher.fetch_quota(&token, &acc_id).await {
            Ok(quota) => {
                if let Some(em) = quota.get("email").and_then(|v| v.as_str()) {
                    if !em.is_empty() {
                        *account.email.write().unwrap() = em.to_string();
                    }
                }
                *account.quota_cache.write().unwrap() = quota.clone();
                Ok(quota)
            }
            Err(e) => {
                let masked = mask_codex_error(&e);
                *account.last_error.write().unwrap() = masked;
                Ok(account.quota_cache.read().unwrap().clone())
            }
        }
    }

    pub async fn fetch_all_quotas(&self, force: bool) -> serde_json::Value {
        let active_accs: Vec<(String, Arc<CodexAccount>)> = {
            let map = self.accounts.read().unwrap();
            map.iter()
                .filter(|(_, a)| a.is_active.load(Ordering::SeqCst))
                .map(|(id, a)| (id.clone(), a.clone()))
                .collect()
        };
        let mut results = serde_json::Map::new();
        for (id, acc) in active_accs {
            if let Ok(q) = self.fetch_account_quota(&acc, force).await {
                results.insert(id, q);
            }
        }
        serde_json::Value::Object(results)
    }

    pub fn account_status(&self, acc: &Arc<CodexAccount>) -> CodexAccountStatus {
        let now = current_time_secs();
        acc.trim_rate_window(now);
        let cd_until = *acc.cooldown_until.read().unwrap();
        let is_act = acc.is_active.load(Ordering::SeqCst);
        let acc_id = acc.account_id.read().unwrap().clone();
        let suffix = if acc_id.chars().count() > 8 {
            acc_id
                .chars()
                .rev()
                .take(8)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect()
        } else {
            acc_id
        };

        CodexAccountStatus {
            id: acc.id.clone(),
            email: acc.email.read().unwrap().clone(),
            account_id_suffix: suffix,
            is_active: is_act,
            active: is_act && cd_until <= now,
            cooldown_remaining: (cd_until - now).max(0.0),
            recent_rpm: acc.recent_requests.read().unwrap().len(),
            total_requests: acc.total_requests.load(Ordering::SeqCst),
            last_error: acc.last_error.read().unwrap().clone(),
            quota: acc.quota_cache.read().unwrap().clone(),
            guardrails: CodexGuardrails::default(),
            is_legacy: acc.id == "legacy",
        }
    }

    pub fn status_struct(&self) -> CodexPoolStatus {
        let now = current_time_secs();
        let map = self.accounts.read().unwrap();
        let mut accounts_list = Vec::new();
        let mut active_accs = Vec::new();
        let mut active_ready = Vec::new();

        for acc in map.values() {
            let st = self.account_status(acc);
            if st.is_active {
                active_accs.push(acc.clone());
                if st.active {
                    active_ready.push(acc.clone());
                }
            }
            accounts_list.push(st);
        }

        accounts_list.sort_by(|a, b| {
            b.is_active
                .cmp(&a.is_active)
                .then_with(|| a.email.cmp(&b.email))
        });

        let primary_acc = active_accs.first().or_else(|| map.values().next());
        let (primary_email, primary_acc_id_suffix) = if let Some(acc) = primary_acc {
            let acc_id = acc.account_id.read().unwrap().clone();
            let suffix = if acc_id.chars().count() > 8 {
                acc_id
                    .chars()
                    .rev()
                    .take(8)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect()
            } else {
                acc_id
            };
            (acc.email.read().unwrap().clone(), suffix)
        } else {
            (String::new(), String::new())
        };

        let min_cooldown = active_accs
            .iter()
            .map(|a| (*a.cooldown_until.read().unwrap() - now).max(0.0))
            .fold(
                0.0f64,
                |acc, val| if acc == 0.0 { val } else { acc.min(val) },
            );

        CodexPoolStatus {
            enabled: !active_accs.is_empty(),
            active: !active_ready.is_empty(),
            total_accounts: map.len(),
            active_accounts: active_ready.len(),
            rotation_enabled: active_accs.len() > 1,
            email: primary_email,
            account_id_suffix: primary_acc_id_suffix,
            cooldown_remaining: min_cooldown,
            accounts: accounts_list,
        }
    }

    pub fn status(&self) -> serde_json::Value {
        serde_json::to_value(self.status_struct()).unwrap_or_else(|_| serde_json::json!({}))
    }

    pub fn list_accounts_json(&self) -> serde_json::Value {
        let st = self.status_struct();
        serde_json::json!({
            "accounts": st.accounts,
            "total": st.total_accounts,
            "rotation_enabled": st.rotation_enabled,
        })
    }

    pub fn toggle_account(&self, id: &str) -> Result<bool, AppError> {
        let toggled = self
            .db
            .toggle_codex_account(id)
            .map_err(|e| AppError::Internal(format!("Failed to toggle codex account: {e}")))?
            .ok_or_else(|| AppError::NotFound("Codex account not found".to_string()))?;

        if let Some(acc) = self.accounts.read().unwrap().get(id) {
            acc.is_active.store(toggled, Ordering::SeqCst);
        }
        let _ = self.load_from_db();
        Ok(toggled)
    }

    pub fn reset_account(&self, id: &str) -> Result<bool, AppError> {
        let acc = self
            .accounts
            .read()
            .unwrap()
            .get(id)
            .cloned()
            .ok_or_else(|| AppError::NotFound("Codex account not found".to_string()))?;

        *acc.cooldown_until.write().unwrap() = 0.0;
        *acc.last_error.write().unwrap() = String::new();
        acc.error_count.store(0, Ordering::SeqCst);
        let _ = self.db.reset_codex_account(id);
        Ok(true)
    }

    pub fn delete_account(&self, id: &str) -> Result<bool, AppError> {
        let record = self
            .db
            .get_codex_account_by_id(id)
            .map_err(|e| AppError::Internal(e.to_string()))?
            .ok_or_else(|| AppError::NotFound("Codex account not found".to_string()))?;

        let _ = self.db.delete_codex_account(id);
        self.accounts.write().unwrap().remove(id);

        let path = PathBuf::from(&record.auth_path);
        if path != self.default_auth_path && path.is_file() {
            let _ = std::fs::remove_file(path);
        }
        let _ = self.load_from_db();
        Ok(true)
    }

    pub fn clean_expired_tickets(&self) {
        let now = current_time_secs();
        self.oauth_tickets
            .write()
            .unwrap()
            .retain(|_, t| t.expires_at > now);
    }

    pub fn start_oauth(&self) -> serde_json::Value {
        let res = self.oauth_start(None);
        serde_json::to_value(res).unwrap_or_else(|_| serde_json::json!({}))
    }

    pub fn oauth_start(&self, redirect_uri: Option<&str>) -> CodexOAuthStartResponse {
        self.clean_expired_tickets();
        let now = current_time_secs();
        let (verifier, challenge) = generate_pkce_pair();
        let state = uuid::Uuid::new_v4().to_string();
        let redir = redirect_uri.unwrap_or(CODEX_DEFAULT_REDIRECT_URI);

        let ticket = CodexOAuthTicket {
            verifier,
            status: "pending".to_string(),
            email: None,
            error: None,
            expires_at: now + CODEX_OAUTH_TICKET_TTL_SECS,
        };

        self.oauth_tickets
            .write()
            .unwrap()
            .insert(state.clone(), ticket);

        let params = [
            ("client_id", CODEX_CLIENT_ID),
            ("redirect_uri", redir),
            ("response_type", "code"),
            ("scope", CODEX_OAUTH_SCOPES),
            ("code_challenge", &challenge),
            ("code_challenge_method", "S256"),
            ("state", &state),
            ("id_token_add_organizations", "true"),
            ("codex_cli_simplified_flow", "true"),
            ("originator", CODEX_ORIGINATOR),
        ];

        let query = params
            .iter()
            .map(|(k, v)| format!("{k}={}", urlencoding_encode(v)))
            .collect::<Vec<_>>()
            .join("&");

        let auth_url = format!("{CODEX_AUTHORIZE_URL}?{query}");

        CodexOAuthStartResponse {
            ok: true,
            ticket_id: state.clone(),
            state,
            authorize_url: auth_url.clone(),
            authorization_url: auth_url,
            redirect_uri: redir.to_string(),
            expires_in: CODEX_OAUTH_TICKET_TTL_SECS as u64,
        }
    }

    pub async fn exchange_oauth_code(
        &self,
        code: &str,
        state: Option<&str>,
        _redirect_uri: Option<&str>,
    ) -> Result<String, AppError> {
        let (verifier, ticket_key) = if let Some(s) = state {
            let tickets = self.oauth_tickets.read().unwrap();
            let ticket = tickets.get(s).ok_or_else(|| {
                AppError::BadRequest(
                    "Phiên đăng nhập đã hết hạn hoặc không tìm thấy. Vui lòng bắt đầu lại."
                        .to_string(),
                )
            })?;
            (ticket.verifier.clone(), s.to_string())
        } else {
            let tickets = self.oauth_tickets.read().unwrap();
            let now = current_time_secs();
            let found_key = tickets
                .iter()
                .filter(|(_, t)| t.status == "pending" && t.expires_at > now)
                .max_by(|a, b| {
                    a.1.expires_at
                        .partial_cmp(&b.1.expires_at)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(k, _)| k.clone());
            if let Some(k) = found_key {
                (tickets.get(&k).unwrap().verifier.clone(), k)
            } else {
                return Err(AppError::BadRequest(
                    "Phiên đăng nhập đã hết hạn hoặc không tìm thấy. Vui lòng bắt đầu lại."
                        .to_string(),
                ));
            }
        };

        let client = reqwest::Client::new();
        let custom_token_url = self.token_url.read().unwrap().clone();
        let token_url = if !custom_token_url.is_empty() {
            custom_token_url
        } else {
            std::env::var("CODEX_TOKEN_URL").unwrap_or_else(|_| CODEX_TOKEN_URL.to_string())
        };
        let params = [
            ("grant_type", "authorization_code"),
            ("client_id", CODEX_CLIENT_ID),
            ("code", code),
            ("code_verifier", &verifier),
            ("redirect_uri", CODEX_DEFAULT_REDIRECT_URI),
        ];

        let resp = client
            .post(&token_url)
            .header(reqwest::header::USER_AGENT, CODEX_USER_AGENT)
            .form(&params)
            .send()
            .await
            .map_err(|e| {
                if let Some(t) = self.oauth_tickets.write().unwrap().get_mut(&ticket_key) {
                    t.status = "error".to_string();
                    t.error = Some(format!("Token exchange network error: {e}"));
                }
                AppError::BadGateway(format!("Token exchange network error: {e}"))
            })?;

        let status = resp.status();
        let body = resp
            .text()
            .await
            .map_err(|e| AppError::BadGateway(format!("Failed to read response: {e}")))?;

        if !status.is_success() {
            if let Some(t) = self.oauth_tickets.write().unwrap().get_mut(&ticket_key) {
                t.status = "error".to_string();
                t.error = Some(format!("OpenAI token exchange failed ({status}): {body}"));
            }
            return Err(AppError::BadRequest(format!(
                "OpenAI token exchange failed ({status}): {body}"
            )));
        }

        let token_data: serde_json::Value = serde_json::from_str(&body).map_err(|e| {
            AppError::BadGateway(format!("Failed to parse token exchange JSON: {e}"))
        })?;

        let email = self.store_oauth_account(&token_data)?;

        if let Some(t) = self.oauth_tickets.write().unwrap().get_mut(&ticket_key) {
            t.status = "done".to_string();
            t.email = Some(email.clone());
        }

        Ok(email)
    }

    pub fn store_oauth_account(&self, token_data: &serde_json::Value) -> Result<String, AppError> {
        let (email, mut account_id) = extract_codex_token_identity(token_data);
        if let Some(aid) = token_data.get("account_id").and_then(|v| v.as_str()) {
            if !aid.is_empty() {
                account_id = aid.to_string();
            }
        }
        if account_id.is_empty() {
            return Err(AppError::BadRequest(
                "OAuth response is missing the ChatGPT account id".to_string(),
            ));
        }

        let auth_data = serde_json::json!({
            "auth_mode": "chatgpt",
            "OPENAI_API_KEY": null,
            "tokens": {
                "id_token": token_data.get("id_token"),
                "access_token": token_data.get("access_token"),
                "refresh_token": token_data.get("refresh_token"),
                "account_id": account_id,
            },
            "last_refresh": Utc::now().to_rfc3339(),
        });

        std::fs::create_dir_all(&self.accounts_dir)
            .map_err(|e| AppError::Internal(format!("Failed to create accounts directory: {e}")))?;

        let existing = if !email.is_empty() {
            self.db
                .get_codex_account_by_email(&email)
                .map_err(|e| AppError::Internal(e.to_string()))?
        } else {
            None
        };

        let target_path = if let Some(ref rec) = existing {
            let old_path = PathBuf::from(&rec.auth_path);
            if rec.id != "legacy" {
                old_path
            } else {
                self.accounts_dir.join(format!(
                    "{}_{}.json",
                    rec.id,
                    &uuid::Uuid::new_v4().to_string()[..8]
                ))
            }
        } else {
            let record_id = uuid::Uuid::new_v4().to_string();
            self.accounts_dir.join(format!("{record_id}.json"))
        };

        atomic_write_private_json(&target_path, &auth_data)
            .map_err(|e| AppError::Internal(format!("Failed to write private auth file: {e}")))?;

        if let Some(ref rec) = existing {
            if rec.id == "legacy" && target_path.to_string_lossy() != rec.auth_path {
                let new_id = uuid::Uuid::new_v4().to_string();
                let _ = self.db.create_codex_account(
                    Some(&new_id),
                    Some(&email),
                    &target_path.to_string_lossy(),
                    true,
                );
                let _ = self.db.delete_codex_account("legacy");
            } else {
                let updated_rec = CodexAccountRecord {
                    id: rec.id.clone(),
                    email: Some(email.clone()),
                    auth_path: target_path.to_string_lossy().to_string(),
                    is_active: true,
                    created_at: rec.created_at.clone(),
                    updated_at: Utc::now().to_rfc3339(),
                    last_error: None,
                };
                let _ = self.db.save_codex_account(&updated_rec);
            }
        } else {
            let record_id = target_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("account")
                .to_string();
            let _ = self.db.create_codex_account(
                Some(&record_id),
                Some(&email),
                &target_path.to_string_lossy(),
                true,
            );
        }

        let _ = self.load_from_db();
        Ok(email)
    }

    pub fn add_account_with_tokens(
        &self,
        tokens: serde_json::Value,
        id: Option<&str>,
    ) -> Result<CodexAccountRecord, AppError> {
        let (email, mut account_id) = extract_codex_token_identity(&tokens);
        if let Some(aid) = tokens.get("account_id").and_then(|v| v.as_str()) {
            if !aid.is_empty() {
                account_id = aid.to_string();
            }
        }

        let aid = id
            .map(|s| s.to_string())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        let target_path = self.accounts_dir.join(format!("{aid}.json"));

        let auth_data = serde_json::json!({
            "auth_mode": "chatgpt",
            "OPENAI_API_KEY": null,
            "tokens": {
                "id_token": tokens.get("id_token").and_then(|v| v.as_str()),
                "access_token": tokens.get("access_token").and_then(|v| v.as_str()),
                "refresh_token": tokens.get("refresh_token").and_then(|v| v.as_str()),
                "account_id": account_id,
            },
            "last_refresh": Utc::now().to_rfc3339(),
        });

        std::fs::create_dir_all(&self.accounts_dir)
            .map_err(|e| AppError::Internal(format!("Failed to create accounts directory: {e}")))?;

        atomic_write_private_json(&target_path, &auth_data)
            .map_err(|e| AppError::Internal(format!("Failed to write private auth file: {e}")))?;

        let record = self
            .db
            .create_codex_account(
                Some(&aid),
                if email.is_empty() { None } else { Some(&email) },
                &target_path.to_string_lossy(),
                true,
            )
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let _ = self.load_from_db();
        Ok(record)
    }

    pub fn oauth_status(&self, state: &str) -> serde_json::Value {
        let now = current_time_secs();
        let tickets = self.oauth_tickets.read().unwrap();
        let resp = if let Some(ticket) = tickets.get(state) {
            if ticket.expires_at < now {
                CodexOAuthStatusResponse {
                    status: "expired".to_string(),
                    email: None,
                    error: None,
                    message: Some("Phiên đăng nhập đã hết hạn (10 phút)".to_string()),
                }
            } else {
                CodexOAuthStatusResponse {
                    status: ticket.status.clone(),
                    email: ticket.email.clone(),
                    error: ticket.error.clone(),
                    message: None,
                }
            }
        } else {
            CodexOAuthStatusResponse {
                status: "idle".to_string(),
                email: None,
                error: None,
                message: None,
            }
        };
        serde_json::to_value(resp).unwrap_or_else(|_| serde_json::json!({}))
    }

    pub fn get_stats_summary(&self, now: f64) -> (usize, usize, usize, f64, usize) {
        let map = self.accounts.read().unwrap();
        let total = map.len();
        let mut active = 0;
        let mut cooldown = 0;
        let mut live_rpm = 0;

        for acc in map.values() {
            let is_act = acc.is_active.load(Ordering::SeqCst);
            let cd_until = *acc.cooldown_until.read().unwrap();
            if is_act && cd_until <= now {
                active += 1;
            }
            if cd_until > now {
                cooldown += 1;
            }
            live_rpm += acc
                .recent_requests
                .read()
                .unwrap()
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
}

/// Normalizes a tool call ID to satisfy OpenAI Codex API constraints:
/// - Maximum length: 64 characters
/// - Minimum length: 1 character
/// - Strips Gemini thought signatures appended with a colon (e.g. `call_123:<1000 char base64 signature>`)
pub fn sanitize_codex_call_id(raw_id: &str, fallback_prefix: &str) -> String {
    let clean = raw_id.trim();
    if clean.is_empty() {
        return fallback_prefix.to_string();
    }
    // Strip Gemini thought signature appended with colon
    let pure = clean.split_once(':').map(|(id, _)| id).unwrap_or(clean);
    if pure.is_empty() {
        fallback_prefix.to_string()
    } else if pure.len() > 64 {
        let mut end = 64;
        while end > 0 && !pure.is_char_boundary(end) {
            end -= 1;
        }
        if end == 0 {
            fallback_prefix.to_string()
        } else {
            pure[..end].to_string()
        }
    } else {
        pure.to_string()
    }
}

pub fn openai_to_codex_input(
    messages: &[ChatMessage],
    tools: Option<&serde_json::Value>,
) -> Result<(serde_json::Value, Option<serde_json::Value>), AppError> {
    if messages.is_empty() {
        return Err(AppError::BadRequest(
            "messages must be a non-empty array".to_string(),
        ));
    }

    let mut tool_call_ids: HashMap<String, String> = HashMap::new();
    let mut tool_calls_by_name: HashMap<String, String> = HashMap::new();
    let mut unconsumed_calls: VecDeque<String> = VecDeque::new();

    for (msg_idx, msg) in messages.iter().enumerate() {
        if msg.role == "assistant" {
            if let Some(tc_arr) = msg.tool_calls.as_ref().and_then(|v| v.as_array()) {
                for (tc_idx, tc) in tc_arr.iter().enumerate() {
                    let raw_id = tc
                        .get("id")
                        .or_else(|| tc.get("call_id"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .trim();
                    let fallback_id = format!("call_{msg_idx}_{tc_idx}");
                    let stable_id = sanitize_codex_call_id(raw_id, &fallback_id);
                    let func_name = tc
                        .get("function")
                        .and_then(|f| f.get("name"))
                        .or_else(|| tc.get("name"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .trim();

                    if !raw_id.is_empty() {
                        tool_call_ids.insert(raw_id.to_string(), stable_id.clone());
                        let pure = raw_id.split_once(':').map(|(id, _)| id).unwrap_or(raw_id);
                        tool_call_ids.insert(pure.to_string(), stable_id.clone());
                    }
                    tool_call_ids.insert(stable_id.clone(), stable_id.clone());
                    if !func_name.is_empty() {
                        tool_calls_by_name.insert(func_name.to_string(), stable_id.clone());
                    }
                    unconsumed_calls.push_back(stable_id);
                }
            }
        }
    }

    let mut converted_input = Vec::new();
    for (msg_idx, msg) in messages.iter().enumerate() {
        match msg.role.as_str() {
            "system" | "developer" => {
                let text = msg.content_text();
                converted_input.push(serde_json::json!({
                    "role": "developer",
                    "content": [{"type": "input_text", "text": text.as_ref()}]
                }));
            }
            "user" => {
                let mut user_parts = Vec::new();
                if let Some(MessageContent::Parts(parts)) = &msg.content {
                    for p in parts {
                        if let Some(t) = p.get("text").and_then(|v| v.as_str()) {
                            user_parts.push(serde_json::json!({"type": "input_text", "text": t}));
                        } else if let Some(t) = p.as_str() {
                            user_parts.push(serde_json::json!({"type": "input_text", "text": t}));
                        } else if p.get("type").and_then(|v| v.as_str()) == Some("input_text") {
                            user_parts.push(p.clone());
                        }
                    }
                }
                if user_parts.is_empty() {
                    let text = msg.content_text();
                    user_parts
                        .push(serde_json::json!({"type": "input_text", "text": text.as_ref()}));
                }
                converted_input.push(serde_json::json!({
                    "role": "user",
                    "content": user_parts
                }));
            }
            "assistant" => {
                let text = msg.content_text();
                let tc_arr_opt = msg.tool_calls.as_ref().and_then(|v| v.as_array());
                let has_tool_calls = tc_arr_opt.is_some_and(|arr| !arr.is_empty());
                let has_text = !text.is_empty();

                if has_text {
                    converted_input.push(serde_json::json!({
                        "role": "assistant",
                        "content": [{"type": "output_text", "text": text.as_ref()}]
                    }));
                } else if !has_tool_calls {
                    converted_input.push(serde_json::json!({
                        "role": "assistant",
                        "content": [{"type": "output_text", "text": ""}]
                    }));
                }

                if let Some(tc_arr) = tc_arr_opt {
                    for (tc_idx, tc) in tc_arr.iter().enumerate() {
                        let raw_id = tc
                            .get("id")
                            .or_else(|| tc.get("call_id"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .trim();
                        let fallback_id = format!("call_{msg_idx}_{tc_idx}");
                        let stable_id = sanitize_codex_call_id(raw_id, &fallback_id);
                        let (name, args) = if let Some(func) = tc.get("function") {
                            let n = func
                                .get("name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let a = if let Some(s) = func.get("arguments").and_then(|v| v.as_str())
                            {
                                s.to_string()
                            } else if let Some(v) = func.get("arguments") {
                                serde_json::to_string(v).unwrap_or_else(|_| "{}".to_string())
                            } else {
                                "{}".to_string()
                            };
                            (n, a)
                        } else {
                            let n = tc
                                .get("name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let a = tc
                                .get("arguments")
                                .and_then(|v| v.as_str())
                                .unwrap_or("{}")
                                .to_string();
                            (n, a)
                        };
                        converted_input.push(serde_json::json!({
                            "type": "function_call",
                            "call_id": stable_id,
                            "name": name,
                            "arguments": args
                        }));
                    }
                }
            }
            "tool" => {
                let call_id = if let Some(tid) = msg
                    .tool_call_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                {
                    tool_call_ids
                        .get(tid)
                        .cloned()
                        .or_else(|| {
                            let pure = tid.split_once(':').map(|(id, _)| id).unwrap_or(tid);
                            tool_call_ids.get(pure).cloned()
                        })
                        .unwrap_or_else(|| {
                            sanitize_codex_call_id(tid, &format!("call_tool_{msg_idx}"))
                        })
                } else if let Some(name) =
                    msg.name.as_deref().map(str::trim).filter(|s| !s.is_empty())
                {
                    tool_calls_by_name.get(name).cloned().unwrap_or_else(|| {
                        unconsumed_calls
                            .pop_front()
                            .unwrap_or_else(|| format!("call_tool_{msg_idx}"))
                    })
                } else if let Some(queued_id) = unconsumed_calls.pop_front() {
                    queued_id
                } else {
                    format!("call_tool_{msg_idx}")
                };

                let final_call_id =
                    sanitize_codex_call_id(&call_id, &format!("call_tool_{msg_idx}"));

                let output_text = msg.content_text();
                converted_input.push(serde_json::json!({
                    "type": "function_call_output",
                    "call_id": final_call_id,
                    "output": output_text.as_ref()
                }));
            }
            other => {
                return Err(AppError::BadRequest(format!(
                    "Unsupported Codex message role: {other}"
                )));
            }
        }
    }

    let converted_tools = if let Some(tools_val) = tools {
        if let Some(tools_arr) = tools_val.as_array() {
            let mut list = Vec::new();
            for tool in tools_arr {
                if tool.get("type").and_then(|v| v.as_str()) == Some("function") {
                    let function = tool
                        .get("function")
                        .cloned()
                        .unwrap_or(serde_json::json!({}));
                    list.push(serde_json::json!({
                        "type": "function",
                        "name": function.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                        "description": function.get("description").and_then(|v| v.as_str()).unwrap_or(""),
                        "parameters": function.get("parameters").cloned().unwrap_or(serde_json::json!({}))
                    }));
                }
            }
            if list.is_empty() {
                None
            } else {
                Some(serde_json::Value::Array(list))
            }
        } else {
            None
        }
    } else {
        None
    };

    Ok((serde_json::Value::Array(converted_input), converted_tools))
}

pub struct CodexSseStream {
    inner: BoxChatStream,
    buffer: Vec<u8>,
    queue: VecDeque<Result<Bytes, AppError>>,
    model: String,
    chat_id: String,
    created: i64,
    current_event: String,
    tool_call_index: usize,
    has_tool_calls: bool,
    prompt_tokens: u32,
    completion_tokens: u32,
    sent_done: bool,
    ended: bool,
    pub pool: Option<Arc<CodexPool>>,
    pub account: Option<Arc<CodexAccount>>,
    pub lease: Option<CodexLease>,
}

impl CodexSseStream {
    pub fn new(inner: BoxChatStream, model: String) -> Self {
        Self::with_pool(inner, model, None, None)
    }

    pub fn with_lease(inner: BoxChatStream, model: String, lease: CodexLease) -> Self {
        let pool = Some(lease.pool.clone());
        let account = Some(lease.account.clone());
        let mut s = Self::with_pool(inner, model, pool, account);
        s.lease = Some(lease);
        s
    }

    pub fn with_pool(
        inner: BoxChatStream,
        model: String,
        pool: Option<Arc<CodexPool>>,
        account: Option<Arc<CodexAccount>>,
    ) -> Self {
        Self {
            inner,
            buffer: Vec::new(),
            queue: VecDeque::new(),
            model,
            chat_id: format!("chatcmpl-{}", uuid::Uuid::new_v4().simple()),
            created: Utc::now().timestamp(),
            current_event: String::new(),
            tool_call_index: 0,
            has_tool_calls: false,
            prompt_tokens: 0,
            completion_tokens: 0,
            sent_done: false,
            ended: false,
            pool,
            account,
            lease: None,
        }
    }

    fn process_line(&mut self, line_bytes: &[u8]) {
        let line = String::from_utf8_lossy(line_bytes).trim().to_string();
        if line.is_empty() {
            return;
        }
        if let Some(ev) = line.strip_prefix("event:") {
            self.current_event = ev.trim().to_string();
            return;
        }
        if let Some(rest) = line.strip_prefix("data:") {
            let data_str = rest.trim();
            if data_str == "[DONE]" {
                self.sent_done = true;
                return;
            }
            if let Ok(data) = serde_json::from_str::<serde_json::Value>(data_str) {
                let event_name = if !self.current_event.is_empty() {
                    std::mem::take(&mut self.current_event)
                } else {
                    data.get("type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string()
                };

                match event_name.as_str() {
                    "response.output_text.delta" => {
                        let delta = if let Some(d) = data.get("delta") {
                            if let Some(s) = d.as_str() {
                                s.to_string()
                            } else if let Some(t) = d.get("text").and_then(|v| v.as_str()) {
                                t.to_string()
                            } else {
                                String::new()
                            }
                        } else {
                            String::new()
                        };
                        if !delta.is_empty() {
                            let chunk = serde_json::json!({
                                "id": self.chat_id,
                                "object": "chat.completion.chunk",
                                "created": self.created,
                                "model": self.model,
                                "choices": [{
                                    "index": 0,
                                    "delta": { "content": delta },
                                    "finish_reason": null
                                }]
                            });
                            self.queue
                                .push_back(Ok(Bytes::from(format!("data: {chunk}\n\n"))));
                        }
                    }
                    "response.output_item.added" => {
                        if data
                            .get("item")
                            .and_then(|i| i.get("type"))
                            .and_then(|v| v.as_str())
                            == Some("function_call")
                        {
                            self.has_tool_calls = true;
                            let item = data.get("item").unwrap();
                            let raw_call_id = item
                                .get("call_id")
                                .or_else(|| item.get("id"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .trim();
                            let call_id = if !raw_call_id.is_empty() {
                                raw_call_id.to_string()
                            } else {
                                format!("call_{}", uuid::Uuid::new_v4().simple())
                            };
                            let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("");
                            let chunk = serde_json::json!({
                                "id": self.chat_id,
                                "object": "chat.completion.chunk",
                                "created": self.created,
                                "model": self.model,
                                "choices": [{
                                    "index": 0,
                                    "delta": {
                                        "tool_calls": [{
                                            "index": self.tool_call_index,
                                            "id": call_id,
                                            "type": "function",
                                            "function": { "name": name, "arguments": "" }
                                        }]
                                    },
                                    "finish_reason": null
                                }]
                            });
                            self.queue
                                .push_back(Ok(Bytes::from(format!("data: {chunk}\n\n"))));
                        }
                    }
                    "response.function_call_arguments.delta" => {
                        self.has_tool_calls = true;
                        let delta = data.get("delta").and_then(|v| v.as_str()).unwrap_or("");
                        let chunk = serde_json::json!({
                            "id": self.chat_id,
                            "object": "chat.completion.chunk",
                            "created": self.created,
                            "model": self.model,
                            "choices": [{
                                "index": 0,
                                "delta": {
                                    "tool_calls": [{
                                        "index": self.tool_call_index,
                                        "function": { "arguments": delta }
                                    }]
                                },
                                "finish_reason": null
                            }]
                        });
                        self.queue
                            .push_back(Ok(Bytes::from(format!("data: {chunk}\n\n"))));
                    }
                    "response.output_item.done" => {
                        if data
                            .get("item")
                            .and_then(|i| i.get("type"))
                            .and_then(|v| v.as_str())
                            == Some("function_call")
                        {
                            self.tool_call_index += 1;
                        }
                    }
                    "response.completed" | "response.done" => {
                        let resp = data.get("response").unwrap_or(&data);
                        if let Some(usage) = resp.get("usage") {
                            if let Some(pt) = usage.get("input_tokens").and_then(|v| v.as_u64()) {
                                self.prompt_tokens = pt as u32;
                            }
                            if let Some(ct) = usage.get("output_tokens").and_then(|v| v.as_u64()) {
                                self.completion_tokens = ct as u32;
                            }
                        }
                    }
                    "response.failed" | "error" => {
                        let err_msg = data
                            .get("error")
                            .map(|e| e.to_string())
                            .unwrap_or_else(|| data.to_string());
                        let is_overloaded = err_msg.contains("server_is_overloaded")
                            || err_msg.contains("rate_limit")
                            || err_msg.contains("overloaded");
                        if let Some(mut lease) = self.lease.take() {
                            lease.commit_error(&err_msg, is_overloaded);
                        } else if let (Some(pool), Some(acc)) = (&self.pool, &self.account) {
                            pool.mark_error(acc, &err_msg, is_overloaded);
                        }
                        self.queue.push_back(Err(AppError::BadGateway(format!(
                            "Codex upstream error: {err_msg}"
                        ))));
                    }
                    _ => {}
                }
            }
        }
    }
}

impl Stream for CodexSseStream {
    type Item = Result<Bytes, AppError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.as_mut().get_mut();

        loop {
            if let Some(item) = this.queue.pop_front() {
                return Poll::Ready(Some(item));
            }

            if this.ended {
                if !this.sent_done {
                    this.sent_done = true;
                    if let Some(mut lease) = this.lease.take() {
                        lease.commit_success();
                    }
                    let finish_reason = if this.has_tool_calls {
                        "tool_calls"
                    } else {
                        "stop"
                    };
                    let finish_chunk = serde_json::json!({
                        "id": this.chat_id,
                        "object": "chat.completion.chunk",
                        "created": this.created,
                        "model": this.model,
                        "choices": [{
                            "index": 0,
                            "delta": {},
                            "finish_reason": finish_reason
                        }],
                        "usage": {
                            "prompt_tokens": this.prompt_tokens,
                            "completion_tokens": this.completion_tokens,
                            "total_tokens": this.prompt_tokens + this.completion_tokens
                        }
                    });
                    this.queue
                        .push_back(Ok(Bytes::from(format!("data: {finish_chunk}\n\n"))));
                    this.queue.push_back(Ok(Bytes::from("data: [DONE]\n\n")));
                    continue;
                }
                return Poll::Ready(None);
            }

            match Pin::new(&mut this.inner).poll_next(cx) {
                Poll::Ready(Some(Ok(chunk))) => {
                    if this.buffer.len() + chunk.len() > MAX_SSE_BUFFER_BYTES {
                        if let Some(mut lease) = this.lease.take() {
                            lease.commit_error(
                                "Codex upstream SSE stream buffer exceeded 2MB limit",
                                false,
                            );
                        }
                        return Poll::Ready(Some(Err(AppError::BadGateway(
                            "Codex upstream SSE stream buffer exceeded 2MB limit (memory safety guardrail)".to_string(),
                        ))));
                    }
                    this.buffer.extend_from_slice(&chunk);
                    while let Some(pos) = this.buffer.iter().position(|&b| b == b'\n') {
                        let line: Vec<u8> = this.buffer.drain(..=pos).collect();
                        this.process_line(&line);
                    }
                }
                Poll::Ready(Some(Err(err))) => {
                    let err_str = err.to_string();
                    if let Some(mut lease) = this.lease.take() {
                        lease.commit_error(&err_str, false);
                    }
                    return Poll::Ready(Some(Err(err)));
                }
                Poll::Ready(None) => {
                    this.ended = true;
                    if !this.buffer.is_empty() {
                        let remaining = std::mem::take(&mut this.buffer);
                        this.process_line(&remaining);
                    }
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

pub struct CodexLatencyStream {
    inner: BoxChatStream,
    trace: Option<crate::latency::CodexLatencyTrace>,
    latency_store: Option<Arc<crate::latency::LatencyStore>>,
    t4: Instant,
    req_start: Instant,
    completed: bool,
    has_error: bool,
}

impl Stream for CodexLatencyStream {
    type Item = Result<Bytes, AppError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.as_mut().get_mut();
        match Pin::new(&mut this.inner).poll_next(cx) {
            Poll::Ready(Some(Ok(item))) => Poll::Ready(Some(Ok(item))),
            Poll::Ready(Some(Err(err))) => {
                this.has_error = true;
                Poll::Ready(Some(Err(err)))
            }
            Poll::Ready(None) => {
                this.completed = true;
                if let Some(mut tr) = this.trace.take() {
                    let now = Instant::now();
                    tr.upstream_total_ms = (now - this.t4).as_secs_f64() * 1000.0;
                    tr.request_total_ms = (now - this.req_start).as_secs_f64() * 1000.0;
                    tr.status = if this.has_error {
                        "error".to_string()
                    } else if this.completed {
                        "completed".to_string()
                    } else {
                        "cancelled".to_string()
                    };
                    if let Some(store) = &this.latency_store {
                        store.push(tr);
                    }
                }
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl Drop for CodexLatencyStream {
    fn drop(&mut self) {
        if let Some(mut tr) = self.trace.take() {
            let now = Instant::now();
            tr.upstream_total_ms = (now - self.t4).as_secs_f64() * 1000.0;
            tr.request_total_ms = (now - self.req_start).as_secs_f64() * 1000.0;
            tr.status = if self.has_error {
                "error".to_string()
            } else if self.completed {
                "completed".to_string()
            } else {
                "cancelled".to_string()
            };
            if let Some(store) = &self.latency_store {
                store.push(tr);
            }
        }
    }
}

pub struct CodexProvider {
    pool: Arc<CodexAccountPool>,
    base_url: String,
    client: reqwest::Client,
    max_attempts: usize,
    native_non_stream: Option<bool>,
    latency_store: Option<Arc<crate::latency::LatencyStore>>,
}

impl std::fmt::Debug for CodexProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodexProvider")
            .field("base_url", &self.base_url)
            .field("max_attempts", &self.max_attempts)
            .field(
                "accounts_in_pool",
                &self.pool.accounts.read().unwrap().len(),
            )
            .field(
                "native_non_stream_enabled",
                &self.is_native_non_stream_enabled(),
            )
            .finish()
    }
}

impl CodexProvider {
    pub fn new(pool: Arc<CodexAccountPool>) -> Self {
        let base_url =
            std::env::var("CODEX_BASE_URL").unwrap_or_else(|_| CODEX_BASE_URL.to_string());
        Self::with_base_url(pool, &base_url)
    }

    pub fn with_base_url(pool: Arc<CodexAccountPool>, base_url: &str) -> Self {
        let timeout_secs = std::env::var("CODEX_REQUEST_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(600);
        let read_timeout_secs = std::env::var("CODEX_READ_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(300);

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .connect_timeout(Duration::from_secs(30))
            .read_timeout(Duration::from_secs(read_timeout_secs))
            .tcp_keepalive(Some(Duration::from_secs(30)))
            .build()
            .expect("Failed to build reqwest client for CodexProvider");
        Self {
            pool,
            base_url: base_url.trim_end_matches('/').to_string(),
            client,
            max_attempts: 4,
            native_non_stream: None,
            latency_store: None,
        }
    }

    pub fn with_latency_store(mut self, store: Arc<crate::latency::LatencyStore>) -> Self {
        self.latency_store = Some(store);
        self
    }

    pub fn with_native_non_stream(mut self, enabled: bool) -> Self {
        self.native_non_stream = Some(enabled);
        self
    }

    pub fn is_native_non_stream_enabled(&self) -> bool {
        if let Some(enabled) = self.native_non_stream {
            return enabled;
        }
        if let Ok(val) = std::env::var("CODEX_NATIVE_NON_STREAM_ENABLED") {
            let v = val.trim().to_lowercase();
            return v == "1" || v == "true" || v == "yes";
        }
        CODEX_NATIVE_NON_STREAM_ENABLED
    }

    pub fn pool(&self) -> &Arc<CodexAccountPool> {
        &self.pool
    }
}

#[axum::async_trait]
impl Provider for CodexProvider {
    async fn complete(
        &self,
        request: &ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, AppError> {
        let (converted_input, converted_tools) =
            openai_to_codex_input(&request.messages, request.tools.as_ref())?;

        let mut upstream_model = request.model.strip_prefix("cx/").unwrap_or(&request.model);
        if upstream_model == "astra" {
            upstream_model = "gpt-6-astra";
        }

        let is_native = self.is_native_non_stream_enabled();
        let body = serde_json::json!({
            "model": upstream_model,
            "input": converted_input,
            "stream": !is_native,
            "store": false,
            "tools": converted_tools,
        });

        let url = format!("{}/codex/responses", self.base_url);

        let active_count = self
            .pool
            .accounts
            .read()
            .unwrap()
            .values()
            .filter(|a| a.is_active.load(Ordering::SeqCst))
            .count();
        let max_attempts = self.max_attempts.min(active_count.max(1));
        let mut last_error = String::new();
        let mut total_queue_ms = 0.0_f64;
        let mut total_refresh_ms = 0.0_f64;

        let req_start = Instant::now();
        for attempt in 0..max_attempts {
            let t0 = Instant::now();
            let mut lease = match self.pool.acquire_lease_with_pacing().await {
                Some(l) => l,
                None => {
                    return Err(AppError::BadGateway(
                        "Không có tài khoản ChatGPT nào sẵn sàng (đang cooldown hoặc quá quota)"
                            .to_string(),
                    ));
                }
            };
            let t1 = Instant::now();
            let queue_or_pacing_ms = (t1 - t0).as_secs_f64() * 1000.0;
            total_queue_ms += queue_or_pacing_ms;
            let acc = lease.account.clone();

            let _gen_guard = acc.generation_lock.lock().await;

            let needs_refresh = {
                let now = current_time_secs();
                let exp = *acc.expires_at.read().unwrap();
                let tok = acc.access_token.read().unwrap();
                exp <= now + CODEX_TOKEN_REFRESH_LEAD_SECS || tok.is_empty()
            };
            let t2 = Instant::now();
            let _token = match self.pool.ensure_token(&acc).await {
                Ok(t) => t,
                Err(e) => {
                    lease.commit_error(&e, false);
                    last_error = e;
                    continue;
                }
            };
            let t3 = Instant::now();
            let token_refresh_ms = if needs_refresh {
                (t3 - t2).as_secs_f64() * 1000.0
            } else {
                0.0
            };
            total_refresh_ms += token_refresh_ms;

            let accept_hdr = if is_native {
                "application/json"
            } else {
                "text/event-stream"
            };
            let headers = acc.headers(accept_hdr, true);

            let t4 = Instant::now();
            let resp_res = self
                .client
                .post(&url)
                .headers(headers)
                .json(&body)
                .send()
                .await;

            let resp = match resp_res {
                Ok(r) => r,
                Err(e) => {
                    let err_msg = e.to_string();
                    let is_timeout = e.is_timeout() || e.is_connect();
                    lease
                        .commit_error_with_cooldown(&err_msg, if is_timeout { 30.0 } else { 15.0 });
                    last_error = err_msg;
                    continue;
                }
            };

            let status = resp.status();
            let t5 = Instant::now();
            let upstream_headers_ms = (t5 - t4).as_secs_f64() * 1000.0;

            if status == reqwest::StatusCode::UNAUTHORIZED && attempt == 0 {
                lease.release();
                let _ = self.pool.refresh_token(&acc, true).await;
                continue;
            }

            if !status.is_success() {
                let headers = resp.headers().clone();
                let err_text = resp.text().await.unwrap_or_default();
                if status == reqwest::StatusCode::BAD_REQUEST
                    || status == reqwest::StatusCode::UNPROCESSABLE_ENTITY
                {
                    lease.release();
                    return Err(AppError::BadRequest(format!(
                        "Codex upstream rejected request payload: {err_text}"
                    )));
                }

                if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                    let retry_after =
                        parse_retry_after(&headers).unwrap_or(COOLDOWN_AFTER_ERROR_SECS);
                    lease.commit_error_with_cooldown(&err_text, retry_after);
                    last_error = err_text;
                    continue;
                }

                if status == reqwest::StatusCode::FORBIDDEN
                    || err_text.to_lowercase().contains("account suspended")
                    || err_text.to_lowercase().contains("banned")
                    || err_text.to_lowercase().contains("account disabled")
                {
                    lease.commit_error_with_cooldown(&err_text, 3600.0);
                    last_error = err_text;
                    continue;
                }

                if status == reqwest::StatusCode::SERVICE_UNAVAILABLE
                    || status == reqwest::StatusCode::INTERNAL_SERVER_ERROR
                    || status == reqwest::StatusCode::BAD_GATEWAY
                    || status == reqwest::StatusCode::GATEWAY_TIMEOUT
                {
                    let retry_after = parse_retry_after(&headers).unwrap_or(30.0);
                    lease.commit_error_with_cooldown(&err_text, retry_after);
                    last_error = err_text;
                    continue;
                }

                lease.commit_error(&err_text, false);
                return Err(AppError::BadGateway(format!(
                    "Codex upstream {status}: {err_text}"
                )));
            }

            let body_bytes = resp
                .bytes()
                .await
                .map_err(|e| AppError::BadGateway(format!("Failed to read stream body: {e}")))?;
            let t6 = Instant::now();
            let upstream_total_ms = (t6 - t4).as_secs_f64() * 1000.0;

            let parse_result = if is_native {
                parse_codex_native_response(&body_bytes, &request.model)
            } else {
                parse_codex_response_body(&body_bytes, &request.model)
            };
            let t7 = Instant::now();
            let router_transform_ms = (t7 - t6).as_secs_f64() * 1000.0;
            let request_total_ms = (t7 - req_start).as_secs_f64() * 1000.0;

            match parse_result {
                Ok(response) => {
                    if let Some(store) = self.latency_store.as_ref() {
                        store.push(crate::latency::CodexLatencyTrace {
                            queue_or_pacing_ms: total_queue_ms,
                            token_refresh_ms: total_refresh_ms,
                            upstream_connect_ms: 0.0,
                            upstream_headers_ms,
                            upstream_ttfb_ms: 0.0,
                            upstream_total_ms,
                            router_transform_ms,
                            request_total_ms,
                            attempt_count: attempt + 1,
                            model: request.model.clone(),
                            is_stream: false,
                            timestamp_secs: SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs_f64(),
                            status: "completed".to_string(),
                        });
                    }
                    lease.commit_success();
                    return Ok(response);
                }
                Err(err) => {
                    let err_msg = err.to_string();
                    let is_retryable = err_msg.contains("server_is_overloaded")
                        || err_msg.contains("rate_limit")
                        || err_msg.contains("overloaded")
                        || err_msg.contains("server_error");
                    if is_retryable {
                        lease.commit_error(&err_msg, is_retryable);
                        last_error = err_msg;
                        continue;
                    } else {
                        lease.release();
                        return Err(err);
                    }
                }
            }
        }

        Err(AppError::BadGateway(if last_error.is_empty() {
            "All Codex accounts failed".to_string()
        } else {
            last_error
        }))
    }

    async fn stream_chat_completion(
        &self,
        request: &ChatCompletionRequest,
    ) -> Result<BoxChatStream, AppError> {
        let (converted_input, converted_tools) =
            openai_to_codex_input(&request.messages, request.tools.as_ref())?;

        let mut upstream_model = request.model.strip_prefix("cx/").unwrap_or(&request.model);
        if upstream_model == "astra" {
            upstream_model = "gpt-6-astra";
        }

        let body = serde_json::json!({
            "model": upstream_model,
            "input": converted_input,
            "stream": true,
            "store": false,
            "tools": converted_tools,
        });

        let url = format!("{}/codex/responses", self.base_url);

        let active_count = self
            .pool
            .accounts
            .read()
            .unwrap()
            .values()
            .filter(|a| a.is_active.load(Ordering::SeqCst))
            .count();
        let max_attempts = self.max_attempts.min(active_count.max(1));
        let mut last_error = String::new();
        let mut total_queue_ms = 0.0_f64;
        let mut total_refresh_ms = 0.0_f64;

        let req_start = Instant::now();
        for attempt in 0..max_attempts {
            let t0 = Instant::now();
            let mut lease = match self.pool.acquire_lease_with_pacing().await {
                Some(l) => l,
                None => {
                    return Err(AppError::BadGateway(
                        "Không có tài khoản ChatGPT nào sẵn sàng (đang cooldown hoặc quá quota)"
                            .to_string(),
                    ));
                }
            };
            let t1 = Instant::now();
            let queue_or_pacing_ms = (t1 - t0).as_secs_f64() * 1000.0;
            total_queue_ms += queue_or_pacing_ms;
            let acc = lease.account.clone();

            let _gen_guard = acc.generation_lock.lock().await;

            let needs_refresh = {
                let now = current_time_secs();
                let exp = *acc.expires_at.read().unwrap();
                let tok = acc.access_token.read().unwrap();
                exp <= now + CODEX_TOKEN_REFRESH_LEAD_SECS || tok.is_empty()
            };
            let t2 = Instant::now();
            let _token = match self.pool.ensure_token(&acc).await {
                Ok(t) => t,
                Err(e) => {
                    lease.commit_error(&e, false);
                    last_error = e;
                    continue;
                }
            };
            let t3 = Instant::now();
            let token_refresh_ms = if needs_refresh {
                (t3 - t2).as_secs_f64() * 1000.0
            } else {
                0.0
            };
            total_refresh_ms += token_refresh_ms;

            let headers = acc.headers("text/event-stream", true);

            let t4 = Instant::now();
            let resp_res = self
                .client
                .post(&url)
                .headers(headers)
                .json(&body)
                .send()
                .await;

            let resp = match resp_res {
                Ok(r) => r,
                Err(e) => {
                    let err_msg = e.to_string();
                    let is_timeout = e.is_timeout() || e.is_connect();
                    lease
                        .commit_error_with_cooldown(&err_msg, if is_timeout { 30.0 } else { 15.0 });
                    last_error = err_msg;
                    continue;
                }
            };

            let status = resp.status();
            let t5 = Instant::now();
            let upstream_connect_ms = (t5 - t4).as_secs_f64() * 1000.0;

            if status == reqwest::StatusCode::UNAUTHORIZED && attempt == 0 {
                lease.release();
                let _ = self.pool.refresh_token(&acc, true).await;
                continue;
            }

            if !status.is_success() {
                let headers = resp.headers().clone();
                let err_text = resp.text().await.unwrap_or_default();
                if status == reqwest::StatusCode::BAD_REQUEST
                    || status == reqwest::StatusCode::UNPROCESSABLE_ENTITY
                {
                    lease.release();
                    return Err(AppError::BadRequest(format!(
                        "Codex upstream rejected request payload: {err_text}"
                    )));
                }

                if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                    let retry_after =
                        parse_retry_after(&headers).unwrap_or(COOLDOWN_AFTER_ERROR_SECS);
                    lease.commit_error_with_cooldown(&err_text, retry_after);
                    last_error = err_text;
                    continue;
                }

                if status == reqwest::StatusCode::FORBIDDEN
                    || err_text.to_lowercase().contains("account suspended")
                    || err_text.to_lowercase().contains("banned")
                    || err_text.to_lowercase().contains("account disabled")
                {
                    lease.commit_error_with_cooldown(&err_text, 3600.0);
                    last_error = err_text;
                    continue;
                }

                if status == reqwest::StatusCode::SERVICE_UNAVAILABLE
                    || status == reqwest::StatusCode::INTERNAL_SERVER_ERROR
                    || status == reqwest::StatusCode::BAD_GATEWAY
                    || status == reqwest::StatusCode::GATEWAY_TIMEOUT
                {
                    let retry_after = parse_retry_after(&headers).unwrap_or(30.0);
                    lease.commit_error_with_cooldown(&err_text, retry_after);
                    last_error = err_text;
                    continue;
                }

                lease.commit_error(&err_text, false);
                return Err(AppError::BadGateway(format!(
                    "Codex upstream {status}: {err_text}"
                )));
            }

            let stream = resp.bytes_stream();
            let box_stream: BoxChatStream =
                Box::pin(futures_util::StreamExt::map(stream, |item| {
                    item.map_err(|e| {
                        error!("Codex upstream stream read error: {e}");
                        AppError::BadGateway(e.to_string())
                    })
                }));

            let mut codex_stream =
                CodexSseStream::with_lease(box_stream, request.model.clone(), lease);

            // Peek initial event to catch instant upstream failures (like server_is_overloaded / server_error)
            // and failover to the next account before committing to the HTTP stream response
            match futures_util::StreamExt::next(&mut codex_stream).await {
                Some(Ok(first_chunk)) => {
                    let t_first = Instant::now();
                    let upstream_ttfb_ms = (t_first - t4).as_secs_f64() * 1000.0;
                    codex_stream.queue.push_front(Ok(first_chunk));

                    let trace = crate::latency::CodexLatencyTrace {
                        queue_or_pacing_ms: total_queue_ms,
                        token_refresh_ms: total_refresh_ms,
                        upstream_connect_ms,
                        upstream_headers_ms: upstream_connect_ms,
                        upstream_ttfb_ms,
                        upstream_total_ms: 0.0,
                        router_transform_ms: 0.0,
                        request_total_ms: 0.0,
                        attempt_count: attempt + 1,
                        model: request.model.clone(),
                        is_stream: true,
                        timestamp_secs: SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs_f64(),
                        status: "completed".to_string(),
                    };

                    let wrapped_stream = CodexLatencyStream {
                        inner: Box::pin(codex_stream),
                        trace: Some(trace),
                        latency_store: self.latency_store.clone(),
                        t4,
                        req_start,
                        completed: false,
                        has_error: false,
                    };

                    return Ok(Box::pin(wrapped_stream));
                }
                Some(Err(err)) => {
                    let err_msg = err.to_string();
                    warn!(
                        "Codex initial stream peek error for account {}: {}",
                        acc.id, err_msg
                    );
                    let is_retryable = err_msg.contains("server_is_overloaded")
                        || err_msg.contains("rate_limit")
                        || err_msg.contains("overloaded")
                        || err_msg.contains("server_error")
                        || err_msg.contains("timed out")
                        || err_msg.contains("error decoding response body");
                    if is_retryable {
                        if let Some(mut l) = codex_stream.lease.take() {
                            l.commit_error(&err_msg, true);
                        }
                        last_error = err_msg;
                        continue;
                    } else {
                        return Err(err);
                    }
                }
                None => {
                    last_error = "Codex upstream stream ended prematurely".to_string();
                    if let Some(mut l) = codex_stream.lease.take() {
                        l.commit_error(&last_error, false);
                    }
                    continue;
                }
            }
        }

        Err(AppError::BadGateway(if last_error.is_empty() {
            "All Codex accounts failed".to_string()
        } else {
            last_error
        }))
    }
}

pub fn parse_codex_native_response(
    body_bytes: &[u8],
    model: &str,
) -> Result<ChatCompletionResponse, AppError> {
    if let Ok(parsed) = serde_json::from_slice::<ChatCompletionResponse>(body_bytes) {
        return Ok(parsed);
    }

    let val: serde_json::Value = serde_json::from_slice(body_bytes).map_err(|e| {
        let preview = String::from_utf8_lossy(body_bytes);
        AppError::BadGateway(format!(
            "Failed to parse Codex native JSON response: {e}; preview: {}",
            preview.chars().take(200).collect::<String>()
        ))
    })?;

    if let Some(err_val) = val.get("error") {
        let err_msg = if let Some(m) = err_val.get("message").and_then(|v| v.as_str()) {
            m.to_string()
        } else if let Some(s) = err_val.as_str() {
            s.to_string()
        } else {
            err_val.to_string()
        };
        return Err(AppError::BadGateway(format!(
            "Codex upstream error: {err_msg}"
        )));
    }

    if val.get("type").and_then(|v| v.as_str()) == Some("error")
        || val.get("status").and_then(|v| v.as_str()) == Some("failed")
    {
        let err_msg = val
            .get("error")
            .map(|e| e.to_string())
            .unwrap_or_else(|| val.to_string());
        return Err(AppError::BadGateway(format!(
            "Codex upstream error: {err_msg}"
        )));
    }

    let id = val
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("chatcmpl-{}", uuid::Uuid::new_v4().simple()));

    let resp_model = val
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or(model)
        .to_string();

    let created = val
        .get("created_at")
        .or_else(|| val.get("created"))
        .and_then(|v| v.as_i64())
        .unwrap_or_else(|| Utc::now().timestamp());

    let mut full_text = String::new();
    let mut tool_calls = Vec::new();

    if let Some(output_arr) = val.get("output").and_then(|v| v.as_array()) {
        for (item_idx, item) in output_arr.iter().enumerate() {
            let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
            match item_type {
                "message" => {
                    if let Some(content) = item.get("content") {
                        if let Some(content_arr) = content.as_array() {
                            for part in content_arr {
                                if let Some(t) = part.get("text").and_then(|v| v.as_str()) {
                                    full_text.push_str(t);
                                } else if let Some(t) = part.as_str() {
                                    full_text.push_str(t);
                                }
                            }
                        } else if let Some(text_str) = content.as_str() {
                            full_text.push_str(text_str);
                        }
                    }
                    if let Some(tc_arr) = item.get("tool_calls").and_then(|v| v.as_array()) {
                        tool_calls.extend(tc_arr.clone());
                    }
                }
                "function_call" => {
                    let raw_call_id = item
                        .get("call_id")
                        .or_else(|| item.get("id"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .trim();
                    let call_id = if !raw_call_id.is_empty() {
                        raw_call_id.to_string()
                    } else {
                        format!("call_{item_idx}_{}", uuid::Uuid::new_v4().simple())
                    };
                    let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let arguments =
                        if let Some(args_str) = item.get("arguments").and_then(|v| v.as_str()) {
                            args_str.to_string()
                        } else if let Some(args_val) = item.get("arguments") {
                            args_val.to_string()
                        } else {
                            String::new()
                        };
                    tool_calls.push(serde_json::json!({
                        "id": call_id,
                        "type": "function",
                        "function": {
                            "name": name,
                            "arguments": arguments,
                        }
                    }));
                }
                "output_text" => {
                    if let Some(t) = item.get("text").and_then(|v| v.as_str()) {
                        full_text.push_str(t);
                    }
                }
                _ => {}
            }
        }
    }

    if full_text.is_empty() {
        if let Some(ot) = val.get("output_text").and_then(|v| v.as_str()) {
            full_text.push_str(ot);
        }
    }

    let mut prompt_tokens = 0u32;
    let mut completion_tokens = 0u32;
    if let Some(usage) = val.get("usage") {
        if let Some(pt) = usage
            .get("input_tokens")
            .or_else(|| usage.get("prompt_tokens"))
            .and_then(|v| v.as_u64())
        {
            prompt_tokens = pt as u32;
        }
        if let Some(ct) = usage
            .get("output_tokens")
            .or_else(|| usage.get("completion_tokens"))
            .and_then(|v| v.as_u64())
        {
            completion_tokens = ct as u32;
        }
    }

    if completion_tokens == 0 && !full_text.is_empty() {
        completion_tokens = (full_text.len() as u32 / 4).max(1);
    }

    let finish_reason = if !tool_calls.is_empty() {
        "tool_calls".to_string()
    } else {
        "stop".to_string()
    };

    let message = ChatChoiceMessage {
        role: "assistant".to_string(),
        content: if full_text.is_empty() && !tool_calls.is_empty() {
            None
        } else {
            Some(full_text)
        },
        tool_calls: if tool_calls.is_empty() {
            None
        } else {
            Some(serde_json::Value::Array(tool_calls))
        },
    };

    Ok(ChatCompletionResponse {
        id,
        object: "chat.completion".to_string(),
        created,
        model: resp_model,
        choices: vec![ChatChoice {
            index: 0,
            message,
            finish_reason,
        }],
        usage: UsageInfo {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
        },
    })
}

pub fn parse_codex_response_body(
    body_bytes: &[u8],
    model: &str,
) -> Result<ChatCompletionResponse, AppError> {
    if let Ok(parsed) = serde_json::from_slice::<ChatCompletionResponse>(body_bytes) {
        return Ok(parsed);
    }

    if let Ok(val) = serde_json::from_slice::<serde_json::Value>(body_bytes) {
        if val.is_object()
            && (val.get("output").is_some()
                || val.get("output_text").is_some()
                || val.get("error").is_some()
                || val.get("choices").is_some())
        {
            return parse_codex_native_response(body_bytes, model);
        }
    }

    let text = String::from_utf8_lossy(body_bytes);
    let mut full_text = String::new();
    let mut prompt_tokens = 0u32;
    let mut completion_tokens = 0u32;
    let mut current_event = String::new();
    let mut tool_calls = Vec::new();
    let mut found_any = false;

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(ev) = line.strip_prefix("event:") {
            current_event = ev.trim().to_string();
            continue;
        }
        if let Some(rest) = line.strip_prefix("data:") {
            let data_str = rest.trim();
            if data_str == "[DONE]" {
                continue;
            }
            if let Ok(data) = serde_json::from_str::<serde_json::Value>(data_str) {
                found_any = true;
                let event_name = if !current_event.is_empty() {
                    std::mem::take(&mut current_event)
                } else {
                    data.get("type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string()
                };

                match event_name.as_str() {
                    "response.failed" | "error" => {
                        let err_msg = data
                            .get("error")
                            .map(|e| e.to_string())
                            .unwrap_or_else(|| data.to_string());
                        return Err(AppError::BadGateway(format!(
                            "Codex upstream error: {err_msg}"
                        )));
                    }
                    "response.output_text.delta" => {
                        if let Some(d) = data.get("delta") {
                            if let Some(s) = d.as_str() {
                                full_text.push_str(s);
                            } else if let Some(t) = d.get("text").and_then(|v| v.as_str()) {
                                full_text.push_str(t);
                            }
                        }
                    }
                    "response.output_item.added" => {
                        if data
                            .get("item")
                            .and_then(|i| i.get("type"))
                            .and_then(|v| v.as_str())
                            == Some("function_call")
                        {
                            let item = data.get("item").unwrap();
                            let raw_call_id = item
                                .get("call_id")
                                .or_else(|| item.get("id"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .trim();
                            let call_id = if !raw_call_id.is_empty() {
                                raw_call_id.to_string()
                            } else {
                                format!("call_{}", uuid::Uuid::new_v4().simple())
                            };
                            let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("");
                            tool_calls.push(serde_json::json!({
                                "id": call_id,
                                "type": "function",
                                "function": {
                                    "name": name,
                                    "arguments": ""
                                }
                            }));
                        }
                    }
                    "response.function_call_arguments.delta" => {
                        let delta = data.get("delta").and_then(|v| v.as_str()).unwrap_or("");
                        if let Some(last) = tool_calls.last_mut() {
                            if let Some(f) =
                                last.get_mut("function").and_then(|v| v.as_object_mut())
                            {
                                if let Some(args) = f.get_mut("arguments").and_then(|v| v.as_str())
                                {
                                    let mut new_args = args.to_string();
                                    new_args.push_str(delta);
                                    f.insert("arguments".to_string(), serde_json::json!(new_args));
                                }
                            }
                        }
                    }
                    "response.output_item.done" => {
                        if data
                            .get("item")
                            .and_then(|i| i.get("type"))
                            .and_then(|v| v.as_str())
                            == Some("function_call")
                        {
                            if let Some(args) = data
                                .get("item")
                                .and_then(|i| i.get("arguments"))
                                .and_then(|v| v.as_str())
                            {
                                if let Some(last) = tool_calls.last_mut() {
                                    if let Some(f) =
                                        last.get_mut("function").and_then(|v| v.as_object_mut())
                                    {
                                        f.insert("arguments".to_string(), serde_json::json!(args));
                                    }
                                }
                            }
                        }
                    }
                    "response.completed" | "response.done" => {
                        let resp = data.get("response").unwrap_or(&data);
                        if let Some(usage) = resp.get("usage") {
                            if let Some(pt) = usage.get("input_tokens").and_then(|v| v.as_u64()) {
                                prompt_tokens = pt as u32;
                            }
                            if let Some(ct) = usage.get("output_tokens").and_then(|v| v.as_u64()) {
                                completion_tokens = ct as u32;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    if found_any {
        if completion_tokens == 0 {
            completion_tokens = (full_text.len() as u32 / 4).max(1);
        }
        let finish_reason = if !tool_calls.is_empty() {
            "tool_calls".to_string()
        } else {
            "stop".to_string()
        };

        let message = ChatChoiceMessage {
            role: "assistant".to_string(),
            content: if full_text.is_empty() && !tool_calls.is_empty() {
                None
            } else {
                Some(full_text)
            },
            tool_calls: if tool_calls.is_empty() {
                None
            } else {
                Some(serde_json::Value::Array(tool_calls))
            },
        };

        return Ok(ChatCompletionResponse {
            id: format!("chatcmpl-{}", uuid::Uuid::new_v4().simple()),
            object: "chat.completion".to_string(),
            created: Utc::now().timestamp(),
            model: model.to_string(),
            choices: vec![ChatChoice {
                index: 0,
                message,
                finish_reason,
            }],
            usage: UsageInfo {
                prompt_tokens,
                completion_tokens,
                total_tokens: prompt_tokens + completion_tokens,
            },
        });
    }

    Err(AppError::BadGateway(format!(
        "Failed to parse Codex response: {}",
        text.chars().take(200).collect::<String>()
    )))
}

fn urlencoding_encode(s: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_pkce_pair() {
        let (verifier, challenge) = generate_pkce_pair();
        assert!(!verifier.is_empty());
        assert!(!challenge.is_empty());
        assert_ne!(verifier, challenge);
        assert!(!challenge.contains('='));
    }

    #[test]
    fn test_mask_codex_error() {
        let err = "Invalid token: 1234567890abcdef and secret: sk-12345";
        let masked = mask_codex_error(err);
        assert!(masked.contains("token: ••••"));
        assert!(!masked.contains("1234567890abcdef"));
        assert!(masked.contains("secret: ••••"));
        assert!(!masked.contains("sk-12345"));
    }

    #[test]
    fn test_jwt_claims_unverified() {
        let payload = serde_json::json!({
            "email": "user@example.com",
            "https://api.openai.com/auth": {
                "chatgpt_account_id": "acc_12345678"
            },
            "exp": 1800000000
        });
        let encoded_payload = URL_SAFE_NO_PAD.encode(payload.to_string().as_bytes());
        let dummy_jwt = format!("header.{encoded_payload}.sig");

        let claims = jwt_claims_unverified(&dummy_jwt);
        assert_eq!(claims["email"], "user@example.com");
        assert_eq!(claims["exp"], 1800000000);

        let (email, account_id) = extract_codex_token_identity(&serde_json::json!({
            "id_token": dummy_jwt,
            "access_token": dummy_jwt
        }));
        assert_eq!(email, "user@example.com");
        assert_eq!(account_id, "acc_12345678");
    }

    #[test]
    fn test_atomic_write_private_json() {
        let dir = std::env::temp_dir().join(format!("codex-test-{}", uuid::Uuid::new_v4()));
        let file_path = dir.join("auth.json");
        let data = serde_json::json!({"test": "data"});
        let res = atomic_write_private_json(&file_path, &data);
        assert!(res.is_ok());
        assert!(file_path.exists());
        let read = std::fs::read_to_string(&file_path).unwrap();
        assert!(read.contains("\"test\": \"data\""));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_openai_to_codex_input_translation() {
        let messages = vec![
            ChatMessage::system("System instruction"),
            ChatMessage::user("Hello codex"),
            ChatMessage::assistant(Some("Hi there"), None),
        ];
        let (input, tools) = openai_to_codex_input(&messages, None).unwrap();
        assert!(tools.is_none());
        let arr = input.as_array().unwrap();
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[0]["role"], "developer");
        assert_eq!(arr[1]["role"], "user");
        assert_eq!(arr[2]["role"], "assistant");
    }

    #[test]
    fn test_parse_codex_response_body_sse() {
        let sse = b"event: response.output_text.delta\ndata: {\"type\": \"response.output_text.delta\", \"delta\": \"Hello \"}\n\nevent: response.output_text.delta\ndata: {\"type\": \"response.output_text.delta\", \"delta\": \"world!\"}\n\nevent: response.completed\ndata: {\"type\": \"response.completed\", \"response\": {\"usage\": {\"input_tokens\": 15, \"output_tokens\": 2}}}\n\n";
        let resp = parse_codex_response_body(sse, "cx/gpt-5.6-sol").unwrap();
        assert_eq!(
            resp.choices[0].message.content.as_deref(),
            Some("Hello world!")
        );
        assert_eq!(resp.choices[0].finish_reason, "stop");
        assert_eq!(resp.usage.prompt_tokens, 15);
        assert_eq!(resp.usage.completion_tokens, 2);
    }

    #[test]
    fn test_codex_account_debug_redacts_tokens() {
        let acc = CodexAccount::new("acc-test", PathBuf::from("/tmp/auth.json"), true);
        *acc.access_token.write().unwrap() = "super-secret-access-token".to_string();
        *acc.refresh_token_value.write().unwrap() = "super-secret-refresh-token".to_string();

        let debug_str = format!("{acc:?}");
        assert!(debug_str.contains("[REDACTED]"));
        assert!(!debug_str.contains("super-secret-access-token"));
        assert!(!debug_str.contains("super-secret-refresh-token"));
    }

    #[test]
    fn test_codex_pool_status_and_list_json_redaction() {
        let db = Arc::new(Database::open_in_memory(None).unwrap());
        let pool = CodexPool::new(db);
        let tokens = serde_json::json!({
            "access_token": "sensitive-access-12345",
            "refresh_token": "sensitive-refresh-12345",
            "account_id": "act-sensitive-99"
        });
        pool.add_account_with_tokens(tokens, Some("acc-sensitive"))
            .unwrap();

        let status_json = pool.status();
        let status_str = status_json.to_string();
        assert!(!status_str.contains("sensitive-access-12345"));
        assert!(!status_str.contains("sensitive-refresh-12345"));

        let list_json = pool.list_accounts_json();
        let list_str = list_json.to_string();
        assert!(!list_str.contains("sensitive-access-12345"));
        assert!(!list_str.contains("sensitive-refresh-12345"));
    }

    #[test]
    fn test_codex_pool_quota_filtering() {
        let db = Arc::new(Database::open_in_memory(None).unwrap());
        let pool = CodexPool::new(db);

        // Account 1: used_percent = 99.0 (>= 98.0%, should be filtered out)
        let tokens1 = serde_json::json!({
            "access_token": "token-1",
            "refresh_token": "rt-1",
            "account_id": "acc-1"
        });
        pool.add_account_with_tokens(tokens1, Some("acc-exhausted"))
            .unwrap();
        let acc1 = pool
            .accounts
            .read()
            .unwrap()
            .get("acc-exhausted")
            .unwrap()
            .clone();
        *acc1.quota_cache.write().unwrap() = serde_json::json!({
            "primary_window": {
                "used_percent": 99.0
            }
        });

        // Account 2: used_percent = 50.0 (< 98.0%, should be picked)
        let tokens2 = serde_json::json!({
            "access_token": "token-2",
            "refresh_token": "rt-2",
            "account_id": "acc-2"
        });
        pool.add_account_with_tokens(tokens2, Some("acc-available"))
            .unwrap();
        let acc2 = pool
            .accounts
            .read()
            .unwrap()
            .get("acc-available")
            .unwrap()
            .clone();
        *acc2.quota_cache.write().unwrap() = serde_json::json!({
            "primary_window": {
                "used_percent": 50.0
            }
        });

        let picked = pool.pick_account().unwrap();
        assert_eq!(picked.id, "acc-available");
    }

    #[test]
    fn test_codex_pool_cooldown_and_reset() {
        let db = Arc::new(Database::open_in_memory(None).unwrap());
        let pool = CodexPool::new(db);

        let tokens = serde_json::json!({
            "access_token": "token-1",
            "refresh_token": "rt-1",
            "account_id": "acc-1"
        });
        pool.add_account_with_tokens(tokens, Some("acc-cd"))
            .unwrap();

        let acc = pool.accounts.read().unwrap().get("acc-cd").unwrap().clone();
        // Set cooldown in the future
        *acc.cooldown_until.write().unwrap() = current_time_secs() + 3600.0;
        assert!(pool.pick_account().is_none());

        // Reset cooldown
        pool.reset_account("acc-cd").unwrap();
        assert_eq!(*acc.cooldown_until.read().unwrap(), 0.0);
        assert!(pool.pick_account().is_some());
    }

    #[test]
    fn test_openai_to_codex_input_with_tools() {
        let messages = vec![ChatMessage::user("What's the weather?")];
        let tools = serde_json::json!([
            {
                "type": "function",
                "function": {
                    "name": "get_weather",
                    "description": "Get current weather",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "location": { "type": "string" }
                        },
                        "required": ["location"]
                    }
                }
            }
        ]);

        let (input, converted_tools) = openai_to_codex_input(&messages, Some(&tools)).unwrap();
        assert_eq!(input.as_array().unwrap().len(), 1);
        let ct = converted_tools.unwrap();
        let ct_arr = ct.as_array().unwrap();
        assert_eq!(ct_arr.len(), 1);
        assert_eq!(ct_arr[0]["type"], "function");
        assert_eq!(ct_arr[0]["name"], "get_weather");
        assert_eq!(ct_arr[0]["description"], "Get current weather");
    }

    #[test]
    fn test_parse_codex_response_body_with_tool_calls() {
        let sse = b"event: response.output_item.added\ndata: {\"type\": \"response.output_item.added\", \"item\": {\"type\": \"function_call\", \"id\": \"call_12345\", \"name\": \"get_weather\"}}\n\nevent: response.function_call_arguments.delta\ndata: {\"type\": \"response.function_call_arguments.delta\", \"delta\": \"{\\\"location\\\": \\\"San Francisco\\\"}\"}\n\nevent: response.output_item.done\ndata: {\"type\": \"response.output_item.done\", \"item\": {\"type\": \"function_call\", \"arguments\": \"{\\\"location\\\": \\\"San Francisco\\\"}\"}}\n\nevent: response.completed\ndata: {\"type\": \"response.completed\", \"response\": {\"usage\": {\"input_tokens\": 20, \"output_tokens\": 10}}}\n\n";
        let resp = parse_codex_response_body(sse, "cx/gpt-5.6-sol").unwrap();
        assert_eq!(resp.choices[0].finish_reason, "tool_calls");
        let tc = resp.choices[0].message.tool_calls.as_ref().unwrap();
        let tc_arr = tc.as_array().unwrap();
        assert_eq!(tc_arr.len(), 1);
        assert_eq!(tc_arr[0]["id"], "call_12345");
        assert_eq!(tc_arr[0]["type"], "function");
        assert_eq!(tc_arr[0]["function"]["name"], "get_weather");
        assert_eq!(
            tc_arr[0]["function"]["arguments"],
            "{\"location\": \"San Francisco\"}"
        );
        assert_eq!(resp.usage.prompt_tokens, 20);
        assert_eq!(resp.usage.completion_tokens, 10);
    }

    #[test]
    fn test_clean_expired_tickets() {
        let db = Arc::new(Database::open_in_memory(None).unwrap());
        let pool = CodexPool::new(db);

        // Insert an expired ticket
        pool.oauth_tickets.write().unwrap().insert(
            "expired-state".to_string(),
            CodexOAuthTicket {
                verifier: "verifier".to_string(),
                status: "pending".to_string(),
                email: None,
                error: None,
                expires_at: current_time_secs() - 100.0,
            },
        );

        // Insert a valid ticket
        pool.oauth_tickets.write().unwrap().insert(
            "valid-state".to_string(),
            CodexOAuthTicket {
                verifier: "verifier2".to_string(),
                status: "pending".to_string(),
                email: None,
                error: None,
                expires_at: current_time_secs() + 600.0,
            },
        );

        assert_eq!(pool.oauth_tickets.read().unwrap().len(), 2);
        pool.clean_expired_tickets();
        assert_eq!(pool.oauth_tickets.read().unwrap().len(), 1);
        assert!(pool
            .oauth_tickets
            .read()
            .unwrap()
            .contains_key("valid-state"));
    }

    #[test]
    fn test_regression_codex_multi_turn_continuation_with_null_assistant_content() {
        let messages = vec![
            ChatMessage::system("System instruction"),
            ChatMessage::user("Get AAPL stock price"),
            ChatMessage {
                role: "assistant".to_string(),
                content: None,
                name: None,
                tool_call_id: None,
                tool_calls: Some(serde_json::json!([{
                    "id": "call_stock_999",
                    "type": "function",
                    "function": {
                        "name": "get_stock_price",
                        "arguments": "{\"symbol\":\"AAPL\"}"
                    }
                }])),
            },
            ChatMessage {
                role: "tool".to_string(),
                content: Some(MessageContent::Text("{\"price\": 225.5}".to_string())),
                name: Some("get_stock_price".to_string()),
                tool_call_id: Some("call_stock_999".to_string()),
                tool_calls: None,
            },
        ];

        let (input, _) = openai_to_codex_input(&messages, None).unwrap();
        let arr = input.as_array().unwrap();
        assert_eq!(arr.len(), 4);

        assert_eq!(arr[0]["role"], "developer");
        assert_eq!(arr[1]["role"], "user");

        // Assistant tool_call is translated to function_call
        assert_eq!(arr[2]["type"], "function_call");
        assert_eq!(arr[2]["call_id"], "call_stock_999");
        assert_eq!(arr[2]["name"], "get_stock_price");

        // Tool result is translated to function_call_output
        assert_eq!(arr[3]["type"], "function_call_output");
        assert_eq!(arr[3]["call_id"], "call_stock_999");
        assert_eq!(arr[3]["output"], "{\"price\": 225.5}");

        // Now test assistant with null content AND NO tool calls
        let empty_assistant = vec![
            ChatMessage::user("Hi"),
            ChatMessage {
                role: "assistant".to_string(),
                content: None,
                name: None,
                tool_call_id: None,
                tool_calls: None,
            },
        ];
        let (input2, _) = openai_to_codex_input(&empty_assistant, None).unwrap();
        let arr2 = input2.as_array().unwrap();
        assert_eq!(arr2.len(), 2);
        assert_eq!(arr2[1]["role"], "assistant");
        assert_eq!(arr2[1]["content"][0]["text"], "");
    }

    #[test]
    fn test_regression_codex_missing_ids_normalized() {
        let messages = vec![
            ChatMessage::user("Missing tool ID test"),
            ChatMessage {
                role: "assistant".to_string(),
                content: None,
                name: None,
                tool_call_id: None,
                tool_calls: Some(serde_json::json!([{
                    "type": "function",
                    "function": {
                        "name": "calc",
                        "arguments": "{\"val\": 10}"
                    }
                }])),
            },
            ChatMessage {
                role: "tool".to_string(),
                content: Some(MessageContent::Text("100".to_string())),
                name: Some("calc".to_string()),
                tool_call_id: None,
                tool_calls: None,
            },
        ];

        let (input, _) = openai_to_codex_input(&messages, None).unwrap();
        let arr = input.as_array().unwrap();
        assert_eq!(arr.len(), 3);

        let call_id = arr[1]["call_id"].as_str().unwrap();
        assert!(!call_id.is_empty());
        let res_id = arr[2]["call_id"].as_str().unwrap();
        assert!(!res_id.is_empty());
        assert_eq!(call_id, res_id);
    }

    #[test]
    fn test_regression_codex_array_content_parts() {
        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: Some(MessageContent::Parts(vec![
                serde_json::json!({"type": "text", "text": "Part A, "}),
                serde_json::json!({"type": "input_text", "text": "Part B"}),
            ])),
            name: None,
            tool_call_id: None,
            tool_calls: None,
        }];

        let (input, _) = openai_to_codex_input(&messages, None).unwrap();
        let arr = input.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["role"], "user");
        let parts = arr[0]["content"].as_array().unwrap();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0]["text"], "Part A, ");
        assert_eq!(parts[1]["text"], "Part B");
    }

    #[test]
    fn test_regression_gemini_thought_signature_call_id_sanitized_for_codex() {
        let long_signature = "A".repeat(1150);
        let raw_call_id = format!("call_12345678:{long_signature}");
        assert_eq!(raw_call_id.len(), 1164);

        let messages = vec![
            ChatMessage::user("Do something"),
            ChatMessage {
                role: "assistant".to_string(),
                content: None,
                name: None,
                tool_call_id: None,
                tool_calls: Some(serde_json::json!([{
                    "id": raw_call_id,
                    "type": "function",
                    "function": {
                        "name": "terminal",
                        "arguments": "{\"command\":\"ls\"}"
                    }
                }])),
            },
            ChatMessage {
                role: "tool".to_string(),
                content: Some(MessageContent::Text("file.txt".to_string())),
                name: Some("terminal".to_string()),
                tool_call_id: Some(raw_call_id),
                tool_calls: None,
            },
        ];

        let (input, _) = openai_to_codex_input(&messages, None).unwrap();
        let arr = input.as_array().unwrap();
        assert_eq!(arr.len(), 3);

        let assistant_call_id = arr[1]["call_id"].as_str().unwrap();
        let tool_call_id = arr[2]["call_id"].as_str().unwrap();

        // 1. Must be trimmed of Gemini signature
        assert_eq!(assistant_call_id, "call_12345678");
        assert_eq!(tool_call_id, "call_12345678");

        // 2. Length must be strictly <= 64 chars
        assert!(assistant_call_id.len() <= 64);
        assert!(tool_call_id.len() <= 64);

        // 3. Must match exactly
        assert_eq!(assistant_call_id, tool_call_id);
    }

    #[tokio::test]
    async fn test_codex_lease_concurrency_and_cancellation_safety() {
        let db = Arc::new(Database::open_in_memory(None).unwrap());
        let pool = Arc::new(CodexPool::new(db));
        let tokens = serde_json::json!({
            "access_token": "lease-tok",
            "refresh_token": "lease-rt",
            "account_id": "acc-lease"
        });
        pool.add_account_with_tokens(tokens, Some("acc-lease"))
            .unwrap();

        // 1. Acquire lease 1
        let lease1 = pool.acquire_lease().expect("lease 1 should succeed");
        assert_eq!(lease1.account.id, "acc-lease");
        assert_eq!(lease1.account.in_flight.load(Ordering::SeqCst), 1);

        // 2. Second concurrent acquire fails due to max_concurrency = 1
        assert!(pool.acquire_lease().is_none());

        // 3. Drop lease1 without committing (simulates client disconnect)
        drop(lease1);

        // 4. In-flight decremented back to 0, account not in cooldown
        let acc = pool
            .accounts
            .read()
            .unwrap()
            .get("acc-lease")
            .unwrap()
            .clone();
        assert_eq!(acc.in_flight.load(Ordering::SeqCst), 0);
        assert_eq!(*acc.cooldown_until.read().unwrap(), 0.0);

        // 5. Subsequent acquire succeeds
        let mut lease2 = pool.acquire_lease().expect("lease 2 should succeed");
        lease2.commit_success();
        assert_eq!(acc.total_requests.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_codex_rate_limit_rpm_guardrail() {
        let db = Arc::new(Database::open_in_memory(None).unwrap());
        let mut pool_inst = CodexPool::new(db);
        pool_inst.guardrails.max_rpm = 2;
        pool_inst.guardrails.min_gap_seconds = 0.0;
        let pool = Arc::new(pool_inst);

        let tokens = serde_json::json!({
            "access_token": "rpm-tok",
            "refresh_token": "rpm-rt",
            "account_id": "acc-rpm"
        });
        pool.add_account_with_tokens(tokens, Some("acc-rpm"))
            .unwrap();

        let mut l1 = pool.acquire_lease().expect("l1");
        l1.commit_success();
        let mut l2 = pool.acquire_lease().expect("l2");
        l2.commit_success();

        // 3rd acquire rejected because recent_rpm == 2 >= max_rpm (2)
        assert!(pool.acquire_lease().is_none());
    }

    #[tokio::test]
    async fn test_codex_stream_memory_safety_buffer_limit() {
        use futures_util::stream;
        use futures_util::StreamExt;

        // Create stream with a chunk exceeding 2MB without newline
        let big_chunk = Bytes::from(vec![b'a'; MAX_SSE_BUFFER_BYTES + 10]);
        let stream_mock: BoxChatStream = Box::pin(stream::iter(vec![Ok(big_chunk)]));

        let mut sse_stream = CodexSseStream::new(stream_mock, "cx/gpt-5.6-sol".to_string());
        let res = sse_stream.next().await;
        assert!(res.is_some());
        match res.unwrap() {
            Err(AppError::BadGateway(msg)) => {
                assert!(msg.contains("2MB limit"));
            }
            other => panic!("Expected BadGateway buffer limit error, got: {other:?}"),
        }
    }

    #[test]
    fn test_parse_codex_native_response_basic() {
        let json_data = serde_json::json!({
            "id": "resp_test_001",
            "object": "response",
            "created_at": 1727184000,
            "model": "gpt-5.6-sol",
            "output": [
                {
                    "id": "item_msg_1",
                    "type": "message",
                    "role": "assistant",
                    "content": [
                        {
                            "type": "output_text",
                            "text": "Hello world from native parser!"
                        }
                    ]
                }
            ],
            "usage": {
                "input_tokens": 15,
                "output_tokens": 8,
                "total_tokens": 23
            }
        });
        let bytes = serde_json::to_vec(&json_data).unwrap();
        let resp = parse_codex_native_response(&bytes, "cx/gpt-5.6-sol").unwrap();

        assert_eq!(resp.id, "resp_test_001");
        assert_eq!(resp.model, "gpt-5.6-sol");
        assert_eq!(
            resp.choices[0].message.content.as_deref(),
            Some("Hello world from native parser!")
        );
        assert_eq!(resp.choices[0].finish_reason, "stop");
        assert_eq!(resp.usage.prompt_tokens, 15);
        assert_eq!(resp.usage.completion_tokens, 8);
        assert_eq!(resp.usage.total_tokens, 23);
    }

    #[test]
    fn test_parse_codex_native_response_with_tools() {
        let json_data = serde_json::json!({
            "id": "resp_tools_001",
            "object": "response",
            "created_at": 1727184000,
            "model": "gpt-5.6-sol",
            "output": [
                {
                    "id": "call_item_1",
                    "type": "function_call",
                    "call_id": "call_calc_123",
                    "name": "calculate",
                    "arguments": "{\"expr\": \"2+2\"}"
                }
            ],
            "usage": {
                "input_tokens": 25,
                "output_tokens": 10
            }
        });
        let bytes = serde_json::to_vec(&json_data).unwrap();
        let resp = parse_codex_native_response(&bytes, "cx/gpt-5.6-sol").unwrap();

        assert_eq!(resp.choices[0].finish_reason, "tool_calls");
        assert!(resp.choices[0].message.content.is_none());
        let tc = resp.choices[0].message.tool_calls.as_ref().unwrap();
        let tc_arr = tc.as_array().unwrap();
        assert_eq!(tc_arr.len(), 1);
        assert_eq!(tc_arr[0]["id"], "call_calc_123");
        assert_eq!(tc_arr[0]["type"], "function");
        assert_eq!(tc_arr[0]["function"]["name"], "calculate");
        assert_eq!(tc_arr[0]["function"]["arguments"], "{\"expr\": \"2+2\"}");
        assert_eq!(resp.usage.prompt_tokens, 25);
        assert_eq!(resp.usage.completion_tokens, 10);
    }

    #[test]
    fn test_parse_codex_native_response_error() {
        let json_data = serde_json::json!({
            "error": {
                "message": "upstream model overloaded"
            }
        });
        let bytes = serde_json::to_vec(&json_data).unwrap();
        let res = parse_codex_native_response(&bytes, "cx/gpt-5.6-sol");
        assert!(res.is_err());
        match res {
            Err(AppError::BadGateway(msg)) => {
                assert!(msg.contains("upstream model overloaded"));
            }
            other => panic!("Expected BadGateway error, got: {other:?}"),
        }
    }

    #[test]
    fn test_parse_codex_native_response_chat_completion_format() {
        let json_data = serde_json::json!({
            "id": "chatcmpl-standard",
            "object": "chat.completion",
            "created": 1727184000,
            "model": "gpt-5.6-sol",
            "choices": [
                {
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "Standard format response"
                    },
                    "finish_reason": "stop"
                }
            ],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5,
                "total_tokens": 15
            }
        });
        let bytes = serde_json::to_vec(&json_data).unwrap();
        let resp = parse_codex_native_response(&bytes, "cx/gpt-5.6-sol").unwrap();
        assert_eq!(resp.id, "chatcmpl-standard");
        assert_eq!(
            resp.choices[0].message.content.as_deref(),
            Some("Standard format response")
        );
    }

    #[tokio::test]
    async fn test_codex_native_non_stream_flag_off() {
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let mock_base_url = format!("http://127.0.0.1:{port}");

        let received_accept = Arc::new(std::sync::Mutex::new(String::new()));
        let received_body = Arc::new(std::sync::Mutex::new(Vec::new()));
        let accept_clone = received_accept.clone();
        let body_clone = received_body.clone();

        let mock_app = axum::Router::new().fallback(move |req: axum::extract::Request| {
            let accept_clone = accept_clone.clone();
            let body_clone = body_clone.clone();
            async move {
                let (parts, body) = req.into_parts();
                let accept = parts
                    .headers
                    .get("accept")
                    .and_then(|h| h.to_str().ok())
                    .unwrap_or("")
                    .to_string();
                *accept_clone.lock().unwrap() = accept;

                let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap_or_default();
                *body_clone.lock().unwrap() = bytes.to_vec();

                let sse_stream = "event: response.output_text.delta\ndata: {\"type\": \"response.output_text.delta\", \"delta\": \"Hello SSE!\"}\n\nevent: response.completed\ndata: {\"type\": \"response.completed\", \"response\": {\"usage\": {\"input_tokens\": 10, \"output_tokens\": 5}}}\n\n";
                (
                    axum::http::StatusCode::OK,
                    [("content-type", "text/event-stream")],
                    sse_stream,
                )
            }
        });

        tokio::spawn(async move {
            axum::serve(listener, mock_app).await.unwrap();
        });

        let test_dir = format!(
            "/tmp/ag-proxy-test-codex-flag-off-{}",
            uuid::Uuid::new_v4().simple()
        );
        let db = Arc::new(
            Database::open_or_create(&PathBuf::from(&test_dir), Some("test-key")).unwrap(),
        );
        let pool = Arc::new(CodexPool::with_components(
            db.clone(),
            Arc::new(MockCodexTokenRefresher::with_token("test-token")),
            Arc::new(MockCodexQuotaFetcher::new()),
            PathBuf::from(&test_dir).join("accounts"),
            PathBuf::from(&test_dir).join("default.json"),
            mock_base_url.clone(),
        ));

        pool.add_account_with_tokens(
            serde_json::json!({
                "access_token": "acc-token-off",
                "refresh_token": "acc-rt-off",
                "account_id": "acc-off"
            }),
            Some("acc-off"),
        )
        .unwrap();
        pool.load_from_db().unwrap();

        // Default provider has flag OFF
        let provider = CodexProvider::with_base_url(pool.clone(), &mock_base_url);
        assert!(!provider.is_native_non_stream_enabled());

        let req = ChatCompletionRequest {
            model: "cx/gpt-5.6-sol".to_string(),
            messages: vec![ChatMessage::user("Ping")],
            stream: Some(false),
            ..Default::default()
        };

        let resp = provider.complete(&req).await.unwrap();
        assert_eq!(
            resp.choices[0].message.content.as_deref(),
            Some("Hello SSE!")
        );
        assert_eq!(resp.usage.prompt_tokens, 10);
        assert_eq!(resp.usage.completion_tokens, 5);

        // Verify sent upstream request
        let accept = received_accept.lock().unwrap().clone();
        assert!(accept.contains("text/event-stream"));

        let sent_body: serde_json::Value =
            serde_json::from_slice(&received_body.lock().unwrap()).unwrap();
        assert_eq!(sent_body["stream"], true);

        let _ = std::fs::remove_dir_all(&test_dir);
    }

    #[tokio::test]
    async fn test_codex_native_non_stream_flag_on_success() {
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let mock_base_url = format!("http://127.0.0.1:{port}");

        let received_accept = Arc::new(std::sync::Mutex::new(String::new()));
        let received_body = Arc::new(std::sync::Mutex::new(Vec::new()));
        let accept_clone = received_accept.clone();
        let body_clone = received_body.clone();

        let mock_app = axum::Router::new().fallback(move |req: axum::extract::Request| {
            let accept_clone = accept_clone.clone();
            let body_clone = body_clone.clone();
            async move {
                let (parts, body) = req.into_parts();
                let accept = parts
                    .headers
                    .get("accept")
                    .and_then(|h| h.to_str().ok())
                    .unwrap_or("")
                    .to_string();
                *accept_clone.lock().unwrap() = accept;

                let bytes = axum::body::to_bytes(body, usize::MAX)
                    .await
                    .unwrap_or_default();
                *body_clone.lock().unwrap() = bytes.to_vec();

                let json_resp = serde_json::json!({
                    "id": "resp_native_success_123",
                    "object": "response",
                    "created_at": 1727184000,
                    "model": "gpt-5.6-sol",
                    "output": [
                        {
                            "id": "item_msg_1",
                            "type": "message",
                            "role": "assistant",
                            "content": [
                                {
                                    "type": "output_text",
                                    "text": "Hello from native non-stream!"
                                }
                            ]
                        }
                    ],
                    "usage": {
                        "input_tokens": 12,
                        "output_tokens": 7,
                        "total_tokens": 19
                    }
                });

                (
                    axum::http::StatusCode::OK,
                    [("content-type", "application/json")],
                    serde_json::to_string(&json_resp).unwrap(),
                )
            }
        });

        tokio::spawn(async move {
            axum::serve(listener, mock_app).await.unwrap();
        });

        let test_dir = format!(
            "/tmp/ag-proxy-test-codex-flag-on-{}",
            uuid::Uuid::new_v4().simple()
        );
        let db = Arc::new(
            Database::open_or_create(&PathBuf::from(&test_dir), Some("test-key")).unwrap(),
        );
        let pool = Arc::new(CodexPool::with_components(
            db.clone(),
            Arc::new(MockCodexTokenRefresher::with_token("test-token")),
            Arc::new(MockCodexQuotaFetcher::new()),
            PathBuf::from(&test_dir).join("accounts"),
            PathBuf::from(&test_dir).join("default.json"),
            mock_base_url.clone(),
        ));

        pool.add_account_with_tokens(
            serde_json::json!({
                "access_token": "acc-token-on",
                "refresh_token": "acc-rt-on",
                "account_id": "acc-on"
            }),
            Some("acc-on"),
        )
        .unwrap();
        pool.load_from_db().unwrap();

        // Flag ON via with_native_non_stream(true)
        let provider =
            CodexProvider::with_base_url(pool.clone(), &mock_base_url).with_native_non_stream(true);
        assert!(provider.is_native_non_stream_enabled());

        let req = ChatCompletionRequest {
            model: "cx/gpt-5.6-sol".to_string(),
            messages: vec![ChatMessage::user("Ping native")],
            stream: Some(false),
            ..Default::default()
        };

        let resp = provider.complete(&req).await.unwrap();
        assert_eq!(resp.id, "resp_native_success_123");
        assert_eq!(resp.model, "gpt-5.6-sol");
        assert_eq!(
            resp.choices[0].message.content.as_deref(),
            Some("Hello from native non-stream!")
        );
        assert_eq!(resp.choices[0].finish_reason, "stop");
        assert_eq!(resp.usage.prompt_tokens, 12);
        assert_eq!(resp.usage.completion_tokens, 7);
        assert_eq!(resp.usage.total_tokens, 19);

        // Verify upstream received stream=false and application/json
        let accept = received_accept.lock().unwrap().clone();
        assert_eq!(accept, "application/json");

        let sent_body: serde_json::Value =
            serde_json::from_slice(&received_body.lock().unwrap()).unwrap();
        assert_eq!(sent_body["stream"], false);

        let _ = std::fs::remove_dir_all(&test_dir);
    }

    #[tokio::test]
    async fn test_codex_native_non_stream_flag_on_429() {
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let mock_base_url = format!("http://127.0.0.1:{port}");

        let attempts = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let attempts_clone = attempts.clone();

        let mock_app = axum::Router::new().fallback(move |_req: axum::extract::Request| {
            let attempts = attempts_clone.clone();
            async move {
                let attempt = attempts.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if attempt == 0 {
                    // First attempt: 429 Too Many Requests
                    (
                        axum::http::StatusCode::TOO_MANY_REQUESTS,
                        [("content-type", "application/json"), ("retry-after", "45")],
                        "{\"error\": {\"message\": \"rate_limit_exceeded\"}}".to_string(),
                    )
                } else {
                    // Second attempt: 200 OK native JSON response
                    let json_resp = serde_json::json!({
                        "id": "resp_failover_ok",
                        "object": "response",
                        "output": [
                            {
                                "type": "message",
                                "role": "assistant",
                                "content": [
                                    {
                                        "type": "output_text",
                                        "text": "Failover native success!"
                                    }
                                ]
                            }
                        ],
                        "usage": {
                            "input_tokens": 10,
                            "output_tokens": 4
                        }
                    });
                    (
                        axum::http::StatusCode::OK,
                        [("content-type", "application/json"), ("retry-after", "0")],
                        serde_json::to_string(&json_resp).unwrap(),
                    )
                }
            }
        });

        tokio::spawn(async move {
            axum::serve(listener, mock_app).await.unwrap();
        });

        let test_dir = format!(
            "/tmp/ag-proxy-test-codex-429-{}",
            uuid::Uuid::new_v4().simple()
        );
        let db = Arc::new(
            Database::open_or_create(&PathBuf::from(&test_dir), Some("test-key")).unwrap(),
        );
        let pool = Arc::new(CodexPool::with_components(
            db.clone(),
            Arc::new(MockCodexTokenRefresher::with_token("test-token")),
            Arc::new(MockCodexQuotaFetcher::new()),
            PathBuf::from(&test_dir).join("accounts"),
            PathBuf::from(&test_dir).join("default.json"),
            mock_base_url.clone(),
        ));

        // Add 2 accounts
        pool.add_account_with_tokens(
            serde_json::json!({
                "access_token": "acc-1-tok",
                "refresh_token": "acc-1-rt",
                "account_id": "acc-1"
            }),
            Some("acc-1"),
        )
        .unwrap();

        pool.add_account_with_tokens(
            serde_json::json!({
                "access_token": "acc-2-tok",
                "refresh_token": "acc-2-rt",
                "account_id": "acc-2"
            }),
            Some("acc-2"),
        )
        .unwrap();

        pool.load_from_db().unwrap();

        let provider =
            CodexProvider::with_base_url(pool.clone(), &mock_base_url).with_native_non_stream(true);

        let req = ChatCompletionRequest {
            model: "cx/gpt-5.6-sol".to_string(),
            messages: vec![ChatMessage::user("Ping failover")],
            stream: Some(false),
            ..Default::default()
        };

        let resp = provider.complete(&req).await.unwrap();
        assert_eq!(
            resp.choices[0].message.content.as_deref(),
            Some("Failover native success!")
        );
        assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 2);

        // Verify account 1 was placed on cooldown
        let acc1 = pool.accounts.read().unwrap().get("acc-1").unwrap().clone();
        let cd1 = *acc1.cooldown_until.read().unwrap();
        assert!(cd1 > current_time_secs());

        let _ = std::fs::remove_dir_all(&test_dir);
    }

    #[tokio::test]
    async fn test_codex_native_non_stream_flag_on_400() {
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let mock_base_url = format!("http://127.0.0.1:{port}");

        let attempts = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let attempts_clone = attempts.clone();

        let mock_app = axum::Router::new().fallback(move |_req: axum::extract::Request| {
            let attempts = attempts_clone.clone();
            async move {
                attempts.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                (
                    axum::http::StatusCode::BAD_REQUEST,
                    [("content-type", "application/json")],
                    "{\"error\": {\"message\": \"Invalid request payload: bad parameter\"}}"
                        .to_string(),
                )
            }
        });

        tokio::spawn(async move {
            axum::serve(listener, mock_app).await.unwrap();
        });

        let test_dir = format!(
            "/tmp/ag-proxy-test-codex-400-{}",
            uuid::Uuid::new_v4().simple()
        );
        let db = Arc::new(
            Database::open_or_create(&PathBuf::from(&test_dir), Some("test-key")).unwrap(),
        );
        let pool = Arc::new(CodexPool::with_components(
            db.clone(),
            Arc::new(MockCodexTokenRefresher::with_token("test-token")),
            Arc::new(MockCodexQuotaFetcher::new()),
            PathBuf::from(&test_dir).join("accounts"),
            PathBuf::from(&test_dir).join("default.json"),
            mock_base_url.clone(),
        ));

        // Add 2 accounts
        pool.add_account_with_tokens(
            serde_json::json!({
                "access_token": "acc-1-tok",
                "refresh_token": "acc-1-rt",
                "account_id": "acc-1"
            }),
            Some("acc-1"),
        )
        .unwrap();

        pool.add_account_with_tokens(
            serde_json::json!({
                "access_token": "acc-2-tok",
                "refresh_token": "acc-2-rt",
                "account_id": "acc-2"
            }),
            Some("acc-2"),
        )
        .unwrap();

        pool.load_from_db().unwrap();

        let provider =
            CodexProvider::with_base_url(pool.clone(), &mock_base_url).with_native_non_stream(true);

        let req = ChatCompletionRequest {
            model: "cx/gpt-5.6-sol".to_string(),
            messages: vec![ChatMessage::user("Bad request test")],
            stream: Some(false),
            ..Default::default()
        };

        let result = provider.complete(&req).await;
        assert!(result.is_err());
        match result {
            Err(AppError::BadRequest(msg)) => {
                assert!(msg.contains("Invalid request payload"));
            }
            other => panic!("Expected BadRequest, got {other:?}"),
        }

        // Fail-fast: must ONLY attempt 1 time, NO failover/rotation to account 2
        assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 1);

        // Account 1 must NOT be put into cooldown
        let acc1 = pool.accounts.read().unwrap().get("acc-1").unwrap().clone();
        let cd1 = *acc1.cooldown_until.read().unwrap();
        assert_eq!(cd1, 0.0);

        let _ = std::fs::remove_dir_all(&test_dir);
    }

    #[tokio::test]
    async fn test_codex_latency_stream_completed_status() {
        use futures_util::StreamExt;
        let store = Arc::new(crate::latency::LatencyStore::new(true));
        let mock_inner = Box::pin(futures_util::stream::iter(vec![Ok(bytes::Bytes::from(
            "data: hello\n\n",
        ))]));
        let trace = crate::latency::CodexLatencyTrace {
            model: "cx/gpt-5.6-luna".to_string(),
            is_stream: true,
            timestamp_secs: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64(),
            ..Default::default()
        };
        let mut stream = CodexLatencyStream {
            inner: mock_inner,
            trace: Some(trace),
            latency_store: Some(store.clone()),
            t4: Instant::now(),
            req_start: Instant::now(),
            completed: false,
            has_error: false,
        };

        let chunk = stream.next().await;
        assert!(chunk.is_some());
        let end = stream.next().await;
        assert!(end.is_none());

        let snapshots = store.snapshot_recent(60.0);
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].status, "completed");
    }

    #[tokio::test]
    async fn test_codex_latency_stream_cancelled_status_on_drop() {
        use futures_util::StreamExt;
        let store = Arc::new(crate::latency::LatencyStore::new(true));
        let mock_inner = Box::pin(futures_util::stream::iter(vec![
            Ok(bytes::Bytes::from("data: chunk1\n\n")),
            Ok(bytes::Bytes::from("data: chunk2\n\n")),
        ]));
        let trace = crate::latency::CodexLatencyTrace {
            model: "cx/gpt-5.6-luna".to_string(),
            is_stream: true,
            timestamp_secs: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64(),
            ..Default::default()
        };
        let mut stream = CodexLatencyStream {
            inner: mock_inner,
            trace: Some(trace),
            latency_store: Some(store.clone()),
            t4: Instant::now(),
            req_start: Instant::now(),
            completed: false,
            has_error: false,
        };

        // Read first chunk, then drop before completion (simulating client disconnect)
        let chunk = stream.next().await;
        assert!(chunk.is_some());
        drop(stream);

        let snapshots = store.snapshot_recent(60.0);
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].status, "cancelled");
    }

    #[tokio::test]
    async fn test_codex_latency_stream_error_status_on_err_chunk() {
        use futures_util::StreamExt;
        let store = Arc::new(crate::latency::LatencyStore::new(true));
        let mock_inner = Box::pin(futures_util::stream::iter(vec![
            Ok(bytes::Bytes::from("data: chunk1\n\n")),
            Err(crate::error::AppError::BadGateway(
                "upstream stream error".to_string(),
            )),
        ]));
        let trace = crate::latency::CodexLatencyTrace {
            model: "cx/gpt-5.6-luna".to_string(),
            is_stream: true,
            timestamp_secs: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64(),
            ..Default::default()
        };
        let mut stream = CodexLatencyStream {
            inner: mock_inner,
            trace: Some(trace),
            latency_store: Some(store.clone()),
            t4: Instant::now(),
            req_start: Instant::now(),
            completed: false,
            has_error: false,
        };

        let chunk1 = stream.next().await;
        assert!(chunk1.is_some());
        let chunk2 = stream.next().await;
        assert!(chunk2.is_some());
        assert!(chunk2.unwrap().is_err());
        // Drop stream after error
        drop(stream);

        let snapshots = store.snapshot_recent(60.0);
        assert_eq!(snapshots.len(), 1, "Exactly one trace must be recorded");
        assert_eq!(snapshots[0].status, "error");
    }

    #[tokio::test]
    async fn test_codex_latency_stream_error_priority_over_completed() {
        use futures_util::StreamExt;
        let store = Arc::new(crate::latency::LatencyStore::new(true));
        let mock_inner = Box::pin(futures_util::stream::iter(vec![Err(
            crate::error::AppError::BadGateway("fatal chunk".to_string()),
        )]));
        let trace = crate::latency::CodexLatencyTrace {
            model: "cx/gpt-5.6-luna".to_string(),
            is_stream: true,
            timestamp_secs: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64(),
            ..Default::default()
        };
        let mut stream = CodexLatencyStream {
            inner: mock_inner,
            trace: Some(trace),
            latency_store: Some(store.clone()),
            t4: Instant::now(),
            req_start: Instant::now(),
            completed: false,
            has_error: false,
        };

        let err_chunk = stream.next().await;
        assert!(err_chunk.is_some());
        assert!(err_chunk.unwrap().is_err());
        // Stream then yields None (normal termination after error)
        let end = stream.next().await;
        assert!(end.is_none());
        drop(stream);

        let snapshots = store.snapshot_recent(60.0);
        assert_eq!(snapshots.len(), 1, "Exactly one trace must be recorded");
        assert_eq!(snapshots[0].status, "error");
    }
}
