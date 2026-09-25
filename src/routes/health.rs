use axum::{extract::State, Json};
use serde_json::{json, Value};

use crate::account::current_time_secs;
use crate::state::AppState;

pub async fn root() -> &'static str {
    "ag-proxy-rust staging service"
}

pub async fn health(State(state): State<AppState>) -> Json<Value> {
    let now = current_time_secs();
    let (google_total, google_active, google_cooldown, _, google_rpm) =
        state.account_pool.get_stats_summary(now);
    let (codex_total, codex_active, codex_cooldown, _, codex_rpm) =
        state.codex_pool.get_stats_summary(now);
    let total = google_total + codex_total;
    let active = google_active + codex_active;
    let cooldown = google_cooldown + codex_cooldown;
    let active_rate = if total > 0 {
        ((active as f64 / total as f64 * 100.0) * 10.0).round() / 10.0
    } else {
        0.0
    };
    let live_rpm = google_rpm + codex_rpm;

    Json(json!({
        "status": "ok",
        "service": "ag-proxy-rust",
        "mode": "staging",
        "port": state.config.port,
        "data_dir": state.config.data_dir.display().to_string(),
        "total_accounts": total,
        "active_accounts": active,
        "cooldown_accounts": cooldown,
        "active_rate": active_rate,
        "live_rpm": live_rpm,
        "codex_accounts": codex_total,
    }))
}
