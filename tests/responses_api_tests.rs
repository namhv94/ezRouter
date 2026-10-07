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
    (AppState::new(config), dir)
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
        .body(Body::from(r#"{"model":"cx/gpt-5.6-sol","input":[]}"#))
        .unwrap();

    let resp = app.clone().oneshot(missing_auth).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    let bad_auth = Request::builder()
        .method("POST")
        .uri("/v1/responses")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, "Bearer invalid-random-token")
        .body(Body::from(r#"{"model":"cx/gpt-5.6-sol","input":[]}"#))
        .unwrap();

    let resp = app.oneshot(bad_auth).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn test_post_responses_rejects_antigravity_model() {
    let (state, dir) = test_state(true);
    let app = app_router(state);

    let req = Request::builder()
        .method("POST")
        .uri("/v1/responses")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, "Bearer test-secret-key")
        .body(Body::from(
            r#"{"model":"ag/gemini-3.8-flash-high","input":[]}"#,
        ))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&bytes);
    assert!(body_str.contains("not compatible with OpenAI Responses API"));

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
            r#"{"model":"cx/gpt-5.6-sol","stream":true,"input":[{"type":"message","role":"user","content":"hello"}]}"#,
        ))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get(header::CONTENT_TYPE).unwrap(),
        "text/event-stream; charset=utf-8"
    );

    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&bytes);
    assert!(body_str.contains("event: response.done"));

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
            r#"{"model":"cx/gpt-5.6-sol","stream":false,"input":[{"type":"message","role":"user","content":"hello"}]}"#,
        ))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/json; charset=utf-8"
    );

    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["status"], "completed");

    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn test_serve_setup_scripts() {
    let (state, dir) = test_state(true);
    let app = app_router(state);

    // 1. /setup-codex.sh
    let req = Request::builder()
        .method("GET")
        .uri("/setup-codex.sh")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("ezRouter - Tích Hợp OpenAI Codex"));
    assert!(text.contains("config.toml"));

    // 2. /setup-codex.bat
    let req = Request::builder()
        .method("GET")
        .uri("/setup-codex.bat")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("ezRouter - Tich Hop OpenAI Codex"));

    // 3. /setup-codex.ps1
    let req = Request::builder()
        .method("GET")
        .uri("/setup-codex.ps1")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("ezRouter - Tích Hợp OpenAI Codex"));

    // 4. /scripts/setup-codex.sh
    let req = Request::builder()
        .method("GET")
        .uri("/scripts/setup-codex.sh")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn test_serve_uninstall_scripts() {
    let (state, dir) = test_state(true);
    let app = app_router(state);

    // 1. /uninstall-codex.sh
    let req = Request::builder()
        .method("GET")
        .uri("/uninstall-codex.sh")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("ezRouter - Gỡ Bỏ Cấu Hình Codex"));

    // 2. /uninstall-codex.bat
    let req = Request::builder()
        .method("GET")
        .uri("/uninstall-codex.bat")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("ezRouter - Go Bo Cau Hinh Codex"));

    // 3. /uninstall-codex.ps1
    let req = Request::builder()
        .method("GET")
        .uri("/uninstall-codex.ps1")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("ezRouter - Gỡ Bỏ Cấu Hình Codex"));

    // 4. /scripts/uninstall-codex.sh
    let req = Request::builder()
        .method("GET")
        .uri("/scripts/uninstall-codex.sh")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // 5. Traversal attempts on /scripts are rejected
    let req = Request::builder()
        .method("GET")
        .uri("/scripts/..%2fCargo.toml")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_extract_responses_usage_all_variants() {
    use ezrouter::routes::responses::extract_responses_usage;
    use serde_json::json;

    // 1. Top-level usage with input_tokens and output_tokens (OpenAI Responses standard)
    let v1 = json!({
        "id": "resp_1",
        "usage": {
            "input_tokens": 15,
            "output_tokens": 30,
            "total_tokens": 45
        }
    });
    assert_eq!(extract_responses_usage(&v1), Some((Some(15), Some(30))));

    // 2. Top-level usage with prompt_tokens and completion_tokens (Chat format fallback)
    let v2 = json!({
        "id": "resp_2",
        "usage": {
            "prompt_tokens": 12,
            "completion_tokens": 24
        }
    });
    assert_eq!(extract_responses_usage(&v2), Some((Some(12), Some(24))));

    // 3. Nested response.usage with input_tokens and output_tokens (SSE response.completed event standard)
    let v3 = json!({
        "type": "response.completed",
        "response": {
            "id": "resp_3",
            "usage": {
                "input_tokens": 18,
                "output_tokens": 42
            }
        }
    });
    assert_eq!(extract_responses_usage(&v3), Some((Some(18), Some(42))));

    // 4. Nested response.usage with prompt_tokens and completion_tokens
    let v4 = json!({
        "type": "response.completed",
        "response": {
            "id": "resp_4",
            "usage": {
                "prompt_tokens": 25,
                "completion_tokens": 55
            }
        }
    });
    assert_eq!(extract_responses_usage(&v4), Some((Some(25), Some(55))));

    // 5. Partial tokens (only input, or only output)
    let v5_in = json!({
        "usage": {
            "input_tokens": 50
        }
    });
    assert_eq!(extract_responses_usage(&v5_in), Some((Some(50), None)));

    let v5_out = json!({
        "response": {
            "usage": {
                "output_tokens": 80
            }
        }
    });
    assert_eq!(extract_responses_usage(&v5_out), Some((None, Some(80))));

    // 6. Missing usage or invalid structures
    let v6_empty = json!({"type": "response.output_text.delta", "delta": "hello"});
    assert_eq!(extract_responses_usage(&v6_empty), None);

    let v6_null_usage = json!({"usage": null});
    assert_eq!(extract_responses_usage(&v6_null_usage), None);

    let v6_non_numeric = json!({"usage": {"input_tokens": "not_a_number"}});
    assert_eq!(extract_responses_usage(&v6_non_numeric), None);
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
            r#"{"model":"cx/gpt-5.6-sol","stream":true,"input":[{"type":"message","role":"user","content":"stream test"}]}"#,
        ))
        .unwrap();

    let resp_stream = app.clone().oneshot(req_stream).await.unwrap();
    assert_eq!(resp_stream.status(), StatusCode::OK);

    // Send non-streaming request
    let req_sync = Request::builder()
        .method("POST")
        .uri("/v1/responses")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, "Bearer test-secret-key")
        .body(Body::from(
            r#"{"model":"cx/gpt-5.6-sol","stream":false,"input":[{"type":"message","role":"user","content":"sync test"}]}"#,
        ))
        .unwrap();

    let resp_sync = app.oneshot(req_sync).await.unwrap();
    assert_eq!(resp_sync.status(), StatusCode::OK);

    // Verify DB records
    let requests = state.db.get_requests(10, 0, None, Some("success")).unwrap();
    assert_eq!(requests.total, 2);
    for item in &requests.items {
        assert_eq!(item.status, "success");
        assert_eq!(item.prompt_tokens, 10);
        assert_eq!(item.completion_tokens, 20);
        assert_eq!(item.total_tokens, 30);
    }

    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn test_db_status_filter_and_summary_includes_completed_legacy() {
    let (state, dir) = test_state(true);

    // Manually record rows with 'success', 'ok', and legacy 'completed'
    let _ = state.db.record_request(
        Some("user1"),
        "cx/gpt-5.6-sol",
        1000.0,
        "success",
        10,
        20,
        150.0,
        None,
    );
    let _ = state.db.record_request(
        Some("user2"),
        "cx/gpt-5.6-sol",
        1001.0,
        "ok",
        15,
        25,
        180.0,
        None,
    );
    let _ = state.db.record_request(
        Some("user3"),
        "cx/gpt-5.6-sol",
        1002.0,
        "completed",
        20,
        30,
        200.0,
        None,
    );
    let _ = state.db.record_request(
        Some("user4"),
        "cx/gpt-5.6-sol",
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
        .find(|s| s.model == "cx/gpt-5.6-sol")
        .unwrap();
    assert_eq!(model_sum.requests, 4);
    assert_eq!(model_sum.ok, 3);
    assert_eq!(model_sum.errors, 1);

    let _ = std::fs::remove_dir_all(dir);
}
