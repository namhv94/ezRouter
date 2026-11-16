use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Instant;

use axum::{
    body::Body,
    extract::{rejection::JsonRejection, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use bytes::Bytes;
use futures_util::stream::Stream;
use futures_util::StreamExt;
use tracing::warn;

use crate::auth::AuthenticatedUser;
use crate::codex::CodexLease;
use crate::error::AppError;
use crate::provider::ChatCompletionRequest;
use crate::state::AppState;

use super::responses_compat::{
    chat_json_to_responses, responses_payload_to_chat, ChatSseToResponsesStream,
};

pub fn extract_responses_usage(val: &serde_json::Value) -> Option<(Option<i64>, Option<i64>)> {
    let usage = val
        .get("usage")
        .or_else(|| val.get("response").and_then(|r| r.get("usage")))?;

    let pt = usage
        .get("input_tokens")
        .or_else(|| usage.get("prompt_tokens"))
        .and_then(|t| t.as_i64());

    let ct = usage
        .get("output_tokens")
        .or_else(|| usage.get("completion_tokens"))
        .and_then(|t| t.as_i64());

    if pt.is_some() || ct.is_some() {
        Some((pt, ct))
    } else {
        None
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_log_responses_request(
    db: Arc<crate::db::Database>,
    account_email: String,
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
            Some(&account_email),
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

pub struct RawResponsesStream {
    inner: Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>>,
    lease: Option<CodexLease>,
    request_id: String,
    live_registry: Arc<crate::live_monitor::ActiveRequestRegistry>,
    db: Arc<crate::db::Database>,
    account_email: String,
    model: String,
    start_time: Instant,
    timestamp: f64,
    completed: bool,
    first_token_recorded: bool,
    prompt_tokens: i64,
    completion_tokens: i64,
}

impl RawResponsesStream {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        inner: impl Stream<Item = Result<Bytes, reqwest::Error>> + Send + 'static,
        lease: CodexLease,
        request_id: String,
        live_registry: Arc<crate::live_monitor::ActiveRequestRegistry>,
        db: Arc<crate::db::Database>,
        account_email: String,
        model: String,
        start_time: Instant,
        timestamp: f64,
    ) -> Self {
        Self {
            inner: Box::pin(inner),
            lease: Some(lease),
            request_id,
            live_registry,
            db,
            account_email,
            model,
            start_time,
            timestamp,
            completed: false,
            first_token_recorded: false,
            prompt_tokens: 0,
            completion_tokens: 0,
        }
    }
}

impl Stream for RawResponsesStream {
    type Item = Result<Bytes, AppError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.as_mut().get_mut();

        if this.completed {
            return Poll::Ready(None);
        }

        match Pin::new(&mut this.inner).poll_next(cx) {
            Poll::Ready(Some(Ok(bytes))) => {
                if !this.first_token_recorded {
                    this.first_token_recorded = true;
                    let ttft_ms = this.start_time.elapsed().as_secs_f64() * 1000.0;
                    this.live_registry
                        .record_first_token(&this.request_id, ttft_ms);
                }

                // Check for usage in Server-Sent Events chunk
                if let Ok(text) = std::str::from_utf8(&bytes) {
                    for line in text.lines() {
                        let trimmed = line.trim();
                        if let Some(rest) = trimmed.strip_prefix("data:") {
                            let data_str = rest.trim();
                            if data_str.is_empty() || data_str == "[DONE]" {
                                continue;
                            }
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(data_str) {
                                if let Some((pt_opt, ct_opt)) = extract_responses_usage(&v) {
                                    if let Some(pt) = pt_opt {
                                        this.prompt_tokens = pt;
                                    }
                                    if let Some(ct) = ct_opt {
                                        this.completion_tokens = ct;
                                    }
                                }
                            }
                        }
                    }
                }

                Poll::Ready(Some(Ok(bytes)))
            }
            Poll::Ready(Some(Err(err))) => {
                warn!(
                    model = %this.model,
                    account = %this.account_email,
                    "Responses upstream stream error: {err}"
                );
                this.completed = true;
                if let Some(mut lease) = this.lease.take() {
                    lease.commit_error(&err.to_string(), false);
                }
                this.live_registry.finish(&this.request_id, "error");
                let duration_ms = this.start_time.elapsed().as_secs_f64() * 1000.0;
                spawn_log_responses_request(
                    this.db.clone(),
                    this.account_email.clone(),
                    this.model.clone(),
                    this.timestamp,
                    "error",
                    this.prompt_tokens,
                    this.completion_tokens,
                    duration_ms,
                    Some(err.to_string()),
                );
                Poll::Ready(Some(Err(AppError::BadGateway(err.to_string()))))
            }
            Poll::Ready(None) => {
                this.completed = true;
                if let Some(mut lease) = this.lease.take() {
                    lease.commit_success();
                }
                this.live_registry.finish(&this.request_id, "success");
                let duration_ms = this.start_time.elapsed().as_secs_f64() * 1000.0;
                spawn_log_responses_request(
                    this.db.clone(),
                    this.account_email.clone(),
                    this.model.clone(),
                    this.timestamp,
                    "success",
                    this.prompt_tokens,
                    this.completion_tokens,
                    duration_ms,
                    None,
                );
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl Drop for RawResponsesStream {
    fn drop(&mut self) {
        if !self.completed {
            self.completed = true;
            if let Some(mut lease) = this_lease(&mut self.lease) {
                lease.release();
            }
            self.live_registry.finish(&self.request_id, "cancelled");
            let duration_ms = self.start_time.elapsed().as_secs_f64() * 1000.0;
            spawn_log_responses_request(
                self.db.clone(),
                self.account_email.clone(),
                self.model.clone(),
                self.timestamp,
                "cancelled",
                this_token(self.prompt_tokens),
                this_token(self.completion_tokens),
                duration_ms,
                Some("client disconnected".to_string()),
            );
        }
    }
}

fn this_token(tok: i64) -> i64 {
    tok
}

fn this_lease(opt: &mut Option<CodexLease>) -> Option<CodexLease> {
    opt.take()
}

pub async fn get_responses() -> Response {
    (
        StatusCode::BAD_REQUEST,
        [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
        serde_json::json!({
            "error": {
                "message": "OpenAI Responses WebSocket is not supported. Please use HTTPS POST /v1/responses (configure supports_websockets = false in ~/.codex/config.toml).",
                "type": "invalid_request_error",
                "param": null,
                "code": "websocket_not_supported"
            }
        })
        .to_string(),
    )
        .into_response()
}

pub async fn post_responses(
    auth: AuthenticatedUser,
    State(state): State<AppState>,
    payload_result: Result<Json<serde_json::Value>, JsonRejection>,
) -> Result<Response, AppError> {
    let Json(payload) = payload_result.map_err(|e| AppError::BadRequest(e.to_string()))?;

    let requested_model = payload
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("cx/gpt-5.6-sol");
    let trimmed_model = requested_model.trim();

    if trimmed_model.is_empty() {
        return Err(AppError::BadRequest("model cannot be empty".to_string()));
    }

    // ------------------------------------------------------------------
    // Route decision. Mirrors `chat::resolve_model_provider`:
    //   - a named combo -> chat translation path (e.g. `all`)
    //   - an active external provider prefix -> chat translation path
    //   - a genuine Codex model -> raw Codex passthrough (unchanged below)
    //   - anything else -> chat translation path (Antigravity pool /
    //     canonical models); unknown models surface the chat pipeline's
    //     "model not found" error.
    // ------------------------------------------------------------------
    let prefix = if trimmed_model.contains('/') {
        trimmed_model.split('/').next().unwrap_or("")
    } else {
        ""
    };
    let external_active = if prefix.is_empty() {
        None
    } else {
        state
            .db
            .get_provider_by_prefix(prefix)
            .ok()
            .flatten()
            .filter(|p| p.is_active)
    };
    let is_combo = state
        .db
        .get_combo_by_name(trimmed_model)
        .ok()
        .flatten()
        .is_some();
    let owned_by_codex = state
        .models
        .get_model(trimmed_model)
        .map(|m| m.owned_by == "openai-codex")
        .unwrap_or(false);
    let codex_route = !is_combo
        && external_active.is_none()
        && (crate::models::is_known_codex_model(trimmed_model) || owned_by_codex);

    if !codex_route {
        return post_responses_via_chat(auth, state, &payload, trimmed_model).await;
    }

    let request_id = uuid::Uuid::new_v4().to_string();
    let client_label = if !auth.name.trim().is_empty() {
        auth.name.clone()
    } else {
        crate::db::mask_api_key(&auth.key)
    };

    let is_stream = payload
        .get("stream")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    state.live_registry.register(
        request_id.clone(),
        client_label,
        trimmed_model.to_string(),
        is_stream,
    );

    let start_time = Instant::now();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);

    if state.config.use_mock_provider {
        state.live_registry.finish(&request_id, "success");
        let _ = state.db.increment_total_requests(&auth.key);
        let duration_ms = start_time.elapsed().as_secs_f64() * 1000.0;
        let _ = state.db.record_request(
            Some("mock@codex.internal"),
            trimmed_model,
            timestamp,
            "success",
            10,
            20,
            duration_ms,
            None,
        );

        if is_stream {
            let mock_sse = "event: response.done\ndata: {\"id\":\"resp_mock\",\"object\":\"response\",\"status\":\"completed\",\"usage\":{\"input_tokens\":10,\"output_tokens\":20}}\n\n";
            return Ok(Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/event-stream; charset=utf-8")
                .header(header::CACHE_CONTROL, "no-cache")
                .header(header::CONNECTION, "keep-alive")
                .body(Body::from(mock_sse))
                .unwrap());
        } else {
            return Ok(Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .body(Body::from(
                    serde_json::json!({
                        "id": "resp_mock",
                        "object": "response",
                        "status": "completed",
                        "model": trimmed_model,
                        "output": [],
                        "usage": {
                            "input_tokens": 10,
                            "output_tokens": 20
                        }
                    })
                    .to_string(),
                ))
                .unwrap());
        }
    }

    let masked_account_label = state
        .codex_pool
        .pick_account()
        .map(|a| crate::live_monitor::mask_account_email(&a.email.read().unwrap()))
        .unwrap_or_else(|| "Codex Pool".to_string());

    state
        .live_registry
        .update_routing(&request_id, "Codex", trimmed_model, &masked_account_label);

    let raw_res = match state
        .codex_raw_provider
        .execute_raw_responses(&payload, is_stream)
        .await
    {
        Ok(res) => res,
        Err(err) => {
            state.live_registry.finish(&request_id, "error");
            return Err(err);
        }
    };

    if is_stream {
        let _ = state.db.increment_total_requests(&auth.key);

        let stream = RawResponsesStream::new(
            raw_res.response.bytes_stream(),
            raw_res.lease,
            request_id,
            state.live_registry.clone(),
            state.db.clone(),
            raw_res.account_email,
            raw_res.upstream_model,
            start_time,
            timestamp,
        );

        Ok(Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/event-stream; charset=utf-8")
            .header(header::CACHE_CONTROL, "no-cache")
            .header(header::CONNECTION, "keep-alive")
            .body(Body::from_stream(stream))
            .unwrap())
    } else {
        let bytes_res = raw_res.response.bytes().await;
        let bytes = match bytes_res {
            Ok(b) => b,
            Err(err) => {
                let mut lease = raw_res.lease;
                lease.commit_error(&err.to_string(), false);
                state.live_registry.finish(&request_id, "error");
                let duration_ms = start_time.elapsed().as_secs_f64() * 1000.0;
                spawn_log_responses_request(
                    state.db.clone(),
                    raw_res.account_email,
                    raw_res.upstream_model,
                    timestamp,
                    "error",
                    0,
                    0,
                    duration_ms,
                    Some(err.to_string()),
                );
                return Err(AppError::BadGateway(err.to_string()));
            }
        };

        let mut lease = raw_res.lease;
        lease.commit_success();
        state.live_registry.finish(&request_id, "success");
        let _ = state.db.increment_total_requests(&auth.key);

        let (prompt_tokens, completion_tokens) =
            if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                if let Some((pt_opt, ct_opt)) = extract_responses_usage(&val) {
                    (pt_opt.unwrap_or(0), ct_opt.unwrap_or(0))
                } else {
                    (0, 0)
                }
            } else {
                (0, 0)
            };

        let duration_ms = start_time.elapsed().as_secs_f64() * 1000.0;
        spawn_log_responses_request(
            state.db.clone(),
            raw_res.account_email,
            raw_res.upstream_model,
            timestamp,
            "success",
            prompt_tokens,
            completion_tokens,
            duration_ms,
            None,
        );

        Ok(Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
            .body(Body::from(bytes))
            .unwrap())
    }
}

/// `/v1/responses` entry for chat-completion upstreams (combos such as `all`,
/// external providers, and the Antigravity pool): translate the Responses
/// request into a chat request, run the shared chat pipeline, then translate
/// the reply (JSON or SSE) back into Responses format.
async fn post_responses_via_chat(
    auth: AuthenticatedUser,
    state: AppState,
    payload: &serde_json::Value,
    requested_model: &str,
) -> Result<Response, AppError> {
    let chat_req: ChatCompletionRequest = responses_payload_to_chat(payload, requested_model)?;
    let headers = HeaderMap::new();
    let chat_res = super::chat::execute_chat_completions(auth, state, headers, chat_req).await?;

    let is_stream = chat_res
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|ct| ct.starts_with("text/event-stream"));

    if is_stream {
        let data_stream = chat_res.into_body().into_data_stream();
        let mapped =
            data_stream.map(|item| item.map_err(|e| AppError::Internal(e.to_string())));
        let translated =
            ChatSseToResponsesStream::new(Box::pin(mapped), requested_model.to_string());
        Ok(Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/event-stream; charset=utf-8")
            .header(header::CACHE_CONTROL, "no-cache")
            .header(header::CONNECTION, "keep-alive")
            .body(Body::from_stream(translated))
            .unwrap())
    } else {
        let bytes = axum::body::to_bytes(chat_res.into_body(), 64 * 1024 * 1024)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let chat_json = serde_json::from_slice::<serde_json::Value>(&bytes)
            .unwrap_or_else(|_| serde_json::json!({ "choices": [] }));
        let resp = chat_json_to_responses(&chat_json, requested_model);
        Ok(Json(resp).into_response())
    }
}
