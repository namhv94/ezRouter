use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use ezrouter::{app_router, AppState, Config};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

fn test_state(mock: bool) -> (AppState, std::path::PathBuf) {
    let dir =
        std::env::temp_dir().join(format!("ezrouter-responses-test-{}", uuid::Uuid::new_v4()));
    let mut config = Config::parse(
        Some("127.0.0.1".to_string()),
        Some("20229".to_string()),
        Some(dir.to_string_lossy().into_owned()),
        Some("test-secret-key".to_string()),
    )
    .unwrap();
    config.use_mock_provider = mock;
    let state = AppState::new(config);
    seed_test_providers(&state);
    (state, dir)
}

/// Models come only from DB-configured providers now, so tests must register
/// the prefixes they exercise.
fn seed_test_providers(state: &AppState) {
    let _ = state.db.create_provider(
        "Test OpenAI",
        "openai",
        "openai",
        "http://127.0.0.1:1",
        "sk-test",
        &serde_json::json!(["gpt-4o", "gpt-5", "o3"]),
        true,
    );
}

#[tokio::test]
async fn test_get_responses_returns_400_with_informative_error() {
    let (state, dir) = test_state(true);
    let app = app_router(state);

    let req = Request::builder()
        .method("GET")
        .uri("/v1/responses")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["error"]["code"], "websocket_not_supported");

    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn test_post_responses_requires_auth() {
    let (state, dir) = test_state(true);
    let app = app_router(state);

    let missing_auth = Request::builder()
        .method("POST")
        .uri("/v1/responses")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"model":"openai/gpt-4o","input":[]}"#))
        .unwrap();

    let resp = app.clone().oneshot(missing_auth).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    let bad_auth = Request::builder()
        .method("POST")
        .uri("/v1/responses")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, "Bearer invalid-random-token")
        .body(Body::from(r#"{"model":"openai/gpt-4o","input":[]}"#))
        .unwrap();

    let resp = app.oneshot(bad_auth).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn test_post_responses_streaming_success_with_mock() {
    let (state, dir) = test_state(true);
    let app = app_router(state);

    let req = Request::builder()
        .method("POST")
        .uri("/v1/responses")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, "Bearer test-secret-key")
        .body(Body::from(
            r#"{"model":"openai/gpt-4o","stream":true,"input":[{"type":"message","role":"user","content":"hello"}]}"#,
        ))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let content_type = resp
        .headers()
        .get(header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(
        content_type.starts_with("text/event-stream"),
        "unexpected content type: {content_type}"
    );

    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&bytes);
    assert!(body_str.contains("event: response.completed"));
    assert!(body_str.contains("event: response.created"));

    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn test_post_responses_non_streaming_success_with_mock() {
    let (state, dir) = test_state(true);
    let app = app_router(state);

    let req = Request::builder()
        .method("POST")
        .uri("/v1/responses")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, "Bearer test-secret-key")
        .body(Body::from(
            r#"{"model":"openai/gpt-4o","stream":false,"input":[{"type":"message","role":"user","content":"hello"}]}"#,
        ))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let content_type = resp
        .headers()
        .get(header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(
        content_type.starts_with("application/json"),
        "unexpected content type: {content_type}"
    );

    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["status"], "completed");

    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn test_responses_db_logging_status_and_tokens() {
    let (state, dir) = test_state(true);
    let app = app_router(state.clone());

    // Send streaming request
    let req_stream = Request::builder()
        .method("POST")
        .uri("/v1/responses")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, "Bearer test-secret-key")
        .body(Body::from(
            r#"{"model":"openai/gpt-4o","stream":true,"input":[{"type":"message","role":"user","content":"stream test"}]}"#,
        ))
        .unwrap();

    let resp_stream = app.clone().oneshot(req_stream).await.unwrap();
    assert_eq!(resp_stream.status(), StatusCode::OK);
    // Drain the SSE body so the streaming logger observes completion.
    let _ = resp_stream.into_body().collect().await.unwrap().to_bytes();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Send non-streaming request
    let req_sync = Request::builder()
        .method("POST")
        .uri("/v1/responses")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, "Bearer test-secret-key")
        .body(Body::from(
            r#"{"model":"openai/gpt-4o","stream":false,"input":[{"type":"message","role":"user","content":"sync test"}]}"#,
        ))
        .unwrap();

    let resp_sync = app.oneshot(req_sync).await.unwrap();
    assert_eq!(resp_sync.status(), StatusCode::OK);

    // Verify DB records
    let requests = state.db.get_requests(10, 0, None, Some("success")).unwrap();
    assert_eq!(requests.total, 2);
    for item in &requests.items {
        assert_eq!(item.status, "success");
        // Usage is whatever the mock provider computed for this model id; the
        // contract under test is that tokens are persisted and self-consistent.
        assert!(item.prompt_tokens > 0, "prompt tokens must be recorded");
        assert!(item.completion_tokens > 0, "completion tokens must be recorded");
        assert_eq!(
            item.total_tokens,
            item.prompt_tokens + item.completion_tokens,
            "total tokens must equal prompt + completion"
        );
    }

    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn test_db_status_filter_and_summary_includes_completed_legacy() {
    let (state, dir) = test_state(true);

    // Manually record rows with 'success', 'ok', and legacy 'completed'
    let _ = state.db.record_request(
        Some("user1"),
        "openai/gpt-4o",
        1000.0,
        "success",
        10,
        20,
        150.0,
        None,
    );
    let _ = state.db.record_request(
        Some("user2"),
        "openai/gpt-4o",
        1001.0,
        "ok",
        15,
        25,
        180.0,
        None,
    );
    let _ = state.db.record_request(
        Some("user3"),
        "openai/gpt-4o",
        1002.0,
        "completed",
        20,
        30,
        200.0,
        None,
    );
    let _ = state.db.record_request(
        Some("user4"),
        "openai/gpt-4o",
        1003.0,
        "error",
        0,
        0,
        50.0,
        Some("upstream timeout"),
    );

    // 1. Filtering by status "success" matches 'success', 'ok', AND 'completed'
    let filtered_success = state.db.get_requests(10, 0, None, Some("success")).unwrap();
    assert_eq!(filtered_success.total, 3);
    let statuses: Vec<String> = filtered_success
        .items
        .into_iter()
        .map(|r| r.status)
        .collect();
    assert!(statuses.contains(&"success".to_string()));
    assert!(statuses.contains(&"ok".to_string()));
    assert!(statuses.contains(&"completed".to_string()));

    // 2. Summary counts all three as ok
    let summary = state.db.get_request_summary().unwrap();
    let model_sum = summary
        .iter()
        .find(|s| s.model == "openai/gpt-4o")
        .unwrap();
    assert_eq!(model_sum.requests, 4);
    assert_eq!(model_sum.ok, 3);
    assert_eq!(model_sum.errors, 1);

    let _ = std::fs::remove_dir_all(dir);
}
