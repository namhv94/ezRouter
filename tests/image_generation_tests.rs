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
