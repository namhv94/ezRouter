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

    if is_codex_image_model(&model) {
        return generate_codex_image(&state, &auth, &model, &prompt, timestamp, start_time).await;
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

pub fn is_codex_image_model(model: &str) -> bool {
    let m = model.to_lowercase();
    m == "cx/image-gen"
        || m == "image-gen"
        || m == "cx/gpt-image"
        || m == "cx/gpt-image-2"
        || m == "gpt-image-2"
        || m == "codex-image"
        || m.starts_with("cx/image")
}

async fn find_first_png(dir: &std::path::Path) -> Option<std::path::PathBuf> {
    let mut entries = tokio::fs::read_dir(dir).await.ok()?;
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.is_dir() {
            if let Some(nested) = Box::pin(find_first_png(&path)).await {
                return Some(nested);
            }
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("png"))
            .unwrap_or(false)
        {
            if let Ok(meta) = tokio::fs::metadata(&path).await {
                if meta.len() > 1000 {
                    return Some(path);
                }
            }
        }
    }
    None
}

pub async fn generate_codex_image(
    state: &AppState,
    auth: &AuthenticatedUser,
    model: &str,
    prompt: &str,
    timestamp: f64,
    start_time: Instant,
) -> Result<Response, AppError> {
    let mut lease = match state.codex_pool.acquire_lease_with_pacing().await {
        Some(l) => l,
        None => {
            let (status, msg) = state.codex_pool.diagnose_unavailable();
            if status == StatusCode::TOO_MANY_REQUESTS {
                return Err(AppError::RateLimit(msg));
            } else {
                return Err(AppError::BadGateway(msg));
            }
        }
    };

    let acc = lease.account.clone();
    let email_label = acc.email.read().unwrap().clone();
    let account_label = format!("codex:{}", email_label);

    let _gen_guard = acc.generation_lock.lock().await;

    if let Err(e) = state.codex_pool.ensure_token(&acc).await {
        lease.commit_error(&e, false);
        return Err(AppError::BadGateway(format!("Token refresh failed: {e}")));
    }

    let scratch_base =
        std::env::var("TMPDIR").unwrap_or_else(|_| "/home/namhv/.hermes/cache/scratch".to_string());
    let run_id = uuid::Uuid::new_v4().simple();
    let run_dir = std::path::PathBuf::from(&scratch_base).join(format!("codex-img-{run_id}"));
    if let Err(e) = tokio::fs::create_dir_all(&run_dir).await {
        lease.release();
        return Err(AppError::Internal(format!(
            "Failed to create run directory: {e}"
        )));
    }

    if let Err(e) = tokio::fs::copy(&acc.auth_path, run_dir.join("auth.json")).await {
        let _ = tokio::fs::remove_dir_all(&run_dir).await;
        lease.release();
        return Err(AppError::Internal(format!(
            "Failed to copy credentials: {e}"
        )));
    }

    let out_file = run_dir.join("output.png");
    let codex_prompt = format!(
        "Dùng tool image_gen tạo 1 ảnh: {prompt}. Lưu file ảnh kết quả vào {}",
        out_file.to_string_lossy()
    );

    let mut cmd = tokio::process::Command::new("codex");
    cmd.args([
        "exec",
        "--skip-git-repo-check",
        "--dangerously-bypass-approvals-and-sandbox",
        &codex_prompt,
    ]);
    cmd.env("CODEX_HOME", &run_dir);
    cmd.current_dir(&run_dir);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let timeout_dur = std::time::Duration::from_secs(240);
    let child_res = tokio::time::timeout(timeout_dur, cmd.output()).await;

    let output = match child_res {
        Ok(Ok(out)) => out,
        Ok(Err(e)) => {
            let _ = tokio::fs::remove_dir_all(&run_dir).await;
            let err_msg = format!("Failed to spawn codex CLI: {e}");
            lease.commit_error(&err_msg, false);
            return Err(AppError::BadGateway(err_msg));
        }
        Err(_) => {
            let _ = tokio::fs::remove_dir_all(&run_dir).await;
            let err_msg = "Codex image generation timed out after 240s".to_string();
            lease.commit_error_with_cooldown(&err_msg, 30.0);
            return Err(AppError::BadGateway(err_msg));
        }
    };

    let resolved_img_path = if out_file.exists()
        && tokio::fs::metadata(&out_file)
            .await
            .map(|m| m.len() > 1000)
            .unwrap_or(false)
    {
        Some(out_file.clone())
    } else {
        find_first_png(&run_dir).await
    };

    let img_path = match resolved_img_path {
        Some(p) => p,
        None => {
            let stdout_snippet = String::from_utf8_lossy(&output.stdout);
            let stderr_snippet = String::from_utf8_lossy(&output.stderr);
            let combined = format!("{stdout_snippet}\n{stderr_snippet}");
            let masked = crate::quota_refresh::sanitize_error_message(&combined);
            let _ = tokio::fs::remove_dir_all(&run_dir).await;
            lease.commit_error(&masked, false);
            return Err(AppError::BadGateway(format!(
                "Codex did not produce an image. Exit code: {}. Output: {}",
                output.status.code().unwrap_or(-1),
                masked.chars().take(300).collect::<String>()
            )));
        }
    };

    let image_bytes = match tokio::fs::read(&img_path).await {
        Ok(b) if !b.is_empty() => b,
        _ => {
            let _ = tokio::fs::remove_dir_all(&run_dir).await;
            lease.commit_error("Generated image file is empty", false);
            return Err(AppError::BadGateway(
                "Generated image file is empty".to_string(),
            ));
        }
    };

    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&image_bytes);
    let data_url = format!("data:image/png;base64,{b64}");

    let _ = tokio::fs::remove_dir_all(&run_dir).await;

    lease.commit_success();

    let duration_ms = start_time.elapsed().as_secs_f64() * 1000.0;
    let _ = state.db.increment_total_requests(&auth.key);
    let _ = state.db.record_request(
        Some(&account_label),
        model,
        timestamp,
        "ok",
        prompt.len() as i64 / 4,
        0,
        duration_ms,
        None,
    );

    let response_body = serde_json::json!({
        "created": timestamp as i64,
        "data": [
            {
                "b64_json": b64,
                "url": data_url,
                "revised_prompt": prompt
            }
        ]
    });

    Ok((StatusCode::OK, Json(response_body)).into_response())
}
