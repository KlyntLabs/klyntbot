use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    Github,
    SkillsSh,
    Local,
    Bundled,
}

impl SourceType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Github => "github",
            Self::SkillsSh => "skills_sh",
            Self::Local => "local",
            Self::Bundled => "bundled",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledSkill {
    pub name: String,
    pub source_type: SourceType,
    pub source_ref: String,
    pub installed_version: String,
    pub installed_sha: String,
    pub enabled: bool,
    pub is_adapted: bool,
    pub bootstrapped_databases: Vec<String>,
    pub installed_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdaptedSkillRow {
    pub cache_key: String,
    pub adapted_skill_md: String,
    pub generated_templates: serde_json::Value, // parsed from stored JSON string
    pub rationale: String,
    pub adapter_model: String,
    pub created_at: String,
}
