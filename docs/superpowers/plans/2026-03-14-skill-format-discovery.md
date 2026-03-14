# Skill Format & Discovery Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace Klyntbot's hardcoded agent/skill system with a runtime-discoverable, Agent Skills spec-compatible skill system.

**Architecture:** New `skill-system` crate at L3 provides `SkillPackage`, `SkillCatalog`, `SkillRouter`, and `SkillContextSource`. Built-in skills are compiled via `include_str!` from a new `skills/` directory (replacing `agents/`). User-installed and project-level skills are discovered at runtime from `{data_dir}/skills/` and `.agents/skills/`. The `agent` crate consumes `skill-system` types, wiring them into the existing `AgentRuntime` pipeline.

**Tech Stack:** Rust, serde_yaml, tokio::fs, async-trait, context_engine::ContextSource

**Spec:** `docs/superpowers/specs/2026-03-14-skill-format-discovery-design.md`

---

## File Structure

### New files (crates/skill-system/)

| File | Responsibility |
|------|---------------|
| `crates/skill-system/Cargo.toml` | Crate manifest — depends on `common`, `config`, `serde`, `serde_yaml`, `tokio`, `async-trait`, `context_engine` |
| `crates/skill-system/src/lib.rs` | Re-exports all public types |
| `crates/skill-system/src/types.rs` | `SkillType`, `SkillScope`, `SkillPackage`, `SkillMetadata`, `KlyntbotMeta`, `SkillManifest`, `SkillCatalog`, `SkillChange`, `EmbedFn` |
| `crates/skill-system/src/parser.rs` | SKILL.md frontmatter parsing (`split_frontmatter`, `parse_skill_md`) — extracts from `agent_profile/types.rs` |
| `crates/skill-system/src/discovery.rs` | `SkillCatalog::discover()` — scanning built-in + user + project scopes |
| `crates/skill-system/src/router.rs` | `SkillRouter` — keyword scoring, semantic matching, orchestrator selection, per-message skill activation |
| `crates/skill-system/src/context.rs` | `SkillContextSource` — replaces `AgentContextSource`, injects orchestrator + always_skills + activated skills |
| `crates/skill-system/src/manifest.rs` | `SkillManifest` parsing from `manifest.json` (stub for Subsystem 2) |

### New files (skills/ directory — replaces agents/)

| File | Migrated from |
|------|--------------|
| `skills/general/SKILL.md` | `agents/general/AGENT.md` |
| `skills/general/references/{search,skill-creator,browser,memory,summarize}.md` | `agents/general/skills/*.md` |
| `skills/task-management/SKILL.md` | `agents/task/AGENT.md` |
| `skills/task-management/references/{todo,daily-planner,weekly-review,task-decompose,project-management,retrospective}.md` | `agents/task/skills/*.md` |
| `skills/finance-management/SKILL.md` | `agents/finance/AGENT.md` |
| `skills/finance-management/references/{spending-analysis,budgeting}.md` | `agents/finance/skills/*.md` |
| `skills/automation/SKILL.md` | `agents/automation/AGENT.md` |
| `skills/automation/references/cron.md` | `agents/automation/skills/cron.md` |
| `skills/communication/SKILL.md` | `agents/communication/AGENT.md` |
| `skills/communication/references/{messaging,notification}.md` | `agents/communication/skills/*.md` |

### Modified files

| File | Changes |
|------|---------|
| `Cargo.toml` (workspace root) | Add `crates/skill-system` to members + workspace deps |
| `crates/config/src/schema/agents.rs` | Add `SkillConfig` (discovery paths, trust settings, thresholds) |
| `crates/config/src/schema/core.rs` | Add `skills: SkillConfig` field, add top-level `project_root: Option<String>` field |
| `crates/agent/Cargo.toml` | Add `skill-system` dependency |
| `crates/agent/src/agent_loop/builder.rs` | Wire `SkillCatalog` + `SkillRouter` instead of `AgentManager` |
| `crates/agent/src/agent_runtime/runtime.rs` | Replace `AgentManager` usage with `SkillCatalog` + `SkillRouter` |
| `crates/app-core/Cargo.toml` | Add `skill-system` dependency |
| `crates/app-core/src/init/mod.rs` | Initialize `SkillCatalog` instead of `AgentManager` |
| `crates/app-core/src/handlers/agents.rs` | Replace `builtin_agents()` calls with `SkillCatalog` API, update `reload_agents()` to use `SkillCatalog::reload()` |
| `crates/desktop-shared/src/commands/agents.rs` | Replace `AgentProfileSummary`/`AgentFileSummary`/`AgentFileContent` with skill equivalents |
| `crates/desktop/src/commands/agents.rs` | Update Tauri commands to use skill types |

### Deleted files

| File | Reason |
|------|--------|
| `crates/agent/src/agent_profile/types.rs` | Replaced by `skill-system/types.rs` + `skill-system/parser.rs` |
| `crates/agent/src/agent_profile/manager.rs` | Replaced by `skill-system/discovery.rs` + `skill-system/router.rs` |
| `crates/agent/src/agent_profile/skill_loader.rs` | Subsumed by `discovery.rs` |
| `crates/agent/src/agent_profile/mod.rs` | Module deleted |
| `crates/agent/src/context_sources/agent.rs` | Replaced by `skill-system/context.rs` |
| `crates/agent/src/content_registry/` | All 4 files — subsumed by `SkillCatalog` |
| `agents/` | Entire directory — replaced by `skills/` |

---

## Chunk 1: Crate Scaffold + Core Types

### Task 1: Create skill-system crate with Cargo.toml

**Files:**
- Create: `crates/skill-system/Cargo.toml`
- Create: `crates/skill-system/src/lib.rs`
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Create Cargo.toml for skill-system crate**

```toml
# crates/skill-system/Cargo.toml
[package]
name = "skill-system"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
common.workspace = true
config.workspace = true
context_engine.workspace = true
serde.workspace = true
serde_json.workspace = true
serde_yaml.workspace = true
tokio.workspace = true
async-trait.workspace = true
tracing.workspace = true
chrono.workspace = true
```

- [ ] **Step 2: Create empty lib.rs**

```rust
// crates/skill-system/src/lib.rs
pub mod types;
```

- [ ] **Step 3: Register crate in workspace Cargo.toml**

In workspace root `Cargo.toml`:
- Add `"crates/skill-system"` to `[workspace] members` array (after `crates/scheduling`)
- Add `skill-system = { path = "crates/skill-system" }` to `[workspace.dependencies]` (after `scheduling`)

- [ ] **Step 4: Verify it compiles**

Run: `cargo build -p skill-system`
Expected: Compiles with 0 errors

- [ ] **Step 5: Commit**

```bash
git add crates/skill-system/ Cargo.toml
git commit -m "feat(skill-system): scaffold new crate at L3"
```

### Task 2: Define core types (SkillPackage, SkillMetadata, KlyntbotMeta)

**Files:**
- Create: `crates/skill-system/src/types.rs`
- Test: inline `#[cfg(test)] mod tests`

- [ ] **Step 1: Write tests for core types**

```rust
// crates/skill-system/src/types.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_type_default_is_skill() {
        assert!(matches!(SkillType::default(), SkillType::Skill));
    }

    #[test]
    fn test_klyntbot_meta_tools_none_means_all_allowed() {
        let meta = KlyntbotMeta::default();
        assert!(meta.tools.is_none(), "None = all tools allowed");
    }

    #[test]
    fn test_klyntbot_meta_tools_empty_means_deny_all() {
        let meta = KlyntbotMeta {
            tools: Some(vec![]),
            ..Default::default()
        };
        assert_eq!(meta.tools.as_ref().unwrap().len(), 0, "Some([]) = deny all");
    }

    #[test]
    fn test_skill_package_allowed_tool_names_none_means_full_access() {
        let pkg = SkillPackage {
            name: "test".into(),
            description: "test".into(),
            skill_type: SkillType::Skill,
            scope: SkillScope::BuiltIn,
            location: PathBuf::new(),
            body: String::new(),
            manifest: None,
            metadata: SkillMetadata::default(),
            loaded_at: SystemTime::now(),
            trusted: true,
        };
        assert!(pkg.allowed_tool_names().is_none());
    }

    #[test]
    fn test_skill_package_allowed_tool_names_explicit_list() {
        let pkg = SkillPackage {
            metadata: SkillMetadata {
                klyntbot: Some(KlyntbotMeta {
                    tools: Some(vec!["tasks".into(), "notes".into()]),
                    ..Default::default()
                }),
                ..Default::default()
            },
            name: "test".into(),
            description: "test".into(),
            skill_type: SkillType::Skill,
            scope: SkillScope::BuiltIn,
            location: PathBuf::new(),
            body: String::new(),
            manifest: None,
            loaded_at: SystemTime::now(),
            trusted: true,
        };
        let allowed = pkg.allowed_tool_names().unwrap();
        assert!(allowed.contains("tasks"));
        assert!(allowed.contains("notes"));
        assert!(allowed.contains("ask_user"));
        assert!(!allowed.contains("finance"));
    }

    #[test]
    fn test_allows_mcp_server_wildcard() {
        let pkg = SkillPackage {
            metadata: SkillMetadata {
                klyntbot: Some(KlyntbotMeta {
                    mcp_tools: vec!["*".into()],
                    ..Default::default()
                }),
                ..Default::default()
            },
            name: "t".into(), description: "t".into(),
            skill_type: SkillType::Skill, scope: SkillScope::BuiltIn,
            location: PathBuf::new(), body: String::new(),
            manifest: None, loaded_at: SystemTime::now(), trusted: true,
        };
        assert!(pkg.allows_mcp_server("anything"));
    }

    #[test]
    fn test_allows_mcp_server_empty_denies() {
        let pkg = SkillPackage {
            metadata: SkillMetadata::default(),
            name: "t".into(), description: "t".into(),
            skill_type: SkillType::Skill, scope: SkillScope::BuiltIn,
            location: PathBuf::new(), body: String::new(),
            manifest: None, loaded_at: SystemTime::now(), trusted: true,
        };
        assert!(!pkg.allows_mcp_server("linear"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p skill-system`
Expected: FAIL — types not defined

- [ ] **Step 3: Implement core types**

```rust
// crates/skill-system/src/types.rs
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use serde::Deserialize;

const MCP_WILDCARD: &str = "*";
const ASK_USER_TOOL_NAME: &str = "ask_user";

/// Callback type for embedding text. Avoids depending on cognitive::TextEmbedder.
pub type EmbedFn = Arc<dyn Fn(&str) -> common::Result<Vec<f32>> + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SkillType {
    #[default]
    Skill,
    Orchestrator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillScope {
    BuiltIn,
    User,
    Project,
}

#[derive(Debug, Clone)]
pub struct SkillPackage {
    pub name: String,
    pub description: String,
    pub skill_type: SkillType,
    pub scope: SkillScope,
    pub location: PathBuf,
    pub body: String,
    pub manifest: Option<SkillManifest>,
    pub metadata: SkillMetadata,
    pub loaded_at: SystemTime,
    pub trusted: bool,
}

impl SkillPackage {
    /// Returns None if all tools allowed (tools field omitted/null).
    /// Returns Some(set) with ask_user always included when tools is explicit.
    pub fn allowed_tool_names(&self) -> Option<HashSet<String>> {
        let tools = self.metadata.klyntbot.as_ref()?.tools.as_ref()?;
        let mut set: HashSet<String> = tools.iter().cloned().collect();
        set.insert(ASK_USER_TOOL_NAME.to_string());
        Some(set)
    }

    /// Check if this skill allows tools from the given MCP server name.
    pub fn allows_mcp_server(&self, server_name: &str) -> bool {
        self.metadata
            .klyntbot
            .as_ref()
            .map(|k| {
                k.mcp_tools
                    .iter()
                    .any(|s| s == MCP_WILDCARD || s == server_name)
            })
            .unwrap_or(false)
    }

    /// Max iterations for ReAct loop. Falls back to default 10.
    pub fn max_iterations(&self) -> u32 {
        self.metadata
            .klyntbot
            .as_ref()
            .and_then(|k| k.max_iterations)
            .unwrap_or(10)
    }

    /// Skills to delegate to (orchestrator only).
    pub fn can_delegate_to(&self) -> &[String] {
        self.metadata
            .klyntbot
            .as_ref()
            .map(|k| k.can_delegate_to.as_slice())
            .unwrap_or(&[])
    }

    /// Always-loaded reference file names (resolved to references/<name>.md).
    pub fn always_skills(&self) -> &[String] {
        self.metadata
            .klyntbot
            .as_ref()
            .map(|k| k.always_skills.as_slice())
            .unwrap_or(&[])
    }
}

#[derive(Debug, Clone, Default)]
pub struct SkillMetadata {
    pub license: Option<String>,
    pub compatibility: Option<String>,
    pub custom: HashMap<String, serde_json::Value>,
    pub klyntbot: Option<KlyntbotMeta>,
}

#[derive(Debug, Clone, Default)]
pub struct KlyntbotMeta {
    pub skill_type: SkillType,
    pub tools: Option<Vec<String>>,
    pub mcp_tools: Vec<String>,
    pub can_delegate_to: Vec<String>,
    pub max_iterations: Option<u32>,
    pub always_skills: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct SkillManifest {
    pub schema_version: String,
    pub entities: HashMap<String, serde_json::Value>, // Stub for Subsystem 2
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum SkillChange {
    Added(String),
    Removed(String),
    Updated(String),
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo nextest run -p skill-system`
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/skill-system/src/types.rs
git commit -m "feat(skill-system): define core types — SkillPackage, SkillMetadata, KlyntbotMeta"
```

### Task 3: Implement SKILL.md parser

**Files:**
- Create: `crates/skill-system/src/parser.rs`
- Modify: `crates/skill-system/src/lib.rs`

- [ ] **Step 1: Write tests for SKILL.md parsing**

```rust
// crates/skill-system/src/parser.rs
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
        let pkg = parse_skill_md(content, PathBuf::from("skills/task-management"), SkillScope::BuiltIn)
            .unwrap();
        assert_eq!(pkg.name, "task-management");
        assert!(pkg.description.contains("OKR+PARA"));
        assert_eq!(pkg.skill_type, SkillType::Orchestrator);
        assert_eq!(pkg.metadata.klyntbot.as_ref().unwrap().tools, Some(vec!["tasks".into(), "project".into(), "area".into()]));
        assert_eq!(pkg.metadata.klyntbot.as_ref().unwrap().can_delegate_to, vec!["finance-management"]);
        assert_eq!(pkg.metadata.klyntbot.as_ref().unwrap().max_iterations, Some(12));
        assert_eq!(pkg.metadata.klyntbot.as_ref().unwrap().always_skills, vec!["todo", "daily-planner"]);
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
        let pkg = parse_skill_md(content, PathBuf::from("skills/search"), SkillScope::User).unwrap();
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
        let pkg = parse_skill_md(content, PathBuf::from("skills/minimal"), SkillScope::BuiltIn).unwrap();
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
        assert!(!pkg.metadata.custom.contains_key("klyntbot"), "klyntbot should be excluded from custom");
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
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p skill-system`
Expected: FAIL — parser functions not defined

- [ ] **Step 3: Implement parser**

```rust
// crates/skill-system/src/parser.rs
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
}

pub fn parse_skill_md(
    content: &str,
    location: PathBuf,
    scope: SkillScope,
) -> common::Result<SkillPackage> {
    let (frontmatter_str, body) = split_frontmatter(content)?;
    let raw: RawFrontmatter = serde_yaml::from_str(&frontmatter_str)
        .map_err(|e| common::ConfigError::Invalid(format!("SKILL.md frontmatter: {e}")))?;

    if raw.description.is_empty() {
        return Err(common::ConfigError::Invalid(format!(
            "Skill '{}' has no description (required by Agent Skills spec)",
            raw.name
        ))
        .into());
    }

    // Parse metadata block
    let (klyntbot_meta, custom) = parse_metadata_block(raw.metadata);

    let skill_type = klyntbot_meta
        .as_ref()
        .map(|k| k.skill_type)
        .unwrap_or_default();

    Ok(SkillPackage {
        name: raw.name,
        description: raw.description,
        skill_type,
        scope,
        location,
        body: body.trim().to_string(),
        manifest: None,
        metadata: SkillMetadata {
            license: raw.license,
            compatibility: raw.compatibility,
            custom,
            klyntbot: klyntbot_meta,
        },
        loaded_at: SystemTime::now(),
        trusted: matches!(scope, SkillScope::BuiltIn | SkillScope::User),
    })
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
        })
    });

    (klyntbot, map)
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
```

- [ ] **Step 4: Add `pub mod parser;` to lib.rs**

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo nextest run -p skill-system`
Expected: All tests pass

- [ ] **Step 6: Commit**

```bash
git add crates/skill-system/src/parser.rs crates/skill-system/src/lib.rs
git commit -m "feat(skill-system): implement SKILL.md parser with frontmatter extraction"
```

---

## Chunk 2: Discovery + SkillCatalog

### Task 4: Implement SkillCatalog discovery

**Files:**
- Create: `crates/skill-system/src/discovery.rs`
- Modify: `crates/skill-system/src/lib.rs`

- [ ] **Step 1: Write tests for built-in skill discovery**

```rust
// crates/skill-system/src/discovery.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_source_produces_skills() {
        let builtin = vec![
            ("general", "---\nname: general\ndescription: General assistant\nmetadata:\n  klyntbot:\n    type: orchestrator\n---\nInstructions."),
            ("search", "---\nname: search\ndescription: Web search.\n---\nSearch instructions."),
        ];
        let source = SkillSource::BuiltIn(
            builtin.iter().map(|(n, c)| (n.to_string(), c.to_string())).collect()
        );
        let catalog = SkillCatalog::discover_sync(&[source]).unwrap();
        assert_eq!(catalog.skills.len(), 2);
        assert!(catalog.get("general").is_some());
        assert!(catalog.get("search").is_some());
        assert_eq!(catalog.get("general").unwrap().skill_type, SkillType::Orchestrator);
    }

    #[test]
    fn test_higher_scope_shadows_lower() {
        let builtin = vec![
            ("search", "---\nname: search\ndescription: Built-in search.\n---\nBuiltin body."),
        ];
        let user = vec![
            ("search", "---\nname: search\ndescription: User search override.\n---\nUser body."),
        ];
        let sources = vec![
            SkillSource::BuiltIn(builtin.iter().map(|(n, c)| (n.to_string(), c.to_string())).collect()),
            SkillSource::Inline(user.iter().map(|(n, c)| (n.to_string(), c.to_string())).collect(), SkillScope::User),
        ];
        let catalog = SkillCatalog::discover_sync(&sources).unwrap();
        assert_eq!(catalog.skills.len(), 1);
        let pkg = catalog.get("search").unwrap();
        assert!(pkg.description.contains("User search override"));
        assert_eq!(pkg.scope, SkillScope::User);
    }

    #[test]
    fn test_catalog_prompt_xml() {
        let builtin = vec![
            ("general", "---\nname: general\ndescription: General assistant.\nmetadata:\n  klyntbot:\n    type: orchestrator\n---\nBody."),
            ("search", "---\nname: search\ndescription: Web search.\n---\nBody."),
        ];
        let source = SkillSource::BuiltIn(
            builtin.iter().map(|(n, c)| (n.to_string(), c.to_string())).collect()
        );
        let catalog = SkillCatalog::discover_sync(&[source]).unwrap();
        let prompt = catalog.catalog_prompt();
        assert!(prompt.contains("<available_skills>"));
        assert!(prompt.contains("name=\"general\""));
        assert!(prompt.contains("type=\"orchestrator\""));
        assert!(prompt.contains("name=\"search\""));
        assert!(prompt.contains("type=\"skill\""));
        assert!(prompt.contains("</available_skills>"));
    }

    #[test]
    fn test_orchestrators_and_regular_skills() {
        let builtin = vec![
            ("general", "---\nname: general\ndescription: General.\nmetadata:\n  klyntbot:\n    type: orchestrator\n---\nBody."),
            ("search", "---\nname: search\ndescription: Search.\n---\nBody."),
        ];
        let source = SkillSource::BuiltIn(
            builtin.iter().map(|(n, c)| (n.to_string(), c.to_string())).collect()
        );
        let catalog = SkillCatalog::discover_sync(&[source]).unwrap();
        assert_eq!(catalog.orchestrators().len(), 1);
        assert_eq!(catalog.regular_skills().len(), 1);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p skill-system`
Expected: FAIL — SkillCatalog not defined

- [ ] **Step 3: Implement SkillCatalog and discovery**

```rust
// crates/skill-system/src/discovery.rs
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;

use crate::parser::parse_skill_md;
use crate::types::*;

/// Where to discover skills from.
pub enum SkillSource {
    /// Built-in skills: Vec<(name, full SKILL.md content)>
    BuiltIn(Vec<(String, String)>),
    /// Filesystem directory to scan for skill subdirs.
    Directory(std::path::PathBuf, SkillScope),
    /// Inline test source: Vec<(name, content)> with a scope.
    #[cfg(test)]
    Inline(Vec<(String, String)>, SkillScope),
}

impl SkillCatalog {
    /// Async discovery — scans filesystem sources.
    pub async fn discover(sources: &[SkillSource]) -> common::Result<Self> {
        let mut skills: HashMap<String, Arc<SkillPackage>> = HashMap::new();

        for source in sources {
            match source {
                SkillSource::BuiltIn(entries) => {
                    for (name, content) in entries {
                        let location = std::path::PathBuf::from(format!("builtin::{name}"));
                        match parse_skill_md(content, location, SkillScope::BuiltIn) {
                            Ok(pkg) => { skills.insert(name.clone(), Arc::new(pkg)); }
                            Err(e) => { tracing::warn!(skill = %name, "Skipping built-in skill: {e}"); }
                        }
                    }
                }
                SkillSource::Directory(dir, scope) => {
                    Self::scan_directory(dir, *scope, &mut skills).await?;
                }
                #[cfg(test)]
                SkillSource::Inline(entries, scope) => {
                    for (name, content) in entries {
                        let location = std::path::PathBuf::from(format!("inline::{name}"));
                        match parse_skill_md(content, location, *scope) {
                            Ok(pkg) => {
                                if let Some(existing) = skills.get(name) {
                                    if scope_priority(*scope) > scope_priority(existing.scope) {
                                        tracing::info!(skill = %name, "Skill shadowed by higher-priority scope");
                                        skills.insert(name.clone(), Arc::new(pkg));
                                    }
                                } else {
                                    skills.insert(name.clone(), Arc::new(pkg));
                                }
                            }
                            Err(e) => { tracing::warn!(skill = %name, "Skipping: {e}"); }
                        }
                    }
                }
            }
        }

        Ok(Self {
            skills,
            embeddings: HashMap::new(),
            loaded_at: SystemTime::now(),
        })
    }

    /// Synchronous discovery for built-in-only sources (test helper).
    /// Only supports BuiltIn and Inline sources — panics on Directory.
    pub fn discover_sync(sources: &[SkillSource]) -> common::Result<Self> {
        Self::discover_sync_inner(sources)
    }

    fn discover_sync_inner(sources: &[SkillSource]) -> common::Result<Self> {
        let mut skills: HashMap<String, Arc<SkillPackage>> = HashMap::new();
        for source in sources {
            match source {
                SkillSource::BuiltIn(entries) => {
                    for (name, content) in entries {
                        let location = std::path::PathBuf::from(format!("builtin::{name}"));
                        match parse_skill_md(content, location, SkillScope::BuiltIn) {
                            Ok(pkg) => { skills.insert(name.clone(), Arc::new(pkg)); }
                            Err(e) => { tracing::warn!(skill = %name, "Skipping: {e}"); }
                        }
                    }
                }
                SkillSource::Directory(_, _) => {
                    return Err(common::ConfigError::Invalid(
                        "Directory sources require async discover()".into(),
                    ).into());
                }
                #[cfg(test)]
                SkillSource::Inline(entries, scope) => {
                    for (name, content) in entries {
                        let location = std::path::PathBuf::from(format!("inline::{name}"));
                        match parse_skill_md(content, location, *scope) {
                            Ok(pkg) => {
                                if let Some(existing) = skills.get(name) {
                                    if scope_priority(*scope) > scope_priority(existing.scope) {
                                        skills.insert(name.clone(), Arc::new(pkg));
                                    }
                                } else {
                                    skills.insert(name.clone(), Arc::new(pkg));
                                }
                            }
                            Err(e) => { tracing::warn!(skill = %name, "Skipping: {e}"); }
                        }
                    }
                }
            }
        }
        Ok(Self {
            skills,
            embeddings: HashMap::new(),
            loaded_at: SystemTime::now(),
        })
    }

    async fn scan_directory(
        dir: &Path,
        scope: SkillScope,
        skills: &mut HashMap<String, Arc<SkillPackage>>,
    ) -> common::Result<()> {
        if !dir.exists() {
            return Ok(());
        }
        let mut entries = tokio::fs::read_dir(dir).await
            .map_err(|e| common::ConfigError::Invalid(format!("Reading skills dir: {e}")))?;

        while let Some(entry) = entries.next_entry().await
            .map_err(|e| common::ConfigError::Invalid(format!("Reading entry: {e}")))? {
            let path = entry.path();
            if !path.is_dir() { continue; }
            // Skip hidden dirs, node_modules, .git
            let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if dir_name.starts_with('.') || dir_name == "node_modules" { continue; }

            let skill_md = path.join("SKILL.md");
            if !skill_md.exists() { continue; }

            let content = match tokio::fs::read_to_string(&skill_md).await {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(path = %skill_md.display(), "Failed to read SKILL.md: {e}");
                    continue;
                }
            };

            match parse_skill_md(&content, path.clone(), scope) {
                Ok(pkg) => {
                    let name = pkg.name.clone();
                    if let Some(existing) = skills.get(&name) {
                        if scope_priority(scope) > scope_priority(existing.scope) {
                            tracing::info!(skill = %name, "Skill shadowed by higher-priority scope");
                            skills.insert(name, Arc::new(pkg));
                        }
                    } else {
                        skills.insert(name, Arc::new(pkg));
                    }
                }
                Err(e) => {
                    tracing::warn!(path = %skill_md.display(), "Skipping skill: {e}");
                }
            }
        }
        Ok(())
    }

    /// Precompute description embeddings for semantic matching.
    pub async fn precompute_embeddings(&mut self, embed: &EmbedFn) {
        let mut embeddings = HashMap::new();
        for (name, pkg) in &self.skills {
            match embed(&pkg.description) {
                Ok(vec) => { embeddings.insert(name.clone(), vec); }
                Err(e) => { tracing::warn!(skill = %name, "Failed to embed: {e}"); }
            }
        }
        tracing::debug!("Precomputed embeddings for {} skills", embeddings.len());
        self.embeddings = embeddings;
    }

    pub fn get(&self, name: &str) -> Option<&Arc<SkillPackage>> {
        self.skills.get(name)
    }

    pub fn orchestrators(&self) -> Vec<&Arc<SkillPackage>> {
        self.skills.values()
            .filter(|p| p.skill_type == SkillType::Orchestrator && p.trusted)
            .collect()
    }

    pub fn regular_skills(&self) -> Vec<&Arc<SkillPackage>> {
        self.skills.values()
            .filter(|p| p.skill_type == SkillType::Skill && p.trusted)
            .collect()
    }

    /// Generate XML catalog for Tier 1 injection into system prompt.
    pub fn catalog_prompt(&self) -> String {
        let mut lines = vec!["<available_skills>".to_string()];
        let mut sorted: Vec<_> = self.skills.values().filter(|p| p.trusted).collect();
        sorted.sort_by_key(|p| &p.name);
        for pkg in sorted {
            let type_str = match pkg.skill_type {
                SkillType::Orchestrator => "orchestrator",
                SkillType::Skill => "skill",
            };
            lines.push(format!(
                "  <skill name=\"{}\" type=\"{}\">",
                pkg.name, type_str
            ));
            lines.push(format!("    {}", pkg.description.trim()));
            lines.push("  </skill>".to_string());
        }
        lines.push("</available_skills>".to_string());
        lines.join("\n")
    }

    pub fn all_skills(&self) -> impl Iterator<Item = &Arc<SkillPackage>> {
        self.skills.values()
    }

    pub fn loaded_at(&self) -> SystemTime {
        self.loaded_at
    }
}

fn scope_priority(scope: SkillScope) -> u8 {
    match scope {
        SkillScope::BuiltIn => 0,
        SkillScope::User => 1,
        SkillScope::Project => 2,
    }
}
```

- [ ] **Step 4: Add `pub mod discovery;` to lib.rs, move SkillCatalog fields/impl to be accessible**

In `types.rs`, make `SkillCatalog` fields `pub(crate)` so `discovery.rs` can construct it:

```rust
pub struct SkillCatalog {
    pub(crate) skills: HashMap<String, Arc<SkillPackage>>,
    pub(crate) embeddings: HashMap<String, Vec<f32>>,
    pub(crate) loaded_at: SystemTime,
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo nextest run -p skill-system`
Expected: All tests pass

- [ ] **Step 6: Commit**

```bash
git add crates/skill-system/src/discovery.rs crates/skill-system/src/types.rs crates/skill-system/src/lib.rs
git commit -m "feat(skill-system): implement SkillCatalog discovery with scope priority"
```

### Task 5: Implement SkillRouter (keyword + semantic matching)

**Files:**
- Create: `crates/skill-system/src/router.rs`
- Modify: `crates/skill-system/src/lib.rs`

- [ ] **Step 1: Write tests for keyword scoring**

```rust
// crates/skill-system/src/router.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::SkillSource;

    fn make_test_catalog() -> SkillCatalog {
        let skills = vec![
            ("task-management", "---\nname: task-management\ndescription: Create, organize, and track tasks, projects, and areas using OKR+PARA. Use when the user mentions todos, tasks, projects, areas, objectives, planning, reviews, or goal tracking.\nmetadata:\n  klyntbot:\n    type: orchestrator\n---\nTask instructions."),
            ("finance-management", "---\nname: finance-management\ndescription: Track expenses, budgets, and financial goals. Use when the user mentions budget, spending, expenses, or financial tracking.\nmetadata:\n  klyntbot:\n    type: orchestrator\n---\nFinance instructions."),
            ("general", "---\nname: general\ndescription: General-purpose assistant and orchestrator for greetings, conversation, and unmatched requests.\nmetadata:\n  klyntbot:\n    type: orchestrator\n---\nGeneral instructions."),
            ("search", "---\nname: search\ndescription: Web search and information retrieval for factual questions.\n---\nSearch instructions."),
        ];
        let source = SkillSource::BuiltIn(
            skills.iter().map(|(n, c)| (n.to_string(), c.to_string())).collect()
        );
        SkillCatalog::discover_sync(&[source]).unwrap()
    }

    #[test]
    fn test_keyword_scores_task_message() {
        let catalog = make_test_catalog();
        let router = SkillRouter::new(&catalog);
        let scores = router.keyword_scores("create a task for reviewing the budget", &catalog);
        assert!(scores.contains_key("task-management"));
        assert!(*scores.get("task-management").unwrap() > 0.0);
    }

    #[test]
    fn test_keyword_scores_no_match() {
        let catalog = make_test_catalog();
        let router = SkillRouter::new(&catalog);
        let scores = router.keyword_scores("hello, how are you today?", &catalog);
        // "general" might partially match on "conversation" etc but scores may be low
        // The key test is that specific domain skills don't match
        let task_score = scores.get("task-management").copied().unwrap_or(0.0);
        let finance_score = scores.get("finance-management").copied().unwrap_or(0.0);
        assert!(task_score == 0.0 || task_score < 0.2);
        assert!(finance_score == 0.0 || finance_score < 0.2);
    }

    #[test]
    fn test_select_orchestrator_falls_back_to_general() {
        let catalog = make_test_catalog();
        let router = SkillRouter::new(&catalog);
        let selected = router.select_orchestrator("hello there!", &catalog);
        assert_eq!(selected.name, "general");
    }

    #[test]
    fn test_select_orchestrator_only_returns_orchestrators() {
        let catalog = make_test_catalog();
        let router = SkillRouter::new(&catalog);
        // "search" should not be selected as orchestrator even if it matches
        let selected = router.select_orchestrator("search the web for me", &catalog);
        assert_eq!(selected.skill_type, SkillType::Orchestrator);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p skill-system`
Expected: FAIL — SkillRouter not defined

- [ ] **Step 3: Implement SkillRouter**

```rust
// crates/skill-system/src/router.rs
use std::collections::HashMap;
use std::sync::Arc;

use crate::types::*;

const GENERAL_SKILL_NAME: &str = "general";
const SKILL_ACTIVATION_THRESHOLD: f64 = 0.4;
const MAX_ACTIVATED_SKILLS: usize = 3;

pub struct SkillRouter {
    /// Pre-tokenized description words per skill (for keyword scoring).
    description_tokens: HashMap<String, Vec<String>>,
}

impl SkillRouter {
    pub fn new(catalog: &SkillCatalog) -> Self {
        let mut description_tokens = HashMap::new();
        for (name, pkg) in &catalog.skills {
            let tokens: Vec<String> = tokenize(&pkg.description);
            description_tokens.insert(name.clone(), tokens);
        }
        Self { description_tokens }
    }

    /// Keyword scores for all skills against a user message.
    pub fn keyword_scores(&self, message: &str, catalog: &SkillCatalog) -> HashMap<String, f64> {
        let msg_tokens: Vec<String> = tokenize(message);
        let mut result = HashMap::new();

        for (name, desc_tokens) in &self.description_tokens {
            if !catalog.skills.contains_key(name) { continue; }
            let mut hits = 0usize;
            for token in desc_tokens {
                if msg_tokens.contains(token) {
                    hits += 1;
                }
            }
            if hits > 0 {
                let normalizer = (desc_tokens.len() as f64 / 3.0).max(1.0);
                let score = (hits as f64 / normalizer).min(1.0);
                result.insert(name.clone(), score);
            }
        }
        result
    }

    /// Select the best orchestrator for a message. Keyword-only (no embeddings).
    /// For full blended scoring, use `select_orchestrator_blended`.
    pub fn select_orchestrator<'a>(
        &self,
        message: &str,
        catalog: &'a SkillCatalog,
    ) -> &'a Arc<SkillPackage> {
        self.select_orchestrator_blended(message, &[], catalog)
    }

    /// Select orchestrator with blended keyword + semantic scoring.
    pub fn select_orchestrator_blended<'a>(
        &self,
        message: &str,
        query_embedding: &[f32],
        catalog: &'a SkillCatalog,
    ) -> &'a Arc<SkillPackage> {
        let kw_scores = self.keyword_scores(message, catalog);
        let mut best: Option<(&str, f64)> = None;

        for pkg in catalog.orchestrators() {
            let kw_score = kw_scores.get(pkg.name.as_str()).copied().unwrap_or(0.0);
            let sem_score = if !query_embedding.is_empty() {
                catalog.embeddings.get(&pkg.name)
                    .map(|emb| cosine_similarity(query_embedding, emb))
                    .unwrap_or(0.0)
            } else {
                0.0
            };

            // Candidacy gate: keyword hit OR semantic >= 0.5
            if kw_score == 0.0 && sem_score < 0.5 {
                continue;
            }

            let blended = kw_score * 0.7 + sem_score * 0.3;
            if best.is_none_or(|(_, s)| blended > *s) {
                best = Some((pkg.name.as_str(), blended));
            }
        }

        if let Some((name, _)) = best {
            catalog.get(name).unwrap()
        } else {
            catalog.get(GENERAL_SKILL_NAME)
                .expect("General orchestrator must exist")
        }
    }

    /// Activate non-orchestrator skills relevant to the message.
    pub fn activate_skills<'a>(
        &self,
        message: &str,
        query_embedding: &[f32],
        catalog: &'a SkillCatalog,
    ) -> Vec<&'a Arc<SkillPackage>> {
        let kw_scores = self.keyword_scores(message, catalog);
        let mut scored: Vec<(&Arc<SkillPackage>, f64)> = Vec::new();

        for pkg in catalog.regular_skills() {
            let kw_score = kw_scores.get(pkg.name.as_str()).copied().unwrap_or(0.0);
            let sem_score = if !query_embedding.is_empty() {
                catalog.embeddings.get(&pkg.name)
                    .map(|emb| cosine_similarity(query_embedding, emb))
                    .unwrap_or(0.0)
            } else {
                0.0
            };

            let blended = kw_score * 0.7 + sem_score * 0.3;
            if blended >= SKILL_ACTIVATION_THRESHOLD {
                scored.push((pkg, blended));
            }
        }

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(MAX_ACTIVATED_SKILLS);
        scored.into_iter().map(|(pkg, _)| pkg).collect()
    }
}

fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .replace('-', " ")
        .split_whitespace()
        .filter(|w| w.len() > 2) // Skip tiny words (a, an, to, etc.)
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
        .filter(|w| !w.is_empty())
        .collect()
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.is_empty() || a.len() != b.len() { return 0.0; }
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 { 0.0 } else { (dot / (norm_a * norm_b)) as f64 }
}
```

- [ ] **Step 4: Add `pub mod router;` to lib.rs**

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo nextest run -p skill-system`
Expected: All tests pass

- [ ] **Step 6: Commit**

```bash
git add crates/skill-system/src/router.rs crates/skill-system/src/lib.rs
git commit -m "feat(skill-system): implement SkillRouter with keyword + semantic scoring"
```

---

## Chunk 3: SkillContextSource + Manifest Stub

### Task 6: Implement SkillContextSource

**Files:**
- Create: `crates/skill-system/src/context.rs`
- Modify: `crates/skill-system/src/lib.rs`

- [ ] **Step 1: Write tests for SkillContextSource**

```rust
// crates/skill-system/src/context.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::SkillSource;

    fn make_catalog_with_always_skills() -> (SkillCatalog, HashMap<String, String>) {
        let skills = vec![
            ("task-mgmt", "---\nname: task-mgmt\ndescription: Task management.\nmetadata:\n  klyntbot:\n    type: orchestrator\n    always_skills: [todo]\n---\nYou are the task agent."),
        ];
        let source = SkillSource::BuiltIn(
            skills.iter().map(|(n, c)| (n.to_string(), c.to_string())).collect()
        );
        let catalog = SkillCatalog::discover_sync(&[source]).unwrap();

        // Simulate references/ files
        let mut reference_files = HashMap::new();
        reference_files.insert(
            "builtin::task-mgmt/references/todo.md".to_string(),
            "# Todo Workflow\n\nCreate tasks using the task tool.".to_string(),
        );
        (catalog, reference_files)
    }

    #[tokio::test]
    async fn test_context_source_injects_orchestrator_body() {
        let (catalog, refs) = make_catalog_with_always_skills();
        let pkg = catalog.get("task-mgmt").unwrap().clone();
        let source = SkillContextSource::new(pkg, vec![], refs);

        let ctx = context_engine::source::SourceContext {
            channel: "test".into(), chat_id: "1".into(),
            message: None, intent_summary: None, project_id: None,
        };
        let result = source.provide(&ctx).await.unwrap();
        assert!(result.contains("task agent"), "Should contain orchestrator body");
    }

    #[tokio::test]
    async fn test_context_source_injects_always_skills() {
        let (catalog, refs) = make_catalog_with_always_skills();
        let pkg = catalog.get("task-mgmt").unwrap().clone();
        let source = SkillContextSource::new(pkg, vec![], refs);

        let ctx = context_engine::source::SourceContext {
            channel: "test".into(), chat_id: "1".into(),
            message: None, intent_summary: None, project_id: None,
        };
        let result = source.provide(&ctx).await.unwrap();
        assert!(result.contains("Todo Workflow"), "Should contain always-loaded skill content");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p skill-system`
Expected: FAIL — SkillContextSource not defined

- [ ] **Step 3: Implement SkillContextSource**

```rust
// crates/skill-system/src/context.rs
use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use context_engine::source::{ContextSource, SourceContext};

use crate::types::SkillPackage;

/// Injects active orchestrator instructions + always-loaded skills + activated skills.
/// Long-lived shared-state design (same pattern as old AgentContextSource).
pub struct SkillContextSource {
    active_orchestrator: Arc<RwLock<Option<Arc<SkillPackage>>>>,
    activated_skills: Arc<RwLock<Vec<Arc<SkillPackage>>>>,
    /// Preloaded reference files keyed by "builtin::{skill}/references/{name}.md"
    /// or "{absolute_path}/references/{name}.md" for filesystem skills.
    reference_files: Arc<HashMap<String, String>>,
}

impl SkillContextSource {
    pub fn new(
        active_orchestrator: Arc<RwLock<Option<Arc<SkillPackage>>>>,
        activated_skills: Arc<RwLock<Vec<Arc<SkillPackage>>>>,
        reference_files: Arc<HashMap<String, String>>,
    ) -> Self {
        Self { active_orchestrator, activated_skills, reference_files }
    }

    /// Load always_skills reference files from the orchestrator's directory.
    fn always_skill_content(&self, orchestrator: &SkillPackage) -> Vec<String> {
        let mut content = Vec::new();
        for skill_name in orchestrator.always_skills() {
            // Try exact path key first (filesystem skills)
            let fs_key = format!(
                "{}/references/{}.md",
                orchestrator.location.display(),
                skill_name
            );
            // Then try name-based key (built-in skills: "builtin::task-management/references/todo.md")
            let builtin_key = format!(
                "builtin::{}/references/{}.md",
                orchestrator.name,
                skill_name
            );
            let text = self.reference_files.get(&fs_key)
                .or_else(|| self.reference_files.get(&builtin_key));

            if let Some(text) = text {
                content.push(format!("# Skill: {}\n\n{}", skill_name, text));
            } else {
                tracing::debug!(
                    skill = %skill_name,
                    orchestrator = %orchestrator.name,
                    "Always-skill reference not found (tried: {fs_key}, {builtin_key})"
                );
            }
        }
        content
    }
}

#[async_trait]
impl ContextSource for SkillContextSource {
    fn name(&self) -> &str {
        "skill_profile"
    }

    fn priority(&self) -> u8 {
        35
    }

    async fn provide(&self, _ctx: &SourceContext) -> Option<String> {
        let orchestrator_guard = self.active_orchestrator.read().await;
        let orchestrator = orchestrator_guard.as_ref()?;

        let mut sections = Vec::new();

        // Orchestrator body
        if !orchestrator.body.is_empty() {
            sections.push(format!(
                "# Agent: {}\n\n{}",
                orchestrator.name, orchestrator.body
            ));
        }

        // Always-loaded skills
        sections.extend(self.always_skill_content(orchestrator));

        // Per-message activated skills
        let skills_guard = self.activated_skills.read().await;
        for pkg in skills_guard.iter() {
            sections.push(format!(
                "# Skill: {} (activated)\n\n{}",
                pkg.name, pkg.body
            ));
        }

        if sections.is_empty() {
            None
        } else {
            Some(sections.join("\n\n---\n\n"))
        }
    }

    fn estimated_tokens(&self) -> usize {
        500
    }
}
```

- [ ] **Step 4: Add `pub mod context;` to lib.rs**

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo nextest run -p skill-system`
Expected: All tests pass

- [ ] **Step 6: Commit**

```bash
git add crates/skill-system/src/context.rs crates/skill-system/src/lib.rs
git commit -m "feat(skill-system): implement SkillContextSource for system prompt injection"
```

### Task 7: Create manifest.rs stub

**Files:**
- Create: `crates/skill-system/src/manifest.rs`
- Modify: `crates/skill-system/src/lib.rs`

- [ ] **Step 1: Create manifest.rs stub for Subsystem 2**

```rust
// crates/skill-system/src/manifest.rs
//! manifest.json parsing — stub for Subsystem 2 (Declarative Feature Modules).

use std::path::Path;

use crate::types::SkillManifest;

/// Parse a manifest.json file. Returns None if file doesn't exist.
pub async fn parse_manifest(skill_dir: &Path) -> common::Result<Option<SkillManifest>> {
    let manifest_path = skill_dir.join("manifest.json");
    if !manifest_path.exists() {
        return Ok(None);
    }

    let content = tokio::fs::read_to_string(&manifest_path).await
        .map_err(|e| common::ConfigError::Invalid(format!("Reading manifest.json: {e}")))?;

    let raw: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| common::ConfigError::Invalid(format!("Parsing manifest.json: {e}")))?;

    Ok(Some(SkillManifest {
        schema_version: raw.get("schema_version")
            .and_then(|v| v.as_str())
            .unwrap_or("1.0")
            .to_string(),
        entities: raw.get("entities")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default(),
        permissions: raw.get("permissions")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default(),
    }))
}
```

- [ ] **Step 2: Add `pub mod manifest;` to lib.rs**

- [ ] **Step 3: Verify compilation**

Run: `cargo build -p skill-system`
Expected: Compiles

- [ ] **Step 4: Commit**

```bash
git add crates/skill-system/src/manifest.rs crates/skill-system/src/lib.rs
git commit -m "feat(skill-system): add manifest.json parser stub for Subsystem 2"
```

---

## Chunk 4: Directory Migration (agents/ → skills/)

### Task 8: Create skills/ directory with migrated SKILL.md files

**Files:**
- Create: `skills/general/SKILL.md`, `skills/general/references/*.md`
- Create: `skills/task-management/SKILL.md`, `skills/task-management/references/*.md`
- Create: `skills/finance-management/SKILL.md`, `skills/finance-management/references/*.md`
- Create: `skills/automation/SKILL.md`, `skills/automation/references/*.md`
- Create: `skills/communication/SKILL.md`, `skills/communication/references/*.md`

This task is a content migration. For each agent:

- [ ] **Step 1: Read all existing AGENT.md + skills/*.md files**

Read the 5 `agents/*/AGENT.md` files and all 14 `agents/*/skills/*.md` files.

- [ ] **Step 2: Create skills/ directory structure**

```bash
mkdir -p skills/general/references
mkdir -p skills/task-management/references
mkdir -p skills/finance-management/references
mkdir -p skills/automation/references
mkdir -p skills/communication/references
```

- [ ] **Step 3: Convert each AGENT.md to SKILL.md format**

For each agent, convert the frontmatter from `AgentFrontmatter` format to Agent Skills spec format. Key changes per file:

**Template for conversion:**
```yaml
# Old (AGENT.md)
---
name: task
description: Task management specialist
tools: [task, area, project]
triggers: [todo, task, ...]
max_iterations: 12
can_delegate_to: [finance]
always_skills: [todo, daily-planner]
---

# New (SKILL.md)
---
name: task-management
description: >
  Task and project management specialist with planning, reviews, and goal tracking.
  Use when the user mentions todos, tasks, projects, areas, objectives,
  planning, reviews, or goal tracking.
metadata:
  author: klyntbot
  version: "1.0.0"
  klyntbot:
    type: orchestrator
    tools: [tasks, project, area, okr, notes, ask_user, memory, grep, glob, read_file, list_dir]
    mcp_tools: ["google-calendar"]
    can_delegate_to: [finance-management]
    max_iterations: 12
    always_skills: [todo, daily-planner]
---

[body stays the same]
```

Do this for all 5 agents. Move triggers content into the `description` field (the description should include keywords that were previously in triggers for keyword matching to work).

- [ ] **Step 4: Copy skill reference files**

For each `agents/{agent}/skills/{skill}.md`, copy to `skills/{new-name}/references/{skill}.md`. Keep the content identical — these are reference files, not standalone skills.

- [ ] **Step 5: Update include_str! macros in discovery.rs**

Update `crates/skill-system/src/discovery.rs` (or a new `builtin.rs`) to define `BUILTIN_SKILLS` using the new paths.

**IMPORTANT**: Before adding any `include_skill_reference!` entry, verify the source file exists in `agents/*/skills/`. The current `BUILTIN_AGENT_SKILLS` in `manager.rs` does NOT include `communication/messaging.md` or `communication/notification.md` — check if those files exist. If they don't, omit them from `BUILTIN_SKILL_REFERENCES` and create them during migration if needed.

```rust
macro_rules! include_skill {
    ($name:expr) => {
        ($name, include_str!(concat!("../../../skills/", $name, "/SKILL.md")))
    };
}

pub const BUILTIN_SKILLS: &[(&str, &str)] = &[
    include_skill!("general"),
    include_skill!("task-management"),
    include_skill!("finance-management"),
    include_skill!("automation"),
    include_skill!("communication"),
];

macro_rules! include_skill_reference {
    ($skill:expr, $ref_name:expr) => {
        ($skill, $ref_name, include_str!(concat!(
            "../../../skills/", $skill, "/references/", $ref_name, ".md"
        )))
    };
}

// NOTE: Only include references that have actual source files.
// Verify each entry exists before adding.
pub const BUILTIN_SKILL_REFERENCES: &[(&str, &str, &str)] = &[
    include_skill_reference!("general", "search"),
    include_skill_reference!("general", "skill-creator"),
    include_skill_reference!("general", "browser"),
    include_skill_reference!("general", "memory"),
    include_skill_reference!("general", "summarize"),
    include_skill_reference!("task-management", "todo"),
    include_skill_reference!("task-management", "daily-planner"),
    include_skill_reference!("task-management", "task-decompose"),
    include_skill_reference!("task-management", "project-management"),
    include_skill_reference!("task-management", "weekly-review"),
    include_skill_reference!("task-management", "retrospective"),
    include_skill_reference!("finance-management", "budgeting"),
    include_skill_reference!("finance-management", "spending-analysis"),
    include_skill_reference!("automation", "cron"),
    // Communication references: include only if source files exist in agents/communication/skills/
];
```

- [ ] **Step 6: Verify compilation**

Run: `cargo build -p skill-system`
Expected: Compiles (include_str! paths resolve correctly)

- [ ] **Step 7: Commit**

```bash
git add skills/ crates/skill-system/
git commit -m "feat(skill-system): migrate agents/ to skills/ in Agent Skills format"
```

---

## Chunk 5: Config + Integration Wiring

### Task 9: Add SkillConfig to config crate

**Files:**
- Modify: `crates/config/src/schema/agents.rs`
- Modify: `crates/config/src/schema/core.rs`

- [ ] **Step 1: Add SkillConfig to agents.rs**

Add at the end of `crates/config/src/schema/agents.rs`:

```rust
/// Configuration for the skill discovery system.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SkillConfig {
    /// Additional directories to scan for skills (beyond data_dir/skills/).
    #[serde(default)]
    pub extra_skill_dirs: Vec<String>,

    /// Orchestrator selection threshold (semantic score >= this to consider).
    #[serde(default = "default_orchestrator_threshold")]
    pub orchestrator_semantic_threshold: f64,

    /// Per-message skill activation threshold.
    #[serde(default = "default_activation_threshold")]
    pub activation_threshold: f64,

    /// Max non-orchestrator skills activated per message.
    #[serde(default = "default_max_activated_skills")]
    pub max_activated_skills: usize,
}

fn default_orchestrator_threshold() -> f64 { 0.5 }
fn default_activation_threshold() -> f64 { 0.4 }
fn default_max_activated_skills() -> usize { 3 }
```

And add `project_root` to the root `Config` struct (NOT inside `SkillConfig`):

```rust
// In crates/config/src/schema/core.rs, add to Config struct:
/// Project root for .agents/skills/ scanning. Falls back to CWD.
/// Set by the desktop app on launch. Distinct from agents.defaults.workspace.
#[serde(default, skip_serializing_if = "Option::is_none")]
pub project_root: Option<String>,
```

- [ ] **Step 2: Add `skills` field to root Config in core.rs**

In `crates/config/src/schema/core.rs`, add to the `Config` struct:

```rust
#[serde(default)]
pub skills: SkillConfig,
```

- [ ] **Step 3: Verify compilation**

Run: `cargo build -p config`
Expected: Compiles

- [ ] **Step 4: Commit**

```bash
git add crates/config/src/schema/agents.rs crates/config/src/schema/core.rs
git commit -m "feat(config): add SkillConfig for discovery paths and thresholds"
```

### Task 10: Wire SkillCatalog into AgentLoop builder

**Files:**
- Modify: `crates/agent/Cargo.toml`
- Modify: `crates/agent/src/agent_loop/builder.rs`
- Modify: `crates/agent/src/agent_runtime/runtime.rs`

This is the largest integration task. The approach:

1. Add `skill-system` as a dependency of `agent`
2. In `builder.rs`, replace `AgentManager` construction with `SkillCatalog::discover()` + `SkillRouter::new()`
3. In `runtime.rs`, replace `AgentManager` usage in `process_message()` with `SkillCatalog` + `SkillRouter`
4. Replace `AgentContextSource` with `SkillContextSource`

- [ ] **Step 1: Add skill-system dependency to agent crate**

In `crates/agent/Cargo.toml`, add:
```toml
skill-system.workspace = true
```

- [ ] **Step 2: Update builder.rs — replace AgentManager with SkillCatalog**

Read `crates/agent/src/agent_loop/builder.rs` to identify all `AgentManager` references. Replace:
- `AgentManager::new()` + `load_builtin_agents()` + `load_workspace_agents()` → `SkillCatalog::discover()` with built-in + user sources
- `agent_manager.set_embedder()` + `precompute_embeddings()` → `catalog.precompute_embeddings(embed_fn)`
- Store `SkillCatalog` and `SkillRouter` in the runtime instead of `AgentManager`

- [ ] **Step 3: Update runtime.rs — replace AgentManager usage in process_message()**

In `crates/agent/src/agent_runtime/runtime.rs`, update `process_message()`:
- Replace `keyword_scores_owned()` → `router.keyword_scores()`
- Replace `blend_scores()` → `router.select_orchestrator_blended()`
- Replace `active_profile` write with selected `SkillPackage`
- Replace `profile.allowed_tool_names()` → `pkg.allowed_tool_names()`
- Replace `profile.allows_mcp_server()` → `pkg.allows_mcp_server()`
- Replace `profile.max_iterations` → `pkg.max_iterations()`

- [ ] **Step 4: Replace AgentContextSource with per-message SkillContextSource**

The existing `AgentContextSource` reads from `Arc<RwLock<Option<Arc<AgentProfile>>>>` — a shared mutable slot written in `process_message()` and read in `ContextEngine::assemble()`.

The new design uses the same shared-state pattern but with skill types:

```rust
// In AgentRuntime (or equivalent holder):
active_orchestrator: Arc<RwLock<Option<Arc<SkillPackage>>>>,
activated_skills: Arc<RwLock<Vec<Arc<SkillPackage>>>>,
reference_files: Arc<HashMap<String, String>>,  // Populated once at startup from BUILTIN_SKILL_REFERENCES
```

In `process_message()`:
1. `router.select_orchestrator_blended()` → write result to `active_orchestrator`
2. `router.activate_skills()` → write result to `activated_skills`
3. `ContextEngine::assemble()` calls `SkillContextSource::provide()` which reads both slots

`SkillContextSource` becomes:
```rust
pub struct SkillContextSource {
    active_orchestrator: Arc<RwLock<Option<Arc<SkillPackage>>>>,
    activated_skills: Arc<RwLock<Vec<Arc<SkillPackage>>>>,
    reference_files: Arc<HashMap<String, String>>,
}
```

This preserves the existing `AgentContextSource` lifecycle pattern (long-lived, shared, per-message mutation via RwLock).

**Reference files population**: At startup, build a `HashMap<String, String>` from `BUILTIN_SKILL_REFERENCES`:
```rust
let mut refs = HashMap::new();
for (skill_name, ref_name, content) in BUILTIN_SKILL_REFERENCES {
    let key = format!("builtin::{skill_name}/references/{ref_name}.md");
    refs.insert(key, content.to_string());
}
```

For filesystem skills, load reference files lazily from `{skill_dir}/references/*.md` during discovery.

- [ ] **Step 5: Verify compilation**

Run: `cargo build --workspace`
Expected: Compiles (may have warnings from unused old code)

- [ ] **Step 6: Run existing agent tests**

Run: `cargo nextest run -p agent`
Expected: Some tests fail (they reference old types) — this is expected and addressed in Task 11

- [ ] **Step 7: Commit**

```bash
git add crates/agent/
git commit -m "feat(agent): wire SkillCatalog + SkillRouter into agent runtime"
```

### Task 11: Port and fix agent tests

**Files:**
- Modify: various test files in `crates/agent/`

- [ ] **Step 1: Update all tests referencing AgentProfile/AgentSkill/AgentManager**

Port tests from `agent_profile/types.rs` and `agent_profile/manager.rs` tests to use `SkillPackage` and `SkillCatalog`. The test logic stays the same, only types change.

Key mappings:
- `AgentProfile { name, tools, triggers, ... }` → `SkillPackage { name, metadata: SkillMetadata { klyntbot: Some(KlyntbotMeta { tools, ... }) }, ... }`
- `AgentManager::new() + load_builtin_agents()` → `SkillCatalog::discover_sync(&[SkillSource::BuiltIn(...)])`
- `mgr.match_agent(msg)` → `router.select_orchestrator(msg, &catalog)`

- [ ] **Step 2: Run all agent tests**

Run: `cargo nextest run -p agent`
Expected: All tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/agent/
git commit -m "test(agent): port all tests to SkillPackage/SkillCatalog types"
```

---

## Chunk 6: Cleanup + Delete Old Code

### Task 12: Delete old agent_profile, content_registry, agents/ directory

**Files:**
- Delete: `crates/agent/src/agent_profile/` (entire directory)
- Delete: `crates/agent/src/content_registry/` (entire directory)
- Delete: `crates/agent/src/context_sources/agent.rs`
- Delete: `agents/` (entire directory)
- Modify: `crates/agent/src/lib.rs` (remove old module declarations)

- [ ] **Step 1: Remove old module declarations from agent crate**

In `crates/agent/src/lib.rs`, remove:
- `pub mod agent_profile;`
- `pub mod content_registry;`
- Any re-exports of `AgentProfile`, `AgentSkill`, `AgentManager`

Update remaining code that imports from these modules to use `skill_system::` types.

- [ ] **Step 2: Delete old directories**

```bash
rm -rf crates/agent/src/agent_profile/
rm -rf crates/agent/src/content_registry/
rm crates/agent/src/context_sources/agent.rs
rm -rf agents/
```

- [ ] **Step 3: Fix any remaining compilation errors**

Run: `cargo build --workspace`
Fix any remaining references to deleted types.

- [ ] **Step 4: Run full test suite**

Run: `cargo nextest run --workspace`
Expected: All tests pass

- [ ] **Step 5: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings (fix any new warnings)

- [ ] **Step 6: Check formatting**

Run: `cargo fmt --all --check`
Expected: No formatting issues

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "refactor(agent): remove old agent_profile, content_registry, and agents/ directory

Replaced by skill-system crate with Agent Skills spec-compatible format."
```

### Task 12b: Audit tools field migration to prevent deny-all regression

**Files:**
- Review: all 5 `agents/*/AGENT.md` files

- [ ] **Step 1: Audit every AGENT.md for tools: [] usage**

The old semantic: `tools: []` = full access (returns `None` from `allowed_tool_names()`).
The new semantic: `tools: null/omitted` = full access, `tools: []` = deny-all.

Check each agent:
- `general/AGENT.md` — currently has `tools: [ask_user, memory, ...]` → explicit list, safe
- `task/AGENT.md` — currently has `tools: [task, tasks, ...]` → explicit list, safe
- `finance/AGENT.md` — check if tools is empty or explicit
- `automation/AGENT.md` — check if tools is empty or explicit
- `communication/AGENT.md` — check if tools is empty or explicit

**Rule**: If any existing AGENT.md has `tools: []` (meaning full access), the new SKILL.md must OMIT the tools field entirely (not set it to `tools: []`).

- [ ] **Step 2: Verify during migration in Task 8**

When writing each SKILL.md, apply the audit results.

### Task 12c: Update delegation names and GENERAL_SKILL_NAME constant

**Files:**
- Modify: `crates/skill-system/src/router.rs`
- Modify: `crates/agent/src/agent_runtime/runtime.rs` (delegation path)

- [ ] **Step 1: Ensure GENERAL_SKILL_NAME = "general" matches the new SKILL.md**

The `general` orchestrator SKILL.md keeps `name: general` (not renamed). Verify this matches `const GENERAL_SKILL_NAME` in `router.rs`.

- [ ] **Step 2: Update can_delegate_to values in new SKILL.md files**

When migrating AGENT.md → SKILL.md in Task 8:
- Old: `can_delegate_to: [finance]` → New: `can_delegate_to: [finance-management]`
- Old: `can_delegate_to: [task]` → New: `can_delegate_to: [task-management]`

All delegation target names must match the new SKILL.md `name` field.

- [ ] **Step 3: Update delegation lookup in runtime.rs**

In `runtime.rs`, the delegation path calls `catalog.get(agent_name)`. The `agent_name` comes from `can_delegate_to` in the orchestrator's SKILL.md. Since we updated the names in Step 2, the lookup will work — but verify there are no hardcoded agent name strings elsewhere in the codebase.

Run: `grep -r '"task"' crates/agent/src/ --include="*.rs" | grep -v test | grep -v "//"`
Run: `grep -r '"finance"' crates/agent/src/ --include="*.rs" | grep -v test | grep -v "//"`

Fix any hardcoded old agent names.

### Task 12d: Update app-core agents UI handlers

**Files:**
- Modify: `crates/app-core/src/handlers/agents.rs`
- Modify: `crates/desktop-shared/src/commands/agents.rs`
- Modify: `crates/desktop/src/commands/agents.rs`

- [ ] **Step 1: Read current agents handler**

Read `crates/app-core/src/handlers/agents.rs` to understand the full surface:
- `builtin_agents()` → listing agents for the UI
- `reload_agents()` → hot-reload after editing
- Agent CRUD operations (read/write AGENT.md files)

- [ ] **Step 2: Replace builtin_agents() with SkillCatalog API**

Replace calls to `agent::agent_profile::builtin_agents()` with `SkillCatalog::all_skills()` or `SkillCatalog::orchestrators()`. The UI now lists skills instead of agents.

- [ ] **Step 3: Replace reload_agents() with SkillCatalog::reload()**

The existing `self.agent.reload_agents()` path through `AgentLoop::reload_agents()` → `AgentManager::reload()` must be replaced with `SkillCatalog::reload()`. Since `SkillRouter` caches description tokens, it must be rebuilt after reload:

```rust
pub fn reload_skills(&mut self) {
    let changes = self.catalog.reload().unwrap_or_default();
    if !changes.is_empty() {
        self.router = SkillRouter::new(&self.catalog);
        // Re-embed if embedder available
    }
}
```

- [ ] **Step 4: Update desktop-shared types**

Replace `AgentProfileSummary`, `AgentFileSummary`, `AgentFileContent` with skill equivalents:
- `SkillSummary { name, description, skill_type, scope }`
- `SkillFileContent { name, content }` (for reading/editing SKILL.md)

- [ ] **Step 5: Update desktop Tauri commands**

Update `crates/desktop/src/commands/agents.rs` to use the new types and call the updated handlers.

- [ ] **Step 6: Verify compilation**

Run: `cargo build --workspace`
Expected: Compiles

- [ ] **Step 7: Commit**

```bash
git add crates/app-core/ crates/desktop-shared/ crates/desktop/
git commit -m "feat(app-core): update agents UI layer to use SkillCatalog"
```

### Task 13: Update app-core initialization

**Files:**
- Modify: `crates/app-core/Cargo.toml`
- Modify: `crates/app-core/src/init/mod.rs`

- [ ] **Step 1: Add skill-system dependency to app-core**

In `crates/app-core/Cargo.toml`:
```toml
skill-system.workspace = true
```

- [ ] **Step 2: Update init_agent() to use SkillCatalog**

In `crates/app-core/src/init/mod.rs`, replace the agent initialization section that creates `AgentManager` with `SkillCatalog::discover()` + `SkillRouter::new()`.

- [ ] **Step 3: Verify full app builds and tests pass**

Run: `cargo build --workspace && cargo nextest run --workspace`
Expected: All pass

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/
git commit -m "feat(app-core): initialize SkillCatalog in app startup pipeline"
```

### Task 14: Final integration test

**Files:**
- Create: `crates/skill-system/tests/integration.rs`

- [ ] **Step 1: Write integration test**

```rust
// crates/skill-system/tests/integration.rs
use skill_system::discovery::{SkillSource, BUILTIN_SKILLS};
use skill_system::router::SkillRouter;
use skill_system::types::*;

#[test]
fn test_full_pipeline_builtin_skills() {
    let source = SkillSource::BuiltIn(
        BUILTIN_SKILLS.iter().map(|(n, c)| (n.to_string(), c.to_string())).collect()
    );
    let catalog = SkillCatalog::discover_sync(&[source]).unwrap();

    // All 5 orchestrators loaded
    assert_eq!(catalog.orchestrators().len(), 5);

    // Router can select orchestrators
    let router = SkillRouter::new(&catalog);
    let selected = router.select_orchestrator("create a task for me", &catalog);
    assert_eq!(selected.name, "task-management");

    let selected = router.select_orchestrator("check my budget", &catalog);
    assert_eq!(selected.name, "finance-management");

    let selected = router.select_orchestrator("hello there!", &catalog);
    assert_eq!(selected.name, "general");

    // Catalog prompt is valid XML-ish
    let prompt = catalog.catalog_prompt();
    assert!(prompt.contains("<available_skills>"));
    assert!(prompt.contains("</available_skills>"));
}

#[tokio::test]
async fn test_filesystem_discovery() {
    let temp = tempfile::tempdir().unwrap();
    let skill_dir = temp.path().join("my-skill");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: my-skill\ndescription: A test skill.\n---\n\nTest body.",
    ).unwrap();

    let source = SkillSource::Directory(temp.path().to_path_buf(), SkillScope::User);
    let catalog = SkillCatalog::discover(&[source]).await.unwrap();
    assert!(catalog.get("my-skill").is_some());
    assert_eq!(catalog.get("my-skill").unwrap().scope, SkillScope::User);
}
```

- [ ] **Step 2: Run integration test**

Run: `cargo nextest run -p skill-system --test integration`
Expected: All pass

- [ ] **Step 3: Final full workspace check**

Run: `cargo nextest run --workspace && cargo clippy --workspace --all-targets --all-features && cargo fmt --all --check`
Expected: All pass, 0 clippy warnings, format clean

- [ ] **Step 4: Commit**

```bash
git add crates/skill-system/tests/
git commit -m "test(skill-system): add integration tests for full discovery + routing pipeline"
```
