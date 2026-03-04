# Agent-Driven Architecture Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the monolithic IntentPipeline with an agent-driven architecture where domain-specific agent profiles shape LLM behavior, agents can delegate to each other, the system auto-decides sync vs async, and a unified learning system personalizes interactions.

**Architecture:** Single LLM with dynamic agent profiles loaded from `agents/` folders (AGENT.md + skills/). AgentRuntime replaces IntentPipeline. DelegationTool enables agent-to-agent composition. Unified LearningSystem replaces fragmented MemoryStore + partial learning. ExecutionCore + ReactiveEngine are reused unchanged.

**Tech Stack:** Rust, SQLite (sqlx), tokio, serde_yaml, fastembed + LanceDB (existing)

**Design Doc:** `docs/plans/2026-03-04-agent-driven-architecture-design.md`

---

## Phase 1: Agent Profile System

### Task 1: Create built-in agent definitions

**Files:**
- Create: `agents/general/AGENT.md`
- Create: `agents/general/skills/memory.md`
- Create: `agents/general/skills/search.md`
- Create: `agents/task/AGENT.md`
- Create: `agents/task/skills/todo.md` (move from `skills/todo/SKILL.md`)
- Create: `agents/task/skills/planning.md` (move from `skills/daily-planning/SKILL.md`)
- Create: `agents/task/skills/project-management.md`
- Create: `agents/finance/AGENT.md`
- Create: `agents/finance/skills/budgeting.md` (move from `skills/finance/SKILL.md`)
- Create: `agents/finance/skills/spending-analysis.md`
- Create: `agents/calendar/AGENT.md`
- Create: `agents/calendar/skills/scheduling.md`
- Create: `agents/automation/AGENT.md`
- Create: `agents/automation/skills/cron.md` (move from `skills/cron/SKILL.md`)
- Create: `agents/communication/AGENT.md`

**Step 1: Create general agent (fallback orchestrator)**

```markdown
# agents/general/AGENT.md
---
name: general
description: General-purpose assistant and orchestrator
tools: [ask_user, memory, web_search, web_fetch, grep, glob, read_file, list_dir, spawn, learning]
max_iterations: 10
can_delegate_to: [task, finance, calendar, automation, communication]
always_skills: []
---

You are a general-purpose assistant. You handle greetings, casual conversation, questions,
and any request that doesn't clearly belong to a specialized domain.

## Behavior
- For simple questions and greetings, respond directly without tools
- When a request touches a specific domain (tasks, finance, calendar), delegate to the specialist agent
- Use web search for factual questions you're unsure about
- Use memory to recall and store important user information
```

**Step 2: Create task agent**

```markdown
# agents/task/AGENT.md
---
name: task
description: Task and project management specialist
tools: [task, area, project, okr, ask_user, memory, grep, glob, read_file, list_dir]
triggers: [todo, task, tasks, create a task, add a task, my tasks, task list, what tasks, check tasks, list tasks, todo list, focus, project, area, objective, key result, okr]
max_iterations: 10
can_delegate_to: [calendar, finance]
always_skills: [todo]
---

You are the task management agent. You help users create, organize, and track tasks,
projects, areas, and objectives using the OKR+PARA framework.

## Behavior
- When creating tasks, follow the todo skill's workflow (ask-first, enrichment, confidence scoring)
- For "plan my day" requests, delegate to calendar agent for schedule context
- When a task relates to finance, delegate to the finance agent for budget context
- Use the OKR framework for objectives and key results

## Response Style
- Be concise and action-oriented
- Confirm task creation with a brief summary including inferred fields
- Suggest next actions when relevant
```

**Step 3: Create finance agent**

```markdown
# agents/finance/AGENT.md
---
name: finance
description: Personal finance management specialist
tools: [finance, ask_user, memory, web_search, web_fetch]
triggers: [finance, money, budget, spending, investment, savings, net worth, account, transaction, portfolio, goal, FIRE, net_worth, price, crypto]
max_iterations: 10
can_delegate_to: [task]
always_skills: [budgeting]
---

You are the finance agent. You help users manage their personal finances including accounts,
transactions, budgets, investments, goals, and financial reports.

## Behavior
- Track accounts, transactions, and budgets
- Provide spending analysis and investment tracking
- Create tasks via delegation when financial actions need follow-up
- Use web search for current market prices when needed

## Response Style
- Present financial data clearly with amounts and percentages
- Highlight trends and anomalies
- Suggest actionable improvements
```

**Step 4: Create calendar, automation, and communication agents**

Create similar AGENT.md files for:
- `agents/calendar/AGENT.md` — tools: `[calendar, task, ask_user]`, triggers: calendar-related keywords
- `agents/automation/AGENT.md` — tools: `[cron, spawn, ask_user]`, triggers: cron/schedule/reminder keywords
- `agents/communication/AGENT.md` — tools: `[message, ask_user]`, triggers: message/send/notify keywords

**Step 5: Move existing skills into agent folders**

Move skill content (body only, not old frontmatter) from:
- `skills/todo/SKILL.md` → `agents/task/skills/todo.md`
- `skills/daily-planning/SKILL.md` → `agents/task/skills/planning.md`
- `skills/finance/SKILL.md` → `agents/finance/skills/budgeting.md`
- `skills/cron/SKILL.md` → `agents/automation/skills/cron.md`
- `skills/browser/SKILL.md` → `agents/general/skills/browser.md`
- `skills/summarize/SKILL.md` → `agents/general/skills/summarize.md`
- `skills/weather/SKILL.md` → `agents/general/skills/weather.md`
- `skills/weekly-report/SKILL.md` → `agents/task/skills/weekly-report.md`
- `skills/skill-creator/SKILL.md` → `agents/general/skills/skill-creator.md`

New skill .md format (simpler than old SKILL.md — no nested JSON metadata):
```yaml
---
name: todo
description: Task creation workflow with confidence scoring and enrichment
always: true
---

(skill body content, same as before)
```

**Step 6: Commit**

```bash
git add agents/
git commit -m "feat(agents): create built-in agent definitions with skills"
```

---

### Task 2: Build AgentProfile types

**Files:**
- Create: `crates/agent/src/agent_profile/types.rs`
- Create: `crates/agent/src/agent_profile/mod.rs`
- Test: `crates/agent/src/agent_profile/types.rs` (inline tests)

**Step 1: Write the failing tests**

```rust
// In crates/agent/src/agent_profile/types.rs
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
        let profile = AgentProfile::parse("general", content, PathBuf::from("builtin::general")).unwrap();
        assert!(profile.tools.is_empty()); // empty = all tools (Full access)
        assert!(profile.triggers.is_empty());
        assert_eq!(profile.max_iterations, 10); // default
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
            tools: vec![], // empty = full access
            ..Default::default()
        };
        assert!(profile.allowed_tool_names().is_none()); // None = all tools
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
        assert!(allowed.contains("ask_user")); // always included
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
}
```

**Step 2: Run tests to verify they fail**

```bash
cargo nextest run -p agent -E 'test(agent_profile)' --no-capture
```
Expected: compilation error — `agent_profile` module doesn't exist yet.

**Step 3: Implement types**

```rust
// crates/agent/src/agent_profile/types.rs
use std::collections::HashSet;
use std::path::PathBuf;
use serde::Deserialize;

#[derive(Debug, Clone, Default)]
pub struct AgentProfile {
    pub name: String,
    pub description: String,
    pub tools: Vec<String>,          // empty = full access (all tools)
    pub triggers: Vec<String>,       // keyword triggers for agent matching
    pub max_iterations: u32,         // ReAct budget (default: 10)
    pub can_delegate_to: Vec<String>, // agents this one can call
    pub always_skills: Vec<String>,  // skills always loaded in prompt
    pub instructions: String,        // AGENT.md body (after frontmatter)
    pub skills: Vec<AgentSkill>,     // loaded from skills/ subfolder
    pub path: PathBuf,
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

fn default_max_iterations() -> u32 { 10 }

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
            .map_err(|e| common::KlyntbotError::Config(format!("Agent {name} frontmatter: {e}")))?;

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
            return None; // full access
        }
        let mut set: HashSet<String> = self.tools.iter().cloned().collect();
        set.insert("ask_user".to_string());
        Some(set)
    }

    /// Check if this agent's triggers match the given message.
    pub fn matches_message(&self, message: &str) -> bool {
        if self.triggers.is_empty() {
            return false;
        }
        let lower = message.to_lowercase();
        self.triggers.iter().any(|trigger| lower.contains(trigger.as_str()))
    }

    /// Format the agent's always-loaded skills for system prompt injection.
    pub fn always_loaded_skill_content(&self) -> Vec<String> {
        self.skills.iter()
            .filter(|s| self.always_skills.contains(&s.name) || s.always)
            .map(|s| format!("# Skill: {}\n\n{}", s.name, s.content))
            .collect()
    }

    /// Format all skill content for trigger-matched injection.
    pub fn formatted_instructions(&self) -> String {
        self.instructions.clone()
    }
}

impl AgentSkill {
    pub fn parse(name: &str, content: &str) -> common::Result<Self> {
        let (frontmatter_str, body) = split_frontmatter(content)?;
        let fm: SkillFrontmatter = serde_yaml::from_str(&frontmatter_str)
            .map_err(|e| common::KlyntbotError::Config(format!("Skill {name} frontmatter: {e}")))?;

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
        Err(common::KlyntbotError::Config("No closing --- in frontmatter".into()))
    }
}
```

```rust
// crates/agent/src/agent_profile/mod.rs
mod types;
pub use types::*;
```

**Step 4: Run tests to verify they pass**

```bash
cargo nextest run -p agent -E 'test(agent_profile)' --no-capture
```
Expected: All tests PASS.

**Step 5: Commit**

```bash
git add crates/agent/src/agent_profile/
git commit -m "feat(agent): add AgentProfile and AgentSkill types with parsing"
```

---

### Task 3: Build AgentManager

**Files:**
- Create: `crates/agent/src/agent_profile/manager.rs`
- Modify: `crates/agent/src/agent_profile/mod.rs`
- Test: inline in `manager.rs`

**Step 1: Write the failing tests**

```rust
// In crates/agent/src/agent_profile/manager.rs
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
    fn test_match_agent_ambiguous_prefers_first_match() {
        let mgr = make_test_manager();
        // "task" triggers should match before "finance" for this message
        let matched = mgr.match_agent("create a task about my budget");
        // Both task and finance could match — should return the one with more trigger hits
        assert!(matched.name == "task" || matched.name == "finance");
    }

    #[test]
    fn test_agents_include_skills() {
        let mgr = make_test_manager();
        let task_agent = mgr.get("task").unwrap();
        assert!(!task_agent.skills.is_empty(), "task agent should have skills loaded");
        assert!(task_agent.skills.iter().any(|s| s.name == "todo"),
            "task agent should have the todo skill");
    }
}
```

**Step 2: Run tests to verify they fail**

```bash
cargo nextest run -p agent -E 'test(agent_profile::manager)' --no-capture
```
Expected: FAIL — `AgentManager` doesn't exist.

**Step 3: Implement AgentManager**

```rust
// crates/agent/src/agent_profile/manager.rs
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use super::{AgentProfile, AgentSkill};

// Built-in agents compiled at build time
macro_rules! include_agent {
    ($name:expr) => {
        ($name, include_str!(concat!("../../../../agents/", $name, "/AGENT.md")))
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

// Built-in agent skills - each entry is (agent_name, skill_name, content)
// Use a macro to include all skill files at compile time
macro_rules! include_agent_skill {
    ($agent:expr, $skill:expr) => {
        ($agent, $skill, include_str!(concat!("../../../../agents/", $agent, "/skills/", $skill, ".md")))
    };
}

const BUILTIN_AGENT_SKILLS: &[(&str, &str, &str)] = &[
    include_agent_skill!("general", "memory"),
    include_agent_skill!("general", "search"),
    include_agent_skill!("task", "todo"),
    include_agent_skill!("task", "planning"),
    include_agent_skill!("task", "project-management"),
    include_agent_skill!("task", "weekly-report"),
    include_agent_skill!("finance", "budgeting"),
    include_agent_skill!("finance", "spending-analysis"),
    include_agent_skill!("calendar", "scheduling"),
    include_agent_skill!("automation", "cron"),
    include_agent_skill!("general", "browser"),
    include_agent_skill!("general", "summarize"),
    include_agent_skill!("general", "weather"),
    include_agent_skill!("general", "skill-creator"),
];

pub struct AgentManager {
    agents: HashMap<String, AgentProfile>,
    general_agent_name: String, // fallback
}

impl AgentManager {
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
            general_agent_name: "general".to_string(),
        }
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

            self.agents.insert(name.to_string(), profile);
        }
        Ok(())
    }

    /// Load workspace agents from ~/.klyntbot/agents/ (overrides built-in by name).
    pub async fn load_workspace_agents(&mut self, workspace_path: &Path) -> common::Result<()> {
        let agents_dir = workspace_path.join("agents");
        if !agents_dir.exists() {
            return Ok(());
        }

        let mut entries = tokio::fs::read_dir(&agents_dir).await
            .map_err(|e| common::KlyntbotError::Config(format!("Reading agents dir: {e}")))?;

        while let Some(entry) = entries.next_entry().await
            .map_err(|e| common::KlyntbotError::Config(format!("Reading agents entry: {e}")))? {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let agent_md = path.join("AGENT.md");
            if !agent_md.exists() {
                continue;
            }
            let name = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            let content = tokio::fs::read_to_string(&agent_md).await
                .map_err(|e| common::KlyntbotError::Config(format!("Reading {}: {e}", agent_md.display())))?;
            let mut profile = AgentProfile::parse(&name, &content, path.clone())?;

            // Load skills from the agent's skills/ subfolder
            let skills_dir = path.join("skills");
            if skills_dir.exists() {
                let mut skill_entries = tokio::fs::read_dir(&skills_dir).await
                    .map_err(|e| common::KlyntbotError::Config(format!("Reading skills dir: {e}")))?;
                while let Some(skill_entry) = skill_entries.next_entry().await
                    .map_err(|e| common::KlyntbotError::Config(format!("Reading skill entry: {e}")))? {
                    let skill_path = skill_entry.path();
                    if skill_path.extension().and_then(|e| e.to_str()) != Some("md") {
                        continue;
                    }
                    let skill_name = skill_path.file_stem()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    let skill_content = tokio::fs::read_to_string(&skill_path).await
                        .map_err(|e| common::KlyntbotError::Config(format!("Reading skill {}: {e}", skill_path.display())))?;
                    let skill = AgentSkill::parse(&skill_name, &skill_content)?;
                    profile.skills.push(skill);
                }
            }

            self.agents.insert(name, profile); // overrides built-in
        }
        Ok(())
    }

    /// Match a user message to an agent profile.
    /// Returns the best-matching agent, or the general fallback.
    pub fn match_agent(&self, message: &str) -> &AgentProfile {
        let lower = message.to_lowercase();

        // Score each agent by number of trigger hits
        let mut best: Option<(&str, usize)> = None;
        for (name, profile) in &self.agents {
            if profile.triggers.is_empty() {
                continue;
            }
            let hits = profile.triggers.iter()
                .filter(|t| lower.contains(t.as_str()))
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

    pub fn get(&self, name: &str) -> Option<&AgentProfile> {
        self.agents.get(name)
    }

    pub fn get_general(&self) -> &AgentProfile {
        self.agents.get(&self.general_agent_name)
            .expect("General agent must exist")
    }

    pub fn all_agents(&self) -> impl Iterator<Item = &AgentProfile> {
        self.agents.values()
    }

    pub fn agent_names(&self) -> Vec<&str> {
        self.agents.keys().map(|s| s.as_str()).collect()
    }
}
```

Update `crates/agent/src/agent_profile/mod.rs`:
```rust
mod types;
mod manager;
pub use types::*;
pub use manager::*;
```

**Step 4: Run tests to verify they pass**

```bash
cargo nextest run -p agent -E 'test(agent_profile)' --no-capture
```
Expected: All tests PASS.

**Step 5: Commit**

```bash
git add crates/agent/src/agent_profile/ agents/
git commit -m "feat(agent): add AgentManager with built-in agent loading and matching"
```

---

### Task 4: Build AgentContextSource

**Files:**
- Create: `crates/agent/src/context_sources/agent.rs`
- Modify: `crates/agent/src/context_sources/mod.rs`
- Test: inline in `agent.rs`

**Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_agent_context_provides_instructions_and_skills() {
        let mut profile = AgentProfile::default();
        profile.name = "task".into();
        profile.instructions = "You are the task agent.".into();
        profile.always_skills = vec!["todo".into()];
        profile.skills = vec![AgentSkill {
            name: "todo".into(),
            description: "Task workflow".into(),
            always: true,
            content: "Create tasks using the task tool.".into(),
        }];

        let source = AgentContextSource::new(Arc::new(RwLock::new(Some(profile))));
        let ctx = SourceContext { channel: "test".into(), chat_id: "1".into(), message: None };
        let result = source.provide(&ctx).await;

        assert!(result.is_some());
        let text = result.unwrap();
        assert!(text.contains("task agent"), "Should contain agent instructions");
        assert!(text.contains("Task workflow") || text.contains("Create tasks"),
            "Should contain always-loaded skill content");
    }
}
```

**Step 2: Run test, verify it fails**

```bash
cargo nextest run -p agent -E 'test(agent_context)' --no-capture
```

**Step 3: Implement AgentContextSource**

This replaces both `SkillSummarySource` (priority 40) and `SkillContentSource` (priority 30) with a single source that injects the active agent's instructions + skills.

```rust
// crates/agent/src/context_sources/agent.rs
use std::sync::Arc;
use tokio::sync::RwLock;
use async_trait::async_trait;
use context_engine::source::{ContextSource, SourceContext};
use crate::agent_profile::AgentProfile;

/// Injects the active agent's instructions and always-loaded skills into the system prompt.
/// Replaces SkillSummarySource + SkillContentSource.
pub struct AgentContextSource {
    active_profile: Arc<RwLock<Option<AgentProfile>>>,
}

impl AgentContextSource {
    pub fn new(active_profile: Arc<RwLock<Option<AgentProfile>>>) -> Self {
        Self { active_profile }
    }
}

#[async_trait]
impl ContextSource for AgentContextSource {
    fn name(&self) -> &str { "agent_profile" }
    fn priority(&self) -> u8 { 35 } // between old summary(40) and content(30)

    async fn provide(&self, _ctx: &SourceContext) -> Option<String> {
        let guard = self.active_profile.read().await;
        let profile = guard.as_ref()?;

        let mut sections = Vec::new();

        // Agent instructions
        if !profile.instructions.is_empty() {
            sections.push(format!("# Agent: {}\n\n{}", profile.name, profile.instructions));
        }

        // Always-loaded skill content
        for skill_content in profile.always_loaded_skill_content() {
            sections.push(skill_content);
        }

        if sections.is_empty() {
            None
        } else {
            Some(sections.join("\n\n---\n\n"))
        }
    }
}
```

**Step 4: Run test, verify it passes**

```bash
cargo nextest run -p agent -E 'test(agent_context)' --no-capture
```

**Step 5: Commit**

```bash
git add crates/agent/src/context_sources/
git commit -m "feat(agent): add AgentContextSource for per-agent prompt injection"
```

---

### Task 5: Wire AgentManager into AgentLoop (parallel run)

**Files:**
- Modify: `crates/agent/src/agent_loop/builder.rs` — add AgentManager construction alongside SkillManager
- Modify: `crates/agent/src/agent_loop/mod.rs` — add `agent_manager` field to AgentLoop
- Modify: `crates/agent/src/lib.rs` or module declarations — export `agent_profile` module

**Step 1: Add `agent_profile` module to agent crate**

In `crates/agent/src/lib.rs` (or wherever modules are declared), add:
```rust
pub mod agent_profile;
```

**Step 2: Add AgentManager to builder.rs**

After the SkillManager loading block (around line 130-142), add:
```rust
// Load agent profiles (new system, runs alongside SkillManager during migration)
let mut agent_manager = crate::agent_profile::AgentManager::new();
agent_manager.load_builtin_agents().map_err(|e| {
    tracing::error!("Failed to load built-in agents: {e}");
    e
})?;
if let Some(data_dir) = &config.data_dir {
    agent_manager.load_workspace_agents(Path::new(data_dir)).await.map_err(|e| {
        tracing::warn!("Failed to load workspace agents: {e}");
        e
    }).ok(); // non-fatal
}
let agent_manager = Arc::new(agent_manager);
```

**Step 3: Add agent_manager field to AgentLoop struct**

In `mod.rs`, add to the AgentLoop struct:
```rust
pub(crate) agent_manager: Arc<crate::agent_profile::AgentManager>,
```

And set it in the builder's `build()` return.

**Step 4: Verify build succeeds**

```bash
cargo build -p agent
cargo nextest run -p agent --no-capture
```
Expected: Build succeeds, all existing tests still pass.

**Step 5: Commit**

```bash
git add crates/agent/
git commit -m "feat(agent): wire AgentManager into AgentLoop alongside SkillManager"
```

---

## Phase 2: Agent Runtime (Core Swap)

### Task 6: Build AgentRuntime struct

**Files:**
- Create: `crates/agent/src/agent_runtime/mod.rs`
- Create: `crates/agent/src/agent_runtime/runtime.rs`
- Test: inline in `runtime.rs`

**Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_agent_runtime_selects_correct_agent() {
        // Build a runtime with mock components
        let agent_manager = {
            let mut mgr = crate::agent_profile::AgentManager::new();
            mgr.load_builtin_agents().unwrap();
            Arc::new(mgr)
        };

        let selected = agent_manager.match_agent("create a task to review budget");
        assert_eq!(selected.name, "task");
    }
}
```

**Step 2: Implement AgentRuntime**

The AgentRuntime replaces IntentPipeline. It takes ownership of:
- `AgentManager` — agent selection
- `IntentAnalyzer` — Direct vs Reactive classification
- `ContextEngine` — context assembly
- `ExecutionRouter` — engine dispatch
- `ResponseValidator` + `CostTracker` — post-processing

The key difference from IntentPipeline: **agent selection happens first**, and the agent profile shapes everything downstream (system prompt, tool filtering, iteration budget).

```rust
// crates/agent/src/agent_runtime/runtime.rs
pub struct AgentRuntime {
    agent_manager: Arc<AgentManager>,
    analyzer: IntentAnalyzer,
    context_engine: Arc<ContextEngine>,
    router: ExecutionRouter,
    validator: ResponseValidator,
    cost_tracker: Arc<CostTracker>,
    config: PipelineConfig,
    strategy_repo: Option<storage::StrategyRepo>,
    confidence_evaluator: Option<Arc<crate::confidence::ConfidenceEvaluator>>,
    active_profile: Arc<RwLock<Option<AgentProfile>>>, // shared with AgentContextSource
}
```

`process_message` flow:
1. `agent_manager.match_agent(message)` → `AgentProfile`
2. Write profile to `active_profile` (AgentContextSource reads this)
3. `analyzer.analyze(message, tool_names)` → `IntentAnalysis`
   - Override `max_iterations` from agent profile
4. Confidence check (same as IntentPipeline)
5. `context_engine.assemble(...)` — now includes AgentContextSource
6. Tool filtering: use `profile.allowed_tool_names()` instead of `ToolGroup`
   - Add DelegationTool if `profile.can_delegate_to` is non-empty
   - MCP tools still bypass filtering
7. `router.execute(...)` — same as before
8. Validation + cost tracking
9. Strategy recording

**Step 3: Run tests**

```bash
cargo nextest run -p agent -E 'test(agent_runtime)' --no-capture
```

**Step 4: Commit**

```bash
git add crates/agent/src/agent_runtime/
git commit -m "feat(agent): add AgentRuntime replacing IntentPipeline"
```

---

### Task 7: Swap IntentPipeline for AgentRuntime in AgentLoop

**Files:**
- Modify: `crates/agent/src/agent_loop/mod.rs` — replace `pipeline` field and `run_pipeline` calls
- Modify: `crates/agent/src/agent_loop/builder.rs` — build AgentRuntime instead of IntentPipeline

**Step 1: Replace pipeline field**

In `AgentLoop` struct, change:
```rust
// Old:
pub(crate) pipeline: Arc<crate::intent_pipeline::IntentPipeline>,
// New:
pub(crate) runtime: Arc<crate::agent_runtime::AgentRuntime>,
```

**Step 2: Update run_pipeline**

The `run_pipeline` method in `mod.rs` currently calls `pipeline.process_message()`. Change it to call `runtime.process_message()` with the same signature. The AgentRuntime's `process_message` should have the same return type (`Result<PipelineResult>`) to minimize churn.

**Step 3: Update builder**

In `builder.rs`, replace the IntentPipeline construction block with AgentRuntime construction. The AgentRuntime takes the same components (analyzer, context_engine, router, cost_tracker) plus the AgentManager.

**Step 4: Remove old skill injection**

Remove the `SkillSummarySource` and `SkillContentSource` registrations from the context sources list in `builder.rs` (lines 182-183). The `AgentContextSource` replaces both.

Remove the `skill_manager` field from `AgentLoop` struct.

**Step 5: Verify build and tests pass**

```bash
cargo build --workspace
cargo nextest run --workspace --no-capture
```

**Step 6: Commit**

```bash
git add crates/agent/
git commit -m "feat(agent): swap IntentPipeline for AgentRuntime in AgentLoop"
```

---

### Task 8: Remove ToolGroup and old SkillManager

**Files:**
- Modify: `crates/agent/src/intent_pipeline/types.rs` — remove `ToolGroup` enum
- Modify: `crates/agent/src/intent_pipeline/analysis.rs` — remove `ToolGroup` references, simplify
- Delete: `crates/agent/src/skills.rs` (or mark deprecated)
- Delete: `crates/agent/src/context_sources/skills.rs`
- Modify: `crates/agent/src/intent_pipeline/pipeline.rs` — remove (file can be deleted if fully replaced)

**Step 1: Remove ToolGroup from IntentAnalysis**

`IntentAnalysis` currently has `tool_groups: Vec<ToolGroup>`. Since the AgentRuntime uses `AgentProfile.tools` for filtering, remove `tool_groups` from `IntentAnalysis` and all code that sets/reads it.

**Step 2: Simplify IntentAnalyzer**

The analyzer still classifies `ExecutionMode` (Direct vs Reactive) and confidence. Remove all `ToolGroup` mapping logic (`map_tool_names_to_groups()`, `allowed_tool_names()` on IntentAnalysis). Keep `matched_skills` → rename to `matched_agent` (single agent name, not a list).

**Step 3: Remove old SkillManager references**

Remove `with_skill_manager()` from IntentAnalyzer. The agent matching is now done by AgentRuntime before calling the analyzer.

**Step 4: Remove old skill context sources**

Delete `crates/agent/src/context_sources/skills.rs` and remove its registration from `mod.rs`.

**Step 5: Verify build and tests**

```bash
cargo build --workspace
cargo nextest run --workspace
```

**Step 6: Commit**

```bash
git add -A
git commit -m "refactor(agent): remove ToolGroup enum and old SkillManager"
```

---

## Phase 3: Delegation + Auto-Async

### Task 9: Build DelegationHandler trait and DelegationTool

**Files:**
- Create: `crates/tools/src/delegation.rs`
- Modify: `crates/tools/src/lib.rs` — export delegation module
- Test: inline in `delegation.rs`

**Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_delegation_tool_schema() {
        let tool = DelegationTool::new();
        assert_eq!(tool.name(), "delegate");
        let schema = tool.parameters();
        // Verify required params
        let required = schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v == "agent"));
        assert!(required.iter().any(|v| v == "query"));
    }

    #[tokio::test]
    async fn test_delegation_tool_without_handler_errors() {
        let tool = DelegationTool::new();
        let args = serde_json::json!({"agent": "calendar", "query": "list events"});
        let ctx = RoutingContext::test_context();
        let result = tool.execute(args, &ctx).await;
        assert!(result.is_err() || result.unwrap().contains("not available"));
    }
}
```

**Step 2: Implement**

```rust
// crates/tools/src/delegation.rs
use async_trait::async_trait;
use serde_json::Value;
use tools_core::{Tool, RoutingContext};

#[async_trait]
pub trait DelegationHandler: Send + Sync {
    /// Delegate a query to another agent, running synchronously.
    /// Returns the agent's response as a string.
    async fn delegate(
        &self,
        agent_name: &str,
        query: &str,
        ctx: &RoutingContext,
        depth: u32,
    ) -> common::Result<String>;
}

pub struct DelegationTool {
    handler: Option<Arc<dyn DelegationHandler>>,
    allowed_agents: Vec<String>, // from AgentProfile.can_delegate_to
    current_depth: u32,
    max_depth: u32,
}

impl DelegationTool {
    pub fn new() -> Self {
        Self { handler: None, allowed_agents: vec![], current_depth: 0, max_depth: 2 }
    }

    pub fn with_handler(handler: Arc<dyn DelegationHandler>) -> Self {
        Self { handler: Some(handler), allowed_agents: vec![], current_depth: 0, max_depth: 2 }
    }

    pub fn with_allowed_agents(mut self, agents: Vec<String>) -> Self {
        self.allowed_agents = agents;
        self
    }

    pub fn with_depth(mut self, current: u32, max: u32) -> Self {
        self.current_depth = current;
        self.max_depth = max;
        self
    }
}

#[async_trait]
impl Tool for DelegationTool {
    fn name(&self) -> &str { "delegate" }

    fn description(&self) -> &str {
        "Delegate a query to a specialist agent. Use when you need another agent's expertise."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "agent": {
                    "type": "string",
                    "description": "Name of the agent to delegate to",
                    "enum": self.allowed_agents,
                },
                "query": {
                    "type": "string",
                    "description": "The query or task to delegate"
                }
            },
            "required": ["agent", "query"]
        })
    }

    async fn execute(&self, args: Value, ctx: &RoutingContext) -> common::Result<String> {
        let handler = self.handler.as_ref()
            .ok_or_else(|| common::KlyntbotError::Tool("Delegation not available".into()))?;

        if self.current_depth >= self.max_depth {
            return Ok("Cannot delegate further — maximum delegation depth reached.".into());
        }

        let agent = args["agent"].as_str()
            .ok_or_else(|| common::KlyntbotError::Tool("Missing 'agent' parameter".into()))?;
        let query = args["query"].as_str()
            .ok_or_else(|| common::KlyntbotError::Tool("Missing 'query' parameter".into()))?;

        if !self.allowed_agents.contains(&agent.to_string()) {
            return Ok(format!("Cannot delegate to '{agent}' — not in allowed agents list."));
        }

        handler.delegate(agent, query, ctx, self.current_depth + 1).await
    }
}
```

**Step 3: Run tests**

```bash
cargo nextest run -p tools -E 'test(delegation)' --no-capture
```

**Step 4: Commit**

```bash
git add crates/tools/src/delegation.rs crates/tools/src/lib.rs
git commit -m "feat(tools): add DelegationHandler trait and DelegationTool"
```

---

### Task 10: Implement delegation in AgentRuntime

**Files:**
- Modify: `crates/agent/src/agent_runtime/runtime.rs` — implement `DelegationHandler`

**Step 1: Write the failing test**

```rust
#[tokio::test]
async fn test_delegation_runs_sub_agent() {
    // Build runtime with mock provider that returns a canned response
    // Call delegate("task", "list my tasks", ctx, 1)
    // Verify it runs with the task agent's profile
}
```

**Step 2: Implement DelegationHandler for AgentRuntime**

```rust
#[async_trait]
impl DelegationHandler for AgentRuntime {
    async fn delegate(
        &self,
        agent_name: &str,
        query: &str,
        ctx: &RoutingContext,
        depth: u32,
    ) -> common::Result<String> {
        let profile = self.agent_manager.get(agent_name)
            .ok_or_else(|| common::KlyntbotError::Tool(format!("Agent '{agent_name}' not found")))?;

        // Build a mini system prompt from the delegated agent's profile
        // Create a reduced-budget ExecutionCore + ReactiveEngine
        // Run with max_iterations = min(profile.max_iterations, 5)
        // DelegationTool for the sub-agent has depth + 1 and the sub-agent's can_delegate_to
        // Return the final response text
    }
}
```

**Step 3: Wire DelegationTool into AgentRuntime::process_message**

In the tool filtering step, if `profile.can_delegate_to` is non-empty and delegation depth < max:
```rust
// Create DelegationTool with this runtime as handler
let delegation_tool = DelegationTool::with_handler(Arc::new(self.clone()))
    .with_allowed_agents(profile.can_delegate_to.clone())
    .with_depth(current_depth, 2);
// Add to filtered tools
```

**Step 4: Run tests**

```bash
cargo nextest run -p agent -E 'test(delegation)' --no-capture
```

**Step 5: Commit**

```bash
git add crates/agent/src/agent_runtime/
git commit -m "feat(agent): implement delegation in AgentRuntime"
```

---

### Task 11: Evolve SubagentManager for auto-async

**Files:**
- Modify: `crates/agent/src/subagent.rs` — add auto-async logic
- Modify: `crates/agent/src/agent_runtime/runtime.rs` — add auto-async check

**Step 1: Add auto-async check to AgentRuntime::process_message**

After intent analysis (step 2), check complexity signals:
```rust
// Auto-async: if estimated_tool_calls > 8 or complexity is high, run async
if analysis.should_run_async() {
    let immediate_response = format!(
        "I'll work on this and message you when I have results."
    );
    // Spawn async using SubagentManager with the current agent profile
    self.spawn_async(profile, message, ctx).await;
    return Ok(PipelineResult { content: immediate_response, ... });
}
```

**Step 2: Make SubagentManager profile-aware**

Currently `SubagentManager::run_subagent_task()` builds its own tool registry. Update it to accept an `AgentProfile` and use the profile's tool list + skills.

**Step 3: Add `should_run_async` to IntentAnalysis**

```rust
impl IntentAnalysis {
    pub fn should_run_async(&self) -> bool {
        match &self.mode {
            ExecutionMode::Reactive { max_iterations } => {
                self.complexity.estimated_tool_calls > 8
                    || *max_iterations > 12
                    || self.complexity.failure_risk == "high"
            }
            _ => false,
        }
    }
}
```

**Step 4: Run tests**

```bash
cargo nextest run --workspace --no-capture
```

**Step 5: Commit**

```bash
git add crates/agent/
git commit -m "feat(agent): add auto-async execution based on complexity signals"
```

---

## Phase 4: Unified Learning System

### Task 12: Create new learning tables

**Files:**
- Create: `crates/storage/migrations/NNN_learning_system.sql` (next migration number)

**Step 1: Write migration**

```sql
-- User profile: explicit facts about the user
CREATE TABLE IF NOT EXISTS user_profile (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    category TEXT NOT NULL,
    key TEXT NOT NULL,
    value TEXT NOT NULL DEFAULT '{}',
    source TEXT NOT NULL DEFAULT 'user_explicit',
    confidence REAL NOT NULL DEFAULT 1.0,
    agent_name TEXT,
    last_confirmed TEXT NOT NULL DEFAULT (datetime('now')),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(category, key)
);

-- Behavioral patterns: observed from interactions
CREATE TABLE IF NOT EXISTS behavioral_patterns (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    pattern_type TEXT NOT NULL,
    pattern_key TEXT NOT NULL,
    pattern_value TEXT NOT NULL DEFAULT '{}',
    sample_count INTEGER NOT NULL DEFAULT 0,
    last_updated TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(pattern_type, pattern_key)
);

-- Agent adaptations: per-agent user preferences
CREATE TABLE IF NOT EXISTS agent_adaptations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    agent_name TEXT NOT NULL,
    preference_key TEXT NOT NULL,
    preference_value TEXT NOT NULL DEFAULT '{}',
    source TEXT NOT NULL DEFAULT 'satisfaction_signal',
    confidence REAL NOT NULL DEFAULT 0.5,
    last_updated TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(agent_name, preference_key)
);

-- Interaction log: raw data for pattern analysis
CREATE TABLE IF NOT EXISTS interaction_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL DEFAULT (datetime('now')),
    agent_name TEXT NOT NULL,
    tool_names TEXT NOT NULL DEFAULT '[]',
    channel TEXT NOT NULL,
    duration_ms INTEGER
);
```

**Step 2: Run migration test**

```bash
cargo nextest run -p storage --no-capture
```
Expected: Migration runs, tables created.

**Step 3: Commit**

```bash
git add crates/storage/migrations/
git commit -m "feat(storage): add learning system tables"
```

---

### Task 13: Build learning repos

**Files:**
- Create: `crates/storage/src/repos/user_profile.rs`
- Create: `crates/storage/src/repos/behavioral_pattern.rs`
- Create: `crates/storage/src/repos/agent_adaptation.rs`
- Create: `crates/storage/src/repos/interaction_log.rs`
- Modify: `crates/storage/src/repos/mod.rs` — add to Repos aggregate

**Step 1: Write failing tests for UserProfileRepo**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::StoragePool;

    #[tokio::test]
    async fn test_upsert_and_get() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = UserProfileRepo::new(pool.get_pool().clone());

        repo.upsert("projects", "active_project", &serde_json::json!("Project X"), "user_explicit", 1.0, None).await.unwrap();
        let entry = repo.get("projects", "active_project").await.unwrap();
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().value, serde_json::json!("Project X"));
    }

    #[tokio::test]
    async fn test_list_by_category() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = UserProfileRepo::new(pool.get_pool().clone());

        repo.upsert("preferences", "timezone", &serde_json::json!("EST"), "user_explicit", 1.0, None).await.unwrap();
        repo.upsert("preferences", "language", &serde_json::json!("en"), "user_explicit", 1.0, None).await.unwrap();

        let entries = repo.list_by_category("preferences").await.unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[tokio::test]
    async fn test_list_high_confidence() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = UserProfileRepo::new(pool.get_pool().clone());

        repo.upsert("habits", "morning_routine", &serde_json::json!("coffee"), "system_inferred", 0.3, None).await.unwrap();
        repo.upsert("habits", "finance_day", &serde_json::json!("friday"), "system_inferred", 0.8, None).await.unwrap();

        let entries = repo.list_above_confidence(0.5).await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].key, "finance_day");
    }
}
```

**Step 2: Implement repos following the existing pattern**

Follow the same pattern as `MemoryNoteRepo` and `OutcomeRepo` — struct with `SqlitePool`, async CRUD methods, sqlx queries.

**Step 3: Add to Repos aggregate**

In `crates/storage/src/repos/mod.rs`, add new repo fields to `Repos` struct and `Repos::from_pool()`.

**Step 4: Run tests**

```bash
cargo nextest run -p storage --no-capture
```

**Step 5: Commit**

```bash
git add crates/storage/src/repos/
git commit -m "feat(storage): add repos for user_profile, behavioral_patterns, agent_adaptations, interaction_log"
```

---

### Task 14: Build LearningContextSource

**Files:**
- Create: `crates/agent/src/context_sources/learning.rs`
- Modify: `crates/agent/src/context_sources/mod.rs`
- Test: inline

**Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_learning_context_includes_user_profile() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repos = Repos::from_pool(&pool);

        repos.user_profile.upsert("projects", "active_project",
            &serde_json::json!("Klyntbot"), "user_explicit", 1.0, None).await.unwrap();

        let source = LearningContextSource::new(repos.user_profile.clone(), /* ... */);
        let ctx = SourceContext { channel: "test".into(), chat_id: "1".into(), message: None };
        let result = source.provide(&ctx).await;

        assert!(result.is_some());
        let text = result.unwrap();
        assert!(text.contains("Klyntbot"));
        assert!(text.contains("About the User"));
    }
}
```

**Step 2: Implement LearningContextSource**

Replaces `MemorySource` (priority 80) + `ConfidenceSource` (priority 50):

```rust
pub struct LearningContextSource {
    user_profile_repo: UserProfileRepo,
    pattern_repo: BehavioralPatternRepo,
    adaptation_repo: AgentAdaptationRepo,
    confidence_bits: Arc<AtomicU32>,
    conversation_memory: Option<Arc<MemoryStore>>, // kept for backward compat
    active_agent: Arc<RwLock<Option<String>>>,     // current agent name
    cache: Mutex<Option<CachedLearning>>,
}

impl ContextSource for LearningContextSource {
    fn name(&self) -> &str { "learning" }
    fn priority(&self) -> u8 { 60 }

    async fn provide(&self, ctx: &SourceContext) -> Option<String> {
        // 1. User profile (high confidence entries)
        // 2. Behavioral patterns (reliable patterns)
        // 3. Agent preferences (for current agent)
        // 4. Confidence threshold
        // 5. Conversation memory (ANN recall, if embeddings enabled)
        // Format as structured sections, cache with 60s TTL
    }
}
```

**Step 3: Run tests**

```bash
cargo nextest run -p agent -E 'test(learning_context)' --no-capture
```

**Step 4: Commit**

```bash
git add crates/agent/src/context_sources/learning.rs
git commit -m "feat(agent): add LearningContextSource replacing MemorySource + ConfidenceSource"
```

---

### Task 15: Build interaction recorder and pattern analyzer

**Files:**
- Create: `crates/agent/src/learning/interaction_recorder.rs`
- Create: `crates/agent/src/learning/pattern_analyzer.rs`
- Modify: `crates/agent/src/learning/mod.rs`

**Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_record_interaction() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let recorder = InteractionRecorder::new(pool.repos().interaction_log.clone());
        recorder.record("task", &["task", "memory"], "telegram", 150).await.unwrap();
        // Verify it was stored
    }

    #[tokio::test]
    async fn test_pattern_analysis_detects_day_of_week() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let log_repo = pool.repos().interaction_log.clone();
        let pattern_repo = pool.repos().behavioral_patterns.clone();

        // Insert 15 interactions on Mondays with agent="task"
        for _ in 0..15 {
            log_repo.create("task", &["task"], "telegram", Some(100),
                DateTime::parse_from_rfc3339("2026-03-02T10:00:00Z").unwrap().into() // Monday
            ).await.unwrap();
        }

        let analyzer = PatternAnalyzer::new(log_repo, pattern_repo);
        analyzer.analyze().await.unwrap();

        let patterns = pool.repos().behavioral_patterns
            .list_by_type("day_of_week").await.unwrap();
        assert!(!patterns.is_empty());
    }
}
```

**Step 2: Implement InteractionRecorder**

Lightweight — no LLM call, just writes to `interaction_log` table after each message:

```rust
pub struct InteractionRecorder {
    repo: InteractionLogRepo,
}

impl InteractionRecorder {
    pub async fn record(&self, agent: &str, tools: &[&str], channel: &str, duration_ms: u64) {
        self.repo.create(agent, tools, channel, Some(duration_ms as i64), Utc::now()).await.ok();
    }
}
```

**Step 3: Implement PatternAnalyzer**

Runs hourly in the LearningService background loop:

```rust
pub struct PatternAnalyzer {
    log_repo: InteractionLogRepo,
    pattern_repo: BehavioralPatternRepo,
}

impl PatternAnalyzer {
    pub async fn analyze(&self) -> common::Result<()> {
        let logs = self.log_repo.list_recent(1000).await?;

        // Analyze day-of-week patterns
        self.analyze_day_of_week(&logs).await?;
        // Analyze time-of-day patterns
        self.analyze_time_of_day(&logs).await?;
        // Analyze agent usage frequency
        self.analyze_agent_usage(&logs).await?;

        Ok(())
    }
}
```

**Step 4: Run tests**

```bash
cargo nextest run -p agent -E 'test(interaction_recorder|pattern_analyzer)' --no-capture
```

**Step 5: Commit**

```bash
git add crates/agent/src/learning/
git commit -m "feat(agent): add interaction recorder and behavioral pattern analyzer"
```

---

### Task 16: Wire OutcomeRecorder into ExecutionCore

**Files:**
- Modify: `crates/agent/src/execution/core.rs` — add outcome recording after tool execution
- Modify: `crates/agent/src/agent_loop/builder.rs` — inject OutcomeRecorder into ExecutionCore

**Step 1: Add OutcomeRecorder field to ExecutionCore**

```rust
pub struct ExecutionCore {
    pub provider: DynProvider,
    pub tool_registry: Arc<RwLock<ToolRegistry>>,
    pub outcome_recorder: Option<Arc<crate::learning::OutcomeRecorder>>, // new
}
```

**Step 2: Record outcomes after each tool execution**

In `run_cycle()`, after the `join_all` for tool execution (around line 519), add:

```rust
// Record tool outcomes for learning
if let Some(recorder) = &self.outcome_recorder {
    for result in &tool_results {
        recorder.record_tool_outcome(
            &result.tool_name,
            result.success,
            result.error_category.as_deref(),
            result.duration_ms,
            None, // confidence assessment (future: pass from pre-execution check)
            crate::learning::types::ExecutionMode::Chat,
            &routing_ctx.session_key(),
        ).await;
    }
}
```

**Step 3: Wire in builder**

In `builder.rs`, pass `Arc<OutcomeRecorder>` to ExecutionCore during construction.

**Step 4: Run tests**

```bash
cargo nextest run --workspace --no-capture
```

**Step 5: Commit**

```bash
git add crates/agent/
git commit -m "feat(agent): wire OutcomeRecorder into ExecutionCore for per-tool learning"
```

---

### Task 17: Wire InteractionRecorder into AgentRuntime

**Files:**
- Modify: `crates/agent/src/agent_runtime/runtime.rs` — record interactions after each message

**Step 1: Add InteractionRecorder to AgentRuntime**

```rust
pub struct AgentRuntime {
    // ... existing fields ...
    interaction_recorder: Option<InteractionRecorder>,
}
```

**Step 2: Record after process_message**

At the end of `process_message`, after the pipeline result:

```rust
// Record interaction for pattern learning
if let Some(recorder) = &self.interaction_recorder {
    let tools_used: Vec<&str> = result.tools_used.iter().map(|s| s.as_str()).collect();
    recorder.record(
        &selected_agent.name,
        &tools_used,
        &ctx.channel.to_string(),
        pipeline_duration_ms,
    ).await;
}
```

**Step 3: Expand LearningService to run PatternAnalyzer**

In `learning/service.rs`, add pattern analysis to the background loop after the existing analysis:

```rust
// Existing: run threshold analysis
self.run_analysis().await;
// New: run pattern analysis
if let Some(analyzer) = &self.pattern_analyzer {
    analyzer.analyze().await.ok();
}
```

**Step 4: Run tests**

```bash
cargo nextest run --workspace --no-capture
```

**Step 5: Commit**

```bash
git add crates/agent/
git commit -m "feat(agent): wire InteractionRecorder into AgentRuntime and LearningService"
```

---

### Task 18: Replace MemorySource + ConfidenceSource with LearningContextSource

**Files:**
- Modify: `crates/agent/src/agent_loop/builder.rs` — swap context sources
- Delete or deprecate: `crates/agent/src/context_sources/memory.rs`
- Delete or deprecate: `crates/agent/src/context_sources/confidence.rs`

**Step 1: Replace in builder**

Remove the `MemorySource` and `ConfidenceSource` registrations. Add `LearningContextSource`:

```rust
// Old (remove):
// Box::new(MemorySource::new(memory_store.clone())),
// Box::new(ConfidenceSource::new(initial_threshold)),

// New:
Box::new(LearningContextSource::new(
    repos.user_profile.clone(),
    repos.behavioral_patterns.clone(),
    repos.agent_adaptations.clone(),
    confidence_handle.clone(),
    Some(memory_store.clone()), // keep conversation memory
    active_agent_name.clone(),
)),
```

**Step 2: Evolve MemoryTool to write UserProfile**

The existing `MemoryTool` actions for `search_conversations` and `search_all` stay. Add a new action `update_profile` that writes to `user_profile` table:

```rust
"update_profile" => {
    let category = args["category"].as_str().unwrap_or("context");
    let key = args["key"].as_str().ok_or("Missing key")?;
    let value = &args["value"];
    self.user_profile_repo.upsert(category, key, value, "agent_observed", 0.8, agent_name).await?;
    Ok(format!("Updated profile: {category}/{key}"))
}
```

**Step 3: Run full test suite**

```bash
cargo nextest run --workspace --no-capture
cargo clippy --workspace --all-targets --all-features
```

**Step 4: Commit**

```bash
git add -A
git commit -m "feat(agent): replace MemorySource + ConfidenceSource with unified LearningContextSource"
```

---

### Task 19: Clean up old skills/ directory and update CLAUDE.md

**Files:**
- Delete: `skills/` directory (old SKILL.md files, now in `agents/`)
- Modify: `CLAUDE.md` — update architecture docs
- Modify: `crates/agent/src/skills.rs` — delete or gut (if fully replaced)

**Step 1: Remove old skills directory**

The skill content has been moved to `agents/*/skills/`. Delete the old `skills/` directory.

**Step 2: Remove old BUILTIN_SKILLS const**

If `skills.rs` is still referenced anywhere, remove it. If not, delete the file.

**Step 3: Update CLAUDE.md**

Update the architecture section to reflect:
- `AgentRuntime` replaces `IntentPipeline`
- `AgentManager` replaces `SkillManager`
- Agent profiles in `agents/` directory
- Unified Learning System
- Delegation + auto-async

**Step 4: Run full test suite and clippy**

```bash
cargo build --workspace
cargo nextest run --workspace
cargo clippy --workspace --all-targets --all-features
cargo fmt --all --check
```

**Step 5: Commit**

```bash
git add -A
git commit -m "refactor: clean up old skills system, update CLAUDE.md for agent-driven architecture"
```

---

## Verification Checklist

After all phases are complete, verify:

- [ ] `cargo build --workspace` succeeds
- [ ] `cargo nextest run --workspace` — all tests pass
- [ ] `cargo clippy --workspace --all-targets --all-features` — zero warnings
- [ ] `cargo fmt --all --check` — formatted
- [ ] `cargo build --no-default-features` — builds without email feature
- [ ] Agent matching works: "create a task" → TaskAgent, "hello" → GeneralAgent
- [ ] Delegation works: TaskAgent can delegate to CalendarAgent
- [ ] Auto-async triggers for high-complexity requests
- [ ] Learning context shows user profile data in system prompt
- [ ] OutcomeRecorder writes to learning_outcomes table after tool execution
- [ ] Interaction patterns are detected after sufficient interactions
