# Phase 2: Tool Registry with Rich Metadata + Agent Skills Spec

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend the Tool trait with rich metadata (category, tags, cost, examples) and update the skill format to Agent Skills spec compatibility with runtime filesystem loading.

**Architecture:** Two independent subsystems. Tool metadata extends the existing `Tool` trait + `ToolRegistry` with optional metadata, search, and usage tracking. Skills spec replaces custom YAML frontmatter with Agent Skills spec format and adds a `SkillLoader` for filesystem discovery. Both feed into the search infrastructure built in Phase 1.

**Tech Stack:** Rust proc macros (syn, quote), serde, async-trait, notify (filesystem watching)

**Depends on:** Phase 1 (BM25 search for tool/skill discovery — soft dependency, can stub)

---

## File Structure

### Tool Metadata (Upgrade 4)
| File | Action | Responsibility |
|------|--------|---------------|
| `crates/tools-core/src/metadata.rs` | Create | ToolMetadata, ToolCategory, ToolSource, CostHint types |
| `crates/tools-core/src/lib.rs` | Modify | Add `metadata()` default method to Tool trait, re-export types |
| `crates/tools-core/src/registry.rs` | Modify | Add metadata storage, usage tracking, search |
| `crates/tools-core-macros/src/lib.rs` | Modify | Parse metadata attributes in derive(Tool) |
| `crates/tools/src/*.rs` (each tool) | Modify | Add `#[tool(category, tags, cost)]` attributes |

### Agent Skills Spec (Upgrade 5)
| File | Action | Responsibility |
|------|--------|---------------|
| `crates/agent/src/agent_profile/types.rs` | Modify | Update AgentSkill struct with spec fields |
| `crates/agent/src/agent_profile/parser.rs` | Modify | Parse new frontmatter format |
| `crates/agent/src/skill_loader.rs` | Create | Runtime skill loading from filesystem |
| `crates/config/src/lib.rs` | Modify | Add skills_dir config field |
| `agents/*/skills/*.md` (14 files) | Modify | Rewrite frontmatter |
| `agents/*/AGENT.md` (5 files) | Modify | Update frontmatter with metadata |

---

## Chunk 1: Tool Metadata Types + Trait Extension

### Task 1: ToolMetadata Types

**Files:**
- Create: `crates/tools-core/src/metadata.rs`

- [ ] **Step 1: Write the metadata types**

```rust
// crates/tools-core/src/metadata.rs

//! Rich metadata for tool discovery and categorization.

use serde::{Deserialize, Serialize};

/// Category for organizing tools.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, Hash)]
pub enum ToolCategory {
    #[default]
    General,
    FileSystem,
    Search,
    Web,
    Communication,
    TaskManagement,
    Memory,
    Finance,
    Productivity,
    System,
    Mcp,
    Plugin,
}

impl std::fmt::Display for ToolCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Where the tool originated.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum ToolSource {
    #[default]
    Native,
    Feature(String),
    Mcp(String),
    Plugin(String),
    External,
}

/// Estimated cost per tool invocation.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum CostHint {
    #[default]
    Free,
    Low,
    Medium,
    High,
    Variable,
}

/// An example usage of the tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExample {
    pub description: String,
    pub params: serde_json::Value,
}

/// Rich metadata for tool discovery and categorization.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolMetadata {
    pub category: ToolCategory,
    pub tags: Vec<String>,
    pub author: String,
    pub version: String,
    pub source: ToolSource,
    pub examples: Vec<ToolExample>,
    pub related_tools: Vec<String>,
    pub cost_hint: CostHint,
}
```

- [ ] **Step 2: Add module to lib.rs and extend Tool trait**

Add to `crates/tools-core/src/lib.rs`:
```rust
pub mod metadata;
pub use metadata::{CostHint, ToolCategory, ToolExample, ToolMetadata, ToolSource};
```

Extend the `Tool` trait with a default `metadata()` method:
```rust
/// Rich metadata for discovery. Override to provide category, tags, etc.
fn metadata(&self) -> ToolMetadata {
    ToolMetadata::default()
}
```

- [ ] **Step 3: Run to verify compilation**

Run: `cargo nextest run -p tools-core`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/tools-core/src/metadata.rs crates/tools-core/src/lib.rs
git commit -m "feat(tools-core): add ToolMetadata types and extend Tool trait"
```

---

### Task 2: Extend ToolRegistry with Search + Usage Tracking

**Files:**
- Modify: `crates/tools-core/src/registry.rs`

- [ ] **Step 1: Write failing tests for registry search and usage**

Add to `crates/tools-core/src/registry.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::*;
    use async_trait::async_trait;
    use serde_json::json;

    struct FakeSearchTool;

    #[async_trait]
    impl Tool for FakeSearchTool {
        fn name(&self) -> &str { "search" }
        fn description(&self) -> &str { "Search for files and content" }
        fn parameters(&self) -> Value { json!({"type": "object"}) }
        async fn execute(&self, _args: Value, _ctx: &RoutingContext) -> Result<String> {
            Ok("ok".into())
        }
        fn metadata(&self) -> ToolMetadata {
            ToolMetadata {
                category: ToolCategory::Search,
                tags: vec!["file".into(), "content".into(), "grep".into()],
                cost_hint: CostHint::Free,
                ..Default::default()
            }
        }
    }

    struct FakeWebTool;

    #[async_trait]
    impl Tool for FakeWebTool {
        fn name(&self) -> &str { "web_search" }
        fn description(&self) -> &str { "Search the web for information" }
        fn parameters(&self) -> Value { json!({"type": "object"}) }
        async fn execute(&self, _args: Value, _ctx: &RoutingContext) -> Result<String> {
            Ok("ok".into())
        }
        fn metadata(&self) -> ToolMetadata {
            ToolMetadata {
                category: ToolCategory::Web,
                tags: vec!["web".into(), "search".into(), "internet".into()],
                cost_hint: CostHint::Low,
                ..Default::default()
            }
        }
    }

    #[test]
    fn test_registry_stores_metadata() {
        let mut reg = ToolRegistry::new();
        reg.register(FakeSearchTool);

        let meta = reg.get_metadata("search");
        assert!(meta.is_some());
        assert_eq!(meta.unwrap().category, ToolCategory::Search);
    }

    #[test]
    fn test_registry_by_category() {
        let mut reg = ToolRegistry::new();
        reg.register(FakeSearchTool);
        reg.register(FakeWebTool);

        let search_tools = reg.by_category(&ToolCategory::Search);
        assert_eq!(search_tools.len(), 1);
        assert_eq!(search_tools[0], "search");
    }

    #[test]
    fn test_registry_usage_tracking() {
        let mut reg = ToolRegistry::new();
        reg.register(FakeSearchTool);

        reg.record_usage("search");
        reg.record_usage("search");
        reg.record_usage("search");

        let top = reg.top_used(5);
        assert_eq!(top.len(), 1);
        assert_eq!(top[0].0, "search");
        assert_eq!(top[0].1, 3);
    }

    #[test]
    fn test_registry_search_by_query() {
        let mut reg = ToolRegistry::new();
        reg.register(FakeSearchTool);
        reg.register(FakeWebTool);

        let results = reg.search_tools("web internet", 10);
        assert!(!results.is_empty());
        // web_search should rank higher for "web internet"
        assert_eq!(results[0].0, "web_search");
    }
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo nextest run -p tools-core -E 'test(registry)'`
Expected: FAIL — methods don't exist

- [ ] **Step 3: Implement registry extensions**

Add to `ToolRegistry`:

```rust
use crate::metadata::{ToolCategory, ToolMetadata};

pub struct ToolRegistry {
    tools: HashMap<String, DynTool>,
    metadata: HashMap<String, ToolMetadata>,       // NEW
    usage_counts: HashMap<String, u64>,             // NEW
    cached_definitions: Mutex<Option<Arc<Vec<Value>>>>,
    permissions: Option<ToolPermissions>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            metadata: HashMap::new(),
            usage_counts: HashMap::new(),
            cached_definitions: Mutex::new(None),
            permissions: None,
        }
    }

    pub fn register(&mut self, tool: impl Tool + 'static) {
        let name = tool.name().to_string();
        let meta = tool.metadata();
        debug!("Registering tool: {}", name);
        self.metadata.insert(name.clone(), meta);
        self.tools.insert(name, Arc::new(tool));
        self.invalidate_cache();
    }

    pub fn get_metadata(&self, name: &str) -> Option<&ToolMetadata> {
        self.metadata.get(name)
    }

    pub fn by_category(&self, category: &ToolCategory) -> Vec<&str> {
        self.metadata
            .iter()
            .filter(|(_, meta)| &meta.category == category)
            .map(|(name, _)| name.as_str())
            .collect()
    }

    pub fn record_usage(&mut self, name: &str) {
        *self.usage_counts.entry(name.to_string()).or_insert(0) += 1;
    }

    pub fn top_used(&self, n: usize) -> Vec<(&str, u64)> {
        let mut counts: Vec<_> = self.usage_counts.iter().map(|(k, v)| (k.as_str(), *v)).collect();
        counts.sort_by(|a, b| b.1.cmp(&a.1));
        counts.truncate(n);
        counts
    }

    /// Simple keyword search across tool names, descriptions, and tags.
    pub fn search_tools(&self, query: &str, limit: usize) -> Vec<(String, f64)> {
        let query_lower = query.to_lowercase();
        let terms: Vec<&str> = query_lower.split_whitespace().collect();

        let mut scores: Vec<(String, f64)> = self.tools.iter().map(|(name, tool)| {
            let desc = tool.description().to_lowercase();
            let name_lower = name.to_lowercase();
            let tags = self.metadata.get(name)
                .map(|m| m.tags.join(" ").to_lowercase())
                .unwrap_or_default();

            let mut score = 0.0;
            for term in &terms {
                if name_lower.contains(term) { score += 3.0; }
                if desc.contains(term) { score += 1.0; }
                if tags.contains(term) { score += 2.0; }
            }
            (name.clone(), score)
        }).filter(|(_, score)| *score > 0.0).collect();

        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores.truncate(limit);
        scores
    }

    // ... existing methods remain unchanged ...
}
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p tools-core`
Expected: All PASS

- [ ] **Step 5: Commit**

```bash
git add crates/tools-core/src/registry.rs
git commit -m "feat(tools-core): extend ToolRegistry with metadata, usage tracking, and search"
```

---

### Task 3: Update Derive Macro for Metadata Attributes

**Files:**
- Modify: `crates/tools-core-macros/src/lib.rs`

- [ ] **Step 1: Read the current derive macro implementation**

Read `crates/tools-core-macros/src/lib.rs` to understand how `#[tool(...)]` attributes are currently parsed.

- [ ] **Step 2: Extend macro to parse `category`, `tags`, `cost` attributes**

Add parsing for:
```rust
#[tool(
    name = "read_file",
    description = "Read file contents",
    category = "FileSystem",     // NEW — maps to ToolCategory enum
    tags = "file,read,content",  // NEW — comma-separated
    cost = "Free",               // NEW — maps to CostHint enum
)]
```

The macro should generate a `metadata()` impl that returns a `ToolMetadata` with these fields populated. Unspecified fields default via `ToolMetadata::default()`.

- [ ] **Step 3: Test macro expansion compiles**

Create a test tool in `crates/tools-core-macros/tests/` or verify by adding metadata to one existing tool.

Run: `cargo nextest run -p tools-core-macros`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/tools-core-macros/src/lib.rs
git commit -m "feat(tools-core-macros): extend derive(Tool) to parse category, tags, cost attributes"
```

---

### Task 4: Add Metadata to Built-in Tools

**Files:**
- Modify: `crates/tools/src/*.rs` (all tool files)

This is a **user contribution point** — the category/tag choices shape tool discovery.

- [ ] **Step 1: Add metadata to all tools**

Update each tool's `#[tool()]` attribute. Here are the recommended mappings:

| Tool | Category | Tags | Cost |
|------|----------|------|------|
| `read_file` | FileSystem | file,read,content | Free |
| `list_dir` | FileSystem | file,directory,list | Free |
| `write_file` | FileSystem | file,write,create | Free |
| `delete` | FileSystem | file,delete,remove | Free |
| `grep` | Search | search,content,regex | Free |
| `glob` | Search | search,file,pattern | Free |
| `web_search` | Web | search,web,internet | Low |
| `web_fetch` | Web | web,fetch,url,scrape | Low |
| `memory` | Memory | memory,fact,recall,learn | Free |
| `learning` | Memory | learn,preference,behavior | Free |
| `task` | TaskManagement | task,todo,action,plan | Free |
| `project` | TaskManagement | project,manage,plan | Free |
| `okr` | Productivity | okr,objective,goal | Free |
| `area` | Productivity | area,para,responsibility | Free |
| `spawn` | System | agent,delegate,spawn | Variable |
| `cron` | System | schedule,cron,recurring | Free |
| `message` | Communication | message,send,reply | Free |
| `ask_user` | Communication | ask,input,confirm | Free |
| `browser` | Web | browser,navigate,scrape | Low |
| `delegation` | System | delegate,agent,spawn | Variable |
| `annotate` | Memory | annotation,note,gotcha | Free |

- [ ] **Step 2: Run all tools tests**

Run: `cargo nextest run -p tools`
Expected: All PASS

- [ ] **Step 3: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

- [ ] **Step 4: Commit**

```bash
git add crates/tools/src/
git commit -m "feat(tools): add category, tags, and cost metadata to all built-in tools"
```

---

## Chunk 2: Agent Skills Spec Compatibility

### Task 5: Update AgentSkill Struct

**Files:**
- Modify: `crates/agent/src/agent_profile/types.rs`

- [ ] **Step 1: Read current AgentSkill struct**

Read `crates/agent/src/agent_profile/types.rs` to see current fields.

- [ ] **Step 2: Extend AgentSkill with spec fields**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSkill {
    pub name: String,
    pub description: String,
    pub content: String,

    // Agent Skills spec fields
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub updated_on: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,

    // Klyntbot extensions (existing)
    #[serde(default)]
    pub always: bool,
    #[serde(default)]
    pub triggers: Vec<String>,
    #[serde(default)]
    pub agent: Option<String>,
}
```

- [ ] **Step 3: Run agent tests to verify backward compatibility**

Run: `cargo nextest run -p agent`
Expected: All PASS (new fields have defaults)

- [ ] **Step 4: Commit**

```bash
git add crates/agent/src/agent_profile/types.rs
git commit -m "feat(agent): extend AgentSkill struct with Agent Skills spec fields"
```

---

### Task 6: Update Skill Parser

**Files:**
- Modify: `crates/agent/src/agent_profile/parser.rs`

- [ ] **Step 1: Read current parser**

Read `crates/agent/src/agent_profile/parser.rs` to understand current frontmatter parsing.

- [ ] **Step 2: Write test for new frontmatter format**

```rust
#[test]
fn test_parse_agent_skills_spec_format() {
    let content = r#"---
name: todo
description: Task creation with confidence scoring
license: MIT
metadata:
  author: klyntbot
  version: "1.0.0"
  updated-on: "2026-03-10"
  source: official
  tags: "task,todo,productivity"
  always: true
  triggers: "create task,add todo"
  agent: task
---

Task creation instructions here.
"#;

    let skill = parse_skill(content).unwrap();
    assert_eq!(skill.name, "todo");
    assert_eq!(skill.author.as_deref(), Some("klyntbot"));
    assert_eq!(skill.version.as_deref(), Some("1.0.0"));
    assert!(skill.always);
    assert_eq!(skill.triggers, vec!["create task", "add todo"]);
    assert_eq!(skill.tags, vec!["task", "todo", "productivity"]);
}

#[test]
fn test_parse_legacy_format_still_works() {
    let content = r#"---
name: todo
description: Task creation
always: true
triggers: []
---

Legacy content.
"#;

    let skill = parse_skill(content).unwrap();
    assert_eq!(skill.name, "todo");
    assert!(skill.always);
    assert!(skill.author.is_none());
}
```

- [ ] **Step 3: Update parser to handle both formats**

The parser should:
1. Try parsing with `metadata` block (new format)
2. Fall back to flat keys (legacy format)
3. Extract `metadata.tags`, `metadata.always`, `metadata.triggers` etc.

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p agent -E 'test(parse)'`
Expected: All PASS

- [ ] **Step 5: Commit**

```bash
git add crates/agent/src/agent_profile/parser.rs
git commit -m "feat(agent): update skill parser for Agent Skills spec + legacy format support"
```

---

### Task 7: SkillLoader — Runtime Filesystem Loading

**Files:**
- Create: `crates/agent/src/skill_loader.rs`
- Modify: `crates/config/src/lib.rs`

- [ ] **Step 1: Add `skills_dir` to config**

Add to the Config struct in `crates/config/src/lib.rs`:
```rust
#[serde(default = "default_skills_dir")]
pub skills_dir: Option<String>,

fn default_skills_dir() -> Option<String> {
    None  // Defaults to ~/.klyntbot/.agents/skills/
}
```

- [ ] **Step 2: Write SkillLoader tests**

```rust
// crates/agent/src/skill_loader.rs

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    #[test]
    fn test_load_skills_from_directory() {
        let dir = TempDir::new().unwrap();
        let skill_path = dir.path().join("test-skill.md");
        fs::write(&skill_path, r#"---
name: test-skill
description: A test skill
metadata:
  author: test
  version: "1.0.0"
  tags: "test,example"
---

Test skill content.
"#).unwrap();

        let loader = SkillLoader::new(dir.path().to_path_buf());
        let skills = loader.load_external_skills().unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "test-skill");
        assert_eq!(skills[0].tags, vec!["test", "example"]);
    }

    #[test]
    fn test_load_skills_empty_directory() {
        let dir = TempDir::new().unwrap();
        let loader = SkillLoader::new(dir.path().to_path_buf());
        let skills = loader.load_external_skills().unwrap();
        assert!(skills.is_empty());
    }

    #[test]
    fn test_load_skills_nonexistent_directory() {
        let loader = SkillLoader::new("/nonexistent/path".into());
        let skills = loader.load_external_skills().unwrap();
        assert!(skills.is_empty());
    }

    #[test]
    fn test_skills_for_agent() {
        let dir = TempDir::new().unwrap();

        fs::write(dir.path().join("task-skill.md"), r#"---
name: task-skill
description: Task skill
metadata:
  agent: task
---
Content.
"#).unwrap();

        fs::write(dir.path().join("general-skill.md"), r#"---
name: general-skill
description: General skill
metadata:
  agent: general
---
Content.
"#).unwrap();

        let mut loader = SkillLoader::new(dir.path().to_path_buf());
        loader.refresh().unwrap();

        let task_skills = loader.skills_for_agent("task");
        assert_eq!(task_skills.len(), 1);
        assert_eq!(task_skills[0].name, "task-skill");
    }
}
```

- [ ] **Step 3: Implement SkillLoader**

```rust
// crates/agent/src/skill_loader.rs

use std::path::PathBuf;
use crate::agent_profile::types::AgentSkill;

pub struct SkillLoader {
    skills_dir: PathBuf,
    external_skills: Vec<AgentSkill>,
}

impl SkillLoader {
    pub fn new(skills_dir: PathBuf) -> Self {
        Self {
            skills_dir,
            external_skills: Vec::new(),
        }
    }

    pub fn load_external_skills(&self) -> common::Result<Vec<AgentSkill>> {
        if !self.skills_dir.exists() {
            return Ok(Vec::new());
        }
        let mut skills = Vec::new();
        for entry in std::fs::read_dir(&self.skills_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "md") {
                let content = std::fs::read_to_string(&path)?;
                match crate::agent_profile::parser::parse_skill(&content) {
                    Ok(skill) => skills.push(skill),
                    Err(e) => {
                        tracing::warn!("Failed to parse skill {:?}: {}", path, e);
                    }
                }
            }
        }
        Ok(skills)
    }

    pub fn refresh(&mut self) -> common::Result<()> {
        self.external_skills = self.load_external_skills()?;
        Ok(())
    }

    pub fn skills_for_agent(&self, agent_name: &str) -> Vec<&AgentSkill> {
        self.external_skills
            .iter()
            .filter(|s| s.agent.as_deref() == Some(agent_name))
            .collect()
    }

    pub fn all_external_skills(&self) -> &[AgentSkill] {
        &self.external_skills
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p agent -E 'test(skill_loader)'`
Expected: All PASS

- [ ] **Step 5: Commit**

```bash
git add crates/agent/src/skill_loader.rs crates/config/src/lib.rs
git commit -m "feat(agent): add SkillLoader for runtime filesystem skill discovery"
```

---

### Task 8: Update Built-in Skill Frontmatter

**Files:**
- Modify: `agents/*/skills/*.md` (14 files)
- Modify: `agents/*/AGENT.md` (5 files)

This is a **user contribution point** — choosing tags and metadata for each skill.

- [ ] **Step 1: Update all 14 skill files to new format**

Each skill file should be updated from:
```yaml
---
name: todo
description: Task creation with confidence scoring
always: true
triggers: []
---
```

To:
```yaml
---
name: todo
description: Task creation with confidence scoring
license: MIT
metadata:
  author: klyntbot
  version: "1.0.0"
  updated-on: "2026-03-10"
  source: official
  tags: "task,todo,productivity"
  always: true
  triggers: ""
  agent: task
---
```

- [ ] **Step 2: Verify all skills parse correctly**

Run: `cargo nextest run -p agent -E 'test(parse)'`
Expected: All PASS

- [ ] **Step 3: Run full agent tests**

Run: `cargo nextest run -p agent`
Expected: All PASS

- [ ] **Step 4: Commit**

```bash
git add agents/
git commit -m "feat(agent): update all skill files to Agent Skills spec format"
```

---

### Task 9: Final Integration + Verification

- [ ] **Step 1: Run workspace tests**

Run: `cargo nextest run --workspace`
Expected: All PASS

- [ ] **Step 2: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

- [ ] **Step 3: Format**

Run: `cargo fmt --all --check`
Expected: Clean

- [ ] **Step 4: Commit any fixes**

```bash
git commit -m "fix: address clippy and formatting issues from Phase 2"
```
