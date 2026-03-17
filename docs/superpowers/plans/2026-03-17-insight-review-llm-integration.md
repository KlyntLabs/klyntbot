# Insight Review LLM Integration (Phase 2.5) Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire the Insight Review handler to make actual LLM calls, stream synthesis results via Tauri events, generate quiz/gap/concept-map content in parallel, cache results, and implement the regenerate-tab command — turning the empty UI shell into a working feature.

**Architecture:** The insight handler in `app-core` spawns a `tokio::spawn` background task that: (1) assembles context from the note + related notes + cognitive memory, (2) streams Tab 1 (Synthesis) via `AppEventEmitter` Tauri events, (3) fires Tabs 2-4 as parallel structured JSON LLM calls, (4) emits `insight:tab-done` for each completed tab, (5) caches all results in `InsightCacheRepo`. Uses the existing `cognitive_provider` (`DynProvider`) for LLM calls and `AppEventEmitter` trait for Tauri event emission.

**Tech Stack:** Rust (tokio, providers::LlmProvider, serde_json), futures_util for streaming.

**Spec:** `docs/superpowers/specs/2026-03-16-insight-review-design.md` (§9: LLM Prompts, §1: Loading Strategy)

**Depends on:** Phase 2 (complete — UI shell, handler stub, IPC commands, hook, tab components)

---

## File Structure

### New files

| File | Responsibility |
|------|---------------|
| `crates/app-core/src/handlers/notes/insight_prompts.rs` | LLM prompt templates for all 4 tabs |
| `crates/app-core/src/handlers/notes/insight_context.rs` | Context assembly: note + related notes + memory into prompt context block |

### Modified files

| File | Change |
|------|--------|
| `crates/app-core/src/handlers/notes/insight.rs` | Replace TODO with actual LLM pipeline: spawn background task, stream, cache |
| `crates/app-core/src/handlers/notes/mod.rs` | Register new modules |

---

## Chunk 1: Prompt Templates + Context Assembly

### Task 1: LLM Prompt Templates

**Files:**
- Create: `crates/app-core/src/handlers/notes/insight_prompts.rs`

- [ ] **Step 1: Create prompt templates module**

Create `crates/app-core/src/handlers/notes/insight_prompts.rs`:

```rust
//! LLM prompt templates for the 4 Insight Review tabs.
//! Each function takes an assembled context block and returns a system prompt.

/// Tab 1: Synthesis — streaming markdown response.
pub fn synthesis_prompt(context: &str) -> String {
    format!(
        r#"You are a research synthesis assistant. Given the user's note and its related notes from their knowledge base, write a deep synthesis that:

1. Identifies the 3-5 key themes across these notes
2. Draws non-obvious connections between concepts
3. Highlights where ideas reinforce or build on each other
4. Surfaces insights the user may not have explicitly written

Format as clean Markdown with ## headings for each theme.
Keep it focused and insightful — not a summary, but a synthesis.
Do not repeat content verbatim from the notes.

--- BEGIN KNOWLEDGE CONTEXT ---
{context}
--- END KNOWLEDGE CONTEXT ---"#
    )
}

/// Tab 2: Gap Analysis — markdown + trailing JSON block.
pub fn gap_analysis_prompt(context: &str) -> String {
    format!(
        r#"You are a knowledge gap analyst. Given the user's note cluster, identify:

1. **Missing concepts** — important topics referenced but never explored in depth
2. **Contradictions** — places where notes disagree or present conflicting info
3. **Shallow coverage** — topics mentioned briefly that deserve deeper treatment
4. **Research suggestions** — specific papers, books, or topics to explore next
5. **Notes to create** — suggest 2-3 new note titles that would strengthen the network

Format as Markdown with clear sections. Be specific and actionable.
For each gap, reference which note(s) it relates to.

ALSO return a machine-readable JSON block at the end, wrapped in ```json fences:
[{{"topic": "short title", "description": "1-2 sentence description", "suggestedTitle": "New Note: ..."}}]

--- BEGIN KNOWLEDGE CONTEXT ---
{context}
--- END KNOWLEDGE CONTEXT ---"#
    )
}

/// Tab 3: Self-Assessment — pure JSON response.
pub fn self_assessment_prompt(context: &str) -> String {
    format!(
        r#"You are an educational assessment designer. Generate a self-assessment quiz based on the user's knowledge network.

Generate exactly 8 questions:
- 4 multiple choice (4 options each, one correct)
- 4 short answer (expecting 1-2 sentence responses)

For each question, include:
- A unique short id (e.g. "q1", "q2")
- The question text
- The correct answer
- A brief explanation of why
- Which note(s) the question draws from
- Difficulty: "easy", "medium", or "hard"
- Difficulty score: 0.0-1.0 (for FSRS initialization)

Questions should test understanding, not memorization. Include questions that require connecting ideas across multiple notes.

Respond ONLY with a JSON array (no markdown, no explanation):
[{{"id": "q1", "type": "multiple_choice", "question": "...", "choices": ["A", "B", "C", "D"], "correct_answer": "...", "explanation": "...", "source_notes": ["note title"], "difficulty": "medium", "difficulty_score": 0.5}}]

--- BEGIN KNOWLEDGE CONTEXT ---
{context}
--- END KNOWLEDGE CONTEXT ---"#
    )
}

/// Tab 4: Concept Map — mermaid mindmap syntax.
pub fn concept_map_prompt(context: &str, root_title: &str) -> String {
    format!(
        r#"You are a concept mapping specialist. Create a Mermaid mindmap diagram showing how concepts connect across the user's note cluster.

Rules:
- Use Mermaid mindmap syntax exactly
- Root node = root(({root_title}))
- Branch into major themes/concepts
- Show connections to related notes by name
- Max 4 levels deep, max 5-6 branches per node
- Max 6 words per node label
- Use clean, short labels (no full sentences)

If you cannot generate valid Mermaid syntax, return a clean indented text outline instead, prefixed with "FALLBACK:" on the first line.

Example format:
mindmap
  root((Machine Learning Notes))
    Supervised Learning
      Regression
      Classification
    Neural Networks
      Deep Learning
      Transformers

--- BEGIN KNOWLEDGE CONTEXT ---
{context}
--- END KNOWLEDGE CONTEXT ---"#
    )
}
```

- [ ] **Step 2: Register module**

In `crates/app-core/src/handlers/notes/mod.rs`, add:
```rust
mod insight_prompts;
```

- [ ] **Step 3: Build**

Run: `cargo build -p app-core`

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/handlers/notes/insight_prompts.rs crates/app-core/src/handlers/notes/mod.rs
git commit -m "feat(app-core): add LLM prompt templates for Insight Review tabs"
```

---

### Task 2: Context Assembly

**Files:**
- Create: `crates/app-core/src/handlers/notes/insight_context.rs`
- Modify: `crates/app-core/src/handlers/notes/mod.rs`

This module assembles a context block from the note, its related notes, and cognitive memory for use in prompts.

- [ ] **Step 1: Create context assembly module**

Create `crates/app-core/src/handlers/notes/insight_context.rs`:

```rust
//! Assemble a knowledge context block for Insight Review prompts.
//!
//! Gathers: current note, related notes (backlinks), and optionally
//! cognitive memory entries. Formats into a structured text block
//! that LLM prompts can consume.

use feature_notes::models::NoteRow;

/// Assembled context ready for prompt injection.
pub struct InsightContext {
    pub text: String,
    pub note_title: String,
    pub related_count: usize,
}

/// Build a context block from a note and its related notes.
pub fn assemble_context(
    note: &NoteRow,
    related_notes: &[NoteRow],
    memory_entries: Option<&[String]>,
) -> InsightContext {
    let mut parts = Vec::new();

    // Current note
    parts.push(format!(
        "## Current Note: {}\n\n{}",
        note.title, note.body
    ));

    // Related notes
    for related in related_notes {
        let body_preview = if related.body.len() > 2000 {
            format!("{}...", &related.body[..2000])
        } else {
            related.body.clone()
        };
        parts.push(format!(
            "## Related Note: {}\n\n{}",
            related.title, body_preview
        ));
    }

    // Cognitive memory (if available)
    if let Some(entries) = memory_entries {
        if !entries.is_empty() {
            parts.push("## Relevant Memory".to_string());
            for entry in entries {
                parts.push(format!("- {entry}"));
            }
        }
    }

    InsightContext {
        text: parts.join("\n\n"),
        note_title: note.title.clone(),
        related_count: related_notes.len(),
    }
}
```

- [ ] **Step 2: Register module**

In `crates/app-core/src/handlers/notes/mod.rs`, add:
```rust
mod insight_context;
```

- [ ] **Step 3: Build**

Run: `cargo build -p app-core`

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/handlers/notes/insight_context.rs crates/app-core/src/handlers/notes/mod.rs
git commit -m "feat(app-core): add context assembly for Insight Review prompts"
```

---

## Chunk 2: LLM Pipeline + Streaming

### Task 3: Wire the Insight Handler to LLM

**Files:**
- Modify: `crates/app-core/src/handlers/notes/insight.rs` (full rewrite of `note_insight_review` and `note_insight_regenerate_tab`)

This is the core task. The handler needs to:
1. Assemble context from note + related notes
2. Spawn a background tokio task
3. Stream Tab 1 (Synthesis) via `emit_event`
4. Fire Tabs 2-4 in parallel as structured calls
5. Emit `insight:tab-done` for each
6. Cache results

**Key patterns to follow:**
- `AppCore` has `cognitive_provider: Option<providers::DynProvider>` for LLM calls
- `AppCore` has `event_emitter: Arc<dyn AppEventEmitter>` for Tauri events
- `providers::cognitive_chat_params(config, max_tokens)` builds `ChatParams`
- `provider.chat(messages, None, &params)` for non-streaming calls
- `provider.chat_stream(messages, None, &params)` for streaming (returns `LlmStream`)
- `providers::Message::System { content }` and `providers::Message::User { content: providers::UserContent::Text(text) }` for messages
- `event_emitter.emit_event("event:name", serde_json::json!({...}))` for Tauri events

- [ ] **Step 1: Rewrite the insight handler**

Replace the contents of `crates/app-core/src/handlers/notes/insight.rs` with the full LLM-integrated implementation.

The key changes:
1. `note_insight_review`: after cache check, assemble context, then `tokio::spawn` a background task
2. The background task calls `run_insight_pipeline` which:
   - Streams synthesis via `chat_stream` → emits `insight:synthesis-chunk` events → emits `insight:synthesis-done`
   - Fires gap_analysis, self_assessment, concept_map as parallel `chat` calls → emits `insight:tab-done` for each
   - Caches all results in `InsightCacheRepo`
   - On error, emits `insight:error`
3. `note_insight_regenerate_tab`: re-runs a single tab's LLM call

The handler should:
- Read config via `self.config.read().await` to get `ChatParams`
- Clone `Arc` references for the spawned task (provider, emitter, repos)
- Use `futures_util::StreamExt` for consuming the LLM stream
- Gracefully handle missing `cognitive_provider` (return error "LLM provider not configured")

**Important implementation notes:**
- `providers::UserContent` has a `Text(String)` variant for plain text user messages
- The config is `RwLock<config::Config>` — read lock is `.read().await`
- The `NoteRepo` has `get_note(id)` returning `Option<NoteRow>`
- Related notes come from `get_backlinks_with_context` which returns `Vec<(NoteRow, Option<String>)>`
- For fetching related note bodies, use `get_note` on each backlink ID
- `InsightCacheRepo.upsert(note_id, content_hash, synthesis, gap_analysis, self_assessment, concept_map)` for caching
- Self-assessment content should be cached as JSON string (`serde_json::to_string(&questions)`)
- The spawned task must be `'static` — all borrows must be cloned into owned values

The implementer should read these files for patterns:
- `crates/app-core/src/handlers/cognitive/memory.rs` — how cognitive_provider is used
- `crates/app-core/src/events.rs` — AppEventEmitter trait
- `crates/providers/src/types.rs` — Message enum, ChatParams, LlmStream
- `crates/app-core/src/handlers/notes/insight_prompts.rs` — the prompt templates (just created)
- `crates/app-core/src/handlers/notes/insight_context.rs` — context assembly (just created)

**Event names for the frontend (must match `useInsightReview.ts`):**
- `"insight:synthesis-chunk"` — payload: `{ "content": "incremental text" }`
- `"insight:synthesis-done"` — payload: `{}`
- `"insight:tab-done"` — payload: `{ "tab": "gaps"|"assessment"|"concept-map", "content": "full content" }`
- `"insight:error"` — payload: `{ "tab": "synthesis"|"gaps"|"assessment"|"concept-map", "error": "message" }`

- [ ] **Step 2: Build**

Run: `cargo build -p app-core`
Expected: compiles.

- [ ] **Step 3: Build workspace**

Run: `cargo build --workspace`
Expected: compiles (verify no downstream breakage).

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/handlers/notes/insight.rs
git commit -m "feat(app-core): wire Insight Review handler to LLM pipeline with streaming"
```

---

### Task 4: Final Verification

- [ ] **Step 1: Full tests**

Run: `cargo nextest run --workspace`
Expected: all tests pass.

- [ ] **Step 2: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: no new warnings.

- [ ] **Step 3: Format**

Run: `cargo fmt --all`

- [ ] **Step 4: Frontend build**

Run: `cd desktop-ui && bun run build`
Expected: builds (no frontend changes in this phase, but verify nothing broke).

- [ ] **Step 5: Commit if needed**

```bash
cargo fmt --all
git add -A && git commit -m "style: format Phase 2.5 implementation"
```
