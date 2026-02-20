//! Project management configuration.

use serde::{Deserialize, Serialize};

use super::core::default_true;

/// Project management configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
}
