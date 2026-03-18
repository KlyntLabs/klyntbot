//! Persona skill metadata parsing — extracts persona-specific fields from PERSONA.md frontmatter.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaSkillMetadata {
    pub expertise_areas: Vec<String>,
    pub analysis_frameworks: Vec<String>,
    pub questioning_style: String,
    pub tone: String,
    pub cognitive_bias: String,
    #[serde(default)]
    pub references: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ParsedPersonaSkill {
    pub name: String,
    pub description: String,
    pub version: String,
    pub icon: String,
    pub domains: Vec<String>,
    pub metadata: PersonaSkillMetadata,
    pub body: String,
}

/// Intermediate struct for YAML frontmatter deserialization.
#[derive(Deserialize)]
struct PersonaFrontmatter {
    name: String,
    description: String,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    icon: Option<String>,
    #[serde(default)]
    domains: Vec<String>,
    #[serde(default)]
    #[allow(dead_code)]
    persona_only: Option<bool>,
    metadata: PersonaMetadataBlock,
}

#[derive(Deserialize)]
struct PersonaMetadataBlock {
    #[serde(default)]
    expertise_areas: Vec<String>,
    #[serde(default)]
    analysis_frameworks: Vec<String>,
    #[serde(default = "default_questioning_style")]
    questioning_style: String,
    #[serde(default = "default_tone")]
    tone: String,
    #[serde(default = "default_cognitive_bias")]
    cognitive_bias: String,
    #[serde(default)]
    references: Vec<String>,
}

fn default_questioning_style() -> String {
    "analytical".to_string()
}
fn default_tone() -> String {
    "neutral".to_string()
}
fn default_cognitive_bias() -> String {
    "balanced".to_string()
}

/// Parse a PERSONA.md file into a `ParsedPersonaSkill`.
pub fn parse_persona_skill(content: &str) -> common::Result<ParsedPersonaSkill> {
    let (yaml_str, body) = crate::parser::split_frontmatter(content)?;

    if yaml_str.is_empty() {
        return Err(common::ConfigError::Invalid("Missing frontmatter delimiter".into()).into());
    }

    let fm: PersonaFrontmatter = serde_yaml::from_str(&yaml_str)
        .map_err(|e| common::ConfigError::Invalid(format!("YAML parse error: {e}")))?;

    Ok(ParsedPersonaSkill {
        name: fm.name,
        description: fm.description,
        version: fm.version.unwrap_or_else(|| "1.0.0".to_string()),
        icon: fm.icon.unwrap_or_default(),
        domains: fm.domains,
        metadata: PersonaSkillMetadata {
            expertise_areas: fm.metadata.expertise_areas,
            analysis_frameworks: fm.metadata.analysis_frameworks,
            questioning_style: fm.metadata.questioning_style,
            tone: fm.metadata.tone,
            cognitive_bias: fm.metadata.cognitive_bias,
            references: fm.metadata.references,
        },
        body: body.trim().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_PERSONA: &str = r#"---
name: deep-analyst
description: >
  Rigorous financial analyst specializing in DCF valuation.
persona_only: true
version: "1.0.0"
icon: "\U0001F4CA"
domains: [finance, productivity]
metadata:
  expertise_areas:
    - DCF valuation
    - ratio analysis
  analysis_frameworks:
    - bottom-up
    - comparative
  questioning_style: interrogative
  tone: rigorous
  cognitive_bias: precision
  references:
    - dcf-guide
---

You are a rigorous financial analyst.
"#;

    #[test]
    fn test_parse_persona_skill() {
        let result = parse_persona_skill(SAMPLE_PERSONA).unwrap();
        assert_eq!(result.name, "deep-analyst");
        assert_eq!(result.domains, vec!["finance", "productivity"]);
        assert_eq!(
            result.metadata.expertise_areas,
            vec!["DCF valuation", "ratio analysis"]
        );
        assert_eq!(result.metadata.questioning_style, "interrogative");
        assert_eq!(result.metadata.tone, "rigorous");
        assert!(result.body.contains("rigorous financial analyst"));
    }

    #[test]
    fn test_persona_skill_ignores_tool_fields() {
        let md = r#"---
name: test
description: Test persona
persona_only: true
metadata:
  expertise_areas: [testing]
  questioning_style: direct
  tone: neutral
  cognitive_bias: balanced
---
Body text.
"#;
        let result = parse_persona_skill(md).unwrap();
        assert_eq!(result.metadata.expertise_areas, vec!["testing"]);
    }
}
