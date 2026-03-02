//! Skills system with YAML frontmatter parsing and progressive loading.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::OnceLock;
use tokio::fs;
use tracing::{debug, warn};

use common::Result;
use storage::rows::session_context::SessionContextRow;

/// Built-in skill definitions (bundled at compile time)
const BUILTIN_SKILLS: &[(&str, &str)] = &[
    ("browser", include_str!("../../../skills/browser/SKILL.md")),
    ("cron", include_str!("../../../skills/cron/SKILL.md")),
    (
        "daily-planning",
        include_str!("../../../skills/daily-planning/SKILL.md"),
    ),
    ("finance", include_str!("../../../skills/finance/SKILL.md")),
    (
        "skill-creator",
        include_str!("../../../skills/skill-creator/SKILL.md"),
    ),
    (
        "summarize",
        include_str!("../../../skills/summarize/SKILL.md"),
    ),
    ("todo", include_str!("../../../skills/todo/SKILL.md")),
    ("weather", include_str!("../../../skills/weather/SKILL.md")),
    (
        "weekly-report",
        include_str!("../../../skills/weekly-report/SKILL.md"),
    ),
];

/// Optional scope that restricts when a skill is included.
///
/// Same format as persona scopes — parsed from `scope` in YAML frontmatter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SkillScope {
    /// No scope — always included.
    Global,
    /// Only included when the session is in this area.
    Area { name: String },
    /// Only included when the session is in this project.
    Project { name: String },
    /// Only included when `entity_kind` matches (e.g. "finance", "finance.budgets").
    Feature { kind: String },
}

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

    /// Scope restriction (Global if not set)
    #[serde(default = "default_scope")]
    pub scope: SkillScope,

    /// File path
    pub path: PathBuf,

    /// Full content (loaded on-demand)
    #[serde(skip)]
    pub content: Option<String>,

    /// Whether requirements are met
    #[serde(skip)]
    pub available: bool,
}

fn default_scope() -> SkillScope {
    SkillScope::Global
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

        // Parse scope (same format as persona scopes)
        let scope = metadata
            .get("scope")
            .and_then(|v| {
                // scope can be a mapping like { feature: "finance" }
                let map: HashMap<String, String> = serde_yaml::from_value(v.clone()).ok()?;
                if let Some(area) = map.get("area") {
                    Some(SkillScope::Area { name: area.clone() })
                } else if let Some(project) = map.get("project") {
                    Some(SkillScope::Project {
                        name: project.clone(),
                    })
                } else if let Some(feature) = map.get("feature") {
                    Some(SkillScope::Feature {
                        kind: feature.clone(),
                    })
                } else {
                    None
                }
            })
            .unwrap_or(SkillScope::Global);

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
            scope,
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

    /// Returns skills that match the current persona and session context.
    ///
    /// Includes a skill if any of:
    /// - The skill name is in `persona_skills` (explicitly requested by persona).
    /// - The skill has no scope (Global) — always included.
    /// - The skill's scope matches `session_context.entity_kind`.
    ///
    /// When both `persona_skills` is empty and `session_context` is None,
    /// all skills are returned (current behavior).
    pub fn skills_for_context(
        &self,
        persona_skills: &HashSet<String>,
        session_context: Option<&SessionContextRow>,
    ) -> Vec<&Skill> {
        // No filtering when no persona and no session context
        if persona_skills.is_empty() && session_context.is_none() {
            return self.skills.values().collect();
        }

        self.skills
            .values()
            .filter(|skill| {
                // Always include if persona explicitly names this skill
                if persona_skills.contains(&skill.name) {
                    return true;
                }

                // Always include global (unscoped) skills
                if skill.scope == SkillScope::Global {
                    return true;
                }

                // Check if skill scope matches the session context
                if let Some(ctx) = session_context {
                    return scope_matches_context(&skill.scope, ctx);
                }

                false
            })
            .collect()
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

/// Check if a skill scope matches the session context.
///
/// Uses the same dot-prefix matching as persona Feature scopes:
/// scope "finance" matches entity_kind "finance", "finance.budgets", etc.
fn scope_matches_context(scope: &SkillScope, ctx: &SessionContextRow) -> bool {
    match scope {
        SkillScope::Global => true,
        SkillScope::Area { name } => ctx.area_id.as_deref().is_some_and(|_| {
            // Area matching would require resolved scope IDs (like PersonaManager).
            // For skills, we match by name if entity_kind == "area" and entity is this area.
            // Simplified: match if entity_kind is "area" — full resolution happens via persona.
            ctx.entity_kind.as_deref() == Some("area")
                && ctx
                    .entity_id
                    .as_deref()
                    .is_some_and(|id| id == name || ctx.area_id.as_deref() == Some(id))
        }),
        SkillScope::Project { name } => {
            ctx.entity_kind.as_deref() == Some("project")
                && ctx
                    .entity_id
                    .as_deref()
                    .is_some_and(|id| id == name || ctx.project_id.as_deref() == Some(id))
        }
        SkillScope::Feature { kind } => ctx.entity_kind.as_deref().is_some_and(|ek| {
            ek == kind
                || (ek.starts_with(kind.as_str()) && ek.as_bytes().get(kind.len()) == Some(&b'.'))
        }),
    }
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

    fn make_context(
        entity_kind: Option<&str>,
        entity_id: Option<&str>,
        area_id: Option<&str>,
        project_id: Option<&str>,
    ) -> SessionContextRow {
        SessionContextRow {
            session_key: "test:1".to_string(),
            context_type: "general".to_string(),
            entity_kind: entity_kind.map(|s| s.to_string()),
            entity_id: entity_id.map(|s| s.to_string()),
            area_id: area_id.map(|s| s.to_string()),
            project_id: project_id.map(|s| s.to_string()),
            is_ephemeral: false,
            is_pinned: false,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    fn make_skill(name: &str, scope: SkillScope) -> Skill {
        Skill {
            name: name.to_string(),
            description: format!("{name} skill"),
            version: "1.0".to_string(),
            always: false,
            triggers: Vec::new(),
            requires_bins: Vec::new(),
            requires_env: Vec::new(),
            scope,
            path: PathBuf::from(format!("builtin::{name}")),
            content: Some(format!("{name} content")),
            available: true,
        }
    }

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

    #[test]
    fn skill_scope_parsed_from_frontmatter() {
        let mgr = SkillManager::new();
        let content = "---\nname: finance\ndescription: Finance\nscope:\n  feature: finance\n---\nContent here";
        let skill = mgr
            .parse_skill_content("finance", content, PathBuf::from("builtin::finance"))
            .unwrap();
        assert_eq!(
            skill.scope,
            SkillScope::Feature {
                kind: "finance".to_string()
            }
        );
    }

    #[test]
    fn skill_scope_global_when_no_scope_field() {
        let mgr = SkillManager::new();
        let content = "---\nname: todo\ndescription: Todo\n---\nContent";
        let skill = mgr
            .parse_skill_content("todo", content, PathBuf::from("builtin::todo"))
            .unwrap();
        assert_eq!(skill.scope, SkillScope::Global);
    }

    #[test]
    fn skills_for_context_no_filter_returns_all() {
        let mut mgr = SkillManager::new();
        mgr.skills
            .insert("todo".into(), make_skill("todo", SkillScope::Global));
        mgr.skills.insert(
            "finance".into(),
            make_skill(
                "finance",
                SkillScope::Feature {
                    kind: "finance".to_string(),
                },
            ),
        );

        let result = mgr.skills_for_context(&HashSet::new(), None);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn skills_for_context_filters_by_feature_scope() {
        let mut mgr = SkillManager::new();
        mgr.skills
            .insert("todo".into(), make_skill("todo", SkillScope::Global));
        mgr.skills.insert(
            "finance".into(),
            make_skill(
                "finance",
                SkillScope::Feature {
                    kind: "finance".to_string(),
                },
            ),
        );

        // Task session — finance skill should be excluded
        let ctx = make_context(Some("task"), Some("t-1"), None, None);
        let result = mgr.skills_for_context(&HashSet::new(), Some(&ctx));
        let names: HashSet<_> = result.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains("todo"));
        assert!(!names.contains("finance"));
    }

    #[test]
    fn skills_for_context_includes_matching_feature() {
        let mut mgr = SkillManager::new();
        mgr.skills
            .insert("todo".into(), make_skill("todo", SkillScope::Global));
        mgr.skills.insert(
            "finance".into(),
            make_skill(
                "finance",
                SkillScope::Feature {
                    kind: "finance".to_string(),
                },
            ),
        );

        // Finance budgets session — both should be included
        let ctx = make_context(Some("finance.budgets"), None, None, None);
        let result = mgr.skills_for_context(&HashSet::new(), Some(&ctx));
        let names: HashSet<_> = result.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains("todo"));
        assert!(names.contains("finance"));
    }

    #[test]
    fn skills_for_context_persona_override_includes_scoped_skill() {
        let mut mgr = SkillManager::new();
        mgr.skills
            .insert("todo".into(), make_skill("todo", SkillScope::Global));
        mgr.skills.insert(
            "finance".into(),
            make_skill(
                "finance",
                SkillScope::Feature {
                    kind: "finance".to_string(),
                },
            ),
        );

        // Task session but persona explicitly requests finance skill
        let ctx = make_context(Some("task"), Some("t-1"), None, None);
        let persona_skills: HashSet<String> = ["finance".to_string()].into();
        let result = mgr.skills_for_context(&persona_skills, Some(&ctx));
        let names: HashSet<_> = result.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains("todo"));
        assert!(names.contains("finance"));
    }

    #[test]
    fn scope_feature_exact_match() {
        let ctx = make_context(Some("finance"), None, None, None);
        assert!(scope_matches_context(
            &SkillScope::Feature {
                kind: "finance".to_string()
            },
            &ctx
        ));
    }

    #[test]
    fn scope_feature_dot_prefix_match() {
        let ctx = make_context(Some("finance.budgets"), None, None, None);
        assert!(scope_matches_context(
            &SkillScope::Feature {
                kind: "finance".to_string()
            },
            &ctx
        ));
    }

    #[test]
    fn scope_feature_no_false_positive() {
        // "financials" should NOT match scope "finance"
        let ctx = make_context(Some("financials"), None, None, None);
        assert!(!scope_matches_context(
            &SkillScope::Feature {
                kind: "finance".to_string()
            },
            &ctx
        ));
    }

    #[test]
    fn builtin_skills_include_browser_finance_weekly_report() {
        let mut mgr = SkillManager::new();
        mgr.load_builtin_skills().unwrap();

        assert!(mgr.get("browser").is_some());
        assert!(mgr.get("finance").is_some());
        assert!(mgr.get("weekly-report").is_some());
    }

    #[test]
    fn finance_builtin_has_feature_scope() {
        let mut mgr = SkillManager::new();
        mgr.load_builtin_skills().unwrap();

        let finance = mgr.get("finance").unwrap();
        assert_eq!(
            finance.scope,
            SkillScope::Feature {
                kind: "finance".to_string()
            }
        );
    }
}
