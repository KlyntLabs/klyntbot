# Atoms as Silent Cognitive Substrate — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transform knowledge atoms from a user-facing feature with accept/dismiss UI into a fully silent, internal metric layer — no user interaction, no atom UI, background-only generation with smarter triggers.

**Architecture:** Three subsystems change: (1) Atom extraction triggers shift from 5s-debounce-on-every-edit to a new `NoteEditingFinished` event (fired on blur/close/idle) + daily cron catch-all. (2) Atom lifecycle simplifies — atoms are created directly as `active` (no `suggested` staging), eliminating the accept/dismiss workflow entirely. (3) All user-facing atom UI is removed: Tauri commands, desktop-shared types, frontend components, and the Atoms tab in Insight Review.

**Tech Stack:** Rust (app-core, cognitive, bus, desktop, desktop-shared, config crates), TypeScript/React (desktop-ui), SQLite, FSRS-5

---

## File Map

### Backend — New/Modified

| Action | File | Responsibility |
|--------|------|----------------|
| Modify | `crates/bus/src/domain_events.rs` | Add `NoteEditingFinished` event variant |
| Modify | `crates/cognitive/src/services/atom_extraction.rs` | Replace `NoteContentChanged` trigger with `NoteEditingFinished`; expand atom types; create atoms as `active` |
| Modify | `crates/config/src/schema/cognitive.rs` | Add `idle_timeout_secs` to `AtomExtractionConfig` |
| Modify | `crates/cognitive/src/repos/knowledge_atom.rs` | Remove `accept()`, `restore()` methods; `find_by_subject_across_notes` now matches all non-archived (not just `active`) |
| Modify | `crates/app-core/src/handlers/notes/crud.rs` | Add `note_editing_finished` handler that fires `NoteEditingFinished` |
| Modify | `crates/app-core/src/handlers/notes/insight.rs` | `create_atoms_from_gaps` creates atoms as `active` instead of `suggested` |
| Modify | `crates/app-core/src/adapters/cognitive_accessor.rs` | Remove `status == "active"` post-filter (all non-archived are now active) |
| Modify | `crates/cognitive/src/services/atom_decay.rs` | No status filter needed (all atoms are `active` or `archived`) |

### Backend — Remove User-Facing Atom API

| Action | File | Responsibility |
|--------|------|----------------|
| Gut | `crates/app-core/src/handlers/atoms.rs` | Remove `atom_accept`, `atom_dismiss`, `atom_restore`, `atoms_bulk_accept`, `atoms_migration_status`. Keep `atoms_for_note` (internal-only), `atom_next_card`, `enrich_atom` as pub(crate) |
| Gut | `crates/desktop/src/commands/atoms.rs` | Remove all 7 Tauri commands, `DEV_COMMANDS`, and `dispatch_dev` |
| Gut | `crates/desktop-shared/src/commands/atoms.rs` | Remove all param/response types except `KnowledgeAtomResponse` (still used internally by knowledge_health) |
| Modify | `crates/desktop/src/main.rs:485-492` | Remove all `commands::atoms::*` from Tauri handler list |
| Modify | `crates/desktop/src/commands/mod.rs` | Remove `pub mod atoms` |
| Modify | `crates/desktop-shared/src/commands/mod.rs` | Remove `mod atoms` re-export (or keep minimal if KnowledgeAtomResponse used elsewhere) |
| Modify | `crates/app-core/src/handlers/mod.rs` | Remove `pub mod atoms` |
| Modify | `crates/app-core/src/init/mod.rs` | Remove `mod atoms` (vocab migration) |
| Delete | `crates/app-core/src/init/atoms.rs` | One-time migration no longer needed |

### Frontend — Remove Atom UI

| Action | File | Responsibility |
|--------|------|----------------|
| Delete | `desktop-ui/src/features/notes/components/KnowledgeAtomsPanel.tsx` | Atom list panel |
| Delete | `desktop-ui/src/features/notes/components/AtomCard.tsx` | Per-atom row with accept/dismiss |
| Delete | `desktop-ui/src/features/notes/components/BulkAcceptModal.tsx` | Bulk accept modal |
| Delete | `desktop-ui/src/features/notes/components/InlineReview.tsx` | Inline flashcard review from atom |
| Delete | `desktop-ui/src/features/notes/components/WhyThisPopover.tsx` | "Why this atom?" popover |
| Delete | `desktop-ui/src/features/notes/components/insight/AtomsTab.tsx` | Atoms tab wrapper |
| Delete | `desktop-ui/src/features/notes/hooks/useKnowledgeAtoms.ts` | All atom hooks |
| Modify | `desktop-ui/src/features/notes/components/InsightReviewPanel.tsx` | Remove Atoms tab from tab list and render switch |
| Modify | `desktop-ui/src/features/notes/components/insight/KnowledgeGrowthMetrics.tsx` | Remove atom counts UI (or delete entirely) |
| Modify | `desktop-ui/src/features/notes/hooks/useVocabularySave.ts` | Remove `invalidateQueries("atoms_for_note")` |
| Modify | `desktop-ui/src/features/notes/pages/KnowledgeBasePage.tsx` | Remove `atomId` URL param handling |

### Frontend — Add Blur/Idle Trigger

| Action | File | Responsibility |
|--------|------|----------------|
| Modify | Note editor component (the component that wraps the editor) | Fire `note_editing_finished` IPC on blur/unmount |
| Create | `crates/desktop/src/commands/notes.rs` (or add to existing) | `note_editing_finished` Tauri command |

---

## Task Breakdown

### Task 1: Add `NoteEditingFinished` Domain Event

**Files:**
- Modify: `crates/bus/src/domain_events.rs:216-219`

- [ ] **Step 1: Write the test**

```rust
// In crates/bus/src/domain_events.rs, in the existing #[cfg(test)] mod tests block:
#[test]
fn test_note_editing_finished_event() {
    let bus = DomainEventBus::new(32);
    let mut rx = bus.subscribe();
    bus.publish(DomainEvent::NoteEditingFinished {
        note_id: "note-1".to_string(),
        content: "some content".to_string(),
    });
    let event = rx.try_recv().unwrap();
    assert!(matches!(event, DomainEvent::NoteEditingFinished { note_id, .. } if note_id == "note-1"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p bus -E 'test(note_editing_finished)'`
Expected: FAIL — `NoteEditingFinished` variant doesn't exist

- [ ] **Step 3: Add the event variant**

In `crates/bus/src/domain_events.rs`, after the `NoteContentChanged` variant (line ~219), add:

```rust
    NoteEditingFinished {
        note_id: String,
        content: String,
    },
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo nextest run -p bus -E 'test(note_editing_finished)'`
Expected: PASS

- [ ] **Step 5: Run full bus crate tests**

Run: `cargo nextest run -p bus`
Expected: All pass

- [ ] **Step 6: Commit**

```bash
git add crates/bus/src/domain_events.rs
git commit -m "feat(bus): add NoteEditingFinished domain event"
```

---

### Task 2: Add `note_editing_finished` Handler + Tauri Command

This handler receives the blur/close/idle signal from the frontend and publishes `NoteEditingFinished`.

**Files:**
- Modify: `crates/app-core/src/handlers/notes/crud.rs`
- Modify: `crates/desktop/src/commands/notes.rs`
- Modify: `crates/desktop-shared/src/commands/notes.rs` (add params type)

- [ ] **Step 1: Add the params type in desktop-shared**

In `crates/desktop-shared/src/commands/notes.rs`, add:

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteEditingFinishedParams {
    pub note_id: String,
}
```

Also add `NoteEditingFinishedParams` to the module's re-exports if using a `pub use` block.

- [ ] **Step 2: Add the handler in app-core**

In `crates/app-core/src/handlers/notes/crud.rs`, add a new public method on `AppCore`:

```rust
    /// Signal that the user has finished editing a note (blur/close/idle).
    /// Fires NoteEditingFinished so background services can process the final content.
    pub async fn note_editing_finished(
        &self,
        params: NoteEditingFinishedParams,
    ) -> Result<(), ApiError> {
        let note = self
            .note_repo
            .get_note(&params.note_id)
            .await
            .map_err(map_storage_err)?;

        if let Some(note) = note {
            if let Ok(bus) = self.domain_event_bus() {
                if !note.body.is_empty() {
                    bus.publish(bus::DomainEvent::NoteEditingFinished {
                        note_id: params.note_id,
                        content: note.body,
                    });
                }
            }
        }

        Ok(())
    }
```

- [ ] **Step 3: Add the Tauri command**

In `crates/desktop/src/commands/notes.rs`, add:

```rust
#[tauri::command]
pub async fn note_editing_finished(
    state: State<'_, Arc<AppCore>>,
    params: NoteEditingFinishedParams,
) -> Result<(), ApiError> {
    state.note_editing_finished(params).await
}
```

Add `NoteEditingFinishedParams` to the imports from `desktop_shared::commands`.

- [ ] **Step 4: Register the command in main.rs**

In `crates/desktop/src/main.rs`, add `commands::notes::note_editing_finished` to the Tauri handler list (near the other notes commands).

- [ ] **Step 5: Add to DEV_COMMANDS and dispatch_dev in the notes commands module**

Add `"note_editing_finished"` to the `DEV_COMMANDS` array in `crates/desktop/src/commands/notes.rs` and add a dispatch arm in `dispatch_dev`.

- [ ] **Step 6: Build and verify**

Run: `cargo build --workspace`
Expected: Clean compile

- [ ] **Step 7: Commit**

```bash
git add crates/app-core/src/handlers/notes/crud.rs crates/desktop/src/commands/notes.rs crates/desktop-shared/src/commands/notes.rs crates/desktop/src/main.rs
git commit -m "feat(notes): add note_editing_finished handler and Tauri command"
```

---

### Task 3: Rewire Atom Extraction to `NoteEditingFinished`

Replace the `NoteContentChanged` trigger with `NoteEditingFinished`. Expand valid atom types. Create atoms directly as `active`.

**Files:**
- Modify: `crates/cognitive/src/services/atom_extraction.rs`
- Modify: `crates/config/src/schema/cognitive.rs`

- [ ] **Step 1: Update valid atom types and default status**

In `crates/cognitive/src/services/atom_extraction.rs`:

Change the LLM system prompt (line ~377-383) to include the expanded types:

```rust
    let system_prompt = "You are a knowledge extraction assistant. Analyze this text and identify 3-8 key knowledge atoms worth tracking.\n\n\
        For each item, return:\n\
        - \"subject\": short label (2-5 words)\n\
        - \"atomType\": one of \"concept\", \"fact\", \"procedure\", \"reference\", \"pattern\", \"insight\", \"relation\"\n\
        - \"domain\": category (e.g. \"software-engineering\", \"finance\", \"language:ja\")\n\
        - \"sourceContext\": the relevant sentence or phrase from the text (verbatim)\n\n\
        Return JSON array. Include genuinely learnable concepts, notable facts, procedures, patterns, and insights — skip obvious/trivial content.";
```

Change the `valid_types` slice (line ~175):

```rust
        let valid_types = ["concept", "fact", "procedure", "reference", "pattern", "insight", "relation"];
```

Change the status from `"suggested"` to `"active"` at line ~263:

```rust
            let new_atom = NewKnowledgeAtom {
                subject: atom.subject.clone(),
                atom_type: atom.atom_type.clone(),
                domain: atom.domain.clone(),
                source_note_id: Some(note_id.to_string()),
                source_context: atom.source_context.clone(),
                personal_importance: SUGGESTED_IMPORTANCE,
                status: "active".to_string(),
                ..Default::default()
            };
```

- [ ] **Step 2: Switch event subscription from `NoteContentChanged` to `NoteEditingFinished`**

In the event loop (line ~81), change:

```rust
                            Ok(DomainEvent::NoteEditingFinished { note_id, content }) => {
```

- [ ] **Step 3: Update `find_by_subject_across_notes` to match all non-archived**

In `crates/cognitive/src/repos/knowledge_atom.rs`, at line ~480, change:

```rust
              AND status != 'archived'
```

(was: `AND status = 'active'` — but since all atoms are now `active`, this is for safety/future-proofing)

- [ ] **Step 4: Update the DB CHECK constraint for new atom types**

In `crates/cognitive/migrations/001_cognitive_tables.sql`, update the `atom_type` CHECK to include all types:

```sql
    CHECK (atom_type IN ('vocabulary', 'concept', 'skill', 'fact', 'flashcard_weak_spot', 'socratic_exchange', 'translation_unit', 'procedure', 'reference', 'pattern', 'insight', 'relation'))
```

Note: Per CLAUDE.md, pre-release migrations are updated in-place.

- [ ] **Step 5: Update the doc comment on the module**

Change the module doc at line 1-6 to reflect the new behavior:

```rust
//! Background service that extracts knowledge atoms from note content.
//!
//! Subscribes to [`DomainEventBus`] for `NoteEditingFinished` events (fired when
//! the user blurs, closes, or idles on a note). Debounces rapid events, deduplicates
//! against existing atoms, and creates new `active` atoms via an LLM extraction prompt.
//! Cross-note reinforcement is detected and boosted rather than duplicated.
```

- [ ] **Step 6: Run existing extraction tests**

Run: `cargo nextest run -p cognitive -E 'test(atom_extraction)'`
Expected: All pass (tests are unit tests on split/parse helpers, not integration)

- [ ] **Step 7: Run clippy on cognitive crate**

Run: `cargo clippy -p cognitive --all-targets --all-features`
Expected: 0 warnings

- [ ] **Step 8: Commit**

```bash
git add crates/cognitive/src/services/atom_extraction.rs crates/cognitive/src/repos/knowledge_atom.rs crates/cognitive/migrations/001_cognitive_tables.sql crates/config/src/schema/cognitive.rs
git commit -m "feat(cognitive): rewire atom extraction to NoteEditingFinished, expand types, create as active"
```

---

### Task 4: Simplify Atom Lifecycle — Remove `suggested` State

All atoms are now born `active`. Remove accept/dismiss/restore from the repo. Remove the `suggested`-specific logic.

**Files:**
- Modify: `crates/cognitive/src/repos/knowledge_atom.rs`
- Modify: `crates/app-core/src/adapters/cognitive_accessor.rs:156-168`
- Modify: `crates/app-core/src/handlers/notes/insight.rs` (create_atoms_from_gaps)
- Modify: `crates/cognitive/src/services/atom_decay.rs`

- [ ] **Step 1: Remove `accept()` and `restore()` from KnowledgeAtomRepo**

In `crates/cognitive/src/repos/knowledge_atom.rs`:

Delete the `accept()` method (lines ~190-216) and the `restore()` method (lines ~240-279). Keep `dismiss()` — it's used by the decay service for auto-archival.

- [ ] **Step 2: Simplify `list_for_note` ordering**

The `CASE status WHEN 'active' THEN 0 ELSE 1 END` ordering is no longer needed since all atoms are active. Simplify to:

```rust
    pub async fn list_for_note(&self, note_id: &str) -> Result<Vec<KnowledgeAtomRow>, sqlx::Error> {
        sqlx::query_as::<_, KnowledgeAtomRow>(
            r#"
            SELECT * FROM knowledge_atoms
            WHERE source_note_id = ?1
              AND status != 'archived'
            ORDER BY salience DESC
            "#,
        )
        .bind(note_id)
        .fetch_all(&self.pool)
        .await
    }
```

- [ ] **Step 3: Remove status post-filter in cognitive_accessor**

In `crates/app-core/src/adapters/cognitive_accessor.rs:156-168`, remove the `.filter(|a| a.status == "active")` line. All non-archived atoms are now active.

- [ ] **Step 4: Change `create_atoms_from_gaps` to create as `active`**

In `crates/app-core/src/handlers/notes/insight.rs`, find `create_atoms_from_gaps` (around line 1344). Change:

```rust
                status: "active".to_string(),
```

(was: `"suggested"`)

- [ ] **Step 5: Verify atom_decay doesn't need changes**

The decay service uses `list_stale_active` which queries `WHERE status = 'active'`. This is now correct since all atoms are `active`. No change needed.

- [ ] **Step 6: Remove the `test_find_by_subject_across_notes` test dependency on accept**

In `crates/cognitive/src/repos/knowledge_atom.rs`, around line 801, the test creates an atom with `status: "active"` which is already correct. Verify no test calls `repo.accept()` or `repo.restore()`.

Run: `cargo nextest run -p cognitive -E 'test(knowledge_atom)'`
Expected: All pass (or if tests reference `accept`/`restore`, they'll fail and need updating — fix those by removing them or changing to test `dismiss()` only)

- [ ] **Step 7: Run workspace clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings (may show unused import warnings from removed methods — fix those)

- [ ] **Step 8: Commit**

```bash
git add crates/cognitive/src/repos/knowledge_atom.rs crates/app-core/src/adapters/cognitive_accessor.rs crates/app-core/src/handlers/notes/insight.rs
git commit -m "refactor(cognitive): simplify atom lifecycle — all atoms created as active, remove accept/restore"
```

---

### Task 5: Remove User-Facing Atom API — Backend

Remove all Tauri commands, desktop-shared types, and app-core handlers that expose atoms to the UI.

**Files:**
- Gut: `crates/app-core/src/handlers/atoms.rs`
- Delete content: `crates/desktop/src/commands/atoms.rs`
- Gut: `crates/desktop-shared/src/commands/atoms.rs`
- Modify: `crates/desktop/src/main.rs:485-492`
- Modify: `crates/desktop/src/commands/mod.rs`
- Modify: `crates/app-core/src/handlers/mod.rs`
- Modify: `crates/desktop-shared/src/commands/mod.rs`
- Modify: `crates/app-core/src/init/mod.rs`
- Delete: `crates/app-core/src/init/atoms.rs`

- [ ] **Step 1: Remove Tauri command registrations from main.rs**

In `crates/desktop/src/main.rs`, delete lines 485-492 (the `// Knowledge Atoms` section):

```rust
            // Knowledge Atoms
            commands::atoms::atoms_for_note,
            commands::atoms::atom_accept,
            commands::atoms::atom_dismiss,
            commands::atoms::atom_restore,
            commands::atoms::atom_next_card,
            commands::atoms::atoms_bulk_accept,
            commands::atoms::atoms_migration_status,
```

- [ ] **Step 2: Remove `pub mod atoms` from desktop commands mod.rs**

In `crates/desktop/src/commands/mod.rs`, remove the line `pub mod atoms;`

- [ ] **Step 3: Delete the desktop atoms commands file**

Delete `crates/desktop/src/commands/atoms.rs` entirely.

- [ ] **Step 4: Gut the app-core atoms handler**

Replace `crates/app-core/src/handlers/atoms.rs` with only the internal helpers needed by other handlers:

```rust
use desktop_shared::errors::ApiError;

use crate::state::AppCore;

pub(crate) fn map_db(e: sqlx::Error) -> ApiError {
    ApiError::new("INTERNAL_ERROR", e.to_string())
}

impl AppCore {
    /// Get the next due flashcard for a given atom (used internally by learning features).
    pub(crate) async fn atom_next_card_internal(
        &self,
        atom_id: &str,
    ) -> Result<Option<cognitive::FlashcardRow>, ApiError> {
        let repo = self.flashcard_repo()?;
        repo.next_for_atom(atom_id).await.map_err(map_db)
    }
}
```

Note: Check if `atom_next_card` is used anywhere other than the Tauri command. If not, this can be removed entirely. If `enrich_atom` is used by knowledge_health or other internal callers, keep it as `pub(crate)`.

- [ ] **Step 5: Remove `pub mod atoms` from app-core handlers mod.rs**

In `crates/app-core/src/handlers/mod.rs`, remove `pub mod atoms;`

- [ ] **Step 6: Gut desktop-shared atoms types**

In `crates/desktop-shared/src/commands/atoms.rs`, remove all param types (`AtomsForNoteParams`, `AtomAcceptParams`, `AtomDismissParams`, `AtomRestoreParams`, `AtomNextCardParams`, `AtomBulkAcceptParams`, `AtomMigrationStatusParams`) and `AtomMigrationStatusResponse`. Keep `KnowledgeAtomResponse` only if it's used by other command modules (e.g. knowledge_health). If not, remove the file entirely.

- [ ] **Step 7: Update desktop-shared mod.rs**

In `crates/desktop-shared/src/commands/mod.rs`, remove `mod atoms;` (or update the re-export to be minimal).

- [ ] **Step 8: Remove vocab-to-atoms migration**

In `crates/app-core/src/init/mod.rs`, remove `mod atoms;` and any call to `atoms::migrate_vocab_to_atoms`.

Delete `crates/app-core/src/init/atoms.rs`.

- [ ] **Step 9: Remove `KnowledgeAtomAccepted` domain event usages**

Search for and remove any references to `DomainEvent::KnowledgeAtomAccepted` in event handlers, signal conversion, and streaming code. The event variant can stay in the enum for now (removing enum variants is a broader change) but all publishers and subscribers should be cleaned.

- [ ] **Step 10: Build and verify**

Run: `cargo build --workspace`
Expected: Clean compile. Fix any remaining import errors.

- [ ] **Step 11: Run the dev_server test**

Run: `cargo nextest run -p desktop -E 'test(dev_server_covers_all_tauri_commands)'`
Expected: PASS (the atoms commands are no longer registered, so they shouldn't be in DEV_COMMANDS either)

- [ ] **Step 12: Commit**

```bash
git add -A crates/app-core/ crates/desktop/ crates/desktop-shared/
git commit -m "refactor(desktop): remove user-facing atom API — atoms are now internal-only"
```

---

### Task 6: Remove Frontend Atom UI

**Files:**
- Delete: `desktop-ui/src/features/notes/components/KnowledgeAtomsPanel.tsx`
- Delete: `desktop-ui/src/features/notes/components/AtomCard.tsx`
- Delete: `desktop-ui/src/features/notes/components/BulkAcceptModal.tsx`
- Delete: `desktop-ui/src/features/notes/components/InlineReview.tsx`
- Delete: `desktop-ui/src/features/notes/components/WhyThisPopover.tsx`
- Delete: `desktop-ui/src/features/notes/components/insight/AtomsTab.tsx`
- Delete: `desktop-ui/src/features/notes/hooks/useKnowledgeAtoms.ts`
- Modify: `desktop-ui/src/features/notes/components/InsightReviewPanel.tsx`
- Modify: `desktop-ui/src/features/notes/components/insight/KnowledgeGrowthMetrics.tsx`
- Modify: `desktop-ui/src/features/notes/hooks/useVocabularySave.ts`
- Modify: `desktop-ui/src/features/notes/pages/KnowledgeBasePage.tsx`

- [ ] **Step 1: Delete all atom-specific component files**

Delete these 7 files:
- `desktop-ui/src/features/notes/components/KnowledgeAtomsPanel.tsx`
- `desktop-ui/src/features/notes/components/AtomCard.tsx`
- `desktop-ui/src/features/notes/components/BulkAcceptModal.tsx`
- `desktop-ui/src/features/notes/components/InlineReview.tsx`
- `desktop-ui/src/features/notes/components/WhyThisPopover.tsx`
- `desktop-ui/src/features/notes/components/insight/AtomsTab.tsx`
- `desktop-ui/src/features/notes/hooks/useKnowledgeAtoms.ts`

- [ ] **Step 2: Remove Atoms tab from InsightReviewPanel**

In `desktop-ui/src/features/notes/components/InsightReviewPanel.tsx`:

Remove the `AtomsTab` import (line 29):
```typescript
// DELETE: import { AtomsTab } from "./insight/AtomsTab";
```

Remove `{ id: "atoms", label: "Atoms" }` from the tabs array (line 64).

Remove the `case "atoms":` from any tab content switch (line 93).

Remove the `state.activeTab === "atoms"` render branch (lines 321-322):
```typescript
// DELETE: ) : state.activeTab === "atoms" ? (
// DELETE:   <AtomsTab noteId={state.noteId} />
```

If `"atoms"` is the default `activeTab` (line ~152), change the default to the first remaining tab (e.g. `"synthesis"` or `"gaps"`).

- [ ] **Step 3: Delete or simplify KnowledgeGrowthMetrics**

Delete `desktop-ui/src/features/notes/components/insight/KnowledgeGrowthMetrics.tsx` entirely. If it's imported elsewhere, remove those imports and renderings too. Search for `KnowledgeGrowthMetrics` in the codebase.

- [ ] **Step 4: Clean up useVocabularySave**

In `desktop-ui/src/features/notes/hooks/useVocabularySave.ts`, remove line 49:
```typescript
// DELETE: invalidateQueries("atoms_for_note");
```

- [ ] **Step 5: Clean up KnowledgeBasePage atomId param**

In `desktop-ui/src/features/notes/pages/KnowledgeBasePage.tsx`, remove the `atomId` URL parameter handling (lines 120-125):
```typescript
// DELETE: const atomId = searchParams.get("atomId");
// DELETE: if (atomId) {
// DELETE:   setSearchParams({ atomId }, { replace: true });
// DELETE: }
```

- [ ] **Step 6: Run the frontend linter**

Run: `cd desktop-ui && bun run lint:fix`
Expected: Clean (Biome auto-fixes imports)

- [ ] **Step 7: Run frontend tests**

Run: `cd desktop-ui && bun run test`
Expected: All pass

- [ ] **Step 8: Commit**

```bash
cd /Users/jayden/Projects/Klynt/bot
git add -A desktop-ui/
git commit -m "refactor(ui): remove all atom UI — atoms are now invisible to users"
```

---

### Task 7: Add Frontend Blur/Close Trigger

Fire `note_editing_finished` IPC when the user leaves the note editor.

**Files:**
- Modify: The note editor wrapper component (find by searching for the editor mount/unmount or `onBlur`)

- [ ] **Step 1: Find the note editor component**

Search `desktop-ui/src/` for the component that wraps the note editor (likely in `features/notes/components/` or `features/notes/pages/`). Look for `onBlur`, `useEffect` cleanup on unmount, or the component that passes `noteId` to the editor.

- [ ] **Step 2: Add blur/unmount handler**

In the editor wrapper component, add:

```typescript
import { ipc } from "@shared/hooks/useIpc";

// Inside the component:
const noteIdRef = useRef(noteId);
noteIdRef.current = noteId;

useEffect(() => {
  return () => {
    // Fire on unmount (navigating away from note)
    if (noteIdRef.current) {
      ipc("note_editing_finished", { params: { noteId: noteIdRef.current } });
    }
  };
}, []);

// On editor blur:
const handleEditorBlur = useCallback(() => {
  if (noteId) {
    ipc("note_editing_finished", { params: { noteId } });
  }
}, [noteId]);
```

Wire `handleEditorBlur` to the editor's `onBlur` prop.

- [ ] **Step 3: Build and test manually**

Run: `cd desktop-ui && bun run build`
Expected: Clean build

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/
git commit -m "feat(ui): fire note_editing_finished on editor blur and unmount"
```

---

### Task 8: Add Daily Cron Catch-All for Extraction

A daily cron job scans notes that changed since their last extraction and triggers extraction for each.

**Files:**
- Modify: `crates/app-core/src/init/cron.rs` (or wherever cron jobs are registered)
- Modify: `crates/cognitive/src/repos/atom_extraction_cache.rs` (add query for stale notes)

- [ ] **Step 1: Add `find_stale_notes` query to AtomExtractionCache**

In `crates/cognitive/src/repos/atom_extraction_cache.rs`, add:

```rust
    /// Find note IDs that have been updated since their last extraction (or never extracted).
    /// Joins against the notes table via a raw query.
    pub async fn find_stale_note_ids(
        &self,
        limit: i64,
    ) -> Result<Vec<(String, String)>, sqlx::Error> {
        // Returns (note_id, body) for notes that:
        // 1. Have no entry in atom_extraction_cache, OR
        // 2. Have a different content hash than cached
        sqlx::query_as::<_, (String, String)>(
            r#"
            SELECT n.id, n.body
            FROM notes n
            LEFT JOIN atom_extraction_cache c ON c.note_id = n.id
            WHERE n.body != ''
              AND (c.note_id IS NULL
                   OR c.content_hash != ?1)
            ORDER BY n.updated_at DESC
            LIMIT ?2
            "#,
        )
        .bind("") // placeholder — we can't compute hash here; just find notes without cache entries
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }
```

Actually, a simpler approach: just find notes with no cache entry or cache entry older than 7 days. The exact approach depends on whether `atom_extraction_cache` stores timestamps. Check the table schema and adapt.

- [ ] **Step 2: Register the daily cron job**

In the cron init code, add a job that:
1. Calls `find_stale_note_ids(50)` to get up to 50 notes
2. For each, publishes `NoteEditingFinished` to the bus (which the extraction service picks up)

This reuses the existing extraction pipeline — no new extraction code needed.

- [ ] **Step 3: Build and verify**

Run: `cargo build --workspace`
Expected: Clean compile

- [ ] **Step 4: Commit**

```bash
git add crates/cognitive/src/repos/atom_extraction_cache.rs crates/app-core/src/init/cron.rs
git commit -m "feat(cognitive): add daily cron catch-all for atom extraction on unprocessed notes"
```

---

### Task 9: Stop Publishing `NoteContentChanged` for Atom Extraction

Now that extraction is driven by `NoteEditingFinished`, we should verify `NoteContentChanged` is still needed for other subscribers (BookIndex updater). If BookIndex is the only other subscriber, keep it. Just verify no double-extraction occurs.

**Files:**
- Review: `crates/app-core/src/handlers/notes/crud.rs`

- [ ] **Step 1: Verify other NoteContentChanged subscribers**

Search for `NoteContentChanged` in subscriber/listener code. The BookIndex updater subscribes to this event. Confirm it's separate from atom extraction.

- [ ] **Step 2: No code changes needed if BookIndex uses it**

`NoteContentChanged` stays for BookIndex. `NoteEditingFinished` is for atom extraction only. The extraction service no longer listens to `NoteContentChanged`, so no double-extraction risk.

- [ ] **Step 3: Run full workspace tests**

Run: `cargo nextest run --workspace`
Expected: All pass

- [ ] **Step 4: Run clippy on full workspace**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

- [ ] **Step 5: Commit (if any cleanup was needed)**

```bash
git add -A
git commit -m "chore: verify NoteContentChanged still needed for BookIndex, no atom extraction overlap"
```

---

### Task 10: Final Integration Verification

- [ ] **Step 1: Run full workspace build**

Run: `cargo build --workspace`
Expected: Clean compile

- [ ] **Step 2: Run full test suite**

Run: `cargo nextest run --workspace`
Expected: All pass

- [ ] **Step 3: Run doctests**

Run: `cargo test --workspace --doc`
Expected: All pass

- [ ] **Step 4: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

- [ ] **Step 5: Run format check**

Run: `cargo fmt --all --check`
Expected: Clean

- [ ] **Step 6: Run frontend build**

Run: `cd desktop-ui && bun run build`
Expected: Clean build

- [ ] **Step 7: Run frontend lint**

Run: `cd desktop-ui && bun run lint`
Expected: Clean

- [ ] **Step 8: Run frontend tests**

Run: `cd desktop-ui && bun run test`
Expected: All pass

- [ ] **Step 9: Manual smoke test**

1. Start the dev server: `cargo tauri dev` + `cd desktop-ui && bun run dev`
2. Create a note with substantial content (~100 words)
3. Navigate away from the note (blur)
4. Check logs for `"extracting atoms from note"` — should appear after blur
5. Verify no "Atoms" tab in Insight Review panel
6. Verify flashcard generation still works from a note
7. Verify Insight Review still shows Knowledge State section (powered by atoms invisibly)
