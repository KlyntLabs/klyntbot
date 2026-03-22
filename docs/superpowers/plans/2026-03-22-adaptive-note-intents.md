# Adaptive Note Intents Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make notes purpose-aware (study/research/capture) so the system adapts behavior — atom extraction, insight tabs, and agent tool — per intent, eliminating token waste on non-study notes and providing research-specific analysis tabs.

**Architecture:** Add `intent` column to notebooks and `intent_override` to notes. `NoteIntent` enum in `feature-notes` crate resolves effective intent. Backend gates atom extraction on study-only. Frontend swaps InsightReviewPanel tabs based on intent. Research gets 4 instant graph-driven tabs + 3 opt-in LLM tabs. Capture gets no panel.

**Tech Stack:** Rust (SQLite, sqlx, tokio, serde), TypeScript (React, Tailwind v4, Tiptap), Tauri 2 IPC

**Spec:** `docs/superpowers/specs/2026-03-22-adaptive-note-intents-design.md`

---

## File Structure

### New files
- `crates/feature-notes/src/intent.rs` — `NoteIntent` enum + `effective()` resolver
- `crates/app-core/src/handlers/notes/research.rs` — research tab handlers (graph queries + LLM deepen)
- `crates/desktop-shared/src/commands/research.rs` — response types for research tabs
- `desktop-ui/src/features/notes/components/research/` — research tab components (CrossDomainLinksTab, EvidenceMapTab, SourceTrailTab, RisksGapsTab, DeepenTab)
- `desktop-ui/src/features/notes/components/PurposeBadge.tsx` — intent badge + override popover
- `desktop-ui/src/features/notes/hooks/useResearchTabs.ts` — IPC hooks for research tab data

### Modified files
- `crates/feature-notes/migrations/001_create_notes.sql` — add `intent` to notebooks, `intent_override` to notes
- `crates/feature-notes/src/lib.rs` — export `intent` module, bump migration version
- `crates/feature-notes/src/models.rs` — add intent fields to NoteRow, NotebookRow, Note, Notebook
- `crates/feature-notes/src/repo/notes.rs` — add intent columns to INSERT/UPDATE/SELECT queries
- `crates/feature-notes/src/repo/notebooks.rs` — add intent column to INSERT/UPDATE/SELECT queries
- `crates/feature-notes/src/repo/links.rs` — add intent-filtered `list_notes_by_entity` variant
- `crates/feature-notes/src/tool.rs` — add `intent` parameter to create/search/list_by_entity actions
- `crates/app-core/src/handlers/notes/crud.rs` — gate `NoteContentChanged` on study intent
- `crates/app-core/src/handlers/notes/insight.rs` — route to study vs research insight handlers
- `crates/app-core/src/handlers/notes/insight_prompts.rs` — add research prompt templates
- `crates/app-core/src/handlers/notes/mod.rs` — register research module
- `crates/cognitive/src/services/atom_extraction.rs` — add safety-net intent check
- `crates/desktop-shared/src/commands/notes.rs` — add intent fields to NoteResponse, NotebookResponse, create/update params
- `crates/desktop/src/commands/notes.rs` — add research IPC commands + DEV_COMMANDS
- `crates/desktop/src/dev_server/mod.rs` — register research commands
- `crates/bus/src/domain_events.rs` — no change needed (events are sufficient)
- `desktop-ui/src/features/notes/components/InsightReviewPanel.tsx` — branch on effective intent
- `desktop-ui/src/features/notes/components/editor/EditorToolbar.tsx` — hide Generate Cards for non-study
- `desktop-ui/src/features/notes/pages/KnowledgeBasePage.tsx` — pass intent to InsightReviewPanel

---

## Task 1: NoteIntent enum + effective() resolver

**Files:**
- Create: `crates/feature-notes/src/intent.rs`
- Modify: `crates/feature-notes/src/lib.rs`

- [ ] **Step 1: Write failing tests for NoteIntent::effective()**

```rust
// crates/feature-notes/src/intent.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn study_notebook_no_override() {
        assert_eq!(NoteIntent::effective(None, Some("study")), NoteIntent::Study);
    }

    #[test]
    fn research_notebook_no_override() {
        assert_eq!(NoteIntent::effective(None, Some("research")), NoteIntent::Research);
    }

    #[test]
    fn capture_notebook_no_override() {
        assert_eq!(NoteIntent::effective(None, Some("capture")), NoteIntent::Capture);
    }

    #[test]
    fn override_takes_precedence() {
        assert_eq!(NoteIntent::effective(Some("research"), Some("study")), NoteIntent::Research);
    }

    #[test]
    fn orphan_note_defaults_to_capture() {
        assert_eq!(NoteIntent::effective(None, None), NoteIntent::Capture);
    }

    #[test]
    fn orphan_note_with_override() {
        assert_eq!(NoteIntent::effective(Some("study"), None), NoteIntent::Study);
    }

    #[test]
    fn unknown_value_falls_back_to_capture() {
        assert_eq!(NoteIntent::effective(Some("typo"), None), NoteIntent::Capture);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p feature-notes -E 'test(intent)'`
Expected: FAIL — module doesn't exist yet

- [ ] **Step 3: Implement NoteIntent enum**

```rust
// crates/feature-notes/src/intent.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NoteIntent {
    Study,
    Research,
    Capture,
}

impl NoteIntent {
    /// Resolve effective intent from note override + notebook default.
    /// Orphan notes (no notebook) and unrecognized values fall back to Capture.
    pub fn effective(note_override: Option<&str>, notebook_intent: Option<&str>) -> Self {
        let raw = note_override.or(notebook_intent).unwrap_or("capture");
        match raw {
            "study" => Self::Study,
            "research" => Self::Research,
            _ => Self::Capture,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Study => "study",
            Self::Research => "research",
            Self::Capture => "capture",
        }
    }
}

impl std::fmt::Display for NoteIntent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    // ... tests from Step 1
}
```

- [ ] **Step 4: Export from lib.rs**

In `crates/feature-notes/src/lib.rs`, add `pub mod intent;` after line 6.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo nextest run -p feature-notes -E 'test(intent)'`
Expected: all 7 tests PASS

- [ ] **Step 6: Commit**

```bash
git add crates/feature-notes/src/intent.rs crates/feature-notes/src/lib.rs
git commit -m "feat(notes): add NoteIntent enum with effective() resolver"
```

---

## Task 2: Schema migration — add intent columns

**Files:**
- Modify: `crates/feature-notes/migrations/001_create_notes.sql`
- Modify: `crates/feature-notes/src/lib.rs` (bump migration version)

- [ ] **Step 1: Add intent column to notebooks CREATE TABLE**

In `crates/feature-notes/migrations/001_create_notes.sql`, add after line 8 (`sort_order`):

```sql
    intent      TEXT NOT NULL DEFAULT 'study' CHECK(intent IN ('study', 'research', 'capture')),
```

- [ ] **Step 2: Add intent_override column to notes CREATE TABLE**

In the same file, add after line 29 (`last_visited_at`):

```sql
    intent_override TEXT CHECK(intent_override IN ('study', 'research', 'capture')),
```

- [ ] **Step 3: Bump migration version**

In `crates/feature-notes/src/lib.rs`, change `version: 6` to `version: 7` on line 33. This causes the migration runner to treat it as a new migration. Since we're pre-release with no user data to preserve, the migration runner will drop and recreate the tables (the `CREATE TABLE IF NOT EXISTS` + version bump ensures this). Any existing dev database will need to be wiped (`rm ~/.klyntbot-dev/data.db`) or the migration runner handles the version change by re-running the SQL.

- [ ] **Step 4: Verify build compiles**

Run: `cargo build -p feature-notes`
Expected: SUCCESS

- [ ] **Step 5: Commit**

```bash
git add crates/feature-notes/migrations/001_create_notes.sql crates/feature-notes/src/lib.rs
git commit -m "feat(notes): add intent columns to notebooks and notes schema"
```

---

## Task 3: Domain models — add intent fields to Row types and domain types

**Files:**
- Modify: `crates/feature-notes/src/models.rs`

- [ ] **Step 1: Add intent to NotebookRow**

In `crates/feature-notes/src/models.rs`, add after `sort_order` field (line 60):

```rust
    pub intent: String,
```

- [ ] **Step 2: Add intent_override to NoteRow**

Add after `last_visited_at` field (line 81):

```rust
    pub intent_override: Option<String>,
```

Also add to `NoteSearchResult` after `last_visited_at` (line 102):

```rust
    pub intent_override: Option<String>,
```

- [ ] **Step 3: Add intent to Notebook domain model**

Add after `sort_order` field (line 14):

```rust
    pub intent: crate::intent::NoteIntent,
```

- [ ] **Step 4: Add intent fields to Note domain model**

Add after `tags` field (line 31):

```rust
    pub intent: crate::intent::NoteIntent,
```

- [ ] **Step 5: Update From<NotebookRow> for Notebook**

In the `From<NotebookRow>` impl (around line 142), add intent parsing:

```rust
    intent: crate::intent::NoteIntent::effective(None, Some(&r.intent)),
```

- [ ] **Step 6: Update Note::from_row**

In `Note::from_row` (around line 166), add a third parameter `notebook_intent: Option<&str>` and compute:

```rust
    pub fn from_row(row: NoteRow, tags: Vec<String>, notebook_intent: Option<&str>) -> Self {
        let intent = crate::intent::NoteIntent::effective(
            row.intent_override.as_deref(),
            notebook_intent,
        );
        Self {
            // ... existing fields ...
            intent,
            // ...
        }
    }
```

Update the `From<NoteRow>` impl to pass `None` for notebook_intent (orphan default).

- [ ] **Step 7: Fix all compilation errors from signature change**

Run: `cargo build -p feature-notes 2>&1 | head -50`

Known call sites that use `Note::from_row(row, tags)` and need the third `notebook_intent` parameter:
- `crates/feature-notes/src/tool.rs:147` — in the tool's execute handler. Pass `None` (tool has no notebook context).
- Any other callers found via: `grep -rn "from_row(" crates/ --include="*.rs" | grep -v "test"`

Update each to pass the notebook intent where available, or `None` where not. Note: `feature-notes/src/tool.rs` is in the same crate as `models.rs`, so `cargo build -p feature-notes` will catch it before the workspace build.

- [ ] **Step 8: Verify full workspace builds**

Run: `cargo build --workspace`
Expected: SUCCESS

- [ ] **Step 9: Commit**

```bash
git add crates/feature-notes/src/models.rs
git commit -m "feat(notes): add intent fields to domain models and row types"
```

---

## Task 4: Repository layer — intent in CRUD queries

**Files:**
- Modify: `crates/feature-notes/src/repo/notes.rs`
- Modify: `crates/feature-notes/src/repo/notebooks.rs`

- [ ] **Step 1: Update create_note INSERT to include intent_override**

In `crates/feature-notes/src/repo/notes.rs`, line 11: add `intent_override` to the INSERT column list and add a `.bind(&row.intent_override)` call.

- [ ] **Step 2: Update create_notebook INSERT to include intent**

In `crates/feature-notes/src/repo/notebooks.rs`, line 13: add `intent` to the INSERT column list and add a `.bind(&row.intent)` call.

- [ ] **Step 3: Add get_notebook_intent repo method**

In `crates/feature-notes/src/repo/notebooks.rs`, add to `impl NoteRepo` (notebook methods live on `NoteRepo`, not a separate `NotebookRepo`):

```rust
    pub async fn get_notebook_intent(&self, notebook_id: &str) -> Result<Option<String>, StorageError> {
        let result = sqlx::query_scalar::<_, String>(
            "SELECT intent FROM notebooks WHERE id = ?1"
        )
        .bind(notebook_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(result)
    }
```

- [ ] **Step 4: Add intent-filtered search_notes variant**

In `crates/feature-notes/src/repo/notes.rs`, add a method. Note: the existing `search_notes` uses FTS5 via `notes_fts` virtual table. For intent filtering, use a post-filter approach — join the FTS results back to the notes table + notebooks table to resolve effective intent. Do NOT attempt to add a WHERE clause directly on the FTS5 query:

```rust
    pub async fn search_notes_with_intent(
        &self,
        query: &str,
        limit: i64,
        intent_filter: Option<&str>,
    ) -> Result<Vec<NoteRow>, StorageError> {
        if intent_filter.is_none() {
            return self.search_notes(query, limit).await;
        }
        let intent = intent_filter.unwrap();
        // Use FTS5 with a JOIN to notebooks for intent resolution.
        // The COALESCE resolves: note override > notebook default > 'capture' (orphan).
        let rows = sqlx::query_as::<_, NoteRow>(
            "SELECT n.* FROM notes n
             INNER JOIN notes_fts ON notes_fts.rowid = n.rowid
             LEFT JOIN notebooks nb ON nb.id = n.notebook_id
             WHERE notes_fts MATCH ?1
             AND COALESCE(n.intent_override, nb.intent, 'capture') = ?2
             ORDER BY rank
             LIMIT ?3"
        )
        .bind(query)
        .bind(intent)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
```

- [ ] **Step 5: Add intent-filtered list_notes_by_entity**

In `crates/feature-notes/src/repo/links.rs`, add after existing `list_notes_by_entity`:

```rust
    pub async fn list_notes_by_entity_with_intent(
        &self,
        entity_type: &str,
        entity_id: &str,
        intent_filter: Option<&str>,
    ) -> Result<Vec<NoteRow>, StorageError> {
        // Same as list_notes_by_entity but with optional
        // WHERE COALESCE(n.intent_override, nb.intent, 'capture') = ? filter
    }
```

- [ ] **Step 6: Verify build**

Run: `cargo build -p feature-notes`
Expected: SUCCESS

- [ ] **Step 7: Commit**

```bash
git add crates/feature-notes/src/repo/notes.rs crates/feature-notes/src/repo/notebooks.rs crates/feature-notes/src/repo/links.rs
git commit -m "feat(notes): add intent to CRUD queries and intent-filtered search"
```

---

## Task 5: Desktop-shared types — intent in IPC contract

**Files:**
- Modify: `crates/desktop-shared/src/commands/notes.rs`
- Create: `crates/desktop-shared/src/commands/research.rs`

- [ ] **Step 1: Add intent fields to NoteResponse**

In `crates/desktop-shared/src/commands/notes.rs`, add after `updated_at` field (line 23):

```rust
    pub intent_override: Option<String>,
    pub effective_intent: String,
```

- [ ] **Step 2: Add intent to NotebookResponse**

Add after `note_count` field (line 99):

```rust
    pub intent: String,
```

- [ ] **Step 3: Add intent to NoteCreateParams**

Add after `tags` field (line 32):

```rust
    pub intent_override: Option<String>,
```

- [ ] **Step 4: Add intent_override to NoteUpdateParams**

Add as a new nullable field (same pattern as `icon`):

```rust
    #[serde(default, deserialize_with = "deserialize_nullable_field")]
    pub intent_override: Option<Option<String>>,
```

- [ ] **Step 5: Add intent to NotebookCreateParams**

Locate `NotebookCreateParams` in the same file and add:

```rust
    pub intent: Option<String>,
```

- [ ] **Step 6: Create research response types**

Create `crates/desktop-shared/src/commands/research.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceEntity {
    pub entity_id: String,
    pub entity_type: String,
    pub name: String,
    pub mention_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityRelation {
    pub source_entity_id: String,
    pub target_entity_id: String,
    pub relation_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteReference {
    pub note_id: String,
    pub title: String,
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceMapResponse {
    pub entities: Vec<EvidenceEntity>,
    pub relationships: Vec<EntityRelation>,
    pub source_notes: Vec<NoteReference>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceTrailEntry {
    pub note_id: String,
    pub title: String,
    pub direction: String, // "cites" | "cited_by"
    pub context_snippet: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrossDomainLink {
    pub domain: String,
    pub title: String,
    pub relevance: f64,
    pub connection_type: String,
    pub snippet: Option<String>,
    pub entity_id: Option<String>,
    pub timestamp: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RiskGapItem {
    pub gap_type: String,
    pub subject: String,
    pub suggested_action: Option<String>,
}
```

- [ ] **Step 7: Register research module in desktop-shared**

In `crates/desktop-shared/src/commands/mod.rs`, add both the module declaration AND the pub-use re-export (matching the existing pattern in this file):
```rust
mod research;
// ... (in the pub use block)
pub use research::*;
```

- [ ] **Step 8: Verify build**

Run: `cargo build -p desktop-shared`
Expected: SUCCESS

- [ ] **Step 9: Commit**

```bash
git add crates/desktop-shared/src/commands/notes.rs crates/desktop-shared/src/commands/research.rs crates/desktop-shared/src/commands/mod.rs
git commit -m "feat(notes): add intent fields and research types to IPC contract"
```

---

## Task 6: App-core handlers — intent-gated event publishing

**Files:**
- Modify: `crates/app-core/src/handlers/notes/crud.rs`

- [ ] **Step 1: Write integration test for intent gating**

In the appropriate test file (or inline in `crud.rs`), write a test that:

```rust
#[tokio::test]
async fn research_note_does_not_publish_content_changed() {
    // Setup: in-memory pool, create a notebook with intent="research"
    // Create a note in that notebook
    // Subscribe to DomainEventBus
    // Call note_update with body change
    // Assert: NoteUpdated received, NoteContentChanged NOT received
}

#[tokio::test]
async fn study_note_publishes_content_changed() {
    // Setup: in-memory pool, create a notebook with intent="study"
    // Create a note, update body
    // Assert: both NoteUpdated and NoteContentChanged received
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p app-core -E 'test(intent)'`
Expected: FAIL — test infrastructure may need setup or gating logic not yet present

- [ ] **Step 3: Add notebook intent fetch to note_create**

In `crates/app-core/src/handlers/notes/crud.rs`, in `note_create` (line 146-157), replace the unconditional `NoteContentChanged` publish with:

```rust
    // Resolve effective intent for event gating
    // Note: notebook methods live on NoteRepo (self.note_repo), not a separate notebook_repo.
    let notebook_intent = match created.notebook_id.as_deref() {
        Some(nb_id) => self.note_repo.get_notebook_intent(nb_id).await.ok().flatten(),
        None => None,
    };
    let intent = feature_notes::intent::NoteIntent::effective(
        created.intent_override.as_deref(),
        notebook_intent.as_deref(),
    );

    if let Ok(bus) = self.domain_event_bus() {
        bus.publish(bus::DomainEvent::NoteCreated {
            note_id: id.clone(),
            title: created.title.clone(),
        });
        // NoteUpdated — always (UI cache invalidation)
        bus.publish(bus::DomainEvent::NoteUpdated {
            note_id: id.clone(),
            title: created.title.clone(),
        });
        // NoteContentChanged — study only (triggers atom extraction)
        if intent == feature_notes::intent::NoteIntent::Study && !created.body.is_empty() {
            bus.publish(bus::DomainEvent::NoteContentChanged {
                note_id: id.clone(),
                content: created.body.clone(),
            });
        }
    }
```

- [ ] **Step 4: Add notebook intent fetch to note_update**

Apply the same pattern in `note_update` (lines 213-224): fetch notebook intent, resolve effective intent, gate `NoteContentChanged` on study.

- [ ] **Step 5: Add intent gating to note_version_restore**

In `note_version_restore` (around line 314-373): this handler calls `NoteRepo::update_note` directly and currently publishes **zero domain events**. After the restore call:

1. Fetch notebook intent via `self.note_repo.get_notebook_intent(nb_id)`
2. Publish `NoteUpdated` (always — for UI cache invalidation)
3. Publish `NoteContentChanged` only if `intent == NoteIntent::Study`

```rust
    // After the restore update_note call:
    let notebook_intent = match restored.notebook_id.as_deref() {
        Some(nb_id) => self.note_repo.get_notebook_intent(nb_id).await.ok().flatten(),
        None => None,
    };
    let intent = feature_notes::intent::NoteIntent::effective(
        restored.intent_override.as_deref(),
        notebook_intent.as_deref(),
    );

    if let Ok(bus) = self.domain_event_bus() {
        // NoteUpdated — always (UI cache invalidation, was missing before)
        bus.publish(bus::DomainEvent::NoteUpdated {
            note_id: note_id.to_string(),
            title: restored.title.clone(),
        });
        // NoteContentChanged — study only
        if intent == feature_notes::intent::NoteIntent::Study {
            bus.publish(bus::DomainEvent::NoteContentChanged {
                note_id: note_id.to_string(),
                content: restored.body.clone(),
            });
        }
    }
```

- [ ] **Step 6: Wire intent_override into note_create params**

In `note_create`, populate `intent_override` from `NoteCreateParams` (the new field added in Task 5) into the `NoteRow`.

- [ ] **Step 7: Wire intent_override into note_update params**

In `note_update`, handle the nullable `intent_override` field from `NoteUpdateParams` and pass it to the repo update.

- [ ] **Step 8: Update converters.rs to populate intent fields in responses**

In `crates/app-core/src/handlers/notes/converters.rs`:

1. Update `note_row_to_response` (line 11) to accept `notebook_intent: Option<&str>` as a third parameter:
```rust
pub(crate) fn note_row_to_response(row: &NoteRow, tags: Vec<String>, notebook_intent: Option<&str>) -> NoteResponse {
    let effective = feature_notes::intent::NoteIntent::effective(
        row.intent_override.as_deref(),
        notebook_intent,
    );
    NoteResponse {
        // ... existing fields ...
        intent_override: row.intent_override.clone(),
        effective_intent: effective.as_str().to_string(),
    }
}
```

2. Update `note_with_tags` (line 32) to fetch notebook intent before calling `note_row_to_response`:
```rust
pub(crate) async fn note_with_tags(core: &AppCore, row: &NoteRow) -> Result<NoteResponse, ApiError> {
    let tags = core.note_repo.get_tags(&row.id).await.map_err(map_storage_err)?;
    let nb_intent = match row.notebook_id.as_deref() {
        Some(nb_id) => core.note_repo.get_notebook_intent(nb_id).await.ok().flatten(),
        None => None,
    };
    Ok(note_row_to_response(row, tags, nb_intent.as_deref()))
}
```

3. Update `notes_with_tags_batch` (line 57) similarly — batch-fetch notebook intents for all unique notebook_ids, then pass per-row.

4. Update `notebook_row_to_response` (line 44) to include `intent: row.intent.clone()` in `NotebookResponse`.

- [ ] **Step 9: Run tests**

Run: `cargo nextest run -p app-core -E 'test(intent)'`
Expected: PASS

- [ ] **Step 10: Run full workspace tests**

Run: `cargo nextest run --workspace`
Expected: PASS (no regressions in existing note tests)

- [ ] **Step 11: Commit**

```bash
git add crates/app-core/src/handlers/notes/crud.rs
git commit -m "feat(notes): gate NoteContentChanged on study intent in crud handlers"
```

---

## Task 7: Atom extraction safety net

**Files:**
- Modify: `crates/cognitive/src/services/atom_extraction.rs`

- [ ] **Step 1: Add intent check in event handler**

In `crates/cognitive/src/services/atom_extraction.rs`, inside the `NoteContentChanged` match arm (around line 81), add before the debounce check:

```rust
    // Safety net: verify intent is still Study (handles race conditions
    // where intent changed after NoteContentChanged was queued)
    let intent = resolve_note_intent(&pool, &note_id).await;
    if intent != feature_notes::intent::NoteIntent::Study {
        debug!(note_id, ?intent, "skipping atom extraction — non-study intent");
        continue;
    }
```

- [ ] **Step 2: Add resolve_note_intent helper**

Add at the bottom of the file:

```rust
async fn resolve_note_intent(
    pool: &sqlx::SqlitePool,
    note_id: &str,
) -> feature_notes::intent::NoteIntent {
    let result: Option<(Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT n.intent_override, nb.intent
         FROM notes n
         LEFT JOIN notebooks nb ON nb.id = n.notebook_id
         WHERE n.id = ?1"
    )
    .bind(note_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    match result {
        Some((override_val, nb_intent)) => {
            feature_notes::intent::NoteIntent::effective(
                override_val.as_deref(),
                nb_intent.as_deref(),
            )
        }
        None => feature_notes::intent::NoteIntent::Capture,
    }
}
```

- [ ] **Step 3: Verify build and existing tests**

Run: `cargo nextest run -p cognitive`
Expected: PASS (existing tests unchanged)

- [ ] **Step 4: Commit**

```bash
git add crates/cognitive/src/services/atom_extraction.rs
git commit -m "feat(notes): add intent safety-net check in atom extraction service"
```

---

## Task 8: NotesTool — intent parameter on actions

**Files:**
- Modify: `crates/feature-notes/src/tool.rs`

- [ ] **Step 1: Add intent to parameters() schema**

In `crates/feature-notes/src/tool.rs`, in the `parameters()` JSON (lines 32-70), add to the properties:

```json
"intent": {
    "type": "string",
    "enum": ["study", "research", "capture", "auto"],
    "description": "Note purpose. auto (default) inherits from notebook or infers from calling skill context."
}
```

- [ ] **Step 2: Wire intent into create action**

In the `execute()` match for `"create"` action: read `intent` param, if not `"auto"` and not absent, set it as `intent_override` on the new note.

- [ ] **Step 3: Wire intent filter into search action**

In the `execute()` match for `"search"` action: read `intent` param, pass as filter to `search_notes_with_intent`.

- [ ] **Step 4: Wire intent filter into list_by_entity action**

In the `execute()` match for `"list_by_entity"`: read `intent` param, pass to `list_notes_by_entity_with_intent`.

- [ ] **Step 5: Verify build and run tool tests**

Run: `cargo nextest run -p feature-notes`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/feature-notes/src/tool.rs
git commit -m "feat(notes): add intent parameter to NotesTool create/search/list_by_entity"
```

---

## Task 9: Research tab handlers — graph-driven instant tabs

**Files:**
- Create: `crates/app-core/src/handlers/notes/research.rs`
- Modify: `crates/app-core/src/handlers/notes/mod.rs`

- [ ] **Step 1: Create research.rs with handler stubs**

Create `crates/app-core/src/handlers/notes/research.rs` with four public async methods on `AppCore`:

```rust
use desktop_shared::commands::research::*;

impl AppCore {
    /// Evidence Map: entity graph with relationships and source notes.
    pub async fn research_evidence_map(&self, note_id: &str) -> Result<EvidenceMapResponse, ApiError> {
        // 1. Fetch entity mentions for this note from note_entity_mentions
        // 2. For each entity, query EntityRepo for relationships (one hop)
        // 3. Find source notes that share entities
        // 4. Return structured response
    }

    /// Source Trail: wikilinks + backlinks with context snippets.
    pub async fn research_source_trail(&self, note_id: &str) -> Result<Vec<SourceTrailEntry>, ApiError> {
        // 1. Get outgoing note_links (this note cites)
        // 2. Get backlinks with context (cited by)
        // 3. Return as SourceTrailEntry list
    }

    /// Cross-domain Links: temporal + entity + vector hybrid.
    pub async fn research_cross_domain_links(&self, note_id: &str) -> Result<Vec<CrossDomainLink>, ApiError> {
        // 1. Get entity mentions for this note
        // 2. Temporal window: ±7 days from note created_at
        // 3. Entity overlap: find items sharing entities across domains
        // 4. Vector similarity: embedding search against note embeddings
        // 5. Group by domain (finance, episodic, notes, tasks)
        // 6. Score and sort by relevance
    }

    /// Risks, Gaps & Next Steps: entity gaps + dead links.
    pub async fn research_risks_gaps(&self, note_id: &str) -> Result<Vec<RiskGapItem>, ApiError> {
        // 1. Find entities mentioned but not linked to any notes
        // 2. Find dead wikilinks (link targets that don't exist)
        // 3. Find sparse connections (entities with < 2 relationships)
        // 4. Generate suggested actions
    }
}
```

- [ ] **Step 2: Register in mod.rs**

In `crates/app-core/src/handlers/notes/mod.rs`, add `pub mod research;`.

- [ ] **Step 3: Implement evidence_map**

Implement using `NoteRepo` for entity mentions and `EntityRepo` (via cognitive accessor trait or direct repo) for relationship traversal.

- [ ] **Step 4: Implement source_trail**

Implement using `NoteRepo::get_backlinks_with_context()` and `note_links` queries.

- [ ] **Step 5: Implement cross_domain_links**

This is the most complex handler. Implement the three-layer query:
1. Entity overlap via `note_entity_mentions` JOIN across entity types
2. Temporal window via date range on `created_at`
3. Vector similarity via `VectorStore` search with note embedding

Combine results with weighted scoring and group by domain.

- [ ] **Step 6: Implement risks_gaps**

Query for entities with no linked notes, dead wikilinks (targets not in notes table), and sparse connections.

- [ ] **Step 7: Write integration tests**

Test each handler with in-memory DB: create notes with entities and links, verify correct output shapes.

- [ ] **Step 8: Verify build and tests**

Run: `cargo nextest run -p app-core -E 'test(research)'`
Expected: PASS

- [ ] **Step 9: Commit**

```bash
git add crates/app-core/src/handlers/notes/research.rs crates/app-core/src/handlers/notes/mod.rs
git commit -m "feat(notes): add graph-driven research tab handlers"
```

---

## Task 10: Research prompt templates + insight routing

**Files:**
- Modify: `crates/app-core/src/handlers/notes/insight_prompts.rs`
- Modify: `crates/app-core/src/handlers/notes/insight.rs`

- [ ] **Step 1: Add research prompt templates**

In `crates/app-core/src/handlers/notes/insight_prompts.rs`, add three new public functions:

```rust
pub fn research_executive_summary_prompt() -> &'static str {
    "Synthesize the key findings, conclusions, and actionable insights from this research. \
     Focus on what decisions this research supports, not what the reader should study. \
     Structure as: Key Findings, Implications, Recommended Actions."
}

pub fn research_hypothesis_tracker_prompt() -> &'static str {
    "Extract the explicit and implicit claims/hypotheses in this research. \
     For each, assess confidence level (high/medium/low/speculative) based on \
     supporting evidence found in the content and related context. \
     Return as a JSON array: [{\"claim\": \"...\", \"confidence\": \"high|medium|low|speculative\", \"evidence\": \"...\", \"counter_evidence\": \"...\"}]"
}

pub fn research_counter_arguments_prompt() -> &'static str {
    "Identify the strongest counter-arguments, alternative explanations, or risks \
     that challenge the conclusions in this research. Draw on the provided context \
     for contradicting evidence. Structure as numbered items with supporting reasoning."
}
```

- [ ] **Step 2: Add intent check at top of existing note_insight_review**

The existing `note_insight_review` has signature: `(&self, note_id, scope_params, squad_id, emitter_override)` — 4 params. Do NOT change this signature. Instead, add an intent check at the top (after the existing note fetch at line 50-55):

```rust
    // After fetching the note (existing code):
    let nb_intent = match note.notebook_id.as_deref() {
        Some(nb_id) => self.note_repo.get_notebook_intent(nb_id).await.ok().flatten(),
        None => None,
    };
    let intent = feature_notes::intent::NoteIntent::effective(
        note.intent_override.as_deref(),
        nb_intent.as_deref(),
    );

    if intent == feature_notes::intent::NoteIntent::Capture {
        return Err(ApiError::new("INVALID_OPERATION", "Capture notes have no insight panel"));
    }

    // For study notes: continue with existing logic (unchanged)
    // For research notes: this handler is NOT used — research LLM tabs
    // go through the new research_deepen_tab handler (see Task 9).
    if intent == feature_notes::intent::NoteIntent::Research {
        return Err(ApiError::new("INVALID_OPERATION", "Use research_deepen_tab for research notes"));
    }

    // ... rest of existing study insight logic unchanged ...
```

- [ ] **Step 3: Add research_deepen_tab handler on AppCore**

Create a NEW handler (not modifying the existing `note_insight_review` signature) in `crates/app-core/src/handlers/notes/research.rs`:

```rust
use feature_notes::intent::NoteIntent;

impl AppCore {
    /// Handle LLM-driven research tab generation (Executive Summary, Hypothesis Tracker, Counter-arguments).
    pub async fn research_deepen_tab(
        &self,
        note_id: &str,
        tab_id: &str,
        emitter_override: Option<Arc<dyn crate::events::AppEventEmitter>>,
    ) -> Result<InsightReviewStarted, ApiError> {
        let note = self.note_repo.get_note(note_id).await
            .map_err(map_storage_err)?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "Note not found"))?;

        // Select prompt based on tab_id
        let system_prompt = match tab_id {
            "executive-summary" => insight_prompts::research_executive_summary_prompt(),
            "hypothesis" => insight_prompts::research_hypothesis_tracker_prompt(),
            "counter-arguments" => insight_prompts::research_counter_arguments_prompt(),
            _ => return Err(ApiError::new("VALIDATION", "Unknown research tab")),
        };

        // Reuse InsightService streaming infrastructure with research prompt
        // ... (uses same PromptBuilder context assembly + provider.chat_stream pattern)
    }
}
```

- [ ] **Step 4: Add resolve_effective_intent helper on AppCore**

In `crates/app-core/src/handlers/notes/research.rs` or a shared helper:

```rust
impl AppCore {
    pub(crate) async fn resolve_effective_intent(&self, note_id: &str) -> Result<NoteIntent, ApiError> {
        let note = self.note_repo.get_note(note_id).await.map_err(map_storage_err)?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "Note not found"))?;
        let nb_intent = match note.notebook_id.as_deref() {
            Some(nb_id) => self.note_repo.get_notebook_intent(nb_id).await.ok().flatten(),
            None => None,
        };
        Ok(NoteIntent::effective(note.intent_override.as_deref(), nb_intent.as_deref()))
    }
}
```

- [ ] **Step 4: Verify build**

Run: `cargo build -p app-core`
Expected: SUCCESS

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/handlers/notes/insight_prompts.rs crates/app-core/src/handlers/notes/insight.rs
git commit -m "feat(notes): add research prompts and intent-routed insight handler"
```

---

## Task 11: Tauri commands + DEV_COMMANDS registration

**Files:**
- Modify: `crates/desktop/src/commands/notes.rs`
- Modify: `crates/desktop/src/dev_server/mod.rs`

- [ ] **Step 1: Add research IPC commands**

In `crates/desktop/src/commands/notes.rs`, add 5 new Tauri commands:

```rust
#[tauri::command]
pub async fn research_evidence_map(state: State<'_, AppState>, note_id: String) -> CmdResult<EvidenceMapResponse> {
    Ok(state.app_core.research_evidence_map(&note_id).await?)
}

#[tauri::command]
pub async fn research_source_trail(state: State<'_, AppState>, note_id: String) -> CmdResult<Vec<SourceTrailEntry>> {
    Ok(state.app_core.research_source_trail(&note_id).await?)
}

#[tauri::command]
pub async fn research_cross_domain_links(state: State<'_, AppState>, note_id: String) -> CmdResult<Vec<CrossDomainLink>> {
    Ok(state.app_core.research_cross_domain_links(&note_id).await?)
}

#[tauri::command]
pub async fn research_risks_gaps(state: State<'_, AppState>, note_id: String) -> CmdResult<Vec<RiskGapItem>> {
    Ok(state.app_core.research_risks_gaps(&note_id).await?)
}

#[tauri::command]
pub async fn research_deepen_tab(state: State<'_, AppState>, note_id: String, tab_id: String) -> CmdResult<InsightReviewStarted> {
    Ok(state.app_core.research_deepen_tab(&note_id, &tab_id, None).await?)
}
```

- [ ] **Step 2: Add to DEV_COMMANDS**

Append the 5 new command name strings to the existing `pub const DEV_COMMANDS: &[&str]` array in `crates/desktop/src/commands/notes.rs`:
```rust
    "research_evidence_map",
    "research_source_trail",
    "research_cross_domain_links",
    "research_risks_gaps",
    "research_deepen_tab",
```

- [ ] **Step 3: Register commands in Tauri builder**

In `crates/desktop/src/lib.rs` (the Tauri builder file), add the 5 new commands to the `invoke_handler!()` macro call alongside the existing note commands.

- [ ] **Step 4: Add dev server routes**

In `crates/desktop/src/dev_server/mod.rs`, add HTTP route handlers that delegate to the same AppCore methods.

- [ ] **Step 5: Run parity test**

Run: `cargo nextest run -p desktop -E 'test(dev_server_covers)'`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/desktop/src/commands/notes.rs crates/desktop/src/dev_server/mod.rs
git commit -m "feat(notes): add research IPC commands with DEV_COMMANDS registration"
```

---

## Task 12: Frontend — InsightReviewPanel intent branching

**Files:**
- Modify: `desktop-ui/src/features/notes/components/InsightReviewPanel.tsx`
- Create: `desktop-ui/src/features/notes/hooks/useResearchTabs.ts`

- [ ] **Step 1: Create useResearchTabs hook**

Create `desktop-ui/src/features/notes/hooks/useResearchTabs.ts`:

```typescript
import { useQuery } from "@shared/hooks/useQuery";

export function useResearchTabs(noteId: string | undefined) {
    const evidenceMap = useQuery("research_evidence_map", noteId ? { noteId } : undefined);
    const sourceTrail = useQuery("research_source_trail", noteId ? { noteId } : undefined);
    const crossDomainLinks = useQuery("research_cross_domain_links", noteId ? { noteId } : undefined);
    const risksGaps = useQuery("research_risks_gaps", noteId ? { noteId } : undefined);

    return { evidenceMap, sourceTrail, crossDomainLinks, risksGaps };
}
```

- [ ] **Step 2: Define tab manifests**

In `InsightReviewPanel.tsx`, add research tab manifest alongside the existing study tabs:

```typescript
const RESEARCH_TABS = [
    { id: "cross-domain", label: "Cross-domain Links", icon: Link2, source: "graph" as const },
    { id: "evidence", label: "Evidence Map", icon: Map, source: "graph" as const },
    { id: "sources", label: "Source Trail", icon: FileSearch, source: "graph" as const },
    { id: "risks-gaps", label: "Risks & Next Steps", icon: AlertTriangle, source: "graph" as const },
    { id: "executive-summary", label: "Executive Summary", icon: FileText, source: "llm" as const },
    { id: "hypothesis", label: "Hypothesis Tracker", icon: FlaskConical, source: "llm" as const },
    { id: "counter-arguments", label: "Counter-arguments", icon: Scale, source: "llm" as const },
] as const;
```

- [ ] **Step 3: Branch on effective intent**

At the top of `InsightReviewPanel`, use the server-resolved `effectiveIntent`:

```typescript
const { effectiveIntent } = note;

if (effectiveIntent === "capture") return null;

const tabs = effectiveIntent === "research" ? RESEARCH_TABS : STUDY_TABS;
```

- [ ] **Step 4: Render graph tabs with instant data**

For research graph tabs, render the data from `useResearchTabs` directly — no loading state for the initial fetch (use SWR stale-while-revalidate).

- [ ] **Step 5: Render LLM tabs with "Deepen with AI" placeholder**

For research LLM tabs, show a description + "Deepen with AI" button. On click, call `research_deepen_tab` IPC and stream results using the existing `useInsightSSE` pattern.

- [ ] **Step 6: Add "Generate Full Analysis" button**

At the top of the research panel, add a button that triggers all 3 LLM tabs simultaneously.

- [ ] **Step 7: Verify with dev server**

Run: `cd desktop-ui && bun run dev` + `cargo tauri dev`
Navigate to a research note, verify panel shows research tabs.

- [ ] **Step 8: Commit**

```bash
cd desktop-ui && git add src/features/notes/
git commit -m "feat(notes): add research tab manifest and intent branching to InsightReviewPanel"
```

---

## Task 13: Frontend — Purpose badge + editor toolbar conditioning

**Files:**
- Create: `desktop-ui/src/features/notes/components/PurposeBadge.tsx`
- Modify: `desktop-ui/src/features/notes/components/editor/EditorToolbar.tsx`
- Modify: `desktop-ui/src/features/notes/pages/KnowledgeBasePage.tsx`

- [ ] **Step 1: Create PurposeBadge component**

Create `desktop-ui/src/features/notes/components/PurposeBadge.tsx`:

```tsx
// Subtle pill showing note purpose with icon
// Click opens glass-panel popover with 3 options
// Props: effectiveIntent, onIntentChange, noteId
// Icons: Book (study), BarChart3 (research), BookOpen (capture)
// Colors: indigo (study), amber (research), slate (capture)
// Keyboard shortcut: Cmd+Shift+P
```

- [ ] **Step 2: Hide "Generate Cards" for non-study**

In `desktop-ui/src/features/notes/components/editor/EditorToolbar.tsx` (around line 278-292), wrap the flashcard button in:

```tsx
{effectiveIntent === "study" && (
    <button onClick={onGenerateCards} title="Generate flashcards">
        <Sparkles size={16} />
    </button>
)}
```

Pass `effectiveIntent` as a prop from `NoteEditor` → `EditorToolbar`.

- [ ] **Step 3: Wire PurposeBadge into KnowledgeBasePage**

In the editor header area of `KnowledgeBasePage.tsx`, render `PurposeBadge` next to the note title. Wire the `onIntentChange` callback to call `note_update` with the new `intent_override`.

- [ ] **Step 4: Add intent change prompt for Study switch**

When switching to Study, show a dialog: "Switching to Study mode. Want to run a full analysis now?" with [Yes] / [Later]. "Yes" triggers `note_insight_review`.

- [ ] **Step 5: Verify visual appearance**

Run dev server, check all three intent badges render correctly with colors and icons. Verify Generate Cards button hides for research/capture.

- [ ] **Step 6: Run lint**

Run: `cd desktop-ui && bun run lint:fix`
Expected: no errors

- [ ] **Step 7: Commit**

```bash
cd desktop-ui && git add src/features/notes/
git commit -m "feat(notes): add PurposeBadge and condition editor toolbar on intent"
```

---

## Task 14: Frontend — Notebook creation purpose selection

**Files:**
- Modify: `desktop-ui/src/features/notes/pages/KnowledgeBasePage.tsx` (notebook creation lives here)

- [ ] **Step 1: Add purpose selection step**

Add a 3-option selector (Study/Research/Capture) with icons and descriptions to the notebook creation flow. Default: Study. Pass selected intent in `NotebookCreateParams`.

- [ ] **Step 3: Add title-based inference**

If notebook title contains "analysis", "review", "Q2", "project", "research" (case-insensitive), pre-select Research and show "Recommended for analysis" badge.

- [ ] **Step 4: Add purpose icon to sidebar**

In the notebook list sidebar, show the purpose icon next to each notebook title.

- [ ] **Step 5: Run lint and verify**

Run: `cd desktop-ui && bun run lint:fix`
Verify notebook creation flow in dev server.

- [ ] **Step 6: Commit**

```bash
cd desktop-ui && git add src/
git commit -m "feat(notes): add purpose selection to notebook creation with title inference"
```

---

## Task 15: Full integration verification

**Files:** None (verification only)

- [ ] **Step 1: Run full Rust test suite**

Run: `cargo nextest run --workspace`
Expected: all tests PASS

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

- [ ] **Step 3: Run format check**

Run: `cargo fmt --all --check`
Expected: no formatting issues

- [ ] **Step 4: Run frontend tests**

Run: `cd desktop-ui && bun run test`
Expected: all tests PASS

- [ ] **Step 5: Run frontend lint**

Run: `cd desktop-ui && bun run lint`
Expected: no errors

- [ ] **Step 6: Manual smoke test**

1. Create a Research notebook → verify purpose badge shows amber "Research"
2. Create a note in it → verify no atom extraction fires (check logs)
3. Open insight panel → verify 7 research tabs (4 instant + 3 Deepen)
4. Click Cross-domain Links tab → verify instant results
5. Create a Study notebook → verify atoms + study tabs work as before
6. Create a Capture notebook → verify no insight panel
7. Override a research note to Study → verify "Analyze now?" prompt
8. Use agent tool: `notes.search(intent: "research")` → verify filtered results

- [ ] **Step 7: Final commit if any fixes needed**

```bash
git add -A && git commit -m "fix(notes): address integration test findings"
```
