# Knowledge Atoms Phase 1: "The Atom Core" Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Introduce the Knowledge Atom entity as the central node linking SemanticFacts, Flashcards, and notes — with DomainEvent emission, a right-panel UI, inline quick review, and vocab migration.

**Architecture:** New `knowledge_atoms` and `knowledge_topics` tables in the cognitive crate. `KnowledgeAtomRepo` provides CRUD. Existing `language_save_vocabulary` upgraded to create atoms alongside SemanticFacts+Flashcards. 11 new `DomainEvent` variants flow through the existing broadcast bus to ActivityLog, Salience, and future coaching consumers. Frontend gets a `KnowledgeAtomsPanel` in the note right panel with inline flashcard review.

**Tech Stack:** Rust (SQLite via sqlx, tokio broadcast), React (TypeScript, Radix UI), Tauri IPC, FSRS-5

**Spec:** `docs/superpowers/specs/2026-03-21-unified-learning-system-design.md`

**Phases:** This plan covers Phase 1 only. Phases 2-4 (auto-extraction, coaching, dashboard) will get separate plans after Phase 1 ships.

---

## File Map

### New files
| File | Responsibility |
|---|---|
| `crates/cognitive/src/repos/knowledge_atom.rs` | KnowledgeAtomRepo + KnowledgeTopicRepo (CRUD, queries) |
| `crates/desktop/src/commands/atoms.rs` | Tauri commands + DEV_COMMANDS + dispatch_dev |
| `crates/desktop-shared/src/commands/atoms.rs` | IPC param/response types |
| `crates/app-core/src/handlers/atoms.rs` | AppCore handler methods (accept, dismiss, next_card, restore) |
| `crates/app-core/src/init/atoms.rs` | Vocab → atom migration job |
| `desktop-ui/src/features/notes/components/KnowledgeAtomsPanel.tsx` | Right panel atom section |
| `desktop-ui/src/features/notes/components/AtomCard.tsx` | Single atom card (active + suggested variants) |
| `desktop-ui/src/features/notes/components/InlineReview.tsx` | Inline flashcard review embedded in atom card |
| `desktop-ui/src/features/notes/hooks/useKnowledgeAtoms.ts` | Query + mutation hooks for atoms |

### Modified files
| File | Changes |
|---|---|
| `crates/cognitive/migrations/001_cognitive_tables.sql` | Add knowledge_atoms, knowledge_topics tables + flashcards.atom_id + indexes |
| `crates/cognitive/src/repos/mod.rs` | Export new repos, bump migration version |
| `crates/cognitive/src/repos/flashcard.rs` | Add atom_id to FlashcardRow + NewFlashcard, update queries |
| `crates/bus/src/domain_events.rs` | Add 11 new DomainEvent variants |
| `crates/cognitive/src/services/salience.rs` | Add salience classification for new events |
| `crates/activity-log/src/normalizers.rs` | Add normalization for new events |
| `crates/app-core/src/state.rs` | Add KnowledgeAtomRepo to AppCore |
| `crates/app-core/src/handlers/notes/language.rs` | Create atoms in language_save_vocabulary |
| `crates/app-core/src/handlers/mod.rs` | Add atoms handler module |
| `crates/desktop/src/commands/mod.rs` | Add atoms command module |
| `crates/desktop/src/main.rs` | Register atom commands with Tauri |
| `crates/desktop/src/dev_server/mod.rs` | Add atoms DEV_COMMANDS to dispatch + test list |
| `crates/desktop-shared/src/commands/mod.rs` | Add atoms command types module |
| `desktop-ui/src/features/notes/components/NoteEditor.tsx` | Add KnowledgeAtomsPanel to right panel |
| `desktop-ui/src/features/notes/components/editor/SplitEditor.tsx` | Pass atom data to LanguageLearningPanel area |

---

### Task 1: Schema — knowledge_atoms + knowledge_topics tables

**Files:**
- Modify: `crates/cognitive/migrations/001_cognitive_tables.sql`
- Modify: `crates/cognitive/src/repos/mod.rs`

- [ ] **Step 1: Add knowledge_topics table to migration SQL**

Insert before the `flashcards` table definition (before line ~447) in `001_cognitive_tables.sql`:

```sql
-- ── Knowledge Topics ────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS knowledge_topics (
    id              TEXT PRIMARY KEY NOT NULL,
    name            TEXT NOT NULL,
    domain          TEXT NOT NULL,
    atom_count      INTEGER NOT NULL DEFAULT 0,
    avg_retention   REAL NOT NULL DEFAULT 1.0,
    created_at      TEXT NOT NULL
);

-- ── Knowledge Atoms ─────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS knowledge_atoms (
    id                  TEXT PRIMARY KEY NOT NULL,
    subject             TEXT NOT NULL,
    atom_type           TEXT NOT NULL CHECK (atom_type IN ('vocabulary', 'concept', 'skill', 'fact')),
    domain              TEXT NOT NULL,
    source_note_id      TEXT,
    source_range        TEXT,
    source_context      TEXT,
    secondary_sources   TEXT,
    semantic_fact_id    TEXT,
    retention_pct       REAL NOT NULL DEFAULT 1.0,
    stability           REAL NOT NULL DEFAULT 1.0,
    difficulty          REAL NOT NULL DEFAULT 5.0,
    personal_importance REAL NOT NULL DEFAULT 0.7,
    status              TEXT NOT NULL DEFAULT 'suggested' CHECK (status IN ('suggested', 'active', 'archived')),
    salience            REAL NOT NULL DEFAULT 1.0,
    last_interaction_ts TEXT,
    archived_at         TEXT,
    metadata            TEXT,
    topic_id            TEXT REFERENCES knowledge_topics(id),
    created_at          TEXT NOT NULL,
    updated_at          TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_atoms_note ON knowledge_atoms(source_note_id) WHERE status != 'archived';
CREATE INDEX IF NOT EXISTS idx_atoms_last_interaction ON knowledge_atoms(last_interaction_ts) WHERE status = 'active';
CREATE INDEX IF NOT EXISTS idx_atoms_topic ON knowledge_atoms(topic_id);
CREATE INDEX IF NOT EXISTS idx_atoms_status ON knowledge_atoms(status, salience);
CREATE INDEX IF NOT EXISTS idx_atoms_subject ON knowledge_atoms(subject, domain) WHERE status != 'archived';
```

- [ ] **Step 2: Add atom_id column to flashcards table**

In the existing `CREATE TABLE IF NOT EXISTS flashcards` block, add after the `source_context` column:

```sql
    atom_id         TEXT REFERENCES knowledge_atoms(id),
```

- [ ] **Step 3: Bump migration version**

In `crates/cognitive/src/repos/mod.rs`, find the `cognitive_migrations()` function and bump the version number by 1.

- [ ] **Step 4: Verify schema compiles**

Run: `cargo build -p cognitive 2>&1 | tail -5`
Expected: successful build (warnings OK)

- [ ] **Step 5: Commit**

```
feat(cognitive): add knowledge_atoms + knowledge_topics schema
```

---

### Task 2: KnowledgeAtomRepo — CRUD + queries

**Files:**
- Create: `crates/cognitive/src/repos/knowledge_atom.rs`
- Modify: `crates/cognitive/src/repos/mod.rs`

- [ ] **Step 1: Write the repo with core CRUD methods**

Create `crates/cognitive/src/repos/knowledge_atom.rs`. Follow the pattern from `flashcard.rs` (uses raw sqlx, `SqlitePool`). Include:

- `KnowledgeAtomRow` struct with all columns (serde rename_all = "snake_case" for SQL mapping)
- `KnowledgeTopicRow` struct
- `KnowledgeAtomRepo::new(pool: SqlitePool)`
- `create(&self, atom: &NewKnowledgeAtom) -> Result<KnowledgeAtomRow>`
- `create_batch(&self, atoms: Vec<NewKnowledgeAtom>) -> Result<Vec<KnowledgeAtomRow>>`
- `get(&self, id: &str) -> Result<Option<KnowledgeAtomRow>>`
- `list_for_note(&self, note_id: &str) -> Result<Vec<KnowledgeAtomRow>>` — returns active + suggested atoms for a note
- `accept(&self, id: &str, personal_importance: f64) -> Result<KnowledgeAtomRow>` — sets status="active", importance, last_interaction_ts
- `dismiss(&self, id: &str) -> Result<()>` — sets status="archived", archived_at
- `restore(&self, id: &str) -> Result<KnowledgeAtomRow>` — sets status="active", clears archived_at, resets salience to 0.5
- `update_retention(&self, id: &str, retention_pct: f64, stability: f64, difficulty: f64) -> Result<()>`
- `touch(&self, id: &str) -> Result<()>` — updates last_interaction_ts to now
- `get_or_create_topic(&self, name: &str, domain: &str) -> Result<KnowledgeTopicRow>`
- `update_topic_aggregates(&self, topic_id: &str) -> Result<()>` — recomputes atom_count + avg_retention
- `update_all_topic_aggregates(&self) -> Result<()>` — iterates all topics and calls update_topic_aggregates for each

`NewKnowledgeAtom` struct (with `#[derive(Default)]`) with all fields except id/created_at/updated_at (those are generated).

- [ ] **Step 2: Export from repos/mod.rs**

Add `pub mod knowledge_atom;` and `pub use knowledge_atom::*;` to `crates/cognitive/src/repos/mod.rs`.

- [ ] **Step 3: Write unit tests**

Add `#[cfg(test)] mod tests` at the bottom of `knowledge_atom.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::cognitive_test_pool; // in-memory pool from repos/mod.rs

    #[tokio::test]
    async fn test_create_and_get_atom() { /* create atom, get by id, assert fields */ }

    #[tokio::test]
    async fn test_list_for_note() { /* create 3 atoms for same note, 1 for different, assert count == 3 */ }

    #[tokio::test]
    async fn test_accept_and_dismiss() { /* create suggested, accept, verify status+importance; dismiss, verify archived */ }

    #[tokio::test]
    async fn test_restore_archived_atom() { /* dismiss, restore, verify status=active, salience=0.5 */ }

    #[tokio::test]
    async fn test_get_or_create_topic() { /* create topic, call again with same name, assert same id */ }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p cognitive -E 'test(knowledge_atom)'`
Expected: all tests pass

- [ ] **Step 5: Commit**

```
feat(cognitive): add KnowledgeAtomRepo with CRUD + topic management
```

---

### Task 3: FlashcardRow — add atom_id field

**Files:**
- Modify: `crates/cognitive/src/repos/flashcard.rs`

- [ ] **Step 1: Add atom_id to FlashcardRow struct**

In `FlashcardRow` struct (line ~72), add:
```rust
pub atom_id: Option<String>,
```

- [ ] **Step 2: Add atom_id to NewFlashcard struct**

In `NewFlashcard` struct, add:
```rust
pub atom_id: Option<String>,
```

- [ ] **Step 3: Update create_batch INSERT query**

In `create_batch()`, add `atom_id` to the INSERT column list and the VALUES binding. Follow the existing pattern for nullable fields.

- [ ] **Step 4: Update all SELECT queries**

Any SELECT * or explicit column lists in `get_by_id`, `get_due_cards`, `list_by_note`, etc. must include `atom_id`. If using `SELECT *` (via `query_as!`), this happens automatically from the schema change.

- [ ] **Step 5: Run existing flashcard tests**

Run: `cargo nextest run -p cognitive -E 'test(flashcard)'`
Expected: all existing tests pass (atom_id is Option, defaults to None)

- [ ] **Step 6: Commit**

```
feat(cognitive): add atom_id FK to flashcards table
```

---

### Task 4: DomainEvent variants

**Files:**
- Modify: `crates/bus/src/domain_events.rs`

- [ ] **Step 1: Add 11 new variants to DomainEvent enum**

At the end of the `DomainEvent` enum (before the closing `}`), add:

```rust
    // ── Knowledge Atoms ──────────────────────────────────────────
    KnowledgeAtomCreated {
        atom_id: String,
        atom_type: String,
        domain: String,
        source_note_id: Option<String>,
        personal_importance: f64,
    },
    KnowledgeAtomAccepted {
        atom_id: String,
        atom_type: String,
    },
    KnowledgeAtomArchived {
        atom_id: String,
        reason: String,
    },
    AtomFlashcardReviewed {
        atom_id: String,
        card_id: String,
        quality: u8,
        recall_speed_ms: u64,
        new_retention_pct: f64,
        source_note_id: Option<String>,
    },
    AtomReinforced {
        atom_id: String,
        referencing_note_id: String,
        new_salience: f64,
    },
    AtomInteracted {
        atom_id: String,
        interaction_type: String,
        note_id: Option<String>,
    },
    RetentionMilestoneReached {
        atom_id: String,
        topic_id: Option<String>,
        new_retention_pct: f64,
        milestone: String,
        previous_pct: f64,
    },
    TranslationCompleted {
        note_id: String,
        source_lang: String,
        target_lang: String,
        word_count: usize,
        is_selection: bool,
    },
    NoteStudied {
        note_id: String,
        duration_secs: u64,
        atoms_reviewed: usize,
        mode: String,
    },
    KnowledgeTransferDetected {
        atom_id: String,
        from_domain: String,
        to_domain: String,
        confidence: f64,
    },
    CoachingLearningDigest {
        fading_count: usize,
        archived_count: usize,
        streak_days: usize,
        strongest_topic: Option<String>,
        weakest_topic: Option<String>,
    },
```

- [ ] **Step 2: Build to verify no compile errors**

Run: `cargo build -p bus 2>&1 | tail -5`
Expected: successful build

- [ ] **Step 3: Check clippy for exhaustive match warnings across workspace**

Run: `cargo clippy --workspace --all-targets 2>&1 | grep -i "non-exhaustive\|unreachable" | head -20`
Expected: zero warnings (existing consumers use `_ => {}` catchalls)

- [ ] **Step 4: Commit**

```
feat(bus): add 11 Knowledge Atom DomainEvent variants
```

---

### Task 5: Salience classification + Activity log normalization

**Files:**
- Modify: `crates/cognitive/src/services/salience.rs`
- Modify: `crates/activity-log/src/normalizers.rs`

- [ ] **Step 1: Add salience verdicts for new events**

In `evaluate_salience()` match block in `salience.rs`, add arms:

```rust
DomainEvent::KnowledgeAtomAccepted { .. } => SalienceVerdict::Extract,
DomainEvent::RetentionMilestoneReached { .. } => SalienceVerdict::Extract,
DomainEvent::AtomFlashcardReviewed { .. } => SalienceVerdict::Accumulate,
DomainEvent::TranslationCompleted { .. } => SalienceVerdict::Accumulate,
DomainEvent::AtomReinforced { .. } => SalienceVerdict::Accumulate,
DomainEvent::NoteStudied { .. } => SalienceVerdict::Accumulate,
DomainEvent::KnowledgeAtomCreated { .. } => SalienceVerdict::Discard,
DomainEvent::KnowledgeAtomArchived { .. } => SalienceVerdict::Discard,
DomainEvent::AtomInteracted { .. } => SalienceVerdict::Discard,
DomainEvent::KnowledgeTransferDetected { .. } => SalienceVerdict::Accumulate,
DomainEvent::CoachingLearningDigest { .. } => SalienceVerdict::Discard,
```

- [ ] **Step 2: Add normalization for new events**

In `normalize_domain_event()` in `normalizers.rs`, add match arms mapping each new event to `ActivityLogEntry`. Follow the existing pattern (source, actor, action, preview). Key mappings:
- `KnowledgeAtomCreated` → source: "learning", action: "atom_created", preview: subject
- `KnowledgeAtomAccepted` → action: "atom_accepted"
- `AtomFlashcardReviewed` → source: "learning", action: "flashcard_reviewed"
- `TranslationCompleted` → source: "learning", action: "translation_completed"
- `NoteStudied` → source: "learning", action: "note_studied"
- Others: follow same pattern with descriptive action strings

- [ ] **Step 3: Build workspace to verify**

Run: `cargo build --workspace 2>&1 | tail -5`
Expected: successful build

- [ ] **Step 4: Commit**

```
feat(cognitive,activity-log): classify and normalize Knowledge Atom events
```

---

### Task 6: IPC types — desktop-shared

**Files:**
- Create: `crates/desktop-shared/src/commands/atoms.rs`
- Modify: `crates/desktop-shared/src/commands/mod.rs` (or wherever command types are re-exported)

- [ ] **Step 1: Create atoms command types**

Create `crates/desktop-shared/src/commands/atoms.rs`:

```rust
use serde::{Deserialize, Serialize};

// ── Params ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AtomsForNoteParams {
    pub note_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AtomAcceptParams {
    pub atom_id: String,
    pub personal_importance: Option<f64>, // defaults to 0.7
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AtomDismissParams {
    pub atom_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AtomRestoreParams {
    pub atom_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AtomNextCardParams {
    pub atom_id: String,
}

// ── Responses ───────────────────────────────────────────────────

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeAtomResponse {
    pub id: String,
    pub subject: String,
    pub atom_type: String,
    pub domain: String,
    pub source_note_id: Option<String>,
    pub source_range: Option<String>,
    pub source_context: Option<String>,
    pub semantic_fact_id: Option<String>,
    pub retention_pct: f64,
    pub personal_importance: f64,
    pub status: String,
    pub salience: f64,
    pub last_interaction_ts: Option<String>,
    pub metadata: Option<String>,
    pub topic_name: Option<String>,
    pub linked_card_count: i64,
    pub created_at: String,
}
```

- [ ] **Step 2: Export from mod.rs**

Add `pub mod atoms;` and `pub use atoms::*;` to the commands module.

- [ ] **Step 3: Build to verify**

Run: `cargo build -p desktop-shared 2>&1 | tail -5`
Expected: successful build

- [ ] **Step 4: Commit**

```
feat(desktop-shared): add Knowledge Atom IPC types
```

---

### Task 7: AppCore handlers — atom operations

**Files:**
- Create: `crates/app-core/src/handlers/atoms.rs`
- Modify: `crates/app-core/src/handlers/mod.rs`
- Modify: `crates/app-core/src/state.rs`

- [ ] **Step 1: Add KnowledgeAtomRepo to AppCore state**

In `crates/app-core/src/state.rs`, add to the `AppCore` struct definition:
```rust
pub knowledge_atom_repo: Option<cognitive::KnowledgeAtomRepo>,
```

Then in `crates/app-core/src/init/mod.rs`, in the `AppCore { ... }` struct literal (where `flashcard_repo` is initialized around line ~240), add:
```rust
knowledge_atom_repo: Some(cognitive::KnowledgeAtomRepo::new(storage_pool.inner().clone())),
```

- [ ] **Step 2: Create atoms handler module**

Create `crates/app-core/src/handlers/atoms.rs` with methods on `impl AppCore`:

```rust
use desktop_shared::commands::atoms::*;
use desktop_shared::errors::ApiError;

impl AppCore {
    pub async fn atoms_for_note(&self, params: AtomsForNoteParams) -> Result<Vec<KnowledgeAtomResponse>, ApiError> {
        let repo = self.knowledge_atom_repo.as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Knowledge atoms not available"))?;
        let atoms = repo.list_for_note(&params.note_id).await
            .map_err(|e| ApiError::new("DB_ERROR", e.to_string()))?;
        // Convert rows to responses (with topic name join + linked card count)
        Ok(atoms.into_iter().map(atom_row_to_response).collect())
    }

    pub async fn atom_accept(&self, params: AtomAcceptParams) -> Result<KnowledgeAtomResponse, ApiError> {
        let repo = /* ... */;
        let importance = params.personal_importance.unwrap_or(0.7);
        let atom = repo.accept(&params.atom_id, importance).await?;
        // Emit KnowledgeAtomAccepted event via bus
        if let Some(bus) = &self.event_bus {
            let _ = bus.publish(DomainEvent::KnowledgeAtomAccepted {
                atom_id: atom.id.clone(),
                atom_type: atom.atom_type.clone(),
            });
        }
        // Update topic aggregates
        if let Some(topic_id) = &atom.topic_id {
            repo.update_topic_aggregates(topic_id).await.ok();
        }
        Ok(atom_row_to_response(atom))
    }

    pub async fn atom_dismiss(&self, params: AtomDismissParams) -> Result<(), ApiError> {
        // Similar: dismiss + emit KnowledgeAtomArchived + update topic
    }

    pub async fn atom_restore(&self, params: AtomRestoreParams) -> Result<KnowledgeAtomResponse, ApiError> {
        // Similar: restore + update topic
    }

    pub async fn atom_next_card(&self, params: AtomNextCardParams) -> Result<Option<FlashcardResponse>, ApiError> {
        // Query flashcards WHERE atom_id = ? AND due_at <= now ORDER BY due_at LIMIT 1
        // Fallback: most recently created card for this atom
    }
}
```

- [ ] **Step 3: Register in handlers/mod.rs**

Add `pub mod atoms;` to `crates/app-core/src/handlers/mod.rs`.

- [ ] **Step 4: Build to verify**

Run: `cargo build -p app-core 2>&1 | tail -5`
Expected: successful build

- [ ] **Step 5: Commit**

```
feat(app-core): add Knowledge Atom handler methods
```

---

### Task 8: Upgrade language_save_vocabulary to create atoms

**Files:**
- Modify: `crates/app-core/src/handlers/notes/language.rs`

- [ ] **Step 1: Create atoms in the vocabulary save loop**

In `language_save_vocabulary` (line ~109), after creating the `NewFlashcard` and `SemanticFact`, also create a `NewKnowledgeAtom`:

```rust
// After SemanticFact upsert and before FlashcardRepo::create_batch:
let atom_repo = self.knowledge_atom_repo.as_ref()
    .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Atom repo not available"))?;

let mut new_atoms = Vec::new();
for (i, item) in params.words.iter().enumerate() {
    let topic = atom_repo.get_or_create_topic(
        &params.deck, // deck name = topic name
        &format!("language:{}", /* infer lang from word */),
    ).await.map_err(/* ... */)?;

    let vocab_metadata = serde_json::json!({
        "word": item.word,
        "reading": item.reading,
        "meaning": item.meaning,
        "partOfSpeech": item.part_of_speech,
        "exampleSentence": item.example_sentence,
    });

    new_atoms.push(NewKnowledgeAtom {
        subject: item.word.clone(),
        atom_type: "vocabulary".to_string(),
        domain: format!("language:{}", /* target lang */),
        source_note_id: params.note_id.clone(),
        source_range: None, // vocab from translation doesn't have precise range
        source_context: item.example_sentence.clone(),
        semantic_fact_id: Some(fact_id.clone()),
        personal_importance: 0.7,
        status: "active".to_string(), // vocab saves are user-intentional
        metadata: Some(serde_json::to_string(&vocab_metadata).unwrap_or_default()),
        topic_id: Some(topic.id.clone()),
    });
}

let created_atoms = atom_repo.create_batch(new_atoms).await?;

// Link flashcards to atoms by matching word/front
for card in &created {
    if let Some(atom) = created_atoms.iter().find(|a| a.subject == card.front) {
        // UPDATE flashcards SET atom_id = atom.id WHERE id = card.id
    }
}

// Emit events
if let Some(bus) = &self.event_bus {
    for atom in &created_atoms {
        let _ = bus.publish(DomainEvent::KnowledgeAtomCreated { /* fields */ });
        let _ = bus.publish(DomainEvent::KnowledgeAtomAccepted { /* fields */ });
    }
}
```

- [ ] **Step 2: Add TranslationCompleted event to translate_breakdown**

Note: `TranslationCompleted` requires a `note_id`, but `TranslateBreakdownParams` doesn't carry one. This event is better emitted from the **frontend** side (which knows the note context) or from a wrapper handler. For Phase 1, skip emitting `TranslationCompleted` from this handler — it will be wired in Phase 2 when the extraction pipeline has the full note context. The DomainEvent variant is still added in Task 4 for forward compatibility.

- [ ] **Step 3: Add AtomFlashcardReviewed event to flashcard_record_review**

In `crates/cognitive/src/repos/flashcard.rs` `record_review()` method, this requires access to the bus. Since the repo doesn't have the bus, the event emission should happen in the app-core handler that calls `record_review()`. Find the `flashcard_record_review` handler in `app-core` and add event emission after the repo call:

```rust
if let Some(atom_id) = &updated_card.atom_id {
    if let Some(bus) = &self.event_bus {
        let _ = bus.publish(DomainEvent::AtomFlashcardReviewed {
            atom_id: atom_id.clone(),
            card_id: updated_card.id.clone(),
            quality,
            recall_speed_ms,
            new_retention_pct: updated_card.retention_pct(),
            source_note_id: updated_card.source_note_id.clone(),
        });
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run --workspace -E 'test(language) | test(flashcard)'`
Expected: all pass

- [ ] **Step 5: Commit**

```
feat(app-core): create Knowledge Atoms on vocabulary save + emit events
```

---

### Task 9: Desktop commands — Tauri + dev server

**Files:**
- Create: `crates/desktop/src/commands/atoms.rs`
- Modify: `crates/desktop/src/commands/mod.rs`
- Modify: `crates/desktop/src/main.rs`
- Modify: `crates/desktop/src/dev_server/mod.rs`

- [ ] **Step 1: Create atoms command module**

Create `crates/desktop/src/commands/atoms.rs` following the pattern from `commands/language.rs`:

```rust
use app_core::AppCore;
use desktop_shared::commands::atoms::*;
use desktop_shared::errors::ApiError;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn atoms_for_note(
    state: State<'_, Arc<AppCore>>,
    params: AtomsForNoteParams,
) -> Result<Vec<KnowledgeAtomResponse>, ApiError> {
    state.atoms_for_note(params).await
}

#[tauri::command]
pub async fn atom_accept(
    state: State<'_, Arc<AppCore>>,
    params: AtomAcceptParams,
) -> Result<KnowledgeAtomResponse, ApiError> {
    state.atom_accept(params).await
}

#[tauri::command]
pub async fn atom_dismiss(
    state: State<'_, Arc<AppCore>>,
    params: AtomDismissParams,
) -> Result<(), ApiError> {
    state.atom_dismiss(params).await
}

#[tauri::command]
pub async fn atom_restore(
    state: State<'_, Arc<AppCore>>,
    params: AtomRestoreParams,
) -> Result<KnowledgeAtomResponse, ApiError> {
    state.atom_restore(params).await
}

#[tauri::command]
pub async fn atom_next_card(
    state: State<'_, Arc<AppCore>>,
    params: AtomNextCardParams,
) -> Result<Option<FlashcardResponse>, ApiError> {
    state.atom_next_card(params).await
}

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "atoms_for_note",
    "atom_accept",
    "atom_dismiss",
    "atom_restore",
    "atom_next_card",
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "atoms_for_note" => dev::val(core.atoms_for_note(try_field!(dev::parse_params(body))).await),
        "atom_accept" => dev::val(core.atom_accept(try_field!(dev::parse_params(body))).await),
        "atom_dismiss" => dev::val(core.atom_dismiss(try_field!(dev::parse_params(body))).await),
        "atom_restore" => dev::val(core.atom_restore(try_field!(dev::parse_params(body))).await),
        "atom_next_card" => dev::val(core.atom_next_card(try_field!(dev::parse_params(body))).await),
        _ => return None,
    })
}
```

- [ ] **Step 2: Register in commands/mod.rs**

Add `pub mod atoms;` to `crates/desktop/src/commands/mod.rs`.

- [ ] **Step 3: Register Tauri commands in main.rs**

In `main.rs`, find the `.invoke_handler(tauri::generate_handler![...])` block and add:
```rust
commands::atoms::atoms_for_note,
commands::atoms::atom_accept,
commands::atoms::atom_dismiss,
commands::atoms::atom_restore,
commands::atoms::atom_next_card,
```

- [ ] **Step 4: Add to dev server dispatch + test coverage**

1. In `crates/desktop/src/dev_server/dispatch.rs` (NOT mod.rs), add the dispatch call inside the `dispatch()` function, alongside the other module dispatches:
```rust
if let Some(r) = commands::atoms::dispatch_dev(cmd, core, &body).await {
    return into_api_result(r);
}
```
2. In `crates/desktop/src/dev_server/mod.rs`, add `commands::atoms::DEV_COMMANDS` to the test coverage list (the BTreeSet in the parity test).

- [ ] **Step 5: Build desktop crate**

Run: `cargo build -p desktop 2>&1 | tail -5`
Expected: successful build

- [ ] **Step 6: Run dev_server parity test**

Run: `cargo nextest run -p desktop -E 'test(dev_server_covers)'`
Expected: pass (new commands are covered)

- [ ] **Step 7: Commit**

```
feat(desktop): add atom Tauri commands + dev server dispatch
```

---

### Task 10: Vocab migration job

**Files:**
- Create: `crates/app-core/src/init/atoms.rs`
- Modify: `crates/app-core/src/init/mod.rs`

- [ ] **Step 1: Create migration job**

Create `crates/app-core/src/init/atoms.rs`:

```rust
use cognitive::repos::{KnowledgeAtomRepo, SemanticFactRepo, FlashcardRepo};
use sqlx::SqlitePool;
use tracing::{info, warn};

/// Migration flag stored as a knowledge_atom with a sentinel subject.
const MIGRATION_SENTINEL: &str = "__atoms_migration_v1__";

pub async fn migrate_vocab_to_atoms(pool: &SqlitePool) -> common::Result<usize> {
    // 1. Check if already migrated (use a sentinel row in knowledge_atoms itself)
    let existing: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM knowledge_atoms WHERE subject = ?",
    ).bind(MIGRATION_SENTINEL).fetch_optional(pool).await?;
    if existing.is_some() {
        return Ok(0);
    }

    let sf_repo = SemanticFactRepo::new(pool.clone());
    let atom_repo = KnowledgeAtomRepo::new(pool.clone());

    // 2. Fetch all active vocabulary semantic facts
    let facts = sf_repo.list_vocabulary_facts().await?; // Need to add this method
    info!("Migrating {} vocabulary items to Knowledge Atoms", facts.len());

    let mut count = 0;
    for fact in &facts {
        // 3. Create atom for each
        let domain = format!("language:{}", /* infer from fact or default "unknown" */);
        let topic = atom_repo.get_or_create_topic(&domain, &domain).await?;

        let atom = atom_repo.create(&NewKnowledgeAtom {
            subject: fact.subject.clone(),
            atom_type: "vocabulary".to_string(),
            domain,
            source_note_id: parse_note_source(&fact.source),
            source_context: Some(fact.object.clone()), // meaning as context
            semantic_fact_id: Some(fact.id.clone()),
            personal_importance: 0.7,
            status: "active".to_string(),
            metadata: None,
            topic_id: Some(topic.id.clone()),
            ..Default::default()
        }).await?;

        // 4. Link existing flashcard if found
        // UPDATE flashcards SET atom_id = ? WHERE front = ? AND card_type = 'vocabulary' AND atom_id IS NULL
        sqlx::query("UPDATE flashcards SET atom_id = ? WHERE front = ? AND card_type = 'vocabulary' AND atom_id IS NULL")
            .bind(&atom.id)
            .bind(&fact.subject)
            .execute(pool).await.ok();

        count += 1;
    }

    // 5. Update topic aggregates
    // SELECT DISTINCT topic_id FROM knowledge_atoms → update each
    atom_repo.update_all_topic_aggregates().await.ok();

    // 6. Set migration flag (sentinel row in knowledge_atoms)
    sqlx::query(
        "INSERT INTO knowledge_atoms (id, subject, atom_type, domain, status, created_at, updated_at) VALUES (?, ?, 'fact', 'system', 'active', ?, ?)"
    )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(MIGRATION_SENTINEL)
        .bind(&now).bind(&now)
        .execute(pool).await?;

    info!("Migration complete: {} atoms created", count);
    Ok(count)
}

fn parse_note_source(source: &str) -> Option<String> {
    source.strip_prefix("note:").map(|s| s.to_string())
}
```

- [ ] **Step 2: Wire into AppCore init**

In `crates/app-core/src/init/mod.rs`, add `pub mod atoms;` and call `atoms::migrate_vocab_to_atoms(pool).await` during AppCore initialization (after storage migrations, before returning AppCore).

- [ ] **Step 3: Build and verify**

Run: `cargo build -p app-core 2>&1 | tail -5`
Expected: successful build

- [ ] **Step 4: Commit**

```
feat(app-core): add vocab-to-atoms one-time migration
```

---

### Task 11: Frontend — useKnowledgeAtoms hook

**Files:**
- Create: `desktop-ui/src/features/notes/hooks/useKnowledgeAtoms.ts`

- [ ] **Step 1: Create the query + mutation hooks**

```typescript
import { ipc } from "@shared/hooks/useIpc";
import { useMutation } from "@shared/hooks/useMutation";
import { useQuery } from "@shared/hooks/useQuery";

export interface KnowledgeAtomResponse {
  id: string;
  subject: string;
  atomType: string;
  domain: string;
  sourceNoteId: string | null;
  sourceRange: string | null;
  sourceContext: string | null;
  retentionPct: number;
  personalImportance: number;
  status: string;
  salience: number;
  lastInteractionTs: string | null;
  metadata: string | null;
  topicName: string | null;
  linkedCardCount: number;
  createdAt: string;
}

export function useKnowledgeAtoms(noteId: string | null) {
  const { data, loading, refetch } = useQuery<KnowledgeAtomResponse[]>(
    noteId ? "atoms_for_note" : null,
    noteId ? { noteId } : undefined,
    [],
  );

  const activeAtoms = data?.filter((a) => a.status === "active") ?? [];
  const suggestedAtoms = data?.filter((a) => a.status === "suggested") ?? [];

  return { activeAtoms, suggestedAtoms, loading, refetch };
}

export function useAtomAccept() {
  return useMutation<KnowledgeAtomResponse>("atom_accept");
}

export function useAtomDismiss() {
  return useMutation<void>("atom_dismiss");
}

export function useAtomRestore() {
  return useMutation<KnowledgeAtomResponse>("atom_restore");
}
```

- [ ] **Step 2: Commit**

```
feat(desktop-ui): add useKnowledgeAtoms hooks
```

---

### Task 12: Frontend — KnowledgeAtomsPanel + AtomCard

**Files:**
- Create: `desktop-ui/src/features/notes/components/KnowledgeAtomsPanel.tsx`
- Create: `desktop-ui/src/features/notes/components/AtomCard.tsx`

- [ ] **Step 1: Create AtomCard component**

`AtomCard.tsx` — renders a single atom card with:
- Subject (bold), source context snippet (muted, truncated)
- Retention % badge (green >80%, amber 50-80%, red <50%)
- For active atoms: "Review" button
- For suggested atoms: dimmed styling, "Accept" and dismiss "X" buttons
- Click on active atom emits `AtomInteracted` via IPC

- [ ] **Step 2: Create KnowledgeAtomsPanel component**

`KnowledgeAtomsPanel.tsx` — the right-panel section:
- Uses `useKnowledgeAtoms(noteId)` hook
- Header: "KNOWLEDGE ATOMS (N)" + "Accept all (M)" button (if suggested atoms exist)
- Active atoms list (AtomCard components)
- Separator + "Suggested" label
- Suggested atoms list (dimmed AtomCards)
- Accept all calls `atom_accept` for each suggested atom + invalidates queries

- [ ] **Step 3: Commit**

```
feat(desktop-ui): add KnowledgeAtomsPanel + AtomCard components
```

---

### Task 13: Frontend — Inline quick review

**Files:**
- Create: `desktop-ui/src/features/notes/components/InlineReview.tsx`
- Modify: `desktop-ui/src/features/notes/components/AtomCard.tsx`

- [ ] **Step 1: Create InlineReview component**

`InlineReview.tsx` — embedded flashcard review:
- Fetches next card via `ipc("atom_next_card", { atomId })`
- Reuses card rendering logic from `desktop-ui/src/features/learn/components/CardRenderer.tsx` (import the VocabularyCard/BasicCard components)
- Shows source context snippet below the card
- Rating buttons call `ipc("flashcard_record_review", { cardId, quality, recallSpeedMs })`
- After rating: invalidate atoms query (retention updated), collapse back to AtomCard

- [ ] **Step 2: Wire into AtomCard**

In AtomCard, the "Review" button toggles an `isReviewing` state. When true, render `<InlineReview atomId={atom.id} onDone={() => setIsReviewing(false)} />` instead of the normal card content.

- [ ] **Step 3: Commit**

```
feat(desktop-ui): add InlineReview for in-panel flashcard review
```

---

### Task 14: Frontend — Wire KnowledgeAtomsPanel into NoteEditor

**Files:**
- Modify: `desktop-ui/src/features/notes/components/NoteEditor.tsx`

- [ ] **Step 1: Add KnowledgeAtomsPanel to the right panel area**

In `NoteEditor.tsx`, import `KnowledgeAtomsPanel` and render it in the right panel. The exact placement depends on the current mode:
- In split/translate mode: add as a section inside the `SplitEditor` right pane (between Translation and Words sections)
- In single mode: the right sidebar (where AI Suggestions, Backlinks, etc. live) gets a new collapsible section

For the initial implementation, add it to the right sidebar in single mode as a new section above "AI SUGGESTIONS". In translate mode, it will naturally show alongside the translation panel.

Pass `noteId={note.id}` as prop.

- [ ] **Step 2: Lint check**

Run: `cd desktop-ui && bun run lint 2>&1 | grep -E "atoms|Atom|knowledge" | head -10`
Expected: no errors in new files

- [ ] **Step 3: Commit**

```
feat(desktop-ui): wire KnowledgeAtomsPanel into note editor
```

---

### Task 15: Launcher command — knowledge explorer

**Files:**
- Modify: `desktop-ui/src/features/tray/` (or appropriate launcher integration point)

- [ ] **Step 1: Add "knowledge" launcher command**

Register a launcher command `knowledge` (or `atoms`) that opens a basic list view of all active atoms grouped by topic. This is a simple searchable list — not the full dashboard (that's Phase 4). Show: topic name, atom count, avg retention per topic. Click topic → expand to show atoms.

- [ ] **Step 2: Commit**

```
feat(desktop-ui): add knowledge launcher command (basic explorer)
```

---

### Task 16: Migration toast + final integration test

**Files:**
- Modify: `desktop-ui/src/App.tsx` or appropriate init location
- Create: new IPC command `atoms_migration_status` in `crates/app-core/src/handlers/atoms.rs`

- [ ] **Step 1: Add atoms_migration_status IPC command**

Add a method to AppCore that returns `{ migrated: bool, count: usize }` by checking if the migration sentinel row exists and counting atoms. Register as a Tauri command + dev server dispatch alongside the other atom commands.

- [ ] **Step 2: Show migration toast on first load**

In the frontend startup flow, call `ipc("atoms_migration_status")`. If `migrated && count > 0`, show toast:
"Your knowledge graph is live — N atoms created from your vocabulary. [Open a note to explore] [Show my strongest topics]"
- "Open a note" navigates to `/notes`
- "Show my strongest topics" opens the launcher `knowledge` command

- [ ] **Step 2: End-to-end manual test**

1. Start the app: `cargo tauri dev`
2. Open browser at `localhost:1420`
3. Verify migration toast appears (if you have existing vocab)
4. Open a note with existing vocabulary translations
5. Verify Knowledge Atoms section appears in right panel
6. Click "Review" on an atom → inline flashcard appears
7. Rate the card → retention % updates
8. Right-click in editor → "Translate to..." → pick a language → atoms should appear for new vocab

- [ ] **Step 3: Commit**

```
feat: Knowledge Atoms Phase 1 complete — migration toast + integration
```

---

## Verification Checklist

After all tasks complete:

- [ ] `cargo nextest run --workspace` — all tests pass
- [ ] `cargo clippy --workspace --all-targets --all-features` — zero warnings
- [ ] `cargo fmt --all --check` — formatted
- [ ] `cd desktop-ui && bun run lint` — no new errors in changed files
- [ ] Dev server parity test passes
- [ ] Manual test: vocab save creates atoms, atoms show in panel, inline review works
