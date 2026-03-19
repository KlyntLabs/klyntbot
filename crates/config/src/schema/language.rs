use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageConfig {
    /// Source language for translation (e.g., "zh", "ja", "en")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_lang: Option<String>,

    /// Target language for translation (e.g., "en", "vi")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_lang: Option<String>,

    /// Auto-detect source language when not configured
    #[serde(default = "super::core::default_true")]
    pub auto_detect: bool,

    /// User's proficiency level (e.g., "HSK 3", "CEFR B1")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proficiency_level: Option<String>,
}

impl Default for LanguageConfig {
    fn default() -> Self {
        Self {
            source_lang: None,
            target_lang: None,
            auto_detect: true,
            proficiency_level: None,
        }
    }
}
