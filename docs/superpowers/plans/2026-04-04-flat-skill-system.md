# Flat Skill System — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the routed/delegated skill system with a flat architecture — all tools available to every message, skills as user-editable markdown, KLYNTBOT.md as soul file, no delegation.

**Architecture:** Gut the `skill-system` crate down to a `SkillStore` that reads `.md` files from `~/.klyntbot/skills/`. Replace `SkillRouter`, `IntentAnalyzer`, `DelegationTool`, and orchestration override with a flat tool pool and on-demand `skill_reference` loading. Add `KLYNTBOT.md` as always-loaded soul context via a new `ContextSource`.

**Tech Stack:** Rust, serde, serde_yaml, tokio, async-trait

---

## File Structure

### New files to create:
- `crates/skill-system/src/store.rs` — `SkillStore`, `SkillEntry` (frontmatter + body), file loading, hot-reload
- `crates/skill-system/src/soul.rs` — `SoulContextSource` (loads KLYNTBOT.md as ContextSource)
- `crates/skill-system/src/listing.rs` — `SkillListingSource` (formats skill YAML into system-reminder)
- `skills/notebook/SKILL.md` — New notebook skill
- `skills/learning/SKILL.md` — New learning skill

### Files to modify significantly:
- `crates/skill-system/src/lib.rs` — Replace re-exports with new modules
- `crates/skill-system/Cargo.toml` — Remove unused deps
- `crates/agent/src/agent_runtime/runtime.rs` — Gut `AgentRuntime` to 3-phase pipeline
- `crates/agent/src/agent_loop/builder.rs` — Remove skill catalog/router/analyzer construction
- `crates/agent/src/agent_loop/mod.rs` — Remove skill_catalog/router fields from AgentLoop
- `crates/tools/src/domain/mod.rs` — Remove `delegation` module
- `crates/tools/src/lib.rs` — Remove `DelegationHandler` trait and re-exports
- `crates/config/src/schema/mod.rs` — Remove `orchestrator` module
- `crates/config/src/schema/core.rs` — Remove `orchestrator` field from root Config
- `crates/config/src/schema/agents.rs` — Remove `SkillConfig`
- `crates/simulator/src/agent_harness.rs` — Simplify to flat tool pool

### Files to delete entirely:
- `crates/skill-system/src/router.rs` — SkillRouter (replaced by flat tool pool)
- `crates/skill-system/src/context.rs` — SkillContextSource (replaced by SoulContextSource + SkillListingSource)
- `crates/skill-system/src/discovery.rs` — Compiled skill discovery (replaced by SkillStore file loading)
- `crates/agent/src/intent_pipeline/analysis.rs` — IntentAnalyzer, AC matchers, all classification
- `crates/agent/src/intent_pipeline/types.rs` — ExecutionMode, ComplexityLevel, PipelineConfig, etc.
- `crates/agent/src/intent_pipeline/mod.rs` — Intent pipeline module
- `crates/agent/src/autotuner/shadow_classifier.rs` — Shadow classifier
- `crates/tools/src/domain/delegation.rs` — DelegationTool
- `crates/config/src/schema/orchestrator.rs` — OrchestratorConfig
- `skills/general/` — General skill (no longer needed)
- `skills/communication/` — Communication skill (no longer needed)

### Files with minor updates:
- `crates/agent/src/lib.rs` — Update re-exports
- `crates/agent/src/events.rs` — Remove classification events (ClassificationComplete, ExecutionStarted engine field)
- `crates/agent/src/subagent.rs` — Remove SkillPackage references
- `crates/app-core/src/handlers/chat/streaming.rs` — Remove classification event handlers
- `crates/config/src/lib.rs` — Remove OrchestratorConfig re-export

---

### Task 1: Create SkillStore — File-Based Skill Loading

**Files:**
- Create: `crates/skill-system/src/store.rs`
- Modify: `crates/skill-system/src/lib.rs`
- Modify: `crates/skill-system/Cargo.toml`

The foundation — loads `.md` files from disk, parses YAML frontmatter, serves entries.

- [ ] **Step 1: Create store.rs with SkillEntry and SkillStore**

Create `crates/skill-system/src/store.rs`:

```rust
//! SkillStore — loads skill markdown files from disk.
//!
//! Skills are `.md` files with YAML frontmatter (name, description, whenToUse).
//! The frontmatter is always available; the full body is loaded on demand
//! via the `skill_reference` tool.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use tracing::{debug, warn};

/// Maximum description length in the skill listing (tokens budget).
const MAX_DESCRIPTION_CHARS: usize = 250;

/// Default skills embedded in the binary, installed on first run.
const DEFAULT_SKILLS: &[(&str, &str)] = &[
    ("task-management.md", include_str!("../../../skills/task-management/SKILL.md")),
    ("finance-management.md", include_str!("../../../skills/finance-management/SKILL.md")),
    ("automation.md", include_str!("../../../skills/automation/SKILL.md")),
    ("notebook.md", include_str!("../../../skills/notebook/SKILL.md")),
    ("learning.md", include_str!("../../../skills/learning/SKILL.md")),
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
                common::KlyntbotError::Internal(format!(
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
            common::KlyntbotError::Internal(format!(
                "Failed to read skills dir {}: {e}",
                skills_dir.display()
            ))
        })? {
            let dir_entry = dir_entry.map_err(|e| {
                common::KlyntbotError::Internal(format!("Failed to read dir entry: {e}"))
            })?;
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
            let truncated = if desc.len() > MAX_DESCRIPTION_CHARS {
                format!("{}…", &desc[..MAX_DESCRIPTION_CHARS - 1])
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

    /// Build a SkillReferenceIndex for the skill_reference tool.
    pub fn build_reference_index(&self) -> HashMap<String, String> {
        self.entries
            .iter()
            .map(|(name, entry)| (name.clone(), entry.body.clone()))
            .collect()
    }

    // ── Private ──────────────────────────────────────────────

    fn load_one(path: &Path) -> common::Result<SkillEntry> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            common::KlyntbotError::Internal(format!(
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
                common::KlyntbotError::Internal(format!(
                    "Failed to write default skill {}: {e}",
                    path.display()
                ))
            })?;
            debug!(path = %path.display(), "Installed default skill");
        }
        Ok(())
    }
}

/// Split a markdown file into YAML frontmatter and body.
fn split_frontmatter(content: &str) -> common::Result<(SkillFrontmatter, String)> {
    let trimmed = content.trim();
    if !trimmed.starts_with("---") {
        return Err(common::KlyntbotError::Internal(
            "Skill file must start with YAML frontmatter (---)".to_string(),
        ));
    }

    let after_first = &trimmed[3..];
    let end_idx = after_first.find("\n---").ok_or_else(|| {
        common::KlyntbotError::Internal("Missing closing --- for YAML frontmatter".to_string())
    })?;

    let yaml_str = &after_first[..end_idx];
    let body = after_first[end_idx + 4..].trim().to_string();

    let frontmatter: SkillFrontmatter = serde_yaml::from_str(yaml_str).map_err(|e| {
        common::KlyntbotError::Internal(format!("Failed to parse skill YAML: {e}"))
    })?;

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
}
```

- [ ] **Step 2: Update lib.rs — replace old modules with store**

Replace `crates/skill-system/src/lib.rs` entirely:

```rust
pub mod parser;
pub mod persona;
pub mod store;
pub mod types;

pub use persona::{parse_persona_skill, ParsedPersonaSkill, PersonaSkillMetadata};
pub use store::{SkillEntry, SkillFrontmatter, SkillStore};
```

Note: Keep `parser.rs` (still used by persona parsing), `persona.rs` (persona skills), `types.rs` (will clean later). Remove `router`, `context`, `discovery` modules.

- [ ] **Step 3: Build and run tests**

Run: `cargo build -p skill-system && cargo nextest run -p skill-system -E 'test(store)'`
Expected: 4 store tests pass. Build may fail due to removed modules referenced elsewhere — that's expected, we fix consumers in later tasks.

- [ ] **Step 4: Commit**

```bash
git add crates/skill-system/src/store.rs crates/skill-system/src/lib.rs
git commit -m "feat(skill-system): add SkillStore for file-based skill loading"
```

---

### Task 2: Create New Skill Files (notebook, learning) + Convert Existing Skills

**Files:**
- Create: `skills/notebook/SKILL.md`
- Create: `skills/learning/SKILL.md`
- Modify: `skills/task-management/SKILL.md` — Simplify YAML to new format
- Modify: `skills/finance-management/SKILL.md` — Simplify YAML to new format
- Modify: `skills/automation/SKILL.md` — Simplify YAML to new format

Convert existing skills from the old complex YAML (with `metadata.klyntbot.tools`, `can_delegate_to`, etc.) to the new minimal format (name, description, whenToUse only). Create the two new skills.

- [ ] **Step 1: Create notebook skill**

Create `skills/notebook/SKILL.md`:

```markdown
---
name: notebook
description: Note-taking, knowledge capture, and idea organization
whenToUse: When the user mentions notes, jot down, write down, capture, notebook, or ideas
---

You are the notebook specialist. You help users capture, organize, and retrieve notes and ideas.

## Core Workflow

1. **Capture** — quickly jot down thoughts, ideas, meeting notes, or observations
2. **Organize** — tag, link, and categorize notes for easy retrieval
3. **Retrieve** — search and surface relevant notes when needed

## Note Types

| Type | When to use | Example |
|------|-------------|---------|
| Quick note | Fleeting thought or idea | "Note: check the new API docs" |
| Meeting note | During or after meetings | "Meeting with team about Q2 planning" |
| Reference note | Facts or information to remember | "The deploy process uses these steps..." |
| Reflection | Personal insights or learnings | "I realized that..." |

## Guidelines

- Keep notes concise — capture the essence, not everything
- Always add context (who, what, when, why) so notes are useful later
- Link related notes when the user mentions connections
- When searching, show the most relevant notes first with snippets
```

- [ ] **Step 2: Create learning skill**

Create `skills/learning/SKILL.md`:

```markdown
---
name: learning
description: Flashcard generation, spaced repetition, and study workflows
whenToUse: When the user mentions study, flashcards, review, learn, quiz, or spaced repetition
---

You are the learning specialist. You help users learn and retain knowledge through flashcards, spaced repetition, and structured study workflows.

## Core Workflow

1. **Generate** — create flashcards from conversations, notes, or explicit requests
2. **Review** — present cards due for review using spaced repetition scheduling
3. **Track** — monitor learning progress and retention rates

## Flashcard Guidelines

- Each card should test ONE concept (atomic knowledge)
- Use cloze deletions for factual recall: "The capital of France is {{Paris}}"
- Use Q&A format for conceptual understanding
- Include context tags for filtering (e.g., #rust, #finance, #cooking)
- Generate cards from natural conversation when the user learns something new

## Study Patterns

| Pattern | Trigger | Action |
|---------|---------|--------|
| Quick review | "review my cards" | Present due cards in priority order |
| Topic study | "study rust concepts" | Filter cards by tag, present in order |
| Generate from chat | User learns something new | Offer to create a flashcard |
| Progress check | "how am I doing" | Show retention stats and streaks |
```

- [ ] **Step 3: Convert task-management YAML to new format**

In `skills/task-management/SKILL.md`, replace the entire YAML frontmatter block (everything between `---` markers) with:

```yaml
---
name: task-management
description: Create, organize, and track tasks, projects, areas using OKR+PARA
whenToUse: When the user mentions todos, tasks, projects, areas, objectives, planning, reviews, or goal tracking
---
```

Keep the body (everything after the second `---`) unchanged.

- [ ] **Step 4: Convert finance-management YAML to new format**

In `skills/finance-management/SKILL.md`, replace the YAML frontmatter with:

```yaml
---
name: finance-management
description: Personal finance tracking with multi-currency support, budgeting, and FIRE analytics
whenToUse: When the user mentions expenses, budget, accounts, transactions, spending, savings, or investments
---
```

Keep the body unchanged.

- [ ] **Step 5: Convert automation YAML to new format**

In `skills/automation/SKILL.md`, replace the YAML frontmatter with:

```yaml
---
name: automation
description: Reminders, cron jobs, and recurring automations
whenToUse: When the user mentions remind, schedule, every day, recurring, cron, or automate
---
```

Keep the body unchanged.

- [ ] **Step 6: Verify all 5 skills parse correctly**

Run a quick test by adding to `store.rs` tests:

```rust
#[test]
fn default_skills_parse() {
    for (filename, content) in super::DEFAULT_SKILLS {
        let result = super::split_frontmatter(content);
        assert!(result.is_ok(), "Failed to parse {}: {:?}", filename, result.err());
        let (fm, body) = result.unwrap();
        assert!(!fm.name.is_empty(), "{} has empty name", filename);
        assert!(!fm.description.is_empty(), "{} has empty description", filename);
        assert!(!body.is_empty(), "{} has empty body", filename);
    }
}
```

Run: `cargo nextest run -p skill-system -E 'test(default_skills_parse)'`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add skills/notebook/ skills/learning/ skills/task-management/SKILL.md skills/finance-management/SKILL.md skills/automation/SKILL.md crates/skill-system/src/store.rs
git commit -m "feat(skills): add notebook and learning skills, convert existing to flat YAML format"
```

---

### Task 3: Create SoulContextSource (KLYNTBOT.md) and SkillListingSource

**Files:**
- Create: `crates/skill-system/src/soul.rs`
- Create: `crates/skill-system/src/listing.rs`
- Modify: `crates/skill-system/src/lib.rs`
- Modify: `crates/skill-system/Cargo.toml` — Add `context_engine` dependency

These are two new `ContextSource` implementations that replace `SkillContextSource`.

- [ ] **Step 1: Create soul.rs — KLYNTBOT.md loader**

Create `crates/skill-system/src/soul.rs`:

```rust
//! SoulContextSource — loads KLYNTBOT.md as always-present context.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;
use tracing::{debug, warn};

use context_engine::source::{ContextSource, SourceContext};

/// Default KLYNTBOT.md content, installed on first run.
const DEFAULT_SOUL: &str = r#"# Klyntbot

You are Klyntbot, a personal AI assistant.

## Personality
- Helpful, concise, and proactive
- Speak naturally, not robotically
- Match the user's language (if they write in Vietnamese, respond in Vietnamese)

## Preferences
- Use metric units
- Currency: VND
- Timezone: auto-detect from system
"#;

/// Context source that loads KLYNTBOT.md from the data directory.
pub struct SoulContextSource {
    content: Arc<RwLock<String>>,
    path: PathBuf,
}

impl SoulContextSource {
    /// Create and load the soul file. Installs default if missing.
    pub fn load(data_dir: &Path) -> common::Result<Self> {
        let path = data_dir.join("KLYNTBOT.md");

        if !path.exists() {
            std::fs::write(&path, DEFAULT_SOUL).map_err(|e| {
                common::KlyntbotError::Internal(format!(
                    "Failed to write default KLYNTBOT.md: {e}"
                ))
            })?;
            debug!(path = %path.display(), "Installed default KLYNTBOT.md");
        }

        let content = std::fs::read_to_string(&path).map_err(|e| {
            common::KlyntbotError::Internal(format!(
                "Failed to read KLYNTBOT.md: {e}"
            ))
        })?;

        Ok(Self {
            content: Arc::new(RwLock::new(content)),
            path,
        })
    }

    /// Reload from disk (for hot-reload via config watcher).
    pub async fn reload(&self) -> common::Result<()> {
        let content = std::fs::read_to_string(&self.path).map_err(|e| {
            common::KlyntbotError::Internal(format!(
                "Failed to reload KLYNTBOT.md: {e}"
            ))
        })?;
        *self.content.write().await = content;
        debug!("KLYNTBOT.md reloaded");
        Ok(())
    }
}

#[async_trait]
impl ContextSource for SoulContextSource {
    fn name(&self) -> &str {
        "soul"
    }

    fn priority(&self) -> u8 {
        // Highest priority — soul is the most important context
        50
    }

    fn protected(&self) -> bool {
        true // Never evicted by token budget
    }

    async fn provide(&self, _ctx: &SourceContext) -> Option<String> {
        let content = self.content.read().await;
        if content.is_empty() {
            None
        } else {
            Some(content.clone())
        }
    }

    fn estimated_tokens(&self) -> usize {
        300 // KLYNTBOT.md is typically short
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_soul_is_valid() {
        assert!(DEFAULT_SOUL.contains("Klyntbot"));
        assert!(DEFAULT_SOUL.contains("Personality"));
    }

    #[tokio::test]
    async fn soul_loads_from_tempdir() {
        let dir = tempfile::tempdir().unwrap();
        let source = SoulContextSource::load(dir.path()).unwrap();
        let ctx = SourceContext::default();
        let content = source.provide(&ctx).await;
        assert!(content.is_some());
        assert!(content.unwrap().contains("Klyntbot"));
    }
}
```

- [ ] **Step 2: Create listing.rs — Skill listing context source**

Create `crates/skill-system/src/listing.rs`:

```rust
//! SkillListingSource — injects skill YAML frontmatter into system prompt.

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use context_engine::source::{ContextSource, SourceContext};

use super::store::SkillStore;

/// Context source that formats the skill listing for the system prompt.
pub struct SkillListingSource {
    store: Arc<RwLock<SkillStore>>,
}

impl SkillListingSource {
    pub fn new(store: Arc<RwLock<SkillStore>>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl ContextSource for SkillListingSource {
    fn name(&self) -> &str {
        "skill_listing"
    }

    fn priority(&self) -> u8 {
        40 // After soul (50), before memory (30)
    }

    fn protected(&self) -> bool {
        true // Always present — skills are core context
    }

    async fn provide(&self, _ctx: &SourceContext) -> Option<String> {
        let store = self.store.read().await;
        let listing = store.format_listing();
        if listing.lines().count() <= 1 {
            None // No skills loaded
        } else {
            Some(listing)
        }
    }

    fn estimated_tokens(&self) -> usize {
        200 // ~5 skills × ~40 tokens each
    }
}
```

- [ ] **Step 3: Update lib.rs with new modules**

Replace `crates/skill-system/src/lib.rs`:

```rust
pub mod listing;
pub mod parser;
pub mod persona;
pub mod soul;
pub mod store;
pub mod types;

pub use persona::{parse_persona_skill, ParsedPersonaSkill, PersonaSkillMetadata};
pub use store::{SkillEntry, SkillFrontmatter, SkillStore};
pub use soul::SoulContextSource;
pub use listing::SkillListingSource;
```

- [ ] **Step 4: Add context_engine dependency to Cargo.toml**

In `crates/skill-system/Cargo.toml`, ensure `context_engine` is listed:

```toml
context_engine.workspace = true
```

Also add `tempfile` as a dev dependency for tests:

```toml
[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 5: Build and test**

Run: `cargo build -p skill-system && cargo nextest run -p skill-system -E 'test(soul) | test(listing) | test(store)'`
Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/skill-system/src/soul.rs crates/skill-system/src/listing.rs crates/skill-system/src/lib.rs crates/skill-system/Cargo.toml
git commit -m "feat(skill-system): add SoulContextSource and SkillListingSource"
```

---

### Task 4: Delete Legacy Skill System Code

**Files:**
- Delete: `crates/skill-system/src/router.rs`
- Delete: `crates/skill-system/src/context.rs`
- Delete: `crates/skill-system/src/discovery.rs`

Clean removal of old routing, context injection, and compiled discovery.

- [ ] **Step 1: Delete the files**

```bash
rm crates/skill-system/src/router.rs
rm crates/skill-system/src/context.rs
rm crates/skill-system/src/discovery.rs
```

- [ ] **Step 2: Verify skill-system crate builds**

Run: `cargo build -p skill-system 2>&1 | grep "^error" | head -10`
Expected: Clean build. The old modules are no longer referenced from `lib.rs` (updated in Task 3).

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "refactor(skill-system): delete SkillRouter, SkillContextSource, compiled discovery"
```

---

### Task 5: Delete DelegationTool and IntentAnalyzer

**Files:**
- Delete: `crates/tools/src/domain/delegation.rs`
- Delete: `crates/agent/src/intent_pipeline/analysis.rs`
- Delete: `crates/agent/src/intent_pipeline/types.rs`
- Delete: `crates/agent/src/intent_pipeline/mod.rs`
- Delete: `crates/agent/src/autotuner/shadow_classifier.rs`
- Delete: `crates/config/src/schema/orchestrator.rs`
- Modify: `crates/tools/src/domain/mod.rs` — Remove `delegation` module
- Modify: `crates/tools/src/lib.rs` — Remove `DelegationHandler` trait re-export
- Modify: `crates/agent/src/autotuner/mod.rs` — Remove `shadow_classifier` module
- Modify: `crates/config/src/schema/mod.rs` — Remove `orchestrator` module
- Modify: `crates/config/src/schema/core.rs` — Remove `orchestrator` field from Config
- Modify: `crates/config/src/lib.rs` — Remove OrchestratorConfig re-export
- Modify: `crates/config/src/schema/agents.rs` — Remove SkillConfig

This task deletes all the legacy routing, classification, and delegation code. It will break compilation in `agent` and `app-core` — those are fixed in Tasks 6 and 7.

- [ ] **Step 1: Delete delegation tool**

```bash
rm crates/tools/src/domain/delegation.rs
```

In `crates/tools/src/domain/mod.rs`, remove:
```rust
pub mod delegation;
```

In `crates/tools/src/lib.rs`, remove the `DelegationHandler` and `DelegationTool` re-exports and the `DelegationHandler` trait definition if it's there.

- [ ] **Step 2: Delete intent pipeline**

```bash
rm crates/agent/src/intent_pipeline/analysis.rs
rm crates/agent/src/intent_pipeline/types.rs
rm crates/agent/src/intent_pipeline/mod.rs
```

If `intent_pipeline/engines/` directory still exists with debate/interaction engines, keep those but move them. If the `intent_pipeline/` directory only contains engines now, rename to just `engines/`:

```bash
# Check what's left
ls crates/agent/src/intent_pipeline/
# If only engines/ remains, move it up
mv crates/agent/src/intent_pipeline/engines crates/agent/src/engines_legacy
rm -rf crates/agent/src/intent_pipeline
mv crates/agent/src/engines_legacy crates/agent/src/engines
```

Update `crates/agent/src/lib.rs` — remove `pub mod intent_pipeline`, add `pub mod engines` if needed.

- [ ] **Step 3: Delete shadow classifier**

```bash
rm crates/agent/src/autotuner/shadow_classifier.rs
```

In `crates/agent/src/autotuner/mod.rs`, remove `pub mod shadow_classifier;` and any re-exports.

- [ ] **Step 4: Delete OrchestratorConfig**

```bash
rm crates/config/src/schema/orchestrator.rs
```

In `crates/config/src/schema/mod.rs`, remove `pub mod orchestrator;` and its re-exports.

In `crates/config/src/schema/core.rs`, remove the `orchestrator: OrchestratorConfig` field from the root `Config` struct and its `Default` impl.

In `crates/config/src/lib.rs`, remove `OrchestratorConfig` from re-exports.

- [ ] **Step 5: Remove SkillConfig from agents.rs**

In `crates/config/src/schema/agents.rs`, remove the `SkillConfig` struct and all its related default functions. Remove `pub skills: SkillConfig` field from whichever struct contains it (likely `AgentsConfig`).

- [ ] **Step 6: Verify config crate builds**

Run: `cargo build -p config -p tools 2>&1 | grep "^error" | head -10`
Expected: Clean build for config and tools. Agent and app-core will break — fixed in next tasks.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "refactor: delete DelegationTool, IntentAnalyzer, OrchestratorConfig, SkillConfig"
```

---

### Task 6: Rewrite AgentRuntime — 3-Phase Flat Pipeline

**Files:**
- Modify: `crates/agent/src/agent_runtime/runtime.rs`
- Modify: `crates/agent/src/agent_loop/mod.rs`
- Modify: `crates/agent/src/lib.rs`

This is the largest single change — gut `AgentRuntime` to the 3-phase flat pipeline.

- [ ] **Step 1: Rewrite AgentRuntime struct**

In `crates/agent/src/agent_runtime/runtime.rs`, replace the struct with:

```rust
pub struct AgentRuntime {
    context_engine: Arc<ContextEngine>,
    core: Arc<crate::execution::ExecutionCore>,
    validator: ResponseValidator,
    cost_tracker: Arc<CostTracker>,
    execution_model: String,
    provider_name: String,
    context_window: usize,
    max_response_tokens: usize,
    interaction_recorder: Option<crate::learning::InteractionRecorder>,
    procedural_rule_repo: Option<cognitive::ProceduralRuleRepo>,
    tool_registry: Option<Arc<RwLock<tools::registry::ToolRegistry>>>,
    autotuner_hook: Option<Arc<dyn AutoTunerHook>>,
    user_situation: Option<Arc<tokio::sync::Mutex<cognitive::situation::UserSituation>>>,
    task_repo: Option<storage::TaskRepo>,
    active_view: Option<Arc<tokio::sync::RwLock<Option<context_engine::ActiveView>>>>,
    embedding_engine: Option<Arc<tools::EmbeddingEngine>>,
    domain_event_bus: Option<Arc<DomainEventBus>>,
    hot_config: Arc<RwLock<config::HotConfig>>,
    context_update_queue: Option<Arc<bus::ContextUpdateQueue>>,
}
```

Removed fields: `skill_catalog`, `skill_router`, `analyzer`, `config: PipelineConfig`, `strategy_repo`, `confidence_evaluator`, `active_profile`, `delegation_self_ref`, `current_event_tx`, `activated_skills`.

- [ ] **Step 2: Rewrite new() constructor**

```rust
impl AgentRuntime {
    pub fn new(
        context_engine: Arc<ContextEngine>,
        core: Arc<crate::execution::ExecutionCore>,
        cost_tracker: Arc<CostTracker>,
        execution_model: String,
        provider_name: String,
        context_window: usize,
        max_response_tokens: usize,
        hot_config: Arc<RwLock<config::HotConfig>>,
    ) -> Self {
        Self {
            context_engine,
            core,
            validator: ResponseValidator::new(max_response_tokens),
            cost_tracker,
            execution_model,
            provider_name,
            context_window,
            max_response_tokens,
            interaction_recorder: None,
            procedural_rule_repo: None,
            tool_registry: None,
            autotuner_hook: None,
            user_situation: None,
            task_repo: None,
            active_view: None,
            embedding_engine: None,
            domain_event_bus: None,
            hot_config,
            context_update_queue: None,
        }
    }
    // ... keep all with_* builder methods that set Option fields
    // ... remove: with_strategy_repo, with_confidence_evaluator,
    //     with_activated_skills, set_delegation_self_ref,
    //     active_profile_handle, skill_catalog_handle, skill_router_handle
}
```

- [ ] **Step 3: Rewrite process_message() — 3-phase pipeline**

```rust
pub async fn process_message(
    &self,
    message: &str,
    history: Vec<Message>,
    tool_definitions: &[serde_json::Value], // ALL tools, unfiltered
    ctx: &RoutingContext,
    event_tx: Option<tokio::sync::mpsc::Sender<AgentEvent>>,
    cancel_token: Option<tokio_util::sync::CancellationToken>,
    depth: DepthMode,
) -> Result<RuntimeResult> {
    let pipeline_start = Instant::now();
    let hot = self.hot_config.read().await;
    let safety_timeout_secs = hot.safety_timeout_secs;
    drop(hot);

    // Emit pipeline start
    if let Some(ref tx) = event_tx {
        let _ = tx.send(AgentEvent::PipelineStarted).await;
    }

    // ── Phase 1: Prepare ─────────────────────────────────────
    // Context assembly (memory retrieval, history, KLYNTBOT.md + skill listing
    // are injected by their ContextSource implementations)
    let retrieval_context = self.build_retrieval_context(&history).await;

    let context_request = ContextRequest {
        message_text: message.to_string(),
        history,
        system_prompt: String::new(), // Built by ContextSources
        strategy: ExecutionStrategy::ToolAssisted(30),
        tool_definitions: tool_definitions.to_vec(),
        context_window: self.context_window,
        session_key: Some(common::SessionKey::new(&ctx.channel, &ctx.chat_id).to_string()),
        retrieval_context,
    };

    let assemble_start = Instant::now();
    let assembled = self.context_engine.assemble(context_request).await;
    let assemble_ms = assemble_start.elapsed().as_millis() as u64;

    if let Some(ref tx) = event_tx {
        let _ = tx.send(AgentEvent::ContextAssembled {
            total_tokens: assembled.token_count,
            budget: self.context_window,
            duration_ms: assemble_ms,
        }).await;
    }

    // Create budget
    let mut budget = ExecutionBudget::new(depth, "general");

    // Build execution params — no tool filtering, no planning prompt
    let mut params = ExecutionParams::new(&self.execution_model)
        .with_max_iterations(budget.max_turns())
        .with_original_message(message.to_string())
        .with_context_window(self.context_window);

    if let Some(token) = cancel_token {
        params = params.with_cancel_token(token);
    }
    if let Some(ref queue) = self.context_update_queue {
        params = params.with_context_update_queue(Arc::clone(queue));
    }

    // ── Phase 2: Execute ─────────────────────────────────────
    let safety_timeout = Duration::from_secs(safety_timeout_secs.max(1));

    let loop_result = tokio::time::timeout(
        safety_timeout,
        execute_loop(
            &self.core,
            assembled.messages,
            tool_definitions, // ALL tools, flat
            &params,
            &mut budget,
            ctx,
            event_tx.clone(),
        ),
    )
    .await
    .map_err(|_| {
        common::KlyntbotError::Internal(format!(
            "Safety timeout ({safety_timeout_secs}s) — this is a bug, please report it"
        ))
    })??;

    // Phase 2b: Enrich (depth-gated)
    if depth == DepthMode::DeepThink || depth == DepthMode::Ultra {
        if let Some(ref tx) = event_tx {
            let _ = tx.send(AgentEvent::EnrichmentStarted {
                phase: "mirror_reflection".to_string(),
            }).await;
            let _ = tx.send(AgentEvent::EnrichmentComplete {
                phase: "mirror_reflection".to_string(),
                summary: "placeholder".to_string(),
            }).await;
        }
    }

    // ── Phase 3: Record ──────────────────────────────────────
    let mut validation = self.validator.validate(&loop_result.content);
    let pipeline_elapsed_ms = pipeline_start.elapsed().as_millis() as u64;

    // Record usage
    let cost = crate::output::cost_tracker::estimate_cost(
        &loop_result.usage, &self.execution_model,
    );
    if let Err(e) = self.cost_tracker.record(
        &loop_result.usage,
        &self.execution_model,
        &self.provider_name,
        &depth.to_string(),
        ctx.channel.as_str(),
    ).await {
        warn!("Failed to record usage: {e}");
    }

    if let Some(ref tx) = event_tx {
        let _ = tx.send(AgentEvent::UsageReport {
            prompt_tokens: loop_result.usage.prompt_tokens,
            completion_tokens: loop_result.usage.completion_tokens,
            cache_read_tokens: loop_result.usage.cache_read_tokens,
            cache_write_tokens: loop_result.usage.cache_write_tokens,
            estimated_cost_usd: cost,
            model: self.execution_model.clone(),
            response_time_ms: pipeline_elapsed_ms,
        }).await;

        if let Some(alert) = self.cost_tracker.check_budget().await {
            let _ = tx.send(AgentEvent::BudgetWarning {
                monthly_spend_usd: alert.monthly_spend_usd,
                monthly_budget_usd: alert.monthly_budget_usd,
                usage_percent: alert.usage_percent,
            }).await;
        }
    }

    // Record interaction
    if let Some(ref recorder) = self.interaction_recorder {
        let tools_used: Vec<&str> = loop_result.tool_calls.iter().map(|s| s.as_str()).collect();
        recorder.record(
            "flat", &tools_used, ctx.channel.as_str(), pipeline_elapsed_ms,
        ).await;
    }

    // AutoTuner hook
    if let Some(ref hook) = self.autotuner_hook {
        let tokens = loop_result.usage.prompt_tokens + loop_result.usage.completion_tokens;
        hook.on_message_completed(
            ctx.chat_id.as_str(), "flat", &depth.to_string(), tokens, pipeline_elapsed_ms,
        ).await;
    }

    let final_content = std::mem::take(&mut validation.filtered_content);

    Ok(RuntimeResult {
        content: final_content,
        mode_used: depth.to_string(),
        validation,
        agent_name: "klyntbot".to_string(),
        turns: loop_result.turns,
        budget_exhausted: loop_result.budget_exhausted,
        tool_calls: loop_result.tool_calls,
    })
}
```

- [ ] **Step 4: Update RuntimeResult — remove classification**

```rust
pub struct RuntimeResult {
    pub content: String,
    pub mode_used: String,
    pub validation: ValidationResult,
    pub agent_name: String,
    pub turns: u32,
    pub budget_exhausted: bool,
    pub tool_calls: Vec<String>,
}
```

Remove `classification: IntentAnalysis` field.

- [ ] **Step 5: Remove delegation-related methods**

Delete the `delegate()` method, `delegation_event_filter()` function, `filter_tools_for_profile()` function, `inject_delegation_tool()` function, `ORCHESTRATOR_ALLOWED_TOOLS` constant, `ORCHESTRATOR_AGENT` constant, and `MAX_DELEGATION_DEPTH` constant.

Remove the `DelegationHandler` trait impl block for `AgentRuntime`.

Simplify `build_retrieval_context()` — remove the `profile_name` parameter since there's no active profile.

- [ ] **Step 6: Update AgentLoop struct in agent_loop/mod.rs**

Remove `skill_catalog`, `skill_router` fields from `AgentLoop`. Remove any reload logic for skills (the `SkillStore` handles hot-reload separately). Update the `process()` method to pass all tools to `process_message()` without filtering.

- [ ] **Step 7: Update agent lib.rs re-exports**

Remove `pub mod intent_pipeline`. Remove re-exports of `IntentAnalysis`, `PipelineConfig`, `ComplexityLevel`, etc. Add `pub mod engines` if debate/interaction engines were moved.

- [ ] **Step 8: Fix all compilation errors**

Run: `cargo build -p agent 2>&1 | head -50`

Fix errors iteratively. Main issues will be:
- Removed types referenced in tests
- Removed imports
- Changed `process_message()` signature in callers

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "feat(agent): rewrite AgentRuntime as 3-phase flat pipeline, delete delegation"
```

---

### Task 7: Rewrite AgentLoop Builder

**Files:**
- Modify: `crates/agent/src/agent_loop/builder.rs`

Replace the skill catalog/router/analyzer construction with SkillStore + flat wiring.

- [ ] **Step 1: Replace skill discovery with SkillStore loading**

Remove the entire skill discovery block (lines ~244-319 in current code). Replace with:

```rust
// Load skills from disk
let data_dir = config.data_dir_path();
let skills_dir = data_dir.join("skills");
let skill_store = skill_system::SkillStore::load(&skills_dir)?;
let skill_store = Arc::new(tokio::sync::RwLock::new(skill_store));

// Build skill reference index for the skill_reference tool
let skill_bodies: std::collections::HashMap<String, String> = {
    let store = skill_store.read().await;
    store.build_reference_index()
};
let skill_reference_index = Arc::new(tools::SkillReferenceIndex::new(
    skill_bodies,
    std::collections::HashMap::new(), // No separate reference files in flat system
));
```

- [ ] **Step 2: Replace SkillContextSource with SoulContextSource + SkillListingSource**

Remove the `SkillContextSource` wiring. Replace with:

```rust
// Soul context (KLYNTBOT.md)
let soul_source = skill_system::SoulContextSource::load(&data_dir)?;
// Add to context engine sources
context_sources.push(Box::new(soul_source));

// Skill listing context (YAML frontmatter)
let listing_source = skill_system::SkillListingSource::new(Arc::clone(&skill_store));
context_sources.push(Box::new(listing_source));
```

- [ ] **Step 3: Remove IntentAnalyzer construction**

Delete the `IntentAnalyzer::new()` call and all its builder method chains (`.with_strategy_repo()`, `.with_embedder()`, `.with_semantic_fact_repo()`, `.with_overrides()`, `.with_autotuner()`).

- [ ] **Step 4: Remove ExecutionRouter/DirectEngine/ReactiveEngine construction**

These were already removed in the budget-bounded work, but verify there are no lingering references.

- [ ] **Step 5: Simplify AgentRuntime construction**

Replace the `AgentRuntime::new(...)` call with the simplified constructor:

```rust
let mut runtime = AgentRuntime::new(
    context_engine,
    execution_core,
    cost_tracker,
    config.agents.defaults.model.clone(),
    provider.name().to_string(),
    provider.context_window(),
    config.agents.defaults.max_tokens as usize,
    hot_config,
);

// Wire optional deps (keep these)
runtime = runtime
    .with_tool_registry(Arc::clone(&tool_registry))
    .with_interaction_recorder(interaction_recorder)
    .with_autotuner_hook(autotuner_hook)
    // ... etc
```

Remove: `.with_strategy_repo()`, `.with_confidence_evaluator()`, `.with_activated_skills()`.
Remove: `runtime.set_delegation_self_ref(...)` call.

- [ ] **Step 6: Build and fix errors**

Run: `cargo build -p agent 2>&1 | head -50`
Fix any remaining issues.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "refactor(agent): rewrite builder for flat skill system, remove catalog/router/analyzer"
```

---

### Task 8: Update App-Core and Simulator

**Files:**
- Modify: `crates/app-core/src/handlers/chat/streaming.rs` — Remove classification event handlers
- Modify: `crates/simulator/src/agent_harness.rs` — Simplify to flat tool pool
- Modify: `crates/simulator/src/harness.rs` — Remove skill references
- Modify: Any app-core files referencing `IntentAnalysis`, `PipelineConfig`, `SkillCatalog`

- [ ] **Step 1: Clean streaming.rs event handlers**

In `crates/app-core/src/handlers/chat/streaming.rs`, remove match arms for:
- `AgentEvent::ClassificationComplete` — no more classification
- Update `AgentEvent::ExecutionStarted` — simplify (no engine/mode tracking)

Remove `TransparencyClassification` and `TransparencyExecution` from the transparency struct construction. Remove `strategy_repo` references.

Also remove the `RuntimeResult.classification` field access — it no longer exists.

- [ ] **Step 2: Simplify simulator agent harness**

In `crates/simulator/src/agent_harness.rs`:
- Remove `SkillCatalog`, `SkillRouter`, `IntentAnalyzer` construction
- Remove `active_profile`, `activated_skills` shared state
- Remove `SkillContextSource` wiring
- Pass all tools flat to `AgentRuntime`
- Update `AgentRuntime::new()` call to match new constructor

- [ ] **Step 3: Search and fix all remaining references**

```bash
grep -rn "IntentAnalysis\|PipelineConfig\|SkillCatalog\|SkillRouter\|IntentAnalyzer\|DelegationHandler\|DelegationTool\|OrchestratorConfig\|SkillConfig\|ComplexityLevel\|AnalysisSource\|needs_orchestration\|active_profile\|activated_skills\|skill_catalog\|skill_router" crates/ --include="*.rs" | grep -v "target/" | grep -v "// " | grep -v "test"
```

Fix each remaining reference.

- [ ] **Step 4: Build full workspace**

Run: `cargo build --workspace 2>&1 | grep "^error" | head -20`
Expected: Clean build (ignoring any pre-existing warnings).

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor(app-core, simulator): update for flat skill system"
```

---

### Task 9: Delete Legacy Skill Directories + Clean Up

**Files:**
- Delete: `skills/general/` directory
- Delete: `skills/communication/` directory
- Delete: `skills/language-learning/` directory (if not converted)
- Modify: Various — final cleanup pass

- [ ] **Step 1: Delete unused skill directories**

```bash
rm -rf skills/general
rm -rf skills/communication
```

Check if `language-learning` should be kept or deleted. If it's not in the default skills list, delete it:
```bash
rm -rf skills/language-learning
```

- [ ] **Step 2: Clean up SkillPackage/SkillType/SkillCatalog types in skill-system/types.rs**

The `types.rs` file still has the old `SkillPackage`, `SkillCatalog`, `SkillType`, `SkillScope`, etc. Since `persona.rs` and `parser.rs` may still reference these types, check:

```bash
grep -rn "SkillPackage\|SkillCatalog\|SkillType\|SkillScope\|SkillMetadata" crates/skill-system/src/ --include="*.rs"
```

If only `parser.rs` and `persona.rs` use them (for persona skill parsing), keep the minimum needed types and delete the rest. If nothing uses them, delete `types.rs` entirely.

- [ ] **Step 3: Run full workspace clippy**

Run: `cargo clippy --workspace --all-targets 2>&1 | grep "^error" | head -10`
Expected: Zero errors (pre-existing desktop exceptions ok).

- [ ] **Step 4: Run all tests**

Run: `cargo nextest run -p agent -p config -p app-core -p skill-system 2>&1 | tail -15`
Expected: All tests pass.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "chore: delete legacy skill directories and clean up types"
```

---

### Task 10: Verify End-to-End — Chat Test

- [ ] **Step 1: Build and start the app**

```bash
cd desktop-ui && bun run dev &
cargo tauri dev
```

- [ ] **Step 2: Test single-domain — "Create area Work and Personal"**

Open Chrome to `localhost:1420`. Send: "Create for me two area Work and Personal"

Expected:
- Routes directly (no "General" in sidebar, no delegation)
- Calls `area:create` directly (not `finance:account_add`)
- No `delegate` tool call in the transparency panel
- Response completes in one loop without sub-agent

- [ ] **Step 3: Test cross-domain — "Create a task and log $50 for lunch"**

Send: "Create a task called review docs and log a $50 expense for lunch"

Expected:
- Single loop handles both: calls `tasks:create` then `finance:transaction_add`
- No delegation — model picks both tools from the flat pool
- Both operations complete in the same conversation turn

- [ ] **Step 4: Test skill_reference loading**

Send: "How should I organize my areas and projects?"

Expected:
- Model calls `skill_reference("task-management")` to load OKR+PARA instructions
- Response includes PARA methodology guidance
- Visible in tool calls: `skill_reference` appears

- [ ] **Step 5: Test casual chat (no tools)**

Send: "Hello, how are you?"

Expected:
- Direct response, no tool calls
- No skill_reference loaded (not needed for casual chat)
- Personality matches KLYNTBOT.md settings

- [ ] **Step 6: Run heuristic simulation tests**

```bash
cargo nextest run --test simulation -E 'not test(run_software_engineer_12mo) & not test(run_software_engineer_1mo) & not test(run_cognitive_llm) & not test(run_agent_validation)'
```

Expected: All pass.

- [ ] **Step 7: Commit any test fixes**

```bash
git add -A
git commit -m "test: verify flat skill system end-to-end"
```

---

## Self-Review

**Spec coverage check:**
- ✅ SkillStore loads `.md` files from `~/.klyntbot/skills/` → Task 1
- ✅ YAML frontmatter (name, description, whenToUse) → Task 1 (SkillFrontmatter)
- ✅ Default skills installed on first run → Task 1 (install_defaults)
- ✅ New notebook + learning skills → Task 2
- ✅ Existing skills converted to flat YAML → Task 2
- ✅ KLYNTBOT.md soul file → Task 3 (SoulContextSource)
- ✅ Skill listing in system prompt → Task 3 (SkillListingSource)
- ✅ Delete SkillRouter/IntentAnalyzer/DelegationTool → Tasks 4, 5
- ✅ Delete OrchestratorConfig/SkillConfig → Task 5
- ✅ 3-phase flat pipeline → Task 6
- ✅ Flat tool pool (no filtering) → Task 6
- ✅ Simplified AgentRuntime → Task 6
- ✅ Builder rewrite → Task 7
- ✅ App-core + simulator updates → Task 8
- ✅ Legacy cleanup → Task 9
- ✅ End-to-end verification → Task 10

**Placeholder scan:** No TBDs or TODOs. Phase 2b enrichment placeholder is carried from budget-bounded spec (intentional).

**Type consistency:**
- `SkillFrontmatter` / `SkillEntry` / `SkillStore` — consistent across Tasks 1, 3, 7
- `SoulContextSource` / `SkillListingSource` — consistent across Tasks 3, 7
- `RuntimeResult` — simplified in Task 6, consumed in Task 8
- `AgentRuntime::new()` — redefined in Task 6, called in Tasks 7, 8
- `process_message()` signature — defined in Task 6, called in Tasks 7, 8
