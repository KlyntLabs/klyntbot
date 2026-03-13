//! User profile configuration.

use serde::{Deserialize, Serialize};

/// User profile settings collected during onboarding.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UserConfig {
    #[serde(default)]
    pub name: String,
}
