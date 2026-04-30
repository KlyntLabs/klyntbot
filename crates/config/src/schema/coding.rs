use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CodingConfig {
    #[serde(default)]
    pub permissions: CodingPermissions,
    #[serde(default)]
    pub sandbox: CodingSandbox,
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
