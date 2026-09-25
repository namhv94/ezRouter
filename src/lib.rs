pub mod account;
pub mod auth;
pub mod codex;
pub mod config;
pub mod context_optimizer;
pub mod db;
pub mod error;
pub mod latency;
pub mod live_monitor;
pub mod models;
pub mod provider;
pub mod quota_refresh;
pub mod routes;
pub mod state;
pub mod system_log;
pub mod token_saver;

use axum::{
    routing::{get, patch, post},
    Router,
};

pub use account::{
    parse_google_quota_summary, AccountActionResponse, AccountPool, AccountQuotaResponse,
    AccountResponse, CreateAccountRequest, CreateAccountResponse, DefaultGoogleQuotaFetcher,
    GoogleQuotaFetcher, GoogleTokenRefresher, ListAccountsResponse, MockGoogleQuotaFetcher,
    MockTokenRefresher, RefreshAllAccountsQuotaResponse, TokenRefreshOutput, TokenRefresher,
    AG_BASE_URL, AG_CLIENT_ID, AG_CLIENT_SECRET, AG_QUOTA_URL, AG_TOKEN_URL, AG_USER_AGENT,
    QUOTA_CACHE_TTL_SECS,
};
pub use codex::{
    extract_token_identity, generate_pkce_pair, jwt_claims_unverified, mask_codex_error,
    openai_to_codex_input, parse_codex_native_response, parse_codex_response_body, CodexAccount,
    CodexAccountPool, CodexOAuthTicket, CodexPool, CodexProvider, CodexQuotaFetcher,
    CodexSseStream, CodexTokenRefreshOutput, CodexTokenRefresher, DefaultCodexTokenRefresher,
    MockCodexQuotaFetcher, MockCodexTokenRefresher, OpenAiCodexTokenRefresher, CODEX_AUTHORIZE_URL,
    CODEX_BASE_URL, CODEX_CLIENT_ID, CODEX_MAX_RPM, CODEX_MIN_GAP_S,
    CODEX_NATIVE_NON_STREAM_ENABLED, CODEX_OAUTH_REDIRECT_URI, CODEX_OAUTH_SCOPES,
    CODEX_OAUTH_TICKET_TTL_S, CODEX_ORIGINATOR, CODEX_QUOTA_STOP_PERCENT, CODEX_RESPONSES_URL,
    CODEX_TOKEN_REFRESH_LEAD_S, CODEX_TOKEN_URL, CODEX_USAGE_URL, CODEX_USER_AGENT,
};
pub use config::{Config, DEFAULT_API_KEY, DEFAULT_DATA_DIR, DEFAULT_PORT, PROD_FORBIDDEN_PORT};
pub use context_optimizer::{
    compact_messages, measure_context, ContextMetrics, ContextOptimizerConfig,
};
pub use db::{
    is_masked_key, mask_api_key, AccountRecord, AdminStats, ApiKey, CodexAccountRecord,
    ComboRecord, Database, ModelRequestSummary, ProviderRecord, ProviderResponse, RequestLogItem,
    RequestsResponse,
};
pub use latency::{CodexLatencyTrace, LatencyAggResponse, LatencyStore};
pub use live_monitor::{
    mask_account_email, ActiveRequestItem, ActiveRequestRegistry, ActiveRequestsResponse,
};
pub use provider::{
    build_antigravity_payload, resolve_chat_completions_url, AntigravityProvider, BoxChatStream,
    ChatChoice, ChatChoiceMessage, ChatChunkChoice, ChatChunkDelta, ChatCompletionChunk,
    ChatCompletionRequest, ChatCompletionResponse, ChatMessage, GeminiSseStream,
    HttpUpstreamProvider, MockProvider, Provider, UsageInfo, AG_PROJECT, AG_STREAM_URL_PATH,
    MAX_RESPONSE_BYTES,
};
pub use quota_refresh::{
    sanitize_error_message, QuotaRefreshStatusResponse, QuotaRefreshSummary, QuotaRefreshWorker,
    DEFAULT_MAX_REFRESH_CONCURRENCY, DEFAULT_QUOTA_REFRESH_COOLDOWN_SECS,
    DEFAULT_QUOTA_REFRESH_INTERVAL_SECS, MIN_QUOTA_REFRESH_INTERVAL_SECS,
};
pub use state::AppState;
pub use token_saver::{
    apply_token_saver, apply_token_saver_with_request_context, TokenSaverSettings, TokenSaverStats,
};

pub fn app_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(routes::ui::serve_root))
        .route("/assets/*path", get(routes::ui::serve_assets))
        .route("/favicon.ico", get(routes::ui::serve_favicon))
        .route("/health", get(routes::health::health))
        .route("/auth/login", get(routes::auth::auth_login))
        .route("/auth/callback", get(routes::auth::auth_callback))
        .route("/auth/success", get(routes::auth::auth_success))
        .route("/v1/models", get(routes::models::list_models))
        .route("/v1/models/*model_id", get(routes::models::retrieve_model))
        .route("/v1/chat/completions", post(routes::chat::chat_completions))
        .route(
            "/admin/api-keys",
            get(routes::admin::list_api_keys).post(routes::admin::create_api_key),
        )
        .route(
            "/admin/api-keys/:id",
            patch(routes::admin::update_api_key).delete(routes::admin::delete_api_key),
        )
        .route("/admin/stats", get(routes::admin::get_stats))
        .route(
            "/admin/settings",
            get(routes::admin::get_token_saver_settings)
                .post(routes::admin::update_token_saver_settings),
        )
        .route("/admin/requests", get(routes::admin::get_requests))
        .route(
            "/admin/system-logs",
            get(routes::admin::get_system_logs).delete(routes::admin::clear_system_logs),
        )
        .route(
            "/admin/active-requests",
            get(routes::admin::get_active_requests),
        )
        .route(
            "/admin/active-requests/stream",
            get(routes::admin::get_active_requests_stream),
        )
        .route(
            "/admin/request-summary",
            get(routes::admin::get_request_summary),
        )
        .route(
            "/admin/providers",
            get(routes::admin::list_providers)
                .post(routes::admin::create_provider)
                .put(routes::admin::update_provider_root)
                .delete(routes::admin::delete_provider_root),
        )
        .route(
            "/admin/providers/fetch-models",
            post(routes::admin::fetch_models),
        )
        .route(
            "/admin/providers/:id",
            get(routes::admin::get_provider)
                .put(routes::admin::update_provider)
                .delete(routes::admin::delete_provider),
        )
        .route(
            "/admin/providers/:id/test",
            post(routes::admin::test_provider),
        )
        .route(
            "/admin/providers/:id/sync-models",
            post(routes::admin::sync_models),
        )
        .route(
            "/admin/combos",
            get(routes::admin::list_combos)
                .post(routes::admin::create_combo)
                .put(routes::admin::update_combo_root)
                .delete(routes::admin::delete_combo_root),
        )
        .route(
            "/admin/combos/:id",
            get(routes::admin::get_combo)
                .put(routes::admin::update_combo)
                .delete(routes::admin::delete_combo),
        )
        .route(
            "/admin/accounts",
            get(routes::admin::list_accounts)
                .post(routes::admin::create_account)
                .delete(routes::admin::delete_account_root),
        )
        .route(
            "/admin/accounts/refresh-quota",
            post(routes::admin::refresh_all_accounts_quota),
        )
        .route(
            "/admin/accounts/:id",
            get(routes::admin::get_account).delete(routes::admin::delete_account),
        )
        .route(
            "/admin/accounts/:id/reset",
            post(routes::admin::reset_account_cooldown),
        )
        .route(
            "/admin/accounts/:id/refresh-quota",
            post(routes::admin::refresh_account_quota),
        )
        .route(
            "/admin/accounts/:id/test",
            post(routes::admin::test_account),
        )
        .route(
            "/admin/accounts/oauth/start",
            post(routes::admin::start_google_oauth).get(routes::admin::start_google_oauth),
        )
        .route(
            "/admin/accounts/oauth/exchange",
            post(routes::admin::exchange_google_oauth),
        )
        .route("/admin/import-9router", post(routes::admin::import_9router))
        .route("/admin/codex", get(routes::admin::get_codex_status))
        .route(
            "/admin/codex/latency",
            get(routes::admin::get_codex_latency),
        )
        .route(
            "/admin/codex/accounts",
            get(routes::admin::list_codex_accounts).post(routes::admin::create_codex_account),
        )
        .route(
            "/admin/codex/accounts/:id/toggle",
            post(routes::admin::toggle_codex_account),
        )
        .route(
            "/admin/codex/accounts/:id/reset",
            post(routes::admin::reset_codex_account),
        )
        .route(
            "/admin/codex/accounts/:id/refresh-quota",
            post(routes::admin::refresh_codex_account_quota),
        )
        .route(
            "/admin/codex/accounts/:id",
            axum::routing::delete(routes::admin::delete_codex_account),
        )
        .route(
            "/admin/codex/refresh-quota",
            post(routes::admin::refresh_codex_quota),
        )
        .route(
            "/admin/codex/oauth/start",
            post(routes::admin::start_codex_oauth),
        )
        .route(
            "/admin/codex/oauth/exchange",
            post(routes::admin::exchange_codex_oauth),
        )
        .route(
            "/admin/codex/oauth/status",
            get(routes::admin::get_codex_oauth_status),
        )
        .route(
            "/admin/quota-refresh",
            get(routes::admin::get_quota_refresh_status)
                .post(routes::admin::update_quota_refresh_settings),
        )
        .route(
            "/admin/quota-refresh/run",
            post(routes::admin::run_quota_refresh),
        )
        .with_state(state)
}
