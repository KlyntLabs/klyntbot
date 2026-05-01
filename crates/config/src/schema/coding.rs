use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CodingConfig {
    #[serde(default)]
    pub permissions: CodingPermissions,
    #[serde(default)]
    pub sandbox: CodingSandbox,
    #[serde(default)]
    pub skills: super::coding_memory::CodingSkillsConfig,
    #[serde(default)]
    pub sessions: CodingSessionsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodingPermissions {
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default)]
    pub deny: Vec<String>,
    #[serde(default)]
    pub ask: Vec<String>,
    #[serde(default = "default_match")]
    pub default_if_no_match: String,
    #[serde(default)]
    pub mirror_learning: bool,
}
fn default_match() -> String {
    "ask".into()
}
impl Default for CodingPermissions {
    fn default() -> Self {
        Self {
            allow: vec![],
            deny: vec![],
            ask: vec![],
            default_if_no_match: "ask".into(),
            mirror_learning: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodingSandbox {
    #[serde(default = "default_true")]
    pub enforce: bool,
}
fn default_true() -> bool {
    true
}
impl Default for CodingSandbox {
    fn default() -> Self {
        Self { enforce: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodingSessionsConfig {
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,
    #[serde(default = "default_max_total_disk_mb")]
    pub max_total_disk_mb: u32,
    #[serde(default = "default_true")]
    pub preserve_starred: bool,
}

fn default_retention_days() -> u32 {
    90
}
fn default_max_total_disk_mb() -> u32 {
    5120
}

impl Default for CodingSessionsConfig {
    fn default() -> Self {
        Self {
            retention_days: default_retention_days(),
            max_total_disk_mb: default_max_total_disk_mb(),
            preserve_starred: true,
        }
    }
}
