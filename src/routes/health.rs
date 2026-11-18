use axum::{extract::State, Json};
use serde_json::{json, Value};

use crate::state::AppState;

pub async fn root() -> &'static str {
    "ezrouter staging service"
}

pub async fn health(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "ezrouter",
        "mode": "staging",
        "port": state.config.port,
        "data_dir": state.config.data_dir.display().to_string(),
    }))
}