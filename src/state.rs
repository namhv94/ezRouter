use std::sync::Arc;

use crate::config::Config;
use crate::db::Database;
use crate::live_monitor::ActiveRequestRegistry;
use crate::models::ModelRegistry;
use crate::provider::{HttpUpstreamProvider, MockProvider, Provider};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub models: Arc<ModelRegistry>,
    pub provider: Arc<dyn Provider>,
    pub db: Arc<Database>,
    pub live_registry: Arc<ActiveRequestRegistry>,
    pub system_logs: Arc<crate::system_log::SystemLogBuffer>,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("config", &self.config)
            .field("models", &self.models)
            .field("provider", &self.provider)
            .field("db", &self.db)
            .field("live_registry", &"[ActiveRequestRegistry]")
            .field("system_logs", &"[SystemLogBuffer]")
            .finish()
    }
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let db = Database::open_or_create(&config.data_dir, Some(&config.api_key))
            .expect("Failed to initialize database");
        let db = Arc::new(db);

        let provider: Arc<dyn Provider> = if config.use_mock_provider {
            Arc::new(MockProvider::new())
        } else {
            Arc::new(HttpUpstreamProvider::new(
                config.upstream_base_url.clone(),
                config.upstream_api_key.clone(),
                None,
                config.upstream_connect_timeout_secs,
                config.upstream_read_timeout_secs,
                config.upstream_request_timeout_secs,
            ))
        };

        Self {
            config: Arc::new(config),
            models: Arc::new(ModelRegistry::new()),
            provider,
            db,
            live_registry: Arc::new(ActiveRequestRegistry::new()),
            system_logs: crate::system_log::SystemLogBuffer::global(),
        }
    }

    pub fn with_provider(config: Config, provider: Arc<dyn Provider>) -> Self {
        let db = Database::open_or_create(&config.data_dir, Some(&config.api_key))
            .expect("Failed to initialize database");
        Self::with_provider_and_db(config, provider, Arc::new(db))
    }

    pub fn with_provider_and_db(
        config: Config,
        provider: Arc<dyn Provider>,
        db: Arc<Database>,
    ) -> Self {
        Self {
            config: Arc::new(config),
            models: Arc::new(ModelRegistry::new()),
            provider,
            db,
            live_registry: Arc::new(ActiveRequestRegistry::new()),
            system_logs: crate::system_log::SystemLogBuffer::global(),
        }
    }
}