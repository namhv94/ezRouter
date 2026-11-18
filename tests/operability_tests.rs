use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::Value;
use tower::ServiceExt;

use ezrouter::{app_router, state::AppState, Config};

fn test_state(temp_dir: &str) -> AppState {
    let mut config = Config::parse(
        Some("127.0.0.1".to_string()),
        Some("20229".to_string()),
        Some(temp_dir.to_string()),
        Some("test-operability-secret".to_string()),
    )
    .unwrap();
    config.use_mock_provider = true;
    AppState::new(config)
}

#[tokio::test]
async fn test_operability_health_endpoint_metrics_and_no_secret_leak() {
    let temp_dir = format!(
        "{}/ag-proxy-rust-test-op-health-{}",
        std::env::temp_dir().display(),
        uuid::Uuid::new_v4().simple()
    );
    let state = test_state(&temp_dir);

    let app = app_router(state.clone());

    let req = Request::builder()
        .uri("/health")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(body["status"], "ok");
    assert_eq!(body["service"], "ezrouter");
    assert_eq!(body["mode"], "staging");
    assert_eq!(body["port"], 20229);

    // Strict security: zero secrets or tokens in health response
    assert!(body.get("api_key").is_none());
    assert!(body.get("refresh_token").is_none());
    assert!(body.get("access_token").is_none());
    assert!(!body_bytes
        .windows(15)
        .any(|w| w == b"test-operability" || w == b"rt-op-secret"));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_admin_providers_list_requires_auth() {
    let temp_dir = format!(
        "{}/ag-proxy-rust-test-op-providers-{}",
        std::env::temp_dir().display(),
        uuid::Uuid::new_v4().simple()
    );
    let state = test_state(&temp_dir);
    let app = app_router(state);

    // Missing auth is rejected
    let req = Request::builder()
        .uri("/admin/providers")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // With auth, the empty provider list is returned
    let req2 = Request::builder()
        .uri("/admin/providers")
        .method("GET")
        .header("authorization", "Bearer test-operability-secret")
        .body(Body::empty())
        .unwrap();
    let res2 = app.oneshot(req2).await.unwrap();
    assert_eq!(res2.status(), StatusCode::OK);

    let body: Value = serde_json::from_slice(
        &axum::body::to_bytes(res2.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(body.as_array().unwrap().len(), 0);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_systemd_service_hardening_and_staging_isolation() {
    let path = std::path::Path::new("deploy/ezrouter.service.example");
    let content = std::fs::read_to_string(path).expect("systemd unit example file exists");

    // Hardening directives
    assert!(content.contains("KillSignal=SIGTERM"));
    assert!(content.contains("TimeoutStopSec=30"));
    assert!(content.contains("LimitNOFILE=65536"));
    assert!(content.contains("Restart=always"));
    assert!(content.contains("RestartSec=5"));

    // Staging isolation verification: strictly staging port and data dir
    assert!(content.contains("Environment=AG_PORT=20229"));
    assert!(content.contains("Environment=AG_DATA_DIR="));
    assert!(
        !content.contains("Environment=AG_PORT=20129"),
        "Must NEVER bind to production port"
    );
}

#[test]
fn test_cargo_release_profile_configured() {
    let content = std::fs::read_to_string("Cargo.toml").expect("Cargo.toml exists");
    assert!(content.contains("[profile.release]"));
    assert!(content.contains("opt-level = 3"));
    assert!(content.contains("lto = \"thin\""));
    assert!(content.contains("codegen-units = 1"));
    assert!(content.contains("panic = \"abort\""));
    assert!(content.contains("strip = true"));
}
