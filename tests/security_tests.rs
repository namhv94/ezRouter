use ezrouter::{app_router, config::Config, db::Database, state::AppState};
use axum::{
    body::Body,
    http::{header::AUTHORIZATION, Request, StatusCode},
    response::IntoResponse,
};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

fn setup_test_app() -> (axum::Router, AppState, String) {
    let test_dir = format!(
        "/tmp/ag-proxy-rust-test-sec-{}",
        uuid::Uuid::new_v4().simple()
    );
    let mut config = Config::parse(
        Some("127.0.0.1".to_string()),
        Some("20229".to_string()),
        Some(test_dir.clone()),
        Some("admin-master-key".to_string()),
    )
    .unwrap();
    config.use_mock_provider = true;
    let state = AppState::new(config);
    let app = app_router(state.clone());
    (app, state, test_dir)
}

#[tokio::test]
async fn test_rbac_client_vs_admin_on_admin_routes() {
    let (app, state, test_dir) = setup_test_app();

    // 1. Create a client key (default role: client)
    let client_key_record = state
        .db
        .create_key("Client Service", Some("sk-client-key-1234"))
        .unwrap();
    assert_eq!(client_key_record.role, "client");

    // 2. Client key can access /v1/models
    let req = Request::builder()
        .uri("/v1/models")
        .method("GET")
        .header(AUTHORIZATION, "Bearer sk-client-key-1234")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 3. Client key can access /v1/chat/completions
    let chat_payload = serde_json::json!({
        "model": "ag/gemini-3.8-flash-high",
        "messages": [{"role": "user", "content": "hi"}],
        "stream": false
    });
    let req = Request::builder()
        .uri("/v1/chat/completions")
        .method("POST")
        .header(AUTHORIZATION, "Bearer sk-client-key-1234")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&chat_payload).unwrap()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 4. Client key is rejected with 403 Forbidden on all admin routes
    let admin_endpoints = [
        ("GET", "/admin/stats"),
        ("GET", "/admin/requests"),
        ("GET", "/admin/request-summary"),
        ("GET", "/admin/api-keys"),
        ("GET", "/admin/providers"),
        ("GET", "/admin/combos"),
        ("GET", "/admin/accounts"),
        ("GET", "/admin/settings"),
        ("GET", "/admin/active-requests"),
        ("GET", "/admin/codex"),
        ("GET", "/admin/quota-refresh"),
    ];

    for (method, uri) in admin_endpoints {
        let req = Request::builder()
            .uri(uri)
            .method(method)
            .header(AUTHORIZATION, "Bearer sk-client-key-1234")
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(
            res.status(),
            StatusCode::FORBIDDEN,
            "Expected 403 FORBIDDEN for client key on {uri}"
        );
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let json: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["error"]["code"], "forbidden");
    }

    // 5. Admin key can access admin routes with 200 OK
    let req = Request::builder()
        .uri("/admin/stats")
        .method("GET")
        .header(AUTHORIZATION, "Bearer admin-master-key")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let _ = std::fs::remove_dir_all(&test_dir);
}

#[tokio::test]
async fn test_rbac_admin_key_creation_and_access() {
    let (app, _state, test_dir) = setup_test_app();

    // 1. Create a dynamic admin key via POST /admin/api-keys with role="admin"
    let create_payload = serde_json::json!({
        "name": "Co-Admin Key",
        "key": "sk-co-admin-5678",
        "role": "admin"
    });
    let req = Request::builder()
        .uri("/admin/api-keys")
        .method("POST")
        .header(AUTHORIZATION, "Bearer admin-master-key")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&create_payload).unwrap()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let created: Value =
        serde_json::from_slice(&res.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(created["role"], "admin");

    // 2. Co-admin key can access admin routes
    let req = Request::builder()
        .uri("/admin/stats")
        .method("GET")
        .header(AUTHORIZATION, "Bearer sk-co-admin-5678")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 3. Create a dynamic client key via POST /admin/api-keys (omitting role defaults to client)
    let create_payload2 = serde_json::json!({
        "name": "Regular App",
        "key": "sk-app-key-9999"
    });
    let req = Request::builder()
        .uri("/admin/api-keys")
        .method("POST")
        .header(AUTHORIZATION, "Bearer admin-master-key")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&create_payload2).unwrap()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let created2: Value =
        serde_json::from_slice(&res.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(created2["role"], "client");

    // 4. Regular app cannot access admin routes
    let req = Request::builder()
        .uri("/admin/stats")
        .method("GET")
        .header(AUTHORIZATION, "Bearer sk-app-key-9999")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    let _ = std::fs::remove_dir_all(&test_dir);
}

#[tokio::test]
async fn test_query_token_restriction_and_leak_prevention() {
    let (app, state, test_dir) = setup_test_app();

    // Create client key
    let _client_key = state
        .db
        .create_key("Client", Some("sk-client-query-test"))
        .unwrap();

    // 1. Query token is rejected on /v1/models
    let req = Request::builder()
        .uri("/v1/models?token=admin-master-key")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    let req = Request::builder()
        .uri("/v1/models?key=admin-master-key")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // 2. Query token is rejected on /v1/chat/completions
    let chat_payload = serde_json::json!({
        "model": "ag/gemini-3.8-flash-high",
        "messages": [{"role": "user", "content": "hi"}],
        "stream": false
    });
    let req = Request::builder()
        .uri("/v1/chat/completions?token=admin-master-key")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&chat_payload).unwrap()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // 3. Query token is rejected on admin mutation and general read routes
    let req = Request::builder()
        .uri("/admin/stats?token=admin-master-key")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    let req = Request::builder()
        .uri("/admin/api-keys?token=admin-master-key")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // 4. Query token IS accepted on /admin/active-requests for admin key (EventSource support)
    let req = Request::builder()
        .uri("/admin/active-requests?token=admin-master-key")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 5. Query token on /admin/active-requests with client key is rejected with 403 FORBIDDEN
    let req = Request::builder()
        .uri("/admin/active-requests?token=sk-client-query-test")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    let _ = std::fs::remove_dir_all(&test_dir);
}

#[tokio::test]
async fn test_oauth_open_redirect_prevention() {
    let (app, state, test_dir) = setup_test_app();

    // 1. Untrusted origin in /admin/accounts/oauth/start is stripped/ignored
    let req = Request::builder()
        .uri("/admin/accounts/oauth/start?origin=https%3A%2F%2Fattacker.example")
        .method("POST")
        .header(AUTHORIZATION, "Bearer admin-master-key")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json: Value =
        serde_json::from_slice(&res.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let state_str = json["state"].as_str().unwrap();
    assert!(
        !state_str.contains("attacker.example"),
        "State should not contain untrusted origin"
    );

    // 2. Whitelisted origin in /admin/accounts/oauth/start IS preserved
    let req = Request::builder()
        .uri("/admin/accounts/oauth/start?origin=https%3A%2F%2Frouter.example.com")
        .method("POST")
        .header(AUTHORIZATION, "Bearer admin-master-key")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json: Value =
        serde_json::from_slice(&res.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let state_str = json["state"].as_str().unwrap();
    assert!(state_str.starts_with("https://router.example.com|"));

    // 3. Mock OAuth exchange server to test callback flow
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let mock_oauth_server = axum::Router::new()
        .route(
            "/token",
            axum::routing::post(|| async move {
                (
                    StatusCode::OK,
                    [("content-type", "application/json")],
                    r#"{"access_token":"mock-token","refresh_token":"rt-safe","expires_in":3600}"#,
                )
                    .into_response()
            }),
        )
        .route(
            "/userinfo",
            axum::routing::get(|| async move {
                (
                    StatusCode::OK,
                    [("content-type", "application/json")],
                    r#"{"email":"oauth-tester@example.com","id":"12345"}"#,
                )
                    .into_response()
            }),
        );
    tokio::spawn(async move {
        axum::serve(listener, mock_oauth_server).await.unwrap();
    });

    std::env::set_var("AG_TOKEN_URL", format!("http://127.0.0.1:{port}/token"));
    std::env::set_var(
        "AG_USERINFO_URL",
        format!("http://127.0.0.1:{port}/userinfo"),
    );
    std::env::set_var("AG_GOOGLE_CLIENT_SECRET", "mock-secret");

    // Craft state with attacker.example
    let ticket = state
        .account_pool
        .start_oauth("http://127.0.0.1:20229/auth/callback", None);
    let raw_state = format!("https://attacker.example|{}", ticket.state);
    let encoded_state = ezrouter::account::urlencoding_encode(&raw_state);

    let req = Request::builder()
        .uri(format!(
            "/auth/callback?code=mock-code&state={encoded_state}"
        ))
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let location = res
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        !location.contains("attacker.example"),
        "Must NOT redirect to attacker domain: {location}"
    );
    assert!(
        location.starts_with("/auth/success"),
        "Must fall back to local /auth/success: {location}"
    );

    std::env::remove_var("AG_TOKEN_URL");
    std::env::remove_var("AG_USERINFO_URL");
    std::env::remove_var("AG_GOOGLE_CLIENT_SECRET");
    let _ = std::fs::remove_dir_all(&test_dir);
}

#[tokio::test]
async fn test_db_schema_role_migration_and_persistence() {
    let test_dir = format!(
        "/tmp/ag-proxy-rust-test-db-role-{}",
        uuid::Uuid::new_v4().simple()
    );
    let path = std::path::Path::new(&test_dir);
    let _ = std::fs::create_dir_all(path);
    let db_path = path.join("data.sqlite");

    // 1. Create a legacy table without role column using raw sqlite
    {
        let raw_conn = rusqlite::Connection::open(&db_path).unwrap();
        raw_conn
            .execute_batch(
                "CREATE TABLE api_keys (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    key TEXT UNIQUE NOT NULL,
                    is_active INTEGER DEFAULT 1,
                    total_requests INTEGER DEFAULT 0,
                    created_at TEXT
                );
                INSERT INTO api_keys VALUES ('legacy-id', 'Legacy Key', 'sk-legacy-1', 1, 5, '2026-01-01');",
            )
            .unwrap();
    }

    // 2. Open via Database::open_file -> schema migration must add role column automatically
    {
        let db = Database::open_file(&db_path, Some("master-seed")).unwrap();

        // Legacy installations treated active keys as trusted admin keys; preserve that
        // behavior during migration. Newly created keys default to client.
        let legacy = db.get_key("sk-legacy-1").unwrap().unwrap();
        assert_eq!(legacy.role, "admin");
        assert_eq!(legacy.total_requests, 5);

        // Verify newly created key with role
        let new_admin = db
            .create_key_with_role("New Admin", Some("sk-new-admin"), "admin")
            .unwrap();
        assert_eq!(new_admin.role, "admin");
    }

    // 3. Re-open and verify persistence
    {
        let db = Database::open_file(&db_path, None).unwrap();
        let new_admin = db.get_key("sk-new-admin").unwrap().unwrap();
        assert_eq!(new_admin.role, "admin");
        let legacy = db.get_key("sk-legacy-1").unwrap().unwrap();
        assert_eq!(legacy.role, "admin");
    }

    // 4. Fresh DB auto-seeds default key with role 'admin'
    {
        let fresh_dir = format!(
            "/tmp/ag-proxy-rust-test-db-fresh-{}",
            uuid::Uuid::new_v4().simple()
        );
        let fresh_path = std::path::Path::new(&fresh_dir);
        let _ = std::fs::create_dir_all(fresh_path);
        let db = Database::open_file(fresh_path.join("data.sqlite"), Some("master-seed")).unwrap();
        let master = db.get_key("master-seed").unwrap().unwrap();
        assert_eq!(master.role, "admin");
        let _ = std::fs::remove_dir_all(fresh_path);
    }

    let _ = std::fs::remove_dir_all(path);
}
