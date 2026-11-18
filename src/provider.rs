use std::pin::Pin;
use std::sync::Mutex;
use std::time::Duration;

use bytes::Bytes;
use futures_util::stream::Stream;
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::error::AppError;


pub const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024; // 16 MB bounded response limit

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
    // Omitted rather than sent as `null`: several OpenAI-compatible upstreams
    // reject `"stream": null` with a 400 even though the field is optional.
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum EmbeddingInput {
    String(String),
    StringArray(Vec<String>),
}

impl EmbeddingInput {
    pub fn to_vec(&self) -> Vec<String> {
        match self {
            Self::String(s) => vec![s.clone()],
            Self::StringArray(arr) => arr.clone(),
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Self::String(_) => 1,
            Self::StringArray(arr) => arr.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Self::String(s) => s.trim().is_empty(),
            Self::StringArray(arr) => arr.is_empty() || arr.iter().all(|s| s.trim().is_empty()),
        }
    }
}

fn default_encoding_format() -> String {
    "float".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EmbeddingRequest {
    pub input: EmbeddingInput,
    pub model: String,
    #[serde(default = "default_encoding_format")]
    pub encoding_format: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EmbeddingData {
    pub object: String,
    pub index: usize,
    pub embedding: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct EmbeddingUsage {
    pub prompt_tokens: usize,
    pub total_tokens: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EmbeddingResponse {
    pub object: String,
    pub data: Vec<EmbeddingData>,
    pub model: String,
    pub usage: EmbeddingUsage,
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

    async fn embed(&self, request: &EmbeddingRequest) -> Result<EmbeddingResponse, AppError> {
        let _ = request;
        Err(AppError::BadRequest(
            "Embeddings are not supported by this provider".to_string(),
        ))
    }
}

/// Strips the provider's own routing prefix from a model id before dispatching upstream.
///
/// Only the prefix matching THIS provider's configured `prefix` is removed, e.g. provider
/// `amd` turns `amd/DeepSeek-V4-Flash` into `DeepSeek-V4-Flash`, while provider `nvidia2`
/// turns `nvidia2/nvidia/nemotron-...` into `nvidia/nemotron-...` — preserving the model's
/// own `nvidia/` namespace, which upstreams like NVIDIA require. (The old logic blindly
/// stripped the first `/` segment, which mangled namespaced ids and caused upstream 404s.)
///
/// Models that don't start with the provider prefix, missing or empty prefixes, and
/// empty remainders are all returned unchanged.
fn strip_provider_prefix(model: &str, prefix: Option<&str>) -> String {
    let prefix = match prefix {
        Some(p) if !p.is_empty() => p,
        _ => return model.to_string(),
    };
    let prefixed = format!("{prefix}/");
    match model.strip_prefix(&prefixed) {
        Some(stripped) if !stripped.is_empty() => stripped.to_string(),
        _ => model.to_string(),
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

/// Resolves the upstream endpoint for embeddings, avoiding duplicate `/v1`.
pub fn resolve_embeddings_url(base_url: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.ends_with("/v1") {
        format!("{trimmed}/embeddings")
    } else {
        format!("{trimmed}/v1/embeddings")
    }
}

/// Resolves the upstream endpoint for image generations, avoiding duplicate `/v1`.
pub fn resolve_image_generations_url(base_url: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.ends_with("/v1") {
        format!("{trimmed}/images/generations")
    } else {
        format!("{trimmed}/v1/images/generations")
    }
}

/// Resolves the upstream endpoint for image models, avoiding duplicate `/v1`.
pub fn resolve_image_models_url(base_url: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.ends_with("/v1") {
        format!("{trimmed}/images/models")
    } else {
        format!("{trimmed}/v1/images/models")
    }
}

pub fn format_reqwest_stream_error(err: &reqwest::Error) -> AppError {
    let mut details = err.to_string();
    let mut curr: &dyn std::error::Error = err;
    while let Some(src) = curr.source() {
        details.push_str(&format!(" -> {src}"));
        curr = src;
    }
    let lower = details.to_lowercase();
    if err.is_timeout() || lower.contains("timed out") || lower.contains("timeout") {
        AppError::GatewayTimeout(format!("Upstream stream read timed out: {details}"))
    } else {
        AppError::BadGateway(format!("Upstream stream error: {details}"))
    }
}

pub struct HttpUpstreamProvider {
    base_url: Option<String>,
    api_key: Option<String>,
    prefix: Option<String>,
    client: Client,
    max_response_bytes: usize,
}

impl std::fmt::Debug for HttpUpstreamProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpUpstreamProvider")
            .field("base_url", &self.base_url)
            .field("api_key", &self.api_key.as_ref().map(|_| "[REDACTED]"))
            .field("prefix", &self.prefix)
            .field("max_response_bytes", &self.max_response_bytes)
            .finish()
    }
}

impl HttpUpstreamProvider {
    pub fn new(
        base_url: Option<String>,
        api_key: Option<String>,
        prefix: Option<String>,
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
            prefix,
            client,
            max_response_bytes: MAX_RESPONSE_BYTES,
        }
    }

    pub fn with_client(base_url: Option<String>, api_key: Option<String>, prefix: Option<String>, client: Client) -> Self {
        Self {
            base_url,
            api_key,
            prefix,
            client,
            max_response_bytes: MAX_RESPONSE_BYTES,
        }
    }

    pub fn with_max_response_bytes(mut self, max_bytes: usize) -> Self {
        self.max_response_bytes = max_bytes;
        self
    }

    pub async fn generate_image(
        &self,
        request: &serde_json::Value,
        routing_prefix: Option<&str>,
    ) -> Result<serde_json::Value, AppError> {
        let base_url = self
            .base_url
            .as_deref()
            .ok_or_else(|| AppError::Internal("Upstream base URL is not configured".to_string()))?;
        let url = resolve_image_generations_url(base_url);
        let mut upstream_request = request.clone();
        let model = upstream_request
            .get("model")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| AppError::BadRequest("model must be a non-empty string".to_string()))?
            .to_string();
        if let Some(upstream_model) = routing_prefix
            .filter(|prefix| !prefix.is_empty())
            .and_then(|prefix| model.strip_prefix(prefix))
            .and_then(|model| model.strip_prefix('/'))
        {
            upstream_request["model"] = serde_json::Value::String(upstream_model.to_string());
        }

        tracing::info!(
            upstream_url = %url,
            model = %upstream_request["model"].as_str().unwrap_or(&model),
            "Dispatching image generation request to upstream provider"
        );

        let mut request_builder = self.client.post(&url).json(&upstream_request);
        if let Some(key) = self.api_key.as_ref().filter(|key| !key.trim().is_empty()) {
            request_builder =
                request_builder.header(reqwest::header::AUTHORIZATION, format!("Bearer {key}"));
        }
        let response = request_builder.send().await.map_err(|err| {
            if err.is_timeout() {
                AppError::GatewayTimeout(format!("Upstream image request timed out: {err}"))
            } else if err.is_connect() {
                AppError::BadGateway(format!("Upstream image connection failed: {err}"))
            } else {
                AppError::BadGateway(format!("Upstream image request failed: {err}"))
            }
        })?;
        let status = response.status();
        if response
            .content_length()
            .is_some_and(|length| length > self.max_response_bytes as u64)
        {
            return Err(AppError::BadGateway(format!(
                "Upstream response exceeded limit of {} bytes",
                self.max_response_bytes
            )));
        }

        let mut body = Vec::new();
        let mut response = response;
        while let Some(chunk) = response.chunk().await.map_err(|err| {
            if err.is_timeout() {
                AppError::GatewayTimeout(format!("Upstream image response read timed out: {err}"))
            } else {
                AppError::BadGateway(format!(
                    "Failed to read upstream image response body: {err}"
                ))
            }
        })? {
            if body.len() + chunk.len() > self.max_response_bytes {
                return Err(AppError::BadGateway(format!(
                    "Upstream response exceeded limit of {} bytes",
                    self.max_response_bytes
                )));
            }
            body.extend_from_slice(&chunk);
        }

        if !status.is_success() {
            let message = match serde_json::from_slice::<serde_json::Value>(&body) {
                Ok(value) => value
                    .pointer("/error/message")
                    .or_else(|| value.get("detail"))
                    .and_then(serde_json::Value::as_str)
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| value.to_string().chars().take(500).collect()),
                Err(_) => {
                    let text = String::from_utf8_lossy(&body);
                    if text.trim().is_empty() {
                        format!("Upstream returned HTTP status {status}")
                    } else {
                        text.chars().take(500).collect()
                    }
                }
            };
            return match status.as_u16() {
                400 => Err(AppError::BadRequest(message)),
                404 => Err(AppError::NotFound(message)),
                408 | 504 => Err(AppError::GatewayTimeout(message)),
                _ => Err(AppError::BadGateway(format!(
                    "Upstream error ({status}): {message}"
                ))),
            };
        }

        serde_json::from_slice(&body).map_err(|err| {
            AppError::BadGateway(format!("Failed to parse upstream image response: {err}"))
        })
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
        req_to_send.model =
            strip_provider_prefix(&request.model, self.prefix.as_deref());

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
        upstream_req.model =
            strip_provider_prefix(&request.model, self.prefix.as_deref());
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

        let stream = response
            .bytes_stream()
            .map(|chunk_result| chunk_result.map_err(|err| format_reqwest_stream_error(&err)));

        Ok(Box::pin(stream))
    }

    async fn embed(&self, request: &EmbeddingRequest) -> Result<EmbeddingResponse, AppError> {
        let base_url = self
            .base_url
            .as_deref()
            .ok_or_else(|| AppError::Internal("Upstream base URL is not configured".to_string()))?;

        let url = resolve_embeddings_url(base_url);

        let mut req_to_send = request.clone();
        req_to_send.model =
            strip_provider_prefix(&request.model, self.prefix.as_deref());

        tracing::info!(
            upstream_url = %url,
            model = %req_to_send.model,
            items_count = req_to_send.input.len(),
            dimensions = ?req_to_send.dimensions,
            "Dispatching embedding request to upstream provider"
        );

        let mut req_builder = self.client.post(&url).json(&req_to_send);

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
                        "Upstream embedding request timed out: {err}"
                    )));
                }
                if err.is_connect() {
                    return Err(AppError::BadGateway(format!(
                        "Upstream embedding connection failed: {err}"
                    )));
                }
                return Err(AppError::BadGateway(format!(
                    "Upstream embedding request failed: {err}"
                )));
            }
        };

        let status = response.status();
        let body_bytes = match response.bytes().await {
            Ok(b) => b,
            Err(err) => {
                return Err(AppError::BadGateway(format!(
                    "Failed to read upstream embedding response body: {err}"
                )));
            }
        };

        if !status.is_success() {
            let body_str = String::from_utf8_lossy(&body_bytes);
            let error_msg = format!("Upstream embedding error (status {status}): {body_str}");
            tracing::warn!(status = %status, body = %body_str, "Upstream embedding returned error");

            return match status.as_u16() {
                400 => Err(AppError::BadRequest(error_msg)),
                401 => Err(AppError::Unauthorized(error_msg)),
                403 => Err(AppError::Forbidden(error_msg)),
                404 => Err(AppError::NotFound(error_msg)),
                429 | 402 => Err(AppError::BadRequest(error_msg)),
                504 => Err(AppError::GatewayTimeout(error_msg)),
                _ => Err(AppError::BadGateway(error_msg)),
            };
        }

        let mut parsed: EmbeddingResponse = serde_json::from_slice(&body_bytes).map_err(|err| {
            let snippet = String::from_utf8_lossy(&body_bytes[..body_bytes.len().min(500)]);
            AppError::BadGateway(format!(
                "Failed to parse upstream embedding response as EmbeddingResponse: {err}. Body: {snippet}"
            ))
        })?;

        if let Some(requested_dims) = request.dimensions {
            for item in &mut parsed.data {
                if item.embedding.len() > requested_dims {
                    item.embedding.truncate(requested_dims);
                }
            }
        }

        Ok(parsed)
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

    async fn embed(&self, request: &EmbeddingRequest) -> Result<EmbeddingResponse, AppError> {
        let dims = request.dimensions.unwrap_or(768);
        let texts = request.input.to_vec();
        let mut data = Vec::with_capacity(texts.len());
        let mut total_chars = 0;

        for (idx, text) in texts.iter().enumerate() {
            total_chars += text.len();
            let mut embedding = Vec::with_capacity(dims);
            for d in 0..dims {
                let val = (((d + idx * 7) as f32) * 0.001).sin();
                embedding.push(val);
            }
            data.push(EmbeddingData {
                object: "embedding".to_string(),
                index: idx,
                embedding,
            });
        }

        let prompt_tokens = (total_chars / 4).max(1);
        Ok(EmbeddingResponse {
            object: "list".to_string(),
            data,
            model: request.model.clone(),
            usage: EmbeddingUsage {
                prompt_tokens,
                total_tokens: prompt_tokens,
            },
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


    #[test]
    fn test_resolve_embeddings_url() {
        assert_eq!(
            resolve_embeddings_url("https://api.openai.com"),
            "https://api.openai.com/v1/embeddings"
        );
        assert_eq!(
            resolve_embeddings_url("https://api.openai.com/"),
            "https://api.openai.com/v1/embeddings"
        );
        assert_eq!(
            resolve_embeddings_url("https://api.openai.com/v1"),
            "https://api.openai.com/v1/embeddings"
        );
        assert_eq!(
            resolve_embeddings_url("https://api.openai.com/v1/"),
            "https://api.openai.com/v1/embeddings"
        );
        assert_eq!(
            resolve_embeddings_url("https://openrouter.ai/api/v1"),
            "https://openrouter.ai/api/v1/embeddings"
        );
        assert_eq!(
            resolve_embeddings_url("http://127.0.0.1:11434"),
            "http://127.0.0.1:11434/v1/embeddings"
        );
        assert_eq!(
            resolve_embeddings_url("http://127.0.0.1:11434/v1"),
            "http://127.0.0.1:11434/v1/embeddings"
        );
    }


    #[test]
    fn test_resolve_image_models_and_generations_url() {
        assert_eq!(
            resolve_image_generations_url("https://openrouter.ai/api/v1"),
            "https://openrouter.ai/api/v1/images/generations"
        );
        assert_eq!(
            resolve_image_generations_url("https://openrouter.ai/api/v1/"),
            "https://openrouter.ai/api/v1/images/generations"
        );
        assert_eq!(
            resolve_image_generations_url("https://api.openai.com"),
            "https://api.openai.com/v1/images/generations"
        );
        assert_eq!(
            resolve_image_models_url("https://openrouter.ai/api/v1"),
            "https://openrouter.ai/api/v1/images/models"
        );
        assert_eq!(
            resolve_image_models_url("https://openrouter.ai/api/v1/"),
            "https://openrouter.ai/api/v1/images/models"
        );
        assert_eq!(
            resolve_image_models_url("https://api.openai.com"),
            "https://api.openai.com/v1/images/models"
        );
    }


    #[test]
    fn test_embedding_serde_single_and_batch() {
        let single_json = r#"{
            "model": "text-embedding-004",
            "input": "hello world",
            "dimensions": 768
        }"#;
        let req1: EmbeddingRequest = serde_json::from_str(single_json).unwrap();
        assert_eq!(req1.model, "text-embedding-004");
        assert_eq!(req1.dimensions, Some(768));
        assert_eq!(req1.encoding_format, "float");
        assert_eq!(req1.input.to_vec(), vec!["hello world".to_string()]);

        let batch_json = r#"{
            "model": "openrouter/google/gemini-embedding-001",
            "input": ["item 1", "item 2", "item 3"]
        }"#;
        let req2: EmbeddingRequest = serde_json::from_str(batch_json).unwrap();
        assert_eq!(req2.model, "openrouter/google/gemini-embedding-001");
        assert_eq!(req2.dimensions, None);
        assert_eq!(req2.input.len(), 3);
        assert_eq!(req2.input.to_vec(), vec!["item 1", "item 2", "item 3"]);
    }

    #[tokio::test]
    async fn test_mock_provider_embed() {
        let mock = MockProvider::new();
        let req = EmbeddingRequest {
            model: "text-embedding-004".to_string(),
            input: EmbeddingInput::StringArray(vec!["text A".to_string(), "text B".to_string()]),
            encoding_format: "float".to_string(),
            dimensions: Some(768),
            user: None,
        };

        let resp = mock.embed(&req).await.unwrap();
        assert_eq!(resp.object, "list");
        assert_eq!(resp.data.len(), 2);
        assert_eq!(resp.data[0].index, 0);
        assert_eq!(resp.data[0].embedding.len(), 768);
        assert_eq!(resp.data[1].index, 1);
        assert_eq!(resp.data[1].embedding.len(), 768);
        assert_eq!(resp.model, "text-embedding-004");
        assert!(resp.usage.prompt_tokens > 0);
    }

    #[tokio::test]
    async fn test_mock_provider_deterministic_response() {
        let mock = MockProvider::new();
        let req = ChatCompletionRequest {
            model: "gemini/gemini-2.5-flash".to_string(),
            messages: vec![ChatMessage::new("user", "Hello mock")],
            stream: Some(false),
            ..Default::default()
        };

        let resp = mock.complete(&req).await.unwrap();
        assert_eq!(resp.id, "chatcmpl-mock-deterministic");
        assert_eq!(resp.object, "chat.completion");
        assert_eq!(resp.created, 1700000000);
        assert_eq!(resp.model, "gemini/gemini-2.5-flash");
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
        assert_eq!(last.model, "gemini/gemini-2.5-flash");
    }


    #[test]
    fn test_serde_chat_completion_request_with_tools() {
        let json_data = serde_json::json!({
            "model": "anthropic/claude-3-5-sonnet",
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
        assert_eq!(req.model, "anthropic/claude-3-5-sonnet");
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
            None,
            5,
            10,
            15,
        );
        let debug_str = format!("{provider:?}");
        assert!(!debug_str.contains("super-secret-key-12345"));
        assert!(debug_str.contains("[REDACTED]"));
        assert!(debug_str.contains("https://api.example.com"));
    }


    #[test]
    fn test_strip_provider_prefix_strips_own_prefix() {
        // Provider `amd`: bare model id is what tokenlab expects upstream.
        assert_eq!(
            strip_provider_prefix("amd/DeepSeek-V4-Flash", Some("amd")),
            "DeepSeek-V4-Flash"
        );
        // Provider `groq`: Groq's model id keeps its `qwen/` namespace.
        assert_eq!(
            strip_provider_prefix("groq/qwen/qwen3.8-27b", Some("groq")),
            "qwen/qwen3.8-27b"
        );
    }


    #[test]
    fn test_strip_provider_prefix_preserves_model_namespace() {
        // REGRESSION: the old logic blindly stripped the first `/` segment, turning
        // `nvidia2/nvidia/nemotron-...` into `nemotron-...` for the `nvidia` provider
        // (upstream 404). The new logic must only strip the provider's OWN prefix.
        assert_eq!(
            strip_provider_prefix(
                "nvidia2/nvidia/nemotron-3-ultra-550b-a55b",
                Some("nvidia2")
            ),
            "nvidia/nemotron-3-ultra-550b-a55b"
        );
    }


    #[test]
    fn test_strip_provider_prefix_leaves_unrelated_models_unchanged() {
        // Model does not start with this provider's prefix -> untouched.
        assert_eq!(strip_provider_prefix("foo/bar", Some("amd")), "foo/bar");
        assert_eq!(strip_provider_prefix("bare-model", Some("amd")), "bare-model");
        // Similar-but-different prefix must NOT be stripped (no partial matches).
        assert_eq!(strip_provider_prefix("amd2/model", Some("amd")), "amd2/model");
    }


    #[test]
    fn test_strip_provider_prefix_no_prefix_configured() {
        assert_eq!(strip_provider_prefix("amd/model", None), "amd/model");
        assert_eq!(strip_provider_prefix("amd/model", Some("")), "amd/model");
    }


    #[test]
    fn test_strip_provider_prefix_empty_remainder_unchanged() {
        // `<prefix>/` alone would strip to an empty model id -> keep original.
        assert_eq!(strip_provider_prefix("amd/", Some("amd")), "amd/");
    }

    #[tokio::test]
    async fn test_http_provider_missing_base_url() {
        let provider = HttpUpstreamProvider::new(None, None, None, 5, 10, 15);
        let req = ChatCompletionRequest {
            model: "gemini/gemini-2.5-flash".to_string(),
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
            model: "gemini/gemini-2.5-flash".to_string(),
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
        assert_eq!(chunk0.model, "gemini/gemini-2.5-flash");
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
        assert_eq!(last.model, "gemini/gemini-2.5-flash");
    }

    #[tokio::test]
    async fn test_http_provider_missing_base_url_streaming() {
        let provider = HttpUpstreamProvider::new(None, None, None, 5, 10, 15);
        let req = ChatCompletionRequest {
            model: "gemini/gemini-2.5-flash".to_string(),
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
    fn test_optional_stream_is_omitted_not_null_when_absent() {
        // A client that omits `stream` must not send `"stream": null` upstream:
        // several OpenAI-compatible providers reject that with a 400.
        let req = ChatCompletionRequest {
            model: "groq/llama-3.3-70b".to_string(),
            messages: vec![ChatMessage::new("user", "hi")],
            ..Default::default()
        };

        let serialized = serde_json::to_value(&req).unwrap();
        assert!(
            serialized.get("stream").is_none(),
            "stream must be omitted when unset, got {serialized}"
        );

        // An explicit value must still be forwarded verbatim.
        let streaming = ChatCompletionRequest {
            stream: Some(true),
            ..req.clone()
        };
        assert_eq!(
            serde_json::to_value(&streaming).unwrap()["stream"],
            serde_json::json!(true)
        );

        let non_streaming = ChatCompletionRequest {
            stream: Some(false),
            ..req
        };
        assert_eq!(
            serde_json::to_value(&non_streaming).unwrap()["stream"],
            serde_json::json!(false)
        );
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
}
