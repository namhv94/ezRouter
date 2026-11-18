pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod live_monitor;
pub mod models;
pub mod provider;
pub mod routes;
pub mod state;
pub mod system_log;
pub mod token_saver;

use axum::{
    extract::DefaultBodyLimit,
    routing::{get, patch, post},
    Router,
};

pub const MAX_REQUEST_BODY_BYTES: usize = 100 * 1024 * 1024; // 100 MB request body limit

pub use config::{Config, DEFAULT_API_KEY, DEFAULT_DATA_DIR, DEFAULT_PORT, PROD_FORBIDDEN_PORT};
pub use db::{
    is_masked_key, mask_api_key, AccountRecord, AdminStats, ApiKey, ComboRecord,
    Database, ModelRequestSummary, ProviderRecord, ProviderResponse, RequestLogItem,
    RequestsResponse,
};
pub use live_monitor::{
    mask_account_email, ActiveRequestItem, ActiveRequestRegistry, ActiveRequestsResponse,
};
pub use provider::{
    resolve_chat_completions_url, resolve_embeddings_url,
    BoxChatStream, ChatChoice, ChatChoiceMessage, ChatChunkChoice,
    ChatChunkDelta, ChatCompletionChunk, ChatCompletionRequest, ChatCompletionResponse,
    ChatMessage, EmbeddingData, EmbeddingInput, EmbeddingRequest, EmbeddingResponse,
    EmbeddingUsage, HttpUpstreamProvider, MockProvider, Provider, UsageInfo,
    MAX_RESPONSE_BYTES,
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
        .route("/scripts/*path", get(routes::ui::serve_scripts))
        .route(
            "/setup-claude-code.sh",
            get(routes::ui::serve_setup_claude_code_sh),
        )
        .route(
            "/setup-claude-code.bat",
            get(routes::ui::serve_setup_claude_code_bat),
        )
        .route(
            "/setup-claude-code.ps1",
            get(routes::ui::serve_setup_claude_code_ps1),
        )
        .route(
            "/uninstall-claude-code.sh",
            get(routes::ui::serve_uninstall_claude_code_sh),
        )
        .route(
            "/uninstall-claude-code.bat",
            get(routes::ui::serve_uninstall_claude_code_bat),
        )
        .route(
            "/uninstall-claude-code.ps1",
            get(routes::ui::serve_uninstall_claude_code_ps1),
        )
        .route("/health", get(routes::health::health))
        .route("/v1/models", get(routes::models::list_models))
        .route("/v1/models/*model_id", get(routes::models::retrieve_model))
        .route("/v1/chat/completions", post(routes::chat::chat_completions))
        .route("/v1/messages", post(routes::messages::post_messages))
        .route(
            "/v1/responses",
            post(routes::responses::post_responses).get(routes::responses::get_responses),
        )
        .route("/v1/embeddings", post(routes::embeddings::embeddings))
        .route(
            "/v1/images/generations",
            post(routes::images::image_generations),
        )
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
            "/admin/token-analytics",
            get(routes::admin::get_token_analytics),
        )
        .route(
            "/admin/maintenance/cleanup-logs",
            post(routes::admin::cleanup_request_logs),
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
        .layer(DefaultBodyLimit::max(MAX_REQUEST_BODY_BYTES))
        .with_state(state)
}