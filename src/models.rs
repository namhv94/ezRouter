use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::SystemTime;

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
        canonical_id: "ag/claude-opus-5-5-high",
        owned_by: "antigravity",
        aliases: &[
            "claude-opus-5-5-high",
            "ag/claude-opus-5-5",
            "claude-opus-5-5",
            "ag/claude-opus-5.5",
            "claude-opus-5.5",
        ],
    },
    StaticModelDef {
        canonical_id: "ag/claude-opus-5-5-medium",
        owned_by: "antigravity",
        aliases: &["claude-opus-5-5-medium"],
    },
    StaticModelDef {
        canonical_id: "ag/claude-opus-5-5-low",
        owned_by: "antigravity",
        aliases: &["claude-opus-5-5-low"],
    },
    StaticModelDef {
        canonical_id: "ag/claude-sonnet-5-5-high",
        owned_by: "antigravity",
        aliases: &[
            "claude-sonnet-5-5-high",
            "ag/claude-sonnet-5-5",
            "claude-sonnet-5-5",
            "ag/claude-sonnet-5.5",
            "claude-sonnet-5.5",
        ],
    },
    StaticModelDef {
        canonical_id: "ag/claude-sonnet-5-5-medium",
        owned_by: "antigravity",
        aliases: &["claude-sonnet-5-5-medium"],
    },
    StaticModelDef {
        canonical_id: "ag/claude-sonnet-5-5-low",
        owned_by: "antigravity",
        aliases: &["claude-sonnet-5-5-low"],
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
        canonical_id: "cx/gpt-6.1-sol",
        owned_by: "openai-codex",
        aliases: &["gpt-6.1-sol", "gpt-6.1", "cx/gpt-6.1"],
    },
    StaticModelDef {
        canonical_id: "cx/gpt-6-sol",
        owned_by: "openai-codex",
        aliases: &["gpt-6-sol", "gpt-6", "cx/gpt-6"],
    },
    StaticModelDef {
        canonical_id: "cx/gpt-6-luna",
        owned_by: "openai-codex",
        aliases: &["gpt-6-luna"],
    },
    StaticModelDef {
        canonical_id: "cx/gpt-6-astra",
        owned_by: "openai-codex",
        aliases: &["gpt-6-astra", "astra", "cx/astra"],
    },
    StaticModelDef {
        canonical_id: "cx/gpt-5.6-sol",
        owned_by: "openai-codex",
        aliases: &["gpt-5.6-sol", "gpt-5.6", "cx/gpt-5.6"],
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
        canonical_id: "cx/gpt-reserve",
        owned_by: "openai-codex",
        aliases: &["gpt-reserve"],
    },
    StaticModelDef {
        canonical_id: "cx/gpt-5.5",
        owned_by: "openai-codex",
        aliases: &["gpt-5.5"],
    },
    StaticModelDef {
        canonical_id: "cx/codex-auto-review",
        owned_by: "openai-codex",
        aliases: &["codex-auto-review"],
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

struct CodexCacheState {
    last_mtime: Option<SystemTime>,
    cached_models: Vec<ModelEntry>,
}

static CODEX_CACHE: Mutex<Option<CodexCacheState>> = Mutex::new(None);

pub fn resolve_codex_models_cache_path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("CODEX_MODELS_CACHE_PATH") {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }
    if let Ok(codex_home) = std::env::var("CODEX_HOME") {
        let p = PathBuf::from(codex_home).join("models_cache.json");
        if p.exists() {
            return Some(p);
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        let p = PathBuf::from(home).join(".codex/models_cache.json");
        if p.exists() {
            return Some(p);
        }
    }
    None
}

pub fn get_cached_codex_models() -> Vec<ModelEntry> {
    let path = match resolve_codex_models_cache_path() {
        Some(p) => p,
        None => return Vec::new(),
    };

    let current_mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();

    if let Ok(guard) = CODEX_CACHE.lock() {
        if let Some(ref state) = *guard {
            if state.last_mtime.is_some() && state.last_mtime == current_mtime {
                return state.cached_models.clone();
            }
        }
    }

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let parsed: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let mut models = Vec::new();
    if let Some(models_arr) = parsed.get("models").and_then(|m| m.as_array()) {
        for m in models_arr {
            if let Some(slug) = m.get("slug").and_then(|s| s.as_str()) {
                let id = format!("cx/{slug}");
                models.push(ModelEntry {
                    id,
                    object: "model".to_string(),
                    created: DEFAULT_CREATED_TIMESTAMP,
                    owned_by: "openai-codex".to_string(),
                });
            }
        }
    }

    if let Ok(mut guard) = CODEX_CACHE.lock() {
        *guard = Some(CodexCacheState {
            last_mtime: current_mtime,
            cached_models: models.clone(),
        });
    }

    models
}

pub fn is_known_codex_model(target_model: &str) -> bool {
    if target_model.starts_with("cx/") {
        return true;
    }
    let stripped = target_model.strip_prefix("cx/").unwrap_or(target_model);
    let dynamic = get_cached_codex_models();
    dynamic
        .iter()
        .any(|m| m.id == target_model || m.id.strip_prefix("cx/").unwrap_or(&m.id) == stripped)
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
        assert_eq!(list_resp.data.len(), 24);

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
        assert!(ids.contains(&"cx/gpt-6.1-sol"));
        assert!(ids.contains(&"cx/gpt-6-sol"));
        assert!(ids.contains(&"cx/gpt-6-luna"));
        assert!(ids.contains(&"cx/gpt-6-astra"));
        assert!(ids.contains(&"cx/gpt-reserve"));
        assert!(ids.contains(&"cx/codex-auto-review"));
        assert!(ids.contains(&"ag/claude-opus-5-5-high"));
        assert!(ids.contains(&"ag/claude-sonnet-5-5-high"));
        assert!(!ids.contains(&"gemini-3.8-flash-high"));
        assert!(!ids.contains(&"gemini-pro-agent"));
        assert!(!ids.contains(&"ag/gemini-3.1-pro"));
        assert!(!ids.contains(&"ag/gemini-3.1-pro-high"));
        assert!(!ids.contains(&"gemini-3.1-pro"));
        assert!(!ids.contains(&"gemini-3.1-pro-high"));
        assert!(!ids.contains(&"gemini-3.1-pro-low"));
        assert!(!ids.contains(&"gpt-5.6-sol"));
        assert!(!ids.contains(&"gpt-6.1-sol"));
        assert!(!ids.contains(&"gpt-6-sol"));
        assert!(!ids.contains(&"gpt-6-luna"));
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
        assert!(registry.get_model("gpt-5.6").is_some());
        assert!(registry.get_model("cx/gpt-5.6").is_some());

        // Codex GPT-6.1, GPT-6, Astra, Reserve lookups
        let m_61 = registry.get_model("cx/gpt-6.1-sol");
        assert!(m_61.is_some());
        assert_eq!(m_61.unwrap().owned_by, "openai-codex");
        assert!(registry.get_model("gpt-6.1-sol").is_some());
        assert!(registry.get_model("gpt-6.1").is_some());
        assert!(registry.get_model("cx/gpt-6.1").is_some());
        assert!(registry.get_model("cx/gpt-6-sol").is_some());
        assert!(registry.get_model("gpt-6-sol").is_some());
        assert!(registry.get_model("gpt-6").is_some());
        assert!(registry.get_model("cx/gpt-6").is_some());
        assert!(registry.get_model("cx/gpt-6-luna").is_some());
        assert!(registry.get_model("gpt-6-luna").is_some());
        assert!(registry.get_model("cx/gpt-6-astra").is_some());
        assert!(registry.get_model("gpt-6-astra").is_some());
        assert!(registry.get_model("astra").is_some());
        assert!(registry.get_model("cx/astra").is_some());
        assert!(registry.get_model("cx/gpt-reserve").is_some());
        assert!(registry.get_model("gpt-reserve").is_some());
        assert!(registry.get_model("cx/codex-auto-review").is_some());
        assert!(registry.get_model("codex-auto-review").is_some());

        // Claude 5.5 lookups
        let m_opus55 = registry.get_model("ag/claude-opus-5-5-high");
        assert!(m_opus55.is_some());
        assert_eq!(m_opus55.unwrap().owned_by, "antigravity");
        assert!(registry.get_model("claude-opus-5-5-high").is_some());
        assert!(registry.get_model("ag/claude-opus-5-5").is_some());
        assert!(registry.get_model("claude-opus-5-5").is_some());
        assert!(registry.get_model("ag/claude-opus-5.5").is_some());
        assert!(registry.get_model("claude-opus-5.5").is_some());
        assert!(registry.get_model("ag/claude-opus-5-5-medium").is_some());
        assert!(registry.get_model("claude-opus-5-5-medium").is_some());
        assert!(registry.get_model("ag/claude-opus-5-5-low").is_some());
        assert!(registry.get_model("claude-opus-5-5-low").is_some());

        let m_sonnet55 = registry.get_model("ag/claude-sonnet-5-5-high");
        assert!(m_sonnet55.is_some());
        assert_eq!(m_sonnet55.unwrap().owned_by, "antigravity");
        assert!(registry.get_model("claude-sonnet-5-5-high").is_some());
        assert!(registry.get_model("ag/claude-sonnet-5-5").is_some());
        assert!(registry.get_model("claude-sonnet-5-5").is_some());
        assert!(registry.get_model("ag/claude-sonnet-5.5").is_some());
        assert!(registry.get_model("claude-sonnet-5.5").is_some());
        assert!(registry.get_model("ag/claude-sonnet-5-5-medium").is_some());
        assert!(registry.get_model("claude-sonnet-5-5-medium").is_some());
        assert!(registry.get_model("ag/claude-sonnet-5-5-low").is_some());
        assert!(registry.get_model("claude-sonnet-5-5-low").is_some());

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

    #[test]
    fn test_dynamic_codex_models_from_cache_and_passthrough() {
        let temp_dir =
            std::env::temp_dir().join(format!("test_codex_cache_{}", uuid::Uuid::new_v4()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let cache_file = temp_dir.join("models_cache.json");

        let json_payload = serde_json::json!({
            "models": [
                {
                    "slug": "gpt-future-v1",
                    "display_name": "GPT Future V1",
                    "description": "Next gen model"
                },
                {
                    "slug": "gpt-omni-coding",
                    "display_name": "Omni Coding",
                    "description": "Specialized coding"
                }
            ]
        });

        std::fs::write(&cache_file, serde_json::to_string(&json_payload).unwrap()).unwrap();
        std::env::set_var("CODEX_MODELS_CACHE_PATH", cache_file.to_str().unwrap());

        let cached = get_cached_codex_models();
        assert!(cached.iter().any(|m| m.id == "cx/gpt-future-v1"));
        assert!(cached.iter().any(|m| m.id == "cx/gpt-omni-coding"));

        // is_known_codex_model tests
        assert!(is_known_codex_model("cx/gpt-future-v1"));
        assert!(is_known_codex_model("gpt-future-v1"));
        assert!(is_known_codex_model("cx/gpt-omni-coding"));
        assert!(is_known_codex_model("gpt-omni-coding"));
        // Passthrough for any cx/*
        assert!(is_known_codex_model("cx/any-brand-new-model"));
        // Unknown non-cx model is false
        assert!(!is_known_codex_model("nonexistent-unprefixed-model"));

        std::env::remove_var("CODEX_MODELS_CACHE_PATH");
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
