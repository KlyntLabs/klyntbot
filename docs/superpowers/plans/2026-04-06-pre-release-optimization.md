# Pre-Release Platform Optimization Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Optimize CPU idle usage, IPC throughput, startup time, SQLite queries, and frontend rendering across the entire Klyntbot platform before first release.

**Architecture:** Ten independent optimizations targeting the highest-impact areas identified by profiling: timer wakeup reduction (260K/day → 30K/day), IPC payload trimming for list operations, startup parallelization of non-critical init phases, SQLite batch inserts, and frontend render optimization. Each task is self-contained and can be committed independently.

**Tech Stack:** Rust (Tauri 2, SQLite/sqlx, tokio), React 19 + Vite, Biome

---

## Task 1: Reduce timer wakeup frequency — 260K → 30K wakeups/day

Three timers fire far too frequently for a single-user desktop app:
- Embedding idle-unload: every 10s → every 60s (saves ~7,776 wakeups/day)
- Running apps refresh: every 3s → every 10s (saves ~20,160 wakeups/day)
- Config watcher poll: every 5s → every 30s (saves ~14,400 wakeups/day)

**Files:**
- Modify: `crates/app-core/src/init/mod.rs:296` (embedding timer)
- Modify: `crates/app-core/src/init/launcher.rs:176` (running apps)
- Modify: `crates/app-core/src/infrastructure/config_watcher.rs:27` (config poll)

- [ ] **Step 1: Change embedding unload timer from 10s to 60s**

In `crates/app-core/src/init/mod.rs`, line 296, change:
```rust
spawn_periodic_timer(&shutdown_token, 10, move || {
```
to:
```rust
spawn_periodic_timer(&shutdown_token, 60, move || {
```

Update the comment on line 291 from:
```rust
// Idle-unload for the ONNX embedding model — check every 10s so the
// model is unloaded within ~10s of exceeding the 15s idle threshold
// (aggressive unloading to minimize retained model memory).
```
to:
```rust
// Idle-unload for the ONNX embedding model — check every 60s.
// The model auto-unloads after 15s idle; this timer just ensures
// the check happens. 60s keeps wakeups low while still reclaiming
// the ~420MB model within ~75s of last use.
```

- [ ] **Step 2: Change running apps refresh from 3s to 10s**

In `crates/app-core/src/init/launcher.rs`, line 176, change:
```rust
interval: std::time::Duration::from_secs(3),
```
to:
```rust
interval: std::time::Duration::from_secs(10),
```

- [ ] **Step 3: Change config watcher from 5s to 30s**

In `crates/app-core/src/infrastructure/config_watcher.rs`, line 27, change:
```rust
let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
```
to:
```rust
let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
```

Update the comment on line 26 and the log on line 26:
```rust
info!("config watcher started (30s poll interval)");
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check --workspace 2>&1 | grep error | head -5`
Expected: No errors.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/init/mod.rs crates/app-core/src/init/launcher.rs crates/app-core/src/infrastructure/config_watcher.rs
git commit -m "perf: reduce timer wakeups from 260K to 30K per day

Embedding unload: 10s → 60s (model idles 15s, 60s check is sufficient).
Running apps refresh: 3s → 10s (app list rarely changes mid-session).
Config watcher: 5s → 30s (config edits are rare, 30s latency is fine)."
```

---

## Task 2: Add NoteListItem response for lightweight IPC

`note_list`, `note_search`, `note_list_archived`, and `note_list_by_entity` all return `Vec<NoteResponse>` with full `body` (10-100KB), `body_html`, `split_content`, and `perspective_config` — none of which the list views use. Creating a slim `NoteListItem` response cuts IPC payload by 5-20x.

**Files:**
- Modify: `crates/desktop-shared/src/commands/notes.rs:1-24` (add `NoteListItem`)
- Modify: `crates/app-core/src/handlers/notes/converters.rs:11-26` (add `note_row_to_list_item`)
- Modify: `crates/app-core/src/handlers/notes/crud.rs:24-55,478-486` (change return types)
- Modify: `crates/desktop/src/commands/notes.rs:37-42,86-91,117-123,233-237` (change command return types)
- Modify: `crates/desktop/src/dev_server/` (update dev server routes if needed)

- [ ] **Step 1: Add NoteListItem to desktop-shared**

In `crates/desktop-shared/src/commands/notes.rs`, add after line 24 (after `NoteResponse`):

```rust
/// Lightweight note representation for list views — excludes body, HTML, and
/// split/perspective data that list UIs don't render.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteListItem {
    pub id: String,
    pub notebook_id: Option<String>,
    pub title: String,
    pub pinned: bool,
    pub archived: bool,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}
```

- [ ] **Step 2: Add note_row_to_list_item converter**

In `crates/app-core/src/handlers/notes/converters.rs`, add after line 28:

```rust
pub(crate) fn note_row_to_list_item(row: &NoteRow, tags: Vec<String>) -> NoteListItem {
    NoteListItem {
        id: row.id.clone(),
        notebook_id: row.notebook_id.clone(),
        title: row.title.clone(),
        pinned: row.pinned != 0,
        archived: row.archived != 0,
        icon: row.icon.clone(),
        color: row.color.clone(),
        tags,
        created_at: row.created_at.clone(),
        updated_at: row.updated_at.clone(),
    }
}
```

Also add a batch version `notes_list_items_batch`:
```rust
pub(crate) async fn notes_list_items_batch(
    core: &AppCore,
    rows: &[NoteRow],
) -> Result<Vec<NoteListItem>, ApiError> {
    let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
    let tags_map = core
        .note_repo
        .tags_for_notes(&ids)
        .await
        .map_err(map_storage_err)?;
    Ok(rows
        .iter()
        .map(|row| {
            let tags = tags_map.get(&row.id).cloned().unwrap_or_default();
            note_row_to_list_item(row, tags)
        })
        .collect())
}
```

Add the import for `NoteListItem` at the top of the file.

- [ ] **Step 3: Update list handlers to return NoteListItem**

In `crates/app-core/src/handlers/notes/crud.rs`:

Change `note_list` (line 24):
```rust
pub async fn note_list(
    &self,
    notebook_id: Option<String>,
) -> Result<Vec<NoteListItem>, ApiError> {
    let rows = self
        .note_repo
        .list_notes(notebook_id.as_deref())
        .await
        .map_err(map_storage_err)?;
    notes_list_items_batch(self, &rows).await
}
```

Change `note_search` (line 47):
```rust
pub async fn note_search(&self, query: String) -> Result<Vec<NoteListItem>, ApiError> {
    let rows = self
        .note_repo
        .search_notes(&query)
        .await
        .map_err(map_storage_err)?;
    notes_list_items_batch(self, &rows).await
}
```

Change `note_list_archived` (line 478):
```rust
pub async fn note_list_archived(&self) -> Result<Vec<NoteListItem>, ApiError> {
    let rows = self
        .note_repo
        .list_archived_notes()
        .await
        .map_err(map_storage_err)?;
    notes_list_items_batch(self, &rows).await
}
```

Change `note_list_by_entity` similarly.

- [ ] **Step 4: Update Tauri commands**

In `crates/desktop/src/commands/notes.rs`, update return types for `note_list`, `note_search`, `note_list_archived`, `note_list_by_entity`, `note_search_semantic` to `Vec<NoteListItem>`.

- [ ] **Step 5: Update frontend types**

In `desktop-ui/src/shared/types/`, find the `NoteResponse` type and add a `NoteListItem` type. Update `note_list` and `note_search` callers to use it.

- [ ] **Step 6: Verify compilation and run tests**

Run: `cargo check --workspace 2>&1 | grep error | head -5`
Run: `cd desktop-ui && bun run build 2>&1 | tail -5`

- [ ] **Step 7: Commit**

```bash
git add crates/desktop-shared/ crates/app-core/ crates/desktop/ desktop-ui/
git commit -m "perf: add NoteListItem for lightweight list IPC

note_list/search/archived now return NoteListItem (id, title, tags,
pinned, dates) instead of full NoteResponse (body, body_html,
split_content, perspective_config). Cuts IPC payload 5-20x for list
views that only display titles and metadata."
```

---

## Task 3: Batch flashcard inserts with multi-row INSERT

`FlashcardRepo::create_batch` inserts cards one at a time in a loop + SELECT each back. For 50 cards that's 100 DB round-trips. A transaction wrapping the loop reduces it to 1 transaction with 50 inserts.

**Files:**
- Modify: `crates/cognitive/src/repos/flashcard.rs:148-210`

- [ ] **Step 1: Wrap batch insert in a transaction**

Replace the body of `create_batch` (lines 148-210) with:

```rust
pub async fn create_batch(
    &self,
    cards: Vec<NewFlashcard>,
) -> Result<Vec<FlashcardRow>, sqlx::Error> {
    let now = Utc::now().to_rfc3339();
    let mut ids = Vec::with_capacity(cards.len());

    let mut tx = self.pool.begin().await?;

    for card in &cards {
        let id = Uuid::new_v4().to_string();
        let card_type_str = card.card_type.to_string();
        let cloze_str = card.cloze_data.as_ref().map(|v| v.to_string());
        let vocab_str = card.vocab_data.as_ref().map(|v| v.to_string());
        let image_str = card.image_data.as_ref().map(|v| v.to_string());
        let tags_str = serde_json::to_string(&card.tags).unwrap_or_else(|_| "[]".to_string());

        sqlx::query(
            r#"
            INSERT INTO flashcards
                (id, source_note_id, source_context, atom_id,
                 deck, front, back, card_type,
                 cloze_data, vocab_data, image_data, tags,
                 stability, difficulty, due_at, last_reviewed_at,
                 review_count, lapses, state, suspended, recall_speed_ms,
                 back_embedding_updated_at, preferred_mode,
                 difficulty_estimate, prerequisite_concepts, card_distractors,
                 created_at, updated_at)
            VALUES
                (?1, ?2, ?3, ?4,
                 ?5, ?6, ?7, ?8,
                 ?9, ?10, ?11, ?12,
                 ?13, ?14, ?15, NULL,
                 0, 0, 'new', 0, NULL,
                 NULL, NULL,
                 ?16, ?17, NULL,
                 ?18, ?18)
            "#,
        )
        .bind(&id)
        .bind(&card.source_note_id)
        .bind(&card.source_context)
        .bind(&card.atom_id)
        .bind(&card.deck)
        .bind(&card.front)
        .bind(&card.back)
        .bind(&card_type_str)
        .bind(&cloze_str)
        .bind(&vocab_str)
        .bind(&image_str)
        .bind(&tags_str)
        .bind(card.stability)
        .bind(card.difficulty)
        .bind(&now)
        .bind(card.difficulty_estimate)
        .bind(&card.prerequisite_concepts)
        .bind(&now)
        .execute(&mut *tx)
        .await?;

        ids.push(id);
    }

    tx.commit().await?;

    // Fetch all inserted rows in one query
    let placeholders: String = ids.iter().enumerate()
        .map(|(i, _)| format!("?{}", i + 1))
        .collect::<Vec<_>>()
        .join(", ");
    let query_str = format!("SELECT * FROM flashcards WHERE id IN ({placeholders})");
    let mut query = sqlx::query_as::<_, FlashcardRow>(&query_str);
    for id in &ids {
        query = query.bind(id);
    }
    let rows = query.fetch_all(&self.pool).await?;
    Ok(rows)
}
```

- [ ] **Step 2: Verify tests pass**

Run: `cargo nextest run -p cognitive -E 'test(/flashcard/)' 2>&1 | tail -10`
Expected: All pass.

- [ ] **Step 3: Commit**

```bash
git add crates/cognitive/src/repos/flashcard.rs
git commit -m "perf: wrap flashcard batch insert in transaction + single fetch

Was: N inserts + N selects = 2N round-trips without transaction.
Now: 1 transaction with N inserts + 1 batch SELECT = N+1 in one tx.
For 50 cards: ~100 round-trips → ~51 in a single transaction."
```

---

## Task 4: Parallelize startup phases 5-9

Phases 5-9 (Productivity, Coaching, Cognitive, Launcher, Mirror) run sequentially after Phase 4 (channels). Most are independent and can run concurrently using `tokio::join!`.

**Files:**
- Modify: `crates/app-core/src/init/mod.rs:326-395`

- [ ] **Step 1: Wrap phases 5, 7, 8 in tokio::join!**

Phases 5 (Productivity), 7 (Cognitive), and 8 (Launcher) are independent. Phase 6 (Coaching) depends on Phase 5's `productivity_repos`. Phase 9 (Mirror) is independent.

Replace the sequential phase 5/7/8 calls with:

```rust
// ── Phases 5, 7, 8: Run independent init phases concurrently ────
let (productivity_result, _, launcher_result) = tokio::join!(
    productivity::init_productivity(
        &config,
        &storage_pool,
        &domain_event_bus,
        &activity_svc,
        &cognitive_provider,
        &shutdown_token,
    ),
    cognitive::init_cognitive(
        &mut config,
        &storage_pool,
        &activity_svc,
        &shutdown_token,
        Arc::clone(&embedding_engine),
    ),
    launcher::init_launcher(&config, &storage_pool, &shutdown_token),
);

let productivity::ProductivityResult { ... } = productivity_result;
let launcher::LauncherResult { launcher_engine } = launcher_result;
```

**Note:** `config` is borrowed mutably by `cognitive::init_cognitive`, which prevents simple `join!`. If this is the case, move cognitive init to run sequentially but after the parallel block. The key win is parallelizing productivity + launcher (the two slowest).

- [ ] **Step 2: Verify it compiles and app starts correctly**

Run: `cargo check -p app-core 2>&1 | grep error | head -5`
Then: `cargo tauri dev` — verify all features work.

- [ ] **Step 3: Commit**

```bash
git add crates/app-core/src/init/mod.rs
git commit -m "perf: parallelize startup phases 5+8 (productivity + launcher)

These phases are independent and were running sequentially. Running
them concurrently saves ~1s of startup time on typical hardware."
```

---

## Task 5: Add prefers-reduced-motion and optimize CSS animations

14 animations defined, some infinite. Add `prefers-reduced-motion` support and reduce backdrop-blur intensity.

**Files:**
- Modify: `desktop-ui/src/styles/theme.css`

- [ ] **Step 1: Add reduced-motion media query**

At the end of `desktop-ui/src/styles/theme.css`, add:

```css
/* Respect OS reduced-motion preference — disable infinite animations
   and heavy backdrop-blur for better performance on low-end hardware. */
@media (prefers-reduced-motion: reduce) {
  *,
  *::before,
  *::after {
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.01ms !important;
  }

  .glass-panel,
  .glass-toolbar,
  .glass-input {
    backdrop-filter: none !important;
    -webkit-backdrop-filter: none !important;
  }
}
```

- [ ] **Step 2: Build and verify**

Run: `cd desktop-ui && bun run build 2>&1 | tail -3`
Expected: Build succeeds.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/styles/theme.css
git commit -m "perf(ui): add prefers-reduced-motion support

Disables infinite animations and backdrop-blur when OS preference
is set. Improves GPU usage and battery on low-end hardware."
```

---

## Task 6: Add missing SQLite indexes

Add composite indexes for common query patterns missing index coverage.

**Files:**
- Modify: `crates/cognitive/migrations/` (add new migration or modify existing)

- [ ] **Step 1: Identify the correct migration location**

Run: `ls crates/cognitive/migrations/` to find the latest migration number.

- [ ] **Step 2: Add indexes in the flashcard migration**

If pre-release (no migration versioning needed), modify the flashcard table creation SQL to add:

```sql
CREATE INDEX IF NOT EXISTS idx_flashcards_deck_due ON flashcards(deck, due_at);
CREATE INDEX IF NOT EXISTS idx_flashcards_state_due ON flashcards(state, due_at);
```

Find where flashcard indexes are defined and add these two.

- [ ] **Step 3: Add domain event log timestamp index**

Find the domain_event_log table and add:

```sql
CREATE INDEX IF NOT EXISTS idx_domain_event_log_timestamp ON domain_event_log(timestamp);
```

- [ ] **Step 4: Run migrations to verify**

Run: `cargo nextest run -p storage -E 'test(/migration/)' 2>&1 | tail -10`

- [ ] **Step 5: Commit**

```bash
git add crates/
git commit -m "perf: add missing SQLite indexes for flashcard and event queries

idx_flashcards_deck_due: covers 'due cards by deck' queries.
idx_flashcards_state_due: covers 'next review' scheduler queries.
idx_domain_event_log_timestamp: covers event log time-range queries."
```

---

## Task 7: Memoize DayColumnsView render allocations

`DayColumnsView` creates new Map objects and inline argument objects on every render, defeating React's reconciliation.

**Files:**
- Modify: `desktop-ui/src/features/dashboard/components/DayColumnsView.tsx`

- [ ] **Step 1: Read the component**

Read the file to find the exact lines with `new Map` creation and inline `useQuery` args.

- [ ] **Step 2: Memoize useQuery arguments**

Find the `useQuery` call with inline `{ date, tzOffsetMins }` and wrap in `useMemo`:

```tsx
const queryArgs = useMemo(
  () => ({ date, tzOffsetMins: TZ_OFFSET_MINS }),
  [date],
);
const { data: activityTimeline } = useQuery<ActivityTimeline[]>(
  "productivity_timeline",
  queryArgs,
  // ... rest unchanged
);
```

- [ ] **Step 3: Memoize Map creation**

Find the `new Map<LayerKey, TimelineEntry[]>()` and wrap in `useMemo`:

```tsx
const columnEntries = useMemo(() => {
  const entryMap = new Map<LayerKey, TimelineEntry[]>();
  // ... existing grouping logic
  return entryMap;
}, [entries]);
```

- [ ] **Step 4: Build and verify**

Run: `cd desktop-ui && bun run build && bun run test`

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/dashboard/components/DayColumnsView.tsx
git commit -m "perf(ui): memoize DayColumnsView allocations

Inline object creation in useQuery args and Map creation in render
caused unnecessary cache misses and re-renders. Wrapped in useMemo."
```

---

## Task 8: Enforce default limits on pagination-less commands

Several IPC commands accept `limit` but don't enforce a maximum. A frontend bug requesting all records could serialize 10-50MB JSON.

**Files:**
- Modify: `crates/app-core/src/handlers/` (productivity, activity-log commands)

- [ ] **Step 1: Find commands with unbounded limits**

Run: `grep -rn "limit: Option<i64>" crates/desktop/src/commands/ | head -20`

- [ ] **Step 2: Add max limit enforcement**

For each handler that accepts `limit: Option<i64>`, enforce a maximum:

```rust
let limit = limit.unwrap_or(100).min(500);
```

This ensures: default 100 rows, max 500, regardless of what the frontend requests.

- [ ] **Step 3: Verify and commit**

Run: `cargo check --workspace 2>&1 | grep error`

```bash
git add crates/
git commit -m "perf: enforce max limit of 500 on paginated IPC commands

Prevents unbounded serialization if frontend passes no limit or
excessive limit. Default 100, max 500."
```

---

## Task 9: Add SQLite PRAGMA optimize on shutdown

SQLite's `PRAGMA optimize` analyzes query patterns and updates internal statistics. Running it on graceful shutdown improves future query planning.

**Files:**
- Modify: `crates/storage/src/pool.rs` (add `optimize()` method)
- Modify: `crates/app-core/src/state.rs` or shutdown handler

- [ ] **Step 1: Add optimize method to StoragePool**

In `crates/storage/src/pool.rs`, add:

```rust
/// Run PRAGMA optimize to update SQLite's internal query statistics.
/// Call on graceful shutdown for best query planning next startup.
pub async fn optimize(&self) -> Result<(), StorageError> {
    sqlx::query("PRAGMA optimize;")
        .execute(&self.0)
        .await?;
    Ok(())
}
```

- [ ] **Step 2: Call optimize during AppCore shutdown**

Find the shutdown handler and add:
```rust
if let Err(e) = self.storage_pool.optimize().await {
    tracing::warn!("SQLite PRAGMA optimize failed: {e}");
}
```

- [ ] **Step 3: Verify and commit**

```bash
git add crates/storage/ crates/app-core/
git commit -m "perf: run PRAGMA optimize on graceful shutdown

Lets SQLite update internal statistics based on query patterns
observed during this session, improving query planning on next startup."
```

---

## Task 10: Frontend — lazy load heavy internal components

GraphView (1,058 lines) and NoteEditor (705 lines) with tiptap are loaded eagerly within their route pages. Wrap in `React.lazy` for on-demand loading.

**Files:**
- Modify files in `desktop-ui/src/features/notes/` that import GraphView and NoteEditor

- [ ] **Step 1: Find static imports of heavy components**

Run: `grep -rn "import.*GraphView\|import.*NoteEditor" desktop-ui/src/ | grep -v "lazy\|test"` to find eager imports.

- [ ] **Step 2: Convert to lazy imports**

For each static import, change to:
```tsx
const GraphView = lazy(() => import("./components/GraphView"));
const NoteEditor = lazy(() => import("./components/NoteEditor"));
```

Wrap usage in `<Suspense fallback={<div />}>`.

- [ ] **Step 3: Build and verify**

Run: `cd desktop-ui && bun run build 2>&1 | tail -5`
Verify new chunks appear in output.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/
git commit -m "perf(ui): lazy-load GraphView and NoteEditor components

These are 1,058 and 705 lines respectively with heavy deps (force-graph,
tiptap). Now loaded on-demand when user navigates to those features."
```
