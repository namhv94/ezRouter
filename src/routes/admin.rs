use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::auth::AdminUser;
use crate::db::{
    AdminStats, ApiKey, ComboRecord, ModelRequestSummary, ProviderResponse, RequestsResponse,
    TokenAnalyticsResponse,
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
    let stats = state.db.get_admin_stats()?;
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

#[derive(Debug, Deserialize)]
pub struct TokenAnalyticsQuery {
    pub period: Option<String>,
}

pub async fn get_token_analytics(
    _auth: AdminUser,
    State(state): State<AppState>,
    Query(query): Query<TokenAnalyticsQuery>,
) -> Result<Json<TokenAnalyticsResponse>, AppError> {
    let analytics = state.db.get_token_analytics(query.period.as_deref())?;
    Ok(Json(analytics))
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
    let rx = state.live_registry.subscribe();
    let initial = state.live_registry.get_active();
    let initial_json = serde_json::to_string(&initial).unwrap_or_else(|_| "{}".to_string());

    enum StreamState {
        Initial(
            String,
            tokio::sync::broadcast::Receiver<crate::live_monitor::ActiveRequestsResponse>,
            AppState,
        ),
        Streaming(
            tokio::sync::broadcast::Receiver<crate::live_monitor::ActiveRequestsResponse>,
            AppState,
        ),
    }

    let stream = futures_util::stream::unfold(
        StreamState::Initial(initial_json, rx, state),
        |st| async move {
            match st {
                StreamState::Initial(json, rx, state) => Some((
                    Ok(Event::default().data(json)),
                    StreamState::Streaming(rx, state),
                )),
                StreamState::Streaming(mut rx, state) => match rx.recv().await {
                    Ok(active) => {
                        let json =
                            serde_json::to_string(&active).unwrap_or_else(|_| "{}".to_string());
                        Some((
                            Ok(Event::default().data(json)),
                            StreamState::Streaming(rx, state),
                        ))
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        let active = state.live_registry.get_active();
                        let json =
                            serde_json::to_string(&active).unwrap_or_else(|_| "{}".to_string());
                        Some((
                            Ok(Event::default().data(json)),
                            StreamState::Streaming(rx, state),
                        ))
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => None,
                },
            }
        },
    );

    Sse::new(stream).keep_alive(KeepAlive::default())
}

#[derive(Debug, Deserialize)]
pub struct CleanupLogsRequest {
    pub retention_days: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct CleanupLogsResponse {
    pub ok: bool,
    pub deleted: usize,
    pub retention_days: u32,
}

pub async fn cleanup_request_logs(
    _auth: AdminUser,
    State(state): State<AppState>,
    payload: Option<Json<CleanupLogsRequest>>,
) -> Result<Json<CleanupLogsResponse>, AppError> {
    let days = payload
        .and_then(|Json(p)| p.retention_days)
        .unwrap_or(state.config.request_log_retention_days);
    let deleted = state.db.cleanup_old_request_logs(days)?;
    Ok(Json(CleanupLogsResponse {
        ok: true,
        deleted,
        retention_days: days,
    }))
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

fn extract_model_names_from_json(body: &serde_json::Value) -> Vec<String> {
    if let Some(data) = body.get("data").and_then(|d| d.as_array()) {
        data.iter()
            .filter_map(|m| {
                m.get("id")
                    .and_then(|id| id.as_str())
                    .map(|s| s.to_string())
            })
            .filter(|s| !s.trim().is_empty())
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
            .filter(|s| !s.trim().is_empty())
            .collect()
    } else {
        Vec::new()
    }
}

async fn fetch_upstream_models(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    is_openrouter: bool,
) -> Result<Vec<String>, AppError> {
    let trimmed_base = base_url.trim().trim_end_matches('/');
    let url = format!("{trimmed_base}/models");
    let mut req = client.get(&url);
    if !api_key.trim().is_empty() {
        req = req.bearer_auth(api_key);
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

    let mut model_names = extract_model_names_from_json(&body);

    if is_openrouter {
        let img_url = crate::provider::resolve_image_models_url(base_url);
        let mut img_req = client.get(&img_url);
        if !api_key.trim().is_empty() {
            img_req = img_req.bearer_auth(api_key);
        }
        if let Ok(img_res) = img_req.send().await {
            if img_res.status().is_success() {
                if let Ok(img_body) = img_res.json::<serde_json::Value>().await {
                    for m in extract_model_names_from_json(&img_body) {
                        if !model_names.contains(&m) {
                            model_names.push(m);
                        }
                    }
                }
            }
        }
    }

    Ok(model_names)
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

    let is_openrouter = ptype == "openrouter" || base_url.contains("openrouter.ai");

    if state.config.use_mock_provider {
        let mut models = vec![
            "gpt-4o".to_string(),
            "gpt-4o-mini".to_string(),
            "claude-3-5-sonnet".to_string(),
            "gemini-2.0-flash".to_string(),
            "deepseek-chat".to_string(),
        ];
        if is_openrouter {
            models.push("recraft/recraft-v4.1-flash".to_string());
            models.push("black-forest-labs/flux.2-klein-4b".to_string());
        }
        return Ok(Json(serde_json::json!({
            "status": "ok",
            "models": models,
        })));
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| AppError::Internal(format!("Failed to build HTTP client: {e}")))?;

    let model_names = fetch_upstream_models(&client, &base_url, &api_key, is_openrouter).await?;

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

    let is_openrouter = provider.provider_type == "openrouter"
        || provider.prefix == "openrouter"
        || provider.base_url.contains("openrouter.ai");

    let models_val: serde_json::Value = if state.config.use_mock_provider {
        let mut models = vec![
            "gpt-4o".to_string(),
            "gpt-4o-mini".to_string(),
            "claude-3-5-sonnet".to_string(),
            "gemini-2.0-flash".to_string(),
            "deepseek-chat".to_string(),
        ];
        if is_openrouter {
            models.push("recraft/recraft-v4.1-flash".to_string());
            models.push("black-forest-labs/flux.2-klein-4b".to_string());
        }
        serde_json::json!(models)
    } else {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|e| AppError::Internal(format!("Failed to build HTTP client: {e}")))?;

        let model_names = fetch_upstream_models(
            &client,
            &provider.base_url,
            &provider.api_key,
            is_openrouter,
        )
        .await?;

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
