use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;

use serde::Deserialize;

use crate::types::{KlyntbotMeta, SkillMetadata, SkillPackage, SkillScope, SkillType};

#[derive(Deserialize)]
struct RawFrontmatter {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    license: Option<String>,
    #[serde(default)]
    compatibility: Option<String>,
    #[serde(default)]
    metadata: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Deserialize, Default)]
struct RawKlyntbotMeta {
    #[serde(default, rename = "type")]
    skill_type: Option<String>,
    #[serde(default)]
    tools: Option<Vec<String>>,
    #[serde(default)]
    mcp_tools: Vec<String>,
    #[serde(default)]
    can_delegate_to: Vec<String>,
    #[serde(default)]
    max_iterations: Option<u32>,
    #[serde(default)]
    always_skills: Vec<String>,
    #[serde(default)]
    invokes: Vec<String>,
    #[serde(default)]
    triggers: Vec<String>,
    #[serde(default)]
    summary: Option<String>,
}

pub fn parse_skill_md(
    content: &str,
    location: PathBuf,
    scope: SkillScope,
) -> common::Result<SkillPackage> {
    let (frontmatter_str, body) = split_frontmatter(content)?;

    // Lenient YAML parsing: try strict first, fallback to fixing common issues
    let raw: RawFrontmatter = match serde_yaml::from_str(&frontmatter_str) {
        Ok(r) => r,
        Err(e) => {
            // Fallback: try fixing unquoted colons in values (common cross-client issue)
            let fixed = fix_yaml_colons(&frontmatter_str);
            serde_yaml::from_str(&fixed).map_err(|_| {
                // Report original error, not the fixup error
                common::ConfigError::Invalid(format!("SKILL.md frontmatter: {e}"))
            })?
        }
    };

    if raw.description.is_empty() {
        return Err(common::ConfigError::Invalid(format!(
            "Skill '{}' has no description (required by Agent Skills spec)",
            raw.name
        ))
        .into());
    }

    // Lenient name validation: warn but load anyway (per spec recommendation)
    validate_skill_name(&raw.name, &location);

    // Parse metadata block
    let (klyntbot_meta, custom) = parse_metadata_block(raw.metadata);

    let skill_type = klyntbot_meta
        .as_ref()
        .map(|k| k.skill_type)
        .unwrap_or_default();

    let trimmed_body = body.trim().to_string();
    let summary = klyntbot_meta
        .as_ref()
        .and_then(|k| k.summary.clone())
        .unwrap_or_else(|| extract_first_sentence(&trimmed_body));

    Ok(SkillPackage {
        name: raw.name,
        description: raw.description,
        skill_type,
        scope,
        location,
        body: trimmed_body,
        metadata: SkillMetadata {
            license: raw.license,
            compatibility: raw.compatibility,
            custom,
            klyntbot: klyntbot_meta,
        },
        resources: Vec::new(),
        loaded_at: SystemTime::now(),
        trusted: matches!(scope, SkillScope::BuiltIn | SkillScope::User),
        summary,
    })
}

/// Validate skill name per Agent Skills spec. Warns but never errors.
fn validate_skill_name(name: &str, location: &std::path::Path) {
    if name.len() > 64 {
        tracing::warn!(
            skill = %name,
            "Skill name exceeds 64 characters (spec violation, loading anyway)"
        );
    }
    if name.starts_with('-') || name.ends_with('-') {
        tracing::warn!(
            skill = %name,
            "Skill name starts or ends with hyphen (spec violation, loading anyway)"
        );
    }
    if name.contains("--") {
        tracing::warn!(
            skill = %name,
            "Skill name contains consecutive hyphens (spec violation, loading anyway)"
        );
    }
    if name.chars().any(|c| c.is_uppercase()) {
        tracing::warn!(
            skill = %name,
            "Skill name contains uppercase characters (spec violation, loading anyway)"
        );
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        tracing::warn!(
            skill = %name,
            "Skill name contains invalid characters (spec allows lowercase alphanumeric + hyphens only)"
        );
    }
    // Check name matches directory name (lenient: warn only)
    if let Some(dir_name) = location.file_name().and_then(|n| n.to_str()) {
        // Skip for built-in (location is "builtin::name") and inline ("inline::name")
        if !dir_name.starts_with("builtin::")
            && !dir_name.starts_with("inline::")
            && dir_name != name
        {
            tracing::warn!(
                skill = %name,
                dir = %dir_name,
                "Skill name doesn't match directory name (spec recommends matching)"
            );
        }
    }
}

/// Fix common YAML issues for cross-client compatibility.
/// Handles unquoted colons in string values (the most common issue).
fn fix_yaml_colons(yaml: &str) -> String {
    yaml.lines()
        .map(|line| {
            let trimmed = line.trim();
            // Skip lines that are already structured YAML (arrays, nested objects, comments)
            if trimmed.starts_with('-')
                || trimmed.starts_with('#')
                || trimmed.is_empty()
                || trimmed.ends_with(':')
            {
                return line.to_string();
            }
            // For simple "key: value" lines where value contains a colon
            if let Some(first_colon) = trimmed.find(':') {
                let key = &trimmed[..first_colon];
                let value = trimmed[first_colon + 1..].trim();
                // If the value part contains another colon and isn't already quoted
                if value.contains(':')
                    && !value.starts_with('"')
                    && !value.starts_with('\'')
                    && !value.starts_with('>')
                    && !value.starts_with('|')
                    && !value.starts_with('[')
                    && !value.starts_with('{')
                {
                    let indent = &line[..line.len() - line.trim_start().len()];
                    return format!("{indent}{key}: \"{value}\"");
                }
            }
            line.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn parse_metadata_block(
    raw: Option<HashMap<String, serde_json::Value>>,
) -> (Option<KlyntbotMeta>, HashMap<String, serde_json::Value>) {
    let Some(mut map) = raw else {
        return (None, HashMap::new());
    };

    let klyntbot = map.remove("klyntbot").and_then(|v| {
        let raw_km: RawKlyntbotMeta = serde_json::from_value(v).ok()?;
        let skill_type = match raw_km.skill_type.as_deref() {
            Some("orchestrator") => SkillType::Orchestrator,
            _ => SkillType::Skill,
        };
        Some(KlyntbotMeta {
            skill_type,
            tools: raw_km.tools,
            mcp_tools: raw_km.mcp_tools,
            can_delegate_to: raw_km.can_delegate_to,
            max_iterations: raw_km.max_iterations,
            always_skills: raw_km.always_skills,
            invokes: raw_km.invokes,
            triggers: raw_km.triggers,
            summary: raw_km.summary,
        })
    });

    (klyntbot, map)
}

/// Extract the first sentence from a text body.
pub(crate) fn extract_first_sentence(body: &str) -> String {
    let trimmed = body.trim();
    // Period followed by space (mid-text sentence boundary)
    if let Some(idx) = trimmed.find(". ") {
        return trimmed[..=idx].to_string();
    }
    // Period followed by newline
    if let Some(idx) = trimmed.find(".\n") {
        return trimmed[..=idx].to_string();
    }
    // Period at end of string
    if trimmed.ends_with('.') {
        return trimmed.to_string();
    }
    // First line
    if let Some(idx) = trimmed.find('\n') {
        let first_line = trimmed[..idx].trim();
        if !first_line.is_empty() {
            return first_line.to_string();
        }
    }
    // Fallback: truncate
    trimmed.chars().take(200).collect()
}

pub fn split_frontmatter(content: &str) -> common::Result<(String, String)> {
    let trimmed = content.trim();
    if !trimmed.starts_with("---") {
        return Ok((String::new(), content.to_string()));
    }
    let after_first = &trimmed[3..];
    if let Some(end_idx) = after_first.find("\n---") {
        let fm = after_first[..end_idx].trim().to_string();
        let body = after_first[end_idx + 4..].to_string();
        Ok((fm, body))
    } else {
        Err(common::ConfigError::Invalid("No closing --- in frontmatter".into()).into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_orchestrator_skill_md() {
        let content = r#"---
name: task-management
description: >
  Create, organize, and track tasks using OKR+PARA.
  Use when the user mentions todos or tasks.
metadata:
  author: klyntbot
  version: "1.0.0"
  klyntbot:
    type: orchestrator
    tools: [tasks, project, area]
    can_delegate_to: [finance-management]
    max_iterations: 12
    always_skills: [todo, daily-planner]
---

You are the task management specialist.

## Behavior
- Create tasks efficiently
"#;
        let pkg = parse_skill_md(
            content,
            PathBuf::from("skills/task-management"),
            SkillScope::BuiltIn,
        )
        .unwrap();
        assert_eq!(pkg.name, "task-management");
        assert!(pkg.description.contains("OKR+PARA"));
        assert_eq!(pkg.skill_type, SkillType::Orchestrator);
        assert_eq!(
            pkg.metadata.klyntbot.as_ref().unwrap().tools,
            Some(vec!["tasks".into(), "project".into(), "area".into()])
        );
        assert_eq!(
            pkg.metadata.klyntbot.as_ref().unwrap().can_delegate_to,
            vec!["finance-management"]
        );
        assert_eq!(
            pkg.metadata.klyntbot.as_ref().unwrap().max_iterations,
            Some(12)
        );
        assert_eq!(
            pkg.metadata.klyntbot.as_ref().unwrap().always_skills,
            vec!["todo", "daily-planner"]
        );
        assert!(pkg.body.contains("task management specialist"));
    }

    #[test]
    fn test_parse_simple_skill_md() {
        let content = r#"---
name: search
description: Web search and information retrieval.
---

Use web_search for real-time info.
"#;
        let pkg =
            parse_skill_md(content, PathBuf::from("skills/search"), SkillScope::User).unwrap();
        assert_eq!(pkg.name, "search");
        assert_eq!(pkg.skill_type, SkillType::Skill);
        assert!(pkg.metadata.klyntbot.is_none());
        assert!(pkg.body.contains("web_search"));
    }

    #[test]
    fn test_parse_skill_md_missing_description_fails() {
        let content = r#"---
name: bad-skill
---

No description.
"#;
        let result = parse_skill_md(content, PathBuf::from("skills/bad"), SkillScope::User);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_skill_md_empty_body_ok() {
        let content = r#"---
name: minimal
description: A minimal skill.
---
"#;
        let pkg = parse_skill_md(
            content,
            PathBuf::from("skills/minimal"),
            SkillScope::BuiltIn,
        )
        .unwrap();
        assert_eq!(pkg.body, "");
    }

    #[test]
    fn test_parse_skill_md_custom_metadata_excludes_klyntbot() {
        let content = r#"---
name: test
description: Test skill.
metadata:
  author: someone
  version: "2.0"
  klyntbot:
    type: skill
  custom_key: custom_value
---

Body.
"#;
        let pkg = parse_skill_md(content, PathBuf::from("skills/test"), SkillScope::User).unwrap();
        assert!(pkg.metadata.klyntbot.is_some());
        assert!(pkg.metadata.custom.contains_key("author"));
        assert!(pkg.metadata.custom.contains_key("custom_key"));
        assert!(
            !pkg.metadata.custom.contains_key("klyntbot"),
            "klyntbot should be excluded from custom"
        );
    }

    #[test]
    fn test_split_frontmatter() {
        let content = "---\nname: test\n---\n\nBody here.";
        let (fm, body) = split_frontmatter(content).unwrap();
        assert_eq!(fm, "name: test");
        assert!(body.contains("Body here."));
    }

    #[test]
    fn test_split_frontmatter_no_closing() {
        let content = "---\nname: test\nNo closing delimiter.";
        let result = split_frontmatter(content);
        assert!(result.is_err());
    }

    #[test]
    fn test_lenient_yaml_unquoted_colon() {
        // This is technically invalid YAML but common in cross-client skills
        let content = "---\nname: my-skill\ndescription: Use this skill when: the user asks about PDFs\n---\n\nBody.";
        let pkg =
            parse_skill_md(content, PathBuf::from("skills/my-skill"), SkillScope::User).unwrap();
        assert_eq!(pkg.name, "my-skill");
        assert!(pkg.description.contains("PDFs"));
    }

    #[test]
    fn test_name_validation_loads_despite_issues() {
        // Uppercase name: warn but load
        let content = "---\nname: My-Skill\ndescription: Test.\n---\n\nBody.";
        let pkg =
            parse_skill_md(content, PathBuf::from("skills/My-Skill"), SkillScope::User).unwrap();
        assert_eq!(pkg.name, "My-Skill");
    }

    #[test]
    fn parse_skill_md_extracts_triggers() {
        let content = r#"---
name: test-skill
description: A test skill for managing tasks
metadata:
  klyntbot:
    type: orchestrator
    triggers:
      - "add task"
      - "create todo"
      - "show tasks"
---
Test body content.
"#;
        let pkg = parse_skill_md(content, PathBuf::from("/tmp/test"), SkillScope::BuiltIn).unwrap();
        let meta = pkg.metadata.klyntbot.unwrap();
        assert_eq!(meta.triggers.len(), 3);
        assert!(meta.triggers.contains(&"add task".to_string()));
    }

    #[test]
    fn parse_skill_md_empty_triggers_is_ok() {
        let content = r#"---
name: test-skill
description: A test skill
---
Body.
"#;
        let pkg = parse_skill_md(content, PathBuf::from("/tmp/test"), SkillScope::BuiltIn).unwrap();
        let meta = pkg.metadata.klyntbot;
        assert!(meta.is_none() || meta.unwrap().triggers.is_empty());
    }

    #[test]
    fn test_parse_summary_from_frontmatter() {
        let md = "---\nname: test-skill\ndescription: A test skill.\nmetadata:\n  klyntbot:\n    summary: Handles task CRUD and project management.\n---\nFull body here.";
        let pkg = parse_skill_md(
            md,
            std::path::PathBuf::from("test"),
            crate::types::SkillScope::BuiltIn,
        )
        .unwrap();
        assert_eq!(pkg.summary, "Handles task CRUD and project management.");
    }

    #[test]
    fn test_parse_summary_fallback_to_first_sentence() {
        let md = "---\nname: test-skill\ndescription: A test skill.\n---\nThis is the first sentence. This is the second sentence.";
        let pkg = parse_skill_md(
            md,
            std::path::PathBuf::from("test"),
            crate::types::SkillScope::BuiltIn,
        )
        .unwrap();
        assert_eq!(pkg.summary, "This is the first sentence.");
    }

    #[test]
    fn test_parse_summary_fallback_short_body() {
        let md = "---\nname: test-skill\ndescription: A test skill.\n---\nShort body";
        let pkg = parse_skill_md(
            md,
            std::path::PathBuf::from("test"),
            crate::types::SkillScope::BuiltIn,
        )
        .unwrap();
        assert_eq!(pkg.summary, "Short body");
    }
}
