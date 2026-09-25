use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use regex::Regex;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::account::{current_time_secs, AccountPool};
use crate::codex::CodexAccountPool;
use crate::db::Database;

pub const DEFAULT_QUOTA_REFRESH_INTERVAL_SECS: u64 = 300; // conservative 5 minutes
pub const DEFAULT_QUOTA_REFRESH_COOLDOWN_SECS: u64 = 60; // 60s per-account cooldown
pub const DEFAULT_MAX_REFRESH_CONCURRENCY: usize = 2; // bounded concurrency to prevent upstream storms
pub const MIN_QUOTA_REFRESH_INTERVAL_SECS: u64 = 10; // minimum allowed interval

/// Scrub sensitive tokens, keys, passwords and credentials from error messages before logging or storing.
pub fn sanitize_error_message(value: &str) -> String {
    let mut text = value.replace(['\r', '\n'], " ").trim().to_string();

    // 1. Mask Bearer tokens
    if let Ok(re_bearer) = Regex::new(r"(?i)\bbearer\s+([a-zA-Z0-9_\-\.]{6,})") {
        text = re_bearer.replace_all(&text, "bearer: ••••").to_string();
    }

    // 2. Mask OpenAI / Anthropic / general API keys (sk-...)
    if let Ok(re_sk) = Regex::new(r"(?i)\b(?:sk|sess)-[a-zA-Z0-9_\-]{8,}") {
        text = re_sk.replace_all(&text, "[REDACTED_KEY]").to_string();
    }

    // 3. Mask Google OAuth refresh tokens (1//...)
    if let Ok(re_rt) = Regex::new(r"(?i)\b1//[a-zA-Z0-9_\-]{8,}") {
        text = re_rt
            .replace_all(&text, "[REDACTED_REFRESH_TOKEN]")
            .to_string();
    }

    // 4. Mask Google OAuth access tokens (ya29....)
    if let Ok(re_at) = Regex::new(r"(?i)\bya29\.[a-zA-Z0-9_\-]{8,}") {
        text = re_at
            .replace_all(&text, "[REDACTED_ACCESS_TOKEN]")
            .to_string();
    }

    // 5. Mask key=value or key: value credential pairs
    if let Ok(re_pairs) = Regex::new(
        r"(?i)\b(bearer|token|refresh_token|access_token|secret|password|key|client_secret|auth|authorization)\s*[:=]\s*\S+",
    ) {
        text = re_pairs.replace_all(&text, "$1: ••••").to_string();
    }

    let trimmed = text.trim();
    if trimmed.chars().count() > 240 {
        trimmed.chars().take(240).collect()
    } else {
        trimmed.to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct QuotaRefreshSummary {
    pub google_total: usize,
    pub google_refreshed: usize,
    pub google_skipped_cooldown: usize,
    pub google_skipped_inactive: usize,
    pub codex_total: usize,
    pub codex_refreshed: usize,
    pub codex_skipped_cooldown: usize,
    pub codex_skipped_inactive: usize,
    pub errors: usize,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaRefreshStatusResponse {
    pub ok: bool,
    pub enabled: bool,
    pub interval_secs: u64,
    pub last_refresh: Option<f64>,
    pub next_refresh: Option<f64>,
    pub last_error: Option<String>,
    pub is_refreshing: bool,
    pub last_summary: Option<QuotaRefreshSummary>,
}

struct RefreshGuard<'a>(&'a AtomicBool);

impl<'a> Drop for RefreshGuard<'a> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

pub struct QuotaRefreshWorker {
    db: Arc<Database>,
    account_pool: Arc<AccountPool>,
    codex_pool: Arc<CodexAccountPool>,
    enabled: AtomicBool,
    interval_secs: AtomicU64,
    account_cooldown_secs: AtomicU64,
    max_concurrency: usize,
    is_refreshing: AtomicBool,
    last_refresh: RwLock<Option<f64>>,
    next_refresh: RwLock<Option<f64>>,
    last_error: RwLock<Option<String>>,
    last_summary: RwLock<Option<QuotaRefreshSummary>>,
    notify: Arc<tokio::sync::Notify>,
}

impl QuotaRefreshWorker {
    pub fn new(
        db: Arc<Database>,
        account_pool: Arc<AccountPool>,
        codex_pool: Arc<CodexAccountPool>,
    ) -> Self {
        // 1. Interval: env override > DB persisted > conservative default (300s / 5 min)
        let interval_secs = std::env::var("AG_QUOTA_REFRESH_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .or_else(|| {
                db.get_quota_refresh_setting("interval_secs")
                    .ok()
                    .flatten()
                    .and_then(|s| s.parse::<u64>().ok())
            })
            .unwrap_or(DEFAULT_QUOTA_REFRESH_INTERVAL_SECS)
            .max(MIN_QUOTA_REFRESH_INTERVAL_SECS);

        // 2. Enabled: env override > DB persisted > strictly false default
        let enabled = std::env::var("AG_QUOTA_REFRESH_ENABLED")
            .ok()
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .or_else(|| {
                db.get_quota_refresh_setting("enabled")
                    .ok()
                    .flatten()
                    .map(|s| s == "true" || s == "1")
            })
            .unwrap_or(false);

        // 3. Cooldown: env or default 60s
        let account_cooldown_secs = std::env::var("AG_QUOTA_REFRESH_COOLDOWN_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(DEFAULT_QUOTA_REFRESH_COOLDOWN_SECS);

        // 4. Concurrency: env or default 2
        let max_concurrency = std::env::var("AG_QUOTA_REFRESH_MAX_CONCURRENCY")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(DEFAULT_MAX_REFRESH_CONCURRENCY)
            .max(1);

        // 5. Restore last refresh / error / summary from DB
        let last_refresh = db
            .get_quota_refresh_setting("last_refresh")
            .ok()
            .flatten()
            .and_then(|s| s.parse::<f64>().ok());

        let last_error = db
            .get_quota_refresh_setting("last_error")
            .ok()
            .flatten()
            .map(|s| sanitize_error_message(&s));

        let last_summary = db
            .get_quota_refresh_setting("last_summary")
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str::<QuotaRefreshSummary>(&s).ok());

        let now = current_time_secs();
        let next_refresh = if enabled {
            Some(now + interval_secs as f64)
        } else {
            None
        };

        Self {
            db,
            account_pool,
            codex_pool,
            enabled: AtomicBool::new(enabled),
            interval_secs: AtomicU64::new(interval_secs),
            account_cooldown_secs: AtomicU64::new(account_cooldown_secs),
            max_concurrency,
            is_refreshing: AtomicBool::new(false),
            last_refresh: RwLock::new(last_refresh),
            next_refresh: RwLock::new(next_refresh),
            last_error: RwLock::new(last_error),
            last_summary: RwLock::new(last_summary),
            notify: Arc::new(tokio::sync::Notify::new()),
        }
    }

    pub fn status(&self) -> QuotaRefreshStatusResponse {
        QuotaRefreshStatusResponse {
            ok: true,
            enabled: self.enabled.load(Ordering::SeqCst),
            interval_secs: self.interval_secs.load(Ordering::SeqCst),
            last_refresh: *self.last_refresh.read().unwrap(),
            next_refresh: *self.next_refresh.read().unwrap(),
            last_error: self.last_error.read().unwrap().clone(),
            is_refreshing: self.is_refreshing.load(Ordering::SeqCst),
            last_summary: self.last_summary.read().unwrap().clone(),
        }
    }

    pub fn set_enabled(&self, enabled: bool) -> Result<(), rusqlite::Error> {
        self.enabled.store(enabled, Ordering::SeqCst);
        self.db
            .set_quota_refresh_setting("enabled", &enabled.to_string())?;

        let now = current_time_secs();
        if enabled {
            let interval = self.interval_secs.load(Ordering::SeqCst);
            *self.next_refresh.write().unwrap() = Some(now + interval as f64);
        } else {
            *self.next_refresh.write().unwrap() = None;
        }

        self.notify.notify_waiters();
        info!("Quota refresh auto-worker set enabled: {}", enabled);
        Ok(())
    }

    pub fn set_interval_secs(&self, interval_secs: u64) -> Result<(), rusqlite::Error> {
        let interval = interval_secs.max(MIN_QUOTA_REFRESH_INTERVAL_SECS);
        self.interval_secs.store(interval, Ordering::SeqCst);
        self.db
            .set_quota_refresh_setting("interval_secs", &interval.to_string())?;

        if self.enabled.load(Ordering::SeqCst) {
            let now = current_time_secs();
            *self.next_refresh.write().unwrap() = Some(now + interval as f64);
        }

        self.notify.notify_waiters();
        info!("Quota refresh interval set to {} seconds", interval);
        Ok(())
    }

    pub async fn run_refresh(&self) -> Result<QuotaRefreshSummary, String> {
        // Enforce no duplicate concurrent refresh
        if self
            .is_refreshing
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            debug!("Quota refresh already in progress, skipping concurrent trigger");
            return Err("Quota refresh already in progress".to_string());
        }

        let _guard = RefreshGuard(&self.is_refreshing);
        let start = std::time::Instant::now();
        let now = current_time_secs();
        let cooldown_window = self.account_cooldown_secs.load(Ordering::SeqCst) as f64;
        let sem = Arc::new(tokio::sync::Semaphore::new(self.max_concurrency));

        let mut summary = QuotaRefreshSummary::default();
        let mut last_cycle_error = None;

        // 1. Google Antigravity Accounts (bounded concurrent execution)
        let google_accounts = self.account_pool.list_accounts();
        summary.google_total = google_accounts.len();

        let mut eligible_google = Vec::new();
        for acc in google_accounts {
            if !acc.is_active {
                summary.google_skipped_inactive += 1;
                continue;
            }
            if acc.cooldown_remaining > 0.0 {
                summary.google_skipped_cooldown += 1;
                continue;
            }
            if let Some(last_attempt) = self.account_pool.get_quota_last_attempt(&acc.id) {
                if now - last_attempt < cooldown_window {
                    summary.google_skipped_cooldown += 1;
                    continue;
                }
            }
            eligible_google.push(acc);
        }

        if !eligible_google.is_empty() {
            let mut join_set = tokio::task::JoinSet::new();
            for acc in eligible_google {
                let pool = self.account_pool.clone();
                let sem_clone = sem.clone();
                join_set.spawn(async move {
                    let _permit = sem_clone.acquire_owned().await.ok();
                    let res = pool.refresh_quota(&acc.id, true).await;
                    (acc.id, res)
                });
            }

            while let Some(res) = join_set.join_next().await {
                match res {
                    Ok((_acc_id, Ok(_))) => {
                        summary.google_refreshed += 1;
                    }
                    Ok((acc_id, Err(err))) => {
                        summary.errors += 1;
                        let masked = sanitize_error_message(&err.to_string());
                        let _ = self.account_pool.mark_error(&acc_id, &masked, false);
                        warn!(
                            "Google quota refresh failed for account '{}': {}",
                            acc_id, masked
                        );
                        last_cycle_error = Some(masked);
                    }
                    Err(e) => {
                        warn!("Google quota refresh join error: {e}");
                    }
                }
            }
        }

        // 2. OpenAI Codex Accounts (bounded concurrent execution)
        let codex_accounts: Vec<(String, Arc<crate::codex::CodexAccount>)> = {
            let map = self.codex_pool.accounts.read().unwrap();
            map.iter().map(|(id, a)| (id.clone(), a.clone())).collect()
        };
        summary.codex_total = codex_accounts.len();

        let mut eligible_codex = Vec::new();
        for (acc_id, acc) in codex_accounts {
            if !acc.is_active.load(Ordering::SeqCst) {
                summary.codex_skipped_inactive += 1;
                continue;
            }
            let last_attempt = *acc.quota_last_attempt.read().unwrap();
            if now - last_attempt < cooldown_window {
                summary.codex_skipped_cooldown += 1;
                continue;
            }
            eligible_codex.push((acc_id, acc));
        }

        if !eligible_codex.is_empty() {
            let mut join_set = tokio::task::JoinSet::new();
            for (_acc_id, acc) in eligible_codex {
                let pool = self.codex_pool.clone();
                let sem_clone = sem.clone();
                join_set.spawn(async move {
                    let _permit = sem_clone.acquire_owned().await.ok();
                    let res = pool.fetch_account_quota(&acc, true).await;
                    (acc.id.clone(), acc, res)
                });
            }

            while let Some(res) = join_set.join_next().await {
                match res {
                    Ok((_acc_id, _acc, Ok(_))) => {
                        summary.codex_refreshed += 1;
                    }
                    Ok((acc_id, acc, Err(err))) => {
                        summary.errors += 1;
                        let masked = sanitize_error_message(&err);
                        self.codex_pool.mark_error(&acc, &masked, false);
                        warn!(
                            "Codex quota refresh failed for account '{}': {}",
                            acc_id, masked
                        );
                        last_cycle_error = Some(masked);
                    }
                    Err(e) => {
                        warn!("Codex quota refresh join error: {e}");
                    }
                }
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        summary.duration_ms = duration_ms;

        let finish_now = current_time_secs();
        *self.last_refresh.write().unwrap() = Some(finish_now);
        let interval = self.interval_secs.load(Ordering::SeqCst);
        let next_interval = Self::calculate_jittered_interval(interval);
        if self.enabled.load(Ordering::SeqCst) {
            *self.next_refresh.write().unwrap() = Some(finish_now + next_interval);
        } else {
            *self.next_refresh.write().unwrap() = None;
        }
        *self.last_summary.write().unwrap() = Some(summary.clone());

        if let Some(ref err) = last_cycle_error {
            *self.last_error.write().unwrap() = Some(err.clone());
            let _ = self.db.set_quota_refresh_setting("last_error", err);
        }

        // Persist safely to DB
        let _ = self
            .db
            .set_quota_refresh_setting("last_refresh", &finish_now.to_string());
        if let Ok(sum_json) = serde_json::to_string(&summary) {
            let _ = self.db.set_quota_refresh_setting("last_summary", &sum_json);
        }

        info!(
            "Quota refresh cycle finished in {}ms: Google refreshed={}/{} (cd_skip={}, inact_skip={}), Codex refreshed={}/{} (cd_skip={}, inact_skip={}), errors={}",
            summary.duration_ms,
            summary.google_refreshed,
            summary.google_total,
            summary.google_skipped_cooldown,
            summary.google_skipped_inactive,
            summary.codex_refreshed,
            summary.codex_total,
            summary.codex_skipped_cooldown,
            summary.codex_skipped_inactive,
            summary.errors
        );

        Ok(summary)
    }

    pub fn calculate_jittered_interval(base_interval_secs: u64) -> f64 {
        if base_interval_secs <= 10 {
            return base_interval_secs as f64;
        }
        let max_jitter = (base_interval_secs as f64 * 0.05).clamp(1.0, 10.0);
        let jitter = (uuid::Uuid::new_v4().as_u128() % 1000) as f64 / 1000.0 * max_jitter;
        base_interval_secs as f64 + jitter
    }

    pub fn start_background_task(self: Arc<Self>) {
        if tokio::runtime::Handle::try_current().is_err() {
            debug!("No tokio runtime active; skipping quota worker background task spawn");
            return;
        }

        let worker = self.clone();
        tokio::spawn(async move {
            let jitter_ms = (uuid::Uuid::new_v4().as_u128() % 3000) as u64;
            tokio::time::sleep(Duration::from_millis(2000 + jitter_ms)).await;
            if worker.enabled.load(Ordering::SeqCst) {
                info!("Executing startup initial quota refresh with anti-herd jitter");
                let _ = worker.run_refresh().await;
            }

            worker.run_loop().await;
        });
    }

    pub async fn run_loop(self: Arc<Self>) {
        loop {
            if !self.enabled.load(Ordering::SeqCst) {
                self.notify.notified().await;
                continue;
            }

            let now = current_time_secs();
            let next = *self.next_refresh.read().unwrap();

            match next {
                Some(target) if now >= target => {
                    let _ = self.run_refresh().await;
                }
                Some(target) => {
                    let sleep_secs = (target - now).max(0.1);
                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_secs_f64(sleep_secs)) => {
                            let _ = self.run_refresh().await;
                        }
                        _ = self.notify.notified() => {
                            // Settings changed, re-evaluate
                        }
                    }
                }
                None => {
                    let interval = self.interval_secs.load(Ordering::SeqCst);
                    *self.next_refresh.write().unwrap() = Some(now + interval as f64);
                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_secs(interval)) => {
                            let _ = self.run_refresh().await;
                        }
                        _ = self.notify.notified() => {}
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn create_test_worker() -> (
        Arc<QuotaRefreshWorker>,
        Arc<Database>,
        Arc<AccountPool>,
        Arc<CodexAccountPool>,
    ) {
        let db = Arc::new(Database::open_in_memory(Some("test-secret")).unwrap());
        let ag_pool = Arc::new(AccountPool::new(db.clone()));
        let cx_pool = Arc::new(CodexAccountPool::new(db.clone()));
        let worker = Arc::new(QuotaRefreshWorker::new(
            db.clone(),
            ag_pool.clone(),
            cx_pool.clone(),
        ));
        (worker, db, ag_pool, cx_pool)
    }

    #[test]
    fn test_token_sanitization() {
        let raw = "Error upstream: Bearer ya29.a0AfH6SMDh3829-super-secret-token and sk-proj-1234567890abcdef and 1//0gXyz123456789refresh token=secret123 password: mypassword";
        let sanitized = sanitize_error_message(raw);
        assert!(!sanitized.contains("ya29.a0AfH6SMDh3829-super-secret-token"));
        assert!(!sanitized.contains("sk-proj-1234567890abcdef"));
        assert!(!sanitized.contains("1//0gXyz123456789refresh"));
        assert!(!sanitized.contains("secret123"));
        assert!(!sanitized.contains("mypassword"));
        assert!(
            sanitized.contains("bearer: ••••") || sanitized.contains("[REDACTED_ACCESS_TOKEN]")
        );
    }

    #[test]
    fn test_scheduler_interval_and_config_defaults() {
        let (worker, db, _, _) = create_test_worker();
        let status = worker.status();
        assert!(!status.enabled, "Default should be strictly OFF");
        assert_eq!(status.interval_secs, DEFAULT_QUOTA_REFRESH_INTERVAL_SECS);
        assert!(status.next_refresh.is_none());

        // Update interval to 60s
        worker.set_interval_secs(60).unwrap();
        assert_eq!(worker.status().interval_secs, 60);
        assert_eq!(
            db.get_quota_refresh_setting("interval_secs").unwrap(),
            Some("60".to_string())
        );

        // Update interval below minimum clamps to MIN_QUOTA_REFRESH_INTERVAL_SECS
        worker.set_interval_secs(5).unwrap();
        assert_eq!(
            worker.status().interval_secs,
            MIN_QUOTA_REFRESH_INTERVAL_SECS
        );

        // Enable auto refresh
        worker.set_enabled(true).unwrap();
        assert!(worker.status().enabled);
        assert!(worker.status().next_refresh.is_some());
        assert_eq!(
            db.get_quota_refresh_setting("enabled").unwrap(),
            Some("true".to_string())
        );

        // Disable auto refresh
        worker.set_enabled(false).unwrap();
        assert!(!worker.status().enabled);
        assert!(worker.status().next_refresh.is_none());
    }

    #[tokio::test]
    async fn test_no_duplicate_concurrent_refresh() {
        let (worker, _, _, _) = create_test_worker();

        // Artificially lock is_refreshing
        worker.is_refreshing.store(true, Ordering::SeqCst);

        // Attempt concurrent refresh
        let res = worker.run_refresh().await;
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("already in progress"));

        // Release lock
        worker.is_refreshing.store(false, Ordering::SeqCst);

        // Subsequent call succeeds
        let res2 = worker.run_refresh().await;
        assert!(res2.is_ok());
        assert!(!worker.is_refreshing.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_cooldown_and_skips() {
        let (worker, _, ag_pool, _) = create_test_worker();

        // 1. Create active normal account
        let _acc1 = ag_pool.add_account("normal@test.com", "rt-1").unwrap();

        // 2. Create inactive account
        let acc2 = ag_pool.add_account("inactive@test.com", "rt-2").unwrap();
        ag_pool.set_account_active(&acc2.id, false);

        // 3. Create account in error cooldown
        let acc3 = ag_pool.add_account("cooldown@test.com", "rt-3").unwrap();
        ag_pool.set_account_cooldown(&acc3.id, current_time_secs() + 300.0);

        // Run refresh
        let summary = worker.run_refresh().await.unwrap();
        assert_eq!(summary.google_total, 3);
        assert_eq!(summary.google_refreshed, 1);
        assert_eq!(summary.google_skipped_inactive, 1);
        assert_eq!(summary.google_skipped_cooldown, 1);

        // Immediate second run should skip acc1 because of the per-account 60s cooldown!
        let summary2 = worker.run_refresh().await.unwrap();
        assert_eq!(summary2.google_refreshed, 0);
        assert_eq!(summary2.google_skipped_cooldown, 2); // acc1 (within 60s) + acc3 (in cooldown)
    }

    #[tokio::test]
    async fn test_error_and_backoff() {
        let (worker, db, ag_pool, _) = create_test_worker();

        // Inject sanitized error into worker status directly and verify persistence
        let error_msg =
            "Upstream quota call failed with 429 Too Many Requests (Bearer secret123456)";
        let masked = sanitize_error_message(error_msg);
        *worker.last_error.write().unwrap() = Some(masked.clone());
        db.set_quota_refresh_setting("last_error", &masked).unwrap();

        let loaded = db.get_quota_refresh_setting("last_error").unwrap();
        assert_eq!(loaded, Some(masked.clone()));
        assert!(!masked.contains("secret123456"));
        assert!(masked.contains("bearer: ••••"));

        // Test account error backoff via mark_error
        let acc = ag_pool.add_account("err@test.com", "rt-err").unwrap();
        ag_pool
            .mark_error(&acc.id, "429 Rate limit", false)
            .unwrap();
        let updated = ag_pool.get_account(&acc.id).unwrap();
        assert!(updated.cooldown_remaining > 0.0);
        assert_eq!(updated.error_count, 1);
    }
    #[tokio::test]
    async fn test_quota_refresh_bounded_concurrency_and_jitter() {
        let (worker, _, ag_pool, _) = create_test_worker();

        for i in 1..=5 {
            let email = format!("user{}@example.com", i);
            let rt = format!("rt-{}", i);
            let _ = ag_pool.add_account(&email, &rt).unwrap();
        }

        let summary = worker.run_refresh().await.unwrap();
        assert_eq!(summary.google_total, 5);
        assert_eq!(summary.google_refreshed, 5);
        assert_eq!(summary.errors, 0);

        // Verify jitter calculation produces value >= base interval
        let base = 300;
        let jittered = QuotaRefreshWorker::calculate_jittered_interval(base);
        assert!((300.0..=320.0).contains(&jittered));
    }
}
