use std::sync::Arc;
use std::time::Instant;

use axum::{
    extract::{rejection::JsonRejection, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};

use crate::auth::AuthenticatedUser;
use crate::error::AppError;
use crate::provider::{EmbeddingRequest, HttpUpstreamProvider, Provider};
use crate::state::AppState;

fn classify_error_status(err: &AppError) -> &'static str {
    let s = err.to_string().to_lowercase();
    if s.contains("quota")
        || s.contains("cooldown")
        || s.contains("usage_limit")
        || s.contains("429")
        || s.contains("resource_exhausted")
        || s.contains("rate_limit")
        || s.contains("payment required")
        || s.contains("credits")
    {
        "quota_exhausted"
    } else {
        "error"
    }
}

pub async fn embeddings(
    auth: AuthenticatedUser,
    State(state): State<AppState>,
    payload_result: Result<Json<EmbeddingRequest>, JsonRejection>,
) -> Result<Response, AppError> {
    let Json(payload) = payload_result.map_err(|e| AppError::BadRequest(e.to_string()))?;

    let start_time = Instant::now();
    let timestamp = chrono::Utc::now().timestamp() as f64;
    let request_id = uuid::Uuid::new_v4().to_string();
    let client_label = if !auth.name.trim().is_empty() {
        auth.name.clone()
    } else {
        crate::db::mask_api_key(&auth.key)
    };

    state.live_registry.register(
        request_id.clone(),
        client_label,
        payload.model.trim().to_string(),
        false,
    );

    // 1. Validation
    if payload.input.is_empty() {
        state.live_registry.finish(&request_id, "error");
        return Err(AppError::BadRequest("input cannot be empty".to_string()));
    }

    let trimmed_model = payload.model.trim();
    if trimmed_model.is_empty() {
        state.live_registry.finish(&request_id, "error");
        return Err(AppError::BadRequest("model cannot be empty".to_string()));
    }

    // 2. Resolve Provider
    let (chosen_provider, account_label) = if state.config.use_mock_provider {
        (state.provider.clone(), "MockProvider".to_string())
    } else {
        let prefix = if trimmed_model.contains('/') {
            trimmed_model.split('/').next().unwrap_or("")
        } else {
            ""
        };

        let prov_match = if !prefix.is_empty() {
            state
                .db
                .get_provider_by_prefix(prefix)
                .ok()
                .flatten()
                .filter(|p| p.is_active)
        } else {
            // Check if any active provider lists this model in its models JSON
            state.db.list_providers().ok().and_then(|providers| {
                providers.into_iter().find(|p| {
                    p.is_active
                        && p.models.as_array().is_some_and(|arr| {
                            arr.iter().any(|v| {
                                v.as_str() == Some(trimmed_model)
                                    || v.get("id").and_then(|s| s.as_str()) == Some(trimmed_model)
                            })
                        })
                })
            })
        };

        if let Some(prov) = prov_match {
            let label = format!("provider:{}", prov.prefix);
            let upstream = Arc::new(HttpUpstreamProvider::new(
                Some(prov.base_url),
                Some(prov.api_key),
                state.config.upstream_connect_timeout_secs,
                state.config.upstream_read_timeout_secs,
                state.config.upstream_request_timeout_secs,
            ));
            (upstream as Arc<dyn Provider>, label)
        } else {
            state.live_registry.finish(&request_id, "error");
            let duration_ms = start_time.elapsed().as_secs_f64() * 1000.0;
            let err_msg =
                format!("Model '{trimmed_model}' not found or no active provider configured");
            let _ = state.db.record_request(
                None,
                trimmed_model,
                timestamp,
                "error",
                0,
                0,
                duration_ms,
                Some(&err_msg),
            );
            return Err(AppError::NotFound(err_msg));
        }
    };

    // 3. Dispatch to provider
    match chosen_provider.embed(&payload).await {
        Ok(response) => {
            let duration_ms = start_time.elapsed().as_secs_f64() * 1000.0;
            state.live_registry.finish(&request_id, "ok");
            let _ = state.db.increment_total_requests(&auth.key);
            let _ = state.db.record_request(
                Some(&account_label),
                trimmed_model,
                timestamp,
                "ok",
                response.usage.prompt_tokens as i64,
                0,
                duration_ms,
                None,
            );
            Ok((StatusCode::OK, Json(response)).into_response())
        }
        Err(err) => {
            let duration_ms = start_time.elapsed().as_secs_f64() * 1000.0;
            let status = classify_error_status(&err);
            state.live_registry.finish(&request_id, status);
            let err_str = err.to_string();
            let _ = state.db.record_request(
                Some(&account_label),
                trimmed_model,
                timestamp,
                status,
                0,
                0,
                duration_ms,
                Some(&err_str),
            );
            Err(err)
        }
    }
}
