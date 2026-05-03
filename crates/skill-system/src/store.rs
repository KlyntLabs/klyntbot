//! SkillStore — loads skill markdown files from disk.
//!
//! Skills are `.md` files with YAML frontmatter (name, description, whenToUse).
//! The frontmatter is always available; the full body is loaded on demand
//! via the `skill_reference` tool.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use common::ConfigError;
use serde::Deserialize;
use tracing::{debug, warn};

/// Maximum description length in the skill listing (tokens budget).
const MAX_DESCRIPTION_CHARS: usize = 250;

/// Default skills embedded in the binary, installed on first run.
const DEFAULT_SKILLS: &[(&str, &str)] = &[
    (
        "task-management.md",
        include_str!("../../../skills/task-management/SKILL.md"),
    ),
    (
        "finance-management.md",
        include_str!("../../../skills/finance-management/SKILL.md"),
    ),
    (
        "automation.md",
        include_str!("../../../skills/automation/SKILL.md"),
    ),
    (
        "notebook.md",
        include_str!("../../../skills/notebook/SKILL.md"),
    ),
    (
        "learning.md",
        include_str!("../../../skills/learning/SKILL.md"),
    ),
    (
        "coding-orchestrator.md",
        include_str!("../../../skills/coding-orchestrator/SKILL.md"),
    ),
];

// ── Types ────────────────────────────────────────────────────

/// YAML frontmatter fields parsed from a skill file.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillFrontmatter {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub when_to_use: Option<String>,
    #[serde(default)]
    pub references: Vec<String>,
    /// Optional scope filter — e.g. `"coding"` or `"finance"`.
    #[serde(default)]
    pub scope: Option<String>,
    /// Optional repository ID for scope-aware filtering.
    #[serde(default, rename = "scope_repo_id")]
    pub scope_repo_id: Option<String>,
}

/// A loaded skill — frontmatter + body.
#[derive(Debug, Clone)]
pub struct SkillEntry {
    pub frontmatter: SkillFrontmatter,
    pub body: String,
    pub path: PathBuf,
}

/// In-memory store of all loaded skills.
#[derive(Debug)]
pub struct SkillStore {
    entries: HashMap<String, SkillEntry>,
    skills_dir: PathBuf,
}

// ── Implementation ───────────────────────────────────────────

impl SkillStore {
    /// Load all skills from the given directory.
    /// If the directory is empty, install default skills first.
    pub fn load(skills_dir: &Path) -> common::Result<Self> {
        // Ensure directory exists
        if !skills_dir.exists() {
            std::fs::create_dir_all(skills_dir).map_err(|e| {
                skill_err(&format!(
                    "Failed to create skills dir {}: {e}",
                    skills_dir.display()
                ))
            })?;
        }

        // Install defaults if empty
        let has_skills = std::fs::read_dir(skills_dir)
            .map(|mut d| d.any(|e| e.is_ok()))
            .unwrap_or(false);

        if !has_skills {
            Self::install_defaults(skills_dir)?;
        }

        // Load all .md files
        let mut entries = HashMap::new();
        for dir_entry in std::fs::read_dir(skills_dir).map_err(|e| {
            skill_err(&format!(
                "Failed to read skills dir {}: {e}",
                skills_dir.display()
            ))
        })? {
            let dir_entry =
                dir_entry.map_err(|e| skill_err(&format!("Failed to read dir entry: {e}")))?;
            let path = dir_entry.path();

            // Support both flat .md files and subdirectories with SKILL.md
            let skill_path = if path.is_dir() {
                let skill_md = path.join("SKILL.md");
                if skill_md.exists() {
                    skill_md
                } else {
                    continue;
                }
            } else if path.extension().is_some_and(|ext| ext == "md") {
                path.clone()
            } else {
                continue;
            };

            match Self::load_one(&skill_path) {
                Ok(entry) => {
                    debug!(name = %entry.frontmatter.name, path = %skill_path.display(), "Loaded skill");
                    entries.insert(entry.frontmatter.name.clone(), entry);
                }
                Err(e) => {
                    warn!(path = %skill_path.display(), error = %e, "Failed to load skill, skipping");
                }
            }
        }

        debug!(count = entries.len(), "SkillStore loaded");
        Ok(Self {
            entries,
            skills_dir: skills_dir.to_path_buf(),
        })
    }

    /// Reload all skills from disk (for hot-reload).
    pub fn reload(&mut self) -> common::Result<()> {
        let reloaded = Self::load(&self.skills_dir)?;
        self.entries = reloaded.entries;
        Ok(())
    }

    /// Get a skill entry by name.
    pub fn get(&self, name: &str) -> Option<&SkillEntry> {
        self.entries.get(name)
    }

    /// Get the full body of a skill (for skill_reference tool).
    pub fn get_body(&self, name: &str) -> Option<&str> {
        self.entries.get(name).map(|e| e.body.as_str())
    }

    /// List skills filtered by scope or repo. Pass `None` to get global skills.
    pub fn list_for_scope(&self, scope: Option<&str>) -> Vec<&SkillEntry> {
        self.entries
            .values()
            .filter(|e| match scope {
                Some(s) => {
                    e.frontmatter.scope.as_deref() == Some(s)
                        || e.frontmatter.scope_repo_id.as_deref() == Some(s)
                }
                None => e.frontmatter.scope.is_none() && e.frontmatter.scope_repo_id.is_none(),
            })
            .collect()
    }

    /// Format the skill listing for the system prompt.
    /// Returns a compact listing of all skills with name + description + whenToUse.
    pub fn format_listing(&self) -> String {
        let mut lines = vec![
            "Available skills (use skill_reference tool to load full instructions):".to_string(),
        ];

        let mut sorted: Vec<_> = self.entries.values().collect();
        sorted.sort_by(|a, b| a.frontmatter.name.cmp(&b.frontmatter.name));

        for entry in sorted {
            let desc = &entry.frontmatter.description;
            let truncated = if desc.chars().count() > MAX_DESCRIPTION_CHARS {
                let end: String = desc.chars().take(MAX_DESCRIPTION_CHARS - 1).collect();
                format!("{end}…")
            } else {
                desc.clone()
            };

            let line = if let Some(ref when) = entry.frontmatter.when_to_use {
                format!("- {}: {} — {}", entry.frontmatter.name, truncated, when)
            } else {
                format!("- {}: {}", entry.frontmatter.name, truncated)
            };
            lines.push(line);
        }

        lines.join("\n")
    }

    /// List all skill names (for skill_reference tool's available list).
    pub fn names(&self) -> Vec<&str> {
        self.entries.keys().map(|s| s.as_str()).collect()
    }

    /// Build a reference index mapping skill name → full body.
    pub fn build_reference_index(&self) -> HashMap<String, String> {
        self.entries
            .iter()
            .map(|(name, entry)| (name.clone(), entry.body.clone()))
            .collect()
    }

    // ── Private ──────────────────────────────────────────────

    fn load_one(path: &Path) -> common::Result<SkillEntry> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            skill_err(&format!(
                "Failed to read skill file {}: {e}",
                path.display()
            ))
        })?;

        let (frontmatter, body) = split_frontmatter(&content)?;

        Ok(SkillEntry {
            frontmatter,
            body,
            path: path.to_path_buf(),
        })
    }

    fn install_defaults(skills_dir: &Path) -> common::Result<()> {
        for (filename, content) in DEFAULT_SKILLS {
            let path = skills_dir.join(filename);
            std::fs::write(&path, content).map_err(|e| {
                skill_err(&format!(
                    "Failed to write default skill {}: {e}",
                    path.display()
                ))
            })?;
            debug!(path = %path.display(), "Installed default skill");
        }
        Ok(())
    }
}

/// Helper to create a skill-system error.
fn skill_err(msg: &str) -> common::KlyntbotError {
    common::KlyntbotError::Config(ConfigError::Invalid(msg.to_string()))
}

/// Split a markdown file into YAML frontmatter and body.
pub fn split_frontmatter(content: &str) -> common::Result<(SkillFrontmatter, String)> {
    let trimmed = content.trim();
    if !trimmed.starts_with("---") {
        return Err(skill_err(
            "Skill file must start with YAML frontmatter (---)",
        ));
    }

    let after_first = &trimmed[3..];
    let end_idx = after_first
        .find("\n---")
        .ok_or_else(|| skill_err("Missing closing --- for YAML frontmatter"))?;

    let yaml_str = &after_first[..end_idx];
    let body = after_first[end_idx + 4..].trim().to_string();

    let frontmatter: SkillFrontmatter = serde_yaml::from_str(yaml_str)
        .map_err(|e| skill_err(&format!("Failed to parse skill YAML: {e}")))?;

    Ok((frontmatter, body))
}

// ── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_skill_frontmatter() {
        let content = "---\nname: test-skill\ndescription: A test skill\nwhenToUse: When testing\n---\n\nBody content here.";
        let (fm, body) = split_frontmatter(content).unwrap();
        assert_eq!(fm.name, "test-skill");
        assert_eq!(fm.description, "A test skill");
        assert_eq!(fm.when_to_use.as_deref(), Some("When testing"));
        assert_eq!(body, "Body content here.");
    }

    #[test]
    fn parse_skill_without_when_to_use() {
        let content = "---\nname: minimal\ndescription: Minimal skill\n---\n\nBody.";
        let (fm, body) = split_frontmatter(content).unwrap();
        assert_eq!(fm.name, "minimal");
        assert!(fm.when_to_use.is_none());
        assert_eq!(body, "Body.");
    }

    #[test]
    fn missing_frontmatter_errors() {
        let content = "No frontmatter here.";
        assert!(split_frontmatter(content).is_err());
    }

    #[test]
    fn format_listing_includes_all_skills() {
        let mut entries = HashMap::new();
        entries.insert(
            "test".to_string(),
            SkillEntry {
                frontmatter: SkillFrontmatter {
                    name: "test".to_string(),
                    description: "Test skill".to_string(),
                    when_to_use: Some("When testing".to_string()),
                    references: vec![],
                    scope: None,
                    scope_repo_id: None,
                },
                body: "Body".to_string(),
                path: PathBuf::from("test.md"),
            },
        );
        let store = SkillStore {
            entries,
            skills_dir: PathBuf::from("/tmp"),
        };
        let listing = store.format_listing();
        assert!(listing.contains("test: Test skill — When testing"));
    }

    #[test]
    fn default_skills_parse() {
        for (filename, content) in DEFAULT_SKILLS {
            let result = split_frontmatter(content);
            assert!(
                result.is_ok(),
                "Failed to parse {}: {:?}",
                filename,
                result.err()
            );
            let (fm, body) = result.unwrap();
            assert!(!fm.name.is_empty(), "{} has empty name", filename);
            assert!(
                !fm.description.is_empty(),
                "{} has empty description",
                filename
            );
            assert!(!body.is_empty(), "{} has empty body", filename);
        }
    }
}
