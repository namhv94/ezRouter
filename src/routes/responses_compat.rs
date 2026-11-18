//! Translation between the OpenAI **Responses API** (`/v1/responses`) and the
//! internal **Chat Completions** pipeline (`/v1/chat/completions`).
//!
//! OpenAI's Responses API clients only speak the Responses API, but ezRouter's
//! own upstreams (combos such as `all` and external providers)
//! are chat-completion based. This module converts in both directions:
//!
//! - `responses_payload_to_chat`: Responses request  -> chat request
//! - `chat_json_to_responses`:      chat response    -> Responses response
//! - `ChatSseToResponsesStream`:    chat SSE chunks  -> Responses SSE events
//!
//! The raw `/v1/responses` passthrough in `routes::responses` never goes
//! through this module.

use std::collections::HashMap;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Bytes;
use futures_util::stream::Stream;
use tracing::warn;

use crate::error::AppError;
use crate::provider::{ChatCompletionRequest, ChatMessage, MessageContent};

// ---------------------------------------------------------------------------
// Request conversion: Responses -> Chat
// ---------------------------------------------------------------------------

fn content_parts_to_chat_parts(items: &[serde_json::Value]) -> Vec<serde_json::Value> {
    let mut parts = Vec::with_capacity(items.len());
    for item in items {
        let ty = item.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match ty {
            "input_text" | "output_text" | "text" | "summary_text" => {
                if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                    parts.push(serde_json::json!({ "type": "text", "text": text }));
                }
            }
            "input_image" => {
                if let Some(url) = item
                    .get("image_url")
                    .and_then(|u| u.as_str())
                    .or_else(|| item.get("url").and_then(|u| u.as_str()))
                {
                    parts.push(serde_json::json!({
                        "type": "image_url",
                        "image_url": { "url": url }
                    }));
                }
            }
            "refusal" => {
                if let Some(text) = item.get("refusal").and_then(|t| t.as_str()) {
                    parts.push(serde_json::json!({ "type": "text", "text": text }));
                }
            }
            // Silently ignore unsupported part types (e.g. input_audio, file).
            _ => {}
        }
    }
    parts
}

fn normalize_role(role: &str) -> &str {
    match role {
        "developer" | "system" => "system",
        "assistant" => "assistant",
        _ => "user",
    }
}

fn push_input_item(
    item: &serde_json::Value,
    messages: &mut Vec<ChatMessage>,
    warn_ctx: &str,
) {
    let ty = item.get("type").and_then(|t| t.as_str()).unwrap_or("message");
    match ty {
        "message" => {
            let role = item.get("role").and_then(|r| r.as_str()).unwrap_or("user");
            let chat_role = normalize_role(role);
            let content = match item.get("content") {
                None | Some(serde_json::Value::Null) => MessageContent::Text(String::new()),
                Some(serde_json::Value::String(s)) => MessageContent::Text(s.clone()),
                Some(serde_json::Value::Array(parts)) => {
                    MessageContent::Parts(content_parts_to_chat_parts(parts))
                }
                Some(other) => {
                    warn!("{warn_ctx}: unsupported message content {other}, coercing to text");
                    MessageContent::Text(other.to_string())
                }
            };
            messages.push(ChatMessage {
                role: chat_role.to_string(),
                content: Some(content),
                name: None,
                tool_call_id: None,
                tool_calls: None,
            });
        }
        "function_call" => {
            // Assistant tool invocation carried as a standalone input item.
            let name = item.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let arguments = item
                .get("arguments")
                .and_then(|a| a.as_str())
                .unwrap_or("{}");
            let call_id = item
                .get("call_id")
                .and_then(|c| c.as_str())
                .or_else(|| item.get("id").and_then(|c| c.as_str()))
                .unwrap_or("");
            if !name.is_empty() && !call_id.is_empty() {
                messages.push(ChatMessage {
                    role: "assistant".to_string(),
                    content: None,
                    name: None,
                    tool_call_id: None,
                    tool_calls: Some(serde_json::json!([{
                        "id": call_id,
                        "type": "function",
                        "function": { "name": name, "arguments": arguments }
                    }])),
                });
            }
        }
        "function_call_output" => {
            let call_id = item.get("call_id").and_then(|c| c.as_str()).unwrap_or("");
            let output = match item.get("output") {
                Some(serde_json::Value::String(s)) => s.clone(),
                Some(serde_json::Value::Null) => String::new(),
                Some(other) => other.to_string(),
                None => String::new(),
            };
            if !call_id.is_empty() {
                messages.push(ChatMessage {
                    role: "tool".to_string(),
                    content: Some(MessageContent::Text(output)),
                    name: None,
                    tool_call_id: Some(call_id.to_string()),
                    tool_calls: None,
                });
            }
        }
        // Reasoning items, images, etc. carry no chat-completion semantics.
        _ => {
            warn!("{warn_ctx}: ignoring unsupported Responses input item type '{ty}'");
        }
    }
}

fn convert_tools(payload: &serde_json::Value) -> Option<serde_json::Value> {
    let tools = payload.get("tools")?.as_array()?;
    let mut converted = Vec::with_capacity(tools.len());
    for tool in tools {
        let ty = tool.get("type").and_then(|t| t.as_str()).unwrap_or("");
        if ty == "function" {
            if tool.get("function").is_some() {
                // Already in chat format.
                converted.push(tool.clone());
            } else {
                // Responses format: {type: "function", name, description, parameters}
                let mut function = serde_json::Map::new();
                for key in ["name", "description", "parameters", "strict"] {
                    if let Some(v) = tool.get(key) {
                        function.insert(key.to_string(), v.clone());
                    }
                }
                converted.push(serde_json::json!({ "type": "function", "function": function }));
            }
        } else {
            warn!(
                "responses->chat: dropping unsupported tool type '{ty}' \
                 (only 'function' tools are supported on chat upstreams)"
            );
        }
    }
    if converted.is_empty() {
        None
    } else {
        Some(serde_json::Value::Array(converted))
    }
}

fn convert_tool_choice(payload: &serde_json::Value) -> Option<serde_json::Value> {
    let tc = payload.get("tool_choice")?;
    match tc {
        serde_json::Value::String(_) => Some(tc.clone()),
        serde_json::Value::Object(obj) => {
            let ty = obj.get("type").and_then(|t| t.as_str()).unwrap_or("");
            if ty == "function" {
                if obj.get("function").is_some() {
                    Some(tc.clone())
                } else {
                    let name = obj.get("name").and_then(|n| n.as_str()).unwrap_or("");
                    Some(serde_json::json!({
                        "type": "function",
                        "function": { "name": name }
                    }))
                }
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Convert a raw `/v1/responses` JSON payload into a chat-completions request.
///
/// `requested_model` is the model id the client asked for (a combo name such
/// as `all`, an external `prefix/model`, or a canonical model); candidate
/// resolution happens later inside the shared chat pipeline.
pub fn responses_payload_to_chat(
    payload: &serde_json::Value,
    requested_model: &str,
) -> Result<ChatCompletionRequest, AppError> {
    let mut messages: Vec<ChatMessage> = Vec::new();

    // 1. Instructions -> leading system message.
    if let Some(instructions) = payload.get("instructions").and_then(|i| i.as_str()) {
        if !instructions.trim().is_empty() {
            messages.push(ChatMessage::system(instructions));
        }
    }

    // 2. Input: either a plain string or an array of typed items.
    match payload.get("input") {
        None | Some(serde_json::Value::Null) => {
            return Err(AppError::BadRequest(
                "input cannot be empty".to_string(),
            ));
        }
        Some(serde_json::Value::String(text)) => {
            if text.trim().is_empty() {
                return Err(AppError::BadRequest(
                    "input cannot be empty".to_string(),
                ));
            }
            messages.push(ChatMessage::user(text.clone()));
        }
        Some(serde_json::Value::Array(items)) => {
            if items.is_empty() {
                return Err(AppError::BadRequest(
                    "input cannot be empty".to_string(),
                ));
            }
            for item in items {
                push_input_item(item, &mut messages, "responses->chat");
            }
        }
        Some(other) => {
            return Err(AppError::BadRequest(format!(
                "unsupported Responses input type: {other}"
            )));
        }
    }

    if messages.is_empty() {
        return Err(AppError::BadRequest(
            "input contains no convertible messages".to_string(),
        ));
    }

    let stream = payload
        .get("stream")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    // Only forward `tool_choice` when the converted tool list is non-empty.
    // Clients commonly send `tool_choice: "auto"`, and several chat upstreams
    // reject it with HTTP 200 + an SSE error when no tools are present.
    let tools = convert_tools(payload);
    let has_tools = tools
        .as_ref()
        .and_then(|t| t.as_array())
        .is_some_and(|t| !t.is_empty());
    let tool_choice = if has_tools {
        convert_tool_choice(payload)
    } else {
        None
    };

    Ok(ChatCompletionRequest {
        model: requested_model.to_string(),
        messages,
        stream: Some(stream),
        tools,
        tool_choice,
        temperature: payload
            .get("temperature")
            .and_then(|t| t.as_f64())
            .map(|t| t as f32),
        top_p: payload
            .get("top_p")
            .and_then(|t| t.as_f64())
            .map(|t| t as f32),
        max_tokens: payload
            .get("max_output_tokens")
            .and_then(|t| t.as_u64())
            .map(|t| t as u32),
        max_completion_tokens: None,
        stream_options: None,
        response_format: convert_response_format(payload),
        stop: None,
        user: None,
        reasoning_effort: None,
    })
}

/// Convert the Responses `text.format` block into a Chat Completions
/// `response_format`.
///
/// The two APIs use different shapes:
/// - Responses: `{"type":"json_schema","name":..,"schema":..,"strict":..}`
/// - Chat:      `{"type":"json_schema","json_schema":{"name":..,"schema":..,"strict":..}}`
///
/// Passing the Responses shape through verbatim makes chat upstreams reject the
/// request (`missing field json_schema`), which breaks structured-output extraction.
fn convert_response_format(payload: &serde_json::Value) -> Option<serde_json::Value> {
    let format = payload.get("text")?.get("format")?;
    match format.get("type").and_then(|v| v.as_str())? {
        "json_schema" => {
            let mut schema = serde_json::Map::new();
            for key in ["name", "schema", "strict", "description"] {
                if let Some(value) = format.get(key) {
                    schema.insert(key.to_string(), value.clone());
                }
            }
            Some(serde_json::json!({
                "type": "json_schema",
                "json_schema": schema,
            }))
        }
        "json_object" => Some(serde_json::json!({ "type": "json_object" })),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Response conversion: Chat -> Responses
// ---------------------------------------------------------------------------

fn usage_to_responses(usage: Option<&serde_json::Value>) -> serde_json::Value {
    let pt = usage
        .and_then(|u| u.get("prompt_tokens"))
        .and_then(|t| t.as_i64())
        .unwrap_or(0);
    let ct = usage
        .and_then(|u| u.get("completion_tokens"))
        .and_then(|t| t.as_i64())
        .unwrap_or(0);
    serde_json::json!({
        "input_tokens": pt,
        "input_tokens_details": { "cached_tokens": 0 },
        "output_tokens": ct,
        "output_tokens_details": { "reasoning_tokens": 0 },
        "total_tokens": pt + ct,
    })
}

fn message_item(id: &str, text: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "type": "message",
        "role": "assistant",
        "status": "completed",
        "content": [{ "type": "output_text", "text": text, "annotations": [] }]
    })
}

fn function_call_item(id: &str, call_id: &str, name: &str, arguments: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "type": "function_call",
        "call_id": call_id,
        "name": name,
        "arguments": arguments,
        "status": "completed"
    })
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Convert a non-streaming chat completion into a Responses response object.
///
/// `chat` is the raw upstream JSON (choices/usage), `requested_model` is the
/// model id the client asked for.
pub fn chat_json_to_responses(chat: &serde_json::Value, requested_model: &str) -> serde_json::Value {
    let choice = chat
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|c| c.first())
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let message = choice.get("message").cloned().unwrap_or(serde_json::Value::Null);

    let text = message
        .get("content")
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();

    let mut output = Vec::new();
    let msg_id = format!("msg_{}", uuid::Uuid::new_v4().simple());
    if !text.is_empty() {
        output.push(message_item(&msg_id, &text));
    }
    if let Some(tool_calls) = message.get("tool_calls").and_then(|t| t.as_array()) {
        for tc in tool_calls.iter() {
            let call_id = tc.get("id").and_then(|c| c.as_str()).unwrap_or("");
            let name = tc
                .get("function")
                .and_then(|f| f.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or("");
            let arguments = tc
                .get("function")
                .and_then(|f| f.get("arguments"))
                .and_then(|a| a.as_str())
                .unwrap_or("{}");
            if name.is_empty() {
                continue;
            }
            output.push(function_call_item(
                &format!("fc_{}", uuid::Uuid::new_v4().simple()),
                call_id,
                name,
                arguments,
            ));
        }
    }

    let finish = choice.get("finish_reason").and_then(|f| f.as_str());
    let (status, incomplete) = match finish {
        Some("length") => ("incomplete", serde_json::json!({ "reason": "max_output_tokens" })),
        Some("content_filter") => ("incomplete", serde_json::json!({ "reason": "content_filter" })),
        _ => ("completed", serde_json::Value::Null),
    };

    let mut out = serde_json::json!({
        "id": format!("resp_{}", uuid::Uuid::new_v4().simple()),
        "object": "response",
        "created_at": now_unix(),
        "status": status,
        "model": requested_model,
        "output": output,
        "output_text": text,
        "usage": usage_to_responses(chat.get("usage")),
        "error": serde_json::Value::Null,
        "incomplete_details": incomplete,
        "metadata": {},
    });
    if status == "completed" && out["output"].as_array().is_some_and(|a| a.is_empty()) {
        // Ensure at least one message item so clients always have content.
        out["output"] = serde_json::json!([message_item(&msg_id, "")]);
    }
    out
}

// ---------------------------------------------------------------------------
// Streaming conversion: Chat SSE -> Responses SSE
// ---------------------------------------------------------------------------

#[derive(Default)]
struct OpenToolCall {
    item_id: String,
    call_id: String,
    name: String,
    arguments: String,
    finalized: bool,
}

/// Stream transformer that converts an upstream chat-completions SSE stream
/// into the Responses API event stream expected by Responses API clients.
pub struct ChatSseToResponsesStream {
    inner: Pin<Box<dyn Stream<Item = Result<Bytes, AppError>> + Send>>,
    buffer: Vec<u8>,
    model: String,
    response_id: String,
    created_emitted: bool,
    // Message state.
    msg_item_added: bool,
    msg_item_id: String,
    msg_text: String,
    msg_done: bool,
    // Function-call accumulation, keyed by chat `tool_calls[].index`.
    open_calls: HashMap<u64, OpenToolCall>,
    output_index: u64,
    usage: Option<(i64, i64)>,
    response_emitted: bool,
    done_emitted: bool,
}

impl ChatSseToResponsesStream {
    pub fn new(
        inner: Pin<Box<dyn Stream<Item = Result<Bytes, AppError>> + Send>>,
        model: String,
    ) -> Self {
        Self {
            inner,
            buffer: Vec::new(),
            model,
            response_id: format!("resp_{}", uuid::Uuid::new_v4().simple()),
            created_emitted: false,
            msg_item_added: false,
            msg_item_id: String::new(),
            msg_text: String::new(),
            msg_done: false,
            open_calls: HashMap::new(),
            output_index: 0,
            usage: None,
            response_emitted: false,
            done_emitted: false,
        }
    }

    fn response_skeleton(&self, status: &str) -> serde_json::Value {
        serde_json::json!({
            "id": self.response_id,
            "object": "response",
            "created_at": now_unix(),
            "status": status,
            "model": self.model,
            "output": [],
            "usage": usage_to_responses(None),
            "error": serde_json::Value::Null,
            "incomplete_details": serde_json::Value::Null,
        })
    }

    fn sse(event: &str, data: &serde_json::Value) -> Bytes {
        Bytes::from(format!("event: {event}\ndata: {data}\n\n"))
    }

    fn ensure_created(&mut self, out: &mut Vec<Bytes>) {
        if !self.created_emitted {
            self.created_emitted = true;
            let response = self.response_skeleton("in_progress");
            out.push(Self::sse("response.created", &{
                serde_json::json!({ "type": "response.created", "sequence_number": 0, "response": response })
            }));
        }
    }

    fn ensure_message_open(&mut self, out: &mut Vec<Bytes>) {
        if self.msg_done {
            return;
        }
        if !self.msg_item_added {
            self.msg_item_added = true;
            self.msg_item_id = format!("msg_{}", uuid::Uuid::new_v4().simple());
            let item = serde_json::json!({
                "id": self.msg_item_id,
                "type": "message",
                "role": "assistant",
                "status": "in_progress",
                "content": []
            });
            out.push(Self::sse("response.output_item.added", &serde_json::json!({
                "type": "response.output_item.added",
                "output_index": self.output_index,
                "item": item,
            })));
            out.push(Self::sse("response.content_part.added", &serde_json::json!({
                "type": "response.content_part.added",
                "item_id": self.msg_item_id,
                "output_index": self.output_index,
                "content_index": 0,
                "part": { "type": "output_text", "text": "", "annotations": [] }
            })));
        }
    }

    fn close_message(&mut self, out: &mut Vec<Bytes>) {
        if self.msg_item_added && !self.msg_done {
            self.msg_done = true;
            let item = message_item(&self.msg_item_id.clone(), &self.msg_text.clone());
            out.push(Self::sse("response.content_part.done", &serde_json::json!({
                "type": "response.content_part.done",
                "item_id": self.msg_item_id,
                "output_index": self.output_index,
                "content_index": 0,
                "part": { "type": "output_text", "text": self.msg_text.clone(), "annotations": [] }
            })));
            out.push(Self::sse("response.output_item.done", &serde_json::json!({
                "type": "response.output_item.done",
                "output_index": self.output_index,
                "item": item,
            })));
            self.output_index += 1;
        }
    }

    fn close_open_calls(&mut self, out: &mut Vec<Bytes>) {
        let mut indices: Vec<u64> = self.open_calls.keys().copied().collect();
        indices.sort_unstable();
        for idx in indices {
            if let Some(call) = self.open_calls.get_mut(&idx) {
                if !call.finalized {
                    call.finalized = true;
                    out.push(Self::sse("response.function_call_arguments.done", &serde_json::json!({
                        "type": "response.function_call_arguments.done",
                        "item_id": call.item_id,
                        "output_index": self.output_index,
                        "arguments": call.arguments.clone(),
                    })));
                    let item = function_call_item(
                        &call.item_id.clone(),
                        &call.call_id.clone(),
                        &call.name.clone(),
                        &call.arguments.clone(),
                    );
                    out.push(Self::sse("response.output_item.done", &serde_json::json!({
                        "type": "response.output_item.done",
                        "output_index": self.output_index,
                        "item": item,
                    })));
                    self.output_index += 1;
                }
            }
        }
    }

    fn finalize(&mut self, out: &mut Vec<Bytes>) {
        if self.response_emitted {
            return;
        }
        self.response_emitted = true;
        let mut events = Vec::new();
        self.ensure_created(&mut events);
        self.close_message(&mut events);
        self.close_open_calls(&mut events);
        out.extend(events);
        let mut response = self.response_skeleton("completed");
        if let Some((pt, ct)) = self.usage {
            response["usage"] = usage_to_responses(Some(&serde_json::json!({
                "prompt_tokens": pt,
                "completion_tokens": ct,
            })));
        }
        response["output_text"] = serde_json::Value::String(self.msg_text.clone());
        out.push(Self::sse("response.completed", &serde_json::json!({
            "type": "response.completed",
            "response": response,
        })));
    }

    /// Convert one complete chat SSE `data:` payload into Responses events.
    fn handle_chunk(&mut self, data_str: &str, out: &mut Vec<Bytes>) {
        let Ok(chunk) = serde_json::from_str::<serde_json::Value>(data_str) else {
            return;
        };

        if let Some(usage) = chunk.get("usage").filter(|u| !u.is_null()) {
            let pt = usage
                .get("prompt_tokens")
                .and_then(|t| t.as_i64())
                .unwrap_or(0);
            let ct = usage
                .get("completion_tokens")
                .and_then(|t| t.as_i64())
                .unwrap_or(0);
            if pt > 0 || ct > 0 {
                self.usage = Some((pt, ct));
            }
        }

        let Some(choice) = chunk
            .get("choices")
            .and_then(|c| c.as_array())
            .and_then(|c| c.first())
        else {
            return;
        };

        self.ensure_created(out);

        let delta = choice.get("delta").cloned().unwrap_or(serde_json::Value::Null);

        // Text deltas.
        if let Some(text) = delta.get("content").and_then(|c| c.as_str()) {
            if !text.is_empty() {
                self.ensure_message_open(out);
                self.msg_text.push_str(text);
                out.push(Self::sse("response.output_text.delta", &serde_json::json!({
                    "type": "response.output_text.delta",
                    "item_id": self.msg_item_id,
                    "output_index": self.output_index,
                    "content_index": 0,
                    "delta": text,
                })));
            }
        }

        // Tool-call deltas.
        if let Some(tool_calls) = delta.get("tool_calls").and_then(|t| t.as_array()) {
            for tc in tool_calls {
                let idx = tc.get("index").and_then(|i| i.as_u64()).unwrap_or(0);
                if !self.open_calls.contains_key(&idx) {
                    let item_id = format!("fc_{}", uuid::Uuid::new_v4().simple());
                    let call_id = tc
                        .get("id")
                        .and_then(|c| c.as_str())
                        .unwrap_or("")
                        .to_string();
                    let name = tc
                        .get("function")
                        .and_then(|f| f.get("name"))
                        .and_then(|n| n.as_str())
                        .unwrap_or("")
                        .to_string();
                    // Announce the new function_call item.
                    let item = serde_json::json!({
                        "id": item_id,
                        "type": "function_call",
                        "call_id": call_id,
                        "name": name,
                        "arguments": "",
                        "status": "in_progress"
                    });
                    out.push(Self::sse("response.output_item.added", &serde_json::json!({
                        "type": "response.output_item.added",
                        "output_index": self.output_index,
                        "item": item,
                    })));
                    self.output_index += 1;
                    self.open_calls.insert(
                        idx,
                        OpenToolCall {
                            item_id,
                            call_id,
                            name,
                            arguments: String::new(),
                            finalized: false,
                        },
                    );
                }
                let entry = self.open_calls.get_mut(&idx).expect("just inserted");
                if let Some(args) = tc
                    .get("function")
                    .and_then(|f| f.get("arguments"))
                    .and_then(|a| a.as_str())
                {
                    if !args.is_empty() {
                        entry.arguments.push_str(args);
                        out.push(Self::sse("response.function_call_arguments.delta", &serde_json::json!({
                            "type": "response.function_call_arguments.delta",
                            "item_id": entry.item_id,
                            "output_index": self.output_index,
                            "delta": args,
                        })));
                    }
                }
                if let Some(id) = tc.get("id").and_then(|c| c.as_str()) {
                    if entry.call_id.is_empty() {
                        entry.call_id = id.to_string();
                    }
                }
                if let Some(name) = tc
                    .get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(|n| n.as_str())
                {
                    if entry.name.is_empty() {
                        entry.name = name.to_string();
                    }
                }
            }
        }

        match choice.get("finish_reason").and_then(|f| f.as_str()) {
            Some("tool_calls") | Some("function") => {
                self.close_message(out);
                self.close_open_calls(out);
            }
            Some(_) => {
                self.close_message(out);
                self.close_open_calls(out);
            }
            None => {}
        }
    }
}

impl Stream for ChatSseToResponsesStream {
    type Item = Result<Bytes, AppError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.as_mut().get_mut();
        loop {
            // Feed more bytes into the buffer.
            match Pin::new(&mut this.inner).poll_next(cx) {
                Poll::Ready(Some(Ok(bytes))) => {
                    this.buffer.extend_from_slice(&bytes);
                    // Process every complete line currently buffered.
                    let mut events: Vec<Bytes> = Vec::new();
                    while let Some(pos) = this.buffer.iter().position(|&b| b == b'\n') {
                        let drained: Vec<u8> = this.buffer.drain(..=pos).collect();
                        let line = String::from_utf8_lossy(&drained);
                        let trimmed = line.trim();
                        let data_str = match trimmed.strip_prefix("data:") {
                            Some(rest) => rest.trim(),
                            None => continue,
                        };
                        if data_str.is_empty() {
                            continue;
                        }
                        if data_str == "[DONE]" {
                            this.finalize(&mut events);
                            continue;
                        }
                        this.handle_chunk(data_str, &mut events);
                    }
                    if !events.is_empty() {
                        return Poll::Ready(Some(Ok(join_events(events))));
                    }
                }
                Poll::Ready(Some(Err(err))) => return Poll::Ready(Some(Err(err))),
                Poll::Ready(None) => {
                    // Flush any trailing buffered line, then finalize.
                    let mut events: Vec<Bytes> = Vec::new();
                    let leftover = std::mem::take(&mut this.buffer);
                    if !leftover.is_empty() {
                        let line = String::from_utf8_lossy(&leftover);
                        if let Some(rest) = line.trim().strip_prefix("data:") {
                            let data_str = rest.trim();
                            if !data_str.is_empty() && data_str != "[DONE]" {
                                this.handle_chunk(data_str, &mut events);
                            }
                        }
                    }
                    if !this.response_emitted {
                        // Empty response: still emit an empty message item so
                        // clients receive at least one output item.
                        if !this.msg_item_added && this.open_calls.is_empty() {
                            this.ensure_message_open(&mut events);
                        }
                        this.finalize(&mut events);
                    }
                    if !this.done_emitted {
                        this.done_emitted = true;
                        return Poll::Ready(Some(Ok(join_events(events))));
                    }
                    return Poll::Ready(None);
                }
                Poll::Pending => {
                    return Poll::Pending;
                }
            }
        }
    }
}

fn join_events(events: Vec<Bytes>) -> Bytes {
    let mut out = Vec::new();
    for e in events {
        out.extend_from_slice(&e);
    }
    Bytes::from(out)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::StreamExt;

    #[test]
    fn test_string_input_to_messages() {
        let payload = serde_json::json!({
            "model": "all",
            "input": "Hello",
            "instructions": "Be concise."
        });
        let req = responses_payload_to_chat(&payload, "all").unwrap();
        assert_eq!(req.model, "all");
        assert_eq!(req.messages.len(), 2);
        assert_eq!(req.messages[0].role, "system");
        assert_eq!(req.messages[0].content.as_ref().unwrap().text(), "Be concise.");
        assert_eq!(req.messages[1].role, "user");
        assert_eq!(req.messages[1].content.as_ref().unwrap().text(), "Hello");
        assert_eq!(req.stream, Some(true));
    }

    #[test]
    fn test_input_items_with_tool_roundtrip() {
        let payload = serde_json::json!({
            "model": "all",
            "instructions": "sys",
            "input": [
                { "type": "message", "role": "user",
                  "content": [{ "type": "input_text", "text": "weather in paris?" }] },
                { "type": "function_call", "call_id": "call_1", "name": "get_weather",
                  "arguments": "{\"city\":\"paris\"}" },
                { "type": "function_call_output", "call_id": "call_1", "output": "sunny" }
            ],
            "tools": [
                { "type": "function", "name": "get_weather", "description": "w",
                  "parameters": { "type": "object", "properties": {} } }
            ],
            "tool_choice": { "type": "function", "name": "get_weather" },
            "max_output_tokens": 256
        });
        let req = responses_payload_to_chat(&payload, "all").unwrap();
        assert_eq!(req.messages.len(), 4);
        assert_eq!(req.messages[0].role, "system");
        assert_eq!(req.messages[1].role, "user");
        assert!(matches!(req.messages[1].content, Some(MessageContent::Parts(_))));
        assert_eq!(req.messages[2].role, "assistant");
        assert!(req.messages[2].tool_calls.is_some());
        assert_eq!(req.messages[3].role, "tool");
        assert_eq!(req.messages[3].tool_call_id.as_deref(), Some("call_1"));
        let tools = req.tools.unwrap();
        assert_eq!(tools[0]["type"], "function");
        assert_eq!(tools[0]["function"]["name"], "get_weather");
        assert_eq!(req.tool_choice.unwrap()["function"]["name"], "get_weather");
        assert_eq!(req.max_tokens, Some(256));
    }

    #[test]
    fn test_json_schema_text_format_is_nested_for_chat() {
        let payload = serde_json::json!({
            "model": "all",
            "input": "extract",
            "text": {
                "format": {
                    "type": "json_schema",
                    "name": "memory",
                    "schema": { "type": "object", "properties": { "a": { "type": "string" } } },
                    "strict": true
                }
            }
        });
        let req = responses_payload_to_chat(&payload, "all").unwrap();
        let rf = req.response_format.expect("response_format");
        assert_eq!(rf["type"], "json_schema");
        assert_eq!(rf["json_schema"]["name"], "memory");
        assert_eq!(rf["json_schema"]["strict"], true);
        assert_eq!(rf["json_schema"]["schema"]["type"], "object");
        assert!(rf.get("name").is_none(), "must not be flattened");
    }

    #[test]
    fn test_plain_text_format_has_no_response_format() {
        let payload = serde_json::json!({
            "model": "all",
            "input": "hi",
            "text": { "format": { "type": "text" } }
        });
        let req = responses_payload_to_chat(&payload, "all").unwrap();
        assert!(req.response_format.is_none());
    }

    #[test]
    fn test_tool_choice_dropped_when_no_tools() {
        let payload = serde_json::json!({
            "model": "all",
            "input": "hi",
            "tool_choice": "auto"
        });
        let req = responses_payload_to_chat(&payload, "all").unwrap();
        assert!(req.tools.is_none());
        assert!(
            req.tool_choice.is_none(),
            "tool_choice must be dropped when tools are absent"
        );
    }

    #[test]
    fn test_chat_json_to_responses() {
        let chat = serde_json::json!({
            "id": "chatcmpl-1",
            "choices": [{
                "index": 0,
                "message": { "role": "assistant", "content": "OK" },
                "finish_reason": "stop"
            }],
            "usage": { "prompt_tokens": 10, "completion_tokens": 2, "total_tokens": 12 }
        });
        let resp = chat_json_to_responses(&chat, "all");
        assert_eq!(resp["object"], "response");
        assert_eq!(resp["status"], "completed");
        assert_eq!(resp["model"], "all");
        let output = resp["output"].as_array().unwrap();
        assert_eq!(output.len(), 1);
        assert_eq!(output[0]["type"], "message");
        assert_eq!(output[0]["content"][0]["text"], "OK");
        assert_eq!(resp["usage"]["input_tokens"], 10);
        assert_eq!(resp["usage"]["output_tokens"], 2);
    }

    #[test]
    fn test_chat_json_tool_calls_to_responses() {
        let chat = serde_json::json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": { "name": "get_weather", "arguments": "{\"city\":\"paris\"}" }
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        });
        let resp = chat_json_to_responses(&chat, "all");
        let output = resp["output"].as_array().unwrap();
        assert_eq!(output.len(), 1);
        assert_eq!(output[0]["type"], "function_call");
        assert_eq!(output[0]["call_id"], "call_1");
        assert_eq!(output[0]["name"], "get_weather");
        assert_eq!(output[0]["arguments"], "{\"city\":\"paris\"}");
    }

    #[tokio::test]
    async fn test_stream_text_only() {
        let chat_chunks = vec![
            Ok(Bytes::from("data: {\"choices\":[{\"delta\":{\"content\":\"He\"}}]}\n\n")),
            Ok(Bytes::from("data: {\"choices\":[{\"delta\":{\"content\":\"llo\"}}]}\n\n")),
            Ok(Bytes::from("data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":3,\"completion_tokens\":2}}\n\n")),
            Ok(Bytes::from("data: [DONE]\n\n")),
        ];
        let inner: Pin<Box<dyn Stream<Item = Result<Bytes, AppError>> + Send>> =
            Box::pin(futures_util::stream::iter(chat_chunks));
        let mut stream = ChatSseToResponsesStream::new(inner, "all".to_string());
        let mut acc = String::new();
        while let Some(item) = stream.next().await {
            let bytes = item.unwrap();
            acc.push_str(&String::from_utf8(bytes.to_vec()).unwrap());
        }

        assert!(acc.contains("event: response.created"));
        assert!(acc.contains("event: response.output_item.added"));
        assert!(acc.contains("\"delta\":\"He\""));
        assert!(acc.contains("\"delta\":\"llo\""));
        assert!(acc.contains("event: response.output_text.delta"));
        assert!(acc.contains("event: response.output_item.done"));
        assert!(acc.contains("event: response.completed"));
        assert!(acc.contains("\"status\":\"completed\""));
        assert!(acc.contains("\"input_tokens\":3"));
        // Must not leak chat chunks.
        assert!(!acc.contains("chat.completion.chunk"));
        assert!(!acc.contains("[DONE]"));
    }

    #[tokio::test]
    async fn test_stream_tool_calls() {
        let chat_chunks = vec![
            Ok(Bytes::from("data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"function\":{\"name\":\"get_weather\",\"arguments\":\"\"}}]}}]}\n\n")),
            Ok(Bytes::from("data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"city\\\":\"}}]}}]}\n\n")),
            Ok(Bytes::from("data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"\\\"paris\\\"}\"}}]},\"finish_reason\":\"tool_calls\"}]}\n\n")),
            Ok(Bytes::from("data: [DONE]\n\n")),
        ];
        let inner: Pin<Box<dyn Stream<Item = Result<Bytes, AppError>> + Send>> =
            Box::pin(futures_util::stream::iter(chat_chunks));
        let mut stream = ChatSseToResponsesStream::new(inner, "all".to_string());
        let mut acc = String::new();
        while let Some(item) = stream.next().await {
            acc.push_str(&String::from_utf8(item.unwrap().to_vec()).unwrap());
        }

        assert!(acc.contains("event: response.output_item.added"));
        assert!(acc.contains("\"type\":\"function_call\""));
        assert!(acc.contains("event: response.function_call_arguments.delta"));
        assert!(acc.contains("\"delta\":\"{\\\"city\\\":\""));
        assert!(acc.contains("event: response.function_call_arguments.done"));
        assert!(acc.contains("event: response.completed"));
        // Two output items: no message (no content), one function_call.
        assert!(acc.contains("\"call_id\":\"call_1\""));
    }

    #[tokio::test]
    async fn test_stream_split_chunks_and_crlf() {
        let chat_chunks = vec![
            Ok(Bytes::from("data: {\"choices\":[{\"delta\":{\"content\":\"He\"}}]}\r")),
            Ok(Bytes::from("\ndata: {\"choices\":[{\"delta\":{\"content\":\"y\"}}]}\r\n")),
            Ok(Bytes::from("data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n")),
        ];
        let inner: Pin<Box<dyn Stream<Item = Result<Bytes, AppError>> + Send>> =
            Box::pin(futures_util::stream::iter(chat_chunks));
        let mut stream = ChatSseToResponsesStream::new(inner, "all".to_string());
        let mut acc = String::new();
        while let Some(item) = stream.next().await {
            acc.push_str(&String::from_utf8(item.unwrap().to_vec()).unwrap());
        }
        assert!(acc.contains("\"delta\":\"He\""));
        assert!(acc.contains("\"delta\":\"y\""));
        assert!(acc.contains("event: response.completed"));
    }

    #[tokio::test]
    async fn test_stream_empty_response_still_completes() {
        let chat_chunks = vec![Ok(Bytes::from("data: [DONE]\n\n"))];
        let inner: Pin<Box<dyn Stream<Item = Result<Bytes, AppError>> + Send>> =
            Box::pin(futures_util::stream::iter(chat_chunks));
        let mut stream = ChatSseToResponsesStream::new(inner, "all".to_string());
        let mut acc = String::new();
        while let Some(item) = stream.next().await {
            acc.push_str(&String::from_utf8(item.unwrap().to_vec()).unwrap());
        }
        assert!(acc.contains("event: response.created"));
        assert!(acc.contains("event: response.completed"));
        assert!(acc.contains("\"status\":\"completed\""));
    }
}
