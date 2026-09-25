use std::env;
use std::fmt;
use std::path::{Path, PathBuf};

pub const DEFAULT_PORT: u16 = 20229;
pub const PROD_FORBIDDEN_PORT: u16 = 20129;
pub const DEFAULT_DATA_DIR: &str = "./data";
pub const PROD_DATA_DIR_SUFFIX: &str = ".ag-proxy";
pub const DEFAULT_API_KEY: &str = "ag-proxy-key";

pub const DEFAULT_UPSTREAM_CONNECT_TIMEOUT_SECS: u64 = 10;
pub const DEFAULT_UPSTREAM_READ_TIMEOUT_SECS: u64 = 60;
pub const DEFAULT_UPSTREAM_REQUEST_TIMEOUT_SECS: u64 = 120;
pub const DEFAULT_OAUTH_REDIRECT_URI: &str = "http://localhost:20229/auth/callback";

#[derive(Clone, PartialEq, Eq)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub data_dir: PathBuf,
    pub api_key: String,
    pub upstream_base_url: Option<String>,
    pub upstream_api_key: Option<String>,
    pub use_mock_provider: bool,
    pub upstream_connect_timeout_secs: u64,
    pub upstream_read_timeout_secs: u64,
    pub upstream_request_timeout_secs: u64,
    pub oauth_redirect_uri: String,
    pub codex_latency_telemetry_enabled: bool,
    pub codex_native_non_stream_enabled: bool,
    pub codex_context_optimizer_enabled: bool,
    pub codex_context_max_messages: usize,
    pub codex_context_max_bytes: usize,
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("data_dir", &self.data_dir)
            .field("api_key", &"[REDACTED]")
            .field("upstream_base_url", &self.upstream_base_url)
            .field(
                "upstream_api_key",
                &self.upstream_api_key.as_ref().map(|_| "[REDACTED]"),
            )
            .field("use_mock_provider", &self.use_mock_provider)
            .field(
                "upstream_connect_timeout_secs",
                &self.upstream_connect_timeout_secs,
            )
            .field(
                "upstream_read_timeout_secs",
                &self.upstream_read_timeout_secs,
            )
            .field(
                "upstream_request_timeout_secs",
                &self.upstream_request_timeout_secs,
            )
            .field("oauth_redirect_uri", &self.oauth_redirect_uri)
            .field(
                "codex_latency_telemetry_enabled",
                &self.codex_latency_telemetry_enabled,
            )
            .field(
                "codex_native_non_stream_enabled",
                &self.codex_native_non_stream_enabled,
            )
            .field(
                "codex_context_optimizer_enabled",
                &self.codex_context_optimizer_enabled,
            )
            .field(
                "codex_context_max_messages",
                &self.codex_context_max_messages,
            )
            .field("codex_context_max_bytes", &self.codex_context_max_bytes)
            .finish()
    }
}

impl Config {
    pub fn parse(
        host_val: Option<String>,
        port_val: Option<String>,
        data_dir_val: Option<String>,
        api_key_val: Option<String>,
    ) -> Result<Self, String> {
        Self::parse_with_upstream(
            host_val,
            port_val,
            data_dir_val,
            api_key_val,
            None,
            None,
            false,
        )
    }

    pub fn parse_with_upstream(
        host_val: Option<String>,
        port_val: Option<String>,
        data_dir_val: Option<String>,
        api_key_val: Option<String>,
        upstream_base_url: Option<String>,
        upstream_api_key: Option<String>,
        use_mock_provider: bool,
    ) -> Result<Self, String> {
        let host = host_val.unwrap_or_else(|| "127.0.0.1".to_string());

        let port: u16 = match port_val {
            Some(p_str) => p_str
                .parse::<u16>()
                .map_err(|e| format!("Invalid AG_PORT value '{p_str}': {e}"))?,
            None => DEFAULT_PORT,
        };

        if port == PROD_FORBIDDEN_PORT {
            return Err(format!(
                "Port {PROD_FORBIDDEN_PORT} is reserved for production Python ag-proxy. \
                 ag-proxy-rust staging must use port {DEFAULT_PORT} or another non-production port."
            ));
        }

        let data_dir_str = data_dir_val.unwrap_or_else(|| DEFAULT_DATA_DIR.to_string());
        let data_dir = PathBuf::from(&data_dir_str);

        if Self::is_prod_data_dir(&data_dir) {
            return Err(format!(
                "Data directory '{data_dir_str}' conflicts with production ag-proxy. \
                 ag-proxy-rust must use isolated staging data dir (default: {DEFAULT_DATA_DIR})."
            ));
        }

        let api_key = api_key_val
            .filter(|k| !k.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_API_KEY.to_string());

        let upstream_base_url = upstream_base_url.filter(|u| !u.trim().is_empty());
        let upstream_api_key = upstream_api_key.filter(|k| !k.trim().is_empty());

        let oauth_redirect_uri = env::var("AG_OAUTH_REDIRECT_URI")
            .unwrap_or_else(|_| format!("http://localhost:{port}/auth/callback"));

        Ok(Self {
            host,
            port,
            data_dir,
            api_key,
            upstream_base_url,
            upstream_api_key,
            use_mock_provider,
            upstream_connect_timeout_secs: DEFAULT_UPSTREAM_CONNECT_TIMEOUT_SECS,
            upstream_read_timeout_secs: DEFAULT_UPSTREAM_READ_TIMEOUT_SECS,
            upstream_request_timeout_secs: DEFAULT_UPSTREAM_REQUEST_TIMEOUT_SECS,
            oauth_redirect_uri,
            codex_latency_telemetry_enabled: true,
            codex_native_non_stream_enabled: false,
            codex_context_optimizer_enabled: false,
            codex_context_max_messages: 0,
            codex_context_max_bytes: 0,
        })
    }

    pub fn from_env() -> Result<Self, String> {
        let host = env::var("AG_HOST").ok();
        let port = env::var("AG_PORT").ok();
        let data_dir = env::var("AG_DATA_DIR").ok();
        let api_key = env::var("AG_API_KEY").ok();
        let upstream_base_url = env::var("AG_UPSTREAM_BASE_URL")
            .or_else(|_| env::var("UPSTREAM_BASE_URL"))
            .ok();
        let upstream_api_key = env::var("AG_UPSTREAM_API_KEY")
            .or_else(|_| env::var("UPSTREAM_API_KEY"))
            .ok();
        let use_mock_provider = env::var("AG_USE_MOCK_PROVIDER")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes"))
            .unwrap_or_else(|_| {
                env::var("AG_PROVIDER")
                    .map(|p| p.eq_ignore_ascii_case("mock"))
                    .unwrap_or(false)
            });

        let mut cfg = Self::parse_with_upstream(
            host,
            port,
            data_dir,
            api_key,
            upstream_base_url,
            upstream_api_key,
            use_mock_provider,
        )?;

        if let Ok(val) = env::var("AG_UPSTREAM_CONNECT_TIMEOUT_SECS") {
            if let Ok(secs) = val.parse::<u64>() {
                cfg.upstream_connect_timeout_secs = secs;
            }
        }
        if let Ok(val) = env::var("AG_UPSTREAM_READ_TIMEOUT_SECS") {
            if let Ok(secs) = val.parse::<u64>() {
                cfg.upstream_read_timeout_secs = secs;
            }
        }
        if let Ok(val) = env::var("AG_UPSTREAM_REQUEST_TIMEOUT_SECS") {
            if let Ok(secs) = val.parse::<u64>() {
                cfg.upstream_request_timeout_secs = secs;
            }
        }

        if let Ok(val) = env::var("CODEX_LATENCY_TELEMETRY_ENABLED") {
            cfg.codex_latency_telemetry_enabled =
                val != "0" && !val.eq_ignore_ascii_case("false") && !val.eq_ignore_ascii_case("no");
        }

        if let Ok(val) = env::var("CODEX_NATIVE_NON_STREAM_ENABLED") {
            cfg.codex_native_non_stream_enabled =
                val == "1" || val.eq_ignore_ascii_case("true") || val.eq_ignore_ascii_case("yes");
        }

        if let Ok(val) = env::var("CODEX_CONTEXT_OPTIMIZER_ENABLED") {
            cfg.codex_context_optimizer_enabled =
                val == "1" || val.eq_ignore_ascii_case("true") || val.eq_ignore_ascii_case("yes");
        }

        if let Ok(val) = env::var("CODEX_CONTEXT_MAX_MESSAGES") {
            if let Ok(num) = val.parse::<usize>() {
                cfg.codex_context_max_messages = num;
            }
        }

        if let Ok(val) = env::var("CODEX_CONTEXT_MAX_BYTES") {
            if let Ok(num) = val.parse::<usize>() {
                cfg.codex_context_max_bytes = num;
            }
        }

        Ok(cfg)
    }

    pub fn is_prod_data_dir(path: &Path) -> bool {
        let normalized = path.to_string_lossy();
        normalized.ends_with(PROD_DATA_DIR_SUFFIX)
            || normalized.ends_with(&format!("{PROD_DATA_DIR_SUFFIX}/"))
            || normalized.contains(&format!("{PROD_DATA_DIR_SUFFIX}/"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = Config::parse(None, None, None, None).expect("default config should succeed");
        assert_eq!(cfg.port, 20229);
        assert_eq!(cfg.data_dir, PathBuf::from("./data"));
        assert_eq!(cfg.host, "127.0.0.1");
        assert_eq!(cfg.api_key, "ag-proxy-key");
        assert_eq!(cfg.upstream_base_url, None);
        assert_eq!(cfg.upstream_api_key, None);
        assert!(!cfg.use_mock_provider);
        assert_eq!(cfg.upstream_connect_timeout_secs, 10);
        assert_eq!(cfg.upstream_read_timeout_secs, 60);
        assert_eq!(cfg.upstream_request_timeout_secs, 120);
        assert!(
            cfg.oauth_redirect_uri == DEFAULT_OAUTH_REDIRECT_URI
                || cfg.oauth_redirect_uri == "http://localhost:20229/auth/callback"
                || cfg.oauth_redirect_uri == "http://127.0.0.1:20229/auth/callback"
        );
        assert!(cfg.codex_latency_telemetry_enabled);
        assert!(!cfg.codex_native_non_stream_enabled);
        assert!(!cfg.codex_context_optimizer_enabled);
        assert_eq!(cfg.codex_context_max_messages, 0);
        assert_eq!(cfg.codex_context_max_bytes, 0);
    }

    #[test]
    fn test_oauth_redirect_uri_override() {
        env::set_var(
            "AG_OAUTH_REDIRECT_URI",
            "http://127.0.0.1:20229/auth/callback",
        );
        let cfg = Config::parse(None, None, None, None).unwrap();
        assert_eq!(
            cfg.oauth_redirect_uri,
            "http://127.0.0.1:20229/auth/callback"
        );
        env::remove_var("AG_OAUTH_REDIRECT_URI");
    }

    #[test]
    fn test_rejects_production_port_20129() {
        let res = Config::parse(None, Some("20129".to_string()), None, None);
        assert!(res.is_err());
        let err = res.err().unwrap();
        assert!(err.contains("20129 is reserved for production"));
    }

    #[test]
    fn test_rejects_production_data_dir() {
        let res = Config::parse(None, None, Some("/var/test/.ag-proxy".to_string()), None);
        assert!(res.is_err());
        let err = res.err().unwrap();
        assert!(err.contains("conflicts with production ag-proxy"));

        let res_sub = Config::parse(
            None,
            None,
            Some("/var/test/.ag-proxy/sub".to_string()),
            None,
        );
        assert!(res_sub.is_err());
    }

    #[test]
    fn test_custom_staging_port_and_dir() {
        let cfg = Config::parse(
            Some("0.0.0.0".to_string()),
            Some("20230".to_string()),
            Some("/tmp/ag-proxy-test-dir".to_string()),
            Some("custom-secret".to_string()),
        )
        .expect("custom valid config should succeed");
        assert_eq!(cfg.port, 20230);
        assert_eq!(cfg.data_dir, PathBuf::from("/tmp/ag-proxy-test-dir"));
        assert_eq!(cfg.host, "0.0.0.0");
        assert_eq!(cfg.api_key, "custom-secret");
    }

    #[test]
    fn test_debug_redacts_api_key() {
        let cfg = Config::parse(None, None, None, Some("super-secret-token".to_string())).unwrap();
        let debug_str = format!("{cfg:?}");
        assert!(!debug_str.contains("super-secret-token"));
        assert!(debug_str.contains("[REDACTED]"));
    }

    #[test]
    fn test_debug_redacts_upstream_api_key() {
        let cfg = Config::parse_with_upstream(
            None,
            None,
            None,
            Some("proxy-secret".to_string()),
            Some("https://api.upstream.internal".to_string()),
            Some("upstream-super-secret-key-1234".to_string()),
            false,
        )
        .unwrap();
        let debug_str = format!("{cfg:?}");
        assert!(!debug_str.contains("proxy-secret"));
        assert!(!debug_str.contains("upstream-super-secret-key-1234"));
        assert!(debug_str.contains("https://api.upstream.internal"));
        assert!(debug_str.contains("[REDACTED]"));
    }

    #[test]
    fn test_upstream_config_parsing() {
        let cfg = Config::parse_with_upstream(
            None,
            None,
            None,
            None,
            Some("https://api.openai.com/v1".to_string()),
            Some("sk-test-upstream".to_string()),
            true,
        )
        .unwrap();
        assert_eq!(
            cfg.upstream_base_url,
            Some("https://api.openai.com/v1".to_string())
        );
        assert_eq!(cfg.upstream_api_key, Some("sk-test-upstream".to_string()));
        assert!(cfg.use_mock_provider);
    }
}
