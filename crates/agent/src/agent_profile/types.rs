use std::collections::HashSet;
use std::path::PathBuf;

use serde::Deserialize;

const DEFAULT_MAX_ITERATIONS: u32 = 10;

#[derive(Debug, Clone)]
pub struct AgentProfile {
    pub name: String,
    pub description: String,
    pub tools: Vec<String>,
    pub mcp_tools: Vec<String>,
    pub triggers: Vec<String>,
    pub max_iterations: u32,
    pub can_delegate_to: Vec<String>,
    pub always_skills: Vec<String>,
    pub instructions: String,
    pub skills: Vec<AgentSkill>,
    pub path: PathBuf,
}

impl Default for AgentProfile {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: String::new(),
            tools: vec![],
            mcp_tools: vec![],
            triggers: vec![],
            max_iterations: DEFAULT_MAX_ITERATIONS,
            can_delegate_to: vec![],
            always_skills: vec![],
            instructions: String::new(),
            skills: vec![],
            path: PathBuf::new(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct AgentSkill {
    pub name: String,
    pub description: String,
    pub always: bool,
    pub triggers: Vec<String>,
    pub content: String,
}

#[derive(Deserialize)]
struct AgentFrontmatter {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    tools: Vec<String>,
    #[serde(default)]
    mcp_tools: Vec<String>,
    #[serde(default)]
    triggers: Vec<String>,
    #[serde(default = "default_max_iterations")]
    max_iterations: u32,
    #[serde(default)]
    can_delegate_to: Vec<String>,
    #[serde(default)]
    always_skills: Vec<String>,
}

fn default_max_iterations() -> u32 {
    DEFAULT_MAX_ITERATIONS
}

#[derive(Deserialize)]
struct SkillFrontmatter {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    always: bool,
    #[serde(default)]
    triggers: Vec<String>,
}

impl AgentProfile {
    pub fn parse(name: &str, content: &str, path: PathBuf) -> common::Result<Self> {
        let (frontmatter_str, body) = split_frontmatter(content)?;
        let fm: AgentFrontmatter = serde_yaml::from_str(&frontmatter_str)
            .map_err(|e| common::ConfigError::Invalid(format!("Agent {name} frontmatter: {e}")))?;

        Ok(Self {
            name: fm.name,
            description: fm.description,
            tools: fm.tools,
            mcp_tools: fm.mcp_tools,
            triggers: fm.triggers.into_iter().map(|t| t.to_lowercase()).collect(),
            max_iterations: fm.max_iterations,
            can_delegate_to: fm.can_delegate_to,
            always_skills: fm.always_skills,
            instructions: body.trim().to_string(),
            skills: vec![],
            path,
        })
    }

    /// Returns None if all tools allowed (empty tools list = full access).
    /// Returns Some(set) with ask_user always included.
    pub fn allowed_tool_names(&self) -> Option<HashSet<String>> {
        if self.tools.is_empty() {
            return None;
        }
        let mut set: HashSet<String> = self.tools.iter().cloned().collect();
        set.insert(tools::ask_user::ASK_USER_TOOL_NAME.to_string());
        Some(set)
    }

    /// Check if this profile allows tools from the given MCP server name.
    /// Empty `mcp_tools` denies all. `["*"]` allows all.
    pub fn allows_mcp_server(&self, server_name: &str) -> bool {
        self.mcp_tools.iter().any(|s| s == "*" || s == server_name)
    }

    /// Check if this agent's triggers match the given message.
    /// Normalizes hyphens to spaces so "weekly-review" matches trigger "weekly review".
    pub fn matches_message(&self, message: &str) -> bool {
        if self.triggers.is_empty() {
            return false;
        }
        let normalized = normalize_for_matching(message);
        self.triggers
            .iter()
            .any(|trigger| normalized.contains(trigger.as_str()))
    }

    /// Whether a skill is always-loaded (via `always: true` or listed in `always_skills`).
    pub fn is_always_loaded(&self, skill: &AgentSkill) -> bool {
        skill.always || self.always_skills.contains(&skill.name)
    }

    /// Format the agent's always-loaded skills for system prompt injection.
    pub fn always_loaded_skill_content(&self) -> Vec<String> {
        self.skills
            .iter()
            .filter(|s| self.is_always_loaded(s))
            .map(|s| format!("# Skill: {}\n\n{}", s.name, s.content))
            .collect()
    }

    /// Returns on-demand skills whose triggers match the given message.
    /// Used to dynamically inject relevant skill content into the system prompt.
    pub fn message_activated_skills(&self, message: &str) -> Vec<&AgentSkill> {
        let normalized = normalize_for_matching(message);
        self.skills
            .iter()
            .filter(|s| {
                // Skip always-loaded skills — they're already injected
                if self.is_always_loaded(s) {
                    return false;
                }
                // Match on skill-level triggers
                if !s.triggers.is_empty() {
                    return s.triggers.iter().any(|t| normalized.contains(t.as_str()));
                }
                // Fallback: match on skill name (hyphen-normalized)
                normalized.contains(&normalize_for_matching(&s.name))
            })
            .collect()
    }
}

impl AgentSkill {
    pub fn parse(name: &str, content: &str) -> common::Result<Self> {
        let (frontmatter_str, body) = split_frontmatter(content)?;
        let fm: SkillFrontmatter = serde_yaml::from_str(&frontmatter_str)
            .map_err(|e| common::ConfigError::Invalid(format!("Skill {name} frontmatter: {e}")))?;

        Ok(Self {
            name: fm.name,
            description: fm.description,
            always: fm.always,
            triggers: fm.triggers.into_iter().map(|t| t.to_lowercase()).collect(),
            content: body.trim().to_string(),
        })
    }
}

/// Normalize text for trigger matching: lowercase + hyphens→spaces.
pub(crate) fn normalize_for_matching(text: &str) -> String {
    text.to_lowercase().replace('-', " ")
}

fn split_frontmatter(content: &str) -> common::Result<(String, String)> {
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
    fn test_parse_agent_md_frontmatter() {
        let content = r#"---
name: task
description: Task management specialist
tools: [task, area, project]
triggers: [todo, task, create a task]
max_iterations: 10
can_delegate_to: [calendar, finance]
always_skills: [todo]
---

You are the task management agent.

## Behavior
- Create tasks efficiently
"#;
        let profile = AgentProfile::parse("task", content, PathBuf::from("builtin::task")).unwrap();
        assert_eq!(profile.name, "task");
        assert_eq!(profile.description, "Task management specialist");
        assert_eq!(profile.tools, vec!["task", "area", "project"]);
        assert_eq!(profile.triggers, vec!["todo", "task", "create a task"]);
        assert_eq!(profile.max_iterations, 10);
        assert_eq!(profile.can_delegate_to, vec!["calendar", "finance"]);
        assert_eq!(profile.always_skills, vec!["todo"]);
        assert!(profile.instructions.contains("task management agent"));
    }

    #[test]
    fn test_parse_agent_md_defaults() {
        let content = r#"---
name: general
description: General assistant
---

Instructions here.
"#;
        let profile =
            AgentProfile::parse("general", content, PathBuf::from("builtin::general")).unwrap();
        assert!(profile.tools.is_empty());
        assert!(profile.triggers.is_empty());
        assert_eq!(profile.max_iterations, 10);
        assert!(profile.can_delegate_to.is_empty());
        assert!(profile.always_skills.is_empty());
    }

    #[test]
    fn test_parse_skill_md() {
        let content = r#"---
name: todo
description: Task creation workflow
always: true
---

Skill body content here.
"#;
        let skill = AgentSkill::parse("todo", content).unwrap();
        assert_eq!(skill.name, "todo");
        assert_eq!(skill.description, "Task creation workflow");
        assert!(skill.always);
        assert_eq!(skill.content, "Skill body content here.");
    }

    #[test]
    fn test_agent_profile_allowed_tools_full_access() {
        let profile = AgentProfile {
            name: "general".into(),
            tools: vec![],
            ..Default::default()
        };
        assert!(profile.allowed_tool_names().is_none());
    }

    #[test]
    fn test_agent_profile_allowed_tools_filtered() {
        let profile = AgentProfile {
            name: "task".into(),
            tools: vec!["task".into(), "area".into()],
            ..Default::default()
        };
        let allowed = profile.allowed_tool_names().unwrap();
        assert!(allowed.contains("task"));
        assert!(allowed.contains("area"));
        assert!(allowed.contains("ask_user"));
        assert!(!allowed.contains("finance"));
    }

    #[test]
    fn test_trigger_matching() {
        let profile = AgentProfile {
            name: "task".into(),
            triggers: vec!["todo".into(), "create a task".into(), "my tasks".into()],
            ..Default::default()
        };
        assert!(profile.matches_message("can you create a task for me?"));
        assert!(profile.matches_message("show me my tasks"));
        assert!(profile.matches_message("add to my todo list"));
        assert!(!profile.matches_message("what's the weather?"));
    }

    #[test]
    fn test_trigger_matching_normalizes_hyphens() {
        let profile = AgentProfile {
            name: "task".into(),
            triggers: vec!["weekly review".into(), "decompose".into()],
            ..Default::default()
        };
        // Hyphenated form should match space-separated trigger
        assert!(profile.matches_message("use weekly-review skills"));
        // Normal space form should still work
        assert!(profile.matches_message("show me my weekly review"));
    }

    #[test]
    fn test_parse_skill_with_triggers() {
        let content = r#"---
name: weekly-review
description: Weekly review workflow
always: false
triggers: [weekly review, review my week]
---

Review content here.
"#;
        let skill = AgentSkill::parse("weekly-review", content).unwrap();
        assert_eq!(skill.name, "weekly-review");
        assert!(!skill.always);
        assert_eq!(skill.triggers, vec!["weekly review", "review my week"]);
    }

    #[test]
    fn test_message_activated_skills() {
        let profile = AgentProfile {
            name: "task".into(),
            always_skills: vec!["todo".into()],
            skills: vec![
                AgentSkill {
                    name: "todo".into(),
                    always: true,
                    content: "Always loaded".into(),
                    ..Default::default()
                },
                AgentSkill {
                    name: "weekly-review".into(),
                    always: false,
                    triggers: vec!["weekly review".into(), "review my week".into()],
                    content: "Review workflow".into(),
                    ..Default::default()
                },
                AgentSkill {
                    name: "retrospective".into(),
                    always: false,
                    triggers: vec!["retrospective".into(), "monthly review".into()],
                    content: "Retro workflow".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        // Should activate weekly-review (via trigger match)
        let activated = profile.message_activated_skills("show me my weekly review");
        assert_eq!(activated.len(), 1);
        assert_eq!(activated[0].name, "weekly-review");

        // Should activate weekly-review even with hyphen
        let activated = profile.message_activated_skills("use weekly-review skill");
        assert_eq!(activated.len(), 1);
        assert_eq!(activated[0].name, "weekly-review");

        // Should NOT include always-loaded todo skill
        let activated = profile.message_activated_skills("show me my todo list");
        assert!(activated.is_empty());

        // Should activate retrospective
        let activated = profile.message_activated_skills("run my monthly review");
        assert_eq!(activated.len(), 1);
        assert_eq!(activated[0].name, "retrospective");
    }

    #[test]
    fn test_message_activated_skills_fallback_to_name() {
        let profile = AgentProfile {
            name: "task".into(),
            always_skills: vec![],
            skills: vec![AgentSkill {
                name: "task-decompose".into(),
                always: false,
                triggers: vec![], // No explicit triggers
                content: "Decompose content".into(),
                ..Default::default()
            }],
            ..Default::default()
        };

        // Should match via name fallback (hyphen normalized)
        let activated = profile.message_activated_skills("use task-decompose");
        assert_eq!(activated.len(), 1);
        assert_eq!(activated[0].name, "task-decompose");

        let activated = profile.message_activated_skills("use task decompose");
        assert_eq!(activated.len(), 1);
    }

    #[test]
    fn test_parse_agent_md_with_mcp_tools() {
        let content = r#"---
name: communication
description: Communication agent
tools: [message, ask_user]
mcp_tools: [linear, slack]
---

Instructions here.
"#;
        let profile =
            AgentProfile::parse("communication", content, PathBuf::from("builtin::communication"))
                .unwrap();
        assert_eq!(profile.mcp_tools, vec!["linear", "slack"]);
        assert!(profile.allows_mcp_server("linear"));
        assert!(profile.allows_mcp_server("slack"));
        assert!(!profile.allows_mcp_server("github"));
    }

    #[test]
    fn test_parse_agent_md_mcp_tools_defaults_empty() {
        let content = r#"---
name: task
description: Task agent
tools: [task]
---

Instructions here.
"#;
        let profile =
            AgentProfile::parse("task", content, PathBuf::from("builtin::task")).unwrap();
        assert!(profile.mcp_tools.is_empty());
        assert!(!profile.allows_mcp_server("linear"));
    }

    #[test]
    fn test_mcp_tools_wildcard_allows_all() {
        let profile = AgentProfile {
            name: "general".into(),
            mcp_tools: vec!["*".into()],
            ..Default::default()
        };
        assert!(profile.allows_mcp_server("linear"));
        assert!(profile.allows_mcp_server("github"));
        assert!(profile.allows_mcp_server("anything"));
    }

    #[test]
    fn test_mcp_tools_empty_denies_all() {
        let profile = AgentProfile {
            name: "task".into(),
            mcp_tools: vec![],
            ..Default::default()
        };
        assert!(!profile.allows_mcp_server("linear"));
        assert!(!profile.allows_mcp_server("github"));
    }
}
