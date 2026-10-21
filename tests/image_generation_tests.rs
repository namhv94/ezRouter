use std::sync::{Arc, Mutex};

use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderMap, Request, StatusCode},
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use ezrouter::{app_router, AppState, Config};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tower::ServiceExt;

fn test_state(mock: bool) -> (AppState, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("ezrouter-image-test-{}", uuid::Uuid::new_v4()));
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

async fn response_json(response: axum::response::Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn images_generations_requires_auth_and_valid_json_fields() {
    let (state, dir) = test_state(true);
    let app = app_router(state);

    let missing_auth = Request::builder()
        .method("POST")
        .uri("/v1/images/generations")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"model":"mock/image","prompt":"draw"}"#))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(missing_auth).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );

    let wrong_key = Request::builder()
        .method("POST")
        .uri("/v1/images/generations")
        .header(header::AUTHORIZATION, "Bearer wrong-key")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"model":"mock/image","prompt":"draw"}"#))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(wrong_key).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );

    for body in [
        r#"{"model":"mock/image","prompt":" "}"#,
        r#"{"model":" ","prompt":"draw"}"#,
        r#"{"model":"mock/image"}"#,
        r#"{"model":"mock/image","prompt":42}"#,
        r#"{"model":"mock/image","prompt":"draw"#,
    ] {
        let request = Request::builder()
            .method("POST")
            .uri("/v1/images/generations")
            .header(header::AUTHORIZATION, "Bearer test-secret-key")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body))
            .unwrap();
        assert_eq!(
            app.clone().oneshot(request).await.unwrap().status(),
            StatusCode::BAD_REQUEST,
            "body should be rejected: {body}"
        );
    }

    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn images_generations_mock_is_deterministic() {
    let (state, dir) = test_state(true);
    let app = app_router(state);
    let body = r#"{"model":"mock/image-v1","prompt":"a rust crab","size":"1024x1024","n":2,"custom":{"x":1}}"#;

    let send = || {
        Request::builder()
            .method("POST")
            .uri("/v1/images/generations")
            .header(header::AUTHORIZATION, "Bearer test-secret-key")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body))
            .unwrap()
    };
    let first = app.clone().oneshot(send()).await.unwrap();
    let second = app.oneshot(send()).await.unwrap();
    assert_eq!(first.status(), StatusCode::OK);
    assert_eq!(second.status(), StatusCode::OK);
    assert_eq!(response_json(first).await, response_json(second).await);

    let _ = std::fs::remove_dir_all(dir);
}

#[derive(Clone, Debug)]
struct CapturedRequest {
    path: String,
    auth: Option<String>,
    body: Value,
}

async fn capture_success(
    State(captured): State<Arc<Mutex<Vec<CapturedRequest>>>>,
    headers: HeaderMap,
    uri: axum::http::Uri,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    captured.lock().unwrap().push(CapturedRequest {
        path: uri.path().to_string(),
        auth: headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .map(ToOwned::to_owned),
        body,
    });
    Json(json!({
        "created": 1700000000,
        "data": [{"url": "https://example.invalid/image.png", "revised_prompt": "kept"}],
        "provider_extension": {"seed": 17}
    }))
}

async fn spawn_upstream(router: Router) -> (String, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    (format!("http://{address}"), handle)
}

#[tokio::test]
async fn images_generations_routes_persisted_provider_and_passes_json_through() {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let router = Router::new()
        .route("/v1/images/generations", post(capture_success))
        .with_state(captured.clone());
    let (base_url, handle) = spawn_upstream(router).await;
    let (state, dir) = test_state(false);
    state
        .db
        .create_provider(
            "Image Provider",
            "imgroute",
            "openai-compatible",
            &format!("{base_url}/v1"),
            "upstream-secret",
            &json!(["vendor/image-model"]),
            true,
        )
        .unwrap();
    let app = app_router(state);
    let request = Request::builder()
        .method("POST")
        .uri("/v1/images/generations")
        .header(header::AUTHORIZATION, "Bearer test-secret-key")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"model":"imgroute/vendor/image-model","prompt":"draw it","n":2,"size":"512x512","response_format":"b64_json","provider_option":{"seed":9}}"#,
        ))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response_json(response).await,
        json!({
            "created": 1700000000,
            "data": [{"url": "https://example.invalid/image.png", "revised_prompt": "kept"}],
            "provider_extension": {"seed": 17}
        })
    );

    let requests = captured.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].path, "/v1/images/generations");
    assert_eq!(requests[0].auth.as_deref(), Some("Bearer upstream-secret"));
    assert_eq!(requests[0].body["model"], "vendor/image-model");
    assert_eq!(requests[0].body["prompt"], "draw it");
    assert_eq!(requests[0].body["provider_option"]["seed"], 9);
    drop(requests);

    handle.abort();
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn images_generations_preserves_unprefixed_nested_model_id() {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let router = Router::new()
        .route("/v1/images/generations", post(capture_success))
        .with_state(captured.clone());
    let (base_url, handle) = spawn_upstream(router).await;
    let (state, dir) = test_state(false);
    state
        .db
        .create_provider(
            "Nested Model Provider",
            "nested-route",
            "openai-compatible",
            &base_url,
            "upstream-secret",
            &json!(["vendor/image-model"]),
            true,
        )
        .unwrap();
    let app = app_router(state);
    let request = Request::builder()
        .method("POST")
        .uri("/v1/images/generations")
        .header(header::AUTHORIZATION, "Bearer test-secret-key")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"model":"vendor/image-model","prompt":"draw it"}"#,
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let requests = captured.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].body["model"], "vendor/image-model");
    drop(requests);

    handle.abort();
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn images_generations_rejects_unknown_or_inactive_provider() {
    let (state, dir) = test_state(false);
    state
        .db
        .create_provider(
            "Inactive",
            "inactive",
            "openai-compatible",
            "http://127.0.0.1:1/v1",
            "secret",
            &json!(["image-model"]),
            false,
        )
        .unwrap();
    let app = app_router(state);

    for model in ["missing/image-model", "inactive/image-model"] {
        let request = Request::builder()
            .method("POST")
            .uri("/v1/images/generations")
            .header(header::AUTHORIZATION, "Bearer test-secret-key")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({"model": model, "prompt": "draw"}).to_string(),
            ))
            .unwrap();
        assert_eq!(
            app.clone().oneshot(request).await.unwrap().status(),
            StatusCode::NOT_FOUND
        );
    }

    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn images_generations_maps_upstream_errors_and_bounds_bodies() {
    async fn status_handler(State(status): State<StatusCode>) -> (StatusCode, Json<Value>) {
        (
            status,
            Json(json!({"error": {"message": "upstream image error"}})),
        )
    }

    for (upstream, expected) in [
        (StatusCode::BAD_REQUEST, StatusCode::BAD_REQUEST),
        (StatusCode::NOT_FOUND, StatusCode::NOT_FOUND),
        (StatusCode::REQUEST_TIMEOUT, StatusCode::GATEWAY_TIMEOUT),
        (StatusCode::GATEWAY_TIMEOUT, StatusCode::GATEWAY_TIMEOUT),
        (StatusCode::TOO_MANY_REQUESTS, StatusCode::BAD_GATEWAY),
        (StatusCode::INTERNAL_SERVER_ERROR, StatusCode::BAD_GATEWAY),
    ] {
        let router = Router::new()
            .route("/v1/images/generations", post(status_handler))
            .with_state(upstream);
        let (base_url, handle) = spawn_upstream(router).await;
        let (state, dir) = test_state(false);
        state
            .db
            .create_provider(
                "Errors",
                "errors",
                "openai-compatible",
                &base_url,
                "secret",
                &json!([]),
                true,
            )
            .unwrap();
        let request = Request::builder()
            .method("POST")
            .uri("/v1/images/generations")
            .header(header::AUTHORIZATION, "Bearer test-secret-key")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"model":"errors/image","prompt":"draw"}"#))
            .unwrap();
        assert_eq!(
            app_router(state).oneshot(request).await.unwrap().status(),
            expected
        );
        handle.abort();
        let _ = std::fs::remove_dir_all(dir);
    }

    let oversized = "x".repeat(16 * 1024 * 1024 + 1);
    let router = Router::new().route(
        "/v1/images/generations",
        post(move || {
            let oversized = oversized.clone();
            async move { (StatusCode::OK, oversized) }
        }),
    );
    let (base_url, handle) = spawn_upstream(router).await;
    let (state, dir) = test_state(false);
    state
        .db
        .create_provider(
            "Large",
            "large",
            "openai-compatible",
            &base_url,
            "secret",
            &json!([]),
            true,
        )
        .unwrap();
    let request = Request::builder()
        .method("POST")
        .uri("/v1/images/generations")
        .header(header::AUTHORIZATION, "Bearer test-secret-key")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"model":"large/image","prompt":"draw"}"#))
        .unwrap();
    let response = app_router(state).oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let body = response_json(response).await;
    assert!(body["error"]["message"]
        .as_str()
        .unwrap()
        .contains("exceeded limit"));

    handle.abort();
    let _ = std::fs::remove_dir_all(dir);
}

const SAMPLE_PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE,
    0x42, 0x60, 0x82,
];

const SAMPLE_JPEG: &[u8] = &[
    0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x01, 0x00, 0x48,
    0x00, 0x48, 0x00, 0x00, 0xFF, 0xD9,
];

const SAMPLE_WEBP: &[u8] = &[
    b'R', b'I', b'F', b'F', 0x14, 0x00, 0x00, 0x00, b'W', b'E', b'B', b'P', b'V', b'P', b'8', b' ',
    0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

const SAMPLE_GIF: &[u8] = &[
    b'G', b'I', b'F', b'8', b'9', b'a', 0x01, 0x00, 0x01, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00,
    0xFF, 0xFF, 0xFF, 0x21, 0xF9, 0x04, 0x01, 0x00, 0x00, 0x00, 0x00, 0x2C, 0x00, 0x00, 0x00, 0x00,
    0x01, 0x00, 0x01, 0x00, 0x00, 0x02, 0x02, 0x44, 0x01, 0x00, 0x3B,
];

#[tokio::test]
async fn images_generations_rejects_arbitrary_host_reference_image_paths() {
    let (state, dir) = test_state(true);
    let app = app_router(state);

    let malicious_paths = [
        "/etc/passwd",
        "/etc/shadow",
        "/home/namhv/.ssh/id_rsa",
        "~/.bashrc",
        "../secret.png",
        "./relative_file.jpg",
        "file:///etc/passwd",
        "host_path.png",
        "data:text/plain;base64,aGVsbG8=",
        "data:image/png;base64,bm90YW5pbWFnZQ==",
    ];

    for path in malicious_paths {
        let request = Request::builder()
            .method("POST")
            .uri("/v1/images/generations")
            .header(header::AUTHORIZATION, "Bearer test-secret-key")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({
                    "model": "mock/image",
                    "prompt": "draw",
                    "reference_image": path,
                })
                .to_string(),
            ))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "Path should be rejected as bad request: {path}"
        );
        let body = response_json(response).await;
        let msg = body["error"]["message"].as_str().unwrap_or("");
        assert!(
            msg.contains("reference_image") || msg.contains("host file path"),
            "Error message should mention reference_image issue for: {path}, got: {msg}"
        );
    }

    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn images_generations_accepts_valid_reference_image_payloads() {
    use base64::Engine;
    let b64_png = base64::engine::general_purpose::STANDARD.encode(SAMPLE_PNG);
    let data_url_png = format!("data:image/png;base64,{b64_png}");

    let (state, dir) = test_state(true);
    let app = app_router(state);

    // 1. Valid data URL
    let request1 = Request::builder()
        .method("POST")
        .uri("/v1/images/generations")
        .header(header::AUTHORIZATION, "Bearer test-secret-key")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "model": "mock/image",
                "prompt": "draw with ref",
                "image": data_url_png,
            })
            .to_string(),
        ))
        .unwrap();

    let response1 = app.clone().oneshot(request1).await.unwrap();
    assert_eq!(response1.status(), StatusCode::OK);

    // 2. Valid raw base64
    let request2 = Request::builder()
        .method("POST")
        .uri("/v1/images/generations")
        .header(header::AUTHORIZATION, "Bearer test-secret-key")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "model": "mock/image",
                "prompt": "draw with raw base64",
                "reference_image": b64_png,
            })
            .to_string(),
        ))
        .unwrap();

    let response2 = app.oneshot(request2).await.unwrap();
    assert_eq!(response2.status(), StatusCode::OK);

    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn images_generations_accepts_raw_base64_jpeg_with_leading_and_internal_slashes() {
    use base64::Engine;
    let b64_jpeg = base64::engine::general_purpose::STANDARD.encode(SAMPLE_JPEG);
    assert!(b64_jpeg.starts_with("/9j/"), "JPEG base64 starts with /9j/");
    assert!(b64_jpeg.contains('/'), "JPEG base64 contains slashes");

    // JPEG with additional internal slashes
    let mut realistic_jpeg = SAMPLE_JPEG.to_vec();
    realistic_jpeg.extend_from_slice(&[0xFB, 0xEF, 0xFF, 0xD9]);
    let b64_with_slashes = base64::engine::general_purpose::STANDARD.encode(&realistic_jpeg);
    assert!(b64_with_slashes.contains('/'));

    let (state, dir) = test_state(true);
    let app = app_router(state);

    // 1. Mock accepts raw base64 JPEG starting with /9j/
    let req1 = Request::builder()
        .method("POST")
        .uri("/v1/images/generations")
        .header(header::AUTHORIZATION, "Bearer test-secret-key")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "model": "mock/image",
                "prompt": "draw cat with jpeg ref",
                "reference_image": b64_jpeg,
            })
            .to_string(),
        ))
        .unwrap();

    let resp1 = app.clone().oneshot(req1).await.unwrap();
    assert_eq!(
        resp1.status(),
        StatusCode::OK,
        "Raw base64 JPEG starting with /9j/ must not be falsely rejected as host path"
    );

    // 2. Mock accepts raw base64 JPEG containing internal slashes
    let req2 = Request::builder()
        .method("POST")
        .uri("/v1/images/generations")
        .header(header::AUTHORIZATION, "Bearer test-secret-key")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "model": "mock/image",
                "prompt": "draw cat with slash jpeg ref",
                "image": b64_with_slashes,
            })
            .to_string(),
        ))
        .unwrap();

    let resp2 = app.oneshot(req2).await.unwrap();
    assert_eq!(
        resp2.status(),
        StatusCode::OK,
        "Raw base64 JPEG containing internal slashes must be accepted"
    );

    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn images_generations_codex_endpoint_fails_closed_503_and_records_live_registry() {
    let (state, dir) = test_state(false);
    let app = app_router(state.clone());

    let codex_models = [
        "cx/image-gen",
        "image-gen",
        "cx/gpt-image",
        "cx/gpt-image-2",
        "gpt-image-2",
        "codex-image",
        "cx/image/v1",
    ];

    for model in codex_models {
        let request = Request::builder()
            .method("POST")
            .uri("/v1/images/generations")
            .header(header::AUTHORIZATION, "Bearer test-secret-key")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({
                    "model": model,
                    "prompt": "generate a cat",
                })
                .to_string(),
            ))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        // Since Codex CLI execution is unsafe without an isolated OS sandbox restricted to image_gen,
        // it must fail closed with 503 SERVICE_UNAVAILABLE
        assert_eq!(
            response.status(),
            StatusCode::SERVICE_UNAVAILABLE,
            "Model {model} must fail closed with 503 Service Unavailable"
        );
        let body = response_json(response).await;
        let msg = body["error"]["message"].as_str().unwrap_or("");
        assert!(
            msg.contains("disabled") || msg.contains("sandbox"),
            "Expected disabled/sandbox security message, got: {msg}"
        );
        assert_eq!(body["error"]["code"], "service_unavailable");
    }

    // Verify live_registry recorded the requests and tracked them to Codex with error status
    let active = state.live_registry.get_active();
    assert!(
        !active.items.is_empty(),
        "Live registry should have recorded the Codex requests"
    );
    let codex_items: Vec<_> = active
        .items
        .iter()
        .filter(|i| i.provider == "Codex")
        .collect();
    assert!(
        !codex_items.is_empty(),
        "Live registry must have routed items to provider 'Codex'"
    );
    for item in codex_items {
        assert_eq!(item.status, "error");
        assert!(item.completed_at.is_some());
    }

    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn images_generations_codex_endpoint_validates_reference_image_before_pool_dispatch() {
    let (state, dir) = test_state(false);
    let app = app_router(state);

    // Malicious host path sent to Codex model must return 400 Bad Request (NOT 502 Bad Gateway)
    let bad_req = Request::builder()
        .method("POST")
        .uri("/v1/images/generations")
        .header(header::AUTHORIZATION, "Bearer test-secret-key")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "model": "cx/image-gen",
                "prompt": "generate a mountain",
                "reference_image": "/etc/shadow",
            })
            .to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(bad_req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "Codex endpoint must reject invalid reference image with 400 Bad Request"
    );

    // Empty string reference_image must not be silently accepted
    let empty_ref = Request::builder()
        .method("POST")
        .uri("/v1/images/generations")
        .header(header::AUTHORIZATION, "Bearer test-secret-key")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "model": "cx/image-gen",
                "prompt": "generate a mountain",
                "reference_image": "",
            })
            .to_string(),
        ))
        .unwrap();

    let resp_empty = app.clone().oneshot(empty_ref).await.unwrap();
    assert_eq!(
        resp_empty.status(),
        StatusCode::BAD_REQUEST,
        "Codex endpoint must reject empty reference_image with 400 Bad Request"
    );

    // Integer reference_image must not be silently accepted
    let num_ref = Request::builder()
        .method("POST")
        .uri("/v1/images/generations")
        .header(header::AUTHORIZATION, "Bearer test-secret-key")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "model": "cx/image-gen",
                "prompt": "generate a mountain",
                "reference_image": 12345,
            })
            .to_string(),
        ))
        .unwrap();

    let resp_num = app.oneshot(num_ref).await.unwrap();
    assert_eq!(
        resp_num.status(),
        StatusCode::BAD_REQUEST,
        "Codex endpoint must reject non-string reference_image with 400 Bad Request"
    );

    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn images_generations_mock_rejects_empty_and_non_string_reference_image() {
    let (state, dir) = test_state(true);
    let app = app_router(state);

    for bad_payload in [
        json!({"model": "mock/image", "prompt": "draw", "reference_image": ""}),
        json!({"model": "mock/image", "prompt": "draw", "reference_image": 123}),
        json!({"model": "mock/image", "prompt": "draw", "reference_image": null}),
        json!({"model": "mock/image", "prompt": "draw", "image": "   "}),
        json!({"model": "mock/image", "prompt": "draw", "image": false}),
    ] {
        let req = Request::builder()
            .method("POST")
            .uri("/v1/images/generations")
            .header(header::AUTHORIZATION, "Bearer test-secret-key")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(bad_payload.to_string()))
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::BAD_REQUEST,
            "Invalid reference_image must not be silently accepted: {bad_payload}"
        );
    }

    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn test_image_format_magic_bytes_detection_and_mime() {
    use ezrouter::routes::images::ImageFormat;

    assert_eq!(
        ImageFormat::from_magic_bytes(SAMPLE_PNG),
        Some(ImageFormat::Png)
    );
    assert_eq!(ImageFormat::Png.mime_type(), "image/png");
    assert_eq!(ImageFormat::Png.extension(), "png");

    assert_eq!(
        ImageFormat::from_magic_bytes(SAMPLE_JPEG),
        Some(ImageFormat::Jpeg)
    );
    assert_eq!(ImageFormat::Jpeg.mime_type(), "image/jpeg");
    assert_eq!(ImageFormat::Jpeg.extension(), "jpg");

    assert_eq!(
        ImageFormat::from_magic_bytes(SAMPLE_WEBP),
        Some(ImageFormat::Webp)
    );
    assert_eq!(ImageFormat::Webp.mime_type(), "image/webp");
    assert_eq!(ImageFormat::Webp.extension(), "webp");

    assert_eq!(
        ImageFormat::from_magic_bytes(SAMPLE_GIF),
        Some(ImageFormat::Gif)
    );
    assert_eq!(ImageFormat::Gif.mime_type(), "image/gif");
    assert_eq!(ImageFormat::Gif.extension(), "gif");

    assert_eq!(
        ImageFormat::from_magic_bytes(b"hello world not an image"),
        None
    );
    assert_eq!(ImageFormat::from_magic_bytes(&[]), None);
}

#[tokio::test]
async fn test_find_generated_image_deterministic_precedence_and_no_25kb_threshold() {
    use ezrouter::routes::images::find_generated_image;

    let test_dir =
        std::env::temp_dir().join(format!("ezrouter-imgfinder-{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&test_dir).await.unwrap();

    let out_file = test_dir.join("output.png");
    // Write 68-byte PNG (far below previous 25,000-byte threshold)
    tokio::fs::write(&out_file, SAMPLE_PNG).await.unwrap();

    // 1. Designated output file is found even though < 25KB!
    let found = find_generated_image(&test_dir, &out_file).await;
    assert_eq!(found, Some(out_file.clone()));

    // 2. Write a large non-image file (50KB text) - must NOT be picked as image
    let fake_img = test_dir.join("output.jpg");
    tokio::fs::write(&fake_img, vec![b'a'; 50_000])
        .await
        .unwrap();
    let found2 = find_generated_image(&test_dir, &fake_img).await;
    // Should still pick output.png, not the 50KB fake output.jpg
    assert_eq!(found2, Some(out_file.clone()));

    // Remove output.png to test generated_images directory fallback
    tokio::fs::remove_file(&out_file).await.unwrap();
    let gen_dir = test_dir.join("generated_images");
    tokio::fs::create_dir_all(&gen_dir).await.unwrap();
    let gen_file = gen_dir.join("codex_gen.png");
    tokio::fs::write(&gen_file, SAMPLE_PNG).await.unwrap();

    let found_in_gen = find_generated_image(&test_dir, &out_file).await;
    assert_eq!(found_in_gen, Some(gen_file));

    let _ = tokio::fs::remove_dir_all(&test_dir).await;
}

#[test]
fn test_codex_prompt_is_neutral_and_unbiased() {
    use ezrouter::routes::images::build_codex_prompt;
    use std::path::Path;

    let out_path = Path::new("/tmp/test/output.png");

    let prompt_no_ref = build_codex_prompt("a sunset over mountains", out_path, false);
    assert!(prompt_no_ref.contains("a sunset over mountains"));
    assert!(prompt_no_ref.contains("/tmp/test/output.png"));
    assert!(!prompt_no_ref.contains("cô gái"));
    assert!(!prompt_no_ref.contains("chân dung"));
    assert!(!prompt_no_ref.contains("candid smartphone photography"));

    let prompt_with_ref = build_codex_prompt("a sleek electric sports car", out_path, true);
    assert!(prompt_with_ref.contains("a sleek electric sports car"));
    assert!(prompt_with_ref.contains("-i"));
    assert!(prompt_with_ref.contains("/tmp/test/output.png"));
    assert!(!prompt_with_ref.contains("cô gái"));
    assert!(!prompt_with_ref.contains("chân dung"));
    assert!(!prompt_with_ref.contains("candid smartphone photography"));
}

#[tokio::test]
async fn test_run_dir_guard_cleans_up_on_drop() {
    use ezrouter::routes::images::RunDirGuard;

    let test_dir =
        std::env::temp_dir().join(format!("ezrouter-guard-test-{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&test_dir).await.unwrap();
    let auth_path = test_dir.join("auth.json");
    tokio::fs::write(&auth_path, b"{\"secret_token\": \"xyz\"}")
        .await
        .unwrap();

    assert!(auth_path.exists());

    {
        let _guard = RunDirGuard::new(test_dir.clone());
        assert!(_guard.path().exists());
    }

    assert!(
        !auth_path.exists(),
        "auth.json should be wiped on guard drop"
    );
    assert!(
        !test_dir.exists(),
        "run_dir should be removed on guard drop"
    );
}

#[tokio::test]
async fn images_generations_external_provider_preserves_url_and_rejects_dangerous_paths() {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let router = Router::new()
        .route("/v1/images/generations", post(capture_success))
        .with_state(captured.clone());
    let (base_url, handle) = spawn_upstream(router).await;
    let (state, dir) = test_state(false);
    state
        .db
        .create_provider(
            "External Img",
            "extimg",
            "openai-compatible",
            &base_url,
            "upstream-secret",
            &json!(["remote/model"]),
            true,
        )
        .unwrap();
    let app = app_router(state);

    // 1. Valid web URL reference image passes through untouched
    let req_ok = Request::builder()
        .method("POST")
        .uri("/v1/images/generations")
        .header(header::AUTHORIZATION, "Bearer test-secret-key")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "model": "extimg/remote/model",
                "prompt": "draw character",
                "image": "https://cdn.example.com/character.png",
                "reference_image": "https://cdn.example.com/ref.jpg"
            })
            .to_string(),
        ))
        .unwrap();

    let resp_ok = app.clone().oneshot(req_ok).await.unwrap();
    assert_eq!(resp_ok.status(), StatusCode::OK);
    {
        let reqs = captured.lock().unwrap();
        assert_eq!(reqs.len(), 1);
        assert_eq!(
            reqs[0].body["image"],
            "https://cdn.example.com/character.png"
        );
        assert_eq!(
            reqs[0].body["reference_image"],
            "https://cdn.example.com/ref.jpg"
        );
    }

    // 2. Dangerous host paths are rejected across all routes, including external provider
    let req_bad = Request::builder()
        .method("POST")
        .uri("/v1/images/generations")
        .header(header::AUTHORIZATION, "Bearer test-secret-key")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "model": "extimg/remote/model",
                "prompt": "draw character",
                "reference_image": "/etc/shadow"
            })
            .to_string(),
        ))
        .unwrap();

    let resp_bad = app.oneshot(req_bad).await.unwrap();
    assert_eq!(resp_bad.status(), StatusCode::BAD_REQUEST);

    handle.abort();
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn images_generations_external_passthrough_preserves_avif_and_svg_without_decoding() {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let router = Router::new()
        .route("/v1/images/generations", post(capture_success))
        .with_state(captured.clone());
    let (base_url, handle) = spawn_upstream(router).await;
    let (state, dir) = test_state(false);
    state
        .db
        .create_provider(
            "Ext Media Provider",
            "extmedia",
            "openai-compatible",
            &base_url,
            "upstream-secret",
            &json!(["provider/media-model"]),
            true,
        )
        .unwrap();
    let app = app_router(state);

    // 1. AVIF data URL passes through without decoding or rejection
    let avif_data_url = "data:image/avif;base64,AAAAIGZ0eXBhdmlmAAAAAG1pZjFhdmlmAAACAG1ldGE=";
    let req_avif = Request::builder()
        .method("POST")
        .uri("/v1/images/generations")
        .header(header::AUTHORIZATION, "Bearer test-secret-key")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "model": "extmedia/provider/media-model",
                "prompt": "transform avif",
                "image": avif_data_url,
            })
            .to_string(),
        ))
        .unwrap();

    let resp_avif = app.clone().oneshot(req_avif).await.unwrap();
    assert_eq!(resp_avif.status(), StatusCode::OK);

    // 2. SVG data URL passes through without decoding or rejection
    let svg_data_url =
        "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciPjwvc3ZnPg==";
    let req_svg = Request::builder()
        .method("POST")
        .uri("/v1/images/generations")
        .header(header::AUTHORIZATION, "Bearer test-secret-key")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "model": "extmedia/provider/media-model",
                "prompt": "render svg",
                "reference_image": svg_data_url,
            })
            .to_string(),
        ))
        .unwrap();

    let resp_svg = app.clone().oneshot(req_svg).await.unwrap();
    assert_eq!(resp_svg.status(), StatusCode::OK);

    // 3. Raw base64 JPEG starting with /9j/ passes through to external provider
    use base64::Engine;
    let b64_jpeg = base64::engine::general_purpose::STANDARD.encode(SAMPLE_JPEG);
    let req_jpeg = Request::builder()
        .method("POST")
        .uri("/v1/images/generations")
        .header(header::AUTHORIZATION, "Bearer test-secret-key")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "model": "extmedia/provider/media-model",
                "prompt": "enhance jpeg",
                "reference_image": b64_jpeg,
            })
            .to_string(),
        ))
        .unwrap();

    let resp_jpeg = app.clone().oneshot(req_jpeg).await.unwrap();
    assert_eq!(resp_jpeg.status(), StatusCode::OK);

    // Check all 3 requests were captured exactly by upstream
    let reqs = captured.lock().unwrap();
    assert_eq!(reqs.len(), 3);
    assert_eq!(reqs[0].body["image"], avif_data_url);
    assert_eq!(reqs[1].body["reference_image"], svg_data_url);
    assert_eq!(reqs[2].body["reference_image"], b64_jpeg);
    drop(reqs);

    handle.abort();
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn test_images_generations_live_registry_lifecycle() {
    let (state, dir) = test_state(true);
    let registry = state.live_registry.clone();
    let app = app_router(state);

    // 1. Success lifecycle on mock image generation
    let req = Request::builder()
        .method("POST")
        .uri("/v1/images/generations")
        .header(header::AUTHORIZATION, "Bearer test-secret-key")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "model": "mock/image",
                "prompt": "monitor test prompt",
            })
            .to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // In live_registry: recent requests stay for 3.5 seconds
    let active = registry.get_active();
    assert!(
        !active.items.is_empty(),
        "Live registry should have recorded the image request"
    );
    let item = active
        .items
        .iter()
        .find(|i| i.model == "mock/image")
        .expect("Item should be present");
    assert_eq!(item.status, "success");
    assert_eq!(item.provider, "MockProvider");
    assert_eq!(item.account, "mock");
    assert!(!item.stream);
    assert!(item.completed_at.is_some());

    // 2. Error lifecycle when invalid request is provided
    let bad_req = Request::builder()
        .method("POST")
        .uri("/v1/images/generations")
        .header(header::AUTHORIZATION, "Bearer test-secret-key")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "model": "mock/image",
                "prompt": "test prompt",
                "reference_image": "/etc/passwd",
            })
            .to_string(),
        ))
        .unwrap();

    let resp_bad = app.oneshot(bad_req).await.unwrap();
    assert_eq!(resp_bad.status(), StatusCode::BAD_REQUEST);

    let active_after = registry.get_active();
    let error_item = active_after
        .items
        .iter()
        .find(|i| i.status == "error")
        .expect("Error item should be recorded");
    assert_eq!(error_item.status, "error");
    assert!(error_item.completed_at.is_some());

    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn test_images_generations_cancellation_guard_marks_cancelled() {
    use ezrouter::routes::images::ImageCancellationGuard;

    let (state, dir) = test_state(true);
    let registry = state.live_registry.clone();
    let req_id = uuid::Uuid::new_v4().to_string();

    registry.register(
        req_id.clone(),
        "test-client".to_string(),
        "mock/image".to_string(),
        false,
    );

    {
        let _guard = ImageCancellationGuard {
            live_registry: registry.clone(),
            request_id: req_id.clone(),
            completed: false,
        };
        // _guard dropped without completed = true
    }

    let active = registry.get_active();
    let item = active
        .items
        .iter()
        .find(|i| i.id == req_id)
        .expect("Cancelled request must be in registry");
    assert_eq!(item.status, "cancelled");
    assert!(item.completed_at.is_some());

    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn test_validate_reference_image_rejects_predecode_oversized_payload() {
    use ezrouter::routes::images::{validate_reference_image, MAX_REFERENCE_IMAGE_INPUT_CHARS};

    let oversized = "a".repeat(MAX_REFERENCE_IMAGE_INPUT_CHARS + 1);
    let err = validate_reference_image(&oversized).unwrap_err();
    assert!(err.to_string().contains("exceeds maximum allowed size"));
}

#[tokio::test]
async fn test_validate_reference_image_rejects_magic_mime_mismatch() {
    use base64::Engine;
    use ezrouter::routes::images::validate_reference_image;

    // PNG payload declared as JPEG in data URL
    let b64_png = base64::engine::general_purpose::STANDARD.encode(SAMPLE_PNG);
    let mismatch = format!("data:image/jpeg;base64,{b64_png}");
    let err = validate_reference_image(&mismatch).unwrap_err();
    assert!(err.to_string().contains("does not match detected format"));

    // JPEG payload declared as PNG in data URL
    let b64_jpeg = base64::engine::general_purpose::STANDARD.encode(SAMPLE_JPEG);
    let mismatch2 = format!("data:image/png;base64,{b64_jpeg}");
    let err2 = validate_reference_image(&mismatch2).unwrap_err();
    assert!(err2.to_string().contains("does not match detected format"));
}

#[tokio::test]
async fn test_find_generated_image_and_verify_image_file_skip_symlinks() {
    use ezrouter::routes::images::{find_generated_image, verify_image_file};

    let test_dir = std::env::temp_dir().join(format!("ezrouter-symtest-{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&test_dir).await.unwrap();

    // Create real target image
    let real_png = test_dir.join("real_target.png");
    tokio::fs::write(&real_png, SAMPLE_PNG).await.unwrap();

    // Create symlink to that image
    let symlink_path = test_dir.join("symlink.png");
    #[cfg(unix)]
    tokio::fs::symlink(&real_png, &symlink_path).await.unwrap();

    // verify_image_file must reject the symlink
    let verified = verify_image_file(&symlink_path).await;
    assert_eq!(
        verified, None,
        "Symlink must not be verified as a valid image file"
    );

    // find_generated_image when designated output is a symlink must ignore it
    let designated_sym = test_dir.join("output.png");
    #[cfg(unix)]
    tokio::fs::symlink(&real_png, &designated_sym)
        .await
        .unwrap();

    let found = find_generated_image(&test_dir, &designated_sym).await;
    // real_target.png should be found via dir scan, NOT the designated symlink
    assert_eq!(found, Some(real_png));

    let _ = tokio::fs::remove_dir_all(&test_dir).await;
}

#[test]
fn test_codex_image_execution_is_not_safe() {
    use ezrouter::routes::images::is_codex_image_execution_safe;
    assert!(
        !is_codex_image_execution_safe(),
        "Codex image execution must report false until a secure OS sandbox restricted to image_gen is present"
    );
}

#[tokio::test]
async fn test_no_bypass_flag_in_images_route() {
    let images_rs = tokio::fs::read_to_string("src/routes/images.rs")
        .await
        .expect("Must be able to read src/routes/images.rs");

    assert!(
        !images_rs.contains("--dangerously-bypass-approvals-and-sandbox"),
        "src/routes/images.rs MUST NOT contain --dangerously-bypass-approvals-and-sandbox"
    );
    assert!(
        !images_rs.contains("--dangerously-bypass"),
        "src/routes/images.rs MUST NOT contain any --dangerously-bypass flag"
    );
}

#[tokio::test]
async fn test_command_injection_prompts_fail_closed_and_do_not_execute() {
    let (state, dir) = test_state(false);
    let app = app_router(state);

    let probe_dir =
        std::env::temp_dir().join(format!("ezrouter-inj-test-{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&probe_dir).await.unwrap();

    let injection_payloads = [
        format!("; touch {}/probe1 ;", probe_dir.display()),
        format!("$(touch {}/probe2)", probe_dir.display()),
        format!("`touch {}/probe3`", probe_dir.display()),
        format!("test | touch {}/probe4", probe_dir.display()),
        format!("test && touch {}/probe5", probe_dir.display()),
        format!("\ntouch {}/probe6\n", probe_dir.display()),
        format!("test'; touch {}/probe7; echo '", probe_dir.display()),
    ];

    for (idx, payload) in injection_payloads.iter().enumerate() {
        let req = Request::builder()
            .method("POST")
            .uri("/v1/images/generations")
            .header(header::AUTHORIZATION, "Bearer test-secret-key")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({
                    "model": "cx/image-gen",
                    "prompt": payload,
                })
                .to_string(),
            ))
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::SERVICE_UNAVAILABLE,
            "Injection payload {idx} must fail closed with 503 Service Unavailable"
        );

        let probe_file = probe_dir.join(format!("probe{}", idx + 1));
        assert!(
            !probe_file.exists(),
            "Command injection payload MUST NOT execute or create probe file: {}",
            probe_file.display()
        );
    }

    let _ = tokio::fs::remove_dir_all(probe_dir).await;
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn test_codex_endpoint_validates_reference_image_before_failing_closed() {
    use base64::Engine;

    let (state, dir) = test_state(false);
    let app = app_router(state);

    // 1. Invalid reference image fails with 400 Bad Request
    let req_invalid = Request::builder()
        .method("POST")
        .uri("/v1/images/generations")
        .header(header::AUTHORIZATION, "Bearer test-secret-key")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "model": "cx/image-gen",
                "prompt": "draw landscape",
                "reference_image": "file:///etc/passwd",
            })
            .to_string(),
        ))
        .unwrap();

    let resp_invalid = app.clone().oneshot(req_invalid).await.unwrap();
    assert_eq!(
        resp_invalid.status(),
        StatusCode::BAD_REQUEST,
        "Invalid reference image scheme must be rejected with 400 Bad Request"
    );

    // 2. Valid reference image passes validation, then fails closed with 503
    let b64_png = base64::engine::general_purpose::STANDARD.encode(SAMPLE_PNG);
    let valid_data_url = format!("data:image/png;base64,{b64_png}");

    let req_valid = Request::builder()
        .method("POST")
        .uri("/v1/images/generations")
        .header(header::AUTHORIZATION, "Bearer test-secret-key")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "model": "cx/image-gen",
                "prompt": "draw landscape",
                "reference_image": valid_data_url,
            })
            .to_string(),
        ))
        .unwrap();

    let resp_valid = app.oneshot(req_valid).await.unwrap();
    assert_eq!(
        resp_valid.status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "Valid reference image must pass validation and fail closed with 503 Service Unavailable"
    );

    let _ = std::fs::remove_dir_all(dir);
}
