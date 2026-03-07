use serde::{Deserialize, Serialize};

/// Configuration for background cognitive tasks (extraction, consolidation,
/// reflection, coaching reasoning).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CognitiveConfig {
    /// Model for cognitive LLM calls. Falls back to agents.defaults.model if unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Provider name override. Falls back to agents.defaults.provider if unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,

    /// Temperature for cognitive calls (default: 0.2 — low creativity, high consistency).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Max tokens per cognitive call (default: 1024).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,

    /// Max tokens for reflection calls (default: 2048).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reflection_max_tokens: Option<u32>,

    /// Cron expression for weekly reflection (default: "0 9 * * 1" — Monday 9am).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reflection_schedule: Option<String>,
}
