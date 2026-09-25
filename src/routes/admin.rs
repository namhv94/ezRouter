use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
    Json,
};
use serde::{Deserialize, Serialize};

use crate::account::{
    AccountActionResponse, AccountQuotaResponse, AccountResponse, CreateAccountRequest,
    CreateAccountResponse, ListAccountsResponse, RefreshAllAccountsQuotaResponse,
};
use crate::auth::AdminUser;
use crate::db::{
    AdminStats, ApiKey, ComboRecord, ModelRequestSummary, ProviderResponse, RequestsResponse,
};
use crate::error::AppError;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct CreateApiKeyRequest {
    pub name: String,
    pub key: Option<String>,
    pub role: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateApiKeyRequest {
    pub is_active: bool,
}

#[derive(Debug, Serialize)]
pub struct DeleteApiKeyResponse {
    pub status: &'static str,
    pub id: String,
}

pub async fn list_api_keys(
    _auth: AdminUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<ApiKey>>, AppError> {
    let keys = state
        .db
        .list_keys()?
        .into_iter()
        .map(|mut key| {
            key.key = crate::db::mask_api_key(&key.key);
            key
        })
        .collect();
    Ok(Json(keys))
}

pub async fn create_api_key(
    _auth: AdminUser,
    State(state): State<AppState>,
    Json(payload): Json<CreateApiKeyRequest>,
) -> Result<(StatusCode, Json<ApiKey>), AppError> {
    let trimmed_name = payload.name.trim();
    if trimmed_name.is_empty() {
        return Err(AppError::BadRequest("name cannot be empty".to_string()));
    }
    let role = payload.role.as_deref().unwrap_or("client");
    let key = state
        .db
        .create_key_with_role(trimmed_name, payload.key.as_deref(), role)?;
    Ok((StatusCode::CREATED, Json(key)))
}

pub async fn update_api_key(
    _auth: AdminUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateApiKeyRequest>,
) -> Result<Json<ApiKey>, AppError> {
    let updated = state.db.set_key_active(&id, payload.is_active)?;
    if !updated {
        return Err(AppError::NotFound(format!("API key '{id}' not found")));
    }
    let mut key = state
        .db
        .get_key_by_id(&id)?
        .ok_or_else(|| AppError::NotFound(format!("API key '{id}' not found")))?;
    key.key = crate::db::mask_api_key(&key.key);
    Ok(Json(key))
}

pub async fn delete_api_key(
    _auth: AdminUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<DeleteApiKeyResponse>, AppError> {
    let deleted = state.db.delete_key(&id)?;
    if !deleted {
        return Err(AppError::NotFound(format!("API key '{id}' not found")));
    }
    Ok(Json(DeleteApiKeyResponse {
        status: "deleted",
        id,
    }))
}

#[derive(Debug, Deserialize)]
pub struct GetRequestsQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub model: Option<String>,
    pub status: Option<String>,
}

pub async fn get_stats(
    _auth: AdminUser,
    State(state): State<AppState>,
) -> Result<Json<AdminStats>, AppError> {
    let mut stats = state.db.get_admin_stats()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    let (google_total, google_active, google_cooldown, _, google_rpm) =
        state.account_pool.get_stats_summary(now);
    let (codex_total, codex_active, codex_cooldown, _, codex_rpm) =
        state.codex_pool.get_stats_summary(now);
    let total_acc = google_total + codex_total;
    let active_acc = google_active + codex_active;
    let cooldown_acc = google_cooldown + codex_cooldown;
    let active_rate = if total_acc > 0 {
        ((active_acc as f64 / total_acc as f64 * 100.0) * 10.0).round() / 10.0
    } else {
        0.0
    };
    let pool_rpm = google_rpm + codex_rpm;
    stats.total_accounts = total_acc;
    stats.active_accounts = active_acc;
    stats.cooldown_accounts = cooldown_acc;
    stats.active_rate = active_rate;
    stats.rpm = stats.rpm.max(pool_rpm as i64);
    Ok(Json(stats))
}

pub async fn get_requests(
    _auth: AdminUser,
    State(state): State<AppState>,
    Query(query): Query<GetRequestsQuery>,
) -> Result<Json<RequestsResponse>, AppError> {
    let limit = query.limit.unwrap_or(50);
    let offset = query.offset.unwrap_or(0);
    let resp = state.db.get_requests(
        limit,
        offset,
        query.model.as_deref(),
        query.status.as_deref(),
    )?;
    Ok(Json(resp))
}

pub async fn get_request_summary(
    _auth: AdminUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<ModelRequestSummary>>, AppError> {
    let summary = state.db.get_request_summary()?;
    Ok(Json(summary))
}

pub async fn get_active_requests(
    _auth: AdminUser,
    State(state): State<AppState>,
) -> Result<Json<crate::live_monitor::ActiveRequestsResponse>, AppError> {
    let active = state.live_registry.get_active();
    Ok(Json(active))
}

pub async fn get_active_requests_stream(
    _auth: AdminUser,
    State(state): State<AppState>,
) -> Sse<impl futures_util::stream::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let stream = futures_util::stream::unfold(state, |state| async move {
        let active = state.live_registry.get_active();
        let json = serde_json::to_string(&active).unwrap_or_else(|_| "{}".to_string());
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
        Some((Ok(Event::default().data(json)), state))
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}

#[derive(Debug, Deserialize)]
pub struct SystemLogsQuery {
    pub limit: Option<usize>,
    pub level: Option<String>,
    pub search: Option<String>,
}

pub async fn get_system_logs(
    _auth: AdminUser,
    State(state): State<AppState>,
    Query(query): Query<SystemLogsQuery>,
) -> Result<Json<crate::system_log::SystemLogsResponse>, AppError> {
    let limit = query.limit.unwrap_or(100);
    let resp = state
        .system_logs
        .get_logs(limit, query.level.as_deref(), query.search.as_deref());
    Ok(Json(resp))
}

pub async fn clear_system_logs(
    _auth: AdminUser,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    state.system_logs.clear();
    Ok(Json(serde_json::json!({ "success": true })))
}

pub fn validate_prefix(prefix: &str) -> Result<(), AppError> {
    let p = prefix.trim();
    if p.is_empty() || p.len() > 64 {
        return Err(AppError::BadRequest(
            "Prefix must be between 1 and 64 characters".to_string(),
        ));
    }
    if !p
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(AppError::BadRequest(
            "Prefix must contain only alphanumeric characters, dashes, or underscores".to_string(),
        ));
    }
    match p.to_ascii_lowercase().as_str() {
        "v1" | "admin" | "health" | "models" | "chat" => {
            Err(AppError::BadRequest(format!("Prefix '{p}' is reserved")))
        }
        _ => Ok(()),
    }
}

pub fn validate_provider_type(ptype: &str) -> Result<(), AppError> {
    const VALID_PROVIDER_TYPES: &[&str] = &[
        "openai",
        "openai-compatible",
        "openai_compatible",
        "anthropic",
        "gemini",
        "google",
        "azure",
        "azure_openai",
        "azure-openai",
        "openrouter",
        "deepseek",
        "groq",
        "ollama",
        "mistral",
        "custom",
        "antigravity",
    ];
    let normalized = ptype.trim().to_ascii_lowercase();
    if !VALID_PROVIDER_TYPES.contains(&normalized.as_str()) {
        return Err(AppError::BadRequest(format!(
            "Unsupported provider type '{ptype}'. Allowed types: {}",
            VALID_PROVIDER_TYPES.join(", ")
        )));
    }
    Ok(())
}

pub fn validate_base_url(url: &str) -> Result<(), AppError> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err(AppError::BadRequest("base_url cannot be empty".to_string()));
    }
    match reqwest::Url::parse(trimmed) {
        Ok(parsed) => {
            let scheme = parsed.scheme();
            if scheme != "http" && scheme != "https" {
                return Err(AppError::BadRequest(format!(
                    "Invalid base_url scheme '{scheme}': must be http or https"
                )));
            }
            if parsed.host_str().is_none() {
                return Err(AppError::BadRequest(
                    "Invalid base_url: missing host".to_string(),
                ));
            }
            Ok(())
        }
        Err(e) => Err(AppError::BadRequest(format!("Invalid base_url: {e}"))),
    }
}

pub fn validate_combo_strategy(strat: &str) -> Result<(), AppError> {
    const VALID_STRATEGIES: &[&str] = &[
        "round-robin",
        "round_robin",
        "fallback",
        "priority",
        "random",
        "weighted",
    ];
    let normalized = strat.trim().to_ascii_lowercase();
    if !VALID_STRATEGIES.contains(&normalized.as_str()) {
        return Err(AppError::BadRequest(format!(
            "Unsupported combo strategy '{strat}'. Allowed strategies: {}",
            VALID_STRATEGIES.join(", ")
        )));
    }
    Ok(())
}

pub fn validate_combo_models(models: &serde_json::Value) -> Result<(), AppError> {
    match models {
        serde_json::Value::Array(arr) => {
            if arr.is_empty() {
                return Err(AppError::BadRequest(
                    "Combo models list cannot be empty".to_string(),
                ));
            }
            for item in arr {
                match item {
                    serde_json::Value::String(s) if !s.trim().is_empty() => {}
                    serde_json::Value::Object(map)
                        if map.contains_key("model") || map.contains_key("id") => {}
                    _ => {
                        return Err(AppError::BadRequest(
                            "Each combo model must be a non-empty string or model object"
                                .to_string(),
                        ));
                    }
                }
            }
            Ok(())
        }
        _ => Err(AppError::BadRequest(
            "Combo models must be a JSON array".to_string(),
        )),
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateProviderRequest {
    pub name: String,
    pub prefix: String,
    #[serde(rename = "type")]
    pub provider_type: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub models: Option<serde_json::Value>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProviderRequest {
    pub id: Option<String>,
    pub name: Option<String>,
    pub prefix: Option<String>,
    #[serde(rename = "type")]
    pub provider_type: Option<String>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub models: Option<serde_json::Value>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct EntityIdQuery {
    pub id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct EntityIdBody {
    pub id: Option<String>,
}

pub async fn list_providers(
    _auth: AdminUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<ProviderResponse>>, AppError> {
    let providers = state.db.list_providers()?;
    let responses = providers.into_iter().map(|p| p.to_response()).collect();
    Ok(Json(responses))
}

pub async fn get_provider(
    _auth: AdminUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ProviderResponse>, AppError> {
    let provider = state
        .db
        .get_provider_by_id(&id)?
        .ok_or_else(|| AppError::NotFound(format!("Provider '{id}' not found")))?;
    Ok(Json(provider.to_response()))
}

pub async fn create_provider(
    _auth: AdminUser,
    State(state): State<AppState>,
    Json(payload): Json<CreateProviderRequest>,
) -> Result<(StatusCode, Json<ProviderResponse>), AppError> {
    let name = payload.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("name cannot be empty".to_string()));
    }
    let prefix = payload.prefix.trim();
    validate_prefix(prefix)?;
    if state.db.get_provider_by_prefix(prefix)?.is_some() {
        return Err(AppError::BadRequest(format!(
            "Provider prefix '{prefix}' already exists"
        )));
    }

    validate_provider_type(&payload.provider_type)?;
    validate_base_url(&payload.base_url)?;

    let models = payload.models.unwrap_or_else(|| serde_json::json!([]));
    let is_active = payload.is_active.unwrap_or(true);
    let api_key = payload.api_key.unwrap_or_default();

    let created = state.db.create_provider(
        name,
        prefix,
        payload.provider_type.trim(),
        payload.base_url.trim(),
        api_key.trim(),
        &models,
        is_active,
    )?;

    Ok((StatusCode::CREATED, Json(created.to_response())))
}

pub async fn update_provider(
    _auth: AdminUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateProviderRequest>,
) -> Result<Json<ProviderResponse>, AppError> {
    let existing = state
        .db
        .get_provider_by_id(&id)?
        .ok_or_else(|| AppError::NotFound(format!("Provider '{id}' not found")))?;

    if let Some(ref n) = payload.name {
        if n.trim().is_empty() {
            return Err(AppError::BadRequest("name cannot be empty".to_string()));
        }
    }

    if let Some(ref p) = payload.prefix {
        let p_trimmed = p.trim();
        validate_prefix(p_trimmed)?;
        if p_trimmed != existing.prefix && state.db.get_provider_by_prefix(p_trimmed)?.is_some() {
            return Err(AppError::BadRequest(format!(
                "Provider prefix '{p_trimmed}' already exists"
            )));
        }
    }

    if let Some(ref t) = payload.provider_type {
        validate_provider_type(t)?;
    }

    if let Some(ref b) = payload.base_url {
        validate_base_url(b)?;
    }

    let updated = state.db.update_provider(
        &id,
        payload.name.as_deref().map(|s| s.trim()),
        payload.prefix.as_deref().map(|s| s.trim()),
        payload.provider_type.as_deref().map(|s| s.trim()),
        payload.base_url.as_deref().map(|s| s.trim()),
        payload.api_key.as_deref().map(|s| s.trim()),
        payload.models.as_ref(),
        payload.is_active,
    )?;

    let record = updated.ok_or_else(|| AppError::NotFound(format!("Provider '{id}' not found")))?;
    Ok(Json(record.to_response()))
}

pub async fn update_provider_root(
    auth: AdminUser,
    state: State<AppState>,
    Json(payload): Json<UpdateProviderRequest>,
) -> Result<Json<ProviderResponse>, AppError> {
    let target_id = payload.id.clone().ok_or_else(|| {
        AppError::BadRequest("id is required in body to update provider".to_string())
    })?;
    update_provider(auth, state, Path(target_id), Json(payload)).await
}

pub async fn delete_provider(
    _auth: AdminUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let deleted = state.db.delete_provider(&id)?;
    if !deleted {
        return Err(AppError::NotFound(format!("Provider '{id}' not found")));
    }
    Ok(Json(serde_json::json!({
        "status": "deleted",
        "id": id,
    })))
}

pub async fn delete_provider_root(
    auth: AdminUser,
    state: State<AppState>,
    Query(query): Query<EntityIdQuery>,
    body: Option<Json<EntityIdBody>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let target_id = query
        .id
        .or_else(|| body.and_then(|b| b.id.clone()))
        .ok_or_else(|| AppError::BadRequest("id is required to delete provider".to_string()))?;
    delete_provider(auth, state, Path(target_id)).await
}

#[derive(Debug, Deserialize)]
pub struct FetchModelsRequest {
    pub provider_id: Option<String>,
    #[serde(rename = "type")]
    pub provider_type: Option<String>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
}

pub async fn fetch_models(
    _auth: AdminUser,
    State(state): State<AppState>,
    Json(payload): Json<FetchModelsRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let (base_url, ptype, api_key) = if let Some(ref pid) = payload.provider_id {
        let p = state
            .db
            .get_provider_by_id(pid)?
            .ok_or_else(|| AppError::NotFound(format!("Provider '{pid}' not found")))?;
        (
            payload.base_url.unwrap_or(p.base_url),
            payload.provider_type.unwrap_or(p.provider_type),
            payload.api_key.unwrap_or(p.api_key),
        )
    } else {
        let base_url = payload
            .base_url
            .ok_or_else(|| AppError::BadRequest("base_url is required".to_string()))?;
        let ptype = payload
            .provider_type
            .unwrap_or_else(|| "openai".to_string());
        let api_key = payload.api_key.unwrap_or_default();
        (base_url, ptype, api_key)
    };

    validate_base_url(&base_url)?;
    validate_provider_type(&ptype)?;

    if state.config.use_mock_provider {
        return Ok(Json(serde_json::json!({
            "status": "ok",
            "models": [
                "gpt-4o",
                "gpt-4o-mini",
                "claude-3-5-sonnet",
                "gemini-2.0-flash",
                "deepseek-chat"
            ]
        })));
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| AppError::Internal(format!("Failed to build HTTP client: {e}")))?;

    let url = format!("{}/models", base_url.trim_end_matches('/'));
    let mut req = client.get(&url);
    if !api_key.trim().is_empty() {
        req = req.bearer_auth(&api_key);
    }

    let res = req
        .send()
        .await
        .map_err(|e| AppError::BadGateway(format!("Failed to fetch models from {url}: {e}")))?;

    if !res.status().is_success() {
        return Err(AppError::BadGateway(format!(
            "Upstream returned HTTP {}",
            res.status()
        )));
    }

    let body: serde_json::Value = res
        .json()
        .await
        .map_err(|e| AppError::BadGateway(format!("Invalid JSON from upstream: {e}")))?;

    let model_names: Vec<String> = if let Some(data) = body.get("data").and_then(|d| d.as_array()) {
        data.iter()
            .filter_map(|m| {
                m.get("id")
                    .and_then(|id| id.as_str())
                    .map(|s| s.to_string())
            })
            .collect()
    } else if let Some(arr) = body.as_array() {
        arr.iter()
            .filter_map(|m| {
                m.as_str().map(|s| s.to_string()).or_else(|| {
                    m.get("id")
                        .and_then(|id| id.as_str())
                        .map(|s| s.to_string())
                })
            })
            .collect()
    } else {
        Vec::new()
    };

    Ok(Json(serde_json::json!({
        "status": "ok",
        "models": model_names,
    })))
}

pub async fn test_provider(
    _auth: AdminUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let provider = state
        .db
        .get_provider_by_id(&id)?
        .ok_or_else(|| AppError::NotFound(format!("Provider '{id}' not found")))?;

    if state.config.use_mock_provider {
        return Ok(Json(serde_json::json!({
            "status": "ok",
            "success": true,
            "message": format!(
                "Provider '{}' ({}) connection successful",
                provider.name, provider.provider_type
            ),
            "latency_ms": 12.5,
        })));
    }

    let start = std::time::Instant::now();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| AppError::Internal(format!("Failed to build HTTP client: {e}")))?;

    let url = format!("{}/models", provider.base_url.trim_end_matches('/'));
    let mut req = client.get(&url);
    if !provider.api_key.trim().is_empty() {
        req = req.bearer_auth(&provider.api_key);
    }

    match req.send().await {
        Ok(res) if res.status().is_success() => {
            let latency_ms = start.elapsed().as_secs_f64() * 1000.0;
            Ok(Json(serde_json::json!({
                "status": "ok",
                "success": true,
                "message": format!("Provider '{}' connection successful", provider.name),
                "latency_ms": (latency_ms * 10.0).round() / 10.0,
            })))
        }
        Ok(res) => {
            let latency_ms = start.elapsed().as_secs_f64() * 1000.0;
            let status = res.status();
            Ok(Json(serde_json::json!({
                "status": "error",
                "success": false,
                "message": format!("Upstream responded with status {status}"),
                "latency_ms": (latency_ms * 10.0).round() / 10.0,
            })))
        }
        Err(e) => {
            let latency_ms = start.elapsed().as_secs_f64() * 1000.0;
            Ok(Json(serde_json::json!({
                "status": "error",
                "success": false,
                "message": format!("Connection failed: {e}"),
                "latency_ms": (latency_ms * 10.0).round() / 10.0,
            })))
        }
    }
}

pub async fn sync_models(
    _auth: AdminUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let provider = state
        .db
        .get_provider_by_id(&id)?
        .ok_or_else(|| AppError::NotFound(format!("Provider '{id}' not found")))?;

    let models_val: serde_json::Value = if state.config.use_mock_provider {
        serde_json::json!([
            "gpt-4o",
            "gpt-4o-mini",
            "claude-3-5-sonnet",
            "gemini-2.0-flash",
            "deepseek-chat"
        ])
    } else {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .map_err(|e| AppError::Internal(format!("Failed to build HTTP client: {e}")))?;

        let url = format!("{}/models", provider.base_url.trim_end_matches('/'));
        let mut req = client.get(&url);
        if !provider.api_key.trim().is_empty() {
            req = req.bearer_auth(&provider.api_key);
        }

        let res = req
            .send()
            .await
            .map_err(|e| AppError::BadGateway(format!("Failed to fetch models from {url}: {e}")))?;

        let body: serde_json::Value = res
            .json()
            .await
            .map_err(|e| AppError::BadGateway(format!("Invalid JSON from upstream: {e}")))?;

        let model_names: Vec<String> =
            if let Some(data) = body.get("data").and_then(|d| d.as_array()) {
                data.iter()
                    .filter_map(|m| {
                        m.get("id")
                            .and_then(|id| id.as_str())
                            .map(|s| s.to_string())
                    })
                    .collect()
            } else if let Some(arr) = body.as_array() {
                arr.iter()
                    .filter_map(|m| {
                        m.as_str().map(|s| s.to_string()).or_else(|| {
                            m.get("id")
                                .and_then(|id| id.as_str())
                                .map(|s| s.to_string())
                        })
                    })
                    .collect()
            } else {
                Vec::new()
            };
        serde_json::json!(model_names)
    };

    let updated = state
        .db
        .update_provider_models(&id, &models_val)?
        .ok_or_else(|| AppError::NotFound(format!("Provider '{id}' not found")))?;

    Ok(Json(serde_json::json!({
        "status": "ok",
        "id": id,
        "models": updated.models,
        "provider": updated.to_response(),
    })))
}

#[derive(Debug, Deserialize)]
pub struct CreateComboRequest {
    pub name: String,
    pub models: serde_json::Value,
    pub strategy: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateComboRequest {
    pub id: Option<String>,
    pub name: Option<String>,
    pub models: Option<serde_json::Value>,
    pub strategy: Option<String>,
}

pub async fn list_combos(
    _auth: AdminUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<ComboRecord>>, AppError> {
    let combos = state.db.list_combos()?;
    Ok(Json(combos))
}

pub async fn get_combo(
    _auth: AdminUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ComboRecord>, AppError> {
    let combo = state
        .db
        .get_combo_by_id(&id)?
        .ok_or_else(|| AppError::NotFound(format!("Combo '{id}' not found")))?;
    Ok(Json(combo))
}

pub async fn create_combo(
    _auth: AdminUser,
    State(state): State<AppState>,
    Json(payload): Json<CreateComboRequest>,
) -> Result<(StatusCode, Json<ComboRecord>), AppError> {
    let name = payload.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("name cannot be empty".to_string()));
    }
    if state.db.get_combo_by_name(name)?.is_some() {
        return Err(AppError::BadRequest(format!(
            "Combo name '{name}' already exists"
        )));
    }

    validate_combo_strategy(&payload.strategy)?;
    validate_combo_models(&payload.models)?;

    let created = state
        .db
        .create_combo(name, &payload.models, payload.strategy.trim())?;

    Ok((StatusCode::CREATED, Json(created)))
}

pub async fn update_combo(
    _auth: AdminUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateComboRequest>,
) -> Result<Json<ComboRecord>, AppError> {
    let existing = state
        .db
        .get_combo_by_id(&id)?
        .ok_or_else(|| AppError::NotFound(format!("Combo '{id}' not found")))?;

    if let Some(ref n) = payload.name {
        let n_trimmed = n.trim();
        if n_trimmed.is_empty() {
            return Err(AppError::BadRequest("name cannot be empty".to_string()));
        }
        if n_trimmed != existing.name && state.db.get_combo_by_name(n_trimmed)?.is_some() {
            return Err(AppError::BadRequest(format!(
                "Combo name '{n_trimmed}' already exists"
            )));
        }
    }

    if let Some(ref s) = payload.strategy {
        validate_combo_strategy(s)?;
    }

    if let Some(ref m) = payload.models {
        validate_combo_models(m)?;
    }

    let updated = state.db.update_combo(
        &id,
        payload.name.as_deref().map(|s| s.trim()),
        payload.models.as_ref(),
        payload.strategy.as_deref().map(|s| s.trim()),
    )?;

    let record = updated.ok_or_else(|| AppError::NotFound(format!("Combo '{id}' not found")))?;
    Ok(Json(record))
}

pub async fn update_combo_root(
    auth: AdminUser,
    state: State<AppState>,
    Json(payload): Json<UpdateComboRequest>,
) -> Result<Json<ComboRecord>, AppError> {
    let target_id = payload.id.clone().ok_or_else(|| {
        AppError::BadRequest("id is required in body to update combo".to_string())
    })?;
    update_combo(auth, state, Path(target_id), Json(payload)).await
}

pub async fn delete_combo(
    _auth: AdminUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let deleted = state.db.delete_combo(&id)?;
    if !deleted {
        return Err(AppError::NotFound(format!("Combo '{id}' not found")));
    }
    Ok(Json(serde_json::json!({
        "status": "deleted",
        "id": id,
    })))
}

pub async fn delete_combo_root(
    auth: AdminUser,
    state: State<AppState>,
    Query(query): Query<EntityIdQuery>,
    body: Option<Json<EntityIdBody>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let target_id = query
        .id
        .or_else(|| body.and_then(|b| b.id.clone()))
        .ok_or_else(|| AppError::BadRequest("id is required to delete combo".to_string()))?;
    delete_combo(auth, state, Path(target_id)).await
}

pub async fn list_accounts(
    _auth: AdminUser,
    State(state): State<AppState>,
) -> Result<Json<ListAccountsResponse>, AppError> {
    // Only refresh missing quota with bounded concurrency to prevent upstream storms
    let missing_quota_ids: Vec<String> = state
        .account_pool
        .list_accounts()
        .into_iter()
        .filter(|a| {
            a.is_active
                && a.cooldown_remaining <= 0.0
                && (a.quota.is_null() || a.quota.as_object().map(|m| m.is_empty()).unwrap_or(true))
        })
        .map(|a| a.id)
        .collect();

    if !missing_quota_ids.is_empty() {
        let sem = std::sync::Arc::new(tokio::sync::Semaphore::new(2));
        let mut join_set = tokio::task::JoinSet::new();
        for id in missing_quota_ids {
            let pool = state.account_pool.clone();
            let sem_clone = sem.clone();
            join_set.spawn(async move {
                let _permit = sem_clone.acquire_owned().await.ok();
                let _ = pool.refresh_quota(&id, false).await;
            });
        }
        while join_set.join_next().await.is_some() {}
    }

    let accounts = state.account_pool.list_accounts();
    let total = accounts.len();
    Ok(Json(ListAccountsResponse { accounts, total }))
}

pub async fn get_account(
    _auth: AdminUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<AccountResponse>, AppError> {
    let acc = state
        .account_pool
        .get_account(&id)
        .ok_or_else(|| AppError::NotFound(format!("Account '{id}' not found")))?;
    Ok(Json(acc))
}

pub async fn create_account(
    _auth: AdminUser,
    State(state): State<AppState>,
    Json(payload): Json<CreateAccountRequest>,
) -> Result<(StatusCode, Json<CreateAccountResponse>), AppError> {
    let refresh_token = payload.refresh_token.unwrap_or_default().trim().to_string();
    if refresh_token.is_empty() {
        return Err(AppError::BadRequest("refresh_token required".to_string()));
    }
    let email = payload.email.unwrap_or_default().trim().to_string();
    if !email.is_empty() && state.db.get_account_by_email(&email)?.is_some() {
        return Err(AppError::BadRequest(format!(
            "Account with email '{email}' already exists"
        )));
    }

    let created = state.account_pool.add_account(&email, &refresh_token)?;
    Ok((
        StatusCode::CREATED,
        Json(CreateAccountResponse {
            id: created.id,
            email: created.email,
        }),
    ))
}

pub async fn delete_account(
    _auth: AdminUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<AccountActionResponse>, AppError> {
    let deleted = state.account_pool.remove_account(&id)?;
    if !deleted {
        return Err(AppError::NotFound(format!("Account '{id}' not found")));
    }
    Ok(Json(AccountActionResponse { ok: true }))
}

pub async fn reset_account_cooldown(
    _auth: AdminUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<AccountActionResponse>, AppError> {
    let reset = state.account_pool.reset_cooldown(&id)?;
    if !reset {
        return Err(AppError::NotFound(format!("Account '{id}' not found")));
    }
    Ok(Json(AccountActionResponse { ok: true }))
}

pub async fn refresh_account_quota(
    _auth: AdminUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<AccountQuotaResponse>, AppError> {
    let quota = state.account_pool.refresh_quota(&id, true).await?;
    Ok(Json(AccountQuotaResponse {
        account_id: id,
        quota,
    }))
}

pub async fn refresh_all_accounts_quota(
    _auth: AdminUser,
    State(state): State<AppState>,
) -> Result<Json<RefreshAllAccountsQuotaResponse>, AppError> {
    let accounts = state.account_pool.list_accounts();
    let total = accounts.len();

    let sem = std::sync::Arc::new(tokio::sync::Semaphore::new(2));
    let mut join_set = tokio::task::JoinSet::new();
    for acc in accounts {
        let pool = state.account_pool.clone();
        let sem_clone = sem.clone();
        join_set.spawn(async move {
            let _permit = sem_clone.acquire_owned().await.ok();
            pool.refresh_quota(&acc.id, true).await.is_ok()
        });
    }

    let mut refreshed = 0;
    while let Some(res) = join_set.join_next().await {
        if let Ok(true) = res {
            refreshed += 1;
        }
    }

    Ok(Json(RefreshAllAccountsQuotaResponse {
        ok: true,
        total,
        refreshed,
    }))
}

#[derive(Debug, Deserialize)]
pub struct DeleteAccountQuery {
    pub id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DeleteAccountBody {
    pub id: Option<String>,
}

pub async fn delete_account_root(
    auth: AdminUser,
    state: State<AppState>,
    Query(query): Query<DeleteAccountQuery>,
    body: Option<Json<DeleteAccountBody>>,
) -> Result<Json<AccountActionResponse>, AppError> {
    let target_id = query
        .id
        .or_else(|| body.and_then(|b| b.id.clone()))
        .ok_or_else(|| AppError::BadRequest("id is required to delete account".to_string()))?;
    delete_account(auth, state, Path(target_id)).await
}

#[derive(Debug, Deserialize)]
pub struct TestAccountRequest {
    pub model: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TestAccountResponse {
    pub ok: bool,
    pub latency_ms: u64,
    pub message: String,
}

pub async fn test_account(
    _auth: AdminUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    payload: Option<Json<TestAccountRequest>>,
) -> Result<Json<TestAccountResponse>, AppError> {
    let model = payload.and_then(|p| p.model.clone());
    let (ok, latency_ms, message) = state
        .account_pool
        .test_account(&id, model.as_deref())
        .await?;
    Ok(Json(TestAccountResponse {
        ok,
        latency_ms,
        message,
    }))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Import9RouterResponse {
    pub imported: usize,
    pub total: usize,
}

pub async fn import_9router(
    _auth: AdminUser,
    State(state): State<AppState>,
) -> Result<Json<Import9RouterResponse>, AppError> {
    let (imported, total) = state.account_pool.import_from_9router(None)?;
    Ok(Json(Import9RouterResponse { imported, total }))
}

// ─── Codex Admin Handlers ───

pub async fn get_codex_status(
    _auth: AdminUser,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(state.codex_pool.status()))
}

pub async fn get_codex_latency(
    _auth: AdminUser,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let traces = state.latency_store.snapshot_recent(900.0);
    let agg = crate::latency::aggregate(&traces, 900.0);
    Json(agg)
}

pub async fn list_codex_accounts(
    _auth: AdminUser,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(state.codex_pool.list_accounts_json()))
}

#[derive(Debug, Deserialize)]
pub struct CreateCodexAccountRequest {
    pub auth_path: Option<String>,
    pub email: Option<String>,
    pub tokens: Option<serde_json::Value>,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub id_token: Option<String>,
    pub account_id: Option<String>,
}

pub async fn create_codex_account(
    _auth: AdminUser,
    State(state): State<AppState>,
    Json(payload): Json<CreateCodexAccountRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    if let Some(tokens_val) = payload.tokens {
        let rec = state.codex_pool.add_account_with_tokens(tokens_val, None)?;
        return Ok((
            StatusCode::CREATED,
            Json(serde_json::json!({
                "ok": true,
                "id": rec.id,
                "email": rec.email.unwrap_or_default()
            })),
        ));
    }

    if let (Some(at), Some(rt)) = (payload.access_token, payload.refresh_token) {
        let tokens_val = serde_json::json!({
            "access_token": at,
            "refresh_token": rt,
            "id_token": payload.id_token.unwrap_or_default(),
            "account_id": payload.account_id.unwrap_or_default(),
        });
        let rec = state.codex_pool.add_account_with_tokens(tokens_val, None)?;
        return Ok((
            StatusCode::CREATED,
            Json(serde_json::json!({
                "ok": true,
                "id": rec.id,
                "email": rec.email.unwrap_or_default()
            })),
        ));
    }

    if let Some(path_str) = payload.auth_path {
        let path = std::path::PathBuf::from(&path_str);
        if !path.exists() {
            return Err(AppError::BadRequest(format!(
                "Auth file not found: {path_str}"
            )));
        }
        let rec = state
            .db
            .create_codex_account(None, payload.email.as_deref(), &path_str, true)?;
        let _ = state.codex_pool.load_from_db();
        return Ok((
            StatusCode::CREATED,
            Json(serde_json::json!({
                "ok": true,
                "id": rec.id,
                "email": rec.email.unwrap_or_default()
            })),
        ));
    }

    Err(AppError::BadRequest(
        "Either auth_path or tokens must be provided".to_string(),
    ))
}

pub async fn toggle_codex_account(
    _auth: AdminUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let new_active = state.codex_pool.toggle_account(&id)?;
    Ok(Json(serde_json::json!({
        "ok": true,
        "id": id,
        "is_active": new_active
    })))
}

pub async fn reset_codex_account(
    _auth: AdminUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let _ = state.codex_pool.reset_account(&id)?;
    Ok(Json(serde_json::json!({
        "ok": true,
        "id": id
    })))
}

pub async fn refresh_codex_account_quota(
    _auth: AdminUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let acc = state
        .codex_pool
        .accounts
        .read()
        .unwrap()
        .get(&id)
        .cloned()
        .ok_or_else(|| AppError::NotFound("Codex account not found".to_string()))?;
    let quota = state
        .codex_pool
        .fetch_account_quota(&acc, true)
        .await
        .unwrap_or_else(|_| serde_json::json!({}));
    Ok(Json(serde_json::json!({
        "ok": true,
        "id": id,
        "quota": quota
    })))
}

pub async fn delete_codex_account(
    _auth: AdminUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let _ = state.codex_pool.delete_account(&id)?;
    Ok(Json(serde_json::json!({
        "ok": true,
        "deleted_id": id
    })))
}

pub async fn refresh_codex_quota(
    _auth: AdminUser,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    state.codex_pool.fetch_all_quotas(true).await;
    Ok(Json(state.codex_pool.status()))
}

pub async fn start_codex_oauth(
    _auth: AdminUser,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(state.codex_pool.start_oauth()))
}

fn percent_decode_str(input: &str) -> String {
    let mut result = Vec::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) =
                u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16)
            {
                result.push(byte);
                i += 3;
                continue;
            }
        } else if bytes[i] == b'+' {
            result.push(b' ');
            i += 1;
            continue;
        }
        result.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(result).unwrap_or_else(|_| input.to_string())
}

fn extract_oauth_code_and_state(qs_or_url: &str) -> (Option<String>, Option<String>) {
    let mut code = None;
    let mut state = None;
    let url_without_fragment = qs_or_url.split('#').next().unwrap_or(qs_or_url);
    let qs = if let Some(pos) = url_without_fragment.find('?') {
        &url_without_fragment[pos + 1..]
    } else {
        url_without_fragment
    };
    for pair in qs.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            let decoded_v = percent_decode_str(v);
            if k == "code" {
                code = Some(decoded_v);
            } else if k == "state" {
                state = Some(decoded_v);
            }
        }
    }
    (code, state)
}

#[derive(Debug, Deserialize)]
pub struct ExchangeCodexOAuthRequest {
    pub code: Option<String>,
    pub callback_url: Option<String>,
    pub state: Option<String>,
    pub ticket_id: Option<String>,
}

pub async fn exchange_codex_oauth(
    _auth: AdminUser,
    State(state): State<AppState>,
    Json(payload): Json<ExchangeCodexOAuthRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let mut code = payload.code.unwrap_or_default().trim().to_string();
    let mut st = payload.state.or(payload.ticket_id);

    if let Some(ref cb_url) = payload.callback_url {
        let (extracted_code, extracted_state) = extract_oauth_code_and_state(cb_url);
        if let Some(c) = extracted_code {
            code = c;
        }
        if st.is_none() {
            st = extracted_state;
        }
    }

    if code.contains("code=")
        || code.contains('?')
        || code.starts_with("http://")
        || code.starts_with("https://")
    {
        let (extracted_code, extracted_state) = extract_oauth_code_and_state(&code);
        if let Some(c) = extracted_code {
            code = c;
        }
        if st.is_none() {
            st = extracted_state;
        }
    }

    if code.is_empty() {
        return Err(AppError::BadRequest(
            "code or callback_url containing code is required".to_string(),
        ));
    }

    let email: String = state
        .codex_pool
        .exchange_oauth_code(&code, st.as_deref(), None)
        .await?;
    Ok(Json(serde_json::json!({
        "ok": true,
        "email": email
    })))
}

#[derive(Debug, Deserialize)]
pub struct CodexOAuthStatusQuery {
    pub state: Option<String>,
    pub ticket_id: Option<String>,
}

pub async fn get_codex_oauth_status(
    _auth: AdminUser,
    State(state): State<AppState>,
    Query(query): Query<CodexOAuthStatusQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let state_str = query.state.or(query.ticket_id).unwrap_or_default();
    Ok(Json(state.codex_pool.oauth_status(&state_str)))
}

#[derive(Debug, Deserialize, Default)]
pub struct StartGoogleOAuthQuery {
    pub origin: Option<String>,
    pub redirect_uri: Option<String>,
}

pub async fn start_google_oauth(
    _auth: AdminUser,
    State(state): State<AppState>,
    Query(query): Query<StartGoogleOAuthQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let redirect_uri = query
        .redirect_uri
        .as_deref()
        .unwrap_or(&state.config.oauth_redirect_uri);
    let safe_origin = query
        .origin
        .as_deref()
        .filter(|o| crate::routes::auth::is_allowed_origin(o, &state.config.oauth_redirect_uri));
    let res = state.account_pool.start_oauth(redirect_uri, safe_origin);
    Ok(Json(
        serde_json::to_value(res).unwrap_or_else(|_| serde_json::json!({})),
    ))
}

#[derive(Debug, Deserialize)]
pub struct ExchangeGoogleOAuthRequest {
    pub code: Option<String>,
    pub callback_url: Option<String>,
    pub redirect_uri: Option<String>,
    pub state: Option<String>,
    pub ticket_id: Option<String>,
}

pub async fn exchange_google_oauth(
    _auth: AdminUser,
    State(state): State<AppState>,
    Json(payload): Json<ExchangeGoogleOAuthRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let mut code = payload.code.unwrap_or_default().trim().to_string();
    let mut st = payload.state.or(payload.ticket_id);

    if let Some(ref cb_url) = payload.callback_url {
        let (extracted_code, extracted_state) = extract_oauth_code_and_state(cb_url);
        if let Some(c) = extracted_code {
            code = c;
        }
        if st.is_none() {
            st = extracted_state;
        }
    }

    if code.contains("code=")
        || code.contains('?')
        || code.starts_with("http://")
        || code.starts_with("https://")
    {
        let (extracted_code, extracted_state) = extract_oauth_code_and_state(&code);
        if let Some(c) = extracted_code {
            code = c;
        }
        if st.is_none() {
            st = extracted_state;
        }
    }

    if code.is_empty() {
        return Err(AppError::BadRequest(
            "code or callback_url containing code is required".to_string(),
        ));
    }

    let validated_redirect_uri = state
        .account_pool
        .validate_and_consume_state(st.as_deref())?;

    let redirect_uri = payload.redirect_uri.unwrap_or(validated_redirect_uri);

    let (account_id, email, is_new) = state
        .account_pool
        .exchange_oauth_code(&code, &redirect_uri, None, None)
        .await?;

    Ok(Json(serde_json::json!({
        "ok": true,
        "account_id": account_id,
        "email": email,
        "is_new": is_new,
        "action": if is_new { "created" } else { "updated" }
    })))
}

#[derive(Debug, Deserialize)]
pub struct UpdateQuotaRefreshRequest {
    pub enabled: Option<bool>,
    pub interval_secs: Option<u64>,
}

pub async fn get_quota_refresh_status(
    _auth: AdminUser,
    State(state): State<AppState>,
) -> Result<Json<crate::quota_refresh::QuotaRefreshStatusResponse>, AppError> {
    Ok(Json(state.quota_worker.status()))
}

pub async fn update_quota_refresh_settings(
    _auth: AdminUser,
    State(state): State<AppState>,
    Json(payload): Json<UpdateQuotaRefreshRequest>,
) -> Result<Json<crate::quota_refresh::QuotaRefreshStatusResponse>, AppError> {
    if let Some(interval) = payload.interval_secs {
        if interval < crate::quota_refresh::MIN_QUOTA_REFRESH_INTERVAL_SECS {
            return Err(AppError::BadRequest(format!(
                "interval_secs must be at least {} seconds",
                crate::quota_refresh::MIN_QUOTA_REFRESH_INTERVAL_SECS
            )));
        }
        state
            .quota_worker
            .set_interval_secs(interval)
            .map_err(|e| AppError::Internal(e.to_string()))?;
    }
    if let Some(enabled) = payload.enabled {
        state
            .quota_worker
            .set_enabled(enabled)
            .map_err(|e| AppError::Internal(e.to_string()))?;
    }
    Ok(Json(state.quota_worker.status()))
}

pub async fn run_quota_refresh(
    _auth: AdminUser,
    State(state): State<AppState>,
) -> Result<Json<crate::quota_refresh::QuotaRefreshStatusResponse>, AppError> {
    match state.quota_worker.run_refresh().await {
        Ok(_) => Ok(Json(state.quota_worker.status())),
        Err(e) if e.contains("already in progress") => Err(AppError::BadRequest(
            "Quota refresh is already in progress".to_string(),
        )),
        Err(e) => Err(AppError::Internal(e)),
    }
}

pub async fn get_token_saver_settings(
    _auth: AdminUser,
    State(state): State<AppState>,
) -> Result<Json<crate::token_saver::TokenSaverSettings>, AppError> {
    let settings = state.db.get_token_saver_settings()?;
    Ok(Json(settings))
}

pub async fn update_token_saver_settings(
    _auth: AdminUser,
    State(state): State<AppState>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    let obj = payload.as_object().ok_or_else(|| {
        AppError::BadRequest("Settings payload must be a JSON object".to_string())
    })?;

    let allowed: std::collections::HashSet<&str> = [
        "token_saver_enabled",
        "rtk_enabled",
        "caveman_level",
        "ponytail_level",
    ]
    .into_iter()
    .collect();

    let mut unknown = Vec::new();
    for k in obj.keys() {
        if !allowed.contains(k.as_str()) {
            unknown.push(k.clone());
        }
    }
    if !unknown.is_empty() {
        unknown.sort();
        return Err(AppError::BadRequest(format!(
            "Unknown settings: {}",
            unknown.join(", ")
        )));
    }

    let mut normalized_updates = std::collections::HashMap::new();

    for (k, v) in obj {
        if k == "token_saver_enabled" || k == "rtk_enabled" {
            let str_val = if let Some(b) = v.as_bool() {
                if b { "true" } else { "false" }.to_string()
            } else if let Some(n) = v.as_i64() {
                if n == 1 {
                    "true".to_string()
                } else if n == 0 {
                    "false".to_string()
                } else {
                    return Err(AppError::BadRequest(format!("{k} must be true or false")));
                }
            } else if let Some(s) = v.as_str() {
                let lowered = s.trim().to_ascii_lowercase();
                match lowered.as_str() {
                    "1" | "true" | "yes" | "on" => "true".to_string(),
                    "0" | "false" | "no" | "off" => "false".to_string(),
                    _ => return Err(AppError::BadRequest(format!("{k} must be true or false"))),
                }
            } else {
                return Err(AppError::BadRequest(format!("{k} must be a boolean")));
            };
            normalized_updates.insert(k.clone(), str_val);
        } else {
            let s = v
                .as_str()
                .ok_or_else(|| {
                    AppError::BadRequest(format!("{k} must be off, lite, full, or ultra"))
                })?
                .trim()
                .to_ascii_lowercase();
            if !crate::token_saver::is_valid_token_saver_level(&s) {
                return Err(AppError::BadRequest(format!(
                    "{k} must be off, lite, full, or ultra"
                )));
            }
            normalized_updates.insert(k.clone(), s);
        }
    }

    let updated_settings = state.db.update_token_saver_settings(&normalized_updates)?;

    Ok(Json(serde_json::json!({
        "ok": true,
        "token_saver_enabled": updated_settings.token_saver_enabled,
        "rtk_enabled": updated_settings.rtk_enabled,
        "caveman_level": updated_settings.caveman_level,
        "ponytail_level": updated_settings.ponytail_level,
    })))
}
