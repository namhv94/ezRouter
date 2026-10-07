use std::pin::Pin;
use std::task::{Context, Poll};

use axum::{
    body::Body,
    extract::{rejection::JsonRejection, State},
    http::{header, HeaderMap, StatusCode},
    response::Response,
    Json,
};
use bytes::Bytes;
use futures_util::stream::Stream;
use futures_util::TryStreamExt;
use serde::Deserialize;

use crate::auth::AuthenticatedUser;
use crate::error::AppError;
use crate::provider::{ChatCompletionRequest, ChatCompletionResponse, ChatMessage, MessageContent};
use crate::state::AppState;

/// Normalizes model names sent by Anthropic SDKs / Claude Code CLI to internal router model identifiers.
pub fn normalize_anthropic_model(model: &str) -> String {
    let trimmed = model.trim();
    if trimmed.starts_with("ag/")
        || trimmed.starts_with("cx/")
        || trimmed.starts_with("openrouter/")
    {
        return trimmed.to_string();
    }
    let lower = trimmed.to_lowercase();
    if lower.contains("opus") {
        "ag/claude-opus-5-5-high".to_string()
    } else if lower.contains("haiku") {
        "ag/gemini-3.8-flash-high".to_string()
    } else if lower.contains("sonnet") || lower.contains("claude") {
        "ag/claude-sonnet-5-5-high".to_string()
    } else if lower.contains("gpt-6") || lower.contains("codex") {
        "cx/gpt-6.1-sol".to_string()
    } else {
        format!("ag/{trimmed}")
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnthropicIncomingMessage {
    pub role: String,
    pub content: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnthropicMessagesRequest {
    pub model: String,
    pub messages: Vec<AnthropicIncomingMessage>,
    pub system: Option<serde_json::Value>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub stream: Option<bool>,
    pub tools: Option<Vec<serde_json::Value>>,
}

/// Converts Anthropic Messages payload into standard OpenAI ChatCompletionRequest
pub fn anthropic_to_openai_request(
    req: AnthropicMessagesRequest,
) -> (ChatCompletionRequest, String) {
    let original_model = req.model.clone();
    let mapped_model = normalize_anthropic_model(&req.model);

    let mut chat_messages = Vec::new();

    // 1. Process system prompt
    if let Some(sys_val) = req.system {
        let system_text = if let Some(s) = sys_val.as_str() {
            s.to_string()
        } else if let Some(arr) = sys_val.as_array() {
            arr.iter()
                .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("\n\n")
        } else {
            sys_val.to_string()
        };

        if !system_text.trim().is_empty() {
            chat_messages.push(ChatMessage {
                role: "system".to_string(),
                content: Some(MessageContent::Text(system_text)),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            });
        }
    }

    // 2. Process conversation messages
    for msg in req.messages {
        let role = msg.role;
        match msg.content {
            serde_json::Value::String(text) => {
                chat_messages.push(ChatMessage {
                    role,
                    content: Some(MessageContent::Text(text)),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                });
            }
            serde_json::Value::Array(blocks) => {
                let mut text_parts = Vec::new();
                let mut tool_calls = Vec::new();

                for block in blocks {
                    let b_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    match b_type {
                        "text" => {
                            if let Some(txt) = block.get("text").and_then(|t| t.as_str()) {
                                text_parts.push(txt.to_string());
                            }
                        }
                        "tool_use" => {
                            let id = block
                                .get("id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let name = block
                                .get("name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let input_args =
                                block.get("input").cloned().unwrap_or(serde_json::json!({}));
                            let args_str = serde_json::to_string(&input_args)
                                .unwrap_or_else(|_| "{}".to_string());

                            tool_calls.push(serde_json::json!({
                                "id": id,
                                "type": "function",
                                "function": {
                                    "name": name,
                                    "arguments": args_str
                                }
                            }));
                        }
                        "tool_result" => {
                            let tool_use_id = block
                                .get("tool_use_id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let content_val = block.get("content");
                            let content_str = if let Some(cv) = content_val {
                                if let Some(s) = cv.as_str() {
                                    s.to_string()
                                } else {
                                    serde_json::to_string(cv).unwrap_or_default()
                                }
                            } else {
                                String::new()
                            };

                            chat_messages.push(ChatMessage {
                                role: "tool".to_string(),
                                content: Some(MessageContent::Text(content_str)),
                                name: None,
                                tool_calls: None,
                                tool_call_id: Some(tool_use_id),
                            });
                        }
                        _ => {}
                    }
                }

                let combined_text = if text_parts.is_empty() {
                    None
                } else {
                    Some(MessageContent::Text(text_parts.join("")))
                };

                let calls_opt = if tool_calls.is_empty() {
                    None
                } else {
                    Some(serde_json::Value::Array(tool_calls))
                };

                if combined_text.is_some() || calls_opt.is_some() {
                    chat_messages.push(ChatMessage {
                        role,
                        content: combined_text,
                        name: None,
                        tool_calls: calls_opt,
                        tool_call_id: None,
                    });
                }
            }
            _ => {
                chat_messages.push(ChatMessage {
                    role,
                    content: Some(MessageContent::Text(msg.content.to_string())),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                });
            }
        }
    }

    // 3. Process tools
    let openai_tools = req.tools.map(|tools_vec| {
        let arr: Vec<serde_json::Value> = tools_vec
            .into_iter()
            .map(|t| {
                let name = t.get("name").cloned().unwrap_or(serde_json::json!(""));
                let desc = t
                    .get("description")
                    .cloned()
                    .unwrap_or(serde_json::json!(""));
                let schema = t.get("input_schema").cloned().unwrap_or(serde_json::json!({
                    "type": "object",
                    "properties": {}
                }));

                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": name,
                        "description": desc,
                        "parameters": schema
                    }
                })
            })
            .collect();
        serde_json::Value::Array(arr)
    });

    let openai_req = ChatCompletionRequest {
        model: mapped_model,
        messages: chat_messages,
        temperature: req.temperature,
        top_p: req.top_p,
        max_tokens: req.max_tokens,
        stream: req.stream,
        tools: openai_tools,
        ..Default::default()
    };

    (openai_req, original_model)
}

/// Converts an OpenAI ChatCompletionResponse into Anthropic Messages format
pub fn openai_to_anthropic_response(
    resp: &ChatCompletionResponse,
    requested_model: &str,
) -> serde_json::Value {
    let choice = resp.choices.first();
    let mut content_blocks = Vec::new();
    let mut stop_reason = "end_turn";

    if let Some(c) = choice {
        if let Some(ref text) = c.message.content {
            if !text.is_empty() {
                content_blocks.push(serde_json::json!({
                    "type": "text",
                    "text": text
                }));
            }
        }

        if let Some(calls) = c.message.tool_calls.as_ref().and_then(|v| v.as_array()) {
            if !calls.is_empty() {
                stop_reason = "tool_use";
                for call in calls {
                    let id = call
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("toolu_auto");
                    let name = call
                        .get("function")
                        .and_then(|f| f.get("name"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let args_val = call
                        .get("function")
                        .and_then(|f| f.get("arguments"))
                        .and_then(|a| {
                            if let Some(s) = a.as_str() {
                                serde_json::from_str::<serde_json::Value>(s).ok()
                            } else {
                                Some(a.clone())
                            }
                        })
                        .unwrap_or(serde_json::json!({}));

                    content_blocks.push(serde_json::json!({
                        "type": "tool_use",
                        "id": id,
                        "name": name,
                        "input": args_val
                    }));
                }
            }
        }
    }

    serde_json::json!({
        "id": format!("msg_{}", uuid::Uuid::new_v4().simple()),
        "type": "message",
        "role": "assistant",
        "content": content_blocks,
        "model": requested_model,
        "stop_reason": stop_reason,
        "stop_sequence": null,
        "usage": {
            "input_tokens": resp.usage.prompt_tokens,
            "output_tokens": resp.usage.completion_tokens
        }
    })
}

/// Stream adapter that transforms incoming OpenAI SSE chunks into Anthropic SSE stream chunks
pub struct AnthropicStreamAdapter {
    inner: Pin<Box<dyn Stream<Item = Result<Bytes, AppError>> + Send>>,
    message_id: String,
    model: String,
    sent_start: bool,
    sent_block_start: bool,
    sent_stop: bool,
    accumulated_output: String,
    input_tokens: i64,
    output_tokens: i64,
}

impl AnthropicStreamAdapter {
    pub fn new(
        inner: impl Stream<Item = Result<Bytes, AppError>> + Send + 'static,
        model: String,
        prompt_tokens: i64,
    ) -> Self {
        Self {
            inner: Box::pin(inner),
            message_id: format!("msg_{}", uuid::Uuid::new_v4().simple()),
            model,
            sent_start: false,
            sent_block_start: false,
            sent_stop: false,
            accumulated_output: String::new(),
            input_tokens: prompt_tokens,
            output_tokens: 0,
        }
    }
}

impl Stream for AnthropicStreamAdapter {
    type Item = Result<Bytes, AppError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.as_mut().get_mut();

        // 1. Send initial message_start & content_block_start if not sent yet
        if !this.sent_start {
            this.sent_start = true;
            let start_event = format!(
                "event: message_start\ndata: {}\n\n",
                serde_json::json!({
                    "type": "message_start",
                    "message": {
                        "id": this.message_id,
                        "type": "message",
                        "role": "assistant",
                        "content": [],
                        "model": this.model,
                        "stop_reason": null,
                        "stop_sequence": null,
                        "usage": {
                            "input_tokens": this.input_tokens,
                            "output_tokens": 1
                        }
                    }
                })
            );
            return Poll::Ready(Some(Ok(Bytes::from(start_event))));
        }

        if !this.sent_block_start {
            this.sent_block_start = true;
            let block_start = format!(
                "event: content_block_start\ndata: {}\n\n",
                serde_json::json!({
                    "type": "content_block_start",
                    "index": 0,
                    "content_block": {
                        "type": "text",
                        "text": ""
                    }
                })
            );
            return Poll::Ready(Some(Ok(Bytes::from(block_start))));
        }

        // 2. Poll inner stream chunks and convert text deltas
        match Pin::new(&mut this.inner).poll_next(cx) {
            Poll::Ready(Some(Ok(chunk))) => {
                let text = match std::str::from_utf8(&chunk) {
                    Ok(t) => t,
                    Err(_) => return Poll::Ready(Some(Ok(chunk))),
                };

                let mut sse_output = String::new();
                for line in text.lines() {
                    let trimmed = line.trim();
                    if let Some(rest) = trimmed.strip_prefix("data:") {
                        let data_str = rest.trim();
                        if data_str.is_empty() || data_str == "[DONE]" {
                            continue;
                        }
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(data_str) {
                            if let Some(delta) = v
                                .get("choices")
                                .and_then(|c| c.get(0))
                                .and_then(|c0| c0.get("delta"))
                            {
                                if let Some(content) = delta.get("content").and_then(|c| c.as_str())
                                {
                                    if !content.is_empty() {
                                        this.output_tokens += 1;
                                        this.accumulated_output.push_str(content);
                                        sse_output.push_str(&format!(
                                            "event: content_block_delta\ndata: {}\n\n",
                                            serde_json::json!({
                                                "type": "content_block_delta",
                                                "index": 0,
                                                "delta": {
                                                    "type": "text_delta",
                                                    "text": content
                                                }
                                            })
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }

                if sse_output.is_empty() {
                    cx.waker().wake_by_ref();
                    Poll::Pending
                } else {
                    Poll::Ready(Some(Ok(Bytes::from(sse_output))))
                }
            }
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e))),
            Poll::Ready(None) => {
                if !this.sent_stop {
                    this.sent_stop = true;
                    // Stream ended, send final closing events
                    let stop_events = format!(
                        "event: content_block_stop\ndata: {}\n\nevent: message_delta\ndata: {}\n\nevent: message_stop\ndata: {}\n\n",
                        serde_json::json!({
                            "type": "content_block_stop",
                            "index": 0
                        }),
                        serde_json::json!({
                            "type": "message_delta",
                            "delta": {
                                "stop_reason": "end_turn",
                                "stop_sequence": null
                            },
                            "usage": {
                                "output_tokens": this.output_tokens.max(1)
                            }
                        }),
                        serde_json::json!({
                            "type": "message_stop"
                        })
                    );
                    Poll::Ready(Some(Ok(Bytes::from(stop_events))))
                } else {
                    Poll::Ready(None)
                }
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

pub async fn post_messages(
    auth: AuthenticatedUser,
    State(state): State<AppState>,
    headers: HeaderMap,
    payload_result: Result<Json<AnthropicMessagesRequest>, JsonRejection>,
) -> Result<Response, AppError> {
    let Json(anthropic_req) = payload_result.map_err(|e| AppError::BadRequest(e.to_string()))?;
    let is_stream = anthropic_req.stream.unwrap_or(false);

    let (openai_req, requested_model) = anthropic_to_openai_request(anthropic_req);

    // Handle mock provider directly in tests
    if state.config.use_mock_provider {
        let _ = state.db.increment_total_requests(&auth.key);
        if is_stream {
            let mock_text = "Xin chào! Tôi là Claude được vận hành qua ezRouter.";
            let sse_body = format!(
                "event: message_start\ndata: {}\n\nevent: content_block_start\ndata: {}\n\nevent: content_block_delta\ndata: {}\n\nevent: content_block_stop\ndata: {}\n\nevent: message_delta\ndata: {}\n\nevent: message_stop\ndata: {}\n\n",
                serde_json::json!({
                    "type": "message_start",
                    "message": {
                        "id": "msg_mock",
                        "type": "message",
                        "role": "assistant",
                        "content": [],
                        "model": requested_model,
                        "stop_reason": null,
                        "stop_sequence": null,
                        "usage": {"input_tokens": 10, "output_tokens": 1}
                    }
                }),
                serde_json::json!({
                    "type": "content_block_start",
                    "index": 0,
                    "content_block": {"type": "text", "text": ""}
                }),
                serde_json::json!({
                    "type": "content_block_delta",
                    "index": 0,
                    "delta": {"type": "text_delta", "text": mock_text}
                }),
                serde_json::json!({"type": "content_block_stop", "index": 0}),
                serde_json::json!({
                    "type": "message_delta",
                    "delta": {"stop_reason": "end_turn", "stop_sequence": null},
                    "usage": {"output_tokens": 15}
                }),
                serde_json::json!({"type": "message_stop"})
            );

            return Ok(Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/event-stream; charset=utf-8")
                .header(header::CACHE_CONTROL, "no-cache")
                .header(header::CONNECTION, "keep-alive")
                .body(Body::from(sse_body))
                .unwrap());
        } else {
            let mock_resp = serde_json::json!({
                "id": format!("msg_{}", uuid::Uuid::new_v4().simple()),
                "type": "message",
                "role": "assistant",
                "content": [
                    {
                        "type": "text",
                        "text": "Xin chào! Tôi là Claude được vận hành qua ezRouter."
                    }
                ],
                "model": requested_model,
                "stop_reason": "end_turn",
                "stop_sequence": null,
                "usage": {
                    "input_tokens": 10,
                    "output_tokens": 15
                }
            });

            return Ok(Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .body(Body::from(mock_resp.to_string()))
                .unwrap());
        }
    }

    // Call chat completions handler
    let chat_res =
        crate::routes::chat::chat_completions(auth, State(state), headers, Ok(Json(openai_req)))
            .await?;

    if is_stream {
        let stream = AnthropicStreamAdapter::new(
            chat_res
                .into_body()
                .into_data_stream()
                .map_err(|e| AppError::Internal(e.to_string())),
            requested_model,
            10,
        );
        Ok(Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/event-stream; charset=utf-8")
            .header(header::CACHE_CONTROL, "no-cache")
            .header(header::CONNECTION, "keep-alive")
            .body(Body::from_stream(stream))
            .unwrap())
    } else {
        let bytes = axum::body::to_bytes(chat_res.into_body(), 10 * 1024 * 1024)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let oai_resp: ChatCompletionResponse = serde_json::from_slice(&bytes)
            .map_err(|e| AppError::Internal(format!("Failed to parse OpenAI response: {e}")))?;

        let anthropic_json = openai_to_anthropic_response(&oai_resp, &requested_model);

        Ok(Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
            .body(Body::from(anthropic_json.to_string()))
            .unwrap())
    }
}
