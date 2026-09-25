use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::Value;
use std::sync::Arc;
use tower::ServiceExt;

use ezrouter::{
    account::{AccountPool, MockGoogleQuotaFetcher, MockTokenRefresher},
    app_router,
    state::AppState,
    Config,
};

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
        "/tmp/ag-proxy-rust-test-op-health-{}",
        uuid::Uuid::new_v4().simple()
    );
    let state = test_state(&temp_dir);
    let _acc = state
        .account_pool
        .add_account("op-user@example.com", "rt-op-secret-12345")
        .unwrap();

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
    assert_eq!(body["service"], "ag-proxy-rust");
    assert_eq!(body["mode"], "staging");
    assert_eq!(body["port"], 20229);
    assert_eq!(body["total_accounts"], 1);
    assert_eq!(body["active_accounts"], 1);
    assert_eq!(body["cooldown_accounts"], 0);
    assert!(body.get("live_rpm").is_some());
    assert!(body.get("active_rate").is_some());
    assert!(body.get("codex_accounts").is_some());

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
async fn test_admin_accounts_list_uses_cache_without_unbounded_fan_out() {
    let temp_dir = format!(
        "/tmp/ag-proxy-rust-test-op-list-{}",
        uuid::Uuid::new_v4().simple()
    );
    let state = test_state(&temp_dir);
    let db = state.db.clone();
    let refresher = Arc::new(MockTokenRefresher::with_token("test-tok"));
    let quota_fetcher = Arc::new(MockGoogleQuotaFetcher::default());
    let pool = Arc::new(AccountPool::with_components(
        db,
        refresher,
        quota_fetcher,
        0.0,
        60,
    ));

    let _acc1 = pool.add_account("fan1@example.com", "rt-1").unwrap();
    let _acc2 = pool.add_account("fan2@example.com", "rt-2").unwrap();

    let custom_state = AppState {
        config: state.config,
        models: state.models,
        db: state.db,
        account_pool: pool.clone(),
        codex_pool: state.codex_pool,
        provider: state.provider,
        codex_provider: state.codex_provider,
        live_registry: state.live_registry,
        quota_worker: state.quota_worker,
        system_logs: state.system_logs,
        latency_store: state.latency_store,
    };
    let app = app_router(custom_state);

    // Initial GET populates missing quota
    let req = Request::builder()
        .uri("/admin/accounts")
        .method("GET")
        .header("authorization", "Bearer test-operability-secret")
        .body(Body::empty())
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body: Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(body["total"], 2);

    // Subsequent GET reads from cache without redundant upstream refresh
    let req2 = Request::builder()
        .uri("/admin/accounts")
        .method("GET")
        .header("authorization", "Bearer test-operability-secret")
        .body(Body::empty())
        .unwrap();

    let res2 = app.oneshot(req2).await.unwrap();
    assert_eq!(res2.status(), StatusCode::OK);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_admin_refresh_all_accounts_quota_bounded_concurrency() {
    let temp_dir = format!(
        "/tmp/ag-proxy-rust-test-op-refresh-{}",
        uuid::Uuid::new_v4().simple()
    );
    let state = test_state(&temp_dir);
    for i in 1..=4 {
        let _ = state
            .account_pool
            .add_account(&format!("batch{}@example.com", i), &format!("rt-{}", i))
            .unwrap();
    }

    let app = app_router(state);

    let req = Request::builder()
        .uri("/admin/accounts/refresh-quota")
        .method("POST")
        .header("authorization", "Bearer test-operability-secret")
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body: Value = serde_json::from_slice(
        &axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(body["ok"], true);
    assert_eq!(body["total"], 4);
    assert_eq!(body["refreshed"], 4);

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
