# Insight Review V2 — Gap Fixes Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix all identified gaps in the Insight Review V2 system — from the critical scope resolution bug (insights see zero related notes) to minor edge-case polish.

**Architecture:** Most fixes are wiring changes — connecting existing implementations to the pipeline. The real ScopeResolver is the largest piece of new code. Everything else is small targeted edits.

**Tech Stack:** Rust (sqlx, async-trait), TypeScript/React, Axum SSE

---

## File Structure

### New files

| File | Responsibility |
|------|---------------|
| `crates/app-core/src/adapters/scope_resolver.rs` | Real `ScopeResolver` impl — Backlinks, Semantic, Project, Manual |
| `desktop-ui/src/features/notes/hooks/useInsightSSE.ts` | Frontend EventSource bridge for insight events in browser dev mode |

### Modified files

| File | Change |
|------|--------|
| `crates/app-core/src/adapters/mod.rs` | Add `pub mod scope_resolver;` |
| `crates/app-core/src/init/mod.rs` | Replace `NoopScopeResolver` with `ScopeResolverImpl` (BEFORE `vector_store` is moved) |
| `crates/feature-insights/src/prompt_builder.rs` | Wire `deep_dive` flag → call 3 deep-dive CognitiveAccessor methods |
| `crates/app-core/src/handlers/notes/insight.rs` | Fix `regenerate_tab` to use InsightService pipeline; add `note_insight_submit_quiz`; populate `note_title` in evolution response |
| `crates/feature-insights/src/progress.rs` | Accept optional `quiz_score` override in `compute` |
| `crates/feature-insights/src/service.rs` | Add `compute_progress_with_quiz` method |
| `desktop-ui/src/features/notes/hooks/useInsightReview.ts` | Submit quiz score via IPC on `revealAll`; fix `total` in `revealAnswer` |
| `crates/desktop-shared/src/commands/notes.rs` | Add `InsightQuizSubmitParams` DTO |
| `crates/desktop/src/commands/notes.rs` | Add `note_insight_submit_quiz` Tauri command + DEV_COMMANDS + dispatch |
| `crates/desktop/src/main.rs` | Register `note_insight_submit_quiz` |
| `crates/desktop/src/dev_server/mod.rs` | Add `MultiEmitter` wrapper + insight SSE broadcast channel |
| `crates/desktop/src/dev_server/streaming.rs` | Add `insight_sse_handler` endpoint |
| `desktop-ui/src/features/notes/components/InsightReviewPanel.tsx` | Use `useInsightSSE` hook |

---

## Chunk 1: Real ScopeResolver (Critical — insights have zero context)

### Task 1: Implement `ScopeResolverImpl`

**Files:**
- Create: `crates/app-core/src/adapters/scope_resolver.rs`
- Modify: `crates/app-core/src/adapters/mod.rs`
- Modify: `crates/app-core/src/init/mod.rs`

Currently `NoopScopeResolver` returns `[]` for all scope types. The insight LLM sees only the single note body with no related context. This is the single biggest quality bug.

The resolver needs access to:
- `NoteRepo` (for backlinks + notebook-based project scope)
- `VectorStore` (for semantic similarity search on `note_embeddings` table)

**Critical note:** `vector_store` is moved into `init_agent()` at `init/mod.rs:L159`. The `ScopeResolverImpl` must be constructed BEFORE that line, alongside the existing `insight_embedder` and `note_embedding_handler` which also clone `vector_store` before the move.

**VectorStore API (verified):** `search_similar` at `crud.rs:L116` takes `(&self, table: &str, query: &[f32], limit: usize, threshold: f64)` — it needs a vector, not an ID. So semantic search requires: (1) `get_embedding("note_embeddings", note_id)` to get the note's vector, (2) `search_similar("note_embeddings", &vector, limit, radius)` with that vector.

- [ ] **Step 1: Create scope_resolver.rs**

```rust
// crates/app-core/src/adapters/scope_resolver.rs
use async_trait::async_trait;
use feature_insights::{ScopeConfig, ScopeResolver, ScopeType};
use feature_notes::repo::NoteRepo;
use storage::VectorStore;

pub struct ScopeResolverImpl {
    note_repo: NoteRepo,
    vector_store: Option<VectorStore>,
}

impl ScopeResolverImpl {
    pub fn new(note_repo: NoteRepo, vector_store: Option<VectorStore>) -> Self {
        Self {
            note_repo,
            vector_store,
        }
    }

    async fn resolve_backlinks(&self, note_id: &str) -> Vec<String> {
        let backlinks = self
            .note_repo
            .get_backlinks_with_context(note_id)
            .await
            .unwrap_or_default();
        let mut ids: Vec<String> = backlinks.into_iter().map(|(note, _ctx)| note.id).collect();
        ids.sort();
        ids
    }

    async fn resolve_semantic(&self, note_id: &str, radius: f64) -> Vec<String> {
        let Some(ref vs) = self.vector_store else {
            return Vec::new();
        };
        // Step 1: fetch the note's own embedding vector
        let embedding = match vs.get_embedding("note_embeddings", note_id).await {
            Ok(Some(v)) => v,
            _ => return Vec::new(),
        };
        // Step 2: search for similar notes using cosine similarity
        match vs
            .search_similar("note_embeddings", &embedding, 20, radius)
            .await
        {
            Ok(results) => results
                .into_iter()
                .map(|(id, _score)| id)
                .filter(|id| id != note_id)
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    async fn resolve_project(&self, note_id: &str) -> Vec<String> {
        let note = match self.note_repo.get_note(note_id).await {
            Ok(Some(n)) => n,
            _ => return Vec::new(),
        };
        let Some(ref notebook_id) = note.notebook_id else {
            return Vec::new();
        };
        match self.note_repo.list_notes(Some(notebook_id)).await {
            Ok(notes) => notes
                .into_iter()
                .map(|n| n.id)
                .filter(|id| id != note_id)
                .collect(),
            Err(_) => Vec::new(),
        }
    }
}

#[async_trait]
impl ScopeResolver for ScopeResolverImpl {
    async fn resolve(&self, note_id: &str, config: &ScopeConfig) -> Vec<String> {
        match config.scope_type {
            ScopeType::Backlinks => self.resolve_backlinks(note_id).await,
            ScopeType::Semantic => self.resolve_semantic(note_id, config.radius).await,
            ScopeType::Project => self.resolve_project(note_id).await,
            ScopeType::Manual => config.node_ids.clone(),
        }
    }
}
```

- [ ] **Step 2: Verify `search_similar` return type**

Read `crates/storage/src/vector_store/crud.rs` at the `search_similar` method to verify it returns `Vec<(String, f64)>` (id + score). The code above assumes this — adjust if the return type differs.

- [ ] **Step 3: Add to adapters/mod.rs**

```rust
pub mod scope_resolver;
```

- [ ] **Step 4: Wire in init/mod.rs — BEFORE `vector_store` is moved**

Place this block after the `cognitive_accessor` construction (~line 143) and BEFORE `init_agent` (~line 159):

```rust
// ── Scope resolver for insight context ──
let scope_resolver: Arc<dyn feature_insights::ScopeResolver> = Arc::new(
    crate::adapters::scope_resolver::ScopeResolverImpl::new(
        note_repo.clone(),
        vector_store.clone(),
    ),
);
```

Then in the `insight_service` block (~line 268), replace:
```rust
Arc::new(feature_insights::NoopScopeResolver), // Task 9 wires real impl
```
with:
```rust
scope_resolver,
```

- [ ] **Step 5: Build + test**

Run: `cargo build --workspace`
Run: `cargo nextest run -p feature-insights`
Run: `cargo nextest run -p desktop -E 'test(dev_server)'`

- [ ] **Step 6: Commit**

```bash
git add crates/app-core/src/adapters/scope_resolver.rs crates/app-core/src/adapters/mod.rs crates/app-core/src/init/mod.rs
git commit -m "feat(app-core): implement real ScopeResolver with backlinks, semantic, project, manual modes"
```

---

## Chunk 2: Wire Deep Dive in PromptBuilder

### Task 2: Call deep-dive CognitiveAccessor methods when `deep_dive=true`

**Files:**
- Modify: `crates/feature-insights/src/prompt_builder.rs`

The 3 deep-dive methods (`user_model_summary`, `entity_neighborhood`, `fact_history`) are fully implemented in `CognitiveAccessorImpl` but `PromptBuilder::build_context` never calls them. It only checks `include_cognitive`.

- [ ] **Step 1: Read current `build_context` in prompt_builder.rs**

Understand where cognitive injection happens (the `include_cognitive` block). The deep-dive section should be added after the medium-tier cognitive section, still inside `build_context`.

- [ ] **Step 2: Add deep-dive section after cognitive injection**

After the existing `if scope_config.include_cognitive { ... }` block, add:

```rust
if scope_config.deep_dive {
    let note_title_for_subject = note.title.clone();
    let (user_model, neighborhood, history) = tokio::join!(
        self.cognitive.user_model_summary(""),
        self.cognitive.entity_neighborhood(&note.id, 2),
        self.cognitive.fact_history(&note_title_for_subject),
    );

    let mut deep_parts = Vec::new();
    if let Some(model) = user_model {
        deep_parts.push(format!("### User Model\n{model}"));
    }
    if !neighborhood.is_empty() {
        deep_parts.push(format!(
            "### Entity Connections\n{}",
            neighborhood.iter().map(|r| format!("- {r}")).collect::<Vec<_>>().join("\n")
        ));
    }
    if !history.is_empty() {
        deep_parts.push(format!(
            "### Knowledge Evolution\n{}",
            history.iter().map(|h| format!("- {h}")).collect::<Vec<_>>().join("\n")
        ));
    }
    if !deep_parts.is_empty() {
        sections.push(format!("## Deep Dive Context\n\n{}", deep_parts.join("\n\n")));
    }
}
```

- [ ] **Step 3: Build + test**

Run: `cargo build --workspace`
Run: `cargo nextest run -p feature-insights`

- [ ] **Step 4: Commit**

```bash
git add crates/feature-insights/src/prompt_builder.rs
git commit -m "feat(feature-insights): wire deep_dive flag to inject user model, entity graph, and fact history"
```

---

## Chunk 3: Fix `regenerate_tab` to Use InsightService Pipeline

### Task 3: Replace legacy context assembly in `regenerate_tab`

**Files:**
- Modify: `crates/app-core/src/handlers/notes/insight.rs`

Currently `note_insight_regenerate_tab` uses `insight_context::assemble_context` (legacy backlinks-only, no cognitive injection). It should use `InsightService::prepare_context` for consistent quality.

**Verified:** `InsightReviewRow.scope_config` is the field name (at `types.rs:L76`), NOT `scope_config_json`.

- [ ] **Step 1: Read current `note_insight_regenerate_tab` implementation**

It's at `insight.rs:~L288-L365`. It fetches the note, builds context via legacy path, then calls the LLM for a single tab.

- [ ] **Step 2: Refactor to use InsightService pipeline**

Replace the legacy context assembly block. Full replacement code:

```rust
// Replace:
//   let related_notes = self.fetch_related_notes(note_id).await;
//   let ctx = insight_context::assemble_context(&note, &related_notes, None);

// With: use InsightService pipeline for consistent context quality
let (ctx_text, ctx_note_title) = if let Some(ref service) = self.insight_service {
    // Reuse scope from the last generation (or default)
    let scope: feature_insights::ScopeConfig = if let Ok(Some(latest)) = service.get_latest(note_id).await {
        serde_json::from_str(&latest.scope_config).unwrap_or_default()
    } else {
        feature_insights::ScopeConfig::default()
    };

    // Resolve scope + fetch related notes
    let scope_ids = service.resolve_scope(note_id, &scope).await;
    let mut related_notes = Vec::new();
    for id in &scope_ids {
        if let Ok(Some(n)) = self.note_repo.get_note(id).await {
            related_notes.push(n);
        }
    }

    let tags = self.note_repo.get_tags(note_id).await.unwrap_or_default();
    let note_domains = insight_context::extract_note_domains(&tags);

    match service.prepare_context(&note, &related_notes, &scope_ids, &scope, &note_domains).await {
        Ok(prepared) => (prepared.context.text, prepared.context.note_title),
        Err(e) => {
            tracing::warn!("prepare_context failed in regenerate_tab, falling back: {e}");
            let related = self.fetch_related_notes(note_id).await;
            let ctx = insight_context::assemble_context(&note, &related, None);
            (ctx.text, ctx.note_title)
        }
    }
} else {
    let related = self.fetch_related_notes(note_id).await;
    let ctx = insight_context::assemble_context(&note, &related, None);
    (ctx.text, ctx.note_title)
};
```

Then update the prompt construction to use `ctx_text` and `ctx_note_title` instead of `ctx.text` and `ctx.note_title`.

- [ ] **Step 3: Build + test**

Run: `cargo build --workspace`

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/handlers/notes/insight.rs
git commit -m "fix(app-core): regenerate_tab uses InsightService pipeline instead of legacy context"
```

---

## Chunk 4: Browser Dev Mode SSE for Insight Events

### Task 4: Add SSE bridge for `insight:*` events in dev server

**Files:**
- Modify: `crates/desktop/src/dev_server/mod.rs`
- Modify: `crates/desktop/src/dev_server/streaming.rs`
- Create: `desktop-ui/src/features/notes/hooks/useInsightSSE.ts`
- Modify: `desktop-ui/src/features/notes/components/InsightReviewPanel.tsx`

**Design decision:** Use a `MultiEmitter` wrapper that forwards events to both the Tauri emitter AND a broadcast channel. This is set at init time in the dev server path.

The `AppCore` is initialized once (in `app_core::init`), and its `event_emitter` is set to `TauriEventEmitter`. The dev server gets a clone of `Arc<AppCore>`. We can't change the emitter after init.

**Chosen approach:** Wrap the `TauriEventEmitter` in a `MultiEmitter` that also sends to a broadcast channel. This requires modifying `app_core::init` to accept an additional broadcast sender for dev mode.

Actually, simpler: since the dev server already has access to `Arc<AppCore>`, and the `event_emitter` field is `Arc<dyn AppEventEmitter>`, we can wrap it BEFORE passing the core to the dev server.

Wait — `event_emitter` is set inside `AppCore::init_with_sender`. To avoid touching that, the cleanest approach: in `main.rs` where the dev server is started, clone the core's `event_emitter`, create a `MultiEmitter`, and... no, the field isn't pub.

**Simplest correct approach:** Add a `broadcast_emitter` field to `DevState` and modify the dispatch for `note_insight_review` to inject a custom emitter. This follows the same pattern as `chat_send` which uses `spawn_chat_relay(stream_info, emitter)`.

**But** `note_insight_review` in `AppCore` captures `self.event_emitter` in the spawned task. To override this, we'd need to add an `emitter_override` parameter to `note_insight_review`.

**Final simplest approach:** Add an optional `emitter_override: Option<Arc<dyn AppEventEmitter>>` parameter to `note_insight_review` in `AppCore`. The Tauri command passes `None` (uses default). The dev server dispatch passes `Some(SseEmitter)`.

- [ ] **Step 1: Add `emitter_override` to `note_insight_review`**

In `crates/app-core/src/handlers/notes/insight.rs`, change the signature:

```rust
pub async fn note_insight_review(
    &self,
    note_id: &str,
    scope_params: Option<&InsightScopeConfigParams>,
    emitter_override: Option<Arc<dyn AppEventEmitter>>,
) -> Result<InsightReviewStarted, ApiError> {
```

And in the background spawn, use the override if present:

```rust
let emitter = emitter_override.unwrap_or_else(|| Arc::clone(&self.event_emitter));
```

Update the Tauri command to pass `None`:
```rust
state.note_insight_review(&note_id, scope_config.as_ref(), None).await
```

- [ ] **Step 2: Add insight broadcast channel to DevState + SSE endpoint**

In `mod.rs`:
```rust
pub(super) struct DevState {
    pub(super) core: Arc<AppCore>,
    pub(super) sse_channels: SseChannels,
    pub(super) insight_tx: broadcast::Sender<(String, Value)>,
}
```

In `start()`:
```rust
let (insight_tx, _) = broadcast::channel(256);
```

Add route:
```rust
.route("/api/insight/events", axum::routing::get(streaming::insight_sse_handler))
```

In `streaming.rs`, add `insight_sse_handler` (similar structure to `cognitive_sse_handler` but reading from `insight_tx`).

- [ ] **Step 3: Wire SseEmitter in dev dispatch for `note_insight_review`**

In `dispatch.rs`, the dispatch for `note_insight_review` creates an `SseEmitter` from `insight_tx`:

```rust
"note_insight_review" => {
    let id = try_field!(dev::get_str(body, "noteId"));
    let scope: Option<desktop_shared::commands::InsightScopeConfigParams> = body
        .get("scopeConfig")
        .and_then(|v| serde_json::from_value(v.clone()).ok());
    let emitter: Arc<dyn AppEventEmitter> = Arc::new(SseEmitter {
        tx: state.insight_tx.clone(),
    });
    dev::val(core.note_insight_review(&id, scope.as_ref(), Some(emitter)).await)
}
```

Note: `dispatch_dev` in `notes.rs` needs access to the `insight_tx`. This means the dispatch signature needs the `DevState` or the `insight_tx` passed through. Check how `chat_send` does it — it has access via `SseChannels`.

- [ ] **Step 4: Create frontend EventSource hook**

```typescript
// desktop-ui/src/features/notes/hooks/useInsightSSE.ts
import { isTauri } from "@shared/lib/utils";
import { useEffect } from "react";

const INSIGHT_EVENTS = [
  "insight:synthesis-chunk",
  "insight:synthesis-done",
  "insight:tab-done",
  "insight:error",
  "insight:perspectives-meta",
];

export function useInsightSSE(active: boolean) {
  useEffect(() => {
    if (!active || isTauri) return;
    const es = new EventSource("http://localhost:3456/api/insight/events");
    const handler = (e: MessageEvent) => {
      try {
        const data = JSON.parse(e.data);
        window.dispatchEvent(new CustomEvent(e.type, { detail: data }));
      } catch {
        // malformed SSE payload
      }
    };
    for (const evt of INSIGHT_EVENTS) {
      es.addEventListener(evt, handler);
    }
    return () => es.close();
  }, [active]);
}
```

- [ ] **Step 5: Use hook in InsightReviewPanel**

In `InsightReviewPanel.tsx`, add:
```typescript
import { useInsightSSE } from "../hooks/useInsightSSE";

// Inside the component:
useInsightSSE(state.isOpen);
```

- [ ] **Step 6: Build + test in browser**

Run: `cargo build --workspace`
Run: `cd desktop-ui && bun run build`
Open `localhost:1420`, open a note, trigger Insight Review on a non-cached note, verify streaming works.

- [ ] **Step 7: Commit**

```bash
git add crates/desktop/src/dev_server/ crates/app-core/src/handlers/notes/insight.rs crates/desktop/src/commands/notes.rs desktop-ui/src/features/notes/
git commit -m "feat(dev-server): add SSE bridge for insight streaming events in browser dev mode"
```

---

## Chunk 5: Quiz Score Persistence

### Task 5: Persist quiz answers and compute real quiz_score

**Files:**
- Modify: `crates/desktop-shared/src/commands/notes.rs` (add DTO)
- Modify: `crates/app-core/src/handlers/notes/insight.rs` (add handler)
- Modify: `crates/desktop/src/commands/notes.rs` (add Tauri command + DEV_COMMANDS + dispatch)
- Modify: `crates/desktop/src/main.rs` (register command)
- Modify: `crates/feature-insights/src/progress.rs` (accept quiz_score override)
- Modify: `crates/feature-insights/src/service.rs` (add compute_progress_with_quiz)
- Modify: `desktop-ui/src/features/notes/hooks/useInsightReview.ts` (submit quiz, fix total bug)

Currently `quiz_score` is hardcoded to 0.0 in `ProgressComputer::compute` (progress.rs:L76). The frontend tracks quiz state locally but never sends answers to the backend.

- [ ] **Step 1: Add DTO in desktop-shared**

```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InsightQuizSubmitParams {
    pub insight_review_id: String,
    pub score: f64,  // 0.0-1.0 normalized
    pub total: i32,
}
```

- [ ] **Step 2: Add handler in app-core**

```rust
pub async fn note_insight_submit_quiz(
    &self,
    params: &InsightQuizSubmitParams,
) -> Result<(), ApiError> {
    let service = self
        .insight_service
        .as_ref()
        .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Insight service not available"))?;

    let insight = service
        .get_version(&params.insight_review_id)
        .await
        .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?
        .ok_or_else(|| ApiError::new("NOT_FOUND", "Insight not found"))?;

    // Get the note body for progress computation
    let note = self
        .note_repo
        .get_note(&insight.note_id)
        .await
        .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?
        .ok_or_else(|| ApiError::new("NOT_FOUND", "Note not found"))?;

    service
        .compute_progress_with_quiz(&params.insight_review_id, &note.body, params.score)
        .await
        .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;

    Ok(())
}
```

- [ ] **Step 3: Modify ProgressComputer to accept quiz_score override**

In `crates/feature-insights/src/progress.rs`, change `compute`:

```rust
pub async fn compute(
    &self,
    insight: &InsightReviewRow,
    note_body: &str,
    quiz_score_override: Option<f64>,
) -> Result<ProgressSnapshotRow, sqlx::Error> {
    // ... existing code for flashcard, drift, gap closure ...
    let quiz_score = quiz_score_override.unwrap_or(0.0);
    // ... upsert with quiz_score ...
}
```

Update all callers of `compute` to pass `None` for the default path, `Some(score)` for quiz submission.

- [ ] **Step 4: Add `compute_progress_with_quiz` to InsightService**

```rust
pub async fn compute_progress_with_quiz(
    &self,
    insight_id: &str,
    note_body: &str,
    quiz_score: f64,
) -> Result<ProgressSnapshotRow, sqlx::Error> {
    let insight = self.repo.get(insight_id).await?.ok_or(sqlx::Error::RowNotFound)?;
    self.progress_computer().compute(&insight, note_body, Some(quiz_score)).await
}
```

Update `compute_progress` to pass `None`:
```rust
pub async fn compute_progress(&self, insight_id: &str, note_body: &str) -> Result<ProgressSnapshotRow, sqlx::Error> {
    let insight = self.repo.get(insight_id).await?.ok_or(sqlx::Error::RowNotFound)?;
    self.progress_computer().compute(&insight, note_body, None).await
}
```

- [ ] **Step 5: Add Tauri command + DEV_COMMANDS + dispatch + main.rs**

Standard pattern — Tauri command delegates to AppCore handler. Add to DEV_COMMANDS, dispatch_dev, and main.rs `invoke_handler`.

- [ ] **Step 6: Fix `revealAnswer` total bug + submit quiz from frontend**

In `useInsightReview.ts`, in the `revealAnswer` callback, add:
```typescript
total: prev.tabs.assessment.questions.length,
```

In the `revealAll` callback, after computing score, submit to backend:
```typescript
// After setState, fire IPC (best-effort)
if (state.insightReviewId && questions.length > 0) {
    ipc("note_insight_submit_quiz", {
        insightReviewId: state.insightReviewId,
        score: score / questions.length,
        total: questions.length,
    }).catch(() => {});
}
```

Note: `revealAll` is a `useCallback` with `[]` deps. It needs access to `state.insightReviewId`. Either add it to the dependency array, or read it from the updater function's `prev` state. The latter is cleaner — but `insightReviewId` isn't in `quizState`. Use a ref:

```typescript
const insightReviewIdRef = useRef(state.insightReviewId);
insightReviewIdRef.current = state.insightReviewId;
```

Then in `revealAll`:
```typescript
const reviewId = insightReviewIdRef.current;
if (reviewId && questions.length > 0) {
    ipc("note_insight_submit_quiz", { ... }).catch(() => {});
}
```

- [ ] **Step 7: Build + test**

Run: `cargo build --workspace`
Run: `cd desktop-ui && bun run build`
Run: `cargo nextest run -p desktop -E 'test(dev_server)'`

- [ ] **Step 8: Commit**

```bash
git add crates/ desktop-ui/
git commit -m "feat: persist quiz answers and compute real quiz_score in progress tracking"
```

---

## Chunk 6: Minor Fixes

### Task 6: Delete orphaned `PersonaSelector.tsx`

**Files:**
- Delete: `desktop-ui/src/features/notes/components/insight/PersonaSelector.tsx`

- [ ] **Step 1: Verify it's unused**

Grep for imports of `PersonaSelector` across the codebase. If zero external imports, delete it.

- [ ] **Step 2: Delete + commit**

```bash
git rm desktop-ui/src/features/notes/components/insight/PersonaSelector.tsx
git commit -m "chore: remove orphaned PersonaSelector component"
```

---

### Task 7: Populate `InsightEvolutionResponse.note_title`

**Files:**
- Modify: `crates/app-core/src/handlers/notes/insight.rs`

Currently hardcoded to `String::new()`. Populate it with a cheap `get_note` call to avoid a breaking API change.

- [ ] **Step 1: Restore the note fetch in `note_insight_get_evolution`**

Add back the note lookup (single DB query, ~0.1ms) and populate `note_title`:

```rust
let note_title = self
    .note_repo
    .get_note(note_id)
    .await
    .ok()
    .flatten()
    .map(|n| n.title)
    .unwrap_or_default();

// ... existing evolution code ...

Ok(InsightEvolutionResponse {
    note_id: note_id.to_string(),
    note_title,
    versions: points,
})
```

- [ ] **Step 2: Build + commit**

```bash
git add crates/app-core/src/handlers/notes/insight.rs
git commit -m "fix(app-core): populate note_title in InsightEvolutionResponse"
```

---

### Task 8: Remove dead backlink fallback in insight handler

**Files:**
- Modify: `crates/app-core/src/handlers/notes/insight.rs`

After Tasks 1 and 3, the `get_related_note_ids` and `fetch_related_notes` private methods should only be reachable from fallback/error paths. Verify and clean up.

- [ ] **Step 1: After Tasks 1+3 are done, grep for call sites**

Search for `get_related_note_ids` and `fetch_related_notes` in insight.rs. If only used in `else` branches that fire when `insight_service` is `None` (which never happens), remove or keep as error fallbacks only.

Note: `fetch_related_notes` may still be needed by the error fallback in Task 3. Don't remove it if it's still referenced.

- [ ] **Step 2: Clean up + commit**

```bash
git add crates/app-core/src/handlers/notes/insight.rs
git commit -m "refactor(app-core): clean up dead backlink fallback paths in insight handler"
```

---

## Chunk 7: Verification

### Task 9: Full verification

- [ ] **Step 1: Backend tests**

Run: `cargo nextest run --workspace`
Expected: all tests pass.

- [ ] **Step 2: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: no new warnings.

- [ ] **Step 3: Format**

Run: `cargo fmt --all`
Run: `cd desktop-ui && bun run lint:fix`

- [ ] **Step 4: Frontend build + test**

Run: `cd desktop-ui && bun run build`
Run: `cd desktop-ui && bun run test`

- [ ] **Step 5: Manual smoke test**

Start: `cargo tauri dev`

1. Open a note with backlinks → click Insight Review → verify synthesis references related notes (not just the single note body)
2. Change scope to Semantic → click Regenerate → verify different related notes are pulled in
3. Change scope to Project → click Regenerate → verify notes from same notebook appear
4. Toggle Deep Dive ON → click Regenerate → verify richer synthesis (should mention user model, entity connections, knowledge evolution)
5. Open History → verify evolution chart shows data with version(s)
6. Click Scope Config → toggle settings → click Regenerate → verify scope hint updates
7. Complete a Self-Assessment quiz → reveal answers one-by-one → verify score displays correctly (total should not be 0)
8. Reveal all → verify quiz score is submitted to backend (check progress snapshot in DB)
9. Regenerate a single tab → verify it uses the same rich context as the original generation
10. In browser dev mode (localhost:1420): open a non-cached note → verify streaming synthesis arrives via SSE
