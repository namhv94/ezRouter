use std::sync::Arc;
use std::time::Instant;

use axum::{
    extract::{rejection::JsonRejection, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::{json, Value};

use crate::auth::AuthenticatedUser;
use crate::error::AppError;
use crate::provider::HttpUpstreamProvider;
use crate::state::AppState;

pub async fn image_generations(
    auth: AuthenticatedUser,
    State(state): State<AppState>,
    payload_result: Result<Json<Value>, JsonRejection>,
) -> Result<Response, AppError> {
    let Json(mut payload) = payload_result.map_err(|err| AppError::BadRequest(err.to_string()))?;
    let object = payload
        .as_object_mut()
        .ok_or_else(|| AppError::BadRequest("request body must be a JSON object".to_string()))?;

    let prompt = object
        .get("prompt")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::BadRequest("prompt must be a non-empty string".to_string()))?
        .to_string();
    let model = object
        .get("model")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::BadRequest("model must be a non-empty string".to_string()))?
        .to_string();

    let start_time = Instant::now();
    let timestamp = chrono::Utc::now().timestamp() as f64;

    if state.config.use_mock_provider {
        let response = (
            StatusCode::OK,
            Json(mock_image_response(&model, &prompt, object)),
        )
            .into_response();
        let _ = state.db.increment_total_requests(&auth.key);
        let _ = state.db.record_request(
            Some("MockProvider"),
            &model,
            timestamp,
            "ok",
            0,
            0,
            start_time.elapsed().as_secs_f64() * 1000.0,
            None,
        );
        return Ok(response);
    }

    let (provider, routing_prefix) = resolve_provider(&state, &model)?;
    let account_label = format!("provider:{}", provider.prefix);
    let upstream = Arc::new(HttpUpstreamProvider::new(
        Some(provider.base_url),
        Some(provider.api_key),
        state.config.upstream_connect_timeout_secs,
        state.config.upstream_read_timeout_secs,
        state.config.upstream_request_timeout_secs,
    ));

    match upstream
        .generate_image(&payload, routing_prefix.as_deref())
        .await
    {
        Ok(response) => {
            let _ = state.db.increment_total_requests(&auth.key);
            let _ = state.db.record_request(
                Some(&account_label),
                &model,
                timestamp,
                "ok",
                0,
                0,
                start_time.elapsed().as_secs_f64() * 1000.0,
                None,
            );
            Ok((StatusCode::OK, Json(response)).into_response())
        }
        Err(err) => {
            let err_string = err.to_string();
            let _ = state.db.record_request(
                Some(&account_label),
                &model,
                timestamp,
                "error",
                0,
                0,
                start_time.elapsed().as_secs_f64() * 1000.0,
                Some(&err_string),
            );
            Err(err)
        }
    }
}

fn resolve_provider(
    state: &AppState,
    model: &str,
) -> Result<(crate::db::ProviderRecord, Option<String>), AppError> {
    if let Some(prefix) = model.split_once('/').map(|(prefix, _)| prefix) {
        if let Some(provider) = state
            .db
            .get_provider_by_prefix(prefix)
            .ok()
            .flatten()
            .filter(|provider| provider.is_active)
        {
            return Ok((provider, Some(prefix.to_string())));
        }
    }

    let provider = state.db.list_providers().ok().and_then(|providers| {
        providers.into_iter().find(|provider| {
            provider.is_active
                && provider.models.as_array().is_some_and(|models| {
                    models.iter().any(|candidate| {
                        candidate.as_str() == Some(model)
                            || candidate.get("id").and_then(Value::as_str) == Some(model)
                    })
                })
        })
    });

    provider.map(|provider| (provider, None)).ok_or_else(|| {
        AppError::NotFound(format!(
            "Model '{model}' not found or no active provider configured"
        ))
    })
}

fn mock_image_response(
    model: &str,
    prompt: &str,
    payload: &serde_json::Map<String, Value>,
) -> Value {
    let n = payload
        .get("n")
        .and_then(Value::as_u64)
        .unwrap_or(1)
        .clamp(1, 10);
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in model.bytes().chain([0]).chain(prompt.bytes()) {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    let data = (0..n)
        .map(|index| {
            json!({
                "url": format!("mock://images/{hash:016x}-{index}.png"),
                "revised_prompt": prompt,
            })
        })
        .collect::<Vec<_>>();

    json!({
        "created": 0,
        "data": data,
    })
}
