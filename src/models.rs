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

#[derive(Debug, Clone)]
struct StaticModelDef {
    pub canonical_id: &'static str,
    pub owned_by: &'static str,
    pub aliases: &'static [&'static str],
}

const STATIC_MODELS: &[StaticModelDef] = &[
    StaticModelDef {
        canonical_id: "ag/gemini-3.8-flash-high",
        owned_by: "antigravity",
        aliases: &[
            "gemini-3.8-flash-high",
            "ag/gemini-3.8-flash",
            "gemini-3.8-flash",
        ],
    },
    StaticModelDef {
        canonical_id: "ag/gemini-3.8-flash-low",
        owned_by: "antigravity",
        aliases: &["gemini-3.8-flash-low"],
    },
    StaticModelDef {
        canonical_id: "ag/gemini-3.7-flash-tiered",
        owned_by: "antigravity",
        aliases: &["gemini-3.7-flash-tiered"],
    },
    StaticModelDef {
        canonical_id: "ag/gemini-3.7-flash-low",
        owned_by: "antigravity",
        aliases: &["gemini-3.7-flash-low"],
    },
    StaticModelDef {
        canonical_id: "ag/claude-sonnet-4-6",
        owned_by: "antigravity",
        aliases: &["claude-sonnet-4-6"],
    },
    StaticModelDef {
        canonical_id: "ag/claude-opus-4-6-thinking",
        owned_by: "antigravity",
        aliases: &["claude-opus-4-6-thinking"],
    },
    StaticModelDef {
        canonical_id: "ag/gemini-pro-agent",
        owned_by: "antigravity",
        aliases: &[
            "gemini-pro-agent",
            "ag/gemini-3.1-pro",
            "gemini-3.1-pro",
            "ag/gemini-3.1-pro-high",
            "gemini-3.1-pro-high",
        ],
    },
    StaticModelDef {
        canonical_id: "ag/gemini-3.1-pro-low",
        owned_by: "antigravity",
        aliases: &["gemini-3.1-pro-low", "ag/gemini-pro-low", "gemini-pro-low"],
    },
    StaticModelDef {
        canonical_id: "cx/gpt-5.6-sol",
        owned_by: "openai-codex",
        aliases: &["gpt-5.6-sol"],
    },
    StaticModelDef {
        canonical_id: "cx/gpt-5.6-terra",
        owned_by: "openai-codex",
        aliases: &["gpt-5.6-terra"],
    },
    StaticModelDef {
        canonical_id: "cx/gpt-5.6-luna",
        owned_by: "openai-codex",
        aliases: &["gpt-5.6-luna"],
    },
    StaticModelDef {
        canonical_id: "cx/gpt-5.5",
        owned_by: "openai-codex",
        aliases: &["gpt-5.5"],
    },
    StaticModelDef {
        canonical_id: "cx/gpt-6-astra",
        owned_by: "openai-codex",
        aliases: &["gpt-6-astra", "astra"],
    },
];

const DEFAULT_CREATED_TIMESTAMP: i64 = 1700000000;

#[derive(Debug, Clone)]
pub struct ModelRegistry {
    canonical_list: Vec<ModelEntry>,
    lookup_map: HashMap<String, ModelEntry>,
}

impl Default for ModelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelRegistry {
    pub fn new() -> Self {
        let mut canonical_list = Vec::with_capacity(STATIC_MODELS.len());
        let mut lookup_map = HashMap::new();

        for def in STATIC_MODELS {
            let canonical_entry = ModelEntry {
                id: def.canonical_id.to_string(),
                object: "model".to_string(),
                created: DEFAULT_CREATED_TIMESTAMP,
                owned_by: def.owned_by.to_string(),
            };

            canonical_list.push(canonical_entry.clone());
            lookup_map.insert(def.canonical_id.to_string(), canonical_entry);

            for alias in def.aliases {
                lookup_map.insert(
                    alias.to_string(),
                    ModelEntry {
                        id: alias.to_string(),
                        object: "model".to_string(),
                        created: DEFAULT_CREATED_TIMESTAMP,
                        owned_by: def.owned_by.to_string(),
                    },
                );
            }
        }

        Self {
            canonical_list,
            lookup_map,
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_canonical_models_uniqueness_and_prefixes() {
        let registry = ModelRegistry::new();
        let list_resp = registry.list_models();

        assert_eq!(list_resp.object, "list");
        assert_eq!(list_resp.data.len(), 13);

        let mut seen_ids = HashSet::new();
        for entry in &list_resp.data {
            assert!(
                seen_ids.insert(entry.id.clone()),
                "duplicate model id found: {}",
                entry.id
            );
            assert_eq!(entry.object, "model");
            assert_eq!(entry.created, 1700000000);
            assert!(
                entry.id.starts_with("ag/") || entry.id.starts_with("cx/"),
                "Model {} does not have canonical ag/ or cx/ prefix",
                entry.id
            );

            if entry.id.starts_with("ag/") {
                assert_eq!(entry.owned_by, "antigravity");
            } else if entry.id.starts_with("cx/") {
                assert_eq!(entry.owned_by, "openai-codex");
            }
        }

        // Specifically check the models required by test_models.py
        let ids: Vec<&str> = list_resp.data.iter().map(|m| m.id.as_str()).collect();
        assert!(ids.contains(&"ag/gemini-3.8-flash-high"));
        assert!(ids.contains(&"ag/gemini-pro-agent"));
        assert!(ids.contains(&"ag/gemini-3.1-pro-low"));
        assert!(ids.contains(&"cx/gpt-5.6-sol"));
        assert!(ids.contains(&"cx/gpt-6-astra"));
        assert!(!ids.contains(&"gemini-3.8-flash-high"));
        assert!(!ids.contains(&"gemini-pro-agent"));
        assert!(!ids.contains(&"ag/gemini-3.1-pro"));
        assert!(!ids.contains(&"ag/gemini-3.1-pro-high"));
        assert!(!ids.contains(&"gemini-3.1-pro"));
        assert!(!ids.contains(&"gemini-3.1-pro-high"));
        assert!(!ids.contains(&"gemini-3.1-pro-low"));
        assert!(!ids.contains(&"gpt-5.6-sol"));
        assert!(!ids.contains(&"ag/gemini-3.8-flash"));
        assert!(!ids.contains(&"gemini-3.8-flash"));
        // Commercial Gemini Pro names are intentionally omitted from static catalog (served via external provider gemini/*).
        assert!(!ids.contains(&"ag/gemini-pro"));
        assert!(!ids.contains(&"ag/gemini-2.5-pro"));
        assert!(!ids.contains(&"ag/gemini-1.5-pro"));
    }

    #[test]
    fn test_lookup_model() {
        let registry = ModelRegistry::new();

        // Canonical ID lookup
        let model = registry.get_model("ag/gemini-3.8-flash-high");
        assert!(model.is_some());
        assert_eq!(model.unwrap().owned_by, "antigravity");

        let codex_model = registry.get_model("cx/gpt-5.6-sol");
        assert!(codex_model.is_some());
        assert_eq!(codex_model.unwrap().owned_by, "openai-codex");

        // Alias lookup
        let alias_model = registry.get_model("gpt-5.6-sol");
        assert!(alias_model.is_some());
        assert_eq!(alias_model.unwrap().owned_by, "openai-codex");

        // Gemini Pro agent canonical and alias lookup
        let m_agent = registry.get_model("ag/gemini-pro-agent");
        assert!(m_agent.is_some());
        assert_eq!(m_agent.unwrap().owned_by, "antigravity");

        let m_low = registry.get_model("ag/gemini-3.1-pro-low");
        assert!(m_low.is_some());
        assert_eq!(m_low.unwrap().owned_by, "antigravity");

        assert!(registry.get_model("gemini-pro-agent").is_some());
        assert!(registry.get_model("ag/gemini-3.1-pro").is_some());
        assert!(registry.get_model("gemini-3.1-pro").is_some());
        assert!(registry.get_model("ag/gemini-3.1-pro-high").is_some());
        assert!(registry.get_model("gemini-3.1-pro-high").is_some());
        assert!(registry.get_model("gemini-3.1-pro-low").is_some());
        assert!(registry.get_model("ag/gemini-pro-low").is_some());
        assert!(registry.get_model("gemini-pro-low").is_some());

        // Unknown / unsupported models return None
        assert!(registry.get_model("non-existent-model").is_none());
        assert!(registry.get_model("ag/unknown-gemini").is_none());
        assert!(registry.get_model("ag/gemini-pro").is_none());
        assert!(registry.get_model("ag/gemini-2.5-pro").is_none());
        assert!(registry.get_model("gemini-pro").is_none());
    }
}
