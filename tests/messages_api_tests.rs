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
