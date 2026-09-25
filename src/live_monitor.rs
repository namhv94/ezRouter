use std::collections::HashMap;
use std::sync::RwLock;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

/// Masks an account identifier (typically email or id) without revealing full credentials.
pub fn mask_account_email(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        return "Unknown".to_string();
    }
    if let Some((local, domain)) = value.split_once('@') {
        let char_count = local.chars().count();
        if char_count <= 2 {
            format!("{local}***@{domain}")
        } else {
            let prefix: String = local.chars().take(2).collect();
            format!("{prefix}***@{domain}")
        }
    } else if let Some(suffix) = value.strip_prefix("codex:") {
        let s: String = suffix.chars().take(8).collect();
        format!("Codex ({s})")
    } else if (value.starts_with("acc_") || value.starts_with("key_")) && value.chars().count() > 10
    {
        let prefix: String = value.chars().take(4).collect();
        let tail: String = value
            .chars()
            .rev()
            .take(4)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        format!("{prefix}...{tail}")
    } else {
        value.to_string()
    }
}

/// Ephemeral active request record.
/// Never stores prompts, completions, tokens, bearer headers, or secrets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveRequestItem {
    pub id: String,
    pub client: String,
    pub requested_model: String,
    pub model: String,
    pub provider: String,
    pub account: String,
    pub status: String, // "routing", "generating", "success", "error", "cancelled"
    pub stream: bool,
    pub started_at: f64,
    pub elapsed_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveRequestsResponse {
    pub items: Vec<ActiveRequestItem>,
    pub server_time: f64,
}

pub struct ActiveRequestRegistry {
    requests: RwLock<HashMap<String, ActiveRequestItem>>,
}

impl Default for ActiveRequestRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ActiveRequestRegistry {
    pub fn new() -> Self {
        Self {
            requests: RwLock::new(HashMap::new()),
        }
    }

    fn now_secs() -> f64 {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0)
    }

    pub fn register(&self, id: String, client: String, requested_model: String, stream: bool) {
        let now = Self::now_secs();
        let item = ActiveRequestItem {
            id: id.clone(),
            client,
            model: requested_model.clone(),
            requested_model,
            provider: "Router".to_string(),
            account: "Đang chọn...".to_string(),
            status: "routing".to_string(),
            stream,
            started_at: now,
            elapsed_ms: 0,
            completed_at: None,
        };

        if let Ok(mut map) = self.requests.write() {
            map.insert(id, item);
        }
    }

    pub fn update_routing(&self, id: &str, provider: &str, model: &str, account: &str) {
        if let Ok(mut map) = self.requests.write() {
            if let Some(item) = map.get_mut(id) {
                item.provider = provider.to_string();
                item.model = model.to_string();
                item.account = account.to_string();
                item.status = "generating".to_string();
            }
        }
    }

    pub fn finish(&self, id: &str, status: &str) {
        let now = Self::now_secs();
        if let Ok(mut map) = self.requests.write() {
            if let Some(item) = map.get_mut(id) {
                item.status = status.to_string();
                item.completed_at = Some(now);
                item.elapsed_ms = ((now - item.started_at) * 1000.0).max(0.0) as u64;
            }
        }
    }

    pub fn get_active(&self) -> ActiveRequestsResponse {
        let now = Self::now_secs();
        let mut map = match self.requests.write() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        // Purge completed requests older than 3.5s or orphaned requests older than 10m
        map.retain(|_, item| {
            if let Some(completed_at) = item.completed_at {
                (now - completed_at) < 3.5
            } else {
                (now - item.started_at) < 600.0
            }
        });

        let mut items: Vec<ActiveRequestItem> = map
            .values()
            .map(|item| {
                let mut copy = item.clone();
                if copy.completed_at.is_none() {
                    copy.elapsed_ms = ((now - copy.started_at) * 1000.0).max(0.0) as u64;
                }
                copy
            })
            .collect();

        items.sort_by(|a, b| {
            a.started_at
                .partial_cmp(&b.started_at)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        ActiveRequestsResponse {
            items,
            server_time: now,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_account_email() {
        assert_eq!(mask_account_email("test@gmail.com"), "te***@gmail.com");
        assert_eq!(mask_account_email("ab@gmail.com"), "ab***@gmail.com");
        assert_eq!(mask_account_email("a@gmail.com"), "a***@gmail.com");
        assert_eq!(
            mask_account_email("codex:0123456789abcdef"),
            "Codex (01234567)"
        );
        assert_eq!(mask_account_email("codex:short"), "Codex (short)");
        assert_eq!(mask_account_email("my-provider"), "my-provider");
    }

    #[test]
    fn test_active_requests_lifecycle() {
        let registry = ActiveRequestRegistry::new();
        let id = "test-req-1".to_string();

        registry.register(
            id.clone(),
            "TestClient".to_string(),
            "ag/gemini-3.8-flash-high".to_string(),
            true,
        );

        let active = registry.get_active();
        assert_eq!(active.items.len(), 1);
        assert_eq!(active.items[0].id, id);
        assert_eq!(active.items[0].client, "TestClient");
        assert_eq!(active.items[0].status, "routing");
        assert_eq!(active.items[0].provider, "Router");

        // Update routing
        registry.update_routing(
            &id,
            "Google Antigravity",
            "ag/gemini-3.8-flash-high",
            "te***@gmail.com",
        );

        let active = registry.get_active();
        assert_eq!(active.items.len(), 1);
        assert_eq!(active.items[0].status, "generating");
        assert_eq!(active.items[0].provider, "Google Antigravity");
        assert_eq!(active.items[0].account, "te***@gmail.com");

        // Finish request
        registry.finish(&id, "success");
        let active = registry.get_active();
        assert_eq!(active.items.len(), 1);
        assert_eq!(active.items[0].status, "success");
        assert!(active.items[0].completed_at.is_some());
    }
}
