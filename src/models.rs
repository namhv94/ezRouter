use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelEntry {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub owned_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelListResponse {
    pub object: String,
    pub data: Vec<ModelEntry>,
}

/// Registry of the models this router can resolve.
///
/// The built-in catalog is intentionally empty: every routable model comes
/// from a configured external provider (`{prefix}/{upstream_id}`) or a combo,
/// both of which are enumerated dynamically from the database.
#[derive(Debug, Clone, Default)]
pub struct ModelRegistry {
    canonical_list: Vec<ModelEntry>,
    lookup_map: HashMap<String, ModelEntry>,
}

impl ModelRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn list_models(&self) -> ModelListResponse {
        ModelListResponse {
            object: "list".to_string(),
            data: self.canonical_list.clone(),
        }
    }

    pub fn get_model(&self, model_id: &str) -> Option<&ModelEntry> {
        self.lookup_map.get(model_id)
    }
}
