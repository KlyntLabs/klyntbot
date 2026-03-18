use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ShortcutsConfig {
    pub launcher: String,
    pub tray: String,
    pub quick_capture: String,
}

impl Default for ShortcutsConfig {
    fn default() -> Self {
        Self {
            launcher: "alt+space".to_string(),
            tray: "alt+shift+space".to_string(),
            quick_capture: "super+shift+c".to_string(),
        }
    }
}
