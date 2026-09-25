use axum::{
    extract::{Path, State},
    Json,
};

use crate::auth::AuthenticatedUser;
use crate::error::AppError;
use crate::models::{ModelEntry, ModelListResponse};
use crate::state::AppState;

pub async fn list_models(
    auth: AuthenticatedUser,
    State(state): State<AppState>,
) -> Json<ModelListResponse> {
    let _ = state.db.increment_total_requests(&auth.key);
    let mut resp = state.models.list_models();

    // Dynamically include models from active external providers (e.g. gemini/gemini-2.5-pro)
    if let Ok(providers) = state.db.list_providers() {
        for prov in providers {
            if !prov.is_active {
                continue;
            }
            if let Some(models) = prov.models.as_array() {
                for m in models {
                    if let Some(m_str) = m.as_str() {
                        let id = if m_str.starts_with(&format!("{}/", prov.prefix)) {
                            m_str.to_string()
                        } else {
                            format!("{}/{}", prov.prefix, m_str)
                        };
                        if !resp.data.iter().any(|existing| existing.id == id) {
                            resp.data.push(ModelEntry {
                                id,
                                object: "model".to_string(),
                                created: 1700000000,
                                owned_by: prov.name.clone(),
                            });
                        }
                    }
                }
            }
        }
    }

    // Dynamically include combos from database (e.g. combo-fast, auto-fallback)
    if let Ok(combos) = state.db.list_combos() {
        for combo in combos {
            if !resp.data.iter().any(|existing| existing.id == combo.name) {
                resp.data.push(ModelEntry {
                    id: combo.name,
                    object: "model".to_string(),
                    created: 1700000000,
                    owned_by: "combo".to_string(),
                });
            }
        }
    }

    Json(resp)
}

pub async fn retrieve_model(
    auth: AuthenticatedUser,
    State(state): State<AppState>,
    Path(model_id): Path<String>,
) -> Result<Json<ModelEntry>, AppError> {
    let trimmed_id = model_id.trim_start_matches('/');
    if let Some(model) = state.models.get_model(trimmed_id) {
        let _ = state.db.increment_total_requests(&auth.key);
        return Ok(Json(model.clone()));
    }

    // Check combos
    if let Ok(Some(combo)) = state.db.get_combo_by_name(trimmed_id) {
        let _ = state.db.increment_total_requests(&auth.key);
        return Ok(Json(ModelEntry {
            id: combo.name,
            object: "model".to_string(),
            created: 1700000000,
            owned_by: "combo".to_string(),
        }));
    }

    // Check active external providers if model is in prefix/model format
    if let Some((prefix, sub_model)) = trimmed_id.split_once('/') {
        if let Ok(Some(prov)) = state.db.get_provider_by_prefix(prefix) {
            if prov.is_active {
                if let Some(models) = prov.models.as_array() {
                    let matches = models
                        .iter()
                        .any(|m| m.as_str() == Some(sub_model) || m.as_str() == Some(trimmed_id));
                    if matches {
                        let _ = state.db.increment_total_requests(&auth.key);
                        return Ok(Json(ModelEntry {
                            id: trimmed_id.to_string(),
                            object: "model".to_string(),
                            created: 1700000000,
                            owned_by: prov.name,
                        }));
                    }
                }
            }
        }
    }

    Err(AppError::NotFound(format!(
        "Model '{trimmed_id}' not found"
    )))
}
