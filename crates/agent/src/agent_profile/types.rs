use std::collections::HashSet;
use std::path::PathBuf;

use serde::Deserialize;

const DEFAULT_MAX_ITERATIONS: u32 = 10;

#[derive(Debug, Clone)]
pub struct AgentProfile {
    pub name: String,
    pub description: String,
    pub tools: Vec<String>,
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

    /// Check if this agent's triggers match the given message.
    pub fn matches_message(&self, message: &str) -> bool {
        if self.triggers.is_empty() {
            return false;
        }
        let lower = message.to_lowercase();
        self.triggers
            .iter()
            .any(|trigger| lower.contains(trigger.as_str()))
    }

    /// Format the agent's always-loaded skills for system prompt injection.
    pub fn always_loaded_skill_content(&self) -> Vec<String> {
        self.skills
            .iter()
            .filter(|s| self.always_skills.contains(&s.name) || s.always)
            .map(|s| format!("# Skill: {}\n\n{}", s.name, s.content))
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
            content: body.trim().to_string(),
        })
    }
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
        let profile =
            AgentProfile::parse("task", content, PathBuf::from("builtin::task")).unwrap();
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
            triggers: vec![
                "todo".into(),
                "create a task".into(),
                "my tasks".into(),
            ],
            ..Default::default()
        };
        assert!(profile.matches_message("can you create a task for me?"));
        assert!(profile.matches_message("show me my tasks"));
        assert!(profile.matches_message("add to my todo list"));
        assert!(!profile.matches_message("what's the weather?"));
    }
}
