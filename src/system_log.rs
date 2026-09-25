use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, OnceLock};
use tracing::Subscriber;
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemLogEntry {
    pub id: u64,
    pub timestamp: String,
    pub level: String,
    pub target: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemLogsResponse {
    pub logs: Vec<SystemLogEntry>,
    pub total: usize,
}

pub struct SystemLogBuffer {
    inner: Mutex<SystemLogInner>,
}

struct SystemLogInner {
    logs: VecDeque<SystemLogEntry>,
    next_id: u64,
    max_capacity: usize,
}

static GLOBAL_BUFFER: OnceLock<Arc<SystemLogBuffer>> = OnceLock::new();

impl SystemLogBuffer {
    pub fn new(max_capacity: usize) -> Self {
        Self {
            inner: Mutex::new(SystemLogInner {
                logs: VecDeque::with_capacity(max_capacity.min(500)),
                next_id: 1,
                max_capacity,
            }),
        }
    }

    pub fn global() -> Arc<Self> {
        GLOBAL_BUFFER
            .get_or_init(|| {
                let buf = Arc::new(Self::new(2000));
                buf.seed_from_journalctl(100);
                buf
            })
            .clone()
    }

    pub fn push_entry(&self, timestamp: String, level: String, target: String, message: String) {
        if let Ok(mut inner) = self.inner.lock() {
            let id = inner.next_id;
            inner.next_id += 1;
            if inner.logs.len() >= inner.max_capacity {
                inner.logs.pop_front();
            }
            inner.logs.push_back(SystemLogEntry {
                id,
                timestamp,
                level,
                target,
                message,
            });
        }
    }

    pub fn get_logs(
        &self,
        limit: usize,
        level_filter: Option<&str>,
        search: Option<&str>,
    ) -> SystemLogsResponse {
        let Ok(inner) = self.inner.lock() else {
            return SystemLogsResponse {
                logs: Vec::new(),
                total: 0,
            };
        };

        let filter_level = level_filter
            .map(|s| s.trim().to_uppercase())
            .filter(|s| !s.is_empty() && s != "ALL");

        let filter_search = search
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty());

        let matched: Vec<SystemLogEntry> = inner
            .logs
            .iter()
            .rev()
            .filter(|entry| {
                if let Some(ref lvl) = filter_level {
                    if entry.level.to_uppercase() != *lvl {
                        return false;
                    }
                }
                if let Some(ref query) = filter_search {
                    if !entry.message.to_lowercase().contains(query)
                        && !entry.target.to_lowercase().contains(query)
                    {
                        return false;
                    }
                }
                true
            })
            .take(limit.clamp(1, 1000))
            .cloned()
            .collect();

        let total = matched.len();
        // Return chronologically ascending (oldest -> newest) for console log readability
        let mut chronological = matched;
        chronological.reverse();

        SystemLogsResponse {
            logs: chronological,
            total,
        }
    }

    pub fn clear(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.logs.clear();
        }
    }

    /// Attempts to read recent logs from journalctl if available (e.g. systemd user service).
    pub fn seed_from_journalctl(&self, lines: usize) {
        let output = std::process::Command::new("journalctl")
            .args([
                "--user",
                "-u",
                "ag-proxy-rust",
                "-n",
                &lines.to_string(),
                "--no-pager",
            ])
            .output();

        let Ok(output) = output else {
            return;
        };

        if !output.status.success() {
            return;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(entry) = parse_journal_line(line) {
                self.push_entry(entry.timestamp, entry.level, entry.target, entry.message);
            }
        }
    }
}

fn parse_journal_line(line: &str) -> Option<SystemLogEntry> {
    // Typical line:
    // Thg 9 23 21:49:23 server1 ezrouter[2057064]: 2026-09-23T14:49:23.647678Z  INFO ezrouter: Initializing ezRouter service...
    // Or without prefix:
    // 2026-09-23T14:49:23.647678Z  INFO ag_proxy_rust: Initializing Router Rust service...
    let content = if let Some((_, rest)) = line.split_once("]: ") {
        rest.trim()
    } else {
        line
    };

    let parts: Vec<&str> = content.split_whitespace().collect();
    if parts.len() >= 3
        && (parts[1] == "INFO" || parts[1] == "WARN" || parts[1] == "ERROR" || parts[1] == "DEBUG")
    {
        let timestamp = parts[0].to_string();
        let level = parts[1].to_string();
        let target = parts[2].trim_end_matches(':').to_string();
        let message = parts[3..].join(" ");
        Some(SystemLogEntry {
            id: 0,
            timestamp,
            level,
            target,
            message,
        })
    } else {
        // Fallback generic line
        Some(SystemLogEntry {
            id: 0,
            timestamp: Utc::now().to_rfc3339(),
            level: "INFO".to_string(),
            target: "system".to_string(),
            message: content.to_string(),
        })
    }
}

pub struct SystemLogLayer;

impl<S: Subscriber> Layer<S> for SystemLogLayer {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let metadata = event.metadata();
        let level = metadata.level().to_string();
        let target = metadata.target().to_string();

        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);

        let timestamp = Utc::now().to_rfc3339();
        SystemLogBuffer::global().push_entry(timestamp, level, target, visitor.message);
    }
}

#[derive(Default)]
struct MessageVisitor {
    message: String,
}

impl tracing::field::Visit for MessageVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            use std::fmt::Write;
            let _ = write!(self.message, "{:?}", value);
        } else {
            use std::fmt::Write;
            if !self.message.is_empty() {
                self.message.push(' ');
            }
            let _ = write!(self.message, "{}={:?}", field.name(), value);
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message.push_str(value);
        } else {
            use std::fmt::Write;
            if !self.message.is_empty() {
                self.message.push(' ');
            }
            let _ = write!(self.message, "{}={}", field.name(), value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_log_buffer_push_and_query() {
        let buffer = SystemLogBuffer::new(10);
        buffer.push_entry(
            "2026-09-23T15:00:00Z".to_string(),
            "INFO".to_string(),
            "test_target".to_string(),
            "System initialized successfully".to_string(),
        );
        buffer.push_entry(
            "2026-09-23T15:00:01Z".to_string(),
            "WARN".to_string(),
            "test_target".to_string(),
            "High memory usage warning".to_string(),
        );
        buffer.push_entry(
            "2026-09-23T15:00:02Z".to_string(),
            "ERROR".to_string(),
            "network".to_string(),
            "Connection failed".to_string(),
        );

        let all = buffer.get_logs(10, None, None);
        assert_eq!(all.total, 3);
        assert_eq!(all.logs[0].level, "INFO");
        assert_eq!(all.logs[2].level, "ERROR");

        let errs = buffer.get_logs(10, Some("ERROR"), None);
        assert_eq!(errs.total, 1);
        assert_eq!(errs.logs[0].message, "Connection failed");

        let searched = buffer.get_logs(10, None, Some("memory"));
        assert_eq!(searched.total, 1);
        assert_eq!(searched.logs[0].level, "WARN");
    }

    #[test]
    fn test_parse_journal_line() {
        let line = "Thg 9 23 21:49:23 server1 ezrouter[2057064]: 2026-09-23T14:49:23.647678Z  INFO ezrouter: Initializing ezRouter service...";
        let entry = parse_journal_line(line).expect("Should parse");
        assert_eq!(entry.level, "INFO");
        assert_eq!(entry.target, "ezrouter");
        assert_eq!(entry.message, "Initializing ezRouter service...");
    }
}
