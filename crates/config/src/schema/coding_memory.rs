//! Configuration for the coding-memory subsystem.

use serde::{Deserialize, Serialize};

/// Root config for coding memory.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CodingMemoryConfig {
    pub enabled: bool,
    pub distiller: CodingDistillerConfig,
    pub ingest: CodingIngestConfig,
    pub privacy: CodingPrivacyConfig,
    pub recall: CodingRecallConfig,
    pub reforge: CodingReforgeConfig,
    pub skills: CodingSkillsConfig,
    pub workbench: CodingWorkbenchConfig,
    pub cli: CodingCliToggles,
}

/// Distiller config.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CodingDistillerConfig {
    pub model: String,
    pub max_input_tokens: u32,
    pub timeout: String,
}

impl Default for CodingDistillerConfig {
    fn default() -> Self {
        Self {
            model: "claude-haiku-4-5-20251001".to_string(),
            max_input_tokens: 8000,
            timeout: "30s".to_string(),
        }
    }
}

/// Ingest-side config.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CodingIngestConfig {
    pub exclude_paths: Vec<String>,
}

impl Default for CodingIngestConfig {
    fn default() -> Self {
        Self {
            exclude_paths: default_exclude_paths(),
        }
    }
}

#[must_use]
pub fn default_exclude_paths() -> Vec<String> {
    [
        "**/.env",
        "**/.env.*",
        "**/secrets/**",
        "**/private/**",
        "**/*.key",
        "**/*.pem",
        "**/*.p12",
        "**/*.pfx",
        "**/id_rsa",
        "**/id_ed25519",
        "**/known_hosts",
        "**/.aws/credentials",
        "**/.gcloud/**",
        "**/.kube/config",
        "**/node_modules/**",
        "**/target/**",
        "**/.git/**",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect()
}

/// Privacy config.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CodingPrivacyConfig {
    pub default_sensitivity: String,
    pub auto_promote_high_paths: Vec<String>,
}

impl Default for CodingPrivacyConfig {
    fn default() -> Self {
        Self {
            default_sensitivity: "normal".to_string(),
            auto_promote_high_paths: vec![
                "**/auth/**".to_string(),
                "**/billing/**".to_string(),
                "**/payment/**".to_string(),
            ],
        }
    }
}

/// Recall API config.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CodingRecallConfig {
    pub session_start_budget: u32,
    pub user_prompt_budget: u32,
    pub dead_end_warnings: bool,
    pub escalation_enabled: bool,
    pub coverage_threshold: f32,
}

impl Default for CodingRecallConfig {
    fn default() -> Self {
        Self {
            session_start_budget: 800,
            user_prompt_budget: 1500,
            dead_end_warnings: true,
            escalation_enabled: true,
            coverage_threshold: 0.25,
        }
    }
}

/// Reforge config.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CodingReforgeConfig {
    pub nightly_cron: String,
    pub rule_artifacts: CodingRuleArtifactsConfig,
    /// Optional override model for Phase 2.5 synthesis.
    pub synth_model: Option<String>,
    /// Optional override model for Phase 3.5 rule artifacts.
    pub rules_model: Option<String>,
    /// Vector-similarity threshold for cross-session fact dedup (Phase 6.5).
    #[serde(default = "default_cross_session_dedup_threshold")]
    pub cross_session_dedup_threshold: f32,
    /// Minimum never-cited retrieval count before selective-delete signal (Phase 6).
    #[serde(default = "default_selective_delete_threshold")]
    pub selective_delete_threshold: u32,
}

impl Default for CodingReforgeConfig {
    fn default() -> Self {
        Self {
            nightly_cron: "0 3 * * *".to_string(),
            rule_artifacts: CodingRuleArtifactsConfig::default(),
            synth_model: None,
            rules_model: None,
            cross_session_dedup_threshold: default_cross_session_dedup_threshold(),
            selective_delete_threshold: default_selective_delete_threshold(),
        }
    }
}

fn default_cross_session_dedup_threshold() -> f32 {
    0.92
}

fn default_selective_delete_threshold() -> u32 {
    5
}

/// Which rule artifacts Reforge should write.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CodingRuleArtifactsConfig {
    pub claude_md: bool,
    pub agents_md: bool,
    pub cursorrules: bool,
    pub continue_rules: bool,
}

impl Default for CodingRuleArtifactsConfig {
    fn default() -> Self {
        Self {
            claude_md: true,
            agents_md: true,
            cursorrules: true,
            continue_rules: true,
        }
    }
}

/// Skill config.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CodingSkillsConfig {
    pub project_skills: bool,
    pub location: String,
}

impl Default for CodingSkillsConfig {
    fn default() -> Self {
        Self {
            project_skills: true,
            location: "private".to_string(),
        }
    }
}

/// Workbench config.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CodingWorkbenchConfig {
    pub enabled: bool,
    pub session_replay_page_size: u32,
    pub causal_graph_max_nodes: u32,
}

impl Default for CodingWorkbenchConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            session_replay_page_size: 500,
            causal_graph_max_nodes: 200,
        }
    }
}

/// Per-CLI toggles.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CodingCliToggles {
    pub claude_code: CodingCliEntry,
    pub codex: CodingCliEntry,
    pub kimi_cli: CodingCliEntry,
    pub opencode: CodingCliEntry,
}

/// One CLI's toggle entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CodingCliEntry {
    pub enabled: bool,
}

impl Default for CodingCliEntry {
    fn default() -> Self {
        // Default to enabled so the hook is installed on first run without
        // requiring the user to toggle it manually.
        Self { enabled: true }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_round_trips_through_serde() {
        let cfg = CodingMemoryConfig::default();
        let json = serde_json::to_string(&cfg).unwrap();
        let parsed: CodingMemoryConfig = serde_json::from_str(&json).unwrap();
        assert!(!parsed.enabled);
        assert_eq!(parsed.recall.session_start_budget, 800);
        assert_eq!(parsed.recall.user_prompt_budget, 1500);
        assert!(parsed.ingest.exclude_paths.contains(&"**/.env".to_string()));
    }

    #[test]
    fn coding_memory_defaults_match_legacy_hardcoded_values() {
        let cfg = CodingMemoryConfig::default();
        assert_eq!(cfg.distiller.model, "claude-haiku-4-5-20251001");
        assert!((cfg.reforge.cross_session_dedup_threshold - 0.92).abs() < 1e-6);
        assert_eq!(cfg.reforge.selective_delete_threshold, 5);
    }
}
