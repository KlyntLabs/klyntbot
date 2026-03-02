# Area-Task Requirement Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make the LLM always require an area when creating tasks by giving it area context and updating the todo skill workflow.

**Architecture:** Three targeted changes: (1) new `AreaSource` context source injects available areas into every system prompt, (2) `to_context_string()` JOIN to show area names on active tasks, (3) todo skill rewrite to enforce area-first workflow.

**Tech Stack:** Rust (async_trait, sqlx, chrono, tokio), Markdown (skill file)

---

### Task 1: Write failing test for `to_context_string()` area name JOIN

**Files:**
- Modify: `crates/storage/src/repos/tests/action_repo_tests.rs:818-833`

**Step 1: Update existing test to assert area name in context string**

The test `context_string_includes_active_actions` already creates an area and a task. Update it to also assert the area name appears in the output.

```rust
#[tokio::test]
async fn context_string_includes_active_actions() {
    let Some((repo, area_repo)) = test_action_repo().await else {
        return;
    };
    // Create area with a known name
    let area_id = "ctx-area".to_string();
    let _ = area_repo
        .create(&AreaRow {
            id: area_id.clone(),
            name: "Work".to_string(),
            description: None,
            color: "blue".to_string(),
            icon: None,
            position: 0,
            status: "active".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        })
        .await;

    let id = unique_id("ctx");
    repo.add(&sample_action(&id, "Context task", &area_id))
        .await
        .unwrap();
    let ctx = repo.to_context_string().await.unwrap();
    assert!(ctx.contains("Context task"));
    assert!(ctx.contains("(Work)"), "context string should include area name, got: {ctx}");

    // Cleanup
    let _ = repo.delete(&id).await;
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p storage -E 'test(context_string_includes_active_actions)' --nocapture`
Expected: FAIL — current output doesn't contain `(Work)`

---

### Task 2: Implement `to_context_string()` area JOIN

**Files:**
- Modify: `crates/storage/src/repos/action_repo.rs:838-876`

**Step 1: Update query to JOIN areas table and include area_name**

```rust
/// Build a context string of active actions for LLM context injection.
#[allow(clippy::type_complexity)]
pub async fn to_context_string(&self) -> Result<String, StorageError> {
    let rows: Vec<(
        String,
        String,
        Option<i16>,
        Option<chrono::DateTime<chrono::Utc>>,
        String,
    )> = sqlx::query_as(
        r#"
            SELECT a.title, a.status, a.priority, a.focused_at, ar.name
            FROM actions a
            JOIN areas ar ON a.area_id = ar.id
            WHERE a.status IN ('todo', 'doing')
              AND a.is_template = FALSE
            ORDER BY
                CASE WHEN a.focused_at IS NOT NULL THEN 0 ELSE 1 END,
                a.priority ASC NULLS LAST,
                a.created_at
            "#,
    )
    .fetch_all(&self.pool)
    .await?;

    if rows.is_empty() {
        return Ok("No active tasks.".to_string());
    }

    let mut out = String::from("Active tasks:\n");
    for (title, status, priority, focused_at, area_name) in &rows {
        let focus_marker = if focused_at.is_some() {
            " [FOCUSED]"
        } else {
            ""
        };
        let priority_str = priority.map(|p| format!(" P{p}")).unwrap_or_default();
        out.push_str(&format!(
            "- [{}]{}{} {} ({})\n",
            status, focus_marker, priority_str, title, area_name
        ));
    }
    Ok(out)
}
```

**Step 2: Run test to verify it passes**

Run: `cargo nextest run -p storage -E 'test(context_string_includes_active_actions)' --nocapture`
Expected: PASS

**Step 3: Run full storage tests to check for regressions**

Run: `cargo nextest run -p storage`
Expected: All tests PASS

**Step 4: Commit**

```bash
git add crates/storage/src/repos/action_repo.rs crates/storage/src/repos/tests/action_repo_tests.rs
git commit -m "feat(storage): include area name in to_context_string() output"
```

---

### Task 3: Create `AreaSource` context source

**Files:**
- Create: `crates/agent/src/context_sources/area.rs`

**Step 1: Create the AreaSource file**

Follow the exact pattern of `TodoSource` (`crates/agent/src/context_sources/todo.rs`):

```rust
//! Area context source — available areas for the LLM.

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use context_engine::source::{ContextSource, SourceContext};
use tokio::sync::Mutex;
use tracing::warn;

/// Default TTL for cached area context (seconds).
const AREA_CACHE_TTL_SECS: i64 = 60;

/// Provides available areas summary with TTL caching.
pub struct AreaSource {
    repo: storage::AreaRepo,
    cache: Mutex<Option<CachedValue>>,
}

struct CachedValue {
    content: String,
    expires_at: DateTime<Utc>,
}

impl AreaSource {
    pub fn new(repo: storage::AreaRepo) -> Self {
        Self {
            repo,
            cache: Mutex::new(None),
        }
    }
}

#[async_trait]
impl ContextSource for AreaSource {
    fn name(&self) -> &str {
        "area"
    }

    fn priority(&self) -> u8 {
        75
    }

    async fn provide(&self, _ctx: &SourceContext) -> Option<String> {
        // Check TTL cache
        {
            let cache = self.cache.lock().await;
            if let Some(ref cached) = *cache {
                if Utc::now() < cached.expires_at {
                    return if cached.content.trim().is_empty() {
                        None
                    } else {
                        Some(cached.content.clone())
                    };
                }
            }
        }

        // Cache miss — fetch fresh
        let content = match self.repo.list(Some("active")).await {
            Ok(areas) => {
                if areas.is_empty() {
                    String::new()
                } else {
                    let mut out = String::from("Available areas:\n");
                    for area in &areas {
                        out.push_str(&format!("- {} (ID: {})\n", area.name, area.id));
                    }
                    out
                }
            }
            Err(e) => {
                warn!("SQL area context failed: {}", e);
                String::new()
            }
        };

        let result = if content.trim().is_empty() {
            None
        } else {
            Some(content.clone())
        };

        // Store in cache
        {
            let mut cache = self.cache.lock().await;
            *cache = Some(CachedValue {
                content,
                expires_at: Utc::now() + Duration::seconds(AREA_CACHE_TTL_SECS),
            });
        }

        result
    }
}
```

**Step 2: Verify it compiles**

Run: `cargo build -p agent`
Expected: PASS (no errors)

---

### Task 4: Register AreaSource in mod.rs and builder.rs

**Files:**
- Modify: `crates/agent/src/context_sources/mod.rs`
- Modify: `crates/agent/src/agent_loop/builder.rs:161-172`

**Step 1: Add module and re-export in mod.rs**

Add after the `pub mod todo;` line:

```rust
pub mod area;
```

Add after the `pub use todo::TodoSource;` line:

```rust
pub use area::AreaSource;
```

**Step 2: Wire into builder sources list**

In `builder.rs`, add import at the top (alongside existing context source imports):

The `use` for context sources in builder.rs — find where `TodoSource` is referenced and ensure `AreaSource` is imported via the `context_sources` module.

Add to the sources vec (after `TodoSource` line):

```rust
Box::new(AreaSource::new(repos.areas.clone())),
```

The full sources vec becomes:
```rust
let mut sources: Vec<Box<dyn ContextSource>> = vec![
    Box::new(IdentitySource::new(
        workspace.clone(),
        config.timezone.clone(),
    )),
    Box::new(BootstrapSource::new(workspace.clone())),
    Box::new(MemorySource::new(memory_store)),
    Box::new(AreaSource::new(repos.areas.clone())),
    Box::new(TodoSource::new(repos.actions.clone())),
    Box::new(confidence_source),
    Box::new(SkillSummarySource::new(Arc::clone(&skill_manager))),
    Box::new(SkillContentSource::new(Arc::clone(&skill_manager))),
];
```

**Step 3: Verify it compiles and tests pass**

Run: `cargo build -p agent && cargo nextest run -p agent`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/agent/src/context_sources/area.rs crates/agent/src/context_sources/mod.rs crates/agent/src/agent_loop/builder.rs
git commit -m "feat(agent): add AreaSource context source for system prompt"
```

---

### Task 5: Update todo skill with area-first workflow

**Files:**
- Modify: `skills/todo/SKILL.md`

**Step 1: Rewrite the skill file**

Replace the entire skill file with the updated version that adds Step 0 (area resolution) before the existing workflow. Key changes:

1. Add new section **Step 0: Determine the area** before Step 1
2. Rules: if 1 area → auto-assign + mention; if multiple → ask user via `ask_user`; if user specified area → use directly
3. Update the `ask_user` example to include area selection question
4. Update the `todo add` examples to always include `area_id`
5. Add `area_id` to the "NEVER DO THIS" rules: NEVER guess `area_id`
6. Note that `key_result_id` is optional, discovered via `area show` → `project list`

Full content:

```markdown
---
name: todo
description: Task creation workflow — ask-first by default, with confidence scoring and enrichment modes.
metadata: '{"klyntbot":{"triggers":["todo","task","focus"],"always":true}}'
---

# Todo Task Creation

## CRITICAL RULE: Every Task Needs an Area

Every task belongs to an area. You MUST determine the area before creating any task. The system prompt includes "Available areas" with names and IDs — use those.

## CRITICAL RULE: Ask Before Creating

When the user asks to create a task, you MUST follow this workflow:

### Step 0: Determine the area

Before anything else, resolve which area this task belongs to:

- **If the user specifies an area** (e.g., "add task to Work: fix bug") → use that area's ID directly
- **If only 1 active area exists** → auto-assign it. Mention which area in your response (e.g., "Added to Work area")
- **If multiple active areas exist and user didn't specify** → ask the user via `ask_user` with a `single_select` of available areas:

```json
{
  "id": "area",
  "title": "Area",
  "text": "Which area does this belong to?",
  "type": "single_select",
  "options": [
    {"value": "area_abc", "label": "Personal"},
    {"value": "area_def", "label": "Work"}
  ]
}
```

Use the area names and IDs from the "Available areas" section in your context.

### Step 1: Assess — Is the request detailed enough?

A request is "detailed enough" if it has:
- A clear title (> 3 words describing a specific action)
- OR the user explicitly provides priority, due date, or description

**Detailed enough examples (create immediately):**
- "add task to Work: buy milk from the corner store, due tomorrow"
- "create task: fix authentication bug in login flow, priority high"
- "todo: review PR #42 for the payments refactor"

**NOT detailed enough examples (must ask first):**
- "add task: buy"
- "create task: fix"
- "todo: meeting"
- "task: stuff"

### Step 2: If NOT detailed enough — Use ask_user FIRST

Call the `ask_user` tool to gather details BEFORE calling `todo add`. Include area selection if multiple areas exist and area wasn't already determined in Step 0:

```json
{
  "title": "New Task Details",
  "questions": [
    {
      "id": "area",
      "title": "Area",
      "text": "Which area does this belong to?",
      "type": "single_select",
      "options": [
        {"value": "area_abc", "label": "Personal"},
        {"value": "area_def", "label": "Work"}
      ]
    },
    {
      "id": "title",
      "title": "Title",
      "text": "What specifically do you want to do? (e.g., 'buy groceries for dinner tonight')",
      "type": "free_text",
      "placeholder": "Describe the task..."
    },
    {
      "id": "priority",
      "title": "Priority",
      "text": "How urgent is this?",
      "type": "single_select",
      "options": [
        {"value": "1", "label": "Urgent", "description": "Do today"},
        {"value": "2", "label": "High", "description": "Do this week"},
        {"value": "3", "label": "Medium", "description": "Normal priority"},
        {"value": "4", "label": "Low", "description": "When you get to it"}
      ]
    }
  ]
}
```

After ask_user returns, call `todo add` with the gathered details AND `confirmed: true`.

### Step 3: If detailed enough — Create with confirmed=true

Call `todo add` with all user-provided fields, `area_id`, and `confirmed: true`:

```json
{
  "action": "add",
  "title": "Buy milk from the corner store",
  "area_id": "area_abc",
  "due_date": "tomorrow",
  "confirmed": true
}
```

### NEVER DO THIS

- NEVER call `todo add` without `area_id`
- NEVER guess or fabricate an `area_id` — use the IDs from "Available areas" context
- NEVER expand a vague title into a specific one without asking (e.g., "buy" → "Buy groceries")
- NEVER invent a description the user didn't provide
- NEVER guess priority, due date, or tags
- NEVER call todo add with optional fields the user didn't explicitly state
- If in doubt, call todo add with ONLY the title and area_id — the enrichment engine will suggest improvements

## Optional: Link to Project or OKR

After confirming the area, you can optionally link the task to a project or OKR key result within that area:

- Use `area show` with the area ID to see its projects
- Use `project list` filtered by area to discover projects
- Set `key_result_id` on the task to link it to an OKR key result

This is optional but improves the confidence score.

## Confidence Scoring

Tasks are scored 0.0-1.0 based on:
- Title quality (25%): > 3 words
- Description (25%): > 10 chars
- Priority (15%): set to 1-5
- Due date (20%): concrete deadline
- Tags (15%): at least one tag

After creating a task, show the confidence score. If < 80%, offer enrichment options via ask_user.

## Focus Mode

- Max 3 tasks focused simultaneously
- 18-hour deadline per focused task
- Auto-unfocus when expired

## Deep Dive

For advanced task management topics, load these references with `read_file` when needed:

- **Enrichment engine**: See `CLAUDE.md` section "Enrichment Configuration" for keyword-based priority/duration inference
- **Semantic search**: See `CLAUDE.md` section "Semantic Search" for embedding-based task discovery
- **Creation modes**: See `CLAUDE.md` section "Task Creation Mode" for ask-first vs yolo vs party modes
```

**Step 2: Verify no syntax errors in frontmatter**

Visually confirm YAML frontmatter has correct `name`, `description`, and `metadata` fields.

**Step 3: Commit**

```bash
git add skills/todo/SKILL.md
git commit -m "feat(skills): add area requirement to todo task creation workflow"
```

---

### Task 6: Run full workspace build and tests

**Files:** None (verification only)

**Step 1: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

**Step 2: Run format check**

Run: `cargo fmt --all --check`
Expected: PASS

**Step 3: Run all tests**

Run: `cargo nextest run --workspace`
Expected: All tests PASS

**Step 4: Final commit if any fixups needed**

If clippy or fmt required changes, commit those separately.
