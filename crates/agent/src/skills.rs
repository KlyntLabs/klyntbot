//! Skills system with YAML frontmatter parsing and progressive loading.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;
use tokio::fs;
use tracing::{debug, warn};

use common::Result;

/// Built-in skill definitions (bundled at compile time)
const BUILTIN_SKILLS: &[(&str, &str)] = &[
    ("cron", include_str!("../../../skills/cron/SKILL.md")),
    (
        "daily-planning",
        include_str!("../../../skills/daily-planning/SKILL.md"),
    ),
    (
        "skill-creator",
        include_str!("../../../skills/skill-creator/SKILL.md"),
    ),
    (
        "summarize",
        include_str!("../../../skills/summarize/SKILL.md"),
    ),
    ("todo", include_str!("../../../skills/todo/SKILL.md")),
    (
        "todo-party",
        include_str!("../../../skills/todo-party/SKILL.md"),
    ),
    (
        "todo-yolo",
        include_str!("../../../skills/todo-yolo/SKILL.md"),
    ),
    ("weather", include_str!("../../../skills/weather/SKILL.md")),
];

/// Skill metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    /// Skill name (from directory name)
    pub name: String,

    /// Description
    pub description: String,

    /// Version
    #[serde(default)]
    pub version: String,

    /// Always load full content (not just summary)
    #[serde(default)]
    pub always: bool,

    /// Trigger keywords
    #[serde(default)]
    pub triggers: Vec<String>,

    /// Required binaries
    #[serde(default)]
    pub requires_bins: Vec<String>,

    /// Required environment variables
    #[serde(default)]
    pub requires_env: Vec<String>,

    /// File path
    pub path: PathBuf,

    /// Full content (loaded on-demand)
    #[serde(skip)]
    pub content: Option<String>,

    /// Whether requirements are met
    #[serde(skip)]
    pub available: bool,
}

/// Skill manager
pub struct SkillManager {
    skills: HashMap<String, Skill>,
    /// Cached XML summary (computed once after loading).
    summary_cache: OnceLock<String>,
}

impl SkillManager {
    /// Create a new skill manager
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
            summary_cache: OnceLock::new(),
        }
    }

    /// Load skills from workspace and built-in directories
    pub async fn load(&mut self, workspace_path: PathBuf) -> Result<()> {
        // Load built-in skills first (as fallback)
        debug!("Loading built-in skills");
        self.load_builtin_skills()?;

        // Load workspace skills (these override built-in skills)
        let workspace_skills_dir = workspace_path.join("skills");
        if workspace_skills_dir.exists() {
            debug!("Loading workspace skills from {:?}", workspace_skills_dir);
            self.load_from_directory(&workspace_skills_dir).await?;
        }

        debug!("Loaded {} skills total", self.skills.len());

        Ok(())
    }

    /// Load built-in skills from bundled content
    fn load_builtin_skills(&mut self) -> Result<()> {
        for (name, content) in BUILTIN_SKILLS {
            match self.parse_skill_content(
                name,
                content,
                PathBuf::from(format!("builtin::{}", name)),
            ) {
                Ok(skill) => {
                    debug!("Loaded built-in skill: {}", skill.name);
                    self.skills.insert(skill.name.clone(), skill);
                }
                Err(e) => {
                    warn!("Failed to load built-in skill '{}': {}", name, e);
                }
            }
        }
        Ok(())
    }

    /// Load skills from a directory
    async fn load_from_directory(&mut self, dir: &PathBuf) -> Result<()> {
        let mut entries = fs::read_dir(dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            if path.is_dir() {
                let skill_file = path.join("SKILL.md");

                if skill_file.exists() {
                    match self.load_skill(&skill_file).await {
                        Ok(skill) => {
                            debug!("Loaded skill: {}", skill.name);
                            self.skills.insert(skill.name.clone(), skill);
                        }
                        Err(e) => {
                            warn!("Failed to load skill from {:?}: {}", skill_file, e);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Load a single skill file
    async fn load_skill(&self, path: &PathBuf) -> Result<Skill> {
        let content = fs::read_to_string(path).await?;
        let name = path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        self.parse_skill_content(&name, &content, path.clone())
    }

    /// Parse skill content (shared by built-in and file-based loading)
    fn parse_skill_content(&self, name: &str, content: &str, path: PathBuf) -> Result<Skill> {
        // Parse frontmatter
        let (metadata, skill_content) = parse_frontmatter(content);

        let description = metadata
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let version = metadata
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("1.0")
            .to_string();

        // Parse klyntbot metadata if present
        let skill_meta = metadata
            .get("metadata")
            .and_then(|v| v.as_str())
            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
            .and_then(|v| v.get("klyntbot").cloned());

        let always = skill_meta
            .as_ref()
            .and_then(|m| m.get("always"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let triggers: Vec<String> = skill_meta
            .as_ref()
            .and_then(|m| m.get("triggers"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect::<Vec<String>>()
            })
            .unwrap_or_default();

        let requires_bins: Vec<String> = skill_meta
            .as_ref()
            .and_then(|m| m.get("requires"))
            .and_then(|r| r.get("bins"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect::<Vec<String>>()
            })
            .unwrap_or_default();

        let requires_env: Vec<String> = skill_meta
            .as_ref()
            .and_then(|m| m.get("requires"))
            .and_then(|r| r.get("env"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect::<Vec<String>>()
            })
            .unwrap_or_default();

        // Check requirements
        let available = check_requirements(&requires_bins, &requires_env);

        Ok(Skill {
            name: name.to_string(),
            description,
            version,
            always,
            triggers,
            requires_bins,
            requires_env,
            path,
            content: Some(skill_content),
            available,
        })
    }

    /// Generate XML skills summary for system prompt (cached after first call).
    pub fn generate_summary(&self) -> &str {
        self.summary_cache.get_or_init(|| {
            let mut summary = String::from("<skills>\n");

            for skill in self.skills.values() {
                summary.push_str(&format!(
                    "  <skill name=\"{}\" available=\"{}\">\n",
                    skill.name, skill.available
                ));
                summary.push_str(&format!(
                    "    <description>{}</description>\n",
                    skill.description
                ));
                summary.push_str(&format!("    <path>{}</path>\n", skill.path.display()));

                if !skill.triggers.is_empty() {
                    summary.push_str(&format!(
                        "    <triggers>{}</triggers>\n",
                        skill.triggers.join(", ")
                    ));
                }

                summary.push_str("  </skill>\n");
            }

            summary.push_str("</skills>");
            summary
        })
    }

    /// Get skills that should always be loaded
    pub fn get_always_loaded(&self) -> Vec<&Skill> {
        self.skills
            .values()
            .filter(|s| s.always && s.available)
            .collect()
    }

    /// Get a skill by name
    pub fn get(&self, name: &str) -> Option<&Skill> {
        self.skills.get(name)
    }

    /// Get all skills
    pub fn all(&self) -> Vec<&Skill> {
        self.skills.values().collect()
    }

    /// Filter loaded skills to only those in the allowed list.
    ///
    /// Used after `load()` to restrict skills to those from enabled packs.
    /// Workspace-loaded skills (non-builtin) are always kept.
    pub fn filter_by_skills(&mut self, allowed_skill_names: &[String]) {
        self.skills.retain(|name, skill| {
            // Keep workspace skills (not builtin) regardless
            if !skill.path.to_string_lossy().starts_with("builtin::") {
                return true;
            }
            // Keep if name is in the allowed list
            allowed_skill_names.iter().any(|a| a == name)
        });
    }
}

impl Default for SkillManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse YAML frontmatter from markdown
fn parse_frontmatter(content: &str) -> (HashMap<String, serde_yaml::Value>, String) {
    let lines: Vec<&str> = content.lines().collect();

    if lines.is_empty() || !lines[0].trim().starts_with("---") {
        return (HashMap::new(), content.to_string());
    }

    // Find end of frontmatter
    let mut end_idx = 0;
    for (i, line) in lines.iter().enumerate().skip(1) {
        if line.trim().starts_with("---") {
            end_idx = i;
            break;
        }
    }

    if end_idx == 0 {
        return (HashMap::new(), content.to_string());
    }

    // Extract frontmatter
    let frontmatter_lines = &lines[1..end_idx];
    let frontmatter_str = frontmatter_lines.join("\n");

    let metadata: HashMap<String, serde_yaml::Value> =
        serde_yaml::from_str(&frontmatter_str).unwrap_or_default();

    // Extract content (everything after frontmatter)
    let content_lines = &lines[(end_idx + 1)..];
    let content_str = content_lines.join("\n");

    (metadata, content_str)
}

/// Check if requirements are met
fn check_requirements(bins: &[String], env_vars: &[String]) -> bool {
    // Check binaries
    for bin in bins {
        if which::which(bin).is_err() {
            return false;
        }
    }

    // Check environment variables
    for var in env_vars {
        if std::env::var(var).is_err() {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_by_skills_includes_matching() {
        let mut mgr = SkillManager::new();
        mgr.load_builtin_skills().unwrap();

        let enabled_skills = vec!["todo".to_string(), "summarize".to_string()];
        mgr.filter_by_skills(&enabled_skills);

        assert!(mgr.get("todo").is_some());
        assert!(mgr.get("summarize").is_some());
        assert!(mgr.get("weather").is_none());
    }

    #[test]
    fn test_filter_by_skills_empty_keeps_nothing() {
        let mut mgr = SkillManager::new();
        mgr.load_builtin_skills().unwrap();

        mgr.filter_by_skills(&[]);

        assert_eq!(mgr.all().len(), 0);
    }

    #[test]
    fn test_filter_by_skills_all_names_keeps_all() {
        let mut mgr = SkillManager::new();
        mgr.load_builtin_skills().unwrap();
        let total = mgr.all().len();

        let all_names: Vec<String> = mgr.all().iter().map(|s| s.name.clone()).collect();
        mgr.filter_by_skills(&all_names);

        assert_eq!(mgr.all().len(), total);
    }
}
