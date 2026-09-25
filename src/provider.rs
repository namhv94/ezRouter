use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bytes::Bytes;
use futures_util::stream::Stream;
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::account::{AccountPool, GoogleAccountLease, TokenRefresher, AG_BASE_URL, AG_USER_AGENT};
use crate::error::AppError;

pub const AG_STREAM_URL_PATH: &str = "/v1internal:streamGenerateContent?alt=sse";
pub const AG_PROJECT: &str = "aicode-consumers";

pub const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024; // 16 MB bounded response limit
pub const MAX_SSE_BUFFER_BYTES: usize = 2 * 1024 * 1024; // 2 MB bounded stream buffer limit

pub fn parse_retry_after(headers: &reqwest::header::HeaderMap) -> Option<f64> {
    if let Some(val) = headers.get(reqwest::header::RETRY_AFTER) {
        if let Ok(s) = val.to_str() {
            let trimmed = s.trim();
            if let Ok(secs) = trimmed.parse::<f64>() {
                return Some(secs.max(1.0));
            }
            if let Ok(dt) = chrono::DateTime::parse_from_rfc2822(trimmed) {
                let now = chrono::Utc::now().timestamp() as f64;
                let target = dt.timestamp() as f64;
                if target > now {
                    return Some((target - now).max(1.0));
                }
            }
        }
    }
    None
}

pub fn deserialize_chat_message_content<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v: Option<serde_json::Value> = serde::Deserialize::deserialize(deserializer)?;
    match v {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::String(s)) => Ok(Some(s)),
        Some(serde_json::Value::Array(arr)) => {
            let mut combined = String::new();
            for item in arr {
                if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                    combined.push_str(text);
                }
            }
            Ok(Some(combined))
        }
        Some(other) => Ok(Some(other.to_string())),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Parts(Vec<serde_json::Value>),
}

impl MessageContent {
    pub fn text(&self) -> std::borrow::Cow<'_, str> {
        match self {
            MessageContent::Text(s) => std::borrow::Cow::Borrowed(s.as_str()),
            MessageContent::Parts(parts) => {
                let mut out = String::new();
                for p in parts {
                    if let Some(s) = p.as_str() {
                        out.push_str(s);
                    } else if let Some(s) = p.get("text").and_then(|t| t.as_str()) {
                        out.push_str(s);
                    } else if let Some(s) = p.get("input_text").and_then(|t| t.as_str()) {
                        out.push_str(s);
                    } else if let Some(s) = p.get("content").and_then(|t| t.as_str()) {
                        out.push_str(s);
                    }
                }
                std::borrow::Cow::Owned(out)
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            MessageContent::Text(s) => s.is_empty(),
            MessageContent::Parts(parts) => parts.is_empty() || self.text().is_empty(),
        }
    }

    pub fn as_parts(&self) -> Option<&[serde_json::Value]> {
        match self {
            MessageContent::Parts(parts) => Some(parts),
            MessageContent::Text(_) => None,
        }
    }
}

impl From<String> for MessageContent {
    fn from(s: String) -> Self {
        MessageContent::Text(s)
    }
}

impl From<&str> for MessageContent {
    fn from(s: &str) -> Self {
        MessageContent::Text(s.to_string())
    }
}

impl From<Vec<serde_json::Value>> for MessageContent {
    fn from(parts: Vec<serde_json::Value>) -> Self {
        MessageContent::Parts(parts)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<MessageContent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<serde_json::Value>,
}

impl Default for ChatMessage {
    fn default() -> Self {
        Self {
            role: "user".to_string(),
            content: None,
            name: None,
            tool_call_id: None,
            tool_calls: None,
        }
    }
}

impl ChatMessage {
    pub fn new(role: impl Into<String>, content: impl Into<MessageContent>) -> Self {
        Self {
            role: role.into(),
            content: Some(content.into()),
            name: None,
            tool_call_id: None,
            tool_calls: None,
        }
    }

    pub fn user(content: impl Into<MessageContent>) -> Self {
        Self::new("user", content)
    }

    pub fn system(content: impl Into<MessageContent>) -> Self {
        Self::new("system", content)
    }

    pub fn assistant(
        content: Option<impl Into<MessageContent>>,
        tool_calls: Option<serde_json::Value>,
    ) -> Self {
        Self {
            role: "assistant".to_string(),
            content: content.map(Into::into),
            name: None,
            tool_call_id: None,
            tool_calls,
        }
    }

    pub fn tool(tool_call_id: impl Into<String>, content: impl Into<MessageContent>) -> Self {
        Self {
            role: "tool".to_string(),
            content: Some(content.into()),
            name: None,
            tool_call_id: Some(tool_call_id.into()),
            tool_calls: None,
        }
    }

    pub fn content_text(&self) -> std::borrow::Cow<'_, str> {
        match &self.content {
            Some(c) => c.text(),
            None => std::borrow::Cow::Borrowed(""),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub stream: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_options: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_format: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
}

impl ChatCompletionRequest {
    pub fn effective_max_tokens(&self) -> Option<u32> {
        self.max_tokens.or(self.max_completion_tokens)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatChoiceMessage {
    pub role: String,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatChoice {
    pub index: u32,
    pub message: ChatChoiceMessage,
    #[serde(default = "default_finish_reason")]
    pub finish_reason: String,
}

fn default_finish_reason() -> String {
    "stop".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UsageInfo {
    #[serde(default)]
    pub prompt_tokens: u32,
    #[serde(default)]
    pub completion_tokens: u32,
    #[serde(default)]
    pub total_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<ChatChoice>,
    pub usage: UsageInfo,
}

pub type BoxChatStream = Pin<Box<dyn Stream<Item = Result<Bytes, AppError>> + Send + 'static>>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatChunkDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatChunkChoice {
    pub index: u32,
    pub delta: ChatChunkDelta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatCompletionChunk {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<ChatChunkChoice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<UsageInfo>,
}

#[axum::async_trait]
pub trait Provider: Send + Sync + std::fmt::Debug {
    async fn complete(
        &self,
        request: &ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, AppError>;

    async fn stream_chat_completion(
        &self,
        request: &ChatCompletionRequest,
    ) -> Result<BoxChatStream, AppError> {
        let _ = request;
        Err(AppError::StreamNotSupported(
            "Streaming is not supported by this provider".to_string(),
        ))
    }
}

/// Resolves the upstream endpoint for chat completions, avoiding duplicate `/v1`.
pub fn resolve_chat_completions_url(base_url: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.ends_with("/v1") {
        format!("{trimmed}/chat/completions")
    } else {
        format!("{trimmed}/v1/chat/completions")
    }
}

pub struct HttpUpstreamProvider {
    base_url: Option<String>,
    api_key: Option<String>,
    client: Client,
    max_response_bytes: usize,
}

impl std::fmt::Debug for HttpUpstreamProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpUpstreamProvider")
            .field("base_url", &self.base_url)
            .field("api_key", &self.api_key.as_ref().map(|_| "[REDACTED]"))
            .field("max_response_bytes", &self.max_response_bytes)
            .finish()
    }
}

impl HttpUpstreamProvider {
    pub fn new(
        base_url: Option<String>,
        api_key: Option<String>,
        connect_timeout_secs: u64,
        read_timeout_secs: u64,
        request_timeout_secs: u64,
    ) -> Self {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(connect_timeout_secs))
            .read_timeout(Duration::from_secs(read_timeout_secs))
            .timeout(Duration::from_secs(request_timeout_secs))
            .build()
            .expect("Failed to build HTTP upstream reqwest client");

        Self {
            base_url,
            api_key,
            client,
            max_response_bytes: MAX_RESPONSE_BYTES,
        }
    }

    pub fn with_client(base_url: Option<String>, api_key: Option<String>, client: Client) -> Self {
        Self {
            base_url,
            api_key,
            client,
            max_response_bytes: MAX_RESPONSE_BYTES,
        }
    }

    pub fn with_max_response_bytes(mut self, max_bytes: usize) -> Self {
        self.max_response_bytes = max_bytes;
        self
    }
}

#[axum::async_trait]
impl Provider for HttpUpstreamProvider {
    async fn complete(
        &self,
        request: &ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, AppError> {
        let base_url = self
            .base_url
            .as_deref()
            .ok_or_else(|| AppError::Internal("Upstream base URL is not configured".to_string()))?;

        let url = resolve_chat_completions_url(base_url);

        let mut req_to_send = request.clone();
        // Strip provider prefix (e.g. "groq/llama-3.3-70b" -> "llama-3.3-70b") for external
        // upstream providers. Do NOT strip ag/ or cx/ prefixes — those are routed by dedicated
        // providers (AntigravityProvider, CodexProvider) and never reach HttpUpstreamProvider.
        {
            let m = &req_to_send.model;
            if !m.starts_with("ag/") && !m.starts_with("cx/") {
                if let Some(pos) = m.find('/') {
                    req_to_send.model = m[pos + 1..].to_string();
                }
            }
        }

        tracing::info!(
            upstream_url = %url,
            model = %req_to_send.model,
            messages_count = req_to_send.messages.len(),
            has_tools = req_to_send.tools.is_some(),
            "Dispatching request to upstream provider"
        );

        let mut req_builder = self.client.post(&url).json(&req_to_send);

        if let Some(ref key) = self.api_key {
            if !key.trim().is_empty() {
                req_builder =
                    req_builder.header(reqwest::header::AUTHORIZATION, format!("Bearer {key}"));
            }
        }

        let mut response = match req_builder.send().await {
            Ok(resp) => resp,
            Err(err) => {
                if err.is_timeout() {
                    return Err(AppError::GatewayTimeout(format!(
                        "Upstream request timed out: {err}"
                    )));
                }
                if err.is_connect() {
                    return Err(AppError::BadGateway(format!(
                        "Upstream connection failed: {err}"
                    )));
                }
                return Err(AppError::BadGateway(format!(
                    "Upstream request failed: {err}"
                )));
            }
        };

        let status = response.status();

        if let Some(content_length) = response.content_length() {
            if content_length > self.max_response_bytes as u64 {
                return Err(AppError::BadGateway(format!(
                    "Upstream response exceeded limit of {} bytes (received {} bytes)",
                    self.max_response_bytes, content_length
                )));
            }
        }

        let mut body_bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|err| {
            if err.is_timeout() {
                AppError::GatewayTimeout(format!("Upstream response read timed out: {err}"))
            } else {
                AppError::BadGateway(format!("Failed to read upstream response body: {err}"))
            }
        })? {
            if body_bytes.len() + chunk.len() > self.max_response_bytes {
                return Err(AppError::BadGateway(format!(
                    "Upstream response exceeded limit of {} bytes",
                    self.max_response_bytes
                )));
            }
            body_bytes.extend_from_slice(&chunk);
        }

        if !status.is_success() {
            let error_message = match serde_json::from_slice::<serde_json::Value>(&body_bytes) {
                Ok(v) => {
                    if let Some(msg) = v
                        .get("error")
                        .and_then(|e| e.get("message"))
                        .and_then(|m| m.as_str())
                    {
                        msg.to_string()
                    } else if let Some(detail) = v.get("detail").and_then(|d| d.as_str()) {
                        detail.to_string()
                    } else {
                        v.to_string()
                    }
                }
                Err(_) => {
                    let s = String::from_utf8_lossy(&body_bytes);
                    if s.trim().is_empty() {
                        format!("Upstream returned HTTP status {status}")
                    } else {
                        s.chars().take(500).collect()
                    }
                }
            };

            return match status.as_u16() {
                400 => Err(AppError::BadRequest(error_message)),
                404 => Err(AppError::NotFound(error_message)),
                408 | 504 => Err(AppError::GatewayTimeout(error_message)),
                _ => Err(AppError::BadGateway(format!(
                    "Upstream error ({status}): {error_message}"
                ))),
            };
        }

        let parsed: ChatCompletionResponse =
            serde_json::from_slice(&body_bytes).map_err(|err| {
                AppError::BadGateway(format!(
                    "Failed to parse upstream response as OpenAI chat completion: {err}"
                ))
            })?;

        Ok(parsed)
    }

    async fn stream_chat_completion(
        &self,
        request: &ChatCompletionRequest,
    ) -> Result<BoxChatStream, AppError> {
        let base_url = self
            .base_url
            .as_deref()
            .ok_or_else(|| AppError::Internal("Upstream base URL is not configured".to_string()))?;

        let url = resolve_chat_completions_url(base_url);

        let mut upstream_req = request.clone();
        // Strip provider prefix for external upstreams only (not ag/ or cx/).
        {
            let m = &upstream_req.model;
            if !m.starts_with("ag/") && !m.starts_with("cx/") {
                if let Some(pos) = m.find('/') {
                    upstream_req.model = m[pos + 1..].to_string();
                }
            }
        }
        upstream_req.stream = Some(true);

        tracing::info!(
            upstream_url = %url,
            model = %upstream_req.model,
            messages_count = upstream_req.messages.len(),
            has_tools = upstream_req.tools.is_some(),
            stream = true,
            "Dispatching streaming request to upstream provider"
        );

        let mut req_builder = self.client.post(&url).json(&upstream_req);

        if let Some(ref key) = self.api_key {
            if !key.trim().is_empty() {
                req_builder =
                    req_builder.header(reqwest::header::AUTHORIZATION, format!("Bearer {key}"));
            }
        }

        let response = match req_builder.send().await {
            Ok(resp) => resp,
            Err(err) => {
                if err.is_timeout() {
                    return Err(AppError::GatewayTimeout(format!(
                        "Upstream request timed out: {err}"
                    )));
                }
                if err.is_connect() {
                    return Err(AppError::BadGateway(format!(
                        "Upstream connection failed: {err}"
                    )));
                }
                return Err(AppError::BadGateway(format!(
                    "Upstream request failed: {err}"
                )));
            }
        };

        let status = response.status();
        if !status.is_success() {
            let mut body_bytes = Vec::new();
            let mut response = response;
            while let Some(chunk) = response.chunk().await.map_err(|err| {
                if err.is_timeout() {
                    AppError::GatewayTimeout(format!("Upstream response read timed out: {err}"))
                } else {
                    AppError::BadGateway(format!("Failed to read upstream response body: {err}"))
                }
            })? {
                if body_bytes.len() + chunk.len() > self.max_response_bytes {
                    return Err(AppError::BadGateway(format!(
                        "Upstream response exceeded limit of {} bytes",
                        self.max_response_bytes
                    )));
                }
                body_bytes.extend_from_slice(&chunk);
            }

            let error_message = match serde_json::from_slice::<serde_json::Value>(&body_bytes) {
                Ok(v) => {
                    if let Some(msg) = v
                        .get("error")
                        .and_then(|e| e.get("message"))
                        .and_then(|m| m.as_str())
                    {
                        msg.to_string()
                    } else if let Some(detail) = v.get("detail").and_then(|d| d.as_str()) {
                        detail.to_string()
                    } else {
                        v.to_string()
                    }
                }
                Err(_) => {
                    let s = String::from_utf8_lossy(&body_bytes);
                    if s.trim().is_empty() {
                        format!("Upstream returned HTTP status {status}")
                    } else {
                        s.chars().take(500).collect()
                    }
                }
            };

            return match status.as_u16() {
                400 => Err(AppError::BadRequest(error_message)),
                404 => Err(AppError::NotFound(error_message)),
                408 | 504 => Err(AppError::GatewayTimeout(error_message)),
                _ => Err(AppError::BadGateway(format!(
                    "Upstream error ({status}): {error_message}"
                ))),
            };
        }

        let stream = response.bytes_stream().map(|chunk_result| {
            chunk_result.map_err(|err| {
                if err.is_timeout() {
                    AppError::GatewayTimeout(format!("Upstream stream read timed out: {err}"))
                } else {
                    AppError::BadGateway(format!("Upstream stream error: {err}"))
                }
            })
        });

        Ok(Box::pin(stream))
    }
}

#[derive(Debug, Default)]
pub struct MockProvider {
    pub recorded_requests: Mutex<Vec<ChatCompletionRequest>>,
}

impl MockProvider {
    pub fn new() -> Self {
        Self {
            recorded_requests: Mutex::new(Vec::new()),
        }
    }

    pub fn last_request(&self) -> Option<ChatCompletionRequest> {
        self.recorded_requests.lock().unwrap().last().cloned()
    }
}

#[axum::async_trait]
impl Provider for MockProvider {
    async fn complete(
        &self,
        request: &ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, AppError> {
        if let Ok(mut lock) = self.recorded_requests.lock() {
            lock.push(request.clone());
        }

        let prompt_chars: usize = request
            .messages
            .iter()
            .map(|m| m.content_text().len())
            .sum();
        let prompt_tokens = (prompt_chars as u32 / 4).max(1);

        let has_tool_msg = request.messages.iter().any(|m| m.role == "tool");
        let has_stock_tool_request = request.tools.is_some()
            && request.messages.iter().any(|m| {
                m.content_text().contains("stock") || m.content_text().contains("get_stock_price")
            });

        if has_stock_tool_request && !has_tool_msg {
            let func_name = request
                .tools
                .as_ref()
                .and_then(|t| t.as_array())
                .and_then(|a| a.first())
                .and_then(|f| f.get("function"))
                .and_then(|f| f.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or("get_stock_price");
            let call_id = format!("call_{}:thought_mock_sig", uuid::Uuid::new_v4().simple());
            return Ok(ChatCompletionResponse {
                id: "chatcmpl-mock-tool".to_string(),
                object: "chat.completion".to_string(),
                created: 1700000000,
                model: request.model.clone(),
                choices: vec![ChatChoice {
                    index: 0,
                    message: ChatChoiceMessage {
                        role: "assistant".to_string(),
                        content: None,
                        tool_calls: Some(serde_json::json!([{
                            "id": call_id,
                            "type": "function",
                            "thought_signature": "thought_mock_sig",
                            "function": {
                                "name": func_name,
                                "arguments": "{\"symbol\":\"AAPL\"}"
                            }
                        }])),
                    },
                    finish_reason: "tool_calls".to_string(),
                }],
                usage: UsageInfo {
                    prompt_tokens,
                    completion_tokens: 15,
                    total_tokens: prompt_tokens + 15,
                },
            });
        }

        let content = if has_tool_msg {
            "The current stock price of Apple Inc. (AAPL) is $225.50 USD.".to_string()
        } else if let Some(ref tools) = request.tools {
            let tool_count = tools.as_array().map(|a| a.len()).unwrap_or(0);
            format!(
                "Deterministic mock completion for {} (received {} tools)",
                request.model, tool_count
            )
        } else {
            format!("Deterministic mock completion for {}", request.model)
        };

        let completion_tokens = (content.len() as u32 / 4).max(1);
        let total_tokens = prompt_tokens + completion_tokens;

        Ok(ChatCompletionResponse {
            id: "chatcmpl-mock-deterministic".to_string(),
            object: "chat.completion".to_string(),
            created: 1700000000,
            model: request.model.clone(),
            choices: vec![ChatChoice {
                index: 0,
                message: ChatChoiceMessage {
                    role: "assistant".to_string(),
                    content: Some(content),
                    tool_calls: None,
                },
                finish_reason: "stop".to_string(),
            }],
            usage: UsageInfo {
                prompt_tokens,
                completion_tokens,
                total_tokens,
            },
        })
    }

    async fn stream_chat_completion(
        &self,
        request: &ChatCompletionRequest,
    ) -> Result<BoxChatStream, AppError> {
        if let Ok(mut lock) = self.recorded_requests.lock() {
            lock.push(request.clone());
        }

        let (chunk1_text, chunk2_text) = if let Some(ref tools) = request.tools {
            let tool_count = tools.as_array().map(|a| a.len()).unwrap_or(0);
            (
                "Deterministic mock streaming completion ".to_string(),
                format!("for {} (received {} tools)", request.model, tool_count),
            )
        } else {
            (
                "Deterministic mock streaming completion ".to_string(),
                format!("for {}", request.model),
            )
        };

        let prompt_chars: usize = request
            .messages
            .iter()
            .map(|m| m.content_text().len())
            .sum();
        let prompt_tokens = (prompt_chars as u32 / 4).max(1);
        let completion_tokens = ((chunk1_text.len() + chunk2_text.len()) as u32 / 4).max(1);

        let chunk1 = ChatCompletionChunk {
            id: "chatcmpl-mock-streaming".to_string(),
            object: "chat.completion.chunk".to_string(),
            created: 1700000000,
            model: request.model.clone(),
            choices: vec![ChatChunkChoice {
                index: 0,
                delta: ChatChunkDelta {
                    role: Some("assistant".to_string()),
                    content: Some(chunk1_text),
                    tool_calls: None,
                },
                finish_reason: None,
            }],
            usage: None,
        };

        let chunk2 = ChatCompletionChunk {
            id: "chatcmpl-mock-streaming".to_string(),
            object: "chat.completion.chunk".to_string(),
            created: 1700000000,
            model: request.model.clone(),
            choices: vec![ChatChunkChoice {
                index: 0,
                delta: ChatChunkDelta {
                    role: None,
                    content: Some(chunk2_text),
                    tool_calls: None,
                },
                finish_reason: None,
            }],
            usage: None,
        };

        let chunk3 = ChatCompletionChunk {
            id: "chatcmpl-mock-streaming".to_string(),
            object: "chat.completion.chunk".to_string(),
            created: 1700000000,
            model: request.model.clone(),
            choices: vec![ChatChunkChoice {
                index: 0,
                delta: ChatChunkDelta {
                    role: None,
                    content: None,
                    tool_calls: None,
                },
                finish_reason: Some("stop".to_string()),
            }],
            usage: Some(UsageInfo {
                prompt_tokens,
                completion_tokens,
                total_tokens: prompt_tokens + completion_tokens,
            }),
        };

        let sse_events: Vec<Result<Bytes, AppError>> = vec![
            Ok(Bytes::from(format!(
                "data: {}\n\n",
                serde_json::to_string(&chunk1).map_err(|e| AppError::Internal(e.to_string()))?
            ))),
            Ok(Bytes::from(format!(
                "data: {}\n\n",
                serde_json::to_string(&chunk2).map_err(|e| AppError::Internal(e.to_string()))?
            ))),
            Ok(Bytes::from(format!(
                "data: {}\n\n",
                serde_json::to_string(&chunk3).map_err(|e| AppError::Internal(e.to_string()))?
            ))),
            Ok(Bytes::from("data: [DONE]\n\n")),
        ];

        Ok(Box::pin(futures_util::stream::iter(sse_events)))
    }
}

fn push_gemini_parts(
    contents: &mut Vec<serde_json::Value>,
    role: &str,
    parts: Vec<serde_json::Value>,
) {
    if parts.is_empty() {
        return;
    }
    if let Some(last) = contents.last_mut() {
        if last.get("role").and_then(|r| r.as_str()) == Some(role) {
            if let Some(existing) = last.get_mut("parts").and_then(|p| p.as_array_mut()) {
                existing.extend(parts);
                return;
            }
        }
    }
    contents.push(serde_json::json!({
        "role": role,
        "parts": parts,
    }));
}

/// Maps client model identifiers (canonical or aliases) to verified upstream Google Cloud Code models.
/// Ensures raw unsupported identifiers like `gemini-3.1-pro` are never forwarded upstream.
pub fn map_antigravity_upstream_model(model: &str) -> &str {
    let clean = model.strip_prefix("ag/").unwrap_or(model);
    match clean {
        "gemini-3.1-pro" | "gemini-3.1-pro-high" | "gemini-pro-agent" => "gemini-pro-agent",
        "gemini-3.1-pro-low" | "gemini-pro-low" => "gemini-3.1-pro-low",
        other => other,
    }
}

/// Normalizes OpenAI/legacy JSON schemas into strict JSON Schema Draft 2020-12 compliance.
/// Specifically required when routing tools to Claude behind Google Antigravity / Vertex AI.
pub fn normalize_tool_schema(schema: &serde_json::Value) -> serde_json::Value {
    fn clean_node(node: &serde_json::Value) -> serde_json::Value {
        match node {
            serde_json::Value::Bool(b) => serde_json::Value::Bool(*b),
            serde_json::Value::Object(map) => {
                let mut result = serde_json::Map::new();

                for (key, value) in map {
                    // Strip unsupported / meta schema fields
                    if matches!(
                        key.as_str(),
                        "$schema"
                            | "definitions"
                            | "$defs"
                            | "dependencies"
                            | "dependentSchemas"
                            | "dependentRequired"
                    ) {
                        continue;
                    }

                    // Preserve boolean or object additionalProperties as-is
                    if key == "additionalProperties" {
                        if value.is_boolean() {
                            result.insert(key.clone(), value.clone());
                        } else if value.is_object() {
                            result.insert(key.clone(), clean_node(value));
                        }
                        continue;
                    }

                    if key == "properties" {
                        if let Some(props) = value.as_object() {
                            let mut clean_props = serde_json::Map::new();
                            for (name, child) in props {
                                clean_props.insert(name.clone(), clean_node(child));
                            }
                            result.insert(
                                "properties".to_string(),
                                serde_json::Value::Object(clean_props),
                            );
                        } else {
                            result.insert("properties".to_string(), serde_json::json!({}));
                        }
                        continue;
                    }

                    if matches!(key.as_str(), "allOf" | "anyOf" | "oneOf" | "prefixItems") {
                        if let Some(arr) = value.as_array() {
                            if !arr.is_empty() {
                                let branch = arr
                                    .iter()
                                    .find(|child| {
                                        !child.is_object()
                                            || child.get("type").and_then(|t| t.as_str())
                                                != Some("null")
                                    })
                                    .unwrap_or(&arr[0]);
                                let walked = clean_node(branch);
                                if let Some(m) = walked.as_object() {
                                    for (k, v) in m {
                                        result.insert(k.clone(), v.clone());
                                    }
                                }
                            }
                        }
                        continue;
                    }

                    if key == "items" {
                        if let Some(arr) = value.as_array() {
                            if !arr.is_empty() {
                                result.insert("items".to_string(), clean_node(&arr[0]));
                            } else {
                                result.insert("items".to_string(), serde_json::json!({}));
                            }
                        } else if value.is_object() || value.is_boolean() {
                            result.insert("items".to_string(), clean_node(value));
                        } else {
                            result.insert("items".to_string(), serde_json::json!({}));
                        }
                        continue;
                    }

                    if key == "required" {
                        if let Some(arr) = value.as_array() {
                            let clean_req: Vec<serde_json::Value> = arr
                                .iter()
                                .filter_map(|item| {
                                    item.as_str()
                                        .map(|s| serde_json::Value::String(s.to_string()))
                                })
                                .collect();
                            if !clean_req.is_empty() {
                                result.insert(
                                    "required".to_string(),
                                    serde_json::Value::Array(clean_req),
                                );
                            }
                        }
                        continue;
                    }

                    if key == "type" {
                        if let Some(arr) = value.as_array() {
                            let types: Vec<&str> = arr
                                .iter()
                                .filter_map(|t| t.as_str())
                                .filter(|t| !t.eq_ignore_ascii_case("null"))
                                .collect();
                            if !types.is_empty() {
                                result.insert(
                                    "type".to_string(),
                                    serde_json::Value::String(types[0].to_lowercase()),
                                );
                            }
                            continue;
                        } else if let Some(s) = value.as_str() {
                            result.insert(
                                "type".to_string(),
                                serde_json::Value::String(s.to_lowercase()),
                            );
                            continue;
                        }
                    }

                    if key == "not" {
                        if value.is_object() || value.is_boolean() {
                            result.insert(key.clone(), clean_node(value));
                        }
                        continue;
                    }

                    result.insert(key.clone(), value.clone());
                }

                // If type is object, enforce properties and validate required
                let is_obj = result.get("type").and_then(|t| t.as_str()) == Some("object")
                    || result.contains_key("properties");
                if is_obj {
                    result
                        .entry("type".to_string())
                        .or_insert_with(|| serde_json::json!("object"));
                    let props_map = result
                        .entry("properties".to_string())
                        .or_insert_with(|| serde_json::json!({}))
                        .as_object()
                        .cloned()
                        .unwrap_or_default();

                    if let Some(req_val) = result.get("required").cloned() {
                        if let Some(req_arr) = req_val.as_array() {
                            let filtered: Vec<serde_json::Value> = req_arr
                                .iter()
                                .filter(|item| {
                                    item.as_str()
                                        .map(|s| props_map.contains_key(s))
                                        .unwrap_or(false)
                                })
                                .cloned()
                                .collect();
                            if filtered.is_empty() {
                                result.remove("required");
                            } else {
                                result.insert(
                                    "required".to_string(),
                                    serde_json::Value::Array(filtered),
                                );
                            }
                        } else {
                            result.remove("required");
                        }
                    }
                }

                // If type is array, ensure items is present
                if result.get("type").and_then(|t| t.as_str()) == Some("array")
                    && !result.contains_key("items")
                {
                    result.insert("items".to_string(), serde_json::json!({}));
                }

                serde_json::Value::Object(result)
            }
            _ => serde_json::json!({}),
        }
    }

    let mut normalized = clean_node(schema);
    if normalized.get("type").and_then(|t| t.as_str()) != Some("object") {
        normalized = serde_json::json!({
            "type": "object",
            "properties": {
                "value": normalized
            }
        });
    }

    if let Some(obj) = normalized.as_object_mut() {
        if !obj.contains_key("properties") || !obj["properties"].is_object() {
            obj.insert("properties".to_string(), serde_json::json!({}));
        }
    }

    normalized
}

/// Extracts pseudo text tool calls like `[Tool Call: name(args_json)]` from text,
/// returning cleaned text and a list of OpenAI-formatted tool_calls.
pub fn extract_text_tool_calls(text: &str) -> (String, Vec<serde_json::Value>) {
    let mut tool_calls = Vec::new();
    let marker = "[tool call:";
    let lower = text.to_lowercase();
    let mut last_end = 0;
    let mut cleaned = String::new();
    let mut search_idx = 0;

    while let Some(idx) = lower[search_idx..].find(marker) {
        let abs_idx = search_idx + idx;
        let mut p = abs_idx + marker.len();
        let bytes = text.as_bytes();

        // Skip whitespace after marker
        while p < bytes.len() && (bytes[p] as char).is_whitespace() {
            p += 1;
        }

        // Extract function name
        let name_start = p;
        while p < bytes.len() {
            let c = bytes[p] as char;
            if c.is_alphanumeric() || c == '_' || c == '-' {
                p += 1;
            } else {
                break;
            }
        }
        let name = text[name_start..p].trim().to_string();

        // Skip whitespace
        while p < bytes.len() && (bytes[p] as char).is_whitespace() {
            p += 1;
        }

        // Expect '('
        if p >= bytes.len() || bytes[p] != b'(' || name.is_empty() {
            search_idx = p.max(abs_idx + 1);
            continue;
        }
        p += 1; // skip '('

        // Skip whitespace
        while p < bytes.len() && (bytes[p] as char).is_whitespace() {
            p += 1;
        }

        // Look for JSON payload: either object '{' or array '[' or empty ')'
        let mut json_end = None;
        if p < bytes.len() && bytes[p] == b'{' {
            let json_start = p;
            let mut depth = 0;
            let mut in_str = false;
            let mut escape = false;
            while p < bytes.len() {
                let b = bytes[p];
                if escape {
                    escape = false;
                } else if b == b'\\' {
                    escape = true;
                } else if b == b'"' {
                    in_str = !in_str;
                } else if !in_str {
                    if b == b'{' {
                        depth += 1;
                    } else if b == b'}' {
                        depth -= 1;
                        if depth == 0 {
                            json_end = Some((json_start, p + 1));
                            p += 1;
                            break;
                        }
                    }
                }
                p += 1;
            }
        } else if p < bytes.len() && bytes[p] == b')' {
            json_end = Some((p, p));
        }

        if let Some((j_start, j_end)) = json_end {
            let raw_args = if j_start == j_end {
                "{}".to_string()
            } else {
                text[j_start..j_end].to_string()
            };

            // Skip whitespace
            while p < bytes.len() && (bytes[p] as char).is_whitespace() {
                p += 1;
            }

            // Expect ')'
            if p < bytes.len() && bytes[p] == b')' {
                p += 1;
                while p < bytes.len() && (bytes[p] as char).is_whitespace() {
                    p += 1;
                }
                // Expect ']'
                if p < bytes.len() && bytes[p] == b']' {
                    p += 1;
                    let parsed_json = if raw_args == "{}" {
                        Some(serde_json::json!({}))
                    } else {
                        serde_json::from_str::<serde_json::Value>(&raw_args).ok()
                    };

                    if let Some(json_val) = parsed_json {
                        let args_str = serde_json::to_string(&json_val).unwrap_or(raw_args);
                        let call_id = format!("call_{}", uuid::Uuid::new_v4().simple());
                        tool_calls.push(serde_json::json!({
                            "id": call_id,
                            "type": "function",
                            "function": {
                                "name": name,
                                "arguments": args_str
                            }
                        }));

                        cleaned.push_str(&text[last_end..abs_idx]);
                        last_end = p;
                        search_idx = p;
                        continue;
                    }
                }
            }
        }

        search_idx = abs_idx + marker.len();
    }

    cleaned.push_str(&text[last_end..]);
    let trimmed_cleaned = cleaned.trim().to_string();
    (trimmed_cleaned, tool_calls)
}

pub fn build_antigravity_payload(request: &ChatCompletionRequest) -> serde_json::Value {
    let upstream_model = map_antigravity_upstream_model(&request.model);
    let is_claude = upstream_model.to_lowercase().contains("claude");

    let mut contents: Vec<serde_json::Value> = Vec::new();
    let mut system_parts: Vec<String> = Vec::new();

    #[allow(dead_code)]
    #[derive(Clone)]
    struct ToolCallMeta {
        name: String,
        stable_id: String,
        pure_id: String,
        thought_signature: Option<String>,
        degraded: bool,
    }

    let mut tool_meta_by_id: std::collections::HashMap<String, ToolCallMeta> =
        std::collections::HashMap::new();
    let mut tool_meta_by_name: std::collections::HashMap<String, ToolCallMeta> =
        std::collections::HashMap::new();
    let mut unconsumed_meta: std::collections::VecDeque<ToolCallMeta> =
        std::collections::VecDeque::new();
    let mut any_degraded = false;

    for (msg_idx, msg) in request.messages.iter().enumerate() {
        if msg.role == "assistant" {
            if let Some(arr) = msg.tool_calls.as_ref().and_then(|v| v.as_array()) {
                for (tc_idx, tc) in arr.iter().enumerate() {
                    let raw_id = tc
                        .get("id")
                        .or_else(|| tc.get("call_id"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .trim();
                    let pure_raw = raw_id.split_once(':').map(|(id, _)| id).unwrap_or(raw_id);
                    let stable_id = if !pure_raw.is_empty() {
                        pure_raw.to_string()
                    } else {
                        format!("call_{}_{}", msg_idx, tc_idx)
                    };

                    let name = tc
                        .get("function")
                        .and_then(|f| f.get("name"))
                        .or_else(|| tc.get("name"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .trim()
                        .to_string();

                    let sig = tc
                        .get("thought_signature")
                        .or_else(|| tc.get("thoughtSignature"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .or_else(|| raw_id.split_once(':').map(|(_, s)| s.to_string()));

                    let degraded = !is_claude && sig.is_none();
                    if degraded {
                        any_degraded = true;
                    }

                    let meta = ToolCallMeta {
                        name: name.clone(),
                        stable_id: stable_id.clone(),
                        pure_id: stable_id.clone(),
                        thought_signature: sig,
                        degraded,
                    };

                    if !raw_id.is_empty() {
                        tool_meta_by_id.insert(raw_id.to_string(), meta.clone());
                    }
                    tool_meta_by_id.insert(stable_id, meta.clone());
                    if !name.is_empty() {
                        tool_meta_by_name.insert(name, meta.clone());
                    }
                    unconsumed_meta.push_back(meta);
                }
            }
        }
    }

    for (msg_idx, msg) in request.messages.iter().enumerate() {
        if msg.role == "system" {
            let text = msg.content_text();
            if !text.is_empty() {
                system_parts.push(text.to_string());
            }
        } else if msg.role == "tool" {
            let raw_tid = msg
                .tool_call_id
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty());
            let meta_match = raw_tid
                .and_then(|tid| tool_meta_by_id.get(tid))
                .or_else(|| {
                    msg.name
                        .as_deref()
                        .and_then(|n| tool_meta_by_name.get(n.trim()))
                })
                .cloned()
                .or_else(|| unconsumed_meta.pop_front());

            let actual_name = meta_match
                .as_ref()
                .map(|m| m.name.as_str())
                .or(msg.name.as_deref())
                .unwrap_or("tool_call");

            let pure_id = meta_match
                .as_ref()
                .map(|m| m.pure_id.clone())
                .unwrap_or_else(|| {
                    let tid = msg.tool_call_id.as_deref().unwrap_or("tool_call");
                    tid.split_once(':')
                        .map(|(id, _)| id)
                        .unwrap_or(tid)
                        .to_string()
                });

            let is_degraded = meta_match.as_ref().map_or(!is_claude, |m| m.degraded);
            if is_degraded {
                any_degraded = true;
            }
            let content_str = msg.content_text();

            if is_degraded {
                let marker = format!("[Tool Result for {}: {}]", actual_name, content_str);
                push_gemini_parts(
                    &mut contents,
                    "user",
                    vec![serde_json::json!({ "text": marker })],
                );
            } else {
                let content_json = serde_json::from_str::<serde_json::Value>(&content_str)
                    .unwrap_or_else(|_| serde_json::json!({ "output": content_str.as_ref() }));
                let content_json = if content_json.is_object() {
                    content_json
                } else {
                    serde_json::json!({ "output": content_json })
                };

                let mut fr_obj = serde_json::json!({
                    "name": actual_name,
                    "response": content_json,
                });
                if !pure_id.is_empty() {
                    fr_obj["id"] = serde_json::json!(pure_id);
                }

                let part = serde_json::json!({
                    "functionResponse": fr_obj,
                });
                push_gemini_parts(&mut contents, "user", vec![part]);
            }
        } else if msg.role == "assistant" && msg.tool_calls.is_some() {
            let mut parts = Vec::new();
            let text = msg.content_text();
            if !text.is_empty() {
                parts.push(serde_json::json!({ "text": text.as_ref() }));
            }
            if let Some(ref tc_val) = msg.tool_calls {
                if let Some(tc_arr) = tc_val.as_array() {
                    for (tc_idx, tc) in tc_arr.iter().enumerate() {
                        let raw_id = tc
                            .get("id")
                            .or_else(|| tc.get("call_id"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .trim();
                        let pure_id = raw_id.split_once(':').map(|(id, _)| id).unwrap_or(raw_id);
                        let stable_id = if !pure_id.is_empty() {
                            pure_id.to_string()
                        } else {
                            format!("call_{}_{}", msg_idx, tc_idx)
                        };

                        let (name, args_val, args_str) = if let Some(func) = tc.get("function") {
                            let n = func
                                .get("name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let (v, s) = if let Some(s) =
                                func.get("arguments").and_then(|v| v.as_str())
                            {
                                (
                                    serde_json::from_str(s)
                                        .unwrap_or_else(|_| serde_json::json!({})),
                                    s.to_string(),
                                )
                            } else if let Some(obj) = func.get("arguments") {
                                (
                                    obj.clone(),
                                    serde_json::to_string(obj).unwrap_or_else(|_| "{}".to_string()),
                                )
                            } else {
                                (serde_json::json!({}), "{}".to_string())
                            };
                            (n, v, s)
                        } else {
                            let n = tc
                                .get("name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let s = tc
                                .get("arguments")
                                .and_then(|v| v.as_str())
                                .unwrap_or("{}")
                                .to_string();
                            let v =
                                serde_json::from_str(&s).unwrap_or_else(|_| serde_json::json!({}));
                            (n, v, s)
                        };

                        let sig = tc
                            .get("thought_signature")
                            .or_else(|| tc.get("thoughtSignature"))
                            .and_then(|v| v.as_str())
                            .or_else(|| raw_id.split_once(':').map(|(_, s)| s));

                        let degraded = !is_claude && sig.is_none();
                        if degraded {
                            let marker = format!("[Tool Call: {}({})]", name, args_str);
                            parts.push(serde_json::json!({ "text": marker }));
                        } else {
                            let mut fc_obj = serde_json::json!({
                                "name": name,
                                "args": args_val,
                            });
                            if !stable_id.is_empty() {
                                fc_obj["id"] = serde_json::json!(stable_id);
                            }
                            let mut part_obj = serde_json::json!({
                                "functionCall": fc_obj,
                            });
                            if let Some(s) = sig {
                                part_obj["thoughtSignature"] = serde_json::json!(s);
                            }
                            parts.push(part_obj);
                        }
                    }
                }
            }
            if parts.is_empty() {
                parts.push(serde_json::json!({ "text": "" }));
            }
            push_gemini_parts(&mut contents, "model", parts);
        } else {
            let role = if msg.role == "assistant" {
                "model"
            } else {
                "user"
            };
            let mut parts = Vec::new();
            if let Some(MessageContent::Parts(user_parts)) = &msg.content {
                for p in user_parts {
                    if let Some(t) = p.get("text").and_then(|v| v.as_str()) {
                        parts.push(serde_json::json!({ "text": t }));
                    } else if let Some(t) = p.as_str() {
                        parts.push(serde_json::json!({ "text": t }));
                    } else if p.get("functionCall").is_some()
                        || p.get("functionResponse").is_some()
                        || p.get("inlineData").is_some()
                    {
                        parts.push(p.clone());
                    }
                }
            }
            if parts.is_empty() {
                let text = msg.content_text();
                parts.push(serde_json::json!({ "text": text.as_ref() }));
            }
            push_gemini_parts(&mut contents, role, parts);
        }
    }

    let has_text_tool_markers = any_degraded
        || request.messages.iter().any(|m| {
            let t = m.content_text();
            t.to_lowercase().contains("[tool call:")
        });

    if has_text_tool_markers {
        system_parts.push(
            "CRITICAL INSTRUCTION FOR TOOL USE:\n\
             Historical tool calls or tool results in the conversation history rendered as text markers (such as `[Tool Call: name(args)]` or `[Tool Result for name: result]`) are archival logs.\n\
             You MUST NOT output text markers like `[Tool Call: ...]` yourself.\n\
             To invoke any tool, you MUST emit native structured function calls (tool_calls) according to the API tools schema."
                .to_string(),
        );
    }

    let system_instruction = if !system_parts.is_empty() {
        Some(serde_json::json!({
            "parts": [{ "text": system_parts.join("\n\n") }]
        }))
    } else {
        None
    };

    let mut gen_config = serde_json::json!({
        "temperature": request.temperature.unwrap_or(1.0),
        "topP": request.top_p.unwrap_or(0.95),
        "maxOutputTokens": request.effective_max_tokens().unwrap_or(if is_claude { 4096 } else { 65536 }),
    });
    if !is_claude {
        gen_config["topK"] = serde_json::json!(40);
    }

    let mut req_obj = serde_json::json!({
        "contents": contents,
        "generationConfig": gen_config,
    });

    if let Some(sys) = system_instruction {
        req_obj["systemInstruction"] = sys;
    }

    if let Some(ref tools) = request.tools {
        if let Some(arr) = tools.as_array() {
            let mut function_declarations = Vec::new();
            for item in arr {
                if item.get("type").and_then(|v| v.as_str()) == Some("function") {
                    if let Some(func) = item.get("function") {
                        let name = func.get("name").and_then(|v| v.as_str()).unwrap_or("");
                        let desc = func
                            .get("description")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let raw_params = func.get("parameters").cloned().unwrap_or_else(
                            || serde_json::json!({"type": "object", "properties": {}}),
                        );
                        let params = normalize_tool_schema(&raw_params);
                        function_declarations.push(serde_json::json!({
                            "name": name,
                            "description": desc,
                            "parameters": params,
                        }));
                    }
                }
            }
            if !function_declarations.is_empty() {
                req_obj["tools"] = serde_json::json!([{
                    "functionDeclarations": function_declarations
                }]);
            }
        }
    }

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let request_id = format!(
        "agent/{}/{now_ms}/{}/1",
        uuid::Uuid::new_v4(),
        uuid::Uuid::new_v4()
    );

    serde_json::json!({
        "project": AG_PROJECT,
        "model": upstream_model,
        "userAgent": "antigravity",
        "requestType": "agent",
        "requestId": request_id,
        "request": req_obj,
    })
}

#[allow(clippy::too_many_arguments)]
fn process_sse_line(
    line: &[u8],
    chat_id: &str,
    model: &str,
    created: i64,
    sent_role: &mut bool,
    sent_tool_call: &mut bool,
    tool_call_index: &mut usize,
    accumulated_text: &mut String,
) -> Vec<Result<Bytes, AppError>> {
    let mut out = Vec::new();
    let text = match std::str::from_utf8(line) {
        Ok(t) => t.trim(),
        Err(_) => return out,
    };
    if !text.starts_with("data:") {
        return out;
    }
    let data_str = text.trim_start_matches("data:").trim();
    if data_str.is_empty() {
        return out;
    }
    if data_str == "[DONE]" {
        out.push(Ok(Bytes::from("data: [DONE]\n\n")));
        return out;
    }

    if let Ok(val) = serde_json::from_str::<serde_json::Value>(data_str) {
        if val.get("choices").and_then(|c| c.as_array()).is_some() {
            out.push(Ok(Bytes::from(format!("data: {data_str}\n\n"))));
            return out;
        }

        let resp_obj = val.get("response").unwrap_or(&val);
        let usage = resp_obj.get("usageMetadata").map(|u| UsageInfo {
            prompt_tokens: u
                .get("promptTokenCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            completion_tokens: u
                .get("candidatesTokenCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            total_tokens: (u
                .get("promptTokenCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0)
                + u.get("candidatesTokenCount")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0)) as u32,
        });

        if let Some(candidates) = resp_obj.get("candidates").and_then(|c| c.as_array()) {
            for cand in candidates {
                if let Some(parts) = cand
                    .get("content")
                    .and_then(|c| c.get("parts"))
                    .and_then(|p| p.as_array())
                {
                    for p in parts {
                        if let Some(part_text) = p.get("text").and_then(|t| t.as_str()) {
                            accumulated_text.push_str(part_text);
                            let role = if !*sent_role {
                                *sent_role = true;
                                Some("assistant".to_string())
                            } else {
                                None
                            };
                            let chunk = ChatCompletionChunk {
                                id: chat_id.to_string(),
                                object: "chat.completion.chunk".to_string(),
                                created,
                                model: model.to_string(),
                                choices: vec![ChatChunkChoice {
                                    index: 0,
                                    delta: ChatChunkDelta {
                                        role,
                                        content: Some(part_text.to_string()),
                                        tool_calls: None,
                                    },
                                    finish_reason: None,
                                }],
                                usage: None,
                            };
                            if let Ok(json_str) = serde_json::to_string(&chunk) {
                                out.push(Ok(Bytes::from(format!("data: {json_str}\n\n"))));
                            }
                        }
                        if let Some(fc) = p.get("functionCall") {
                            *sent_tool_call = true;
                            let role = if !*sent_role {
                                *sent_role = true;
                                Some("assistant".to_string())
                            } else {
                                None
                            };
                            let name = fc.get("name").and_then(|v| v.as_str()).unwrap_or("");
                            let args = fc
                                .get("args")
                                .cloned()
                                .unwrap_or_else(|| serde_json::json!({}));
                            let args_str =
                                serde_json::to_string(&args).unwrap_or_else(|_| "{}".to_string());
                            let sig = p
                                .get("thoughtSignature")
                                .or_else(|| p.get("thought_signature"))
                                .and_then(|v| v.as_str());
                            let call_id = if let Some(s) = sig {
                                format!("call_{}:{}", uuid::Uuid::new_v4().simple(), s)
                            } else {
                                format!("call_{}", uuid::Uuid::new_v4().simple())
                            };
                            let mut tc_delta = serde_json::json!({
                                "index": *tool_call_index,
                                "id": call_id,
                                "type": "function",
                                "function": {
                                    "name": name,
                                    "arguments": args_str
                                }
                            });
                            *tool_call_index += 1;
                            if let Some(s) = sig {
                                tc_delta["thought_signature"] = serde_json::json!(s);
                            }
                            let chunk = ChatCompletionChunk {
                                id: chat_id.to_string(),
                                object: "chat.completion.chunk".to_string(),
                                created,
                                model: model.to_string(),
                                choices: vec![ChatChunkChoice {
                                    index: 0,
                                    delta: ChatChunkDelta {
                                        role,
                                        content: None,
                                        tool_calls: Some(serde_json::json!([tc_delta])),
                                    },
                                    finish_reason: None,
                                }],
                                usage: None,
                            };
                            if let Ok(json_str) = serde_json::to_string(&chunk) {
                                out.push(Ok(Bytes::from(format!("data: {json_str}\n\n"))));
                            }
                        }
                    }
                }
                if let Some(fr) = cand.get("finishReason").and_then(|f| f.as_str()) {
                    if !*sent_tool_call {
                        let (_, recovered) = extract_text_tool_calls(accumulated_text);
                        if !recovered.is_empty() {
                            *sent_tool_call = true;
                            for mut tc in recovered {
                                tc["index"] = serde_json::json!(*tool_call_index);
                                *tool_call_index += 1;
                                let chunk = ChatCompletionChunk {
                                    id: chat_id.to_string(),
                                    object: "chat.completion.chunk".to_string(),
                                    created,
                                    model: model.to_string(),
                                    choices: vec![ChatChunkChoice {
                                        index: 0,
                                        delta: ChatChunkDelta {
                                            role: None,
                                            content: None,
                                            tool_calls: Some(serde_json::json!([tc])),
                                        },
                                        finish_reason: None,
                                    }],
                                    usage: None,
                                };
                                if let Ok(json_str) = serde_json::to_string(&chunk) {
                                    out.push(Ok(Bytes::from(format!("data: {json_str}\n\n"))));
                                }
                            }
                        }
                    }
                    let oai_reason = match fr {
                        "MAX_TOKENS" => "length",
                        _ if *sent_tool_call => "tool_calls",
                        _ => "stop",
                    };
                    let chunk = ChatCompletionChunk {
                        id: chat_id.to_string(),
                        object: "chat.completion.chunk".to_string(),
                        created,
                        model: model.to_string(),
                        choices: vec![ChatChunkChoice {
                            index: 0,
                            delta: ChatChunkDelta {
                                role: None,
                                content: None,
                                tool_calls: None,
                            },
                            finish_reason: Some(oai_reason.to_string()),
                        }],
                        usage: usage.clone(),
                    };
                    if let Ok(json_str) = serde_json::to_string(&chunk) {
                        out.push(Ok(Bytes::from(format!("data: {json_str}\n\n"))));
                    }
                }
            }
        }
    }
    out
}

fn parse_antigravity_response_body(
    body_bytes: &[u8],
    model: &str,
    prompt_tokens_fallback: u32,
) -> Result<ChatCompletionResponse, AppError> {
    if let Ok(resp) = serde_json::from_slice::<ChatCompletionResponse>(body_bytes) {
        return Ok(resp);
    }

    if let Ok(val) = serde_json::from_slice::<serde_json::Value>(body_bytes) {
        let resp_obj = val.get("response").unwrap_or(&val);
        if let Some(candidates) = resp_obj.get("candidates").and_then(|c| c.as_array()) {
            let mut full_text = String::new();
            let mut finish_reason = "stop".to_string();
            let mut tool_calls = Vec::new();
            for cand in candidates {
                if let Some(fr) = cand.get("finishReason").and_then(|f| f.as_str()) {
                    if fr == "MAX_TOKENS" {
                        finish_reason = "length".to_string();
                    }
                }
                if let Some(parts) = cand
                    .get("content")
                    .and_then(|c| c.get("parts"))
                    .and_then(|p| p.as_array())
                {
                    for p in parts {
                        if let Some(t) = p.get("text").and_then(|t| t.as_str()) {
                            full_text.push_str(t);
                        }
                        if let Some(fc) = p.get("functionCall") {
                            tracing::info!("Gemini functionCall part: {:?}", p);
                            let name = fc.get("name").and_then(|v| v.as_str()).unwrap_or("");
                            let args = fc
                                .get("args")
                                .cloned()
                                .unwrap_or_else(|| serde_json::json!({}));
                            let args_str =
                                serde_json::to_string(&args).unwrap_or_else(|_| "{}".to_string());
                            let sig = p
                                .get("thoughtSignature")
                                .or_else(|| p.get("thought_signature"))
                                .and_then(|v| v.as_str());
                            let call_id = if let Some(s) = sig {
                                format!("call_{}:{}", uuid::Uuid::new_v4().simple(), s)
                            } else {
                                format!("call_{}", uuid::Uuid::new_v4().simple())
                            };
                            let mut tc_obj = serde_json::json!({
                                "id": call_id,
                                "type": "function",
                                "function": {
                                    "name": name,
                                    "arguments": args_str
                                }
                            });
                            if let Some(s) = sig {
                                tc_obj["thought_signature"] = serde_json::json!(s);
                            }
                            tool_calls.push(tc_obj);
                        }
                    }
                }
            }
            if tool_calls.is_empty() && !full_text.is_empty() {
                let (cleaned, recovered) = extract_text_tool_calls(&full_text);
                if !recovered.is_empty() {
                    tracing::warn!(
                        "Recovered {} tool calls from plain text in Gemini completion",
                        recovered.len()
                    );
                    tool_calls = recovered;
                    full_text = cleaned;
                }
            }
            if !tool_calls.is_empty() {
                finish_reason = "tool_calls".to_string();
            }
            let (prompt_tokens, completion_tokens) = if let Some(usage) =
                resp_obj.get("usageMetadata")
            {
                let pt = usage
                    .get("promptTokenCount")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(prompt_tokens_fallback as u64) as u32;
                let ct = usage
                    .get("candidatesTokenCount")
                    .and_then(|v| v.as_u64())
                    .unwrap_or((full_text.len() as u64 / 4).max(1)) as u32;
                (pt, ct)
            } else {
                (prompt_tokens_fallback, (full_text.len() as u32 / 4).max(1))
            };
            let content_opt = if full_text.is_empty() && !tool_calls.is_empty() {
                None
            } else {
                Some(full_text)
            };
            let tool_calls_opt = if tool_calls.is_empty() {
                None
            } else {
                Some(serde_json::Value::Array(tool_calls))
            };
            return Ok(ChatCompletionResponse {
                id: format!("chatcmpl-{}", uuid::Uuid::new_v4().simple()),
                object: "chat.completion".to_string(),
                created: chrono::Utc::now().timestamp(),
                model: model.to_string(),
                choices: vec![ChatChoice {
                    index: 0,
                    message: ChatChoiceMessage {
                        role: "assistant".to_string(),
                        content: content_opt,
                        tool_calls: tool_calls_opt,
                    },
                    finish_reason,
                }],
                usage: UsageInfo {
                    prompt_tokens,
                    completion_tokens,
                    total_tokens: prompt_tokens + completion_tokens,
                },
            });
        }
    }

    let text = String::from_utf8_lossy(body_bytes);
    let mut full_text = String::new();
    let mut finish_reason = "stop".to_string();
    let mut tool_calls = Vec::new();
    let mut prompt_tokens = 0u32;
    let mut completion_tokens = 0u32;
    let mut found_any = false;

    for line in text.lines() {
        let line = line.trim();
        if !line.starts_with("data:") {
            continue;
        }
        let data_str = line.trim_start_matches("data:").trim();
        if data_str.is_empty() || data_str == "[DONE]" {
            continue;
        }
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(data_str) {
            found_any = true;
            let resp_obj = val.get("response").unwrap_or(&val);
            if let Some(usage) = resp_obj.get("usageMetadata") {
                if let Some(pt) = usage.get("promptTokenCount").and_then(|v| v.as_u64()) {
                    prompt_tokens = prompt_tokens.max(pt as u32);
                }
                if let Some(ct) = usage.get("candidatesTokenCount").and_then(|v| v.as_u64()) {
                    completion_tokens = completion_tokens.max(ct as u32);
                }
            }
            if let Some(candidates) = resp_obj.get("candidates").and_then(|c| c.as_array()) {
                for cand in candidates {
                    if let Some(fr) = cand.get("finishReason").and_then(|f| f.as_str()) {
                        if fr == "MAX_TOKENS" {
                            finish_reason = "length".to_string();
                        }
                    }
                    if let Some(parts) = cand
                        .get("content")
                        .and_then(|c| c.get("parts"))
                        .and_then(|p| p.as_array())
                    {
                        for p in parts {
                            if let Some(t) = p.get("text").and_then(|t| t.as_str()) {
                                full_text.push_str(t);
                            }
                            if let Some(fc) = p.get("functionCall") {
                                tracing::info!("Gemini SSE line functionCall part: {:?}", p);
                                let name = fc.get("name").and_then(|v| v.as_str()).unwrap_or("");
                                let args = fc
                                    .get("args")
                                    .cloned()
                                    .unwrap_or_else(|| serde_json::json!({}));
                                let args_str = serde_json::to_string(&args)
                                    .unwrap_or_else(|_| "{}".to_string());
                                let sig = p
                                    .get("thoughtSignature")
                                    .or_else(|| p.get("thought_signature"))
                                    .and_then(|v| v.as_str());
                                let call_id = if let Some(s) = sig {
                                    format!("call_{}:{}", uuid::Uuid::new_v4().simple(), s)
                                } else {
                                    format!("call_{}", uuid::Uuid::new_v4().simple())
                                };
                                let mut tc_obj = serde_json::json!({
                                    "id": call_id,
                                    "type": "function",
                                    "function": {
                                        "name": name,
                                        "arguments": args_str
                                    }
                                });
                                if let Some(s) = sig {
                                    tc_obj["thought_signature"] = serde_json::json!(s);
                                }
                                tool_calls.push(tc_obj);
                            }
                        }
                    }
                }
            }
        }
    }

    if found_any {
        if !tool_calls.is_empty() {
            finish_reason = "tool_calls".to_string();
        }
        if prompt_tokens == 0 {
            prompt_tokens = prompt_tokens_fallback;
        }
        if completion_tokens == 0 {
            completion_tokens = (full_text.len() as u32 / 4).max(1);
        }
        let content_opt = if full_text.is_empty() && !tool_calls.is_empty() {
            None
        } else {
            Some(full_text)
        };
        let tool_calls_opt = if tool_calls.is_empty() {
            None
        } else {
            Some(serde_json::Value::Array(tool_calls))
        };
        return Ok(ChatCompletionResponse {
            id: format!("chatcmpl-{}", uuid::Uuid::new_v4().simple()),
            object: "chat.completion".to_string(),
            created: chrono::Utc::now().timestamp(),
            model: model.to_string(),
            choices: vec![ChatChoice {
                index: 0,
                message: ChatChoiceMessage {
                    role: "assistant".to_string(),
                    content: content_opt,
                    tool_calls: tool_calls_opt,
                },
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
        "Failed to parse upstream response as OpenAI or Gemini format: {}",
        text.chars().take(200).collect::<String>()
    )))
}

pub struct GeminiSseStream {
    inner: BoxChatStream,
    buffer: Vec<u8>,
    queue: VecDeque<Result<Bytes, AppError>>,
    model: String,
    chat_id: String,
    created: i64,
    sent_first_role: bool,
    sent_tool_call: bool,
    sent_done: bool,
    ended: bool,
    lease: Option<GoogleAccountLease>,
    pub requested_model: Option<String>,
    pub tool_call_index: usize,
    accumulated_text: String,
}

impl GeminiSseStream {
    pub fn new(
        inner: BoxChatStream,
        model: String,
        chat_id: String,
        created: i64,
        lease: Option<GoogleAccountLease>,
    ) -> Self {
        Self {
            inner,
            buffer: Vec::new(),
            queue: VecDeque::new(),
            model,
            chat_id,
            created,
            sent_first_role: false,
            sent_tool_call: false,
            sent_done: false,
            ended: false,
            lease,
            requested_model: None,
            tool_call_index: 0,
            accumulated_text: String::new(),
        }
    }

    pub fn with_requested_model(mut self, m: String) -> Self {
        self.requested_model = Some(m);
        self
    }
}

impl Stream for GeminiSseStream {
    type Item = Result<Bytes, AppError>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let this = self.as_mut().get_mut();

        loop {
            if let Some(item) = this.queue.pop_front() {
                return std::task::Poll::Ready(Some(item));
            }

            if this.ended {
                if !this.sent_done {
                    this.sent_done = true;
                    if !this.sent_tool_call {
                        let (_, recovered) = extract_text_tool_calls(&this.accumulated_text);
                        if !recovered.is_empty() {
                            this.sent_tool_call = true;
                            let effective_model =
                                this.requested_model.as_deref().unwrap_or(&this.model);
                            for mut tc in recovered {
                                tc["index"] = serde_json::json!(this.tool_call_index);
                                this.tool_call_index += 1;
                                let chunk = ChatCompletionChunk {
                                    id: this.chat_id.clone(),
                                    object: "chat.completion.chunk".to_string(),
                                    created: this.created,
                                    model: effective_model.to_string(),
                                    choices: vec![ChatChunkChoice {
                                        index: 0,
                                        delta: ChatChunkDelta {
                                            role: None,
                                            content: None,
                                            tool_calls: Some(serde_json::json!([tc])),
                                        },
                                        finish_reason: None,
                                    }],
                                    usage: None,
                                };
                                if let Ok(json_str) = serde_json::to_string(&chunk) {
                                    this.queue.push_back(Ok(Bytes::from(format!(
                                        "data: {json_str}\n\n"
                                    ))));
                                }
                            }
                            let finish_chunk = ChatCompletionChunk {
                                id: this.chat_id.clone(),
                                object: "chat.completion.chunk".to_string(),
                                created: this.created,
                                model: effective_model.to_string(),
                                choices: vec![ChatChunkChoice {
                                    index: 0,
                                    delta: ChatChunkDelta {
                                        role: None,
                                        content: None,
                                        tool_calls: None,
                                    },
                                    finish_reason: Some("tool_calls".to_string()),
                                }],
                                usage: None,
                            };
                            if let Ok(json_str) = serde_json::to_string(&finish_chunk) {
                                this.queue
                                    .push_back(Ok(Bytes::from(format!("data: {json_str}\n\n"))));
                            }
                        }
                    }
                    if let Some(mut lease) = this.lease.take() {
                        lease.commit_success();
                    }
                    return std::task::Poll::Ready(Some(Ok(Bytes::from("data: [DONE]\n\n"))));
                }
                return std::task::Poll::Ready(None);
            }

            match Pin::new(&mut this.inner).poll_next(cx) {
                std::task::Poll::Ready(Some(Ok(chunk))) => {
                    if this.buffer.len() + chunk.len() > MAX_SSE_BUFFER_BYTES {
                        if let Some(mut lease) = this.lease.take() {
                            lease.commit_error("SSE stream buffer limit exceeded 2MB", false);
                        }
                        return std::task::Poll::Ready(Some(Err(AppError::BadGateway(
                            "SSE stream buffer limit exceeded 2MB (memory safety guardrail)"
                                .to_string(),
                        ))));
                    }
                    this.buffer.extend_from_slice(&chunk);
                    let effective_model = this.requested_model.as_deref().unwrap_or(&this.model);
                    while let Some(pos) = this.buffer.iter().position(|&b| b == b'\n') {
                        let line: Vec<u8> = this.buffer.drain(..=pos).collect();
                        let events = process_sse_line(
                            &line,
                            &this.chat_id,
                            effective_model,
                            this.created,
                            &mut this.sent_first_role,
                            &mut this.sent_tool_call,
                            &mut this.tool_call_index,
                            &mut this.accumulated_text,
                        );
                        for ev in events {
                            if ev
                                .as_ref()
                                .map(|b| b.as_ref() == b"data: [DONE]\n\n")
                                .unwrap_or(false)
                            {
                                this.sent_done = true;
                                if let Some(mut lease) = this.lease.take() {
                                    lease.commit_success();
                                }
                            }
                            this.queue.push_back(ev);
                        }
                    }
                }
                std::task::Poll::Ready(Some(Err(err))) => {
                    let err_str = err.to_string();
                    if let Some(mut lease) = this.lease.take() {
                        lease.commit_error(&err_str, false);
                    }
                    return std::task::Poll::Ready(Some(Err(err)));
                }
                std::task::Poll::Ready(None) => {
                    this.ended = true;
                    if !this.buffer.is_empty() {
                        let remaining = std::mem::take(&mut this.buffer);
                        let effective_model =
                            this.requested_model.as_deref().unwrap_or(&this.model);
                        let events = process_sse_line(
                            &remaining,
                            &this.chat_id,
                            effective_model,
                            this.created,
                            &mut this.sent_first_role,
                            &mut this.sent_tool_call,
                            &mut this.tool_call_index,
                            &mut this.accumulated_text,
                        );
                        for ev in events {
                            if ev
                                .as_ref()
                                .map(|b| b.as_ref() == b"data: [DONE]\n\n")
                                .unwrap_or(false)
                            {
                                this.sent_done = true;
                                if let Some(mut lease) = this.lease.take() {
                                    lease.commit_success();
                                }
                            }
                            this.queue.push_back(ev);
                        }
                    }
                }
                std::task::Poll::Pending => return std::task::Poll::Pending,
            }
        }
    }
}

pub struct AntigravityProvider {
    pool: Arc<AccountPool>,
    refresher: Arc<dyn TokenRefresher>,
    base_url: String,
    client: Client,
    max_response_bytes: usize,
    max_attempts: usize,
}

impl std::fmt::Debug for AntigravityProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AntigravityProvider")
            .field("base_url", &self.base_url)
            .field("max_response_bytes", &self.max_response_bytes)
            .field("max_attempts", &self.max_attempts)
            .field("accounts_in_pool", &self.pool.account_count())
            .finish()
    }
}

impl AntigravityProvider {
    pub fn new(pool: Arc<AccountPool>, refresher: Arc<dyn TokenRefresher>) -> Self {
        Self::with_base_url(pool, refresher, AG_BASE_URL)
    }

    pub fn with_base_url(
        pool: Arc<AccountPool>,
        refresher: Arc<dyn TokenRefresher>,
        base_url: &str,
    ) -> Self {
        let timeout_secs = std::env::var("UPSTREAM_REQUEST_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(600);
        let read_timeout_secs = std::env::var("UPSTREAM_READ_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(300);

        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .connect_timeout(Duration::from_secs(30))
            .read_timeout(Duration::from_secs(read_timeout_secs))
            .tcp_keepalive(Some(Duration::from_secs(30)))
            .build()
            .expect("Failed to build reqwest client for AntigravityProvider");
        Self::with_client(pool, refresher, base_url, client)
    }

    pub fn with_client(
        pool: Arc<AccountPool>,
        refresher: Arc<dyn TokenRefresher>,
        base_url: &str,
        client: Client,
    ) -> Self {
        Self {
            pool,
            refresher,
            base_url: base_url.trim_end_matches('/').to_string(),
            client,
            max_response_bytes: MAX_RESPONSE_BYTES,
            max_attempts: 4,
        }
    }

    pub fn with_max_response_bytes(mut self, max_bytes: usize) -> Self {
        self.max_response_bytes = max_bytes;
        self
    }

    pub fn with_max_attempts(mut self, max_attempts: usize) -> Self {
        self.max_attempts = max_attempts.max(1);
        self
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn pool(&self) -> &Arc<AccountPool> {
        &self.pool
    }

    pub fn refresher(&self) -> &Arc<dyn TokenRefresher> {
        &self.refresher
    }
}

#[axum::async_trait]
impl Provider for AntigravityProvider {
    async fn complete(
        &self,
        request: &ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, AppError> {
        let mut last_error_msg = String::new();
        let mut last_status = 503;

        let max_attempts = self.max_attempts.min(self.pool.account_count().max(1));

        for _attempt in 0..max_attempts {
            let mut lease = match self
                .pool
                .acquire_lease_with_pacing(Some(&request.model))
                .await
            {
                Some(l) => l,
                None => {
                    let m_lower = request.model.to_lowercase();
                    if m_lower.contains("claude") || m_lower.contains("gpt") {
                        return Err(AppError::BadGateway(
                            "No account with Claude/GPT 5h quota remaining".to_string(),
                        ));
                    }
                    last_error_msg =
                        "No available Antigravity account (all in cooldown or quota exhausted)"
                            .to_string();
                    break;
                }
            };
            let acc = lease.account.clone();

            let access_token = match acc.access_token.as_ref() {
                Some(t)
                    if !t.is_empty()
                        && (!self.base_url.contains("googleapis.com")
                            || !t.starts_with("mock-")) =>
                {
                    t.clone()
                }
                _ => match self.pool.refresh_account_token(&acc.id).await {
                    Ok(tok) => tok,
                    Err(err) => {
                        lease.commit_error(&err, false);
                        last_error_msg = err;
                        continue;
                    }
                },
            };

            let url = format!("{}/v1internal:streamGenerateContent?alt=sse", self.base_url);
            let payload = build_antigravity_payload(request);

            let res = self
                .client
                .post(&url)
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .header(
                    reqwest::header::AUTHORIZATION,
                    format!("Bearer {access_token}"),
                )
                .header(reqwest::header::USER_AGENT, AG_USER_AGENT)
                .json(&payload)
                .send()
                .await;

            let response = match res {
                Ok(r) => r,
                Err(err) => {
                    let is_timeout = err.is_timeout() || err.is_connect();
                    let err_str = err.to_string();
                    lease.commit_error_with_cooldown(
                        if is_timeout { "timeout" } else { &err_str },
                        if is_timeout { 30.0 } else { 15.0 },
                    );
                    last_error_msg = err_str;
                    last_status = if is_timeout { 504 } else { 502 };
                    continue;
                }
            };

            let status = response.status();
            if status == reqwest::StatusCode::UNAUTHORIZED && _attempt == 0 {
                lease.release();
                let _ = self.pool.refresh_account_token(&acc.id).await;
                continue;
            }

            if !status.is_success() {
                let headers = response.headers().clone();
                let body_text = response.text().await.unwrap_or_default();
                let err_msg: String = body_text.chars().take(500).collect();
                if status == reqwest::StatusCode::BAD_REQUEST
                    || status == reqwest::StatusCode::UNPROCESSABLE_ENTITY
                {
                    lease.release();
                    return Err(AppError::BadRequest(format!(
                        "Upstream rejected request payload: {err_msg}"
                    )));
                }

                if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                    let retry_after = parse_retry_after(&headers).unwrap_or(120.0);
                    lease.commit_error_with_cooldown(&format!("{status}:{err_msg}"), retry_after);
                    last_error_msg = err_msg;
                    last_status = 429;
                    continue;
                }

                let is_ban = status == reqwest::StatusCode::FORBIDDEN
                    || err_msg.to_lowercase().contains("banned")
                    || err_msg.to_lowercase().contains("suspended")
                    || err_msg.to_lowercase().contains("account disabled");

                if is_ban {
                    lease.commit_error(&format!("{status}:{err_msg}"), true);
                } else {
                    let cooldown = parse_retry_after(&headers).unwrap_or(30.0);
                    lease.commit_error_with_cooldown(&format!("{status}:{err_msg}"), cooldown);
                }
                last_error_msg = err_msg;
                last_status = status.as_u16();
                continue;
            }

            // Status 200 OK: retries stop here!
            lease.commit_success();

            let body_bytes = response.bytes().await.map_err(|e| {
                AppError::BadGateway(format!("Failed to read upstream response body: {e}"))
            })?;

            if body_bytes.len() > self.max_response_bytes {
                return Err(AppError::BadGateway(format!(
                    "Upstream response exceeded limit of {} bytes",
                    self.max_response_bytes
                )));
            }

            let prompt_chars: usize = request
                .messages
                .iter()
                .map(|m| m.content_text().len())
                .sum();
            let prompt_tokens_fallback = (prompt_chars as u32 / 4).max(1);

            return parse_antigravity_response_body(
                &body_bytes,
                &request.model,
                prompt_tokens_fallback,
            );
        }

        Err(match last_status {
            400 => AppError::BadRequest(last_error_msg),
            404 => AppError::NotFound(last_error_msg),
            408 | 504 => AppError::GatewayTimeout(last_error_msg),
            _ => AppError::BadGateway(format!(
                "All attempts failed ({last_status}): {last_error_msg}"
            )),
        })
    }

    async fn stream_chat_completion(
        &self,
        request: &ChatCompletionRequest,
    ) -> Result<BoxChatStream, AppError> {
        let mut last_error_msg = String::new();
        let mut last_status = 503;

        let max_attempts = self.max_attempts.min(self.pool.account_count().max(1));

        for _attempt in 0..max_attempts {
            let mut lease = match self
                .pool
                .acquire_lease_with_pacing(Some(&request.model))
                .await
            {
                Some(l) => l,
                None => {
                    let m_lower = request.model.to_lowercase();
                    if m_lower.contains("claude") || m_lower.contains("gpt") {
                        return Err(AppError::BadGateway(
                            "No account with Claude/GPT 5h quota remaining".to_string(),
                        ));
                    }
                    last_error_msg =
                        "No available Antigravity account (all in cooldown or quota exhausted)"
                            .to_string();
                    break;
                }
            };
            let acc = lease.account.clone();

            let access_token = match acc.access_token.as_ref() {
                Some(t)
                    if !t.is_empty()
                        && (!self.base_url.contains("googleapis.com")
                            || !t.starts_with("mock-")) =>
                {
                    t.clone()
                }
                _ => match self.pool.refresh_account_token(&acc.id).await {
                    Ok(tok) => tok,
                    Err(err) => {
                        lease.commit_error(&err, false);
                        last_error_msg = err;
                        continue;
                    }
                },
            };

            let url = format!("{}/v1internal:streamGenerateContent?alt=sse", self.base_url);
            let payload = build_antigravity_payload(request);

            let res = self
                .client
                .post(&url)
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .header(
                    reqwest::header::AUTHORIZATION,
                    format!("Bearer {access_token}"),
                )
                .header(reqwest::header::USER_AGENT, AG_USER_AGENT)
                .json(&payload)
                .send()
                .await;

            let response = match res {
                Ok(r) => r,
                Err(err) => {
                    let is_timeout = err.is_timeout() || err.is_connect();
                    let err_str = err.to_string();
                    lease.commit_error_with_cooldown(
                        if is_timeout { "timeout" } else { &err_str },
                        if is_timeout { 30.0 } else { 15.0 },
                    );
                    last_error_msg = err_str;
                    last_status = if is_timeout { 504 } else { 502 };
                    continue;
                }
            };

            let status = response.status();
            if status == reqwest::StatusCode::UNAUTHORIZED && _attempt == 0 {
                lease.release();
                let _ = self.pool.refresh_account_token(&acc.id).await;
                continue;
            }

            if !status.is_success() {
                let headers = response.headers().clone();
                let body_text = response.text().await.unwrap_or_default();
                let err_msg: String = body_text.chars().take(500).collect();
                if status == reqwest::StatusCode::BAD_REQUEST
                    || status == reqwest::StatusCode::UNPROCESSABLE_ENTITY
                {
                    lease.release();
                    return Err(AppError::BadRequest(format!(
                        "Upstream rejected request payload: {err_msg}"
                    )));
                }

                if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                    let retry_after = parse_retry_after(&headers).unwrap_or(120.0);
                    lease.commit_error_with_cooldown(&format!("{status}:{err_msg}"), retry_after);
                    last_error_msg = err_msg;
                    last_status = 429;
                    continue;
                }

                let is_ban = status == reqwest::StatusCode::FORBIDDEN
                    || err_msg.to_lowercase().contains("banned")
                    || err_msg.to_lowercase().contains("suspended")
                    || err_msg.to_lowercase().contains("account disabled");

                if is_ban {
                    lease.commit_error(&format!("{status}:{err_msg}"), true);
                } else {
                    let cooldown = parse_retry_after(&headers).unwrap_or(30.0);
                    lease.commit_error_with_cooldown(&format!("{status}:{err_msg}"), cooldown);
                }
                last_error_msg = err_msg;
                last_status = status.as_u16();
                continue;
            }

            // Status 200 OK: retries stop here!
            let byte_stream = response.bytes_stream().map(|chunk_res| {
                chunk_res.map_err(|e| AppError::BadGateway(format!("Upstream stream error: {e}")))
            });

            let gemini_stream = GeminiSseStream::new(
                Box::pin(byte_stream),
                request.model.clone(),
                format!("chatcmpl-{}", uuid::Uuid::new_v4().simple()),
                chrono::Utc::now().timestamp(),
                Some(lease),
            )
            .with_requested_model(request.model.clone());

            return Ok(Box::pin(gemini_stream));
        }

        Err(match last_status {
            400 => AppError::BadRequest(last_error_msg),
            404 => AppError::NotFound(last_error_msg),
            408 | 504 => AppError::GatewayTimeout(last_error_msg),
            _ => AppError::BadGateway(format!(
                "All attempts failed ({last_status}): {last_error_msg}"
            )),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_chat_completions_url() {
        assert_eq!(
            resolve_chat_completions_url("https://api.openai.com"),
            "https://api.openai.com/v1/chat/completions"
        );
        assert_eq!(
            resolve_chat_completions_url("https://api.openai.com/"),
            "https://api.openai.com/v1/chat/completions"
        );
        assert_eq!(
            resolve_chat_completions_url("https://api.openai.com/v1"),
            "https://api.openai.com/v1/chat/completions"
        );
        assert_eq!(
            resolve_chat_completions_url("https://api.openai.com/v1/"),
            "https://api.openai.com/v1/chat/completions"
        );
        assert_eq!(
            resolve_chat_completions_url("http://127.0.0.1:8080"),
            "http://127.0.0.1:8080/v1/chat/completions"
        );
        assert_eq!(
            resolve_chat_completions_url("http://127.0.0.1:8080/v1"),
            "http://127.0.0.1:8080/v1/chat/completions"
        );
    }

    #[tokio::test]
    async fn test_mock_provider_deterministic_response() {
        let mock = MockProvider::new();
        let req = ChatCompletionRequest {
            model: "ag/gemini-3.8-flash-high".to_string(),
            messages: vec![ChatMessage::new("user", "Hello mock")],
            stream: Some(false),
            ..Default::default()
        };

        let resp = mock.complete(&req).await.unwrap();
        assert_eq!(resp.id, "chatcmpl-mock-deterministic");
        assert_eq!(resp.object, "chat.completion");
        assert_eq!(resp.created, 1700000000);
        assert_eq!(resp.model, "ag/gemini-3.8-flash-high");
        assert_eq!(resp.choices.len(), 1);
        assert_eq!(resp.choices[0].message.role, "assistant");
        assert!(resp.choices[0]
            .message
            .content
            .as_deref()
            .unwrap()
            .contains("Deterministic mock completion"));
        assert_eq!(resp.choices[0].finish_reason, "stop");
        assert_eq!(
            resp.usage.total_tokens,
            resp.usage.prompt_tokens + resp.usage.completion_tokens
        );

        let last = mock.last_request().expect("should have recorded request");
        assert_eq!(last.model, "ag/gemini-3.8-flash-high");
    }

    #[test]
    fn test_serde_chat_completion_request_with_tools() {
        let json_data = serde_json::json!({
            "model": "ag/claude-sonnet-4-6",
            "messages": [
                {"role": "user", "content": "What's the weather in Tokyo?"}
            ],
            "stream": false,
            "tools": [
                {
                    "type": "function",
                    "function": {
                        "name": "get_weather",
                        "description": "Get current weather"
                    }
                }
            ],
            "tool_choice": "auto"
        });

        let req: ChatCompletionRequest = serde_json::from_value(json_data).unwrap();
        assert_eq!(req.model, "ag/claude-sonnet-4-6");
        assert_eq!(req.messages.len(), 1);
        assert_eq!(
            req.messages[0].content_text(),
            "What's the weather in Tokyo?"
        );
        assert_eq!(req.stream, Some(false));
        assert!(req.tools.is_some());
        assert_eq!(req.tool_choice, Some(serde_json::json!("auto")));
    }

    #[test]
    fn test_http_provider_debug_redacts_keys() {
        let provider = HttpUpstreamProvider::new(
            Some("https://api.example.com".to_string()),
            Some("super-secret-key-12345".to_string()),
            5,
            10,
            15,
        );
        let debug_str = format!("{provider:?}");
        assert!(!debug_str.contains("super-secret-key-12345"));
        assert!(debug_str.contains("[REDACTED]"));
        assert!(debug_str.contains("https://api.example.com"));
    }

    #[tokio::test]
    async fn test_http_provider_missing_base_url() {
        let provider = HttpUpstreamProvider::new(None, None, 5, 10, 15);
        let req = ChatCompletionRequest {
            model: "ag/gemini-3.8-flash-high".to_string(),
            messages: vec![ChatMessage::new("user", "test")],
            ..Default::default()
        };

        let err = provider.complete(&req).await.unwrap_err();
        match err {
            AppError::Internal(msg) => assert!(msg.contains("Upstream base URL is not configured")),
            _ => panic!("Expected AppError::Internal for missing base URL, got {err:?}"),
        }
    }

    #[tokio::test]
    async fn test_mock_provider_streaming() {
        let mock = MockProvider::new();
        let req = ChatCompletionRequest {
            model: "ag/gemini-3.8-flash-high".to_string(),
            messages: vec![ChatMessage::new("user", "Hello streaming")],
            stream: Some(true),
            ..Default::default()
        };

        let mut stream = mock.stream_chat_completion(&req).await.unwrap();
        let mut events = Vec::new();
        while let Some(chunk_result) = stream.next().await {
            let chunk_bytes = chunk_result.unwrap();
            let text = String::from_utf8(chunk_bytes.to_vec()).unwrap();
            events.push(text);
        }

        assert_eq!(events.len(), 4);
        assert!(events[0].starts_with("data: "));
        assert!(events[0].ends_with("\n\n"));
        assert!(events[1].starts_with("data: "));
        assert!(events[1].ends_with("\n\n"));
        assert!(events[2].starts_with("data: "));
        assert!(events[2].ends_with("\n\n"));
        assert_eq!(events[3], "data: [DONE]\n\n");

        // Parse chunk 0
        let json_str0 = events[0].trim_start_matches("data: ").trim();
        let chunk0: ChatCompletionChunk = serde_json::from_str(json_str0).unwrap();
        assert_eq!(chunk0.id, "chatcmpl-mock-streaming");
        assert_eq!(chunk0.object, "chat.completion.chunk");
        assert_eq!(chunk0.model, "ag/gemini-3.8-flash-high");
        assert_eq!(chunk0.choices[0].delta.role.as_deref(), Some("assistant"));
        assert!(chunk0.choices[0].delta.content.is_some());
        assert_eq!(chunk0.choices[0].finish_reason, None);

        // Parse chunk 1
        let json_str1 = events[1].trim_start_matches("data: ").trim();
        let chunk1: ChatCompletionChunk = serde_json::from_str(json_str1).unwrap();
        assert_eq!(chunk1.choices[0].delta.role, None);
        assert!(chunk1.choices[0].delta.content.is_some());
        assert_eq!(chunk1.choices[0].finish_reason, None);

        // Parse chunk 2
        let json_str2 = events[2].trim_start_matches("data: ").trim();
        let chunk2: ChatCompletionChunk = serde_json::from_str(json_str2).unwrap();
        assert_eq!(chunk2.choices[0].delta.role, None);
        assert_eq!(chunk2.choices[0].delta.content, None);
        assert_eq!(chunk2.choices[0].finish_reason.as_deref(), Some("stop"));

        let last = mock.last_request().expect("should record stream request");
        assert_eq!(last.model, "ag/gemini-3.8-flash-high");
    }

    #[tokio::test]
    async fn test_http_provider_missing_base_url_streaming() {
        let provider = HttpUpstreamProvider::new(None, None, 5, 10, 15);
        let req = ChatCompletionRequest {
            model: "ag/gemini-3.8-flash-high".to_string(),
            messages: vec![ChatMessage::new("user", "test")],
            stream: Some(true),
            ..Default::default()
        };

        let err = match provider.stream_chat_completion(&req).await {
            Ok(_) => panic!("Expected error for missing base URL, got Ok"),
            Err(e) => e,
        };
        match err {
            AppError::Internal(msg) => assert!(msg.contains("Upstream base URL is not configured")),
            _ => panic!("Expected AppError::Internal for missing base URL, got {err:?}"),
        }
    }

    #[test]
    fn test_map_antigravity_upstream_model() {
        // High / agent aliases map to gemini-pro-agent
        assert_eq!(
            map_antigravity_upstream_model("ag/gemini-pro-agent"),
            "gemini-pro-agent"
        );
        assert_eq!(
            map_antigravity_upstream_model("gemini-pro-agent"),
            "gemini-pro-agent"
        );
        assert_eq!(
            map_antigravity_upstream_model("ag/gemini-3.1-pro"),
            "gemini-pro-agent"
        );
        assert_eq!(
            map_antigravity_upstream_model("gemini-3.1-pro"),
            "gemini-pro-agent"
        );
        assert_eq!(
            map_antigravity_upstream_model("ag/gemini-3.1-pro-high"),
            "gemini-pro-agent"
        );
        assert_eq!(
            map_antigravity_upstream_model("gemini-3.1-pro-high"),
            "gemini-pro-agent"
        );

        // Low aliases map to gemini-3.1-pro-low
        assert_eq!(
            map_antigravity_upstream_model("ag/gemini-3.1-pro-low"),
            "gemini-3.1-pro-low"
        );
        assert_eq!(
            map_antigravity_upstream_model("gemini-3.1-pro-low"),
            "gemini-3.1-pro-low"
        );
        assert_eq!(
            map_antigravity_upstream_model("ag/gemini-pro-low"),
            "gemini-3.1-pro-low"
        );
        assert_eq!(
            map_antigravity_upstream_model("gemini-pro-low"),
            "gemini-3.1-pro-low"
        );

        // Pass-through for existing flash and claude models
        assert_eq!(
            map_antigravity_upstream_model("ag/gemini-3.8-flash-high"),
            "gemini-3.8-flash-high"
        );
        assert_eq!(
            map_antigravity_upstream_model("ag/claude-sonnet-4-6"),
            "claude-sonnet-4-6"
        );

        // Safety assertion: raw unsupported gemini-3.1-pro is never returned
        assert_ne!(
            map_antigravity_upstream_model("ag/gemini-3.1-pro"),
            "gemini-3.1-pro"
        );
        assert_ne!(
            map_antigravity_upstream_model("gemini-3.1-pro"),
            "gemini-3.1-pro"
        );
    }

    #[test]
    fn test_build_antigravity_payload_gemini_pro_aliases() {
        // Test ag/gemini-3.1-pro alias
        let req_pro = ChatCompletionRequest {
            model: "ag/gemini-3.1-pro".to_string(),
            messages: vec![ChatMessage::new("user", "Hello Gemini Pro")],
            ..Default::default()
        };
        let payload_pro = build_antigravity_payload(&req_pro);
        assert_eq!(payload_pro["model"], "gemini-pro-agent");
        assert_ne!(payload_pro["model"], "gemini-3.1-pro");

        // Test ag/gemini-3.1-pro-high alias
        let req_high = ChatCompletionRequest {
            model: "ag/gemini-3.1-pro-high".to_string(),
            messages: vec![ChatMessage::new("user", "Hello Gemini Pro High")],
            ..Default::default()
        };
        let payload_high = build_antigravity_payload(&req_high);
        assert_eq!(payload_high["model"], "gemini-pro-agent");

        // Test unprefixed gemini-3.1-pro
        let req_unprefixed = ChatCompletionRequest {
            model: "gemini-3.1-pro".to_string(),
            messages: vec![ChatMessage::new("user", "Hello raw")],
            ..Default::default()
        };
        let payload_unprefixed = build_antigravity_payload(&req_unprefixed);
        assert_eq!(payload_unprefixed["model"], "gemini-pro-agent");
        assert_ne!(payload_unprefixed["model"], "gemini-3.1-pro");

        // Test ag/gemini-3.1-pro-low
        let req_low = ChatCompletionRequest {
            model: "ag/gemini-3.1-pro-low".to_string(),
            messages: vec![ChatMessage::new("user", "Hello Gemini Pro Low")],
            ..Default::default()
        };
        let payload_low = build_antigravity_payload(&req_low);
        assert_eq!(payload_low["model"], "gemini-3.1-pro-low");
    }

    #[test]
    fn test_build_antigravity_payload_gemini() {
        let req = ChatCompletionRequest {
            model: "ag/gemini-3.8-flash-high".to_string(),
            messages: vec![
                ChatMessage::new("system", "You are a helpful assistant."),
                ChatMessage::new("user", "Hello!"),
            ],
            stream: Some(false),
            tools: Some(serde_json::json!([{
                "type": "function",
                "function": {
                    "name": "lookup",
                    "description": "Lookup information",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "q": {"type": "string"}
                        }
                    }
                }
            }])),
            temperature: Some(0.7),
            max_tokens: Some(2048),
            ..Default::default()
        };

        let payload = build_antigravity_payload(&req);
        assert_eq!(payload["project"], "aicode-consumers");
        assert_eq!(payload["model"], "gemini-3.8-flash-high");
        assert_eq!(payload["userAgent"], "antigravity");
        assert_eq!(payload["requestType"], "agent");
        assert!(payload["requestId"].as_str().unwrap().starts_with("agent/"));

        let request_obj = &payload["request"];
        assert_eq!(
            request_obj["systemInstruction"]["parts"][0]["text"],
            "You are a helpful assistant."
        );
        assert_eq!(request_obj["contents"][0]["role"], "user");
        assert_eq!(request_obj["contents"][0]["parts"][0]["text"], "Hello!");
        assert!(
            (request_obj["generationConfig"]["temperature"]
                .as_f64()
                .unwrap()
                - 0.7)
                .abs()
                < 1e-5
        );
        assert_eq!(request_obj["generationConfig"]["maxOutputTokens"], 2048);
        assert_eq!(request_obj["generationConfig"]["topK"], 40);

        let tools = request_obj["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 1);
        let decls = tools[0]["functionDeclarations"].as_array().unwrap();
        assert_eq!(decls[0]["name"], "lookup");
    }

    #[test]
    fn test_build_antigravity_payload_claude() {
        let req = ChatCompletionRequest {
            model: "ag/claude-sonnet-4-6".to_string(),
            messages: vec![ChatMessage::new("user", "Hi Claude")],
            stream: Some(true),
            ..Default::default()
        };

        let payload = build_antigravity_payload(&req);
        assert_eq!(payload["model"], "claude-sonnet-4-6");
        let request_obj = &payload["request"];
        // Claude defaults: maxOutputTokens 4096, no topK
        assert_eq!(request_obj["generationConfig"]["maxOutputTokens"], 4096);
        assert!(request_obj["generationConfig"].get("topK").is_none());
    }

    #[test]
    fn test_parse_antigravity_response_body_gemini_sse() {
        let sse_data = "data: {\"response\":{\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"Hello from \"}],\"role\":\"model\"}}],\"usageMetadata\":{\"promptTokenCount\":10,\"candidatesTokenCount\":3}}}\n\ndata: {\"response\":{\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"Gemini!\"}],\"role\":\"model\"},\"finishReason\":\"STOP\"}],\"usageMetadata\":{\"promptTokenCount\":10,\"candidatesTokenCount\":6}}}\n\ndata: [DONE]\n\n";

        let resp =
            parse_antigravity_response_body(sse_data.as_bytes(), "ag/gemini-3.8-flash-high", 10)
                .unwrap();

        assert_eq!(resp.model, "ag/gemini-3.8-flash-high");
        assert_eq!(resp.choices.len(), 1);
        assert_eq!(
            resp.choices[0].message.content.as_deref(),
            Some("Hello from Gemini!")
        );
        assert_eq!(resp.choices[0].finish_reason, "stop");
        assert_eq!(resp.usage.prompt_tokens, 10);
        assert_eq!(resp.usage.completion_tokens, 6);
        assert_eq!(resp.usage.total_tokens, 16);
    }

    #[test]
    fn test_parse_antigravity_response_body_openai_json() {
        let openai_json = serde_json::json!({
            "id": "chatcmpl-test-123",
            "object": "chat.completion",
            "created": 1700000000,
            "model": "ag/gemini-3.8-flash-high",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Direct OpenAI response"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 5,
                "completion_tokens": 4,
                "total_tokens": 9
            }
        });

        let resp = parse_antigravity_response_body(
            &serde_json::to_vec(&openai_json).unwrap(),
            "ag/gemini-3.8-flash-high",
            5,
        )
        .unwrap();

        assert_eq!(resp.id, "chatcmpl-test-123");
        assert_eq!(
            resp.choices[0].message.content.as_deref(),
            Some("Direct OpenAI response")
        );
    }

    #[test]
    fn test_antigravity_provider_debug_redacts_tokens() {
        let db = Arc::new(
            crate::db::Database::open_or_create(std::path::Path::new(":memory:"), None).unwrap(),
        );
        let pool = Arc::new(AccountPool::new(db));
        let _ = pool.add_account("test@example.com", "super-secret-refresh-token");
        let refresher = Arc::new(crate::account::MockTokenRefresher::with_token(
            "super-secret-access-token",
        ));

        let provider = AntigravityProvider::new(pool, refresher);
        let debug_str = format!("{provider:?}");

        assert!(!debug_str.contains("super-secret-refresh-token"));
        assert!(!debug_str.contains("super-secret-access-token"));
        assert!(debug_str.contains("AntigravityProvider"));
        assert!(debug_str.contains("accounts_in_pool: 1"));
    }

    #[test]
    fn test_regression_plain_string_content() {
        let msg = ChatMessage::new("user", "Hello plain string");
        assert_eq!(msg.content_text(), "Hello plain string");
        let json_val = serde_json::to_value(&msg).unwrap();
        assert_eq!(json_val["content"], "Hello plain string");

        let deserialized: ChatMessage = serde_json::from_value(json_val).unwrap();
        assert_eq!(
            deserialized.content,
            Some(MessageContent::Text("Hello plain string".to_string()))
        );
        assert_eq!(deserialized.content_text(), "Hello plain string");
    }

    #[test]
    fn test_regression_array_content_parts() {
        let json_raw = serde_json::json!({
            "role": "user",
            "content": [
                {"type": "text", "text": "Hello "},
                {"type": "text", "text": "world!"},
                {"input_text": " Extra info."},
                " Trailing string."
            ]
        });

        let msg: ChatMessage = serde_json::from_value(json_raw).unwrap();
        assert_eq!(
            msg.content_text(),
            "Hello world! Extra info. Trailing string."
        );
        assert!(matches!(msg.content, Some(MessageContent::Parts(_))));

        let serialized = serde_json::to_value(&msg).unwrap();
        let parts = serialized["content"].as_array().unwrap();
        assert_eq!(parts.len(), 4);
        assert_eq!(parts[0]["text"], "Hello ");
        assert_eq!(parts[1]["text"], "world!");
    }

    #[test]
    fn test_regression_antigravity_signed_tool_call_and_result() {
        let req = ChatCompletionRequest {
            model: "ag/gemini-3.8-flash-high".to_string(),
            messages: vec![
                ChatMessage::user("Get AAPL price"),
                ChatMessage {
                    role: "assistant".to_string(),
                    content: None,
                    name: None,
                    tool_call_id: None,
                    tool_calls: Some(serde_json::json!([{
                        "id": "call_abc123:test_thought_signature_xyz",
                        "type": "function",
                        "thought_signature": "test_thought_signature_xyz",
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
                    tool_call_id: Some("call_abc123:test_thought_signature_xyz".to_string()),
                    tool_calls: None,
                },
            ],
            ..Default::default()
        };

        let payload = build_antigravity_payload(&req);
        let contents = payload["request"]["contents"].as_array().unwrap();
        assert_eq!(contents.len(), 3);

        // Assistant model turn
        assert_eq!(contents[1]["role"], "model");
        let model_parts = contents[1]["parts"].as_array().unwrap();
        assert_eq!(
            model_parts[0]["thoughtSignature"],
            "test_thought_signature_xyz"
        );
        assert_eq!(model_parts[0]["functionCall"]["id"], "call_abc123");
        assert_eq!(model_parts[0]["functionCall"]["name"], "get_stock_price");

        // Tool turn mapped to user functionResponse with pure ID
        assert_eq!(contents[2]["role"], "user");
        let tool_parts = contents[2]["parts"].as_array().unwrap();
        assert_eq!(tool_parts[0]["functionResponse"]["id"], "call_abc123");
        assert_eq!(tool_parts[0]["functionResponse"]["name"], "get_stock_price");
    }

    #[test]
    fn test_regression_antigravity_unsigned_degrades_to_text_marker() {
        let req = ChatCompletionRequest {
            model: "ag/gemini-3.8-flash-high".to_string(),
            messages: vec![
                ChatMessage::user("Run historical calculation"),
                ChatMessage {
                    role: "assistant".to_string(),
                    content: None,
                    name: None,
                    tool_call_id: None,
                    tool_calls: Some(serde_json::json!([{
                        "id": "call_unsigned_456",
                        "type": "function",
                        "function": {
                            "name": "calc",
                            "arguments": "{\"x\":42}"
                        }
                    }])),
                },
                ChatMessage {
                    role: "tool".to_string(),
                    content: Some(MessageContent::Text("1764".to_string())),
                    name: Some("calc".to_string()),
                    tool_call_id: Some("call_unsigned_456".to_string()),
                    tool_calls: None,
                },
            ],
            ..Default::default()
        };

        let payload = build_antigravity_payload(&req);
        let contents = payload["request"]["contents"].as_array().unwrap();
        assert_eq!(contents.len(), 3);

        // Unsigned assistant call safely degraded to text marker
        assert_eq!(contents[1]["role"], "model");
        let model_text = contents[1]["parts"][0]["text"].as_str().unwrap();
        assert!(model_text.contains("[Tool Call: calc({\"x\":42})]"));
        assert!(contents[1]["parts"][0].get("functionCall").is_none());

        // Unsigned tool result safely degraded to text marker
        assert_eq!(contents[2]["role"], "user");
        let tool_text = contents[2]["parts"][0]["text"].as_str().unwrap();
        assert!(tool_text.contains("[Tool Result for calc: 1764]"));
        assert!(contents[2]["parts"][0].get("functionResponse").is_none());
    }

    #[test]
    fn test_regression_antigravity_parallel_tool_results_merge_single_user_block() {
        let req = ChatCompletionRequest {
            model: "ag/claude-sonnet-4-6".to_string(),
            messages: vec![
                ChatMessage::user("Parallel query"),
                ChatMessage {
                    role: "assistant".to_string(),
                    content: None,
                    name: None,
                    tool_call_id: None,
                    tool_calls: Some(serde_json::json!([
                        {
                            "id": "call_p1",
                            "type": "function",
                            "function": {"name": "f1", "arguments": "{}"}
                        },
                        {
                            "id": "call_p2",
                            "type": "function",
                            "function": {"name": "f2", "arguments": "{}"}
                        }
                    ])),
                },
                ChatMessage {
                    role: "tool".to_string(),
                    content: Some(MessageContent::Text("r1".to_string())),
                    name: Some("f1".to_string()),
                    tool_call_id: Some("call_p1".to_string()),
                    tool_calls: None,
                },
                ChatMessage {
                    role: "tool".to_string(),
                    content: Some(MessageContent::Text("r2".to_string())),
                    name: Some("f2".to_string()),
                    tool_call_id: Some("call_p2".to_string()),
                    tool_calls: None,
                },
            ],
            ..Default::default()
        };

        let payload = build_antigravity_payload(&req);
        let contents = payload["request"]["contents"].as_array().unwrap();
        // 1 user prompt + 1 model tool_calls + 1 MERGED user block containing 2 functionResponse parts
        assert_eq!(contents.len(), 3);
        assert_eq!(contents[2]["role"], "user");
        let parts = contents[2]["parts"].as_array().unwrap();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0]["functionResponse"]["id"], "call_p1");
        assert_eq!(parts[1]["functionResponse"]["id"], "call_p2");
    }

    #[test]
    fn test_regression_antigravity_missing_ids_normalized() {
        let req = ChatCompletionRequest {
            model: "ag/claude-sonnet-4-6".to_string(),
            messages: vec![
                ChatMessage::user("Missing ID query"),
                ChatMessage {
                    role: "assistant".to_string(),
                    content: None,
                    name: None,
                    tool_call_id: None,
                    tool_calls: Some(serde_json::json!([
                        {
                            "type": "function",
                            "function": {"name": "get_time", "arguments": "{}"}
                        }
                    ])),
                },
                ChatMessage {
                    role: "tool".to_string(),
                    content: Some(MessageContent::Text("12:00".to_string())),
                    name: Some("get_time".to_string()),
                    tool_call_id: None,
                    tool_calls: None,
                },
            ],
            ..Default::default()
        };

        let payload = build_antigravity_payload(&req);
        let contents = payload["request"]["contents"].as_array().unwrap();
        assert_eq!(contents.len(), 3);

        let call_id = contents[1]["parts"][0]["functionCall"]["id"]
            .as_str()
            .unwrap();
        assert!(!call_id.is_empty());
        let res_id = contents[2]["parts"][0]["functionResponse"]["id"]
            .as_str()
            .unwrap();
        assert!(!res_id.is_empty());
        assert_eq!(call_id, res_id);
    }

    #[test]
    fn test_regression_gemini_stream_and_non_stream_thought_signature_preserved() {
        // 1. Non-streaming parsing
        let gemini_json = serde_json::json!({
            "response": {
                "candidates": [{
                    "content": {
                        "parts": [{
                            "functionCall": {
                                "name": "weather",
                                "args": {"loc": "Tokyo"}
                            },
                            "thoughtSignature": "tokyo_thought_sig_123"
                        }],
                        "role": "model"
                    },
                    "finishReason": "STOP"
                }],
                "usageMetadata": {
                    "promptTokenCount": 20,
                    "candidatesTokenCount": 10
                }
            }
        });

        let resp = parse_antigravity_response_body(
            &serde_json::to_vec(&gemini_json).unwrap(),
            "ag/gemini-3.8-flash-high",
            20,
        )
        .unwrap();

        assert_eq!(resp.choices[0].finish_reason, "tool_calls");
        let tc = &resp.choices[0]
            .message
            .tool_calls
            .as_ref()
            .unwrap()
            .as_array()
            .unwrap()[0];
        assert_eq!(tc["function"]["name"], "weather");
        assert_eq!(tc["thought_signature"], "tokyo_thought_sig_123");
        assert!(tc["id"].as_str().unwrap().contains("tokyo_thought_sig_123"));

        // 2. Streaming chunks
        let mut sent_role = false;
        let mut sent_tool_call = false;
        let mut tool_call_index = 0;
        let mut accumulated_text = String::new();
        let sse_line = format!("data: {}\n\n", serde_json::to_string(&gemini_json).unwrap());
        let chunks = process_sse_line(
            sse_line.as_bytes(),
            "chatcmpl-test",
            "ag/gemini-3.8-flash-high",
            1700000000,
            &mut sent_role,
            &mut sent_tool_call,
            &mut tool_call_index,
            &mut accumulated_text,
        );
        assert!(!chunks.is_empty());
        let chunk_bytes = chunks[0].as_ref().unwrap();
        let chunk_str = std::str::from_utf8(chunk_bytes).unwrap();
        assert!(chunk_str.contains("tokyo_thought_sig_123"));
        assert!(chunk_str.contains("thought_signature"));
    }

    #[test]
    fn test_normalize_tool_schema_claude_compliance() {
        let input = serde_json::json!({
            "type": "OBJECT",
            "properties": {
                "command": {
                    "type": "STRING",
                    "description": "Shell command"
                },
                "notify": {
                    "anyOf": [
                        {"type": "BOOLEAN"},
                        {"type": "ARRAY", "items": {"type": "STRING"}}
                    ]
                },
                "items_legacy": {
                    "type": "ARRAY",
                    "items": [{"type": "STRING"}]
                }
            },
            "required": ["command", "nonexistent_field"],
            "$schema": "http://json-schema.org/draft-07/schema#",
            "definitions": {}
        });

        let normalized = normalize_tool_schema(&input);

        // 1. Root and primitive types must be lowercase for Draft 2020-12
        assert_eq!(normalized["type"], "object");
        assert_eq!(normalized["properties"]["command"]["type"], "string");

        // 2. anyOf must be collapsed to a single valid schema branch
        assert_eq!(normalized["properties"]["notify"]["type"], "boolean");

        // 3. Draft-07 tuple items must be flattened to a single object
        assert_eq!(
            normalized["properties"]["items_legacy"]["items"]["type"],
            "string"
        );

        // 4. required array must only contain valid property names
        let req = normalized["required"].as_array().unwrap();
        assert_eq!(req.len(), 1);
        assert_eq!(req[0], "command");

        // 5. Unsupported root metadata keys must be stripped
        assert!(normalized.get("$schema").is_none());
        assert!(normalized.get("definitions").is_none());
    }

    #[test]
    fn test_parse_retry_after_semantics() {
        let mut headers = reqwest::header::HeaderMap::new();
        assert_eq!(parse_retry_after(&headers), None);

        headers.insert(
            reqwest::header::RETRY_AFTER,
            reqwest::header::HeaderValue::from_static("60"),
        );
        assert_eq!(parse_retry_after(&headers), Some(60.0));

        headers.insert(
            reqwest::header::RETRY_AFTER,
            reqwest::header::HeaderValue::from_static("0"),
        );
        assert_eq!(parse_retry_after(&headers), Some(1.0)); // minimum clamped to 1.0s

        headers.insert(
            reqwest::header::RETRY_AFTER,
            reqwest::header::HeaderValue::from_static("invalid"),
        );
        assert_eq!(parse_retry_after(&headers), None);
    }

    #[tokio::test]
    async fn test_gemini_stream_memory_safety_buffer_limit() {
        use futures_util::stream;
        use futures_util::StreamExt;

        let big_chunk = Bytes::from(vec![b'x'; MAX_SSE_BUFFER_BYTES + 10]);
        let stream_mock: BoxChatStream = Box::pin(stream::iter(vec![Ok(big_chunk)]));

        let mut sse_stream = GeminiSseStream::new(
            stream_mock,
            "ag/gemini-3.8-flash-high".to_string(),
            "chatcmpl-test".to_string(),
            chrono::Utc::now().timestamp(),
            None,
        );

        let res = sse_stream.next().await;
        assert!(res.is_some());
        match res.unwrap() {
            Err(AppError::BadGateway(msg)) => {
                assert!(msg.contains("2MB"));
            }
            other => panic!("Expected BadGateway buffer limit error, got: {other:?}"),
        }
    }

    #[test]
    fn test_gemini_sse_parallel_tool_calls_indices() {
        let gemini_json = serde_json::json!({
            "candidates": [{
                "content": {
                    "parts": [
                        {
                            "functionCall": {
                                "name": "get_weather",
                                "args": {"city": "Hanoi"}
                            }
                        },
                        {
                            "functionCall": {
                                "name": "get_time",
                                "args": {"tz": "Asia/Bangkok"}
                            }
                        }
                    ]
                }
            }]
        });

        let mut sent_role = false;
        let mut sent_tool_call = false;
        let mut tool_call_index = 0;
        let mut accumulated_text = String::new();
        let sse_line = format!("data: {}\n\n", serde_json::to_string(&gemini_json).unwrap());
        let chunks = process_sse_line(
            sse_line.as_bytes(),
            "chatcmpl-parallel-tc",
            "ag/gemini-2.5-pro",
            1700000000,
            &mut sent_role,
            &mut sent_tool_call,
            &mut tool_call_index,
            &mut accumulated_text,
        );

        assert_eq!(chunks.len(), 2);
        let chunk0_bytes = chunks[0].as_ref().unwrap();
        let chunk0_str = std::str::from_utf8(chunk0_bytes).unwrap();
        let chunk0_json = chunk0_str.strip_prefix("data: ").unwrap().trim();
        let chunk0: ChatCompletionChunk = serde_json::from_str(chunk0_json).unwrap();

        let chunk1_bytes = chunks[1].as_ref().unwrap();
        let chunk1_str = std::str::from_utf8(chunk1_bytes).unwrap();
        let chunk1_json = chunk1_str.strip_prefix("data: ").unwrap().trim();
        let chunk1: ChatCompletionChunk = serde_json::from_str(chunk1_json).unwrap();

        let tc0 = &chunk0.choices[0].delta.tool_calls.as_ref().unwrap()[0];
        let tc1 = &chunk1.choices[0].delta.tool_calls.as_ref().unwrap()[0];

        assert_eq!(tc0["index"], 0);
        assert_eq!(tc0["function"]["name"], "get_weather");
        assert_eq!(tc1["index"], 1);
        assert_eq!(tc1["function"]["name"], "get_time");
        assert_eq!(tool_call_index, 2);
    }

    #[tokio::test]
    async fn test_gemini_sse_stream_parallel_tool_calls_indices_across_chunks() {
        use futures_util::stream;
        use futures_util::StreamExt;

        let line1_json = serde_json::json!({
            "candidates": [{
                "content": {
                    "parts": [{
                        "functionCall": {
                            "name": "func_a",
                            "args": {"x": 1}
                        }
                    }]
                }
            }]
        });
        let line2_json = serde_json::json!({
            "candidates": [{
                "content": {
                    "parts": [{
                        "functionCall": {
                            "name": "func_b",
                            "args": {"y": 2}
                        }
                    }]
                }
            }]
        });

        let chunk1 = Bytes::from(format!(
            "data: {}\n\n",
            serde_json::to_string(&line1_json).unwrap()
        ));
        let chunk2 = Bytes::from(format!(
            "data: {}\n\n",
            serde_json::to_string(&line2_json).unwrap()
        ));
        let stream_mock: BoxChatStream = Box::pin(stream::iter(vec![Ok(chunk1), Ok(chunk2)]));

        let mut sse_stream = GeminiSseStream::new(
            stream_mock,
            "ag/gemini-2.5-pro".to_string(),
            "chatcmpl-consecutive".to_string(),
            chrono::Utc::now().timestamp(),
            None,
        );

        let res1 = sse_stream.next().await.unwrap().unwrap();
        let res1_str = std::str::from_utf8(&res1).unwrap();
        let res1_json = res1_str.strip_prefix("data: ").unwrap().trim();
        let chunk1_obj: ChatCompletionChunk = serde_json::from_str(res1_json).unwrap();
        let tc0 = &chunk1_obj.choices[0].delta.tool_calls.as_ref().unwrap()[0];
        assert_eq!(tc0["index"], 0);
        assert_eq!(tc0["function"]["name"], "func_a");

        let res2 = sse_stream.next().await.unwrap().unwrap();
        let res2_str = std::str::from_utf8(&res2).unwrap();
        let res2_json = res2_str.strip_prefix("data: ").unwrap().trim();
        let chunk2_obj: ChatCompletionChunk = serde_json::from_str(res2_json).unwrap();
        let tc1 = &chunk2_obj.choices[0].delta.tool_calls.as_ref().unwrap()[0];
        assert_eq!(
            tc1["index"], 1,
            "Tool call index across separate SSE lines/chunks must be 1, not reset to 0"
        );
        assert_eq!(tc1["function"]["name"], "func_b");

        // Verify sequential completion stream construction resets index
        let stream_mock2: BoxChatStream = Box::pin(stream::iter(vec![]));
        let sse_stream2 = GeminiSseStream::new(
            stream_mock2,
            "ag/gemini-2.5-pro".to_string(),
            "chatcmpl-seq2".to_string(),
            chrono::Utc::now().timestamp(),
            None,
        );
        assert_eq!(
            sse_stream2.tool_call_index, 0,
            "Sequential completion stream must reset tool_call_index to 0"
        );
    }

    #[test]
    fn test_gemini_function_response_primitive_wrapped_as_object() {
        // 1. Boolean primitive "true"
        let req_bool = ChatCompletionRequest {
            model: "ag/gemini-3.8-flash-high".to_string(),
            messages: vec![
                ChatMessage::user("Check server status"),
                ChatMessage {
                    role: "assistant".to_string(),
                    content: None,
                    name: None,
                    tool_call_id: None,
                    tool_calls: Some(serde_json::json!([{
                        "id": "call_1:sig1",
                        "type": "function",
                        "thought_signature": "sig1",
                        "function": {
                            "name": "check_status",
                            "arguments": "{}"
                        }
                    }])),
                },
                ChatMessage {
                    role: "tool".to_string(),
                    content: Some(MessageContent::Text("true".to_string())),
                    name: Some("check_status".to_string()),
                    tool_call_id: Some("call_1:sig1".to_string()),
                    tool_calls: None,
                },
            ],
            ..Default::default()
        };

        let payload_bool = build_antigravity_payload(&req_bool);
        let contents_bool = payload_bool["request"]["contents"].as_array().unwrap();
        let fr_bool = &contents_bool[2]["parts"][0]["functionResponse"]["response"];
        assert!(fr_bool.is_object());
        assert_eq!(fr_bool["output"], true);

        // 2. Array primitive "[1, 2, 3]"
        let req_arr = ChatCompletionRequest {
            model: "ag/gemini-3.8-flash-high".to_string(),
            messages: vec![
                ChatMessage::user("Get numbers"),
                ChatMessage {
                    role: "assistant".to_string(),
                    content: None,
                    name: None,
                    tool_call_id: None,
                    tool_calls: Some(serde_json::json!([{
                        "id": "call_2:sig2",
                        "type": "function",
                        "thought_signature": "sig2",
                        "function": {
                            "name": "get_numbers",
                            "arguments": "{}"
                        }
                    }])),
                },
                ChatMessage {
                    role: "tool".to_string(),
                    content: Some(MessageContent::Text("[1, 2, 3]".to_string())),
                    name: Some("get_numbers".to_string()),
                    tool_call_id: Some("call_2:sig2".to_string()),
                    tool_calls: None,
                },
            ],
            ..Default::default()
        };

        let payload_arr = build_antigravity_payload(&req_arr);
        let contents_arr = payload_arr["request"]["contents"].as_array().unwrap();
        let fr_arr = &contents_arr[2]["parts"][0]["functionResponse"]["response"];
        assert!(fr_arr.is_object());
        assert_eq!(fr_arr["output"], serde_json::json!([1, 2, 3]));

        // 3. Object string "{\"success\": true}" -> remains intact
        let req_obj = ChatCompletionRequest {
            model: "ag/gemini-3.8-flash-high".to_string(),
            messages: vec![
                ChatMessage::user("Check health"),
                ChatMessage {
                    role: "assistant".to_string(),
                    content: None,
                    name: None,
                    tool_call_id: None,
                    tool_calls: Some(serde_json::json!([{
                        "id": "call_3:sig3",
                        "type": "function",
                        "thought_signature": "sig3",
                        "function": {
                            "name": "health",
                            "arguments": "{}"
                        }
                    }])),
                },
                ChatMessage {
                    role: "tool".to_string(),
                    content: Some(MessageContent::Text("{\"success\": true}".to_string())),
                    name: Some("health".to_string()),
                    tool_call_id: Some("call_3:sig3".to_string()),
                    tool_calls: None,
                },
            ],
            ..Default::default()
        };

        let payload_obj = build_antigravity_payload(&req_obj);
        let contents_obj = payload_obj["request"]["contents"].as_array().unwrap();
        let fr_obj = &contents_obj[2]["parts"][0]["functionResponse"]["response"];
        assert!(fr_obj.is_object());
        assert_eq!(fr_obj["success"], true);
        assert!(fr_obj.get("output").is_none());
    }

    #[tokio::test]
    async fn test_gemini_sse_stream_with_requested_model() {
        use futures_util::stream;
        use futures_util::StreamExt;

        let gemini_json = serde_json::json!({
            "candidates": [{
                "content": {
                    "parts": [{ "text": "Hello world" }]
                }
            }]
        });
        let sse_line = format!("data: {}\n\n", serde_json::to_string(&gemini_json).unwrap());
        let stream_mock: BoxChatStream = Box::pin(stream::iter(vec![Ok(Bytes::from(sse_line))]));

        let mut sse_stream = GeminiSseStream::new(
            stream_mock,
            "ag/gemini-2.5-pro".to_string(),
            "chatcmpl-test".to_string(),
            chrono::Utc::now().timestamp(),
            None,
        )
        .with_requested_model("my-combo-model".to_string());

        let res = sse_stream.next().await;
        assert!(res.is_some());
        let chunk_bytes = res.unwrap().unwrap();
        let chunk_str = std::str::from_utf8(&chunk_bytes).unwrap();
        assert!(chunk_str.contains("\"model\":\"my-combo-model\""));
        assert!(!chunk_str.contains("ag/gemini-2.5-pro"));
    }

    #[test]
    fn test_http_upstream_model_prefix_strip_logic() {
        let mut req = ChatCompletionRequest {
            model: "groq/llama-3.3-70b".to_string(),
            messages: vec![ChatMessage::new("user", "hi")],
            ..Default::default()
        };

        if let Some(pos) = req.model.find('/') {
            req.model = req.model[pos + 1..].to_string();
        }
        assert_eq!(req.model, "llama-3.3-70b");

        let mut req_no_prefix = ChatCompletionRequest {
            model: "llama-3.3-70b".to_string(),
            messages: vec![ChatMessage::new("user", "hi")],
            ..Default::default()
        };
        if let Some(pos) = req_no_prefix.model.find('/') {
            req_no_prefix.model = req_no_prefix.model[pos + 1..].to_string();
        }
        assert_eq!(req_no_prefix.model, "llama-3.3-70b");
    }

    #[test]
    fn test_extract_text_tool_calls_single_and_multiple() {
        // Single call with arguments
        let text = "Tôi sẽ kiểm tra code.[Tool Call: execute_code({\"code\":\"print('hello')\",\"reset\":false})]";
        let (cleaned, calls) = extract_text_tool_calls(text);
        assert_eq!(cleaned, "Tôi sẽ kiểm tra code.");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0]["function"]["name"], "execute_code");
        let args: serde_json::Value =
            serde_json::from_str(calls[0]["function"]["arguments"].as_str().unwrap()).unwrap();
        assert_eq!(args["code"], "print('hello')");
        assert_eq!(args["reset"], false);

        // Multiple calls
        let multi = "[Tool Call: read_file({\"path\":\"a.txt\"})]\n[Tool Call: read_file({\"path\":\"b.txt\"})]";
        let (cleaned_multi, calls_multi) = extract_text_tool_calls(multi);
        assert_eq!(cleaned_multi, "");
        assert_eq!(calls_multi.len(), 2);
        assert_eq!(calls_multi[0]["function"]["name"], "read_file");
        assert_eq!(calls_multi[1]["function"]["name"], "read_file");
    }

    #[test]
    fn test_extract_text_tool_calls_complex_escaping_and_parens() {
        // Nested parentheses and escaped quotes inside code string
        let complex = "[Tool Call: execute_code({\"code\":\"def foo():\\n    print(\\\"bar()\\\")\\n\",\"reset\":false})]";
        let (cleaned, calls) = extract_text_tool_calls(complex);
        assert_eq!(cleaned, "");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0]["function"]["name"], "execute_code");
        let args: serde_json::Value =
            serde_json::from_str(calls[0]["function"]["arguments"].as_str().unwrap()).unwrap();
        assert!(args["code"].as_str().unwrap().contains("bar()"));
    }

    #[test]
    fn test_extract_text_tool_calls_real_incident_samples() {
        // Real sample from session bebcbf6f8bb0 message 60 using Rust raw string
        let msg60 = concat!(
            "Tôi sẽ kiểm tra lại test suite.",
            r#"[Tool Call: execute_code({"code":"from hermes_tools import terminal\ncmd = \"\"\"set -e\ncd client\nnpm test\nnode --test tests/*.test.mjs\n\"\"\"\nr = terminal(cmd, workdir=\"/workspace/project\", timeout=120)\nprint(r['output'])\nprint('exit_code=', r['exit_code'])","reset":false})]"#
        );
        let (cleaned60, calls60) = extract_text_tool_calls(msg60);
        assert_eq!(cleaned60, "Tôi sẽ kiểm tra lại test suite.");
        assert_eq!(calls60.len(), 1);
        assert_eq!(calls60[0]["function"]["name"], "execute_code");

        // Real sample from session bebcbf6f8bb0 message 62 using Rust raw string
        let msg62 = concat!(
            "Đang làm đúng theo Task 1.**Step 1: Viết failing test**",
            r#"[Tool Call: write_file({"content":"import assert from 'node:assert/strict'\nconst NOW = 1000;\n","path":"/workspace/project/client/src/utils/renewalEligibility.test.js"})]"#
        );
        let (cleaned62, calls62) = extract_text_tool_calls(msg62);
        assert_eq!(
            cleaned62,
            "Đang làm đúng theo Task 1.**Step 1: Viết failing test**"
        );
        assert_eq!(calls62.len(), 1);
        assert_eq!(calls62[0]["function"]["name"], "write_file");
        let args62: serde_json::Value =
            serde_json::from_str(calls62[0]["function"]["arguments"].as_str().unwrap()).unwrap();
        assert_eq!(
            args62["path"],
            "/workspace/project/client/src/utils/renewalEligibility.test.js"
        );
    }

    #[test]
    fn test_parse_antigravity_response_body_text_tool_call_recovery() {
        let gemini_json = serde_json::json!({
            "response": {
                "candidates": [{
                    "content": {
                        "parts": [{
                            "text": "Tôi sẽ chạy lệnh sau:[Tool Call: terminal({\"command\":\"ls -la\"})]"
                        }],
                        "role": "model"
                    },
                    "finishReason": "STOP"
                }],
                "usageMetadata": {
                    "promptTokenCount": 50,
                    "candidatesTokenCount": 20
                }
            }
        });

        let resp = parse_antigravity_response_body(
            &serde_json::to_vec(&gemini_json).unwrap(),
            "ag/gemini-3.8-flash-high",
            50,
        )
        .unwrap();

        assert_eq!(resp.choices.len(), 1);
        assert_eq!(resp.choices[0].finish_reason, "tool_calls");
        assert_eq!(
            resp.choices[0].message.content.as_deref(),
            Some("Tôi sẽ chạy lệnh sau:")
        );
        let tc = resp.choices[0]
            .message
            .tool_calls
            .as_ref()
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(tc.len(), 1);
        assert_eq!(tc[0]["function"]["name"], "terminal");
        assert_eq!(tc[0]["function"]["arguments"], "{\"command\":\"ls -la\"}");
    }

    #[tokio::test]
    async fn test_gemini_sse_stream_text_tool_call_recovery() {
        use futures_util::stream;
        use futures_util::StreamExt;

        let gemini_chunk1 = serde_json::json!({
            "response": {
                "candidates": [{
                    "content": {
                        "parts": [{
                            "text": "Đang chạy test.[Tool Call: execute_code({\"code\":\"run_tests()\"})]"
                        }]
                    }
                }]
            }
        });
        let gemini_chunk2 = serde_json::json!({
            "response": {
                "candidates": [{
                    "finishReason": "STOP"
                }]
            }
        });

        let line1 = format!(
            "data: {}\n\n",
            serde_json::to_string(&gemini_chunk1).unwrap()
        );
        let line2 = format!(
            "data: {}\n\n",
            serde_json::to_string(&gemini_chunk2).unwrap()
        );
        let stream_mock: BoxChatStream = Box::pin(stream::iter(vec![
            Ok(Bytes::from(line1)),
            Ok(Bytes::from(line2)),
        ]));

        let mut sse_stream = GeminiSseStream::new(
            stream_mock,
            "ag/gemini-3.8-flash-high".to_string(),
            "chatcmpl-stream-tc-rec".to_string(),
            chrono::Utc::now().timestamp(),
            None,
        );

        let mut chunks = Vec::new();
        while let Some(chunk_res) = sse_stream.next().await {
            let bytes = chunk_res.unwrap();
            let text = String::from_utf8(bytes.to_vec()).unwrap();
            chunks.push(text);
        }

        // Verify that tool_calls delta and finish_reason: "tool_calls" were emitted
        let full_output = chunks.join("");
        assert!(full_output.contains("\"tool_calls\""));
        assert!(full_output.contains("\"name\":\"execute_code\""));
        assert!(full_output.contains("\"arguments\":\"{\\\"code\\\":\\\"run_tests()\\\"}\""));
        assert!(full_output.contains("\"finish_reason\":\"tool_calls\""));
    }

    #[test]
    fn test_antigravity_degraded_prompt_guard_instruction() {
        // Historical unsigned tool call causes prompt guard injection
        let req = ChatCompletionRequest {
            model: "ag/gemini-3.8-flash-high".to_string(),
            messages: vec![
                ChatMessage::user("Calculate this"),
                ChatMessage {
                    role: "assistant".to_string(),
                    content: None,
                    name: None,
                    tool_call_id: None,
                    tool_calls: Some(serde_json::json!([{
                        "id": "call_unsigned_xyz",
                        "type": "function",
                        "function": {
                            "name": "calc",
                            "arguments": "{}"
                        }
                    }])),
                },
                ChatMessage {
                    role: "tool".to_string(),
                    content: Some(MessageContent::Text("42".to_string())),
                    name: Some("calc".to_string()),
                    tool_call_id: Some("call_unsigned_xyz".to_string()),
                    tool_calls: None,
                },
                ChatMessage::user("Now do step 2"),
            ],
            ..Default::default()
        };

        let payload = build_antigravity_payload(&req);
        let sys_instruction = payload["request"]["systemInstruction"]["parts"][0]["text"]
            .as_str()
            .unwrap();
        assert!(sys_instruction.contains("CRITICAL INSTRUCTION FOR TOOL USE"));
        assert!(sys_instruction.contains("MUST NOT output text markers"));
        assert!(sys_instruction.contains("MUST emit native structured function calls"));
    }
}
