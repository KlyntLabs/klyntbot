# Skill-Driven Task Creation Guard — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Prevent the LLM from hallucinating task details when users give vague inputs like "buy", by adding a config-driven creation mode + code-level guard in TodoTool.

**Architecture:** Two-layer fix. Layer 1: Add `CreationMode` enum to config (`ask-first` | `yolo` | `party`), pass it to `TodoTool`. Layer 2: In TodoTool's `"add"` action, if mode is `ask-first` and the LLM provides optional fields without setting `confirmed: true`, return a soft rejection. Layer 3: Rewrite the `todo` skill as `always: true` so the full ask-first instructions are injected into every system prompt.

**Tech Stack:** Rust, serde, config crate, SKILL.md markdown

**Design doc:** `docs/plans/2026-02-19-skill-driven-task-creation-guard-design.md`

---

### Task 1: Add `CreationMode` enum to config

**Files:**
- Modify: `crates/config/src/schema/core.rs:387-401`

**Step 1: Write the failing test**

Add to `crates/config/src/schema/mod.rs` (in the existing `#[cfg(test)] mod tests` block):

```rust
#[test]
fn test_creation_mode_default_is_ask_first() {
    let config = TodoConfig::default();
    assert_eq!(config.creation_mode, CreationMode::AskFirst);
}

#[test]
fn test_creation_mode_deserialization() {
    let json = r#"{"creationMode": "yolo"}"#;
    let config: TodoConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.creation_mode, CreationMode::Yolo);
}

#[test]
fn test_creation_mode_serialization_camel_case() {
    let config = TodoConfig::default();
    let json = serde_json::to_value(&config).unwrap();
    assert_eq!(json["creationMode"], "ask-first");
}

#[test]
fn test_creation_mode_party() {
    let json = r#"{"creationMode": "party"}"#;
    let config: TodoConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.creation_mode, CreationMode::Party);
}

#[test]
fn test_creation_mode_unknown_falls_back() {
    // Unknown values should default to ask-first
    let json = r#"{"creationMode": "unknown"}"#;
    let config: TodoConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.creation_mode, CreationMode::AskFirst);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p config -E 'test(creation_mode)'`
Expected: FAIL — `CreationMode` type doesn't exist yet

**Step 3: Implement `CreationMode` enum and add to `TodoConfig`**

In `crates/config/src/schema/core.rs`, add before `TodoConfig`:

```rust
/// Task creation mode — controls whether the agent asks for details before creating tasks
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CreationMode {
    /// Ask the user for details via ask_user before creating (default)
    #[serde(rename = "ask-first")]
    AskFirst,
    /// Auto-enrich from conversation context, present for confirmation
    #[serde(rename = "yolo")]
    Yolo,
    /// Interactive brainstorming, one question at a time
    #[serde(rename = "party")]
    Party,
}

impl Default for CreationMode {
    fn default() -> Self {
        Self::AskFirst
    }
}
```

Add a custom deserializer helper so unknown values fall back to `AskFirst`:

```rust
fn deserialize_creation_mode<'de, D>(deserializer: D) -> std::result::Result<CreationMode, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    match s.as_str() {
        "ask-first" => Ok(CreationMode::AskFirst),
        "yolo" => Ok(CreationMode::Yolo),
        "party" => Ok(CreationMode::Party),
        _ => Ok(CreationMode::AskFirst), // graceful fallback
    }
}
```

Add to `TodoConfig` struct:

```rust
pub struct TodoConfig {
    #[serde(default)]
    pub notifications: TodoNotificationConfig,
    #[serde(default)]
    pub focus: TodoFocusConfig,
    #[serde(default)]
    pub enrichment: TodoEnrichmentConfig,
    #[serde(default)]
    pub search: TodoSearchConfig,
    #[serde(default)]
    pub daily_planning: DailyPlanningConfig,
    /// Task creation mode: ask-first (default), yolo, or party
    #[serde(default, deserialize_with = "deserialize_creation_mode")]
    pub creation_mode: CreationMode,
}
```

Make sure to export `CreationMode` from the config crate's public API (check `crates/config/src/lib.rs` re-exports).

**Step 4: Run tests to verify they pass**

Run: `cargo nextest run -p config -E 'test(creation_mode)'`
Expected: All 5 tests PASS

**Step 5: Run full config test suite**

Run: `cargo nextest run -p config`
Expected: All existing tests still pass (no regressions)

**Step 6: Commit**

```bash
git add crates/config/src/schema/core.rs crates/config/src/schema/mod.rs crates/config/src/lib.rs
git commit -m "feat(config): add CreationMode enum to TodoConfig

Adds ask-first/yolo/party creation modes with serde support,
camelCase JSON serialization, and graceful fallback for unknown values."
```

---

### Task 2: Add `confirmed` parameter + creation guard to TodoTool

**Files:**
- Modify: `crates/tools/src/todo.rs:26-41` (struct fields)
- Modify: `crates/tools/src/todo.rs:43-64` (constructor)
- Modify: `crates/tools/src/todo.rs:262-290` (parameters schema)
- Modify: `crates/tools/src/todo.rs:386-470` (execute/add action)

**Step 1: Write the guard logic test**

The guard is a pure function — extract it as a static method for testability. Add to the existing `#[cfg(test)] mod tests` in `todo.rs`:

```rust
#[test]
fn test_guard_triggers_on_vague_unconfirmed_task() {
    // Short title (1 word), 2+ optional fields, not confirmed → should trigger
    assert!(TodoTool::should_guard_creation(
        "buy",                    // title
        true,                     // has_description
        true,                     // has_priority
        false,                    // has_due_date
        false,                    // has_tags
        false,                    // confirmed
    ));
}

#[test]
fn test_guard_skips_when_confirmed() {
    // Same vague task but confirmed=true → should NOT trigger
    assert!(!TodoTool::should_guard_creation(
        "buy",
        true,
        true,
        false,
        false,
        true,  // confirmed!
    ));
}

#[test]
fn test_guard_skips_for_detailed_title() {
    // Title has 4+ words → should NOT trigger even without confirmation
    assert!(!TodoTool::should_guard_creation(
        "buy milk from the store",
        true,
        true,
        true,
        true,
        false,
    ));
}

#[test]
fn test_guard_skips_when_few_optional_fields() {
    // Short title but only 1 optional field → should NOT trigger
    assert!(!TodoTool::should_guard_creation(
        "buy",
        true,   // only description
        false,
        false,
        false,
        false,
    ));
}

#[test]
fn test_guard_skips_for_title_only() {
    // Short title, no optional fields → should NOT trigger (minimal creation is fine)
    assert!(!TodoTool::should_guard_creation(
        "buy",
        false,
        false,
        false,
        false,
        false,
    ));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p tools -E 'test(guard)'`
Expected: FAIL — `should_guard_creation` doesn't exist

**Step 3: Implement the guard function**

Add to the `impl TodoTool` block (after the existing static methods like `calculate_urgency`):

```rust
/// Check if the creation guard should trigger.
///
/// Returns `true` if the task looks like the LLM hallucinated details:
/// - Title is short (≤ 3 words)
/// - 2+ optional fields are filled (description, priority, due_date, tags)
/// - The LLM did NOT set `confirmed: true`
pub fn should_guard_creation(
    title: &str,
    has_description: bool,
    has_priority: bool,
    has_due_date: bool,
    has_tags: bool,
    confirmed: bool,
) -> bool {
    if confirmed {
        return false;
    }

    let word_count = title.split_whitespace().count();
    if word_count > 3 {
        return false;
    }

    let optional_count = [has_description, has_priority, has_due_date, has_tags]
        .iter()
        .filter(|&&v| v)
        .count();

    optional_count >= 2
}
```

**Step 4: Run guard tests to verify they pass**

Run: `cargo nextest run -p tools -E 'test(guard)'`
Expected: All 5 tests PASS

**Step 5: Add `creation_mode` field to `TodoTool` struct and constructor**

In `crates/tools/src/todo.rs`, add to the struct:

```rust
pub struct TodoTool {
    // ... existing fields ...
    /// Task creation mode from config
    creation_mode: config::CreationMode,
}
```

Update `new()`:

```rust
pub fn new(
    repo: storage::TodoRepo,
    max_focus_slots: usize,
    focus_deadline_hours: u64,
    timezone: String,
    creation_mode: config::CreationMode,
) -> Self {
    Self {
        repo,
        max_focus_slots,
        focus_deadline_hours,
        calendar_handler: None,
        enrichment_handler: None,
        embedding_handler: None,
        embedding_repo: None,
        semantic_threshold: 0.5,
        rrf_k: 60,
        timezone,
        feedback_handler: None,
        creation_mode,
    }
}
```

**Step 6: Add `confirmed` to the tool's JSON schema**

In the `parameters()` method, add alongside the other properties:

```rust
"confirmed": {
    "type": "boolean",
    "description": "Set to true ONLY after gathering task details via ask_user. Required when creation mode is ask-first and the title is short with multiple optional fields filled. Do NOT set to true if you haven't used ask_user to confirm details with the user."
},
```

**Step 7: Wire the guard into the `"add"` action**

In the `execute()` method, inside the `"add"` arm, right after extracting parameters (after line 402) and before creating the task (before line 404), add:

```rust
// Creation guard: reject vague tasks with hallucinated details in ask-first mode
if self.creation_mode == config::CreationMode::AskFirst {
    let confirmed = p.optional_bool("confirmed")?.unwrap_or(false);
    if Self::should_guard_creation(
        title,
        todo.description.is_some(),
        todo.priority.is_some(),
        todo.due_date.is_some(),
        !todo.tags.is_empty(),
        confirmed,
    ) {
        return Ok(
            "GUARD: This task has a short title with multiple auto-filled fields that weren't confirmed by the user. \
             Use ask_user FIRST to verify the task details (title, description, priority, due date, tags) with the user, \
             then call todo add again with confirmed=true. \
             Alternatively, call todo add with ONLY the title field and let the enrichment engine handle the rest."
            .to_string()
        );
    }
}
```

**Step 8: Update call site in `agent_loop.rs`**

In `crates/agent/src/agent_loop.rs`, update line 235:

```rust
let mut todo_tool = tools::todo::TodoTool::new(
    todo_repo,
    config.todo.focus.max_slots,
    config.todo.focus.deadline_hours,
    config.timezone.clone(),
    config.todo.creation_mode.clone(),
);
```

**Step 9: Run full tools + agent build**

Run: `cargo build -p tools -p agent`
Expected: Compiles with no errors

**Step 10: Run all tests**

Run: `cargo nextest run -p tools -p agent -p config`
Expected: All tests pass

**Step 11: Commit**

```bash
git add crates/tools/src/todo.rs crates/agent/src/agent_loop.rs
git commit -m "feat(tools): add creation guard to TodoTool

Adds confirmed parameter and should_guard_creation() heuristic.
In ask-first mode, rejects vague tasks (≤3 word title + 2+ optional
fields) unless confirmed=true. Respects CreationMode from config."
```

---

### Task 3: Rewrite the `todo` skill with `always: true`

**Files:**
- Modify: `skills/todo/SKILL.md`
- Modify: `crates/agent/src/context.rs:390-394` (remove hardcoded instruction)

**Step 1: Rewrite `skills/todo/SKILL.md`**

Replace the full content of `skills/todo/SKILL.md`:

```markdown
---
name: todo
description: Task creation workflow — ask-first by default, with confidence scoring and enrichment modes.
metadata: '{"klyntbot":{"triggers":["todo","task","focus"],"always":true}}'
---

# Todo Task Creation

## CRITICAL RULE: Ask Before Creating

When the user asks to create a task, you MUST follow this workflow:

### Step 1: Assess — Is the request detailed enough?

A request is "detailed enough" if it has:
- A clear title (> 3 words describing a specific action)
- OR the user explicitly provides priority, due date, or description

**Detailed enough examples (create immediately):**
- "add task: buy milk from the corner store, due tomorrow"
- "create task: fix authentication bug in login flow, priority high"
- "todo: review PR #42 for the payments refactor"

**NOT detailed enough examples (must ask first):**
- "add task: buy"
- "create task: fix"
- "todo: meeting"
- "task: stuff"

### Step 2: If NOT detailed enough — Use ask_user FIRST

Call the `ask_user` tool to gather details BEFORE calling `todo add`:

```json
{
  "title": "New Task Details",
  "questions": [
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

Call `todo add` with all user-provided fields and `confirmed: true`:

```json
{
  "action": "add",
  "title": "Buy milk from the corner store",
  "due_date": "tomorrow",
  "confirmed": true
}
```

### NEVER DO THIS

- NEVER expand a vague title into a specific one without asking (e.g., "buy" → "Buy groceries")
- NEVER invent a description the user didn't provide
- NEVER guess priority, due date, or tags
- NEVER call todo add with optional fields the user didn't explicitly state
- If in doubt, call todo add with ONLY the title — the enrichment engine will suggest improvements

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
```

**Step 2: Remove hardcoded ask_user instruction from context.rs**

In `crates/agent/src/context.rs`, remove lines 390-394:

```
**Creating To-Do Tasks:**
- **IMPORTANT:** When the user asks to create a todo task, use ask_user FIRST to gather details (title, description, priority, due date, tags)
- Do NOT create the task and then ask for improvements - get the information BEFORE creation
- After ask_user returns with answers, THEN call the todo tool with complete information
- This creates better tasks and avoids the need for updates
```

Replace with a single line referencing the skill:

```
**Creating To-Do Tasks:** Follow the instructions in the `todo` skill (always loaded).
```

**Step 3: Verify the skill loads as always-on**

Run: `cargo nextest run -p agent -E 'test(skill)' --nocapture`
Expected: The todo skill should appear in always-loaded skills. If no existing test covers this, verify manually by checking that `skills.get_always_loaded()` includes "todo".

**Step 4: Run full workspace build + clippy**

Run: `cargo build --workspace && cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings, 0 errors

**Step 5: Commit**

```bash
git add skills/todo/SKILL.md crates/agent/src/context.rs
git commit -m "feat(skills): rewrite todo skill as always-loaded with ask-first workflow

The todo skill is now always injected into the system prompt with
explicit rules: assess detail level, use ask_user for vague inputs,
never hallucinate task fields. Removes hardcoded instruction from
context.rs in favor of the skill."
```

---

### Task 4: Integration test — verify the full flow

**Files:**
- Test: manual verification via `klyntbot chat`

**Step 1: Build the binary**

Run: `cargo build`
Expected: Clean build

**Step 2: Run the full test suite**

Run: `cargo nextest run --workspace && cargo test --workspace --doc`
Expected: All tests pass

**Step 3: Run clippy and fmt**

Run: `cargo clippy --workspace --all-targets --all-features && cargo fmt --all --check`
Expected: 0 warnings, 0 formatting issues

**Step 4: Manual smoke test (ask-first mode)**

Run: `RUST_LOG=debug,info ./target/debug/klyntbot chat`

Test 1 — vague input:
```
> create task: buy
```
Expected: Agent uses ask_user to clarify, does NOT create "Buy groceries" immediately.

Test 2 — detailed input:
```
> create task: buy milk from the store, due tomorrow, priority high
```
Expected: Creates task immediately with confirmed=true.

Test 3 — title-only minimal creation:
```
> add task: meeting
```
Expected: Either asks for details OR creates with just "meeting" as title (guard allows title-only).

**Step 5: Commit final state**

If any adjustments were needed during smoke testing:

```bash
git add -A
git commit -m "fix: adjustments from smoke testing task creation guard"
```

---

### Task 5: Update CLAUDE.md with new config option

**Files:**
- Modify: `CLAUDE.md`

**Step 1: Add `creationMode` to the config documentation**

In the "Enrichment Configuration" section of CLAUDE.md, add a new section:

```markdown
## Task Creation Mode

Controls how klyntbot handles task creation — whether it asks for details first or auto-fills.

**Config schema** (`~/.klyntbot/config.json`):
```json
{
  "todo": {
    "creationMode": "ask-first"
  }
}
```

**Values:**
- `"ask-first"` (default): Uses `ask_user` to gather details before creating vague tasks. The `confirmed` parameter must be set on `todo add` calls with optional fields.
- `"yolo"`: Auto-enriches from conversation context, presents suggestions for approval before applying.
- `"party"`: Interactive brainstorming — asks targeted questions one at a time to build up the task.

**How it works:**
1. User says "create task: buy"
2. In `ask-first` mode: agent calls `ask_user` to clarify title, priority, due date
3. User responds with details
4. Agent calls `todo add` with gathered details + `confirmed: true`
5. Enrichment engine adds any remaining fields (estimated time, etc.)
```

**Step 2: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: add creationMode config to CLAUDE.md"
```
