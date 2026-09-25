use std::collections::HashMap;
use std::pin::Pin;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Instant;

use axum::{
    body::Body,
    extract::{rejection::JsonRejection, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use bytes::Bytes;
use futures_util::stream::Stream;
use tracing::{info, warn};

use crate::auth::AuthenticatedUser;
use crate::db::Database;
use crate::error::AppError;
use crate::provider::{BoxChatStream, ChatCompletionRequest, ChatMessage, Provider};
use crate::state::AppState;

const MAX_AFFINITY_ENTRIES: usize = 10_000;

static COMBO_RR_COUNTERS: LazyLock<Mutex<HashMap<String, usize>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

static COMBO_AFFINITY_MAP: LazyLock<Mutex<HashMap<String, String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn get_combo_affinity(affinity_key: &str) -> Option<String> {
    let map = COMBO_AFFINITY_MAP.lock().unwrap_or_else(|p| p.into_inner());
    map.get(affinity_key).cloned()
}

fn set_combo_affinity(affinity_key: &str, model: &str) {
    let mut map = COMBO_AFFINITY_MAP.lock().unwrap_or_else(|p| p.into_inner());
    if map.len() >= MAX_AFFINITY_ENTRIES && !map.contains_key(affinity_key) {
        if let Some(first_key) = map.keys().next().cloned() {
            map.remove(&first_key);
        }
    }
    map.insert(affinity_key.to_string(), model.to_string());
}

fn compute_affinity_key(api_key: &str, combo_name: &str, messages: &[ChatMessage]) -> String {
    let user_content = messages
        .iter()
        .find(|m| m.role.eq_ignore_ascii_case("user"))
        .and_then(|m| m.content.as_ref())
        .map(|c| c.text())
        .unwrap_or_default();
    let mut data = Vec::with_capacity(api_key.len() + combo_name.len() + user_content.len() + 2);
    data.extend_from_slice(api_key.as_bytes());
    data.push(0);
    data.extend_from_slice(combo_name.as_bytes());
    data.push(0);
    data.extend_from_slice(user_content.as_bytes());
    let digest = ring::digest::digest(&ring::digest::SHA256, &data);
    let mut hex = String::with_capacity(64);
    for b in digest.as_ref() {
        use std::fmt::Write;
        let _ = write!(hex, "{:02x}", b);
    }
    hex
}

fn resolve_candidate_models(
    state: &AppState,
    trimmed_model: &str,
    is_tool_continuation: bool,
    affinity_key: &str,
) -> Result<(Vec<String>, bool), AppError> {
    if let Ok(Some(combo)) = state.db.get_combo_by_name(trimmed_model) {
        let models: Vec<String> = if let Some(arr) = combo.models.as_array() {
            arr.iter()
                .filter_map(|v| {
                    v.as_str()
                        .map(|s| s.to_string())
                        .or_else(|| v.get("id").and_then(|i| i.as_str()).map(|s| s.to_string()))
                        .or_else(|| {
                            v.get("model")
                                .and_then(|m| m.as_str())
                                .map(|s| s.to_string())
                        })
                })
                .collect()
        } else {
            vec![]
        };
        if models.is_empty() {
            return Err(AppError::BadRequest(format!(
                "Combo '{trimmed_model}' không có model thành viên nào"
            )));
        }
        if combo.strategy == "round-robin" {
            if is_tool_continuation {
                if let Some(remembered) = get_combo_affinity(affinity_key) {
                    if let Some(pos) = models.iter().position(|m| m == &remembered) {
                        let mut rotated = models[pos..].to_vec();
                        rotated.extend_from_slice(&models[..pos]);
                        return Ok((rotated, true));
                    }
                }
                let mut map = COMBO_RR_COUNTERS.lock().unwrap_or_else(|p| p.into_inner());
                let count = *map.entry(combo.name.clone()).or_insert(0);
                let offset = count % models.len();
                let mut rotated = models[offset..].to_vec();
                rotated.extend_from_slice(&models[..offset]);
                Ok((rotated, true))
            } else {
                let mut map = COMBO_RR_COUNTERS.lock().unwrap_or_else(|p| p.into_inner());
                let count = map.entry(combo.name.clone()).or_insert(0);
                let offset = *count % models.len();
                *count = count.wrapping_add(1);
                let mut rotated = models[offset..].to_vec();
                rotated.extend_from_slice(&models[..offset]);
                if let Some(first_cand) = rotated.first() {
                    set_combo_affinity(affinity_key, first_cand);
                }
                Ok((rotated, true))
            }
        } else {
            // "fallback": giữ nguyên thứ tự ưu tiên mà người dùng đã sắp xếp
            Ok((models, true))
        }
    } else {
        Ok((vec![trimmed_model.to_string()], false))
    }
}

fn resolve_model_provider(
    state: &AppState,
    auth_key_id: &str,
    target_model: &str,
) -> Result<(Arc<dyn Provider>, String, String, String), AppError> {
    let prefix = if target_model.contains('/') {
        target_model.split('/').next().unwrap_or("")
    } else {
        ""
    };
    let external_prov = if !prefix.is_empty() {
        state
            .db
            .get_provider_by_prefix(prefix)
            .ok()
            .flatten()
            .filter(|p| p.is_active)
    } else {
        None
    };

    if external_prov.is_none() && state.models.get_model(target_model).is_none() {
        return Err(AppError::NotFound(format!(
            "Model '{target_model}' not found"
        )));
    }

    let is_codex_model = state
        .models
        .get_model(target_model)
        .map(|m| m.owned_by == "openai-codex")
        .unwrap_or(false)
        || target_model.starts_with("cx/");

    let (chosen_provider, account_id_for_log, provider_display_name, masked_account_label) =
        if let Some(ref prov) = external_prov {
            let p_name = prov.name.clone();
            let acc = format!("Prefix: {}", prov.prefix);
            if state.config.use_mock_provider {
                (state.provider.clone(), auth_key_id.to_string(), p_name, acc)
            } else {
                let upstream_provider = Arc::new(crate::provider::HttpUpstreamProvider::new(
                    Some(prov.base_url.clone()),
                    Some(prov.api_key.clone()),
                    state.config.upstream_connect_timeout_secs,
                    state.config.upstream_read_timeout_secs,
                    state.config.upstream_request_timeout_secs,
                ));
                (
                    upstream_provider as Arc<dyn Provider>,
                    format!("provider:{}", prov.prefix),
                    p_name,
                    acc,
                )
            }
        } else if is_codex_model {
            let acc_candidate = state
                .codex_pool
                .pick_account()
                .map(|a| a.email.read().unwrap().clone())
                .unwrap_or_else(|| "Codex Pool".to_string());
            let masked = crate::live_monitor::mask_account_email(&acc_candidate);
            (
                state.codex_provider.clone(),
                format!("codex:{}", auth_key_id),
                "Codex".to_string(),
                masked,
            )
        } else {
            let acc_candidate = state
                .account_pool
                .peek_account(Some(target_model))
                .map(|a| a.email)
                .unwrap_or_else(|| "Google Pool".to_string());
            let masked = crate::live_monitor::mask_account_email(&acc_candidate);
            (
                state.provider.clone(),
                auth_key_id.to_string(),
                "Google Antigravity".to_string(),
                masked,
            )
        };

    Ok((
        chosen_provider,
        account_id_for_log,
        provider_display_name,
        masked_account_label,
    ))
}

fn extract_usage_from_bytes(bytes: &[u8]) -> Option<(i64, i64)> {
    if !bytes.windows(7).any(|w| w == b"\"usage\"") {
        return None;
    }
    let text = std::str::from_utf8(bytes).ok()?;
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("data:") {
            let data_str = rest.trim();
            if data_str == "[DONE]" || data_str.is_empty() {
                continue;
            }
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(data_str) {
                if let Some(usage) = v.get("usage") {
                    let pt = usage
                        .get("prompt_tokens")
                        .and_then(|t| t.as_i64())
                        .unwrap_or(0);
                    let ct = usage
                        .get("completion_tokens")
                        .and_then(|t| t.as_i64())
                        .unwrap_or(0);
                    if pt > 0 || ct > 0 {
                        return Some((pt, ct));
                    }
                }
            }
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn spawn_log_completion(
    db: Arc<Database>,
    account_id: String,
    model: String,
    timestamp: f64,
    status: &'static str,
    prompt_tokens: i64,
    completion_tokens: i64,
    duration_ms: f64,
    error: Option<String>,
) {
    let action = move || {
        let _ = db.record_request(
            Some(&account_id),
            &model,
            timestamp,
            status,
            prompt_tokens,
            completion_tokens,
            duration_ms,
            error.as_deref(),
        );
    };

    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        handle.spawn_blocking(action);
    } else {
        std::thread::spawn(action);
    }
}

pub struct ModelRewriteChatStream {
    inner: BoxChatStream,
    requested_model: String,
    buffer: Vec<u8>,
}

impl ModelRewriteChatStream {
    pub fn new(inner: BoxChatStream, requested_model: String) -> Self {
        Self {
            inner,
            requested_model,
            buffer: Vec::new(),
        }
    }
}

impl Stream for ModelRewriteChatStream {
    type Item = Result<Bytes, AppError>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let this = self.as_mut().get_mut();
        loop {
            match Pin::new(&mut this.inner).poll_next(cx) {
                std::task::Poll::Ready(Some(Ok(bytes))) => {
                    this.buffer.extend_from_slice(&bytes);
                    if let Some(last_nl_idx) = this.buffer.iter().rposition(|&b| b == b'\n') {
                        let remainder = this.buffer.split_off(last_nl_idx + 1);
                        let complete = std::mem::replace(&mut this.buffer, remainder);
                        let rewritten =
                            rewrite_sse_chunk(&Bytes::from(complete), &this.requested_model);
                        return std::task::Poll::Ready(Some(Ok(rewritten)));
                    }
                }
                std::task::Poll::Ready(Some(Err(err))) => {
                    return std::task::Poll::Ready(Some(Err(err)))
                }
                std::task::Poll::Ready(None) => {
                    if !this.buffer.is_empty() {
                        let remaining = std::mem::take(&mut this.buffer);
                        let rewritten =
                            rewrite_sse_chunk(&Bytes::from(remaining), &this.requested_model);
                        return std::task::Poll::Ready(Some(Ok(rewritten)));
                    }
                    return std::task::Poll::Ready(None);
                }
                std::task::Poll::Pending => return std::task::Poll::Pending,
            }
        }
    }
}

pub fn rewrite_sse_chunk(bytes: &Bytes, requested_model: &str) -> Bytes {
    if !bytes.windows(5).any(|w| w == b"data:") {
        return bytes.clone();
    }
    let Ok(text) = std::str::from_utf8(bytes) else {
        return bytes.clone();
    };

    let mut out = String::with_capacity(text.len());
    let mut modified = false;

    let mut remainder = text;
    while !remainder.is_empty() {
        let (line, ending, next_remainder) = match remainder.find('\n') {
            Some(idx) => {
                if idx > 0 && remainder.as_bytes()[idx - 1] == b'\r' {
                    (&remainder[..idx - 1], "\r\n", &remainder[idx + 1..])
                } else {
                    (&remainder[..idx], "\n", &remainder[idx + 1..])
                }
            }
            None => (remainder, "", ""),
        };
        remainder = next_remainder;

        if let Some(rest) = line.strip_prefix("data:") {
            let data_str = rest.trim();
            if data_str == "[DONE]" || data_str.is_empty() {
                out.push_str(line);
                out.push_str(ending);
                continue;
            }

            if let Ok(mut val) = serde_json::from_str::<serde_json::Value>(data_str) {
                if let Some(obj) = val.as_object_mut() {
                    if let Some(m) = obj.get("model") {
                        if m.as_str() == Some(requested_model) {
                            out.push_str(line);
                            out.push_str(ending);
                            continue;
                        }
                        obj.insert(
                            "model".to_string(),
                            serde_json::Value::String(requested_model.to_string()),
                        );
                        if let Ok(serialized) = serde_json::to_string(&val) {
                            out.push_str("data: ");
                            out.push_str(&serialized);
                            out.push_str(ending);
                            modified = true;
                            continue;
                        }
                    }
                }
            }
        }

        out.push_str(line);
        out.push_str(ending);
    }

    if modified {
        Bytes::from(out)
    } else {
        bytes.clone()
    }
}

pub struct LoggingChatStream {
    inner: BoxChatStream,
    logged: bool,
    start_time: Instant,
    timestamp: f64,
    account_id: String,
    model: String,
    db: Arc<Database>,
    prompt_tokens: i64,
    completion_tokens: i64,
    live_registry: Arc<crate::live_monitor::ActiveRequestRegistry>,
    request_id: String,
}

impl Stream for LoggingChatStream {
    type Item = Result<Bytes, AppError>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let this = self.as_mut().get_mut();
        match Pin::new(&mut this.inner).poll_next(cx) {
            std::task::Poll::Ready(Some(Ok(bytes))) => {
                if let Some((pt, ct)) = extract_usage_from_bytes(&bytes) {
                    this.prompt_tokens = this.prompt_tokens.max(pt);
                    this.completion_tokens = this.completion_tokens.max(ct);
                }
                std::task::Poll::Ready(Some(Ok(bytes)))
            }
            std::task::Poll::Ready(Some(Err(err))) => {
                if !this.logged {
                    this.logged = true;
                    this.live_registry.finish(&this.request_id, "error");
                    let duration_ms = this.start_time.elapsed().as_secs_f64() * 1000.0;
                    spawn_log_completion(
                        this.db.clone(),
                        this.account_id.clone(),
                        this.model.clone(),
                        this.timestamp,
                        "error",
                        this.prompt_tokens,
                        this.completion_tokens,
                        duration_ms,
                        Some(err.to_string()),
                    );
                }
                std::task::Poll::Ready(Some(Err(err)))
            }
            std::task::Poll::Ready(None) => {
                if !this.logged {
                    this.logged = true;
                    this.live_registry.finish(&this.request_id, "success");
                    let duration_ms = this.start_time.elapsed().as_secs_f64() * 1000.0;
                    spawn_log_completion(
                        this.db.clone(),
                        this.account_id.clone(),
                        this.model.clone(),
                        this.timestamp,
                        "success",
                        this.prompt_tokens,
                        this.completion_tokens,
                        duration_ms,
                        None,
                    );
                }
                std::task::Poll::Ready(None)
            }
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}

impl Drop for LoggingChatStream {
    fn drop(&mut self) {
        if !self.logged {
            self.logged = true;
            self.live_registry.finish(&self.request_id, "cancelled");
            let duration_ms = self.start_time.elapsed().as_secs_f64() * 1000.0;
            let account_id = std::mem::take(&mut self.account_id);
            let model = std::mem::take(&mut self.model);
            spawn_log_completion(
                self.db.clone(),
                account_id,
                model,
                self.timestamp,
                "cancelled",
                self.prompt_tokens,
                self.completion_tokens,
                duration_ms,
                Some("client disconnected".to_string()),
            );
        }
    }
}

struct CancellationGuard {
    db: Arc<Database>,
    account_id: String,
    model: String,
    start_time: Instant,
    timestamp: f64,
    completed: bool,
    live_registry: Arc<crate::live_monitor::ActiveRequestRegistry>,
    request_id: String,
}

impl Drop for CancellationGuard {
    fn drop(&mut self) {
        if !self.completed {
            self.live_registry.finish(&self.request_id, "cancelled");
            let duration_ms = self.start_time.elapsed().as_secs_f64() * 1000.0;
            let account_id = std::mem::take(&mut self.account_id);
            let model = std::mem::take(&mut self.model);
            spawn_log_completion(
                self.db.clone(),
                account_id,
                model,
                self.timestamp,
                "cancelled",
                0,
                0,
                duration_ms,
                Some("client disconnected".to_string()),
            );
        }
    }
}

pub async fn chat_completions(
    auth: AuthenticatedUser,
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    payload_result: Result<Json<ChatCompletionRequest>, JsonRejection>,
) -> Result<Response, AppError> {
    let Json(mut payload) = payload_result.map_err(|e| AppError::BadRequest(e.to_string()))?;

    let request_id = uuid::Uuid::new_v4().to_string();
    let client_label = if !auth.name.trim().is_empty() {
        auth.name.clone()
    } else {
        crate::db::mask_api_key(&auth.key)
    };
    let is_stream = payload.stream.unwrap_or(false);
    state.live_registry.register(
        request_id.clone(),
        client_label,
        payload.model.trim().to_string(),
        is_stream,
    );

    // 1. Reject empty messages
    if payload.messages.is_empty() {
        state.live_registry.finish(&request_id, "error");
        return Err(AppError::BadRequest(
            "messages array cannot be empty".to_string(),
        ));
    }

    // 2. Reject unknown model
    let trimmed_model = payload.model.trim();
    if trimmed_model.is_empty() {
        state.live_registry.finish(&request_id, "error");
        return Err(AppError::BadRequest("model cannot be empty".to_string()));
    }

    let affinity_key = compute_affinity_key(&auth.key, trimmed_model, &payload.messages);

    // 3. Apply Token Saver (RTK compression & Caveman/Ponytail prompts)
    let is_tool_continuation = payload.messages.iter().any(|m| {
        m.role.eq_ignore_ascii_case("tool")
            || m.role.eq_ignore_ascii_case("function")
            || m.tool_call_id.is_some()
    });
    let is_tool_request = payload.tools.as_ref().is_some_and(|t| {
        !t.is_null() && (!t.is_array() || t.as_array().is_some_and(|items| !items.is_empty()))
    }) || payload
        .tool_choice
        .as_ref()
        .is_some_and(|tc| !tc.is_null() && tc.as_str() != Some("none"))
        || is_tool_continuation
        || payload.messages.iter().any(|m| m.tool_calls.is_some());

    let is_structured_output = payload.response_format.as_ref().is_some_and(|rf| {
        !rf.is_null() && (!rf.is_object() || !rf.as_object().unwrap().is_empty())
    });

    let initial_settings = state.db.get_token_saver_settings()?;
    let _token_saver_stats = crate::token_saver::apply_token_saver_with_request_context(
        &headers,
        &mut payload.messages,
        &initial_settings,
        is_tool_request,
        is_structured_output,
    )?;

    let (candidate_models, is_combo) = match resolve_candidate_models(
        &state,
        trimmed_model,
        is_tool_continuation,
        &affinity_key,
    ) {
        Ok(c) => c,
        Err(e) => {
            state.live_registry.finish(&request_id, "error");
            return Err(e);
        }
    };

    let start_time = Instant::now();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);

    let mut cancel_guard = CancellationGuard {
        db: state.db.clone(),
        account_id: String::new(),
        model: trimmed_model.to_string(),
        start_time,
        timestamp,
        completed: false,
        live_registry: state.live_registry.clone(),
        request_id: request_id.clone(),
    };

    let mut last_error: Option<AppError> = None;
    let mut resolved_stream = None;
    let mut chosen_account_for_stream = String::new();

    for (idx, cand_model) in candidate_models.iter().enumerate() {
        let (chosen_provider, account_id_for_log, provider_display_name, masked_account_label) =
            match resolve_model_provider(&state, &auth.key_id, cand_model) {
                Ok(res) => res,
                Err(err) => {
                    warn!(
                        combo = %trimmed_model,
                        candidate = %cand_model,
                        "Failed to resolve provider for candidate: {err}"
                    );
                    last_error = Some(err);
                    continue;
                }
            };

        cancel_guard.account_id = account_id_for_log.clone();

        state.live_registry.update_routing(
            &request_id,
            &provider_display_name,
            cand_model,
            &masked_account_label,
        );

        let mut req_payload = payload.clone();
        req_payload.model = cand_model.clone();

        if state.config.codex_context_optimizer_enabled && cand_model.starts_with("cx/") {
            let opt_config = crate::context_optimizer::ContextOptimizerConfig {
                enabled: true,
                max_messages: state.config.codex_context_max_messages,
                max_bytes: state.config.codex_context_max_bytes,
                ..Default::default()
            };
            let (new_messages, metrics) =
                crate::context_optimizer::compact_messages(req_payload.messages, &opt_config);
            info!(
                model = %cand_model,
                total_messages = metrics.total_messages,
                total_bytes = metrics.total_bytes,
                compacted_messages = metrics.estimated_compacted,
                "Applied codex context optimizer"
            );
            req_payload.messages = new_messages;
        }

        if payload.stream == Some(true) {
            info!(
                model = %trimmed_model,
                candidate = %cand_model,
                attempt = idx + 1,
                messages_count = req_payload.messages.len(),
                has_tools = req_payload.tools.is_some(),
                "Processing streaming chat completion request"
            );
            match chosen_provider.stream_chat_completion(&req_payload).await {
                Ok(s) => {
                    resolved_stream = Some(s);
                    chosen_account_for_stream = account_id_for_log;
                    if is_combo {
                        set_combo_affinity(&affinity_key, cand_model);
                    }
                    break;
                }
                Err(err) => {
                    warn!(
                        combo = %trimmed_model,
                        candidate = %cand_model,
                        attempt = idx + 1,
                        "Candidate model stream failed: {err}"
                    );
                    last_error = Some(err);
                }
            }
        } else {
            info!(
                model = %trimmed_model,
                candidate = %cand_model,
                attempt = idx + 1,
                messages_count = req_payload.messages.len(),
                has_tools = req_payload.tools.is_some(),
                "Processing non-streaming chat completion request"
            );
            match chosen_provider.complete(&req_payload).await {
                Ok(mut response) => {
                    cancel_guard.completed = true;
                    if is_combo {
                        set_combo_affinity(&affinity_key, cand_model);
                    }
                    state.live_registry.finish(&request_id, "success");
                    let _ = state.db.increment_total_requests(&auth.key);
                    let prompt_tokens = response.usage.prompt_tokens as i64;
                    let completion_tokens = response.usage.completion_tokens as i64;
                    let duration_ms = start_time.elapsed().as_secs_f64() * 1000.0;
                    let _ = state.db.record_request(
                        Some(&account_id_for_log),
                        trimmed_model,
                        timestamp,
                        "success",
                        prompt_tokens,
                        completion_tokens,
                        duration_ms,
                        None,
                    );
                    response.model = trimmed_model.to_string();
                    return Ok(Json(response).into_response());
                }
                Err(err) => {
                    warn!(
                        combo = %trimmed_model,
                        candidate = %cand_model,
                        attempt = idx + 1,
                        "Candidate model complete failed: {err}"
                    );
                    last_error = Some(err);
                }
            }
        }
    }

    if payload.stream == Some(true) {
        if let Some(stream) = resolved_stream {
            cancel_guard.completed = true;
            let _ = state.db.increment_total_requests(&auth.key);
            let rewrite_stream = ModelRewriteChatStream::new(stream, trimmed_model.to_string());
            let logging_stream = LoggingChatStream {
                inner: Box::pin(rewrite_stream),
                logged: false,
                start_time,
                timestamp,
                account_id: chosen_account_for_stream,
                model: trimmed_model.to_string(),
                db: state.db.clone(),
                prompt_tokens: 0,
                completion_tokens: 0,
                live_registry: state.live_registry.clone(),
                request_id: request_id.clone(),
            };
            let body = Body::from_stream(logging_stream);
            let response = Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/event-stream")
                .header(header::CACHE_CONTROL, "no-cache")
                .header(header::CONNECTION, "keep-alive")
                .body(body)
                .map_err(|e| AppError::Internal(e.to_string()))?;
            return Ok(response);
        }
    }

    state.live_registry.finish(&request_id, "error");
    cancel_guard.completed = true;
    let duration_ms = start_time.elapsed().as_secs_f64() * 1000.0;
    let final_err = last_error
        .unwrap_or_else(|| AppError::NotFound(format!("Model '{trimmed_model}' not found")));
    let _ = state.db.record_request(
        None,
        trimmed_model,
        timestamp,
        "error",
        0,
        0,
        duration_ms,
        Some(&final_err.to_string()),
    );
    Err(final_err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::ChatMessage;
    use futures_util::StreamExt;

    #[test]
    fn test_affinity_fingerprint_determinism() {
        let msgs1 = vec![
            ChatMessage::user("What is the capital of France?"),
            ChatMessage {
                role: "assistant".to_string(),
                content: None,
                name: None,
                tool_call_id: None,
                tool_calls: Some(serde_json::json!([{"id": "call_1", "type": "function"}])),
            },
            ChatMessage {
                role: "tool".to_string(),
                content: Some(crate::provider::MessageContent::Text("Paris".to_string())),
                name: None,
                tool_call_id: Some("call_1".to_string()),
                tool_calls: None,
            },
        ];

        let msgs2 = vec![
            ChatMessage::user("What is the capital of France?"),
            ChatMessage {
                role: "assistant".to_string(),
                content: None,
                name: None,
                tool_call_id: None,
                tool_calls: Some(serde_json::json!([{"id": "call_1", "type": "function"}])),
            },
        ];

        let k1 = compute_affinity_key("sk-test-key", "combo-coding", &msgs1);
        let k2 = compute_affinity_key("sk-test-key", "combo-coding", &msgs2);
        assert_eq!(k1, k2, "Fingerprints for same conversation should match");

        let k_diff_key = compute_affinity_key("sk-different-key", "combo-coding", &msgs1);
        assert_ne!(
            k1, k_diff_key,
            "Different API key must yield different fingerprint"
        );

        let k_diff_combo = compute_affinity_key("sk-test-key", "combo-other", &msgs1);
        assert_ne!(
            k1, k_diff_combo,
            "Different combo must yield different fingerprint"
        );

        let msgs_diff_content = vec![ChatMessage::user("Tell me a joke")];
        let k_diff_content =
            compute_affinity_key("sk-test-key", "combo-coding", &msgs_diff_content);
        assert_ne!(
            k1, k_diff_content,
            "Different earliest user content must yield different fingerprint"
        );
    }

    #[test]
    fn test_affinity_map_get_set_and_bounded() {
        let test_key = "test-aff-key-1";
        set_combo_affinity(test_key, "model-a");
        assert_eq!(get_combo_affinity(test_key), Some("model-a".to_string()));

        set_combo_affinity(test_key, "model-b");
        assert_eq!(get_combo_affinity(test_key), Some("model-b".to_string()));
    }

    #[test]
    fn test_is_tool_continuation_legacy_function() {
        let msg_fn = ChatMessage {
            role: "function".to_string(),
            content: Some(crate::provider::MessageContent::Text("result".to_string())),
            name: Some("fn1".to_string()),
            tool_call_id: None,
            tool_calls: None,
        };
        let is_tc = [msg_fn].iter().any(|m| {
            m.role.eq_ignore_ascii_case("tool")
                || m.role.eq_ignore_ascii_case("function")
                || m.tool_call_id.is_some()
        });
        assert!(
            is_tc,
            "role=function must be recognized as tool continuation"
        );

        let msg_tool = ChatMessage {
            role: "tool".to_string(),
            content: Some(crate::provider::MessageContent::Text("result".to_string())),
            name: None,
            tool_call_id: Some("id1".to_string()),
            tool_calls: None,
        };
        let is_tc_tool = [msg_tool].iter().any(|m| {
            m.role.eq_ignore_ascii_case("tool")
                || m.role.eq_ignore_ascii_case("function")
                || m.tool_call_id.is_some()
        });
        assert!(
            is_tc_tool,
            "role=tool must be recognized as tool continuation"
        );

        let msg_user = ChatMessage::user("Hello");
        let is_tc_user = [msg_user].iter().any(|m| {
            m.role.eq_ignore_ascii_case("tool")
                || m.role.eq_ignore_ascii_case("function")
                || m.tool_call_id.is_some()
        });
        assert!(!is_tc_user, "user message must not be tool continuation");
    }

    #[test]
    fn test_rewrite_sse_chunk_model() {
        let raw = Bytes::from("data: {\"id\":\"chatcmpl-1\",\"object\":\"chat.completion.chunk\",\"model\":\"cx/gpt-5-codex\",\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n\n");
        let rewritten = rewrite_sse_chunk(&raw, "my-super-combo");
        let s = std::str::from_utf8(&rewritten).unwrap();
        assert!(s.starts_with("data: "));
        assert!(s.ends_with("\n\n"));
        assert!(s.contains("\"model\":\"my-super-combo\""));
        assert!(!s.contains("cx/gpt-5-codex"));
        assert!(s.contains("\"content\":\"Hello\""));
    }

    #[test]
    fn test_rewrite_sse_chunk_preserves_crlf_and_done() {
        let raw_done = Bytes::from("data: [DONE]\r\n\r\n");
        let out_done = rewrite_sse_chunk(&raw_done, "my-combo");
        assert_eq!(raw_done, out_done);

        let raw_crlf = Bytes::from("data: {\"model\":\"old\"}\r\n\r\n");
        let out_crlf = rewrite_sse_chunk(&raw_crlf, "new");
        assert_eq!(
            std::str::from_utf8(&out_crlf).unwrap(),
            "data: {\"model\":\"new\"}\r\n\r\n"
        );

        let raw_non_json = Bytes::from("data: not-json\n\n");
        let out_non_json = rewrite_sse_chunk(&raw_non_json, "new");
        assert_eq!(raw_non_json, out_non_json);

        let raw_non_sse = Bytes::from(": keep-alive ping\n\n");
        let out_non_sse = rewrite_sse_chunk(&raw_non_sse, "new");
        assert_eq!(raw_non_sse, out_non_sse);

        // Already matching model: zero-copy pass through
        let raw_matching = Bytes::from("data: {\"model\":\"same\"}\n\n");
        let out_matching = rewrite_sse_chunk(&raw_matching, "same");
        assert_eq!(raw_matching, out_matching);
    }

    #[tokio::test]
    async fn test_model_rewrite_chat_stream() {
        use futures_util::stream;
        let c1 = Bytes::from("data: {\"model\":\"member-1\",\"choices\":[]}\n\n");
        let c2 = Bytes::from("data: [DONE]\n\n");
        let mock_stream: BoxChatStream = Box::pin(stream::iter(vec![Ok(c1), Ok(c2)]));

        let rewrite_stream = ModelRewriteChatStream::new(mock_stream, "combo-alpha".to_string());
        let mut pinned = Box::pin(rewrite_stream);

        let first = pinned.next().await.unwrap().unwrap();
        let s1 = std::str::from_utf8(&first).unwrap();
        assert!(s1.contains("\"model\":\"combo-alpha\""));
        assert!(!s1.contains("member-1"));

        let second = pinned.next().await.unwrap().unwrap();
        assert_eq!(second, Bytes::from("data: [DONE]\n\n"));

        let third = pinned.next().await;
        assert!(third.is_none());
    }

    #[tokio::test]
    async fn test_model_rewrite_chat_stream_split_across_chunks() {
        use futures_util::stream;
        // Split a single JSON line across two chunks
        let c1 = Bytes::from("data: {\"model\":\"member-1\",");
        let c2 = Bytes::from("\"choices\":[{\"delta\":{\"content\":\"hello\"}}]}\n\n");
        let c3 = Bytes::from("data: [DONE]\n\n");
        let mock_stream: BoxChatStream = Box::pin(stream::iter(vec![Ok(c1), Ok(c2), Ok(c3)]));

        let rewrite_stream = ModelRewriteChatStream::new(mock_stream, "combo-alpha".to_string());
        let mut pinned = Box::pin(rewrite_stream);

        let first = pinned.next().await.unwrap().unwrap();
        let s1 = std::str::from_utf8(&first).unwrap();
        assert!(s1.contains("\"model\":\"combo-alpha\""));
        assert!(!s1.contains("member-1"));
        assert!(s1.contains("\"content\":\"hello\""));

        let second = pinned.next().await.unwrap().unwrap();
        assert_eq!(second, Bytes::from("data: [DONE]\n\n"));

        let third = pinned.next().await;
        assert!(third.is_none());
    }

    #[tokio::test]
    async fn test_model_rewrite_chat_stream_split_crlf_and_flush_remainder() {
        use futures_util::stream;
        // Chunk 1 has a complete line, then an incomplete line ending in \r
        // Chunk 2 completes the line with \r\n
        // Chunk 3 is incomplete remainder flushed at stream end without trailing newline
        let c1 = Bytes::from(": keep-alive\r\n\r\ndata: {\"model\":\"member-1\"}\r");
        let c2 = Bytes::from("\n\r\n");
        let c3 = Bytes::from("data: {\"model\":\"member-1\"}");
        let mock_stream: BoxChatStream = Box::pin(stream::iter(vec![Ok(c1), Ok(c2), Ok(c3)]));

        let rewrite_stream = ModelRewriteChatStream::new(mock_stream, "combo-alpha".to_string());
        let mut pinned = Box::pin(rewrite_stream);

        // First item should be the keep-alive
        let first = pinned.next().await.unwrap().unwrap();
        assert_eq!(first, Bytes::from(": keep-alive\r\n\r\n"));

        // Second item should be the rewritten JSON line with CRLF
        let second = pinned.next().await.unwrap().unwrap();
        let s2 = std::str::from_utf8(&second).unwrap();
        assert!(s2.contains("\"model\":\"combo-alpha\""));
        assert!(s2.ends_with("\r\n\r\n"));

        // Third item: remainder flushed on inner stream end
        let third = pinned.next().await.unwrap().unwrap();
        let s3 = std::str::from_utf8(&third).unwrap();
        assert!(s3.contains("\"model\":\"combo-alpha\""));

        let fourth = pinned.next().await;
        assert!(fourth.is_none());
    }
}
