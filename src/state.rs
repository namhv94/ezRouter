use std::sync::Arc;

use crate::account::{AccountPool, MockTokenRefresher, TokenRefresher};
use crate::codex::{CodexAccountPool, CodexProvider, OpenAiCodexTokenRefresher};
use crate::config::Config;
use crate::db::Database;
use crate::live_monitor::ActiveRequestRegistry;
use crate::models::ModelRegistry;
use crate::provider::{HttpUpstreamProvider, MockProvider, Provider};
use crate::quota_refresh::QuotaRefreshWorker;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub models: Arc<ModelRegistry>,
    pub provider: Arc<dyn Provider>,
    pub db: Arc<Database>,
    pub account_pool: Arc<AccountPool>,
    pub codex_pool: Arc<CodexAccountPool>,
    pub codex_provider: Arc<dyn Provider>,
    pub quota_worker: Arc<QuotaRefreshWorker>,
    pub live_registry: Arc<ActiveRequestRegistry>,
    pub system_logs: Arc<crate::system_log::SystemLogBuffer>,
    pub latency_store: Arc<crate::latency::LatencyStore>,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("config", &self.config)
            .field("models", &self.models)
            .field("provider", &self.provider)
            .field("db", &self.db)
            .field("account_pool", &self.account_pool)
            .field("codex_pool", &self.codex_pool)
            .field("codex_provider", &self.codex_provider)
            .field("quota_worker", &"[QuotaRefreshWorker]")
            .field("live_registry", &"[ActiveRequestRegistry]")
            .field("system_logs", &"[SystemLogBuffer]")
            .field("latency_store", &"[LatencyStore]")
            .finish()
    }
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let db = Database::open_or_create(&config.data_dir, Some(&config.api_key))
            .expect("Failed to initialize database");
        let db = Arc::new(db);
        let ag_refresher: Arc<dyn TokenRefresher> = if config.use_mock_provider {
            Arc::new(MockTokenRefresher::new())
        } else {
            Arc::new(crate::account::GoogleTokenRefresher::new())
        };
        let ag_quota_fetcher: Arc<dyn crate::account::GoogleQuotaFetcher> =
            if config.use_mock_provider {
                Arc::new(crate::account::MockGoogleQuotaFetcher::default())
            } else {
                Arc::new(crate::account::DefaultGoogleQuotaFetcher::new())
            };
        let pool = Arc::new(AccountPool::with_components(
            db.clone(),
            ag_refresher.clone(),
            ag_quota_fetcher,
            0.0,
            60,
        ));
        let _ = pool.load_from_db();

        let cx_refresher = Arc::new(OpenAiCodexTokenRefresher::new());
        let codex_pool = Arc::new(CodexAccountPool::with_refresher(db.clone(), cx_refresher));
        let _ = codex_pool.load_from_db();

        let use_antigravity = std::env::var("AG_PROVIDER")
            .or_else(|_| std::env::var("AG_UPSTREAM_PROVIDER"))
            .map(|p| p.eq_ignore_ascii_case("antigravity"))
            .unwrap_or_else(|_| config.upstream_api_key.is_none() && pool.account_count() > 0);

        let provider: Arc<dyn Provider> = if config.use_mock_provider {
            Arc::new(MockProvider::new())
        } else if use_antigravity {
            let ag_provider = if let Some(ref base_url) = config.upstream_base_url {
                crate::provider::AntigravityProvider::with_base_url(
                    pool.clone(),
                    ag_refresher,
                    base_url,
                )
            } else {
                crate::provider::AntigravityProvider::new(pool.clone(), ag_refresher)
            };
            Arc::new(ag_provider)
        } else {
            Arc::new(HttpUpstreamProvider::new(
                config.upstream_base_url.clone(),
                config.upstream_api_key.clone(),
                config.upstream_connect_timeout_secs,
                config.upstream_read_timeout_secs,
                config.upstream_request_timeout_secs,
            ))
        };

        let latency_store = Arc::new(crate::latency::LatencyStore::new(
            config.codex_latency_telemetry_enabled,
        ));

        let codex_provider: Arc<dyn Provider> = if config.use_mock_provider {
            Arc::new(MockProvider::new())
        } else {
            Arc::new(
                CodexProvider::new(codex_pool.clone())
                    .with_latency_store(latency_store.clone())
                    .with_native_non_stream(config.codex_native_non_stream_enabled),
            )
        };

        let quota_worker = Arc::new(QuotaRefreshWorker::new(
            db.clone(),
            pool.clone(),
            codex_pool.clone(),
        ));
        quota_worker.clone().start_background_task();

        Self {
            config: Arc::new(config),
            models: Arc::new(ModelRegistry::new()),
            provider,
            db,
            account_pool: pool,
            codex_pool,
            codex_provider,
            quota_worker,
            live_registry: Arc::new(ActiveRequestRegistry::new()),
            system_logs: crate::system_log::SystemLogBuffer::global(),
            latency_store,
        }
    }

    pub fn with_antigravity(
        config: Config,
        refresher: Arc<dyn TokenRefresher>,
        base_url: Option<&str>,
    ) -> Self {
        let latency_store = Arc::new(crate::latency::LatencyStore::new(
            config.codex_latency_telemetry_enabled,
        ));
        let db = Database::open_or_create(&config.data_dir, Some(&config.api_key))
            .expect("Failed to initialize database");
        let db = Arc::new(db);
        let ag_quota_fetcher: Arc<dyn crate::account::GoogleQuotaFetcher> =
            if config.use_mock_provider {
                Arc::new(crate::account::MockGoogleQuotaFetcher::default())
            } else {
                Arc::new(crate::account::DefaultGoogleQuotaFetcher::new())
            };
        let pool = Arc::new(AccountPool::with_components(
            db.clone(),
            refresher.clone(),
            ag_quota_fetcher,
            0.0,
            60,
        ));
        let _ = pool.load_from_db();

        let codex_pool = Arc::new(CodexAccountPool::new(db.clone()));
        let _ = codex_pool.load_from_db();

        let ag_provider = if let Some(url) = base_url {
            crate::provider::AntigravityProvider::with_base_url(pool.clone(), refresher, url)
        } else {
            crate::provider::AntigravityProvider::new(pool.clone(), refresher)
        };

        let codex_provider: Arc<dyn Provider> = Arc::new(MockProvider::new());
        let quota_worker = Arc::new(QuotaRefreshWorker::new(
            db.clone(),
            pool.clone(),
            codex_pool.clone(),
        ));

        Self {
            config: Arc::new(config),
            models: Arc::new(ModelRegistry::new()),
            provider: Arc::new(ag_provider),
            db,
            account_pool: pool,
            codex_pool,
            codex_provider,
            quota_worker,
            live_registry: Arc::new(ActiveRequestRegistry::new()),
            system_logs: crate::system_log::SystemLogBuffer::global(),
            latency_store,
        }
    }

    pub fn with_codex(
        config: Config,
        codex_provider: Arc<dyn Provider>,
        codex_pool: Arc<CodexAccountPool>,
    ) -> Self {
        let latency_store = Arc::new(crate::latency::LatencyStore::new(
            config.codex_latency_telemetry_enabled,
        ));
        let db = codex_pool.db().clone();
        let pool = Arc::new(AccountPool::new(db.clone()));
        let _ = pool.load_from_db();
        let quota_worker = Arc::new(QuotaRefreshWorker::new(
            db.clone(),
            pool.clone(),
            codex_pool.clone(),
        ));

        Self {
            config: Arc::new(config),
            models: Arc::new(ModelRegistry::new()),
            provider: Arc::new(MockProvider::new()),
            db,
            account_pool: pool,
            codex_pool,
            codex_provider,
            quota_worker,
            live_registry: Arc::new(ActiveRequestRegistry::new()),
            system_logs: crate::system_log::SystemLogBuffer::global(),
            latency_store,
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
        let latency_store = Arc::new(crate::latency::LatencyStore::new(
            config.codex_latency_telemetry_enabled,
        ));
        let pool = Arc::new(AccountPool::new(db.clone()));
        let _ = pool.load_from_db();
        let codex_pool = Arc::new(CodexAccountPool::new(db.clone()));
        let _ = codex_pool.load_from_db();
        let codex_provider: Arc<dyn Provider> = Arc::new(MockProvider::new());
        let quota_worker = Arc::new(QuotaRefreshWorker::new(
            db.clone(),
            pool.clone(),
            codex_pool.clone(),
        ));

        Self {
            config: Arc::new(config),
            models: Arc::new(ModelRegistry::new()),
            provider,
            db,
            account_pool: pool,
            codex_pool,
            codex_provider,
            quota_worker,
            live_registry: Arc::new(ActiveRequestRegistry::new()),
            system_logs: crate::system_log::SystemLogBuffer::global(),
            latency_store,
        }
    }

    pub fn with_provider_db_and_pool(
        config: Config,
        provider: Arc<dyn Provider>,
        db: Arc<Database>,
        account_pool: Arc<AccountPool>,
    ) -> Self {
        let latency_store = Arc::new(crate::latency::LatencyStore::new(
            config.codex_latency_telemetry_enabled,
        ));
        let codex_pool = Arc::new(CodexAccountPool::new(db.clone()));
        let _ = codex_pool.load_from_db();
        let codex_provider: Arc<dyn Provider> = Arc::new(MockProvider::new());
        let quota_worker = Arc::new(QuotaRefreshWorker::new(
            db.clone(),
            account_pool.clone(),
            codex_pool.clone(),
        ));

        Self {
            config: Arc::new(config),
            models: Arc::new(ModelRegistry::new()),
            provider,
            db,
            account_pool,
            codex_pool,
            codex_provider,
            quota_worker,
            live_registry: Arc::new(ActiveRequestRegistry::new()),
            system_logs: crate::system_log::SystemLogBuffer::global(),
            latency_store,
        }
    }
}
