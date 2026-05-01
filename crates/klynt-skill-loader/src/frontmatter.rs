use common::{ConfigError, KlyntbotError, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct KlyntFrontmatter {
    pub name: String,
    pub description: String,
    #[serde(rename = "allowed-tools")]
    pub allowed_tools: Vec<String>,
    pub paths: Vec<String>,
    pub tags: Vec<String>,
    pub sensitivity: Option<String>,
    pub references: Vec<Reference>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Reference {
    pub name: String,
    pub file: String,
    #[serde(default)]
    pub load: ReferenceLoadMode,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum ReferenceLoadMode {
    Always,
    #[default]
    OnDemand,
}

impl<'de> serde::Deserialize<'de> for ReferenceLoadMode {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(match s.as_str() {
            "always" => ReferenceLoadMode::Always,
            _ => ReferenceLoadMode::OnDemand,
        })
    }
}

impl KlyntFrontmatter {
    pub fn parse(raw: &str) -> Result<(Self, String)> {
        let (yaml, body) = skill_system::parser::split_frontmatter(raw).map_err(|e| {
            KlyntbotError::Config(ConfigError::Invalid(format!(
                "SKILL.md frontmatter split failed: {e}"
            )))
        })?;
        if yaml.is_empty() {
            return Err(KlyntbotError::Config(ConfigError::Invalid(
                "SKILL.md missing frontmatter fence".into(),
            )));
        }

        let mut fm: KlyntFrontmatter = serde_yaml::from_str(&yaml).map_err(|e| {
            KlyntbotError::Config(ConfigError::Invalid(format!(
                "invalid SKILL.md frontmatter: {e}"
            )))
        })?;
        if fm.name.trim().is_empty() {
            return Err(KlyntbotError::Config(ConfigError::Invalid(
                "SKILL.md frontmatter missing required `name`".into(),
            )));
        }
        if fm.description.trim().is_empty() {
            return Err(KlyntbotError::Config(ConfigError::Invalid(
                "SKILL.md frontmatter missing required `description`".into(),
            )));
        }
        for r in &mut fm.references {
            if r.name.trim().is_empty() || r.file.trim().is_empty() {
                return Err(KlyntbotError::Config(ConfigError::Invalid(format!(
                    "SKILL.md reference '{}' missing name or file",
                    r.name
                ))));
            }
        }
        Ok((fm, body))
    }
}
