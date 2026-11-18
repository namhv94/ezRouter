use axum::{
    body::Body,
    extract::{rejection::JsonRejection, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use futures_util::StreamExt;

use crate::auth::AuthenticatedUser;
use crate::error::AppError;
use crate::provider::ChatCompletionRequest;
use crate::state::AppState;

use super::responses_compat::{
    chat_json_to_responses, responses_payload_to_chat, ChatSseToResponsesStream,
};

pub async fn get_responses() -> Response {
    (
        StatusCode::BAD_REQUEST,
        [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
        serde_json::json!({
            "error": {
                "message": "OpenAI Responses WebSocket is not supported. Please use HTTPS POST /v1/responses.",
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
    state: State<AppState>,
    payload_result: Result<Json<serde_json::Value>, JsonRejection>,
) -> Result<Response, AppError> {
    let Json(payload) = payload_result.map_err(|e| AppError::BadRequest(e.to_string()))?;

    let requested_model = payload
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("gpt-4o");
    let trimmed_model = requested_model.trim();

    if trimmed_model.is_empty() {
        return Err(AppError::BadRequest("model cannot be empty".to_string()));
    }

    post_responses_via_chat(auth, state.0, &payload, trimmed_model).await
}

/// `/v1/responses` entry for chat-completion upstreams (combos, external
/// providers configured in the DB): translate the Responses request into a
/// chat request, run the shared chat pipeline, then translate the reply
/// (JSON or SSE) back into Responses format.
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