# Insight Review (Phase 2) Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the Insight Review feature — a 4-tab AI-powered research companion (Synthesis, Gap Analysis, Self-Assessment, Concept Map) that expands from the notes context panel, with flashcard persistence and FSRS scheduling.

**Architecture:** Backend adds FlashcardRepo + InsightCacheRepo (cognitive L5), an insight handler in app-core (L7) that uses the shared InsightForge for context assembly + LLM calls + Tauri event streaming, IPC DTOs in desktop-shared, and 4 Tauri commands in desktop. Frontend adds InsightReviewPanel with 4 tab components, a streaming hook, and mermaid rendering — all using the existing glassmorphism design system.

**Tech Stack:** Rust (SQLite, tokio, Tauri 2 events), React 19, TypeScript, react-markdown + remark-gfm + rehype-highlight, mermaid.js (new dep), Tailwind v4 + CSS tokens, Biome 2.0.

**Spec:** `docs/superpowers/specs/2026-03-16-insight-review-design.md` (v2)

**Depends on:** Phase 0+1 (complete) — InsightForge module, EntityRepo, PersonaRepo, ContextEngine integration.

**Phase 2 scope (from spec):** Tabs 1-4 only. Tab 5 (Perspectives), Scenario Challenges, Cross-Session Tracking, and Persona Management UI are Phase 4-6.

---

## File Structure

### New files (backend)

| File | Responsibility |
|------|---------------|
| `crates/cognitive/src/repos/flashcard.rs` | FlashcardRepo: CRUD, FSRS scheduling, deck management |
| `crates/cognitive/src/repos/insight_cache.rs` | InsightCacheRepo: per-tab caching with content hash invalidation |
| `crates/app-core/src/handlers/notes/insight.rs` | Insight handler: context assembly, LLM calls, streaming events, flashcard saving |

### New files (frontend)

| File | Responsibility |
|------|---------------|
| `desktop-ui/src/features/notes/components/InsightReviewPanel.tsx` | Main panel: header, tabs, content router, footer |
| `desktop-ui/src/features/notes/components/insight/SynthesisTab.tsx` | Streaming markdown renderer for Tab 1 |
| `desktop-ui/src/features/notes/components/insight/GapAnalysisTab.tsx` | Gap analysis markdown + Deep Dive buttons |
| `desktop-ui/src/features/notes/components/insight/SelfAssessmentTab.tsx` | Interactive quiz with cards, score, reveal |
| `desktop-ui/src/features/notes/components/insight/ConceptMapTab.tsx` | Mermaid mindmap renderer + text fallback |
| `desktop-ui/src/features/notes/components/insight/MermaidRenderer.tsx` | Mermaid.js wrapper with dark theme |
| `desktop-ui/src/features/notes/hooks/useInsightReview.ts` | State management, IPC calls, Tauri event streaming |

### Modified files

| File | Change |
|------|--------|
| `crates/cognitive/src/repos/mod.rs` | Register flashcard + insight_cache modules |
| `crates/cognitive/migrations/001_cognitive_tables.sql` | Append flashcards + insight_review_cache DDL |
| `crates/desktop-shared/src/commands/notes.rs` | Add Insight Review DTOs |
| `crates/desktop/src/commands/notes.rs` | Add 4 Tauri commands + DEV_COMMANDS |
| `crates/desktop/src/main.rs` | Register commands in generate_handler! |
| `crates/app-core/src/handlers/notes/mod.rs` | Register insight module |
| `crates/app-core/src/state.rs` | Add InsightCacheRepo + FlashcardRepo to AppCore |
| `desktop-ui/src/features/notes/components/AISuggestionsPanel.tsx` | Replace disabled Synthesize button |
| `desktop-ui/src/features/notes/components/ContextPanel.tsx` | Add insight mode width transition |
| `desktop-ui/src/features/notes/pages/KnowledgeBasePage.tsx` | Add Cmd+Shift+I shortcut + insight state |
| `desktop-ui/package.json` | Add mermaid dependency |

---

## Chunk 1: Database Repos

### Task 1: Flashcard + InsightCache Migration Schema

**Files:**
- Modify: `crates/cognitive/migrations/001_cognitive_tables.sql` (append DDL)
- Modify: `crates/cognitive/src/repos/mod.rs` (bump migration version)

- [ ] **Step 1: Append flashcard + insight_review_cache DDL**

Append to the end of `crates/cognitive/migrations/001_cognitive_tables.sql`:

```sql
-- ── Flashcards (FSRS spaced repetition) ─────────────────────────

CREATE TABLE IF NOT EXISTS flashcards (
    id TEXT PRIMARY KEY,
    source_note_id TEXT,
    source_session_id TEXT,
    insight_review_id TEXT,
    deck TEXT NOT NULL DEFAULT 'general',
    question TEXT NOT NULL,
    answer TEXT NOT NULL,
    card_type TEXT NOT NULL DEFAULT 'short_answer',
    choices JSON,
    stability REAL NOT NULL DEFAULT 1.0,
    difficulty REAL NOT NULL DEFAULT 0.5,
    due_at TEXT,
    last_reviewed_at TEXT,
    review_count INTEGER NOT NULL DEFAULT 0,
    lapses INTEGER NOT NULL DEFAULT 0,
    state TEXT NOT NULL DEFAULT 'new',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_flashcards_source_note ON flashcards(source_note_id);
CREATE INDEX IF NOT EXISTS idx_flashcards_due ON flashcards(due_at);
CREATE INDEX IF NOT EXISTS idx_flashcards_deck ON flashcards(deck);
CREATE INDEX IF NOT EXISTS idx_flashcards_insight ON flashcards(insight_review_id);

-- ── Insight Review Cache ────────────────────────────────────────

CREATE TABLE IF NOT EXISTS insight_review_cache (
    id TEXT PRIMARY KEY,
    note_id TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    synthesis TEXT,
    gap_analysis TEXT,
    self_assessment TEXT,
    concept_map TEXT,
    perspectives TEXT,
    persona_ids JSON,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(note_id, content_hash)
);

CREATE INDEX IF NOT EXISTS idx_insight_cache_note ON insight_review_cache(note_id);
```

- [ ] **Step 2: Bump migration version**

In `crates/cognitive/src/repos/mod.rs`, change `version: 2` to `version: 3` in the `cognitive_migrations()` function (bumped to 2 in Phase 0).

- [ ] **Step 3: Verify**

Run: `cargo build -p cognitive`
Expected: compiles.

- [ ] **Step 4: Commit**

```bash
git add crates/cognitive/migrations/001_cognitive_tables.sql crates/cognitive/src/repos/mod.rs
git commit -m "feat(cognitive): add flashcards and insight_review_cache schema"
```

---

### Task 2: FlashcardRepo

**Files:**
- Create: `crates/cognitive/src/repos/flashcard.rs`
- Modify: `crates/cognitive/src/repos/mod.rs` (register module)

- [ ] **Step 1: Create FlashcardRepo with types, implementation, and tests**

Create `crates/cognitive/src/repos/flashcard.rs`. The repo needs:

**Types:**
- `CardType` enum: `MultipleChoice`, `ShortAnswer` (serde rename to snake_case strings)
- `ReviewQuality` enum: `Again`, `Hard`, `Good`, `Easy` with `compute_new_stability()` method
- `NewFlashcard` struct: source_note_id, insight_review_id, deck, question, answer, card_type, choices, stability, difficulty
- `FlashcardRow` (sqlx::FromRow): all table columns
- `DeckSummary`: name, card_count, due_count

**Key implementation details:**
- `create_batch`: insert each card with `due_at = now` (immediately due), `state = "new"`
- `get_due_cards`: `WHERE deck = ?1 AND (due_at IS NULL OR due_at <= now) ORDER BY due_at ASC LIMIT ?`
- `record_review`: update stability using ReviewQuality:
  - `Again`: halve stability (min 0.1), increment lapses, state="relearning"
  - `Hard`: keep current stability
  - `Good`: use `crate::services::decay::update_stability(current, true, 90.0)`
  - `Easy`: 1.3x the FSRS growth (`current + (grown - current) * 1.3`)
  - Compute `next_due = now + stability_days * 86400 seconds`
- `list_by_note`: filter by source_note_id
- `list_decks`: GROUP BY deck with card_count and due_count
- `delete_deck`: DELETE WHERE deck = ?

**Tests (4):**
1. `test_create_batch_and_list_by_note` — create 2 cards, verify list_by_note returns them
2. `test_get_due_cards` — create card, verify it's immediately due
3. `test_record_review_updates_fsrs` — review with Good, verify stability increased and state changed
4. `test_list_decks` — create 2 cards in same deck, verify count

- [ ] **Step 2: Register module**

In `crates/cognitive/src/repos/mod.rs`, add:
```rust
pub mod flashcard;
pub use flashcard::{FlashcardRepo, FlashcardRow, NewFlashcard, ReviewQuality, DeckSummary, CardType};
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p cognitive -E 'test(flashcard)'`
Expected: all 4 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/cognitive/src/repos/flashcard.rs crates/cognitive/src/repos/mod.rs
git commit -m "feat(cognitive): add FlashcardRepo with FSRS scheduling"
```

---

### Task 3: InsightCacheRepo

**Files:**
- Create: `crates/cognitive/src/repos/insight_cache.rs`
- Modify: `crates/cognitive/src/repos/mod.rs` (register module)

- [ ] **Step 1: Create InsightCacheRepo**

Types:
- `InsightCacheRow` (sqlx::FromRow): all columns
- `InsightCacheRepo` with pool

Methods:
- `get(note_id)` → most recent cache entry for note
- `get_if_fresh(note_id, content_hash)` → cache entry only if hash matches
- `upsert(note_id, content_hash, synthesis?, gap_analysis?, self_assessment?, concept_map?)` → INSERT ON CONFLICT UPDATE (coalesce nulls)
- `update_tab(note_id, content_hash, tab_name, content)` → update single tab column

Tests (2):
1. `test_upsert_and_get` — upsert partial, verify fields
2. `test_update_tab` — upsert then update single tab

- [ ] **Step 2: Register module**

```rust
pub mod insight_cache;
pub use insight_cache::{InsightCacheRepo, InsightCacheRow};
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p cognitive -E 'test(insight_cache)'`
Expected: both tests pass.

- [ ] **Step 4: Run all cognitive tests + clippy**

Run: `cargo nextest run -p cognitive && cargo clippy -p cognitive --all-targets`
Expected: all pass, 0 warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/src/repos/insight_cache.rs crates/cognitive/src/repos/mod.rs
git commit -m "feat(cognitive): add InsightCacheRepo for insight review caching"
```

---

## Chunk 2: Backend Handler + IPC

### Task 4: Shared IPC DTOs

**Files:**
- Modify: `crates/desktop-shared/src/commands/notes.rs` (append after BacklinkResponse)

- [ ] **Step 1: Add Insight Review DTOs**

Append to the end of `crates/desktop-shared/src/commands/notes.rs`:

```rust
// ── Insight Review ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InsightReviewStarted {
    pub insight_review_id: String,
    pub content_hash: String,
    pub cached: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InsightReviewResponse {
    pub insight_review_id: String,
    pub note_id: String,
    pub synthesis: Option<String>,
    pub gap_analysis: Option<String>,
    pub self_assessment: Option<Vec<QuizQuestion>>,
    pub concept_map: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuizQuestion {
    pub id: String,
    #[serde(rename = "type")]
    pub question_type: String,
    pub question: String,
    pub choices: Option<Vec<String>>,
    pub correct_answer: String,
    pub explanation: String,
    pub source_notes: Vec<String>,
    pub difficulty: String,
    pub difficulty_score: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TabContent {
    pub tab: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlashcardResponse {
    pub id: String,
    pub deck: String,
    pub question: String,
    pub answer: String,
    pub card_type: String,
    pub choices: Option<serde_json::Value>,
    pub stability: f64,
    pub difficulty: f64,
    pub due_at: Option<String>,
    pub state: String,
    pub review_count: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InsightSaveFlashcardsParams {
    pub note_id: String,
    pub insight_review_id: String,
    pub deck_name: String,
    pub questions: Vec<QuizQuestion>,
}
```

- [ ] **Step 2: Verify**

Run: `cargo build -p desktop-shared`
Expected: compiles.

- [ ] **Step 3: Commit**

```bash
git add crates/desktop-shared/src/commands/notes.rs
git commit -m "feat(desktop-shared): add Insight Review IPC DTOs"
```

---

### Task 5: App-Core Insight Handler

**Files:**
- Create: `crates/app-core/src/handlers/notes/insight.rs`
- Modify: `crates/app-core/src/handlers/notes/mod.rs` (register module)
- Modify: `crates/app-core/src/state.rs` (add repos to AppCore)

This is the most complex task. The handler:
1. Loads the note + computes content_hash (`SHA-256(title + body + sorted_related_note_ids)`)
2. Checks insight_review_cache — if hit, returns `InsightReviewStarted { cached: true }`
3. If miss, assembles context using InsightForge + note-specific data
4. Spawns a background task that:
   - Streams Tab 1 (Synthesis) via Tauri events (`insight:synthesis-chunk`, `insight:synthesis-done`)
   - Fires Tabs 2-4 in parallel as structured JSON calls
   - Emits `insight:tab-done` for each completed tab
   - Caches all results
5. Returns `InsightReviewStarted { cached: false, insightReviewId, contentHash }`

**Note on LLM calls:** The handler needs access to the LLM provider. Follow the existing pattern in `crates/app-core/src/handlers/notes/suggestions.rs` which calls the agent for note suggestions. The insight handler will use a similar pattern — either calling the agent directly or using the LLM provider from AppCore.

- [ ] **Step 1: Add repos to AppCore state**

In `crates/app-core/src/state.rs`, add fields to the `AppCore` struct (around line 79-96 where optional cognitive repos live):

```rust
pub insight_cache_repo: Option<cognitive::InsightCacheRepo>,
pub flashcard_repo: Option<cognitive::FlashcardRepo>,
```

Initialize them during app init (in the cognitive init path where other repos are created).

- [ ] **Step 2: Create insight handler stub**

Create `crates/app-core/src/handlers/notes/insight.rs` with the handler signatures:

```rust
use common::Result;
use sha2::{Digest, Sha256};

use crate::state::AppCore;
use desktop_shared::commands::notes::*;

impl AppCore {
    /// Start insight review: check cache, spawn LLM tasks, return initial response.
    pub async fn note_insight_review(
        &self,
        note_id: &str,
    ) -> Result<InsightReviewStarted> {
        let note = self.note_repo.get(note_id).await?
            .ok_or_else(|| common::KlyntbotError::not_found("Note not found"))?;

        if note.body.as_deref().map_or(true, |b| b.trim().is_empty()) {
            return Err(common::KlyntbotError::validation("Note has no content"));
        }

        // Compute content hash
        let related_ids = self.get_related_note_ids(note_id).await?;
        let hash_input = format!(
            "{}{}{}",
            note.title,
            note.body.as_deref().unwrap_or(""),
            related_ids.join(",")
        );
        let content_hash = format!("{:x}", Sha256::digest(hash_input.as_bytes()));

        // Check cache
        if let Some(ref repo) = self.insight_cache_repo {
            if let Some(_cached) = repo.get_if_fresh(note_id, &content_hash).await? {
                return Ok(InsightReviewStarted {
                    insight_review_id: format!("ir-{}", nanoid::nanoid!(10)),
                    content_hash,
                    cached: true,
                });
            }
        }

        let insight_review_id = format!("ir-{}", nanoid::nanoid!(10));

        // TODO: Spawn background task for LLM calls + streaming events
        // For now, return uncached response — LLM integration in a follow-up task

        Ok(InsightReviewStarted {
            insight_review_id,
            content_hash,
            cached: false,
        })
    }

    /// Get cached insight review for instant re-open.
    pub async fn note_insight_cache_get(
        &self,
        note_id: &str,
    ) -> Result<Option<InsightReviewResponse>> {
        let repo = match &self.insight_cache_repo {
            Some(r) => r,
            None => return Ok(None),
        };

        let cached = match repo.get(note_id).await? {
            Some(c) => c,
            None => return Ok(None),
        };

        let self_assessment: Option<Vec<QuizQuestion>> = cached
            .self_assessment
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok());

        Ok(Some(InsightReviewResponse {
            insight_review_id: cached.id.clone(),
            note_id: cached.note_id,
            synthesis: cached.synthesis,
            gap_analysis: cached.gap_analysis,
            self_assessment,
            concept_map: cached.concept_map,
        }))
    }

    /// Save quiz questions as flashcards with FSRS init.
    pub async fn insight_save_flashcards(
        &self,
        params: InsightSaveFlashcardsParams,
    ) -> Result<Vec<FlashcardResponse>> {
        let repo = self.flashcard_repo.as_ref()
            .ok_or_else(|| common::KlyntbotError::internal("Flashcard repo not available"))?;

        let cards: Vec<cognitive::NewFlashcard> = params.questions.iter().map(|q| {
            let (stability, difficulty) = match q.difficulty.as_str() {
                "easy" => (4.0, 0.3),
                "hard" => (0.8, 0.7),
                _ => (2.0, 0.5), // medium
            };
            cognitive::NewFlashcard {
                source_note_id: Some(params.note_id.clone()),
                insight_review_id: Some(params.insight_review_id.clone()),
                deck: params.deck_name.clone(),
                question: q.question.clone(),
                answer: q.correct_answer.clone(),
                card_type: if q.question_type == "multiple_choice" {
                    cognitive::CardType::MultipleChoice
                } else {
                    cognitive::CardType::ShortAnswer
                },
                choices: q.choices.as_ref().map(|c| serde_json::json!(c)),
                stability,
                difficulty,
            }
        }).collect();

        let rows = repo.create_batch(cards).await?;

        Ok(rows.into_iter().map(|r| FlashcardResponse {
            id: r.id,
            deck: r.deck,
            question: r.question,
            answer: r.answer,
            card_type: r.card_type,
            choices: r.choices.and_then(|s| serde_json::from_str(&s).ok()),
            stability: r.stability,
            difficulty: r.difficulty,
            due_at: r.due_at,
            state: r.state,
            review_count: r.review_count,
            created_at: r.created_at,
        }).collect())
    }

    /// Regenerate a single tab.
    pub async fn note_insight_regenerate_tab(
        &self,
        note_id: &str,
        tab: &str,
    ) -> Result<TabContent> {
        // TODO: Re-run single LLM call, update cache
        Ok(TabContent {
            tab: tab.to_string(),
            content: String::new(),
        })
    }

    // Helper: get sorted related note IDs for cache hash
    async fn get_related_note_ids(&self, note_id: &str) -> Result<Vec<String>> {
        // Use note suggestions or backlinks to find related notes
        let backlinks = self.note_repo.get_backlinks(note_id).await?;
        let mut ids: Vec<String> = backlinks.into_iter().map(|b| b.id).collect();
        ids.sort();
        Ok(ids)
    }
}
```

- [ ] **Step 3: Register insight module**

In `crates/app-core/src/handlers/notes/mod.rs`, add:
```rust
mod insight;
```

- [ ] **Step 4: Add `sha2` and `nanoid` dependencies**

Run: `cargo add sha2 nanoid -p app-core`

- [ ] **Step 5: Build**

Run: `cargo build -p app-core`
Expected: compiles (the LLM streaming is TODO'd — we're building the handler skeleton first).

- [ ] **Step 6: Commit**

```bash
git add crates/app-core/src/handlers/notes/insight.rs crates/app-core/src/handlers/notes/mod.rs crates/app-core/src/state.rs crates/app-core/Cargo.toml
git commit -m "feat(app-core): add insight review handler with cache + flashcard saving"
```

---

### Task 6: Tauri IPC Commands

**Files:**
- Modify: `crates/desktop/src/commands/notes.rs` (add commands + DEV_COMMANDS)
- Modify: `crates/desktop/src/main.rs` (register in generate_handler!)

- [ ] **Step 1: Add 4 Tauri command functions**

In `crates/desktop/src/commands/notes.rs`, add after the existing note commands:

```rust
#[tauri::command]
pub async fn note_insight_review(
    state: tauri::State<'_, AppState>,
    note_id: String,
) -> Result<desktop_shared::commands::notes::InsightReviewStarted, String> {
    state.core.note_insight_review(&note_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn note_insight_cache_get(
    state: tauri::State<'_, AppState>,
    note_id: String,
) -> Result<Option<desktop_shared::commands::notes::InsightReviewResponse>, String> {
    state.core.note_insight_cache_get(&note_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn note_insight_save_flashcards(
    state: tauri::State<'_, AppState>,
    params: desktop_shared::commands::notes::InsightSaveFlashcardsParams,
) -> Result<Vec<desktop_shared::commands::notes::FlashcardResponse>, String> {
    state.core.insight_save_flashcards(params).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn note_insight_regenerate_tab(
    state: tauri::State<'_, AppState>,
    note_id: String,
    tab: String,
) -> Result<desktop_shared::commands::notes::TabContent, String> {
    state.core.note_insight_regenerate_tab(&note_id, &tab).await.map_err(|e| e.to_string())
}
```

- [ ] **Step 2: Update DEV_COMMANDS**

Add to the `DEV_COMMANDS` const array:
```rust
"note_insight_review",
"note_insight_cache_get",
"note_insight_save_flashcards",
"note_insight_regenerate_tab",
```

- [ ] **Step 3: Update dispatch_dev**

Add match arms in the `dispatch_dev()` function for each new command.

- [ ] **Step 4: Register in main.rs**

In `crates/desktop/src/main.rs`, add to `generate_handler![]`:
```rust
notes::note_insight_review,
notes::note_insight_cache_get,
notes::note_insight_save_flashcards,
notes::note_insight_regenerate_tab,
```

- [ ] **Step 5: Build workspace**

Run: `cargo build --workspace`
Expected: compiles.

- [ ] **Step 6: Run dev_server test**

Run: `cargo nextest run -p klyntbot-server -E 'test(dev_server)'`
Expected: `dev_server_covers_all_tauri_commands` test passes (verifies DEV_COMMANDS is complete).

- [ ] **Step 7: Commit**

```bash
git add crates/desktop/src/commands/notes.rs crates/desktop/src/main.rs
git commit -m "feat(desktop): add 4 Insight Review Tauri IPC commands"
```

---

## Chunk 3: Frontend Hook + Main Panel

### Task 7: Install Mermaid Dependency

**Files:**
- Modify: `desktop-ui/package.json`

- [ ] **Step 1: Add mermaid**

```bash
cd desktop-ui && bun add mermaid
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/package.json desktop-ui/bun.lock
git commit -m "feat(desktop-ui): add mermaid.js for concept map rendering"
```

---

### Task 8: useInsightReview Hook

**Files:**
- Create: `desktop-ui/src/features/notes/hooks/useInsightReview.ts`

- [ ] **Step 1: Create the hook**

This hook manages all Insight Review state: opening/closing, tab switching, streaming, quiz state, and IPC calls. Follow the patterns from `useQuery.ts` and `useEvent.ts`.

Key state shape:
```typescript
type TabStatus = "idle" | "streaming" | "loading" | "done" | "error";

interface InsightReviewState {
  isOpen: boolean;
  noteId: string | null;
  insightReviewId: string | null;
  contentHash: string | null;
  activeTab: "synthesis" | "gaps" | "assessment" | "concept-map";
  tabs: {
    synthesis: { status: TabStatus; content: string };
    gaps: { status: TabStatus; content: string };
    assessment: { status: TabStatus; questions: QuizQuestion[] };
    conceptMap: { status: TabStatus; mermaid: string; fallbackText: string };
  };
  quizState: {
    answers: Record<string, string>;
    revealed: Set<string>;
    score: number;
    total: number;
  };
}
```

Key behaviors:
- `open(noteId)` → calls `ipc("note_insight_review", { noteId })`, sets up event listeners
- `close()` → resets state, removes event listeners
- `switchTab(tab)` → updates activeTab
- `regenerateTab(tab)` → calls `ipc("note_insight_regenerate_tab", { noteId, tab })`
- `saveFlashcards(deckName)` → calls `ipc("note_insight_save_flashcards", { ... })`

Event listeners (via `useEvent`):
- `insight:synthesis-chunk` → append to synthesis.content (streaming)
- `insight:synthesis-done` → set synthesis.status = "done"
- `insight:tab-done` → parse content, set tab.status = "done"
- `insight:error` → set tab.status = "error"

When `cached: true`, immediately call `ipc("note_insight_cache_get", { noteId })` and populate all tabs.

- [ ] **Step 2: Lint**

```bash
cd desktop-ui && bun run lint:fix
```

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/notes/hooks/useInsightReview.ts
git commit -m "feat(desktop-ui): add useInsightReview hook with streaming support"
```

---

### Task 9: InsightReviewPanel (Main Component)

**Files:**
- Create: `desktop-ui/src/features/notes/components/InsightReviewPanel.tsx`
- Modify: `desktop-ui/src/features/notes/components/ContextPanel.tsx` (add insight mode)
- Modify: `desktop-ui/src/features/notes/components/AISuggestionsPanel.tsx` (replace button)
- Modify: `desktop-ui/src/features/notes/pages/KnowledgeBasePage.tsx` (add shortcut + state)

- [ ] **Step 1: Create InsightReviewPanel**

Component structure:
```
InsightReviewPanel
  ├── Header: title "Insight Review", close X button, "Regenerate All" button
  ├── Tab bar: 4 tabs with status dots (dim/pulsing-purple/green/red)
  ├── Content area (scrollable): renders active tab component
  └── Footer: Insert into note, Create note, Save as Deck, Copy buttons
```

Use `glass-panel` for the panel background. Tab bar uses `bg-white/[0.06]` for active state.

Status dots:
- idle: `bg-white/10 w-1.5 h-1.5 rounded-full`
- loading/streaming: `bg-purple-400 w-1.5 h-1.5 rounded-full animate-pulse`
- done: `bg-green-400 w-1.5 h-1.5 rounded-full`
- error: `bg-red-400 w-1.5 h-1.5 rounded-full`

Props: `{ noteId, state (from useInsightReview), onClose }`

- [ ] **Step 2: Modify ContextPanel for insight mode**

In `ContextPanel.tsx`, accept an `insightOpen` prop. When true:
- Render `InsightReviewPanel` instead of normal sections
- Parent controls the width transition (280px ↔ 640px)

- [ ] **Step 3: Modify KnowledgeBasePage**

Add `insightOpen` state. When insight opens, set `rightWidth = 640` with CSS transition. Add keyboard shortcut handler for `Cmd+Shift+I`.

- [ ] **Step 4: Replace Synthesize button in AISuggestionsPanel**

Replace the disabled Synthesize button (lines 137-144) with an active "Insight Review" button:

```tsx
<button
  type="button"
  onClick={() => onOpenInsight?.()}
  disabled={!noteId || !noteBody?.trim()}
  className="flex items-center gap-1 text-[10px] px-2 py-1 rounded-md bg-white/[0.04] text-secondary hover:bg-white/[0.08] hover:text-primary transition-colors disabled:text-dim disabled:cursor-not-allowed"
>
  <Brain size={10} />
  Insight Review
</button>
```

- [ ] **Step 5: Lint + verify**

```bash
cd desktop-ui && bun run lint:fix && bun run build
```

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/notes/
git commit -m "feat(desktop-ui): add InsightReviewPanel with tab bar and panel transition"
```

---

## Chunk 4: Tab Components

### Task 10: SynthesisTab + GapAnalysisTab

**Files:**
- Create: `desktop-ui/src/features/notes/components/insight/SynthesisTab.tsx`
- Create: `desktop-ui/src/features/notes/components/insight/GapAnalysisTab.tsx`

- [ ] **Step 1: SynthesisTab**

Renders streaming markdown. When `status === "streaming"`, show content with a blinking cursor. When `status === "done"`, show final content. Uses `MarkdownContent` from the chat feature (import from `../../chat/components/MarkdownContent`).

Skeleton loader for `status === "loading"`: 3 animated `bg-white/[0.04]` bars.

- [ ] **Step 2: GapAnalysisTab**

Same markdown rendering as SynthesisTab. Additionally, parse the JSON block at the end of the gap analysis content (the `gaps` array). For each gap, render a "Create Note" button that calls the Deep Dive handler.

- [ ] **Step 3: Lint + commit**

```bash
cd desktop-ui && bun run lint:fix
git add desktop-ui/src/features/notes/components/insight/
git commit -m "feat(desktop-ui): add SynthesisTab and GapAnalysisTab components"
```

---

### Task 11: SelfAssessmentTab (Quiz)

**Files:**
- Create: `desktop-ui/src/features/notes/components/insight/SelfAssessmentTab.tsx`

- [ ] **Step 1: Create quiz component**

Renders questions from `state.tabs.assessment.questions`. Each question is a card (`glass-card`):

**Multiple choice:**
- 4 option buttons (A/B/C/D)
- Click selects answer, stores in `quizState.answers[questionId]`
- "Check" button → reveals correct/incorrect, adds to `quizState.revealed`
- Correct: green border. Incorrect: red border + show correct answer.

**Short answer:**
- Text input field
- "Check" button → reveals model answer below

**Score display:**
- Running tally: `{correct} / {attempted}`
- Updates as questions are revealed

**"Reveal All Answers" button:**
- Appears after 3+ individual reveals
- Reveals all remaining answers at once

**"Save as Flashcard Deck" button:**
- Appears after 50%+ quiz answered
- Pulses with brand color when ready
- Calls `saveFlashcards(deckName)`

- [ ] **Step 2: Lint + commit**

```bash
cd desktop-ui && bun run lint:fix
git add desktop-ui/src/features/notes/components/insight/SelfAssessmentTab.tsx
git commit -m "feat(desktop-ui): add SelfAssessmentTab with interactive quiz"
```

---

### Task 12: ConceptMapTab + MermaidRenderer

**Files:**
- Create: `desktop-ui/src/features/notes/components/insight/ConceptMapTab.tsx`
- Create: `desktop-ui/src/features/notes/components/insight/MermaidRenderer.tsx`

- [ ] **Step 1: MermaidRenderer**

Wrapper for mermaid.js. Renders a mermaid diagram string into SVG. Dark theme configuration matching the glassmorphism design:

```typescript
import mermaid from "mermaid";

mermaid.initialize({
  startOnLoad: false,
  theme: "dark",
  themeVariables: {
    primaryColor: "rgba(249, 115, 22, 0.3)",  // --brand with alpha
    primaryTextColor: "#f0f2f5",               // --text-primary
    primaryBorderColor: "rgba(255, 255, 255, 0.12)", // --glass-border
    lineColor: "rgba(255, 255, 255, 0.2)",
    secondaryColor: "rgba(255, 255, 255, 0.06)",
    tertiaryColor: "rgba(255, 255, 255, 0.04)",
  },
});
```

Use `useEffect` + `useRef` to render: `mermaid.render(id, code)` returns SVG string, set as `innerHTML`. Handle parse errors gracefully (return null → ConceptMapTab shows fallback).

- [ ] **Step 2: ConceptMapTab**

If mermaid content starts with "FALLBACK:", render as indented text outline instead. Otherwise render via MermaidRenderer. Include "Copy Mermaid code" button.

- [ ] **Step 3: Lint + build**

```bash
cd desktop-ui && bun run lint:fix && bun run build
```

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/notes/components/insight/
git commit -m "feat(desktop-ui): add ConceptMapTab with MermaidRenderer"
```

---

### Task 13: Footer Actions

**Files:**
- Modify: `desktop-ui/src/features/notes/components/InsightReviewPanel.tsx` (add footer)

- [ ] **Step 1: Implement footer action buttons**

4 buttons in the footer (`border-t border-border` with `glass-button` styling):

| Button | Icon | Action |
|--------|------|--------|
| Insert into note | `FileInput` | Appends `## Insight Review — {date}` + active tab content to note body |
| Create note | `FilePlus` | Creates "Insight: {Note Title}" via `ipc("note_create", ...)` |
| Save as Deck | `BookOpen` | Enabled after 50%+ quiz answered. Calls `saveFlashcards()` |
| Copy | `Copy` | Copies active tab content (markdown or mermaid source) to clipboard |

- [ ] **Step 2: Lint + commit**

```bash
cd desktop-ui && bun run lint:fix
git add desktop-ui/src/features/notes/components/InsightReviewPanel.tsx
git commit -m "feat(desktop-ui): add Insight Review footer actions"
```

---

### Task 14: Final Verification

- [ ] **Step 1: Full backend build + tests**

Run: `cargo build --workspace && cargo nextest run --workspace`
Expected: all tests pass.

- [ ] **Step 2: Frontend build**

Run: `cd desktop-ui && bun run build`
Expected: builds successfully.

- [ ] **Step 3: Frontend lint**

Run: `cd desktop-ui && bun run lint:fix`
Expected: no errors.

- [ ] **Step 4: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings (except pre-existing desktop exceptions).

- [ ] **Step 5: Manual smoke test**

Run: `cargo tauri dev`
- Open a note with content
- Click "Insight Review" button in the right panel
- Panel should expand to 640px
- Tabs should show loading state
- Cmd+Shift+I should toggle the panel
- Escape should close it

- [ ] **Step 6: Final commit if needed**

```bash
cargo fmt --all && cd desktop-ui && bun run lint:fix
git add -A && git commit -m "style: format Phase 2 implementation"
```
