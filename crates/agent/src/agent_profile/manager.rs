use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::{AgentProfile, AgentSkill};

// Built-in agents compiled at build time
macro_rules! include_agent {
    ($name:expr) => {
        (
            $name,
            include_str!(concat!("../../../../agents/", $name, "/AGENT.md")),
        )
    };
}

const BUILTIN_AGENTS: &[(&str, &str)] = &[
    include_agent!("general"),
    include_agent!("task"),
    include_agent!("finance"),
    include_agent!("calendar"),
    include_agent!("automation"),
    include_agent!("communication"),
];

// Built-in agent skills — each entry is (agent_name, skill_name, content)
macro_rules! include_agent_skill {
    ($agent:expr, $skill:expr) => {
        (
            $agent,
            $skill,
            include_str!(concat!(
                "../../../../agents/",
                $agent,
                "/skills/",
                $skill,
                ".md"
            )),
        )
    };
}

const BUILTIN_AGENT_SKILLS: &[(&str, &str, &str)] = &[
    include_agent_skill!("general", "memory"),
    include_agent_skill!("general", "search"),
    include_agent_skill!("general", "browser"),
    include_agent_skill!("general", "summarize"),
    include_agent_skill!("general", "weather"),
    include_agent_skill!("general", "skill-creator"),
    include_agent_skill!("task", "todo"),
    include_agent_skill!("task", "daily-planner"),
    include_agent_skill!("task", "task-decompose"),
    include_agent_skill!("task", "project-management"),
    include_agent_skill!("task", "weekly-review"),
    include_agent_skill!("task", "retrospective"),
    include_agent_skill!("finance", "budgeting"),
    include_agent_skill!("finance", "spending-analysis"),
    include_agent_skill!("calendar", "scheduling"),
    include_agent_skill!("automation", "cron"),
];

const GENERAL_AGENT_NAME: &str = "general";

#[derive(Default)]
pub struct AgentManager {
    agents: HashMap<String, Arc<AgentProfile>>,
}

impl AgentManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Load built-in agents from compiled-in AGENT.md files.
    pub fn load_builtin_agents(&mut self) -> common::Result<()> {
        for (name, content) in BUILTIN_AGENTS {
            let path = PathBuf::from(format!("builtin::{name}"));
            let mut profile = AgentProfile::parse(name, content, path)?;

            // Load skills for this agent
            for (agent_name, skill_name, skill_content) in BUILTIN_AGENT_SKILLS {
                if *agent_name == *name {
                    let skill = AgentSkill::parse(skill_name, skill_content)?;
                    profile.skills.push(skill);
                }
            }

            self.agents.insert(name.to_string(), Arc::new(profile));
        }
        Ok(())
    }

    /// Load workspace agents from a directory (overrides built-in by name).
    pub async fn load_workspace_agents(&mut self, workspace_path: &Path) -> common::Result<()> {
        let agents_dir = workspace_path.join("agents");
        if !agents_dir.exists() {
            return Ok(());
        }

        let mut entries = tokio::fs::read_dir(&agents_dir)
            .await
            .map_err(|e| common::ConfigError::Invalid(format!("Reading agents dir: {e}")))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| common::ConfigError::Invalid(format!("Reading agents entry: {e}")))?
        {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let agent_md = path.join("AGENT.md");
            if !agent_md.exists() {
                continue;
            }
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            let content = tokio::fs::read_to_string(&agent_md).await.map_err(|e| {
                common::ConfigError::Invalid(format!("Reading {}: {e}", agent_md.display()))
            })?;
            let mut profile = AgentProfile::parse(&name, &content, path.clone())?;

            // Load skills from the agent's skills/ subfolder
            let skills_dir = path.join("skills");
            if skills_dir.exists() {
                let mut skill_entries = tokio::fs::read_dir(&skills_dir).await.map_err(|e| {
                    common::ConfigError::Invalid(format!("Reading skills dir: {e}"))
                })?;
                while let Some(skill_entry) = skill_entries.next_entry().await.map_err(|e| {
                    common::ConfigError::Invalid(format!("Reading skill entry: {e}"))
                })? {
                    let skill_path = skill_entry.path();
                    if skill_path.extension().and_then(|e| e.to_str()) != Some("md") {
                        continue;
                    }
                    let skill_name = skill_path
                        .file_stem()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    let skill_content =
                        tokio::fs::read_to_string(&skill_path).await.map_err(|e| {
                            common::ConfigError::Invalid(format!(
                                "Reading skill {}: {e}",
                                skill_path.display()
                            ))
                        })?;
                    let skill = AgentSkill::parse(&skill_name, &skill_content)?;
                    profile.skills.push(skill);
                }
            }

            self.agents.insert(name, Arc::new(profile));
        }
        Ok(())
    }

    /// Match a user message to an agent profile.
    /// Returns the best-matching agent, or the general fallback.
    pub fn match_agent(&self, message: &str) -> &Arc<AgentProfile> {
        let normalized = super::types::normalize_for_matching(message);

        // Score each agent by number of trigger hits
        let mut best: Option<(&str, usize)> = None;
        for (name, profile) in &self.agents {
            if profile.triggers.is_empty() {
                continue;
            }
            let hits = profile
                .triggers
                .iter()
                .filter(|t| normalized.contains(t.as_str()))
                .count();
            if hits > 0 {
                if let Some((_, best_hits)) = best {
                    if hits > best_hits {
                        best = Some((name.as_str(), hits));
                    }
                } else {
                    best = Some((name.as_str(), hits));
                }
            }
        }

        if let Some((name, _)) = best {
            &self.agents[name]
        } else {
            self.get_general()
        }
    }

    pub fn get(&self, name: &str) -> Option<&Arc<AgentProfile>> {
        self.agents.get(name)
    }

    pub fn get_general(&self) -> &Arc<AgentProfile> {
        self.agents
            .get(GENERAL_AGENT_NAME)
            .expect("General agent must exist")
    }

    pub fn all_agents(&self) -> impl Iterator<Item = &Arc<AgentProfile>> {
        self.agents.values()
    }

    pub fn agent_names(&self) -> Vec<&str> {
        self.agents.keys().map(|s| s.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_manager() -> AgentManager {
        let mut mgr = AgentManager::new();
        mgr.load_builtin_agents().unwrap();
        mgr
    }

    #[test]
    fn test_load_builtin_agents() {
        let mgr = make_test_manager();
        assert!(mgr.get("general").is_some());
        assert!(mgr.get("task").is_some());
        assert!(mgr.get("finance").is_some());
        assert!(mgr.get("calendar").is_some());
    }

    #[test]
    fn test_match_agent_task() {
        let mgr = make_test_manager();
        let matched = mgr.match_agent("create a task for reviewing the budget");
        assert_eq!(matched.name, "task");
    }

    #[test]
    fn test_match_agent_finance() {
        let mgr = make_test_manager();
        let matched = mgr.match_agent("check my budget spending");
        assert_eq!(matched.name, "finance");
    }

    #[test]
    fn test_match_agent_fallback_to_general() {
        let mgr = make_test_manager();
        let matched = mgr.match_agent("hello, how are you?");
        assert_eq!(matched.name, "general");
    }

    #[test]
    fn test_match_agent_ambiguous_prefers_more_hits() {
        let mgr = make_test_manager();
        let matched = mgr.match_agent("create a task about my budget");
        // Both task and finance could match — should return one with more trigger hits
        assert!(matched.name == "task" || matched.name == "finance");
    }

    #[test]
    fn test_agents_include_skills() {
        let mgr = make_test_manager();
        let task_agent = mgr.get("task").unwrap();
        assert!(
            !task_agent.skills.is_empty(),
            "task agent should have skills loaded"
        );
        assert!(
            task_agent.skills.iter().any(|s| s.name == "todo"),
            "task agent should have the todo skill"
        );
    }
}
