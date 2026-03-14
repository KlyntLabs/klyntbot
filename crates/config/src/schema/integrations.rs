use serde::{Deserialize, Serialize};

/// Configuration for AI coding tool integrations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationsConfig {
    /// Which AI tools the user has connected (e.g., ["claude-code", "cursor"]).
    #[serde(default)]
    pub ai_tools: Vec<String>,
}

impl Default for IntegrationsConfig {
    fn default() -> Self {
        Self {
            ai_tools: Vec::new(),
        }
    }
}
