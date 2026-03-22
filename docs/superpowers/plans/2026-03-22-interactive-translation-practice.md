# Interactive Translation Practice Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a sentence-by-sentence translation practice workspace with LLM evaluation, Quick Translate popup, and knowledge graph integration — turning every note into a personal language gym.

**Architecture:** Practice Mode is a new `splitMode="practice"` inside `SplitEditor`. The left pane renders `PracticeSourcePanel` (read-only, highlighted source), the right pane renders `PracticeDocPanel` (growing translated document), and a full-width `PracticeBottomBar` toggles between input and evaluation states. A `QuickTranslatePopup` on text selection serves as the primary entry point. Backend: new `practice_sessions` table + 7 IPC commands in `commands/practice.rs` + LLM prompts for segmentation and evaluation.

**Tech Stack:** Rust (Tauri 2, SQLite, LLM providers), React (TypeScript, Tailwind v4, Biome 2.0)

**Spec:** `docs/superpowers/specs/2026-03-22-interactive-translation-practice-design.md`

---

## File Structure

### New Rust Files
- `crates/desktop-shared/src/commands/practice.rs` — IPC request/response types
- `crates/desktop/src/commands/practice.rs` — Tauri command handlers + DEV_COMMANDS
- `crates/app-core/src/handlers/notes/practice.rs` — AppCore business logic
- `crates/app-core/src/handlers/notes/practice_prompts.rs` — LLM prompts (segmentation, evaluation, quick translate)
- `crates/feature-notes/migrations/002_practice_sessions.sql` — Migration SQL
- `crates/feature-notes/src/repo/practice.rs` — PracticeSessionRepo (follows existing `repo/` pattern)

### New Frontend Files
- `desktop-ui/src/features/notes/components/practice/PracticeMode.tsx` — Main container
- `desktop-ui/src/features/notes/components/practice/PracticePreview.tsx` — Segmentation preview overlay
- `desktop-ui/src/features/notes/components/practice/PracticeProgressHeader.tsx` — Single top bar (focus + progress + streak + exit)
- `desktop-ui/src/features/notes/components/practice/PracticeSourcePanel.tsx` — Left pane with highlighting
- `desktop-ui/src/features/notes/components/practice/PracticeDocPanel.tsx` — Right pane, clean document
- `desktop-ui/src/features/notes/components/practice/PracticeBottomBar.tsx` — Input ↔ Eval states
- `desktop-ui/src/features/notes/components/practice/PracticeSessionComplete.tsx` — Results + actions
- `desktop-ui/src/features/notes/components/practice/ConfidenceTap.tsx` — 1-5 stars widget
- `desktop-ui/src/features/notes/components/QuickTranslatePopup.tsx` — Floating glass popup
- `desktop-ui/src/features/notes/components/insight/PracticeHistoryTab.tsx` — Practice History tab
- `desktop-ui/src/features/notes/hooks/usePracticeSession.ts` — Session CRUD hook
- `desktop-ui/src/features/notes/hooks/useSmartSegmentation.ts` — Segmentation + cache hook
- `desktop-ui/src/features/notes/hooks/usePracticeEvaluation.ts` — Per-unit eval hook
- `desktop-ui/src/features/notes/hooks/useQuickTranslate.ts` — Text selection + popup hook

### Modified Rust Files
- `crates/desktop-shared/src/commands/mod.rs` — Add `mod practice; pub use practice::*;`
- `crates/desktop/src/commands/mod.rs` — Add `pub mod practice;`
- `crates/desktop/src/main.rs` — Register practice commands with Tauri
- `crates/desktop/src/dev_server/mod.rs` — Add practice DEV_COMMANDS to coverage list
- `crates/bus/src/domain_events.rs` — Add `PracticeUnitCompleted`, `PracticeSessionCompleted`
- `crates/feature-notes/src/lib.rs` — Add practice migration to `migrations_static()`
- `crates/app-core/src/handlers/notes/mod.rs` — Add `pub mod practice;`
- `crates/app-core/src/handlers/notes/language.rs` — Add `language_quick_translate()` handler
- `crates/app-core/src/handlers/notes/language_prompts.rs` — Add `quick_translate_prompt()`
- `crates/desktop/src/commands/language.rs` — Add `language_quick_translate` command
- `crates/desktop-shared/src/commands/language.rs` — Add `QuickTranslateResponse` type

### Modified Frontend Files
- `desktop-ui/src/features/notes/components/editor/SplitEditor.tsx` — Add `"practice"` to `SplitMode`, render `PracticeMode`, skip content persistence
- `desktop-ui/src/features/notes/components/NoteEditor.tsx` — Add text selection listener for popup
- `desktop-ui/src/features/notes/components/editor/LanguageLearningPanel.tsx` — Add "Turn this into active practice" footer button
- `desktop-ui/src/features/notes/components/InsightReviewPanel.tsx` — Add "Practice" tab
- `desktop-ui/src/features/notes/hooks/useInsightReview.ts` — Add `"practice"` to `TabId`

---

### Task 1: Database migration + PracticeSessionRepo

**Files:**
- Create: `crates/feature-notes/migrations/002_practice_sessions.sql`
- Create: `crates/feature-notes/src/repo/practice.rs`
- Modify: `crates/feature-notes/src/repo/mod.rs`
- Modify: `crates/feature-notes/src/lib.rs`

- [ ] **Step 1: Write the migration SQL**

Create `crates/feature-notes/migrations/002_practice_sessions.sql`:
```sql
CREATE TABLE IF NOT EXISTS practice_sessions (
    id                   TEXT PRIMARY KEY,
    note_id              TEXT NOT NULL,
    source_lang          TEXT NOT NULL,
    target_lang          TEXT NOT NULL,
    status               TEXT NOT NULL DEFAULT 'in_progress',
    segments             TEXT NOT NULL,
    current_index        INTEGER NOT NULL DEFAULT 0,
    results              TEXT NOT NULL DEFAULT '[]',
    user_translation_doc TEXT,
    average_score        REAL,
    started_at           TEXT NOT NULL,
    completed_at         TEXT,
    updated_at           TEXT NOT NULL DEFAULT (datetime('now')),
    created_at           TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_practice_sessions_note_id ON practice_sessions(note_id);
CREATE INDEX IF NOT EXISTS idx_practice_sessions_status ON practice_sessions(status);
```

- [ ] **Step 2: Register migration in feature-notes**

In `crates/feature-notes/src/lib.rs`, update `migrations_static()` to include the new migration. Bump the existing migration to keep it, then add a second entry:
```rust
pub fn migrations_static() -> Vec<FeatureMigration> {
    vec![
        FeatureMigration {
            feature_name: "notes".to_string(),
            version: 6,
            description: "Create notes core tables".to_string(),
            sql: Self::migration_sql().to_string(),
        },
        FeatureMigration {
            feature_name: "notes_practice".to_string(),
            version: 1,
            description: "Create practice_sessions table for translation practice".to_string(),
            sql: include_str!("../migrations/002_practice_sessions.sql").to_string(),
        },
    ]
}
```

- [ ] **Step 3: Create PracticeSessionRepo**

Create `crates/feature-notes/src/practice_repo.rs` with:
- `PracticeSessionRow` struct (mirrors SQL columns, all String/Option types)
- `PracticeSessionRepo` struct wrapping `SqlitePool`
- Methods: `create()`, `get_by_id()`, `get_active_for_note()`, `update_progress()`, `complete()`, `list_for_note()`, `mark_abandoned_stale()`
- `mark_abandoned_stale()`: UPDATE status='abandoned' WHERE status='in_progress' AND updated_at < datetime('now', '-7 days')
- All update methods (update_progress, complete) should SET updated_at = datetime('now')

Follow the pattern from `crates/feature-notes/src/repo/notes.rs` for SqlitePool usage and error handling.

- [ ] **Step 4: Export repo module**

Add `pub mod practice;` to `crates/feature-notes/src/repo/mod.rs` and re-export `PracticeSessionRepo`.

- [ ] **Step 5: Build and verify**

Run: `cargo build -p feature-notes`
Expected: Compiles with no errors.

- [ ] **Step 6: Commit**
```bash
git add crates/feature-notes/
git commit -m "feat(practice): add practice_sessions migration + PracticeSessionRepo"
```

---

### Task 2: IPC types in desktop-shared

**Files:**
- Create: `crates/desktop-shared/src/commands/practice.rs`
- Modify: `crates/desktop-shared/src/commands/mod.rs`
- Modify: `crates/desktop-shared/src/commands/language.rs`

- [ ] **Step 1: Create practice IPC types**

Create `crates/desktop-shared/src/commands/practice.rs` with all request/response types. Follow the pattern from `language.rs`: `#[derive(Debug, Deserialize)]` for params, `#[derive(Debug, Serialize, Deserialize)]` for responses, all with `#[serde(rename_all = "camelCase")]`.

Types needed:
- `PracticeSegmentParams { note_id, source_lang, target_lang }`
- `PracticeSegment { index: u32, text, segment_type, suggested_focus, skipped: bool }`
- `PracticeSegmentResponse { segments: Vec<PracticeSegment>, estimated_mins: u32, cached_at: Option<String> }`
- `PracticeStartParams { note_id, segments: Vec<PracticeSegment>, source_lang, target_lang, start_index: Option<u32> }`
- `PracticeSessionResponse { id, note_id, source_lang, target_lang, status, segments, current_index, results, user_translation_doc, average_score, started_at, completed_at }`
- `PracticeSubmitParams { session_id, index: u32, user_translation }`
- `PracticeEvalResponse { overall_grade, scores: PracticeScores, corrections: Vec<PracticeCorrection>, model_translation, encouragement, improvement_hint: Option<String> }`
- `PracticeScores { meaning, grammar, naturalness, word_choice }` (all String)
- `PracticeCorrection { original, suggested, explanation }`
- `PracticeConfirmParams { session_id, index: u32, final_translation, confidence_rating: u8, edited: bool }`
- `PracticeConfirmResponse { next_index: u32, is_complete: bool }`
- `PracticeGetParams { session_id: Option<String>, note_id: Option<String> }`
- `PracticeCompleteParams { session_id, save_to_sr: bool }`
- `PracticeCompleteResponse { average_score: f64, weak_unit_count: u32, flashcards_created: u32 }`

- [ ] **Step 2: Add QuickTranslateResponse to language.rs**

In `crates/desktop-shared/src/commands/language.rs`, add after the existing types:
```rust
// ── Quick Translate (popup) ───────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickTranslateParams {
    pub text: String,
    pub source_lang: String,
    pub target_lang: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickTranslateResponse {
    pub translation: String,
    pub words: Vec<WordBreakdown>,  // Reuses existing WordBreakdown from same file (line 27)
}
```

- [ ] **Step 3: Register module in mod.rs**

Add to `crates/desktop-shared/src/commands/mod.rs`:
```rust
mod practice;
pub use practice::*;
```

- [ ] **Step 4: Build and verify**

Run: `cargo build -p desktop-shared`
Expected: Compiles with no errors.

- [ ] **Step 5: Commit**
```bash
git add crates/desktop-shared/
git commit -m "feat(practice): add IPC types for practice + quick translate"
```

---

### Task 3: Domain events

**Files:**
- Modify: `crates/bus/src/domain_events.rs`

- [ ] **Step 1: Add domain event variants**

In `crates/bus/src/domain_events.rs`, add after the `NoteStudied` variant (around line 328):
```rust
PracticeUnitCompleted {
    session_id: String,
    note_id: String,
    unit_index: u32,
    grade: String,
    scores: String,      // JSON of PracticeScores
    confidence_rating: u8,
    edited: bool,
},
PracticeSessionCompleted {
    session_id: String,
    note_id: String,
    units_completed: u32,
    average_score: f64,
    source_lang: String,
    target_lang: String,
    weak_unit_count: u32,
},
```

- [ ] **Step 2: Build and verify**

Run: `cargo build --workspace`
Expected: **Will fail** on exhaustive `match` arms for `DomainEvent` in other crates (e.g., `cognitive`, `app-core`). Add wildcard or explicit arms for the two new variants in every `match` block that doesn't already have a catch-all `_ =>`. Fix all compilation errors before proceeding.

- [ ] **Step 3: Commit**
```bash
git add crates/bus/
git commit -m "feat(practice): add PracticeUnitCompleted + PracticeSessionCompleted domain events"
```

---

### Task 4: LLM prompts

**Files:**
- Create: `crates/app-core/src/handlers/notes/practice_prompts.rs`
- Modify: `crates/app-core/src/handlers/notes/language_prompts.rs`
- Modify: `crates/app-core/src/handlers/notes/mod.rs`

- [ ] **Step 1: Create practice_prompts.rs**

Create `crates/app-core/src/handlers/notes/practice_prompts.rs` with three functions:

1. `pub fn segmentation_prompt(source_lang: &str, target_lang: &str) -> String` — the Smart Segmentation prompt from the spec (Section 3).

2. `pub fn evaluation_prompt(source_lang: &str, target_lang: &str) -> String` — the practice evaluation prompt from the spec (Section 6). Includes previous unit result slot and document-level context.

3. `pub fn coaching_nudge_check(results: &[serde_json::Value]) -> Option<String>` — checks last 3 results for same-dimension weakness. Returns nudge message or None.

- [ ] **Step 2: Add quick_translate_prompt to language_prompts.rs**

In `crates/app-core/src/handlers/notes/language_prompts.rs`, add:
```rust
pub fn quick_translate_prompt(source_lang: &str, target_lang: &str) -> String {
    format!(
        r#"Translate the following {source_lang} text into {target_lang}.
Also extract key vocabulary words with their readings and meanings.

Return ONLY JSON:
{{
  "translation": "translated text",
  "words": [
    {{
      "word": "original word",
      "reading": "pronunciation (pinyin/romaji/IPA or null)",
      "meaning": "meaning in {target_lang}",
      "partOfSpeech": "noun/verb/adj/etc",
      "proficiencyLevel": "HSK 1-6 / JLPT N5-N1 / CEFR A1-C2 or null",
      "exampleSentence": null,
      "isNew": false
    }}
  ]
}}"#
    )
}
```

- [ ] **Step 3: Register module**

Add `pub mod practice_prompts;` to `crates/app-core/src/handlers/notes/mod.rs`.

- [ ] **Step 4: Build and verify**

Run: `cargo build -p app-core`

- [ ] **Step 5: Commit**
```bash
git add crates/app-core/src/handlers/notes/
git commit -m "feat(practice): add LLM prompts for segmentation, evaluation, quick translate"
```

---

### Task 5: AppCore handlers — session CRUD + LLM calls

**Files:**
- Create: `crates/app-core/src/handlers/notes/practice.rs`

- [ ] **Step 1: Create practice.rs with all AppCore handler methods**

This is the largest backend file. Follow the pattern from `language.rs` — methods on `impl AppCore`.

Methods to implement:

1. `pub async fn practice_segment_note(&self, params) -> Result<PracticeSegmentResponse>` — Check note `perspective_config` for cached segments. If cached and fresh, return. Otherwise call LLM with `practice_prompts::segmentation_prompt()`, parse JSON response, estimate time (~1.2 min per unit), cache to note metadata.

2. `pub async fn practice_start_session(&self, params) -> Result<PracticeSessionResponse>` — Create row in practice_sessions via repo. Return full session.

3. `pub async fn practice_submit_unit(&self, params) -> Result<PracticeEvalResponse>` — Fetch session, get the source text for the unit index from segments JSON. Build evaluation prompt with full note context + previous unit result. Call LLM, parse response. Include `coaching_nudge_check()` result in response if applicable.

4. `pub async fn practice_confirm_unit(&self, params) -> Result<PracticeConfirmResponse>` — Append result to session's results JSON array. Advance `current_index`. If grade <= A-: create `KnowledgeAtom` (type "translation_unit", domain "language:{pair}") + `SemanticFact` (predicate "translates_to"). Emit `PracticeUnitCompleted` event. Return next index and whether complete.

5. `pub async fn practice_get_session(&self, params) -> Result<Option<PracticeSessionResponse>>` — By session_id or note_id. If by note_id, also call `mark_abandoned_stale()` on repo first.

6. `pub async fn practice_complete_session(&self, params) -> Result<PracticeCompleteResponse>` — Compute average_score from results. Build user_translation_doc by concatenating final_translations. Update session status to "completed". Emit `PracticeSessionCompleted`. If save_to_sr: iterate weak units (grade <= A-), create flashcards with FSRS stability calibrated by grade (A- → 2.0, B+ → 1.5, B → 1.0, C → 0.5).

- [ ] **Step 2: Add `language_quick_translate` handler to language.rs**

In `crates/app-core/src/handlers/notes/language.rs`, add a new method on `impl AppCore`:
`pub async fn language_quick_translate(&self, params: QuickTranslateParams) -> Result<QuickTranslateResponse>`
— Call LLM with `quick_translate_prompt()`. Parse response. Mark words as new/known via SemanticFactRepo (same pattern as `language_translate_breakdown` at lines 55-62 of language.rs).

This goes in `language.rs` (NOT `practice.rs`) because it's a language feature, not a practice-session feature.

- [ ] **Step 3: Wire PracticeSessionRepo into AppCore**

Add `practice_repo: feature_notes::repo::practice::PracticeSessionRepo` field to `AppCore` in `crates/app-core/src/state.rs`. Then update the `AppCore` construction in `crates/app-core/src/init/mod.rs` — find where other repos are created from the `StoragePool` (e.g., `NoteRepo::new(pool.clone())`) and add `PracticeSessionRepo::new(pool.clone())` in the same block. Pass it to the `AppCore` struct.

- [ ] **Step 4: Build and verify**

Run: `cargo build -p app-core`
Expected: Compiles.

- [ ] **Step 4: Commit**
```bash
git add crates/app-core/
git commit -m "feat(practice): add AppCore handlers for practice session + evaluation + quick translate"
```

---

### Task 6: Desktop Tauri commands

**Files:**
- Create: `crates/desktop/src/commands/practice.rs`
- Modify: `crates/desktop/src/commands/mod.rs`
- Modify: `crates/desktop/src/commands/language.rs`
- Modify: `crates/desktop/src/main.rs`
- Modify: `crates/desktop/src/dev_server/mod.rs`

- [ ] **Step 1: Create practice.rs command module**

Create `crates/desktop/src/commands/practice.rs`. Follow the thin-wrapper pattern from other command modules:

```rust
use std::sync::Arc;
use tauri::State;
use crate::app_core::AppCore;
use crate::commands::ApiError;
use desktop_shared::commands::{
    PracticeSegmentParams, PracticeSegmentResponse,
    PracticeStartParams, PracticeSessionResponse,
    PracticeSubmitParams, PracticeEvalResponse,
    PracticeConfirmParams, PracticeConfirmResponse,
    PracticeGetParams, PracticeCompleteParams, PracticeCompleteResponse,
};

#[tauri::command]
pub async fn practice_segment_note(
    state: State<'_, Arc<AppCore>>,
    params: PracticeSegmentParams,
) -> Result<PracticeSegmentResponse, ApiError> {
    state.practice_segment_note(params).await.map_err(Into::into)
}

// ... repeat for all 6 remaining commands
```

Add `DEV_COMMANDS` array and `dispatch_dev()` function at the bottom.

- [ ] **Step 2: Add language_quick_translate command**

In `crates/desktop/src/commands/language.rs`, add:
```rust
#[tauri::command]
pub async fn language_quick_translate(
    state: State<'_, Arc<AppCore>>,
    params: QuickTranslateParams,
) -> Result<QuickTranslateResponse, ApiError> {
    state.language_quick_translate(params).await.map_err(Into::into)
}
```
Update DEV_COMMANDS and dispatch_dev in language.rs to include the new command.

- [ ] **Step 3: Register module**

Add `pub mod practice;` to `crates/desktop/src/commands/mod.rs`.

- [ ] **Step 4: Register commands in main.rs**

Find the `tauri::Builder::default().invoke_handler(tauri::generate_handler![...])` call in `crates/desktop/src/main.rs`. Add all 7 practice commands + `language_quick_translate` to the handler list.

- [ ] **Step 5: Add to dev_server coverage**

In `crates/desktop/src/dev_server/mod.rs`, add `practice::DEV_COMMANDS` to the coverage list and add the `practice::dispatch_dev()` call to the dev server dispatch chain.

- [ ] **Step 6: Build and verify**

Run: `cargo build -p desktop`
Expected: Compiles with no errors.

Run: `cargo nextest run -p desktop -E 'test(dev_server_covers)'`
Expected: The coverage test passes (all practice commands are registered).

- [ ] **Step 7: Commit**
```bash
git add crates/desktop/
git commit -m "feat(practice): add Tauri commands for practice + quick translate"
```

---

### Task 7: Frontend hooks

**Files:**
- Create: `desktop-ui/src/features/notes/hooks/usePracticeSession.ts`
- Create: `desktop-ui/src/features/notes/hooks/useSmartSegmentation.ts`
- Create: `desktop-ui/src/features/notes/hooks/usePracticeEvaluation.ts`
- Create: `desktop-ui/src/features/notes/hooks/useQuickTranslate.ts`

- [ ] **Step 1: Create usePracticeSession hook**

Handles session CRUD via IPC. Follow the pattern from `useTranslationPractice.ts`:
- `startSession(noteId, segments, sourceLang, targetLang, startIndex?)` → calls `ipc("practice_start_session", { params })` → returns `PracticeSessionResponse`
- `getSession(noteId)` → calls `ipc("practice_get_session", { params: { noteId } })` → returns session or null
- `confirmUnit(sessionId, index, finalTranslation, confidenceRating, edited)` → calls `ipc("practice_confirm_unit", { params })`
- `completeSession(sessionId, saveToSR)` → calls `ipc("practice_complete_session", { params })`
- State: `session`, `loading`, `error`

- [ ] **Step 2: Create useSmartSegmentation hook**

- `segment(noteId, sourceLang, targetLang)` → calls `ipc("practice_segment_note", { params })`
- Returns `{ segments, estimatedMins, cachedAt, loading, error }`
- If `cachedAt` is present, segments are from cache (instant)

- [ ] **Step 3: Create usePracticeEvaluation hook**

- `submitUnit(sessionId, index, userTranslation)` → calls `ipc("practice_submit_unit", { params })`
- Returns `{ evaluation, evaluating, error, submitUnit, reset }`
- `evaluation` holds the `PracticeEvalResponse` (grade, scores, corrections, encouragement, etc.)

- [ ] **Step 4: Create useQuickTranslate hook**

- Listens for text selection in a provided ref element (300ms debounce)
- When selection detected, calls `ipc("language_quick_translate", { params: { text, sourceLang, targetLang } })`
- Returns `{ selection, translation, words, loading, position, dismiss }`
- `position` = `{ top, left }` for popup positioning near selection
- `dismiss()` clears the selection and hides popup

- [ ] **Step 5: Lint**

Run: `cd desktop-ui && bun run lint:fix`

- [ ] **Step 6: Commit**
```bash
git add desktop-ui/src/features/notes/hooks/
git commit -m "feat(practice): add frontend hooks for practice session, segmentation, evaluation, quick translate"
```

---

### Task 8: QuickTranslatePopup + NoteEditor wiring

**Files:**
- Create: `desktop-ui/src/features/notes/components/QuickTranslatePopup.tsx`
- Modify: `desktop-ui/src/features/notes/components/NoteEditor.tsx`

- [ ] **Step 1: Create QuickTranslatePopup component**

Compact glassmorphism floating panel. Uses `glass-panel` class from theme. Renders:
- Translation text (3 lines max, truncated)
- Vocabulary chips: `word reading · meaning · HSK level` + "new" badge
- Two buttons: "Save words" (secondary) + "Practice this note →" (hero, purple glow `bg-brand`)
- Positioned via absolute positioning near the text selection

Props: `{ translation, words, position, onSaveWords, onPractice, onDismiss }`

Use portal rendering (append to document.body) to avoid overflow clipping from parent containers.

- [ ] **Step 2: Wire into NoteEditor**

In `NoteEditor.tsx`:
- Import `useQuickTranslate` hook
- Pass the editor container ref to the hook
- Conditionally render `<QuickTranslatePopup>` when `selection` is active
- `onPractice` callback: calls `onSplitModeChange("practice")` with the selected text info
- `onSaveWords` callback: calls existing `language_save_vocabulary` IPC via `useVocabularySave`
- Dismiss on Escape keydown and click outside
- Add `Cmd+Option+P` keyboard shortcut listener on NoteEditor: when pressed, enter practice mode directly (equivalent to clicking "Practice this note" without a selection)

- [ ] **Step 3: Test in dev mode**

Run: `cd desktop-ui && bun run dev` and `cargo tauri dev`
Select text in a note → popup should appear with translation.

- [ ] **Step 4: Commit**
```bash
git add desktop-ui/src/features/notes/components/QuickTranslatePopup.tsx desktop-ui/src/features/notes/components/NoteEditor.tsx
git commit -m "feat(practice): add QuickTranslatePopup on text selection"
```

---

### Task 9: SplitEditor practice mode + PracticeMode container

**Files:**
- Create: `desktop-ui/src/features/notes/components/practice/PracticeMode.tsx`
- Modify: `desktop-ui/src/features/notes/components/editor/SplitEditor.tsx`

- [ ] **Step 1: Add "practice" to SplitMode**

In `SplitEditor.tsx` line 8, update:
```typescript
type SplitMode = "translation" | "annotation" | "cornell" | "practice";
```

- [ ] **Step 2: Skip content persistence for practice mode**

In `flushSave()` (around line 98), add early return:
```typescript
if (splitMode === "practice") return; // Practice state lives in practice_sessions table
```

Similarly skip `parseSplitStore` content loading for practice mode.

- [ ] **Step 3: Render PracticeMode for practice splitMode**

In the right-pane rendering section (around line 428), add a branch:
```typescript
if (splitMode === "practice") {
  return (
    <PracticeMode
      noteId={note.id}
      sourceText={leftContent}
      sourceLang={sourceLang}
      targetLang={targetLang}
      startIndex={practiceStartIndex}
      onExit={() => onModeChange?.("single")}
    />
  );
}
```

**Split-pane reuse:** `PracticeMode` plugs into `SplitEditor`'s existing pane infrastructure. The left pane slot renders `PracticeSourcePanel` and the right pane slot renders `PracticeDocPanel`. `SplitEditor`'s resize handle works as-is. Synced scrolling needs a practice-specific handler (the existing one only activates for translation mode — see lines 299-323). `PracticeProgressHeader` renders above the split panes, `PracticeBottomBar` renders below them — both outside the pane slots.

- [ ] **Step 4: Create PracticeMode.tsx**

Main orchestrator component. Manages the practice session lifecycle:
1. On mount: call `useSmartSegmentation` to get segments
2. Show `PracticePreview` overlay if first visit (no cached segments)
3. On "Start Practice": call `usePracticeSession.startSession()`
4. Render three zones: `PracticeProgressHeader`, split panes (`PracticeSourcePanel` + `PracticeDocPanel`), `PracticeBottomBar`
5. When all units done: render `PracticeSessionComplete`

State machine: `preview` → `active` → `complete`

Props: `{ noteId, sourceText, sourceLang, targetLang, startIndex?, onExit }`

Hooks used: `usePracticeSession`, `useSmartSegmentation`, `usePracticeEvaluation`

- [ ] **Step 5: Build**

Run: `cd desktop-ui && bun run build`

- [ ] **Step 6: Commit**
```bash
git add desktop-ui/src/features/notes/components/
git commit -m "feat(practice): add PracticeMode container + SplitEditor integration"
```

---

### Task 10: Practice workspace panels

**Files:**
- Create: `desktop-ui/src/features/notes/components/practice/PracticeProgressHeader.tsx`
- Create: `desktop-ui/src/features/notes/components/practice/PracticeSourcePanel.tsx`
- Create: `desktop-ui/src/features/notes/components/practice/PracticeDocPanel.tsx`

- [ ] **Step 1: Create PracticeProgressHeader**

Single thin bar with:
- Left: `suggested_focus` from current segment ("Focus: naturalness")
- Center: "Sentence 3/9 · 87%" with thin progress fill behind text
- Right: streak flame + "Exit & Save" pill button (calls `onExit`)

Use `flex items-center justify-between` layout. Height ~32px. Background: `bg-surface-base border-b border-border`.

- [ ] **Step 2: Create PracticeSourcePanel**

Left pane rendering source text with highlighting:
- Split `sourceText` by segments (map segment indices to text ranges)
- Current segment: purple highlight (`bg-brand/10 border-l-2 border-brand`)
- Completed segments: dimmed (`opacity-40 line-through`)
- Future segments: subdued (`text-muted`)
- Auto-scroll to keep current segment visible

Props: `{ segments, currentIndex, completedIndices }`

- [ ] **Step 3: Create PracticeDocPanel**

Right pane — pure display document:
- Renders completed translations sequentially
- Each line has small clickable grade badge (colored by grade: A=green, B=yellow, C=orange)
- Current unit placeholder: "Waiting for your translation..." in `text-muted italic`
- Green flash animation when new translation appears ("+1 unit" micro-animation)
- Auto-scroll to bottom as translations are added

Props: `{ results, currentIndex, onGradeClick }`

- [ ] **Step 4: Lint and build**

Run: `cd desktop-ui && bun run lint:fix && bun run build`

- [ ] **Step 5: Commit**
```bash
git add desktop-ui/src/features/notes/components/practice/
git commit -m "feat(practice): add PracticeProgressHeader, SourcePanel, DocPanel"
```

---

### Task 11: PracticeBottomBar + ConfidenceTap

**Files:**
- Create: `desktop-ui/src/features/notes/components/practice/PracticeBottomBar.tsx`
- Create: `desktop-ui/src/features/notes/components/practice/ConfidenceTap.tsx`

- [ ] **Step 1: Create ConfidenceTap**

Simple 1-5 stars widget:
- 5 star icons, pre-filled at 4 stars
- Click to select rating (1-5)
- Compact inline display

Props: `{ value, onChange }`

- [ ] **Step 2: Create PracticeBottomBar**

Two-state bottom bar. Uses `state: "input" | "eval"` prop:

**Input state:**
- Purple prompt showing current segment's source text
- Textarea with `autoFocus`
- Enter key handler: calls `onSubmit(userTranslation)`
- Loading state during LLM call (disable input, show spinner)

**Eval state:**
- Large grade + 4 per-dimension score badges
- Corrections list: strikethrough original → green suggested + explanation
- Model translation (collapsible)
- Encouragement line (italic, `text-brand`)
- Improvement hint (if present)
- In-session coaching nudge (if triggered by `coaching_nudge_check`)
- `ConfidenceTap` widget
- Two buttons: "Edit my translation" (secondary) + "Got it — Next ⏎" (hero `bg-brand`)
- Enter key handler: calls `onConfirm(finalTranslation, confidenceRating, edited)`

Zero layout shift between states — both states occupy the same vertical space (min-height).

Props:
```typescript
interface PracticeBottomBarProps {
  state: "input" | "eval";
  currentSegment: PracticeSegment;
  evaluation?: PracticeEvalResponse;
  loading?: boolean;
  onSubmit: (userTranslation: string) => void;
  onConfirm: (finalTranslation: string, confidence: number, edited: boolean) => void;
  onEdit: () => void;
}
```

- [ ] **Step 3: Wire into PracticeMode**

Connect the bottom bar to the hooks in PracticeMode.tsx:
- Input → `usePracticeEvaluation.submitUnit()` → shows eval
- Eval "Got it" → `usePracticeSession.confirmUnit()` → advances to next
- Eval "Edit" → switches back to input with pre-filled text

- [ ] **Step 4: Lint and build**

Run: `cd desktop-ui && bun run lint:fix && bun run build`

- [ ] **Step 5: Commit**
```bash
git add desktop-ui/src/features/notes/components/practice/
git commit -m "feat(practice): add PracticeBottomBar with input/eval states + ConfidenceTap"
```

---

### Task 12: PracticePreview overlay

**Files:**
- Create: `desktop-ui/src/features/notes/components/practice/PracticePreview.tsx`

- [ ] **Step 1: Create PracticePreview**

Floating non-modal overlay (glassmorphism, centered):
- Header: "Your Personal Language Gym — 9 units · ~11 min"
- Focus summary from `suggested_focus` distribution
- Scrollable unit list with index, text preview, type badge
- "Edit segments" link (secondary) — expands list for merge/split/skip
- "Start Practice" hero button
- **Resume variant**: collapsed banner ("Resume 3/9 · 87% last time") when session exists

Props:
```typescript
interface PracticePreviewProps {
  segments: PracticeSegment[];
  estimatedMins: number;
  existingSession?: PracticeSessionResponse;
  onStart: (segments: PracticeSegment[]) => void;
  onResume: () => void;
  onCancel: () => void;
}
```

- [ ] **Step 2: Lint and build**

Run: `cd desktop-ui && bun run lint:fix && bun run build`

- [ ] **Step 3: Commit**
```bash
git add desktop-ui/src/features/notes/components/practice/PracticePreview.tsx
git commit -m "feat(practice): add PracticePreview overlay with segment list + resume banner"
```

---

### Task 13: PracticeSessionComplete + atom/flashcard micro-toast

**Files:**
- Create: `desktop-ui/src/features/notes/components/practice/PracticeSessionComplete.tsx`

- [ ] **Step 1: Create PracticeSessionComplete**

Session completion screen with:
1. **"I did this" moment**: Full-width translation document display with "You translated this." fade-in (3s pause)
2. Score overlay: large percentage + "9/9 units · 12 minutes · streak"
3. Per-dimension averages
4. Weak units summary: "3 units need review" with unit numbers
5. Actions:
   - "View My Full Translation" — toggles full-width doc view
   - "Save to Spaced Repetition (N cards)" — hero button, calls `completeSession(id, true)`
   - "Save as new note" — secondary
   - "Review with Coach" — secondary
   - "Close" — calls `onExit()`

Props: `{ session, results, onSaveToSR, onSaveAsNote, onReviewWithCoach, onExit }`

- [ ] **Step 2: Add micro-toast for atom creation**

In `PracticeMode.tsx`, after `confirmUnit` resolves, if the grade was ≤ A-, show a 1.5s toast:
"Atom saved · This unit now lives in your knowledge graph"

Use a simple absolute-positioned div with `animate-fadeIn` and auto-dismiss via setTimeout.

- [ ] **Step 3: Lint and build**

Run: `cd desktop-ui && bun run lint:fix && bun run build`

- [ ] **Step 4: Commit**
```bash
git add desktop-ui/src/features/notes/components/practice/
git commit -m "feat(practice): add PracticeSessionComplete with pride moment + micro-toast"
```

---

### Task 14: Practice History tab + LanguageLearningPanel button

**Files:**
- Create: `desktop-ui/src/features/notes/components/insight/PracticeHistoryTab.tsx`
- Modify: `desktop-ui/src/features/notes/hooks/useInsightReview.ts`
- Modify: `desktop-ui/src/features/notes/components/InsightReviewPanel.tsx`
- Modify: `desktop-ui/src/features/notes/components/LanguageLearningPanel.tsx`

- [ ] **Step 1: Create PracticeHistoryTab**

**Note:** This requires a list endpoint. Add `practice_list_sessions` IPC command:
- Types in `desktop-shared`: `PracticeListParams { note_id }` → `Vec<PracticeSessionResponse>`
- Tauri command in `commands/practice.rs`
- AppCore handler calling `practice_repo.list_for_note(note_id)`
- Add to DEV_COMMANDS

Fetches practice sessions via `ipc("practice_list_sessions", { params: { noteId } })`.

Renders vertical timeline:
- Each session card: date, units completed, average score, "Resume" or "Review" button
- Weak units get "Practice again" chip
- Empty state: "No practice sessions yet. Select text and tap 'Practice this note' to start."

Props: `{ noteId }`

- [ ] **Step 2: Add "practice" to TabId**

In `useInsightReview.ts`, add `"practice"` to the `TabId` union type.

- [ ] **Step 3: Add Practice tab to InsightReviewPanel**

In `InsightReviewPanel.tsx`:
- Add `{ id: "practice", label: "Practice" }` to the TABS array
- Add conditional render for practice tab content:
```tsx
{state.activeTab === "practice" && <PracticeHistoryTab noteId={noteId} />}
```
- Update `tabStatus()` function (switch statement) to return `"done"` for `"practice"` case — without this, TypeScript will error on exhaustiveness check

- [ ] **Step 4: Add "Turn this into active practice" button**

In `desktop-ui/src/features/notes/components/editor/LanguageLearningPanel.tsx` (note: under `editor/` subdirectory), after the last `CollapsibleSection` (around line 143), add:
```tsx
{result && (
  <div className="border-t border-border px-3 py-2 mt-2">
    <button
      type="button"
      onClick={onEnterPractice}
      className="flex items-center justify-center gap-1.5 w-full rounded-md px-3 py-2 text-xs font-medium bg-brand/15 text-brand hover:bg-brand/25 transition-colors"
    >
      Turn this into active practice
    </button>
  </div>
)}
```

Add `onEnterPractice?: () => void` to the component props. Wire it from `SplitEditor` to call `onModeChange("practice")`.

- [ ] **Step 5: Lint and build**

Run: `cd desktop-ui && bun run lint:fix && bun run build`

- [ ] **Step 6: Commit**
```bash
git add desktop-ui/src/features/notes/
git commit -m "feat(practice): add Practice History tab + 'Turn into active practice' button"
```

---

### Task 15: Final build + lint + test

**Files:** All

- [ ] **Step 1: Run Rust build**

Run: `cargo build --workspace`
Expected: Compiles with no errors.

- [ ] **Step 2: Run Rust clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: Zero warnings (fix any that appear).

- [ ] **Step 3: Run Rust tests**

Run: `cargo nextest run --workspace`
Expected: All tests pass.

- [ ] **Step 4: Run frontend lint**

Run: `cd desktop-ui && bun run lint:fix`
Expected: No errors after auto-fix.

- [ ] **Step 5: Run frontend build**

Run: `cd desktop-ui && bun run build`
Expected: Build succeeds.

- [ ] **Step 6: Run frontend tests**

Run: `cd desktop-ui && bun run test`
Expected: All tests pass.

- [ ] **Step 7: Run format check**

Run: `cargo fmt --all --check`
Expected: All formatted.

- [ ] **Step 8: Commit any remaining fixes**
```bash
git add -A
git commit -m "fix(practice): address build + lint + test issues"
```
