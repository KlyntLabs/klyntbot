use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use common::helpers::cosine_similarity;

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
    include_agent_skill!("general", "skill-creator"),
    include_agent_skill!("task", "todo"),
    include_agent_skill!("task", "daily-planner"),
    include_agent_skill!("task", "task-decompose"),
    include_agent_skill!("task", "project-management"),
    include_agent_skill!("task", "weekly-review"),
    include_agent_skill!("task", "retrospective"),
    include_agent_skill!("automation", "cron"),
];

const GENERAL_AGENT_NAME: &str = "general";

/// Metadata about a built-in agent (for UI listing without full parse).
pub struct BuiltinAgentInfo {
    pub name: &'static str,
    pub content: &'static str,
    pub skills: Vec<BuiltinSkillInfo>,
}

/// Metadata about a built-in skill.
pub struct BuiltinSkillInfo {
    pub name: &'static str,
    pub content: &'static str,
}

/// Returns raw built-in agent content for the UI layer.
/// This is a static view — no workspace overrides applied.
pub fn builtin_agents() -> Vec<BuiltinAgentInfo> {
    BUILTIN_AGENTS
        .iter()
        .map(|(name, content)| {
            let skills = BUILTIN_AGENT_SKILLS
                .iter()
                .filter(|(agent, _, _)| *agent == *name)
                .map(|(_, skill_name, skill_content)| BuiltinSkillInfo {
                    name: skill_name,
                    content: skill_content,
                })
                .collect();
            BuiltinAgentInfo {
                name,
                content,
                skills,
            }
        })
        .collect()
}

#[derive(Default)]
pub struct AgentManager {
    agents: HashMap<String, Arc<AgentProfile>>,
    /// One representative embedding per agent (description + triggers concatenated).
    /// Stored inside `Arc` so it can be cheaply cloned out of an async lock.
    agent_embeddings: Arc<HashMap<String, Vec<f32>>>,
    /// Text embedder injected at startup. `None` = keyword-only fallback.
    embedder: Option<Arc<dyn cognitive::TextEmbedder>>,
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

    /// Reload: reset to built-in agents then re-apply workspace overrides.
    /// Called after editing agent files from the UI.
    pub async fn reload(&mut self, workspace_path: &Path) -> common::Result<()> {
        self.agents.clear();
        self.agent_embeddings = Arc::new(HashMap::new());
        self.load_builtin_agents()?;
        self.load_workspace_agents(workspace_path).await.ok(); // non-fatal
        self.precompute_embeddings().await;
        Ok(())
    }

    /// Inject a text embedder for semantic matching. Called once at startup before
    /// `precompute_embeddings`.
    pub fn set_embedder(&mut self, embedder: Arc<dyn cognitive::TextEmbedder>) {
        self.embedder = Some(embedder);
    }

    /// Cheap Arc clone of the embedder — for extracting outside an async lock.
    pub fn get_embedder(&self) -> Option<Arc<dyn cognitive::TextEmbedder>> {
        self.embedder.clone()
    }

    /// Precompute one embedding per agent from its description + trigger list.
    /// No-ops if no embedder is set. Non-fatal: logs warnings on per-agent failures.
    pub async fn precompute_embeddings(&mut self) {
        let Some(ref embedder) = self.embedder else {
            return;
        };
        let mut embeddings = HashMap::new();
        for (name, profile) in &self.agents {
            if profile.triggers.is_empty() {
                continue;
            }
            let text = format!(
                "{}. Triggers: {}",
                profile.description,
                profile.triggers.join(", ")
            );
            match embedder.embed(&text).await {
                Ok(vec) => {
                    embeddings.insert(name.clone(), vec);
                }
                Err(e) => {
                    tracing::warn!(agent = %name, "Failed to precompute agent embedding: {e}");
                }
            }
        }
        tracing::debug!("Precomputed embeddings for {} agents", embeddings.len());
        self.agent_embeddings = Arc::new(embeddings);
    }

    /// Normalized keyword scores for every agent with at least one trigger match.
    /// Score is normalized to `[0, 1]` (raw word-count / 5, capped at 1.0).
    pub fn keyword_scores_owned(&self, message: &str) -> HashMap<String, f64> {
        let normalized = super::types::normalize_for_matching(message);
        let mut result = HashMap::new();
        for (name, profile) in &self.agents {
            if profile.triggers.is_empty() {
                continue;
            }
            let mut score: usize = 0;
            let mut hits: usize = 0;
            for t in &profile.triggers {
                if normalized.contains(t.as_str()) {
                    score += t.split_whitespace().count();
                    hits += 1;
                }
            }
            if hits > 0 {
                result.insert(name.clone(), (score as f64 / 5.0).min(1.0));
            }
        }
        result
    }

    /// Select the best agent by blending keyword and semantic scores.
    ///
    /// Formula: `kw * 0.7 + sem * 0.3`. An agent is only considered if it has a
    /// keyword match OR its semantic similarity is ≥ 0.5. Falls back to `general`
    /// when no agent clears the threshold.
    pub fn blend_scores(&self, kw: &HashMap<String, f64>, query_vec: &[f32]) -> &Arc<AgentProfile> {
        let mut best: Option<(&str, f64)> = None;
        for (name, profile) in &self.agents {
            if profile.triggers.is_empty() {
                continue;
            }
            let kw_score = kw.get(name.as_str()).copied().unwrap_or(0.0);
            let sem_score = self
                .agent_embeddings
                .get(name)
                .map(|av| cosine_similarity(query_vec, av))
                .unwrap_or(0.0);

            // Require meaningful keyword hit or high semantic similarity
            if kw_score == 0.0 && sem_score < 0.5 {
                continue;
            }
            let blended = kw_score * 0.7 + sem_score * 0.3;
            if best.is_none_or(|(_, s)| blended > s) {
                best = Some((name.as_str(), blended));
            }
        }
        if let Some((name, _)) = best {
            &self.agents[name]
        } else {
            self.get_general()
        }
    }

    /// Match a user message to an agent profile.
    /// Returns the best-matching agent, or the general fallback.
    ///
    /// Scoring: each matching trigger contributes its word count as weight,
    /// so longer (more specific) triggers like "remind me" outscore shorter
    /// ones like "schedule". Ties broken by hit count.
    pub fn match_agent(&self, message: &str) -> &Arc<AgentProfile> {
        let normalized = super::types::normalize_for_matching(message);

        let mut best: Option<(&str, usize, usize)> = None; // (name, score, hits)
        for (name, profile) in &self.agents {
            if profile.triggers.is_empty() {
                continue;
            }
            let mut score: usize = 0;
            let mut hits: usize = 0;
            for t in &profile.triggers {
                if normalized.contains(t.as_str()) {
                    // Weight by word count so multi-word triggers score higher
                    score += t.split_whitespace().count();
                    hits += 1;
                }
            }
            if hits > 0 {
                let dominated =
                    best.is_some_and(|(_, bs, bh)| score < bs || (score == bs && hits < bh));
                if !dominated {
                    best = Some((name.as_str(), score, hits));
                }
            }
        }

        if let Some((name, _, _)) = best {
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

    /// Directly inject a precomputed embedding for a named agent (test-only helper).
    fn inject_embedding(mgr: &mut AgentManager, agent: &str, vec: Vec<f32>) {
        let mut map = (*mgr.agent_embeddings).clone();
        map.insert(agent.to_string(), vec);
        mgr.agent_embeddings = Arc::new(map);
    }

    #[test]
    fn test_load_builtin_agents() {
        let mgr = make_test_manager();
        assert!(mgr.get("general").is_some());
        assert!(mgr.get("task").is_some());
    }

    #[test]
    fn test_match_agent_task() {
        let mgr = make_test_manager();
        let matched = mgr.match_agent("create a task for reviewing the budget");
        assert_eq!(matched.name, "task");
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
        assert!(matched.name == "task");
    }

    #[test]
    fn test_match_agent_remind_routes_to_automation() {
        let mgr = make_test_manager();
        let matched = mgr.match_agent("remind me to buy groceries tomorrow");
        assert_eq!(
            matched.name, "automation",
            "remind me should route to automation agent"
        );
    }

    #[test]
    fn test_match_agent_set_reminder_routes_to_automation() {
        let mgr = make_test_manager();
        let matched = mgr.match_agent("set a reminder for my meeting at 3pm");
        assert_eq!(
            matched.name, "automation",
            "reminder should route to automation agent"
        );
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

    #[test]
    fn test_keyword_scores_owned_task() {
        let mgr = make_test_manager();
        let scores = mgr.keyword_scores_owned("create a task for me");
        assert!(
            scores.contains_key("task"),
            "task agent should have a score"
        );
        assert!(*scores.get("task").unwrap() > 0.0);
        assert!(!scores.contains_key("general"), "general has no triggers");
    }

    #[test]
    fn test_keyword_scores_owned_no_match() {
        let mgr = make_test_manager();
        let scores = mgr.keyword_scores_owned("hello, how are you today?");
        assert!(
            scores.is_empty(),
            "no agent should score for a generic greeting"
        );
    }

    #[test]
    fn test_blend_scores_semantic_rescues_keyword_miss() {
        let mut mgr = make_test_manager();
        // Inject a unit vector for "task" and orthogonal vectors for others
        let task_vec = vec![1.0_f32, 0.0, 0.0];
        inject_embedding(&mut mgr, "task", task_vec.clone());
        inject_embedding(&mut mgr, "automation", vec![0.0, 0.0, 1.0]);

        // No keyword hit, but query points straight at task (cosine = 1.0 ≥ 0.5)
        let kw: HashMap<String, f64> = HashMap::new();
        let selected = mgr.blend_scores(&kw, &task_vec);
        assert_eq!(
            selected.name, "task",
            "semantic should rescue when no keyword match"
        );
    }

    #[test]
    fn test_blend_scores_keyword_wins_over_weak_semantic() {
        let mut mgr = make_test_manager();
        inject_embedding(&mut mgr, "task", vec![0.6_f32, 0.0, 0.0]);

        let mut kw: HashMap<String, f64> = HashMap::new();
        kw.insert("task".to_string(), 0.6);

        let query = vec![0.3_f32, 0.4, 0.0];
        let selected = mgr.blend_scores(&kw, &query);
        assert_eq!(
            selected.name, "task",
            "keyword match should dominate over weak semantic"
        );
    }
}
