use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApiKey {
    pub id: String,
    pub name: String,
    pub key: String,
    pub is_active: bool,
    pub total_requests: i64,
    pub created_at: String,
    #[serde(default = "default_role")]
    pub role: String,
}

fn default_role() -> String {
    "client".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RequestLogItem {
    pub id: i64,
    pub timestamp: f64,
    pub model: String,
    pub account_id: Option<String>,
    pub status: String,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    pub duration_ms: f64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RequestsResponse {
    pub items: Vec<RequestLogItem>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AdminStats {
    pub total_requests: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    pub avg_duration_ms: f64,
    pub error_count: i64,
    #[serde(default)]
    pub total_accounts: usize,
    #[serde(default)]
    pub active_accounts: usize,
    #[serde(default)]
    pub cooldown_accounts: usize,
    #[serde(default)]
    pub active_rate: f64,
    #[serde(default)]
    pub error_rate: f64,
    #[serde(default)]
    pub rpm: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelRequestSummary {
    pub model: String,
    pub requests: i64,
    pub ok: i64,
    pub errors: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    pub avg_duration_ms: f64,
}

pub fn mask_api_key(key: &str) -> String {
    let key = key.trim();
    if key.is_empty() {
        return String::new();
    }
    let char_count = key.chars().count();
    if char_count <= 8 {
        return "********".to_string();
    }
    let tail: String = key
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    if key.starts_with("sk-") && char_count > 10 {
        format!("sk-****{tail}")
    } else {
        let prefix: String = key.chars().take(3).collect();
        format!("{prefix}****{tail}")
    }
}

pub fn is_masked_key(key: &str) -> bool {
    key.contains("****") || key == "********"
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderRecord {
    pub id: String,
    pub name: String,
    pub prefix: String,
    #[serde(rename = "type")]
    pub provider_type: String,
    pub base_url: String,
    pub api_key: String,
    pub models: serde_json::Value,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl std::fmt::Debug for ProviderRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderRecord")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("prefix", &self.prefix)
            .field("provider_type", &self.provider_type)
            .field("base_url", &self.base_url)
            .field("api_key", &"[REDACTED]")
            .field("models", &self.models)
            .field("is_active", &self.is_active)
            .field("created_at", &self.created_at)
            .field("updated_at", &self.updated_at)
            .finish()
    }
}

impl ProviderRecord {
    pub fn to_response(&self) -> ProviderResponse {
        ProviderResponse {
            id: self.id.clone(),
            name: self.name.clone(),
            prefix: self.prefix.clone(),
            provider_type: self.provider_type.clone(),
            base_url: self.base_url.clone(),
            api_key: mask_api_key(&self.api_key),
            models: self.models.clone(),
            is_active: self.is_active,
            created_at: self.created_at.clone(),
            updated_at: self.updated_at.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderResponse {
    pub id: String,
    pub name: String,
    pub prefix: String,
    #[serde(rename = "type")]
    pub provider_type: String,
    pub base_url: String,
    pub api_key: String,
    pub models: serde_json::Value,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ComboRecord {
    pub id: String,
    pub name: String,
    pub models: serde_json::Value,
    pub strategy: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct AccountRecord {
    pub id: String,
    pub email: String,
    #[serde(skip)]
    pub refresh_token: String,
    #[serde(skip)]
    pub access_token: Option<String>,
    pub expires_at: f64,
    pub is_active: bool,
    pub cooldown_until: f64,
    pub last_used_at: f64,
    pub total_requests: i64,
    pub error_count: i64,
    pub last_error: Option<String>,
    pub created_at: String,
}

impl std::fmt::Debug for AccountRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccountRecord")
            .field("id", &self.id)
            .field("email", &self.email)
            .field("refresh_token", &"[REDACTED]")
            .field("access_token", &"[REDACTED]")
            .field("expires_at", &self.expires_at)
            .field("is_active", &self.is_active)
            .field("cooldown_until", &self.cooldown_until)
            .field("last_used_at", &self.last_used_at)
            .field("total_requests", &self.total_requests)
            .field("error_count", &self.error_count)
            .field("last_error", &self.last_error)
            .field("created_at", &self.created_at)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CodexAccountRecord {
    pub id: String,
    pub email: Option<String>,
    pub auth_path: String,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
    pub last_error: Option<String>,
}

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
    token_saver_cache: Arc<std::sync::RwLock<crate::token_saver::TokenSaverSettings>>,
}

impl std::fmt::Debug for Database {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Database").finish()
    }
}

impl Database {
    pub fn open_or_create(
        data_dir: &Path,
        default_key: Option<&str>,
    ) -> Result<Self, rusqlite::Error> {
        if data_dir.to_str() == Some(":memory:") {
            return Self::open_in_memory(default_key);
        }

        if let Err(e) = std::fs::create_dir_all(data_dir) {
            tracing::warn!(
                "Failed to create data directory {}: {e}",
                data_dir.display()
            );
        }

        let db_path = data_dir.join("data.sqlite");
        Self::open_file(db_path, default_key)
    }

    pub fn open_file<P: AsRef<Path>>(
        path: P,
        default_key: Option<&str>,
    ) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;
        Self::init_connection(conn, default_key)
    }

    pub fn open_in_memory(default_key: Option<&str>) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open_in_memory()?;
        Self::init_connection(conn, default_key)
    }

    fn init_connection(
        conn: Connection,
        default_key: Option<&str>,
    ) -> Result<Self, rusqlite::Error> {
        conn.busy_timeout(Duration::from_secs(10))?;

        // Retry loop for WAL mode and schema initialization under concurrent multi-thread startup
        let mut initialized = false;
        for attempt in 0..15 {
            let res = conn.execute_batch(
                "PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = NORMAL;
                 PRAGMA foreign_keys = ON;
                 CREATE TABLE IF NOT EXISTS api_keys (
                     id TEXT PRIMARY KEY,
                     name TEXT NOT NULL,
                     key TEXT UNIQUE NOT NULL,
                     is_active INTEGER DEFAULT 1,
                     total_requests INTEGER DEFAULT 0,
                     created_at TEXT,
                     role TEXT NOT NULL DEFAULT 'client'
                 );
                 CREATE INDEX IF NOT EXISTS idx_api_keys_key ON api_keys(key);
                 CREATE TABLE IF NOT EXISTS request_log (
                     id INTEGER PRIMARY KEY AUTOINCREMENT,
                     account_id TEXT,
                     model TEXT,
                     timestamp REAL,
                     status TEXT,
                     prompt_tokens INTEGER DEFAULT 0,
                     completion_tokens INTEGER DEFAULT 0,
                     duration_ms REAL DEFAULT 0,
                     error TEXT
                 );
                 CREATE INDEX IF NOT EXISTS idx_request_log_timestamp ON request_log(timestamp);
                 CREATE INDEX IF NOT EXISTS idx_request_log_model ON request_log(model);
                 CREATE INDEX IF NOT EXISTS idx_request_log_status ON request_log(status);
                 CREATE TABLE IF NOT EXISTS providers (
                     id TEXT PRIMARY KEY,
                     name TEXT NOT NULL,
                     prefix TEXT UNIQUE NOT NULL,
                     type TEXT NOT NULL,
                     base_url TEXT NOT NULL,
                     api_key TEXT NOT NULL,
                     models JSON NOT NULL,
                     is_active INTEGER DEFAULT 1,
                     created_at TEXT NOT NULL,
                     updated_at TEXT NOT NULL
                 );
                 CREATE UNIQUE INDEX IF NOT EXISTS idx_providers_prefix ON providers(prefix);
                 CREATE TABLE IF NOT EXISTS combos (
                     id TEXT PRIMARY KEY,
                     name TEXT UNIQUE NOT NULL,
                     models JSON NOT NULL,
                     strategy TEXT NOT NULL,
                     created_at TEXT NOT NULL,
                     updated_at TEXT NOT NULL
                 );
                 CREATE UNIQUE INDEX IF NOT EXISTS idx_combos_name ON combos(name);
                 CREATE TABLE IF NOT EXISTS accounts (
                     id TEXT PRIMARY KEY,
                     email TEXT UNIQUE,
                     refresh_token TEXT NOT NULL,
                     access_token TEXT,
                     expires_at REAL DEFAULT 0,
                     is_active INTEGER DEFAULT 1,
                     cooldown_until REAL DEFAULT 0,
                     last_used_at REAL DEFAULT 0,
                     total_requests INTEGER DEFAULT 0,
                     error_count INTEGER DEFAULT 0,
                     last_error TEXT,
                     created_at TEXT DEFAULT CURRENT_TIMESTAMP
                 );
                 CREATE UNIQUE INDEX IF NOT EXISTS idx_accounts_email ON accounts(email);
                 CREATE INDEX IF NOT EXISTS idx_accounts_is_active ON accounts(is_active);
                 CREATE INDEX IF NOT EXISTS idx_accounts_cooldown_until ON accounts(cooldown_until);
                 CREATE TABLE IF NOT EXISTS codex_accounts (
                     id TEXT PRIMARY KEY,
                     email TEXT,
                     auth_path TEXT NOT NULL UNIQUE,
                     is_active INTEGER DEFAULT 0,
                     created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                     updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
                     last_error TEXT
                 );
                 CREATE INDEX IF NOT EXISTS idx_codex_accounts_is_active ON codex_accounts(is_active);
                 CREATE TABLE IF NOT EXISTS quota_refresh_settings (
                     key TEXT PRIMARY KEY,
                     value TEXT NOT NULL,
                     updated_at REAL NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS settings (
                     key TEXT PRIMARY KEY,
                     value TEXT
                 );
                 INSERT OR IGNORE INTO settings (key, value) VALUES ('token_saver_enabled', 'true');
                 INSERT OR IGNORE INTO settings (key, value) VALUES ('rtk_enabled', 'true');
                 INSERT OR IGNORE INTO settings (key, value) VALUES ('caveman_level', 'lite');
                 INSERT OR IGNORE INTO settings (key, value) VALUES ('ponytail_level', 'full');",
            );

            match res {
                Ok(_) => {
                    initialized = true;
                    break;
                }
                Err(rusqlite::Error::SqliteFailure(e, _))
                    if e.code == rusqlite::ErrorCode::DatabaseBusy
                        || e.code == rusqlite::ErrorCode::DatabaseLocked =>
                {
                    std::thread::sleep(Duration::from_millis(20 * (attempt + 1)));
                }
                Err(e) => return Err(e),
            }
        }

        if !initialized {
            // Fallback: table create without changing journal_mode if WAL is already set
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS api_keys (
                     id TEXT PRIMARY KEY,
                     name TEXT NOT NULL,
                     key TEXT UNIQUE NOT NULL,
                     is_active INTEGER DEFAULT 1,
                     total_requests INTEGER DEFAULT 0,
                     created_at TEXT,
                     role TEXT NOT NULL DEFAULT 'client'
                 );
                 CREATE INDEX IF NOT EXISTS idx_api_keys_key ON api_keys(key);
                 CREATE TABLE IF NOT EXISTS request_log (
                     id INTEGER PRIMARY KEY AUTOINCREMENT,
                     account_id TEXT,
                     model TEXT,
                     timestamp REAL,
                     status TEXT,
                     prompt_tokens INTEGER DEFAULT 0,
                     completion_tokens INTEGER DEFAULT 0,
                     duration_ms REAL DEFAULT 0,
                     error TEXT
                 );
                 CREATE INDEX IF NOT EXISTS idx_request_log_timestamp ON request_log(timestamp);
                 CREATE INDEX IF NOT EXISTS idx_request_log_model ON request_log(model);
                 CREATE INDEX IF NOT EXISTS idx_request_log_status ON request_log(status);
                 CREATE TABLE IF NOT EXISTS providers (
                     id TEXT PRIMARY KEY,
                     name TEXT NOT NULL,
                     prefix TEXT UNIQUE NOT NULL,
                     type TEXT NOT NULL,
                     base_url TEXT NOT NULL,
                     api_key TEXT NOT NULL,
                     models JSON NOT NULL,
                     is_active INTEGER DEFAULT 1,
                     created_at TEXT NOT NULL,
                     updated_at TEXT NOT NULL
                 );
                 CREATE UNIQUE INDEX IF NOT EXISTS idx_providers_prefix ON providers(prefix);
                 CREATE TABLE IF NOT EXISTS combos (
                     id TEXT PRIMARY KEY,
                     name TEXT UNIQUE NOT NULL,
                     models JSON NOT NULL,
                     strategy TEXT NOT NULL,
                     created_at TEXT NOT NULL,
                     updated_at TEXT NOT NULL
                 );
                 CREATE UNIQUE INDEX IF NOT EXISTS idx_combos_name ON combos(name);
                 CREATE TABLE IF NOT EXISTS accounts (
                     id TEXT PRIMARY KEY,
                     email TEXT UNIQUE,
                     refresh_token TEXT NOT NULL,
                     access_token TEXT,
                     expires_at REAL DEFAULT 0,
                     is_active INTEGER DEFAULT 1,
                     cooldown_until REAL DEFAULT 0,
                     last_used_at REAL DEFAULT 0,
                     total_requests INTEGER DEFAULT 0,
                     error_count INTEGER DEFAULT 0,
                     last_error TEXT,
                     created_at TEXT DEFAULT CURRENT_TIMESTAMP
                 );
                 CREATE UNIQUE INDEX IF NOT EXISTS idx_accounts_email ON accounts(email);
                 CREATE INDEX IF NOT EXISTS idx_accounts_is_active ON accounts(is_active);
                 CREATE INDEX IF NOT EXISTS idx_accounts_cooldown_until ON accounts(cooldown_until);
                 CREATE TABLE IF NOT EXISTS codex_accounts (
                     id TEXT PRIMARY KEY,
                     email TEXT,
                     auth_path TEXT NOT NULL UNIQUE,
                     is_active INTEGER DEFAULT 0,
                     created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                     updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
                     last_error TEXT
                 );
                 CREATE INDEX IF NOT EXISTS idx_codex_accounts_is_active ON codex_accounts(is_active);
                 CREATE TABLE IF NOT EXISTS quota_refresh_settings (
                     key TEXT PRIMARY KEY,
                     value TEXT NOT NULL,
                     updated_at REAL NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS settings (
                     key TEXT PRIMARY KEY,
                     value TEXT
                 );
                 INSERT OR IGNORE INTO settings (key, value) VALUES ('token_saver_enabled', 'true');
                 INSERT OR IGNORE INTO settings (key, value) VALUES ('rtk_enabled', 'true');
                 INSERT OR IGNORE INTO settings (key, value) VALUES ('caveman_level', 'lite');
                 INSERT OR IGNORE INTO settings (key, value) VALUES ('ponytail_level', 'full');",
            )?;
        }

        let role_added = conn
            .execute(
                "ALTER TABLE api_keys ADD COLUMN role TEXT NOT NULL DEFAULT 'client'",
                [],
            )
            .is_ok();
        if role_added {
            // Existing installations treated every active API key as trusted. Preserve that
            // behavior during the one-time schema migration; newly created keys default client.
            let _ = conn.execute("UPDATE api_keys SET role = 'admin'", []);
        }

        if let Some(raw_key) = default_key {
            let trimmed = raw_key.trim();
            if !trimmed.is_empty() {
                for attempt in 0..15 {
                    let count_res: Result<i64, _> =
                        conn.query_row("SELECT COUNT(*) FROM api_keys", [], |row| row.get(0));

                    match count_res {
                        Ok(count) => {
                            if count == 0 {
                                let id = format!("key-{}", Uuid::new_v4().simple());
                                let now = Utc::now().to_rfc3339();
                                let _ = conn.execute(
                                    "INSERT OR IGNORE INTO api_keys (id, name, key, is_active, total_requests, created_at, role)
                                     VALUES (?1, ?2, ?3, 1, 0, ?4, 'admin')",
                                    params![id, "Default Key", trimmed, now],
                                );
                            } else {
                                let _ = conn.execute(
                                    "UPDATE api_keys SET role = 'admin' WHERE key = ?1",
                                    params![trimmed],
                                );
                            }
                            break;
                        }
                        Err(rusqlite::Error::SqliteFailure(e, _))
                            if e.code == rusqlite::ErrorCode::DatabaseBusy
                                || e.code == rusqlite::ErrorCode::DatabaseLocked =>
                        {
                            std::thread::sleep(Duration::from_millis(20 * (attempt + 1)));
                        }
                        Err(_) => break,
                    }
                }
            }
        }

        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
            token_saver_cache: Arc::new(std::sync::RwLock::new(
                crate::token_saver::TokenSaverSettings::default(),
            )),
        };
        if let Ok(settings) = db.get_token_saver_settings_from_db() {
            if let Ok(mut guard) = db.token_saver_cache.write() {
                *guard = settings;
            }
        }
        Ok(db)
    }

    fn lock_conn(&self) -> Result<MutexGuard<'_, Connection>, rusqlite::Error> {
        self.conn.lock().map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(e.to_string())))
        })
    }

    pub fn get_key(&self, key: &str) -> Result<Option<ApiKey>, rusqlite::Error> {
        let conn = self.lock_conn()?;
        conn.query_row(
            "SELECT id, name, key, is_active, total_requests, created_at, role FROM api_keys WHERE key = ?1",
            params![key],
            |row| {
                let is_active_int: i64 = row.get(3)?;
                Ok(ApiKey {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    key: row.get(2)?,
                    is_active: is_active_int != 0,
                    total_requests: row.get(4)?,
                    created_at: row.get(5).unwrap_or_default(),
                    role: row.get(6).unwrap_or_else(|_| "client".to_string()),
                })
            },
        )
        .optional()
    }

    pub fn get_key_by_id(&self, id: &str) -> Result<Option<ApiKey>, rusqlite::Error> {
        let conn = self.lock_conn()?;
        conn.query_row(
            "SELECT id, name, key, is_active, total_requests, created_at, role FROM api_keys WHERE id = ?1",
            params![id],
            |row| {
                let is_active_int: i64 = row.get(3)?;
                Ok(ApiKey {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    key: row.get(2)?,
                    is_active: is_active_int != 0,
                    total_requests: row.get(4)?,
                    created_at: row.get(5).unwrap_or_default(),
                    role: row.get(6).unwrap_or_else(|_| "client".to_string()),
                })
            },
        )
        .optional()
    }

    pub fn list_keys(&self) -> Result<Vec<ApiKey>, rusqlite::Error> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, key, is_active, total_requests, created_at, role FROM api_keys ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            let is_active_int: i64 = row.get(3)?;
            Ok(ApiKey {
                id: row.get(0)?,
                name: row.get(1)?,
                key: row.get(2)?,
                is_active: is_active_int != 0,
                total_requests: row.get(4)?,
                created_at: row.get(5).unwrap_or_default(),
                role: row.get(6).unwrap_or_else(|_| "client".to_string()),
            })
        })?;

        let mut keys = Vec::new();
        for key in rows {
            keys.push(key?);
        }
        Ok(keys)
    }

    pub fn create_key(&self, name: &str, key_val: Option<&str>) -> Result<ApiKey, rusqlite::Error> {
        self.create_key_with_role(name, key_val, "client")
    }

    pub fn create_key_with_role(
        &self,
        name: &str,
        key_val: Option<&str>,
        role: &str,
    ) -> Result<ApiKey, rusqlite::Error> {
        let conn = self.lock_conn()?;
        let id = format!("key-{}", Uuid::new_v4().simple());
        let key = key_val
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| format!("sk-ag-{}", Uuid::new_v4().simple()));
        let now = Utc::now().to_rfc3339();
        let role = if role.trim().eq_ignore_ascii_case("admin") {
            "admin"
        } else {
            "client"
        };

        conn.execute(
            "INSERT INTO api_keys (id, name, key, is_active, total_requests, created_at, role)
             VALUES (?1, ?2, ?3, 1, 0, ?4, ?5)",
            params![id, name, key, now, role],
        )?;

        Ok(ApiKey {
            id,
            name: name.to_string(),
            key,
            is_active: true,
            total_requests: 0,
            created_at: now,
            role: role.to_string(),
        })
    }

    pub fn set_key_active(&self, id: &str, is_active: bool) -> Result<bool, rusqlite::Error> {
        let conn = self.lock_conn()?;
        let active_int = if is_active { 1 } else { 0 };
        let count = conn.execute(
            "UPDATE api_keys SET is_active = ?1 WHERE id = ?2",
            params![active_int, id],
        )?;
        Ok(count > 0)
    }

    pub fn delete_key(&self, id: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.lock_conn()?;
        let count = conn.execute("DELETE FROM api_keys WHERE id = ?1", params![id])?;
        Ok(count > 0)
    }

    pub fn increment_total_requests(&self, key: &str) -> Result<i64, rusqlite::Error> {
        let conn = self.lock_conn()?;
        let rows = conn.execute(
            "UPDATE api_keys SET total_requests = total_requests + 1 WHERE key = ?1",
            params![key],
        )?;
        if rows == 0 {
            // Key authenticated via fallback AG_API_KEY. Record into api_keys with total_requests = 1.
            let id = format!("key-{}", Uuid::new_v4().simple());
            let now = Utc::now().to_rfc3339();
            let _ = conn.execute(
                "INSERT OR IGNORE INTO api_keys (id, name, key, is_active, total_requests, created_at, role)
                 VALUES (?1, ?2, ?3, 1, 1, ?4, 'admin')",
                params![id, "Fallback Key", key, now],
            );
            return Ok(1);
        }
        let count: i64 = conn.query_row(
            "SELECT total_requests FROM api_keys WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_request(
        &self,
        account_id: Option<&str>,
        model: &str,
        timestamp: f64,
        status: &str,
        prompt_tokens: i64,
        completion_tokens: i64,
        duration_ms: f64,
        error: Option<&str>,
    ) -> Result<i64, rusqlite::Error> {
        let conn = self.lock_conn()?;
        let duration_ms = duration_ms.max(0.0);
        let truncated_err = error.map(|e| {
            if e.len() > 500 {
                e.char_indices()
                    .take_while(|(idx, _)| *idx < 500)
                    .map(|(_, ch)| ch)
                    .collect::<String>()
            } else {
                e.to_string()
            }
        });

        conn.execute(
            "INSERT INTO request_log (account_id, model, timestamp, status, prompt_tokens, completion_tokens, duration_ms, error)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                account_id,
                model,
                timestamp,
                status,
                prompt_tokens,
                completion_tokens,
                duration_ms,
                truncated_err,
            ],
        )?;

        Ok(conn.last_insert_rowid())
    }

    pub fn get_admin_stats(&self) -> Result<AdminStats, rusqlite::Error> {
        let conn = self.lock_conn()?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);

        let (total_requests, prompt_tokens, completion_tokens, avg_duration_ms, error_count) = conn
            .query_row(
                "SELECT
                    COUNT(*) AS total_requests,
                    COALESCE(SUM(prompt_tokens), 0) AS prompt_tokens,
                    COALESCE(SUM(completion_tokens), 0) AS completion_tokens,
                    COALESCE(AVG(duration_ms), 0.0) AS avg_duration_ms,
                    COALESCE(SUM(CASE WHEN status = 'error' THEN 1 ELSE 0 END), 0) AS error_count
                 FROM request_log",
                [],
                |row| {
                    let total_requests: i64 = row.get(0)?;
                    let prompt_tokens: i64 = row.get(1)?;
                    let completion_tokens: i64 = row.get(2)?;
                    let avg_duration_ms: f64 = row.get(3)?;
                    let error_count: i64 = row.get(4)?;
                    Ok((
                        total_requests,
                        prompt_tokens,
                        completion_tokens,
                        avg_duration_ms,
                        error_count,
                    ))
                },
            )?;

        let rounded_avg = (avg_duration_ms * 100.0).round() / 100.0;
        let total_tokens = prompt_tokens + completion_tokens;

        let rpm_window = now - 60.0;
        let rpm: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM request_log WHERE timestamp > ?1",
                params![rpm_window],
                |r| r.get(0),
            )
            .unwrap_or(0);

        let error_rate = if total_requests > 0 {
            ((error_count as f64 / total_requests as f64 * 100.0) * 10.0).round() / 10.0
        } else {
            0.0
        };

        Ok(AdminStats {
            total_requests,
            prompt_tokens,
            completion_tokens,
            total_tokens,
            avg_duration_ms: rounded_avg,
            error_count,
            total_accounts: 0,
            active_accounts: 0,
            cooldown_accounts: 0,
            active_rate: 0.0,
            error_rate,
            rpm,
        })
    }

    pub fn get_requests(
        &self,
        limit: i64,
        offset: i64,
        model: Option<&str>,
        status: Option<&str>,
    ) -> Result<RequestsResponse, rusqlite::Error> {
        let conn = self.lock_conn()?;
        let limit = limit.clamp(1, 200);
        let offset = offset.max(0);

        let mut filters = Vec::new();
        let mut count_params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(m) = model.filter(|s| !s.is_empty()) {
            filters.push("model = ?");
            count_params.push(Box::new(m.to_string()));
        }
        if let Some(s) = status.filter(|s| !s.is_empty()) {
            filters.push("status = ?");
            count_params.push(Box::new(s.to_string()));
        }

        let where_clause = if filters.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", filters.join(" AND "))
        };

        let count_sql = format!("SELECT COUNT(*) FROM request_log{where_clause}");
        let mut count_stmt = conn.prepare(&count_sql)?;
        let count_slice: Vec<&dyn rusqlite::ToSql> =
            count_params.iter().map(|p| p.as_ref()).collect();
        let total: i64 = count_stmt.query_row(count_slice.as_slice(), |row| row.get(0))?;

        let query_sql = format!(
            "SELECT id, timestamp, model, account_id, status,
                    prompt_tokens, completion_tokens,
                    prompt_tokens + completion_tokens AS total_tokens,
                    duration_ms, error
             FROM request_log{where_clause}
             ORDER BY id DESC LIMIT ? OFFSET ?"
        );
        let mut query_stmt = conn.prepare(&query_sql)?;

        let mut query_params: Vec<&dyn rusqlite::ToSql> = count_slice;
        query_params.push(&limit);
        query_params.push(&offset);

        let rows = query_stmt.query_map(query_params.as_slice(), |row| {
            Ok(RequestLogItem {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                model: row.get(2)?,
                account_id: row.get(3)?,
                status: row.get(4)?,
                prompt_tokens: row.get(5)?,
                completion_tokens: row.get(6)?,
                total_tokens: row.get(7)?,
                duration_ms: row.get(8)?,
                error: row.get(9)?,
            })
        })?;

        let mut items = Vec::new();
        for item in rows {
            items.push(item?);
        }

        Ok(RequestsResponse {
            items,
            total,
            limit,
            offset,
        })
    }

    pub fn get_request_summary(&self) -> Result<Vec<ModelRequestSummary>, rusqlite::Error> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT model,
                    COUNT(*) AS requests,
                    COALESCE(SUM(CASE WHEN status IN ('ok', 'success') THEN 1 ELSE 0 END), 0) AS ok,
                    COALESCE(SUM(CASE WHEN status = 'error' THEN 1 ELSE 0 END), 0) AS errors,
                    COALESCE(SUM(prompt_tokens), 0) AS prompt_tokens,
                    COALESCE(SUM(completion_tokens), 0) AS completion_tokens,
                    COALESCE(SUM(prompt_tokens + completion_tokens), 0) AS total_tokens,
                    COALESCE(AVG(duration_ms), 0.0) AS avg_duration_ms
             FROM request_log
             GROUP BY model
             ORDER BY requests DESC, model ASC",
        )?;

        let rows = stmt.query_map([], |row| {
            let avg_duration_ms: f64 = row.get(7)?;
            let rounded_avg = (avg_duration_ms * 100.0).round() / 100.0;
            Ok(ModelRequestSummary {
                model: row.get(0)?,
                requests: row.get(1)?,
                ok: row.get(2)?,
                errors: row.get(3)?,
                prompt_tokens: row.get(4)?,
                completion_tokens: row.get(5)?,
                total_tokens: row.get(6)?,
                avg_duration_ms: rounded_avg,
            })
        })?;

        let mut summaries = Vec::new();
        for s in rows {
            summaries.push(s?);
        }
        Ok(summaries)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_provider(
        &self,
        name: &str,
        prefix: &str,
        provider_type: &str,
        base_url: &str,
        api_key: &str,
        models: &serde_json::Value,
        is_active: bool,
    ) -> Result<ProviderRecord, rusqlite::Error> {
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction()?;
        let id = format!("provider-{}", Uuid::new_v4().simple());
        let now = Utc::now().to_rfc3339();
        let models_str = serde_json::to_string(models).unwrap_or_else(|_| "[]".to_string());
        let is_active_int = if is_active { 1 } else { 0 };

        tx.execute(
            "INSERT INTO providers (id, name, prefix, type, base_url, api_key, models, is_active, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                id,
                name,
                prefix,
                provider_type,
                base_url,
                api_key,
                models_str,
                is_active_int,
                now,
                now
            ],
        )?;

        tx.commit()?;

        Ok(ProviderRecord {
            id,
            name: name.to_string(),
            prefix: prefix.to_string(),
            provider_type: provider_type.to_string(),
            base_url: base_url.to_string(),
            api_key: api_key.to_string(),
            models: models.clone(),
            is_active,
            created_at: now.clone(),
            updated_at: now,
        })
    }

    pub fn get_provider_by_id(&self, id: &str) -> Result<Option<ProviderRecord>, rusqlite::Error> {
        let conn = self.lock_conn()?;
        conn.query_row(
            "SELECT id, name, prefix, type, base_url, api_key, models, is_active, created_at, updated_at
             FROM providers WHERE id = ?1",
            params![id],
            |row| {
                let models_str: String = row.get(6)?;
                let models_val: serde_json::Value =
                    serde_json::from_str(&models_str).unwrap_or(serde_json::json!([]));
                let is_active_int: i64 = row.get(7)?;
                Ok(ProviderRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    prefix: row.get(2)?,
                    provider_type: row.get(3)?,
                    base_url: row.get(4)?,
                    api_key: row.get(5)?,
                    models: models_val,
                    is_active: is_active_int != 0,
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                })
            },
        )
        .optional()
    }

    pub fn get_provider_by_prefix(
        &self,
        prefix: &str,
    ) -> Result<Option<ProviderRecord>, rusqlite::Error> {
        let conn = self.lock_conn()?;
        conn.query_row(
            "SELECT id, name, prefix, type, base_url, api_key, models, is_active, created_at, updated_at
             FROM providers WHERE prefix = ?1",
            params![prefix],
            |row| {
                let models_str: String = row.get(6)?;
                let models_val: serde_json::Value =
                    serde_json::from_str(&models_str).unwrap_or(serde_json::json!([]));
                let is_active_int: i64 = row.get(7)?;
                Ok(ProviderRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    prefix: row.get(2)?,
                    provider_type: row.get(3)?,
                    base_url: row.get(4)?,
                    api_key: row.get(5)?,
                    models: models_val,
                    is_active: is_active_int != 0,
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                })
            },
        )
        .optional()
    }

    pub fn list_providers(&self) -> Result<Vec<ProviderRecord>, rusqlite::Error> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, prefix, type, base_url, api_key, models, is_active, created_at, updated_at
             FROM providers ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            let models_str: String = row.get(6)?;
            let models_val: serde_json::Value =
                serde_json::from_str(&models_str).unwrap_or(serde_json::json!([]));
            let is_active_int: i64 = row.get(7)?;
            Ok(ProviderRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                prefix: row.get(2)?,
                provider_type: row.get(3)?,
                base_url: row.get(4)?,
                api_key: row.get(5)?,
                models: models_val,
                is_active: is_active_int != 0,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })?;

        let mut list = Vec::new();
        for item in rows {
            list.push(item?);
        }
        Ok(list)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_provider(
        &self,
        id: &str,
        name: Option<&str>,
        prefix: Option<&str>,
        provider_type: Option<&str>,
        base_url: Option<&str>,
        api_key: Option<&str>,
        models: Option<&serde_json::Value>,
        is_active: Option<bool>,
    ) -> Result<Option<ProviderRecord>, rusqlite::Error> {
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction()?;

        let current: Option<ProviderRecord> = tx
            .query_row(
                "SELECT id, name, prefix, type, base_url, api_key, models, is_active, created_at, updated_at
                 FROM providers WHERE id = ?1",
                params![id],
                |row| {
                    let models_str: String = row.get(6)?;
                    let models_val: serde_json::Value =
                        serde_json::from_str(&models_str).unwrap_or(serde_json::json!([]));
                    let is_active_int: i64 = row.get(7)?;
                    Ok(ProviderRecord {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        prefix: row.get(2)?,
                        provider_type: row.get(3)?,
                        base_url: row.get(4)?,
                        api_key: row.get(5)?,
                        models: models_val,
                        is_active: is_active_int != 0,
                        created_at: row.get(8)?,
                        updated_at: row.get(9)?,
                    })
                },
            )
            .optional()?;

        let mut record = match current {
            Some(r) => r,
            None => return Ok(None),
        };

        if let Some(n) = name {
            record.name = n.to_string();
        }
        if let Some(p) = prefix {
            record.prefix = p.to_string();
        }
        if let Some(t) = provider_type {
            record.provider_type = t.to_string();
        }
        if let Some(b) = base_url {
            record.base_url = b.to_string();
        }
        if let Some(k) = api_key {
            if !is_masked_key(k) && !k.trim().is_empty() {
                record.api_key = k.trim().to_string();
            }
        }
        if let Some(m) = models {
            record.models = m.clone();
        }
        if let Some(a) = is_active {
            record.is_active = a;
        }
        record.updated_at = Utc::now().to_rfc3339();

        let models_str = serde_json::to_string(&record.models).unwrap_or_else(|_| "[]".to_string());
        let is_active_int = if record.is_active { 1 } else { 0 };

        tx.execute(
            "UPDATE providers
             SET name = ?1, prefix = ?2, type = ?3, base_url = ?4, api_key = ?5,
                 models = ?6, is_active = ?7, updated_at = ?8
             WHERE id = ?9",
            params![
                record.name,
                record.prefix,
                record.provider_type,
                record.base_url,
                record.api_key,
                models_str,
                is_active_int,
                record.updated_at,
                id
            ],
        )?;

        tx.commit()?;

        Ok(Some(record))
    }

    pub fn delete_provider(&self, id: &str) -> Result<bool, rusqlite::Error> {
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction()?;
        let count = tx.execute("DELETE FROM providers WHERE id = ?1", params![id])?;
        tx.commit()?;
        Ok(count > 0)
    }

    pub fn update_provider_models(
        &self,
        id: &str,
        models: &serde_json::Value,
    ) -> Result<Option<ProviderRecord>, rusqlite::Error> {
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction()?;

        let now = Utc::now().to_rfc3339();
        let models_str = serde_json::to_string(models).unwrap_or_else(|_| "[]".to_string());
        let count = tx.execute(
            "UPDATE providers SET models = ?1, updated_at = ?2 WHERE id = ?3",
            params![models_str, now, id],
        )?;

        if count == 0 {
            return Ok(None);
        }

        let record = tx.query_row(
            "SELECT id, name, prefix, type, base_url, api_key, models, is_active, created_at, updated_at
             FROM providers WHERE id = ?1",
            params![id],
            |row| {
                let m_str: String = row.get(6)?;
                let m_val: serde_json::Value =
                    serde_json::from_str(&m_str).unwrap_or(serde_json::json!([]));
                let is_active_int: i64 = row.get(7)?;
                Ok(ProviderRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    prefix: row.get(2)?,
                    provider_type: row.get(3)?,
                    base_url: row.get(4)?,
                    api_key: row.get(5)?,
                    models: m_val,
                    is_active: is_active_int != 0,
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                })
            },
        )?;

        tx.commit()?;
        Ok(Some(record))
    }

    pub fn create_combo(
        &self,
        name: &str,
        models: &serde_json::Value,
        strategy: &str,
    ) -> Result<ComboRecord, rusqlite::Error> {
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction()?;
        let id = format!("combo-{}", Uuid::new_v4().simple());
        let now = Utc::now().to_rfc3339();
        let models_str = serde_json::to_string(models).unwrap_or_else(|_| "[]".to_string());

        tx.execute(
            "INSERT INTO combos (id, name, models, strategy, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, name, models_str, strategy, now, now],
        )?;

        tx.commit()?;

        Ok(ComboRecord {
            id,
            name: name.to_string(),
            models: models.clone(),
            strategy: strategy.to_string(),
            created_at: now.clone(),
            updated_at: now,
        })
    }

    pub fn get_combo_by_id(&self, id: &str) -> Result<Option<ComboRecord>, rusqlite::Error> {
        let conn = self.lock_conn()?;
        conn.query_row(
            "SELECT id, name, models, strategy, created_at, updated_at FROM combos WHERE id = ?1",
            params![id],
            |row| {
                let models_str: String = row.get(2)?;
                let models_val: serde_json::Value =
                    serde_json::from_str(&models_str).unwrap_or(serde_json::json!([]));
                Ok(ComboRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    models: models_val,
                    strategy: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            },
        )
        .optional()
    }

    pub fn get_combo_by_name(&self, name: &str) -> Result<Option<ComboRecord>, rusqlite::Error> {
        let conn = self.lock_conn()?;
        conn.query_row(
            "SELECT id, name, models, strategy, created_at, updated_at FROM combos WHERE name = ?1",
            params![name],
            |row| {
                let models_str: String = row.get(2)?;
                let models_val: serde_json::Value =
                    serde_json::from_str(&models_str).unwrap_or(serde_json::json!([]));
                Ok(ComboRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    models: models_val,
                    strategy: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            },
        )
        .optional()
    }

    pub fn list_combos(&self) -> Result<Vec<ComboRecord>, rusqlite::Error> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, models, strategy, created_at, updated_at FROM combos ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            let models_str: String = row.get(2)?;
            let models_val: serde_json::Value =
                serde_json::from_str(&models_str).unwrap_or(serde_json::json!([]));
            Ok(ComboRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                models: models_val,
                strategy: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })?;

        let mut list = Vec::new();
        for item in rows {
            list.push(item?);
        }
        Ok(list)
    }

    pub fn update_combo(
        &self,
        id: &str,
        name: Option<&str>,
        models: Option<&serde_json::Value>,
        strategy: Option<&str>,
    ) -> Result<Option<ComboRecord>, rusqlite::Error> {
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction()?;

        let current: Option<ComboRecord> = tx
            .query_row(
                "SELECT id, name, models, strategy, created_at, updated_at FROM combos WHERE id = ?1",
                params![id],
                |row| {
                    let models_str: String = row.get(2)?;
                    let models_val: serde_json::Value =
                        serde_json::from_str(&models_str).unwrap_or(serde_json::json!([]));
                    Ok(ComboRecord {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        models: models_val,
                        strategy: row.get(3)?,
                        created_at: row.get(4)?,
                        updated_at: row.get(5)?,
                    })
                },
            )
            .optional()?;

        let mut record = match current {
            Some(r) => r,
            None => return Ok(None),
        };

        if let Some(n) = name {
            record.name = n.to_string();
        }
        if let Some(m) = models {
            record.models = m.clone();
        }
        if let Some(s) = strategy {
            record.strategy = s.to_string();
        }
        record.updated_at = Utc::now().to_rfc3339();

        let models_str = serde_json::to_string(&record.models).unwrap_or_else(|_| "[]".to_string());

        tx.execute(
            "UPDATE combos SET name = ?1, models = ?2, strategy = ?3, updated_at = ?4 WHERE id = ?5",
            params![record.name, models_str, record.strategy, record.updated_at, id],
        )?;

        tx.commit()?;
        Ok(Some(record))
    }

    pub fn delete_combo(&self, id: &str) -> Result<bool, rusqlite::Error> {
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction()?;
        let count = tx.execute("DELETE FROM combos WHERE id = ?1", params![id])?;
        tx.commit()?;
        Ok(count > 0)
    }

    pub fn create_account(
        &self,
        id: Option<&str>,
        email: &str,
        refresh_token: &str,
    ) -> Result<AccountRecord, rusqlite::Error> {
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction()?;
        let aid = id
            .map(|s| s.to_string())
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let now_str = Utc::now().to_rfc3339();

        tx.execute(
            "INSERT INTO accounts (
                id, email, refresh_token, access_token, expires_at, is_active,
                cooldown_until, last_used_at, total_requests, error_count, last_error, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                aid,
                email,
                refresh_token,
                Option::<String>::None,
                0.0,
                1,
                0.0,
                0.0,
                0,
                0,
                Option::<String>::None,
                now_str,
            ],
        )?;
        tx.commit()?;

        Ok(AccountRecord {
            id: aid,
            email: email.to_string(),
            refresh_token: refresh_token.to_string(),
            access_token: None,
            expires_at: 0.0,
            is_active: true,
            cooldown_until: 0.0,
            last_used_at: 0.0,
            total_requests: 0,
            error_count: 0,
            last_error: None,
            created_at: now_str,
        })
    }

    pub fn save_account(&self, acc: &AccountRecord) -> Result<(), rusqlite::Error> {
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT OR REPLACE INTO accounts (
                id, email, refresh_token, access_token, expires_at, is_active,
                cooldown_until, last_used_at, total_requests, error_count, last_error, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                acc.id,
                acc.email,
                acc.refresh_token,
                acc.access_token,
                acc.expires_at,
                if acc.is_active { 1 } else { 0 },
                acc.cooldown_until,
                acc.last_used_at,
                acc.total_requests,
                acc.error_count,
                acc.last_error,
                acc.created_at,
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn get_account_by_id(&self, id: &str) -> Result<Option<AccountRecord>, rusqlite::Error> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, email, refresh_token, access_token, expires_at, is_active,
                    cooldown_until, last_used_at, total_requests, error_count, last_error, created_at
             FROM accounts
             WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            let is_active_int: i64 = row.get(5)?;
            Ok(Some(AccountRecord {
                id: row.get(0)?,
                email: row.get(1)?,
                refresh_token: row.get(2)?,
                access_token: row.get(3)?,
                expires_at: row.get(4)?,
                is_active: is_active_int != 0,
                cooldown_until: row.get(6)?,
                last_used_at: row.get(7)?,
                total_requests: row.get(8)?,
                error_count: row.get(9)?,
                last_error: row.get(10)?,
                created_at: row.get(11)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn get_account_by_email(
        &self,
        email: &str,
    ) -> Result<Option<AccountRecord>, rusqlite::Error> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, email, refresh_token, access_token, expires_at, is_active,
                    cooldown_until, last_used_at, total_requests, error_count, last_error, created_at
             FROM accounts
             WHERE email = ?1",
        )?;
        let mut rows = stmt.query(params![email])?;
        if let Some(row) = rows.next()? {
            let is_active_int: i64 = row.get(5)?;
            Ok(Some(AccountRecord {
                id: row.get(0)?,
                email: row.get(1)?,
                refresh_token: row.get(2)?,
                access_token: row.get(3)?,
                expires_at: row.get(4)?,
                is_active: is_active_int != 0,
                cooldown_until: row.get(6)?,
                last_used_at: row.get(7)?,
                total_requests: row.get(8)?,
                error_count: row.get(9)?,
                last_error: row.get(10)?,
                created_at: row.get(11)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn list_accounts(&self) -> Result<Vec<AccountRecord>, rusqlite::Error> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, email, refresh_token, access_token, expires_at, is_active,
                    cooldown_until, last_used_at, total_requests, error_count, last_error, created_at
             FROM accounts
             ORDER BY email ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            let is_active_int: i64 = row.get(5)?;
            Ok(AccountRecord {
                id: row.get(0)?,
                email: row.get(1)?,
                refresh_token: row.get(2)?,
                access_token: row.get(3)?,
                expires_at: row.get(4)?,
                is_active: is_active_int != 0,
                cooldown_until: row.get(6)?,
                last_used_at: row.get(7)?,
                total_requests: row.get(8)?,
                error_count: row.get(9)?,
                last_error: row.get(10)?,
                created_at: row.get(11)?,
            })
        })?;
        let mut result = Vec::new();
        for r in rows {
            result.push(r?);
        }
        Ok(result)
    }

    pub fn delete_account(&self, id: &str) -> Result<bool, rusqlite::Error> {
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction()?;
        let rows = tx.execute("DELETE FROM accounts WHERE id = ?1", params![id])?;
        tx.commit()?;
        Ok(rows > 0)
    }

    pub fn reset_account_cooldown(&self, id: &str) -> Result<bool, rusqlite::Error> {
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction()?;
        let rows = tx.execute(
            "UPDATE accounts SET cooldown_until = 0.0, error_count = 0, last_error = '' WHERE id = ?1",
            params![id],
        )?;
        tx.commit()?;
        Ok(rows > 0)
    }

    pub fn update_account_tokens(
        &self,
        id: &str,
        access_token: &str,
        expires_at: f64,
    ) -> Result<bool, rusqlite::Error> {
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction()?;
        let rows = tx.execute(
            "UPDATE accounts SET access_token = ?1, expires_at = ?2, error_count = 0 WHERE id = ?3",
            params![access_token, expires_at, id],
        )?;
        tx.commit()?;
        Ok(rows > 0)
    }

    pub fn cleanup_stale_errors(&self, now: f64) -> Result<usize, rusqlite::Error> {
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction()?;
        let rows = tx.execute(
            "UPDATE accounts SET last_error = '', error_count = 0
             WHERE is_active = 1
               AND (cooldown_until IS NULL OR cooldown_until <= ?1)",
            params![now],
        )?;
        tx.commit()?;
        Ok(rows)
    }

    pub fn record_account_success(&self, id: &str, now: f64) -> Result<(), rusqlite::Error> {
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE accounts
             SET last_used_at = ?1,
                 total_requests = total_requests + 1,
                 error_count = 0,
                 last_error = ''
             WHERE id = ?2",
            params![now, id],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn record_account_error(
        &self,
        id: &str,
        error: &str,
        cooldown_until: f64,
    ) -> Result<(), rusqlite::Error> {
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE accounts
             SET error_count = error_count + 1,
                 last_error = ?1,
                 cooldown_until = ?2
             WHERE id = ?3",
            params![error, cooldown_until, id],
        )?;
        tx.commit()?;
        Ok(())
    }

    // ─── Codex Accounts Repository ───

    pub fn create_codex_account(
        &self,
        id: Option<&str>,
        email: Option<&str>,
        auth_path: &str,
        is_active: bool,
    ) -> Result<CodexAccountRecord, rusqlite::Error> {
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction()?;
        let aid = id
            .map(|s| s.to_string())
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let now_str = Utc::now().to_rfc3339();

        tx.execute(
            "INSERT INTO codex_accounts (
                id, email, auth_path, is_active, created_at, updated_at, last_error
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                aid,
                email,
                auth_path,
                if is_active { 1 } else { 0 },
                now_str,
                now_str,
                Option::<String>::None,
            ],
        )?;
        tx.commit()?;

        Ok(CodexAccountRecord {
            id: aid,
            email: email.map(|s| s.to_string()),
            auth_path: auth_path.to_string(),
            is_active,
            created_at: now_str.clone(),
            updated_at: now_str,
            last_error: None,
        })
    }

    pub fn save_codex_account(&self, rec: &CodexAccountRecord) -> Result<(), rusqlite::Error> {
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction()?;
        let now_str = Utc::now().to_rfc3339();
        tx.execute(
            "INSERT INTO codex_accounts (
                id, email, auth_path, is_active, created_at, updated_at, last_error
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(id) DO UPDATE SET
                email = excluded.email,
                auth_path = excluded.auth_path,
                is_active = excluded.is_active,
                updated_at = excluded.updated_at,
                last_error = excluded.last_error",
            params![
                rec.id,
                rec.email,
                rec.auth_path,
                if rec.is_active { 1 } else { 0 },
                rec.created_at,
                now_str,
                rec.last_error,
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn get_codex_account_by_id(
        &self,
        id: &str,
    ) -> Result<Option<CodexAccountRecord>, rusqlite::Error> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, email, auth_path, is_active, created_at, updated_at, last_error
             FROM codex_accounts
             WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            let is_active_int: i64 = row.get(3)?;
            Ok(Some(CodexAccountRecord {
                id: row.get(0)?,
                email: row.get(1)?,
                auth_path: row.get(2)?,
                is_active: is_active_int != 0,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
                last_error: row.get(6)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn get_codex_account_by_email(
        &self,
        email: &str,
    ) -> Result<Option<CodexAccountRecord>, rusqlite::Error> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, email, auth_path, is_active, created_at, updated_at, last_error
             FROM codex_accounts
             WHERE email = ?1",
        )?;
        let mut rows = stmt.query(params![email])?;
        if let Some(row) = rows.next()? {
            let is_active_int: i64 = row.get(3)?;
            Ok(Some(CodexAccountRecord {
                id: row.get(0)?,
                email: row.get(1)?,
                auth_path: row.get(2)?,
                is_active: is_active_int != 0,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
                last_error: row.get(6)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn get_codex_account_by_auth_path(
        &self,
        auth_path: &str,
    ) -> Result<Option<CodexAccountRecord>, rusqlite::Error> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, email, auth_path, is_active, created_at, updated_at, last_error
             FROM codex_accounts
             WHERE auth_path = ?1",
        )?;
        let mut rows = stmt.query(params![auth_path])?;
        if let Some(row) = rows.next()? {
            let is_active_int: i64 = row.get(3)?;
            Ok(Some(CodexAccountRecord {
                id: row.get(0)?,
                email: row.get(1)?,
                auth_path: row.get(2)?,
                is_active: is_active_int != 0,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
                last_error: row.get(6)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn list_codex_accounts(&self) -> Result<Vec<CodexAccountRecord>, rusqlite::Error> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, email, auth_path, is_active, created_at, updated_at, last_error
             FROM codex_accounts
             ORDER BY is_active DESC, datetime(created_at), rowid",
        )?;
        let mut rows = stmt.query([])?;
        let mut accounts = Vec::new();
        while let Some(row) = rows.next()? {
            let is_active_int: i64 = row.get(3)?;
            accounts.push(CodexAccountRecord {
                id: row.get(0)?,
                email: row.get(1)?,
                auth_path: row.get(2)?,
                is_active: is_active_int != 0,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
                last_error: row.get(6)?,
            });
        }
        Ok(accounts)
    }

    pub fn delete_codex_account(&self, id: &str) -> Result<bool, rusqlite::Error> {
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction()?;
        let rows = tx.execute("DELETE FROM codex_accounts WHERE id = ?1", params![id])?;
        tx.commit()?;
        Ok(rows > 0)
    }

    pub fn toggle_codex_account(&self, id: &str) -> Result<Option<bool>, rusqlite::Error> {
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction()?;
        let current_active: Option<i64> = tx
            .query_row(
                "SELECT is_active FROM codex_accounts WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .optional()?;

        let is_active = match current_active {
            Some(v) => v != 0,
            None => return Ok(None),
        };

        let new_active = !is_active;
        let now_str = Utc::now().to_rfc3339();
        tx.execute(
            "UPDATE codex_accounts SET is_active = ?1, updated_at = ?2 WHERE id = ?3",
            params![if new_active { 1 } else { 0 }, now_str, id],
        )?;
        tx.commit()?;
        Ok(Some(new_active))
    }

    pub fn reset_codex_account(&self, id: &str) -> Result<bool, rusqlite::Error> {
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction()?;
        let now_str = Utc::now().to_rfc3339();
        let rows = tx.execute(
            "UPDATE codex_accounts SET last_error = NULL, updated_at = ?1 WHERE id = ?2",
            params![now_str, id],
        )?;
        tx.commit()?;
        Ok(rows > 0)
    }

    pub fn update_codex_account_error(
        &self,
        id: &str,
        last_error: Option<&str>,
    ) -> Result<(), rusqlite::Error> {
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction()?;
        let now_str = Utc::now().to_rfc3339();
        tx.execute(
            "UPDATE codex_accounts SET last_error = ?1, updated_at = ?2 WHERE id = ?3",
            params![last_error, now_str, id],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn get_quota_refresh_setting(&self, key: &str) -> Result<Option<String>, rusqlite::Error> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare("SELECT value FROM quota_refresh_settings WHERE key = ?1")?;
        let mut rows = stmt.query(params![key])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }

    pub fn set_quota_refresh_setting(&self, key: &str, value: &str) -> Result<(), rusqlite::Error> {
        let conn = self.lock_conn()?;
        let now = crate::account::current_time_secs();
        conn.execute(
            "INSERT INTO quota_refresh_settings (key, value, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = ?2, updated_at = ?3",
            params![key, value, now],
        )?;
        Ok(())
    }

    pub fn get_token_saver_settings(
        &self,
    ) -> Result<crate::token_saver::TokenSaverSettings, crate::error::AppError> {
        if let Ok(guard) = self.token_saver_cache.read() {
            return Ok(guard.clone());
        }
        self.get_token_saver_settings_from_db()
    }

    pub fn get_token_saver_settings_from_db(
        &self,
    ) -> Result<crate::token_saver::TokenSaverSettings, crate::error::AppError> {
        let conn = self.lock_conn()?;
        let mut settings = crate::token_saver::TokenSaverSettings::default();

        let mut stmt = conn
            .prepare("SELECT key, value FROM settings WHERE key IN ('token_saver_enabled', 'rtk_enabled', 'caveman_level', 'ponytail_level')")
            .map_err(|e| crate::error::AppError::Internal(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
            })
            .map_err(|e| crate::error::AppError::Internal(e.to_string()))?;

        for (key, val_opt) in rows.flatten() {
            let val = val_opt.unwrap_or_default();
            match key.as_str() {
                "token_saver_enabled" => {
                    let lowered = val.trim().to_ascii_lowercase();
                    if matches!(lowered.as_str(), "0" | "false" | "no" | "off") {
                        settings.token_saver_enabled = false;
                    } else if matches!(lowered.as_str(), "1" | "true" | "yes" | "on") {
                        settings.token_saver_enabled = true;
                    } else {
                        settings.token_saver_enabled =
                            crate::token_saver::DEFAULT_TOKEN_SAVER_ENABLED;
                    }
                }
                "rtk_enabled" => {
                    let lowered = val.trim().to_ascii_lowercase();
                    if matches!(lowered.as_str(), "0" | "false" | "no" | "off") {
                        settings.rtk_enabled = false;
                    } else if matches!(lowered.as_str(), "1" | "true" | "yes" | "on") {
                        settings.rtk_enabled = true;
                    } else {
                        settings.rtk_enabled = crate::token_saver::DEFAULT_RTK_ENABLED;
                    }
                }
                "caveman_level" => {
                    let lowered = val.trim().to_ascii_lowercase();
                    if crate::token_saver::is_valid_token_saver_level(&lowered) {
                        settings.caveman_level = lowered;
                    }
                }
                "ponytail_level" => {
                    let lowered = val.trim().to_ascii_lowercase();
                    if crate::token_saver::is_valid_token_saver_level(&lowered) {
                        settings.ponytail_level = lowered;
                    }
                }
                _ => {}
            }
        }
        Ok(settings)
    }

    pub fn update_token_saver_settings(
        &self,
        updates: &std::collections::HashMap<String, String>,
    ) -> Result<crate::token_saver::TokenSaverSettings, crate::error::AppError> {
        let mut conn = self.lock_conn()?;
        let tx = conn
            .transaction()
            .map_err(|e| crate::error::AppError::Internal(e.to_string()))?;
        for (k, v) in updates {
            tx.execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = ?2",
                rusqlite::params![k, v],
            )
            .map_err(|e| crate::error::AppError::Internal(e.to_string()))?;
        }
        tx.commit()
            .map_err(|e| crate::error::AppError::Internal(e.to_string()))?;
        drop(conn);
        let fresh = self.get_token_saver_settings_from_db()?;
        if let Ok(mut guard) = self.token_saver_cache.write() {
            *guard = fresh.clone();
        }
        Ok(fresh)
    }

    pub fn ensure_legacy_codex_account(
        &self,
        legacy_auth_path: &std::path::Path,
        email: Option<&str>,
    ) -> Result<bool, rusqlite::Error> {
        if !legacy_auth_path.exists() {
            return Ok(false);
        }
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction()?;
        let non_legacy_count: i64 = tx.query_row(
            "SELECT COUNT(*) FROM codex_accounts WHERE id != 'legacy'",
            [],
            |row| row.get(0),
        )?;

        if non_legacy_count > 0 {
            tx.execute("DELETE FROM codex_accounts WHERE id = 'legacy'", [])?;
            tx.commit()?;
            return Ok(false);
        }

        let path_str = legacy_auth_path.to_string_lossy();
        let now_str = Utc::now().to_rfc3339();
        tx.execute(
            "INSERT INTO codex_accounts (id, email, auth_path, is_active, created_at, updated_at)
             VALUES ('legacy', ?1, ?2, 1, ?3, ?3)
             ON CONFLICT(id) DO UPDATE SET email = excluded.email, auth_path = excluded.auth_path, updated_at = excluded.updated_at",
            params![email, path_str, now_str],
        )?;
        tx.commit()?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_token_saver_settings() {
        let db = Database::open_in_memory(None).unwrap();
        let settings = db.get_token_saver_settings().unwrap();
        assert!(settings.token_saver_enabled);
        assert!(settings.rtk_enabled);
        assert_eq!(settings.caveman_level, "lite");
        assert_eq!(settings.ponytail_level, "full");
    }

    #[test]
    fn test_db_init_and_auto_seed() {
        let db = Database::open_in_memory(Some("seed-secret-key")).unwrap();
        let key = db
            .get_key("seed-secret-key")
            .unwrap()
            .expect("seeded key should exist");
        assert_eq!(key.name, "Default Key");
        assert_eq!(key.key, "seed-secret-key");
        assert!(key.is_active);
        assert_eq!(key.total_requests, 0);
    }

    #[test]
    fn test_db_key_lookup() {
        let db = Database::open_in_memory(None).unwrap();
        assert!(db.get_key("nonexistent").unwrap().is_none());

        let created = db.create_key("Alice Key", Some("sk-alice-123")).unwrap();
        assert_eq!(created.name, "Alice Key");
        assert_eq!(created.key, "sk-alice-123");

        let fetched = db
            .get_key("sk-alice-123")
            .unwrap()
            .expect("key should be found");
        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.name, "Alice Key");
        assert!(fetched.is_active);
    }

    #[test]
    fn test_db_inactive_key() {
        let db = Database::open_in_memory(None).unwrap();
        let created = db.create_key("Bob Key", Some("sk-bob-456")).unwrap();
        assert!(created.is_active);

        let updated = db.set_key_active(&created.id, false).unwrap();
        assert!(updated);

        let fetched = db.get_key("sk-bob-456").unwrap().unwrap();
        assert!(!fetched.is_active);
    }

    #[test]
    fn test_db_total_requests_increment() {
        let db = Database::open_in_memory(None).unwrap();
        let created = db
            .create_key("Counter Key", Some("sk-counter-789"))
            .unwrap();
        assert_eq!(created.total_requests, 0);

        let count1 = db.increment_total_requests("sk-counter-789").unwrap();
        assert_eq!(count1, 1);

        let count2 = db.increment_total_requests("sk-counter-789").unwrap();
        assert_eq!(count2, 2);

        let fetched = db.get_key("sk-counter-789").unwrap().unwrap();
        assert_eq!(fetched.total_requests, 2);

        // Fallback key not in DB before increment
        let count_fb = db.increment_total_requests("fallback-fresh-key").unwrap();
        assert_eq!(count_fb, 1);
        let fetched_fb = db.get_key("fallback-fresh-key").unwrap().unwrap();
        assert_eq!(fetched_fb.total_requests, 1);
    }

    #[test]
    fn test_db_crud_and_delete() {
        let db = Database::open_in_memory(None).unwrap();
        let k1 = db.create_key("Key One", None).unwrap();
        let k2 = db.create_key("Key Two", None).unwrap();

        let list = db.list_keys().unwrap();
        assert_eq!(list.len(), 2);

        let deleted = db.delete_key(&k1.id).unwrap();
        assert!(deleted);

        assert!(db.get_key_by_id(&k1.id).unwrap().is_none());
        assert!(db.get_key_by_id(&k2.id).unwrap().is_some());
        assert_eq!(db.list_keys().unwrap().len(), 1);
    }

    #[test]
    fn test_db_request_logging_and_stats() {
        let db = Database::open_in_memory(None).unwrap();

        // Initially empty
        let stats = db.get_admin_stats().unwrap();
        assert_eq!(stats.total_requests, 0);
        assert_eq!(stats.prompt_tokens, 0);
        assert_eq!(stats.completion_tokens, 0);
        assert_eq!(stats.total_tokens, 0);
        assert_eq!(stats.avg_duration_ms, 0.0);
        assert_eq!(stats.error_count, 0);

        // Record 1 success
        db.record_request(
            Some("acc-1"),
            "model-a",
            1000.0,
            "success",
            10,
            20,
            100.0,
            None,
        )
        .unwrap();

        // Record 1 error
        db.record_request(
            Some("acc-1"),
            "model-a",
            1001.0,
            "error",
            0,
            0,
            50.0,
            Some("Upstream timeout"),
        )
        .unwrap();

        // Record 1 cancelled
        db.record_request(
            Some("acc-2"),
            "model-b",
            1002.0,
            "cancelled",
            5,
            5,
            150.0,
            Some("client disconnected"),
        )
        .unwrap();

        let stats = db.get_admin_stats().unwrap();
        assert_eq!(stats.total_requests, 3);
        assert_eq!(stats.prompt_tokens, 15);
        assert_eq!(stats.completion_tokens, 25);
        assert_eq!(stats.total_tokens, 40);
        assert_eq!(stats.avg_duration_ms, 100.0); // (100 + 50 + 150) / 3 = 100.0
        assert_eq!(stats.error_count, 1);

        // Test pagination and filters
        let all_reqs = db.get_requests(10, 0, None, None).unwrap();
        assert_eq!(all_reqs.total, 3);
        assert_eq!(all_reqs.items.len(), 3);
        assert_eq!(all_reqs.items[0].model, "model-b"); // id DESC order

        let model_a_reqs = db.get_requests(10, 0, Some("model-a"), None).unwrap();
        assert_eq!(model_a_reqs.total, 2);
        assert_eq!(model_a_reqs.items.len(), 2);

        let error_reqs = db.get_requests(10, 0, None, Some("error")).unwrap();
        assert_eq!(error_reqs.total, 1);
        assert_eq!(error_reqs.items[0].status, "error");
        assert_eq!(
            error_reqs.items[0].error.as_deref(),
            Some("Upstream timeout")
        );

        // Test request summary
        let summary = db.get_request_summary().unwrap();
        assert_eq!(summary.len(), 2);
        // model-a has 2 requests, model-b has 1 -> sorted by requests DESC
        assert_eq!(summary[0].model, "model-a");
        assert_eq!(summary[0].requests, 2);
        assert_eq!(summary[0].ok, 1);
        assert_eq!(summary[0].errors, 1);
        assert_eq!(summary[0].prompt_tokens, 10);
        assert_eq!(summary[0].completion_tokens, 20);

        assert_eq!(summary[1].model, "model-b");
        assert_eq!(summary[1].requests, 1);
    }

    #[test]
    fn test_mask_api_key_helper() {
        assert_eq!(mask_api_key(""), "");
        assert_eq!(mask_api_key("12345678"), "********");
        assert_eq!(mask_api_key("sk-test-1234567890"), "sk-****7890");
        assert_eq!(mask_api_key("anthropic-secret-9999"), "ant****9999");
        assert!(is_masked_key("sk-****7890"));
        assert!(is_masked_key("********"));
        assert!(!is_masked_key("sk-live-secret-key"));
    }

    #[test]
    fn test_db_provider_crud_and_masking() {
        let db = Database::open_in_memory(None).unwrap();

        // 1. Create provider
        let models = serde_json::json!(["gpt-4o", "gpt-4o-mini"]);
        let created = db
            .create_provider(
                "OpenAI Main",
                "openai-main",
                "openai",
                "https://api.openai.com/v1",
                "sk-proj-testkey12345678",
                &models,
                true,
            )
            .unwrap();

        assert_eq!(created.name, "OpenAI Main");
        assert_eq!(created.prefix, "openai-main");
        assert_eq!(created.provider_type, "openai");
        assert_eq!(created.base_url, "https://api.openai.com/v1");
        assert_eq!(created.api_key, "sk-proj-testkey12345678");
        assert!(created.is_active);

        // Verify response masking
        let resp = created.to_response();
        assert_eq!(resp.api_key, "sk-****5678");
        assert!(!resp.api_key.contains("testkey"));

        // 2. Fetch by ID
        let fetched = db
            .get_provider_by_id(&created.id)
            .unwrap()
            .expect("should find provider");
        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.api_key, "sk-proj-testkey12345678");

        // 3. Fetch by Prefix
        let fetched_prefix = db
            .get_provider_by_prefix("openai-main")
            .unwrap()
            .expect("should find provider by prefix");
        assert_eq!(fetched_prefix.id, created.id);

        // 4. Update provider without changing API key (sending masked key)
        let updated = db
            .update_provider(
                &created.id,
                Some("OpenAI Main Renamed"),
                None,
                None,
                None,
                Some("sk-****5678"), // masked key sent back
                None,
                Some(false),
            )
            .unwrap()
            .expect("should update provider");

        assert_eq!(updated.name, "OpenAI Main Renamed");
        assert!(!updated.is_active);
        // Real API key must NOT be overwritten by masked key
        assert_eq!(updated.api_key, "sk-proj-testkey12345678");

        // 5. Update provider with new raw key
        let updated_key = db
            .update_provider(
                &created.id,
                None,
                None,
                None,
                None,
                Some("sk-proj-brandnewkey9999"),
                None,
                None,
            )
            .unwrap()
            .expect("should update key");
        assert_eq!(updated_key.api_key, "sk-proj-brandnewkey9999");
        assert_eq!(updated_key.to_response().api_key, "sk-****9999");

        // 6. Update models
        let new_models = serde_json::json!(["gpt-4.5-preview"]);
        let updated_models = db
            .update_provider_models(&created.id, &new_models)
            .unwrap()
            .expect("should update models");
        assert_eq!(updated_models.models, new_models);

        // 7. List providers
        let list = db.list_providers().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, created.id);

        // 8. Delete provider
        let deleted = db.delete_provider(&created.id).unwrap();
        assert!(deleted);
        assert!(db.get_provider_by_id(&created.id).unwrap().is_none());
        assert_eq!(db.list_providers().unwrap().len(), 0);
    }

    #[test]
    fn test_db_combo_crud() {
        let db = Database::open_in_memory(None).unwrap();

        let models = serde_json::json!(["openai/gpt-4o", "anthropic/claude-3-5-sonnet"]);
        let created = db
            .create_combo("smart-fallback", &models, "fallback")
            .unwrap();

        assert_eq!(created.name, "smart-fallback");
        assert_eq!(created.strategy, "fallback");
        assert_eq!(created.models, models);

        // Fetch by ID
        let fetched = db
            .get_combo_by_id(&created.id)
            .unwrap()
            .expect("should find combo");
        assert_eq!(fetched.name, "smart-fallback");

        // Fetch by Name
        let fetched_name = db
            .get_combo_by_name("smart-fallback")
            .unwrap()
            .expect("should find combo by name");
        assert_eq!(fetched_name.id, created.id);

        // Update combo
        let new_models = serde_json::json!(["openai/gpt-4o"]);
        let updated = db
            .update_combo(
                &created.id,
                Some("smart-fallback-updated"),
                Some(&new_models),
                Some("round-robin"),
            )
            .unwrap()
            .expect("should update combo");
        assert_eq!(updated.name, "smart-fallback-updated");
        assert_eq!(updated.strategy, "round-robin");
        assert_eq!(updated.models, new_models);

        // List combos
        let list = db.list_combos().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, created.id);

        // Delete combo
        let deleted = db.delete_combo(&created.id).unwrap();
        assert!(deleted);
        assert!(db.get_combo_by_id(&created.id).unwrap().is_none());
        assert_eq!(db.list_combos().unwrap().len(), 0);
    }

    #[test]
    fn test_db_persistence_across_reopen() {
        let dir = format!(
            "/tmp/ag-proxy-rust-db-test-persistence-{}",
            Uuid::new_v4().simple()
        );
        let path = std::path::Path::new(&dir);
        let _ = std::fs::create_dir_all(path);
        let db_file = path.join("data.sqlite");

        {
            let db = Database::open_file(&db_file, None).unwrap();
            let p = db
                .create_provider(
                    "Persisted Provider",
                    "persist-p",
                    "anthropic",
                    "https://api.anthropic.com",
                    "sk-ant-1234567890abcdef",
                    &serde_json::json!(["claude-3-5-sonnet"]),
                    true,
                )
                .unwrap();
            let c = db
                .create_combo(
                    "persist-combo",
                    &serde_json::json!(["persist-p/claude-3-5-sonnet"]),
                    "round-robin",
                )
                .unwrap();
            assert!(!p.id.is_empty());
            assert!(!c.id.is_empty());
        }

        // Reopen database from disk
        {
            let db = Database::open_file(&db_file, None).unwrap();
            let providers = db.list_providers().unwrap();
            assert_eq!(providers.len(), 1);
            assert_eq!(providers[0].name, "Persisted Provider");
            assert_eq!(providers[0].prefix, "persist-p");
            assert_eq!(providers[0].api_key, "sk-ant-1234567890abcdef");

            let combos = db.list_combos().unwrap();
            assert_eq!(combos.len(), 1);
            assert_eq!(combos[0].name, "persist-combo");
            assert_eq!(combos[0].strategy, "round-robin");
        }

        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn test_db_account_repository_crud() {
        let db = Database::open_or_create(std::path::Path::new(":memory:"), None).unwrap();

        // 1. Create account
        let acc = db
            .create_account(Some("acc-repo-1"), "repo-test@example.com", "rt-12345")
            .unwrap();
        assert_eq!(acc.id, "acc-repo-1");
        assert_eq!(acc.email, "repo-test@example.com");
        assert_eq!(acc.refresh_token, "rt-12345");
        assert!(acc.is_active);
        assert_eq!(acc.total_requests, 0);

        // 2. Lookup by ID and email
        let by_id = db.get_account_by_id("acc-repo-1").unwrap().unwrap();
        assert_eq!(by_id.email, "repo-test@example.com");

        let by_email = db
            .get_account_by_email("repo-test@example.com")
            .unwrap()
            .unwrap();
        assert_eq!(by_email.id, "acc-repo-1");

        // 3. Record success
        db.record_account_success("acc-repo-1", 1000.0).unwrap();
        let updated = db.get_account_by_id("acc-repo-1").unwrap().unwrap();
        assert_eq!(updated.total_requests, 1);
        assert_eq!(updated.last_used_at, 1000.0);

        // 4. Record error
        db.record_account_error("acc-repo-1", "rate limit 429", 2000.0)
            .unwrap();
        let errored = db.get_account_by_id("acc-repo-1").unwrap().unwrap();
        assert_eq!(errored.error_count, 1);
        assert_eq!(errored.last_error.as_deref(), Some("rate limit 429"));
        assert_eq!(errored.cooldown_until, 2000.0);

        // 5. Reset cooldown
        let reset = db.reset_account_cooldown("acc-repo-1").unwrap();
        assert!(reset);
        let reset_acc = db.get_account_by_id("acc-repo-1").unwrap().unwrap();
        assert_eq!(reset_acc.cooldown_until, 0.0);
        assert_eq!(reset_acc.error_count, 0);
        assert_eq!(reset_acc.last_error.as_deref(), Some(""));

        // 6. Cleanup stale errors
        db.record_account_error("acc-repo-1", "temp error", 100.0)
            .unwrap();
        let cleaned = db.cleanup_stale_errors(150.0).unwrap();
        assert_eq!(cleaned, 1);
        let cleaned_acc = db.get_account_by_id("acc-repo-1").unwrap().unwrap();
        assert_eq!(cleaned_acc.error_count, 0);
        assert_eq!(cleaned_acc.last_error.as_deref(), Some(""));

        // 7. Delete account
        let deleted = db.delete_account("acc-repo-1").unwrap();
        assert!(deleted);
        assert!(db.get_account_by_id("acc-repo-1").unwrap().is_none());
        assert_eq!(db.list_accounts().unwrap().len(), 0);
    }

    #[test]
    fn test_db_codex_account_repository_crud() {
        let db = Database::open_or_create(std::path::Path::new(":memory:"), None).unwrap();

        // 1. Create codex account
        let acc = db
            .create_codex_account(
                Some("cx-acc-1"),
                Some("codex-user@example.com"),
                "/tmp/codex-auth.json",
                true,
            )
            .unwrap();
        assert_eq!(acc.id, "cx-acc-1");
        assert_eq!(acc.email.as_deref(), Some("codex-user@example.com"));
        assert_eq!(acc.auth_path, "/tmp/codex-auth.json");
        assert!(acc.is_active);

        // 2. Lookup by id, email, auth_path
        let by_id = db.get_codex_account_by_id("cx-acc-1").unwrap().unwrap();
        assert_eq!(by_id.id, "cx-acc-1");

        let by_email = db
            .get_codex_account_by_email("codex-user@example.com")
            .unwrap()
            .unwrap();
        assert_eq!(by_email.id, "cx-acc-1");

        let by_path = db
            .get_codex_account_by_auth_path("/tmp/codex-auth.json")
            .unwrap()
            .unwrap();
        assert_eq!(by_path.id, "cx-acc-1");

        // 3. Save modified account
        let mut modified = by_id.clone();
        modified.email = Some("updated-codex@example.com".to_string());
        db.save_codex_account(&modified).unwrap();
        let updated = db.get_codex_account_by_id("cx-acc-1").unwrap().unwrap();
        assert_eq!(updated.email.as_deref(), Some("updated-codex@example.com"));

        // 4. Update error and reset
        db.update_codex_account_error("cx-acc-1", Some("OAuth token expired"))
            .unwrap();
        let errored = db.get_codex_account_by_id("cx-acc-1").unwrap().unwrap();
        assert_eq!(errored.last_error.as_deref(), Some("OAuth token expired"));

        let reset = db.reset_codex_account("cx-acc-1").unwrap();
        assert!(reset);
        let reset_acc = db.get_codex_account_by_id("cx-acc-1").unwrap().unwrap();
        assert!(reset_acc.last_error.is_none());

        // 5. Toggle active
        let toggled = db.toggle_codex_account("cx-acc-1").unwrap();
        assert_eq!(toggled, Some(false));
        let toggled_acc = db.get_codex_account_by_id("cx-acc-1").unwrap().unwrap();
        assert!(!toggled_acc.is_active);

        // 6. Delete
        let deleted = db.delete_codex_account("cx-acc-1").unwrap();
        assert!(deleted);
        assert!(db.get_codex_account_by_id("cx-acc-1").unwrap().is_none());
        assert_eq!(db.list_codex_accounts().unwrap().len(), 0);
    }
}
