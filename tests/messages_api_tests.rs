use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use ezrouter::{app_router, AppState, Config};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

fn test_state(mock: bool) -> (AppState, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("ezrouter-messages-test-{}", uuid::Uuid::new_v4()));
    let mut config = Config::parse(
        Some("127.0.0.1".to_string()),
        Some("20229".to_string()),
        Some(dir.to_string_lossy().into_owned()),
        Some("test-secret-key".to_string()),
    )
    .unwrap();
    config.use_mock_provider = mock;
    (AppState::new(config), dir)
}

#[tokio::test]
async fn test_post_messages_requires_auth() {
    let (state, dir) = test_state(true);
    let app = app_router(state);

    let req = Request::builder()
        .method("POST")
        .uri("/v1/messages")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"model":"claude-3-5-sonnet-20241022","messages":[{"role":"user","content":"hello"}]}"#))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn test_post_messages_with_x_api_key_header() {
    let (state, dir) = test_state(true);
    let app = app_router(state);

    let req = Request::builder()
        .method("POST")
        .uri("/v1/messages")
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-api-key", "test-secret-key")
        .body(Body::from(r#"{"model":"claude-3-5-sonnet-20241022","messages":[{"role":"user","content":"hello"}]}"#))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["type"], "message");
    assert_eq!(json["role"], "assistant");
    assert!(json["content"].is_array());
    assert_eq!(json["stop_reason"], "end_turn");

    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn test_post_messages_streaming_mock() {
    let (state, dir) = test_state(true);
    let app = app_router(state);

    let req = Request::builder()
        .method("POST")
        .uri("/v1/messages")
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-api-key", "test-secret-key")
        .body(Body::from(r#"{"model":"claude-3-7-sonnet","stream":true,"messages":[{"role":"user","content":"write code"}]}"#))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get(header::CONTENT_TYPE).unwrap(),
        "text/event-stream; charset=utf-8"
    );

    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&bytes);

    assert!(body_str.contains("event: message_start"));
    assert!(body_str.contains("event: content_block_start"));
    assert!(body_str.contains("event: content_block_delta"));
    assert!(body_str.contains("event: content_block_stop"));
    assert!(body_str.contains("event: message_delta"));
    assert!(body_str.contains("event: message_stop"));

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_normalize_anthropic_model_aliases() {
    use ezrouter::routes::messages::normalize_anthropic_model;

    assert_eq!(
        normalize_anthropic_model("claude-3-5-sonnet-20241022"),
        "ag/claude-sonnet-5-5-high"
    );
    assert_eq!(
        normalize_anthropic_model("claude-3-7-sonnet"),
        "ag/claude-sonnet-5-5-high"
    );
    assert_eq!(
        normalize_anthropic_model("claude-sonnet-4-20250514"),
        "ag/claude-sonnet-5-5-high"
    );
    assert_eq!(
        normalize_anthropic_model("claude-3-opus-20240229"),
        "ag/claude-opus-5-5-high"
    );
    assert_eq!(
        normalize_anthropic_model("claude-3-5-haiku-20241022"),
        "ag/gemini-3.8-flash-high"
    );
    assert_eq!(
        normalize_anthropic_model("ag/claude-sonnet-5-5-high"),
        "ag/claude-sonnet-5-5-high"
    );
    assert_eq!(
        normalize_anthropic_model("cx/gpt-6.1-sol"),
        "cx/gpt-6.1-sol"
    );
}

#[tokio::test]
async fn test_serve_claude_code_scripts() {
    let (state, dir) = test_state(false);
    let app = app_router(state);

    for path in &[
        "/setup-claude-code.sh",
        "/setup-claude-code.bat",
        "/setup-claude-code.ps1",
        "/uninstall-claude-code.sh",
        "/uninstall-claude-code.bat",
        "/uninstall-claude-code.ps1",
    ] {
        let req = Request::builder().uri(*path).body(Body::empty()).unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK, "Failed for {}", path);
    }

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_anthropic_to_openai_request_with_image_in_user_and_tool_result() {
    use ezrouter::provider::MessageContent;
    use ezrouter::routes::messages::{anthropic_to_openai_request, AnthropicMessagesRequest};

    let b64_pixel = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";
    let b64_jpeg = "dGVzdC1qcGVnLWRhdGE=";

    let json_req = serde_json::json!({
        "model": "claude-3-5-sonnet-20241022",
        "messages": [
            {
                "role": "user",
                "content": [
                    {
                        "type": "text",
                        "text": "Analyze this screenshot:"
                    },
                    {
                        "type": "image",
                        "source": {
                            "type": "base64",
                            "media_type": "image/png",
                            "data": b64_pixel
                        }
                    }
                ]
            },
            {
                "role": "assistant",
                "content": [
                    {
                        "type": "tool_use",
                        "id": "toolu_crop_01",
                        "name": "crop_selection",
                        "input": {"x": 10, "y": 20}
                    }
                ]
            },
            {
                "role": "user",
                "content": [
                    {
                        "type": "tool_result",
                        "tool_use_id": "toolu_crop_01",
                        "content": [
                            {
                                "type": "text",
                                "text": "Crop output:"
                            },
                            {
                                "type": "image",
                                "source": {
                                    "type": "base64",
                                    "media_type": "image/jpeg",
                                    "data": b64_jpeg
                                }
                            }
                        ]
                    }
                ]
            }
        ]
    });

    let anthropic_req: AnthropicMessagesRequest = serde_json::from_value(json_req).unwrap();
    let (openai_req, _mapped_model) = anthropic_to_openai_request(anthropic_req);

    // 1. User message checks
    assert_eq!(openai_req.messages[0].role, "user");
    match &openai_req.messages[0].content {
        Some(MessageContent::Parts(parts)) => {
            assert_eq!(parts.len(), 2);
            assert_eq!(parts[0]["type"], "text");
            assert_eq!(parts[0]["text"], "Analyze this screenshot:");
            assert_eq!(parts[1]["type"], "image_url");
            assert_eq!(
                parts[1]["image_url"]["url"],
                format!("data:image/png;base64,{b64_pixel}")
            );
        }
        other => panic!("Expected user MessageContent::Parts, got {:?}", other),
    }

    // 2. Assistant tool_use message checks
    assert_eq!(openai_req.messages[1].role, "assistant");
    assert!(openai_req.messages[1].tool_calls.is_some());

    // 3. Tool result message checks
    assert_eq!(openai_req.messages[2].role, "tool");
    assert_eq!(
        openai_req.messages[2].tool_call_id.as_deref(),
        Some("toolu_crop_01")
    );
    match &openai_req.messages[2].content {
        Some(MessageContent::Parts(parts)) => {
            assert_eq!(parts.len(), 2);
            assert_eq!(parts[0]["type"], "text");
            assert_eq!(parts[0]["text"], "Crop output:");
            assert_eq!(parts[1]["type"], "image_url");
            assert_eq!(
                parts[1]["image_url"]["url"],
                format!("data:image/jpeg;base64,{b64_jpeg}")
            );
        }
        other => panic!("Expected tool MessageContent::Parts, got {:?}", other),
    }
}

#[tokio::test]
async fn test_post_messages_with_image_content_mock() {
    let (state, dir) = test_state(true);
    let app = app_router(state);

    let req_body = serde_json::json!({
        "model": "claude-3-5-sonnet-20241022",
        "messages": [
            {
                "role": "user",
                "content": [
                    {
                        "type": "text",
                        "text": "What is this?"
                    },
                    {
                        "type": "image",
                        "source": {
                            "type": "base64",
                            "media_type": "image/png",
                            "data": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg=="
                        }
                    }
                ]
            }
        ]
    });

    let req = Request::builder()
        .method("POST")
        .uri("/v1/messages")
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-api-key", "test-secret-key")
        .body(Body::from(serde_json::to_string(&req_body).unwrap()))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["type"], "message");
    assert_eq!(json["role"], "assistant");

    let _ = std::fs::remove_dir_all(dir);
}
