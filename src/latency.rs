// src/latency.rs
// Bounded in-memory latency aggregator. Thread-safe, no DB schema change.
use std::collections::VecDeque;
use std::sync::RwLock;

/// Per-request timing breakdown for one Codex (cx/*) request.
#[derive(Debug, Clone, PartialEq)]
pub struct CodexLatencyTrace {
    pub queue_or_pacing_ms: f64,
    pub token_refresh_ms: f64,
    pub upstream_connect_ms: f64,
    pub upstream_headers_ms: f64,
    pub upstream_ttfb_ms: f64,
    pub upstream_total_ms: f64,
    pub router_transform_ms: f64,
    pub request_total_ms: f64,
    pub attempt_count: usize,
    pub model: String,
    pub is_stream: bool,
    pub timestamp_secs: f64,
    pub status: String, // "completed" | "error" | "cancelled"
}

impl Default for CodexLatencyTrace {
    fn default() -> Self {
        Self {
            queue_or_pacing_ms: 0.0,
            token_refresh_ms: 0.0,
            upstream_connect_ms: 0.0,
            upstream_headers_ms: 0.0,
            upstream_ttfb_ms: 0.0,
            upstream_total_ms: 0.0,
            router_transform_ms: 0.0,
            request_total_ms: 0.0,
            attempt_count: 0,
            model: String::new(),
            is_stream: false,
            timestamp_secs: 0.0,
            status: String::from("completed"),
        }
    }
}

/// Bounded ring buffer of recent traces (max 500).
pub struct LatencyStore {
    traces: RwLock<VecDeque<CodexLatencyTrace>>,
    max_size: usize,
    enabled: bool,
}

impl LatencyStore {
    pub fn new(enabled: bool) -> Self {
        Self {
            traces: RwLock::new(VecDeque::with_capacity(500)),
            max_size: 500,
            enabled,
        }
    }

    pub fn push(&self, trace: CodexLatencyTrace) {
        if !self.enabled {
            return;
        }
        let mut q = self.traces.write().unwrap_or_else(|p| p.into_inner());
        if q.len() >= self.max_size {
            q.pop_front();
        }
        q.push_back(trace);
    }

    pub fn snapshot_recent(&self, window_secs: f64) -> Vec<CodexLatencyTrace> {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        let cutoff = now - window_secs;
        self.traces
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .iter()
            .filter(|t| t.timestamp_secs >= cutoff)
            .cloned()
            .collect()
    }
}

/// Compute percentile from sorted values (0-100).
pub fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

/// Response JSON for /admin/codex/latency
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LatencyAggResponse {
    pub window_seconds: f64,
    pub count: usize,
    pub headers_ms: PercentileStats,
    pub ttfb_ms: PercentileStats,
    pub total_ms: PercentileStats,
    pub pacing_ms: PercentileStats,
    pub refresh_ms: PercentileStats,
    pub upstream_total_ms: PercentileStats,
    pub transform_ms: PercentileStats,
    pub attempts: AttemptStats,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PercentileStats {
    pub p50: f64,
    pub p90: f64,
    pub p95: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AttemptStats {
    pub avg: f64,
}

pub fn aggregate(traces: &[CodexLatencyTrace], window_secs: f64) -> LatencyAggResponse {
    let mut headers: Vec<f64> = traces.iter().map(|t| t.upstream_headers_ms).collect();
    let mut ttfb: Vec<f64> = traces.iter().map(|t| t.upstream_ttfb_ms).collect();
    let mut total: Vec<f64> = traces.iter().map(|t| t.request_total_ms).collect();
    let mut pacing: Vec<f64> = traces.iter().map(|t| t.queue_or_pacing_ms).collect();
    let mut refresh: Vec<f64> = traces.iter().map(|t| t.token_refresh_ms).collect();
    let mut up_total: Vec<f64> = traces.iter().map(|t| t.upstream_total_ms).collect();
    let mut transform: Vec<f64> = traces.iter().map(|t| t.router_transform_ms).collect();
    let avg_attempts = if traces.is_empty() {
        0.0
    } else {
        traces.iter().map(|t| t.attempt_count as f64).sum::<f64>() / traces.len() as f64
    };
    for v in [
        &mut headers,
        &mut ttfb,
        &mut total,
        &mut pacing,
        &mut refresh,
        &mut up_total,
        &mut transform,
    ] {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    }
    LatencyAggResponse {
        window_seconds: window_secs,
        count: traces.len(),
        headers_ms: PercentileStats {
            p50: percentile(&headers, 50.0),
            p90: percentile(&headers, 90.0),
            p95: percentile(&headers, 95.0),
        },
        ttfb_ms: PercentileStats {
            p50: percentile(&ttfb, 50.0),
            p90: percentile(&ttfb, 90.0),
            p95: percentile(&ttfb, 95.0),
        },
        total_ms: PercentileStats {
            p50: percentile(&total, 50.0),
            p90: percentile(&total, 90.0),
            p95: percentile(&total, 95.0),
        },
        pacing_ms: PercentileStats {
            p50: percentile(&pacing, 50.0),
            p90: percentile(&pacing, 90.0),
            p95: percentile(&pacing, 95.0),
        },
        refresh_ms: PercentileStats {
            p50: percentile(&refresh, 50.0),
            p90: percentile(&refresh, 90.0),
            p95: percentile(&refresh, 95.0),
        },
        upstream_total_ms: PercentileStats {
            p50: percentile(&up_total, 50.0),
            p90: percentile(&up_total, 90.0),
            p95: percentile(&up_total, 95.0),
        },
        transform_ms: PercentileStats {
            p50: percentile(&transform, 50.0),
            p90: percentile(&transform, 90.0),
            p95: percentile(&transform, 95.0),
        },
        attempts: AttemptStats { avg: avg_attempts },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn make_test_trace(ttfb: f64, total: f64) -> CodexLatencyTrace {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        CodexLatencyTrace {
            queue_or_pacing_ms: 1.0,
            token_refresh_ms: 0.0,
            upstream_connect_ms: 5.0,
            upstream_headers_ms: 5.0,
            upstream_ttfb_ms: ttfb,
            upstream_total_ms: total - 2.0,
            router_transform_ms: 2.0,
            request_total_ms: total,
            attempt_count: 1,
            model: "cx/gpt-5.6-luna".to_string(),
            is_stream: false,
            timestamp_secs: now,
            status: String::from("completed"),
        }
    }

    #[test]
    fn test_latency_store_push_and_snapshot() {
        let store = LatencyStore::new(true);
        store.push(make_test_trace(10.0, 100.0));
        store.push(make_test_trace(20.0, 200.0));
        store.push(make_test_trace(30.0, 300.0));

        let recent = store.snapshot_recent(60.0);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].upstream_ttfb_ms, 10.0);
        assert_eq!(recent[1].upstream_ttfb_ms, 20.0);
        assert_eq!(recent[2].upstream_ttfb_ms, 30.0);
    }

    #[test]
    fn test_latency_store_max_size() {
        let store = LatencyStore::new(true);
        for i in 1..=501 {
            let mut tr = make_test_trace(i as f64, (i * 10) as f64);
            tr.timestamp_secs = 1000.0 + (i as f64);
            store.push(tr);
        }

        let q = store.traces.read().unwrap();
        assert_eq!(q.len(), 500);
        // The first trace pushed (i=1) should have been dropped
        assert_eq!(q.front().unwrap().upstream_ttfb_ms, 2.0);
        assert_eq!(q.back().unwrap().upstream_ttfb_ms, 501.0);
    }

    #[test]
    fn test_aggregate_percentiles() {
        let mut traces = Vec::new();
        // 10 traces with ttfb from 10 to 100
        for i in 1..=10 {
            traces.push(make_test_trace(i as f64 * 10.0, i as f64 * 100.0));
        }
        let agg = aggregate(&traces, 900.0);
        assert_eq!(agg.count, 10);
        assert_eq!(agg.window_seconds, 900.0);
        assert_eq!(agg.attempts.avg, 1.0);

        // 10 sorted values: 10, 20, 30, 40, 50, 60, 70, 80, 90, 100
        // formula in percentile: idx = ((p / 100.0) * (len - 1)).round()
        // len - 1 = 9
        // p50: 0.5 * 9 = 4.5 -> round() = 5 -> sorted[5] = 60.0
        // p90: 0.9 * 9 = 8.1 -> round() = 8 -> sorted[8] = 90.0
        // p95: 0.95 * 9 = 8.55 -> round() = 9 -> sorted[9] = 100.0
        assert_eq!(agg.ttfb_ms.p50, 60.0);
        assert_eq!(agg.ttfb_ms.p90, 90.0);
        assert_eq!(agg.ttfb_ms.p95, 100.0);

        assert_eq!(agg.total_ms.p50, 600.0);
        assert_eq!(agg.total_ms.p90, 900.0);
        assert_eq!(agg.total_ms.p95, 1000.0);

        assert_eq!(agg.headers_ms.p50, 5.0);
        assert_eq!(agg.headers_ms.p90, 5.0);
        assert_eq!(agg.headers_ms.p95, 5.0);
    }

    #[test]
    fn test_codex_latency_trace_default_and_status() {
        let trace = CodexLatencyTrace::default();
        assert_eq!(trace.status, "completed");
        assert_eq!(trace.upstream_headers_ms, 0.0);
        assert_eq!(trace.upstream_ttfb_ms, 0.0);
    }

    #[test]
    fn test_latency_store_disabled() {
        let store = LatencyStore::new(false);
        store.push(make_test_trace(10.0, 100.0));
        store.push(make_test_trace(20.0, 200.0));

        let recent = store.snapshot_recent(60.0);
        assert!(recent.is_empty());
        assert_eq!(store.traces.read().unwrap().len(), 0);
    }

    #[test]
    fn test_latency_store_poison_recovery() {
        let store = std::sync::Arc::new(LatencyStore::new(true));
        let store_clone = store.clone();

        // Deliberately poison the RwLock by panicking while holding write guard
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = store_clone.traces.write().unwrap();
            panic!("poisoning lock");
        }));

        assert!(store.traces.is_poisoned());

        // push() should recover from poison without panic
        store.push(make_test_trace(15.0, 150.0));

        // snapshot_recent() should also recover from poison without panic
        let recent = store.snapshot_recent(60.0);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].upstream_ttfb_ms, 15.0);
    }
}
