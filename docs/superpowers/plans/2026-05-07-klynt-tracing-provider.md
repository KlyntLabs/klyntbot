# Klynt Tracing Provider — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire Klynt's coding-mode runtime events into the existing `coding-ingest` + `tracing` pipeline so Klynt sessions appear in the same Tracing UI that already serves Claude Code, Codex, Kimi CLI, and OpenCode.

**Architecture:** Four narrow gaps closed: (1) extend the existing `RuntimeEvent` enum with `MidLoopCompressionTriggered`, (2) add a `agent::AgentEvent` → `RuntimeEvent` mapper called from `turn_handler.rs`, (3) build `KlyntTracingProvider` mirroring `KimiTracingProvider` but reading from the existing SQLite, (4) add a frontend provider chip-row replacing the hardcoded `PROVIDER_ID = "kimi"`.

**Tech Stack:** Rust 1.93 (workspace crates), Tauri 2, sqlx, React 19 + TypeScript, Vitest.

**Spec:** `docs/superpowers/specs/2026-05-07-klynt-tracing-provider-design.md`

---

## File Structure

### New Rust files
- `crates/app-core/src/coding/runtime_event_translator.rs` — `agent::AgentEvent` → `RuntimeEvent` mapper
- `crates/app-core/src/tracing/providers/klynt/mod.rs`
- `crates/app-core/src/tracing/providers/klynt/provider_impl.rs`
- `crates/app-core/src/tracing/providers/klynt/discovery.rs`
- `crates/app-core/src/tracing/providers/klynt/loader.rs`
- `crates/app-core/src/tracing/providers/klynt/context_loader.rs`
- `crates/app-core/src/tracing/providers/klynt/state_loader.rs`
- `crates/app-core/src/tracing/providers/klynt/subagent_loader.rs`
- `crates/app-core/src/tracing/providers/klynt/summary.rs`
- `crates/app-core/src/tracing/providers/klynt/stats.rs`

### Modified Rust files
- `crates/coding-memory/src/sink/translator.rs` — add `RuntimeEvent::MidLoopCompressionTriggered` variant + match arm
- `crates/app-core/src/coding/mod.rs` — add `pub mod runtime_event_translator;`
- `crates/app-core/src/coding/turn_handler.rs` — wire Translator → Distiller in event-bridge task
- `crates/app-core/src/tracing/mod.rs` (or `providers/mod.rs`) — add `pub mod klynt;`
- `crates/app-core/src/init/*.rs` — register `KlyntTracingProvider` (exact init file determined in Task 11)
- `crates/app-core/src/tracing_handlers.rs` — add `tracing_list_providers` handler
- `crates/desktop/src/commands/tracing.rs` — add `tracing_list_providers` Tauri command
- `crates/desktop/src/specta_builder.rs` — list new command in `klynt_collect_commands![...]`
- `crates/coding-ingest/tests/cross_cli_normalization.rs` — extend proptest with KlyntCli source

### Modified TypeScript files
- `desktop-ui/src/tracing/lib/api.ts` — replace `PROVIDER_ID` constant with `getCurrentProviderId()` + `listProviders()`
- `desktop-ui/src/tracing/index.tsx` — add `<ProviderChips />` component to header

### New TypeScript files
- `desktop-ui/src/tracing/components/provider-chips.tsx`
- `desktop-ui/src/tracing/components/provider-chips.test.tsx`

---

# Phase 1 — Extend RuntimeEvent enum

## Task 1: Add `MidLoopCompressionTriggered` variant

**Files:**
- Modify: `crates/coding-memory/src/sink/translator.rs`

- [ ] **Step 1: Read existing RuntimeEvent enum**

Run: `grep -n "pub enum RuntimeEvent" /Users/jayden/Projects/Klynt/bot/crates/coding-memory/src/sink/translator.rs`
Expected: line ~20.

- [ ] **Step 2: Add the new variant at the end of the enum**

In `crates/coding-memory/src/sink/translator.rs`, after the `ApprovalResolved` variant (around line 120, before the closing `}`):

```rust
    /// Mid-loop compression was triggered.
    MidLoopCompressionTriggered {
        /// Tokens before compression.
        before_tokens: u32,
        /// Tokens after compression.
        after_tokens: u32,
        /// Number of messages condensed.
        messages_condensed: u32,
    },
```

- [ ] **Step 3: Add the translator match arm**

In the `Translator::translate` match block (around line 140), add a new arm before the closing `}` of the match:

```rust
            RuntimeEvent::MidLoopCompressionTriggered {
                before_tokens,
                after_tokens,
                messages_condensed,
            } => vec![EventKind::CompressionApplied {
                before_tokens: *before_tokens,
                after_tokens: *after_tokens,
                messages_condensed: *messages_condensed,
            }],
```

- [ ] **Step 4: Verify EventKind::CompressionApplied exists**

Run: `grep -n "CompressionApplied" /Users/jayden/Projects/Klynt/bot/crates/coding-ingest/src/event.rs`
Expected: at least one hit. If absent, add it to `EventKind` per active spec §10.

- [ ] **Step 5: Build to verify**

Run: `cargo build -p coding-memory`
Expected: clean build.

- [ ] **Step 6: Commit**

```bash
git add crates/coding-memory/src/sink/translator.rs
git commit -m "feat(coding-memory): add MidLoopCompressionTriggered RuntimeEvent variant"
```

---

## Task 2: Add unit test for the new translator path

**Files:**
- Modify: `crates/coding-memory/src/sink/translator.rs` (test module)

- [ ] **Step 1: Find the existing test module**

Run: `grep -n "#\[cfg(test)\]" /Users/jayden/Projects/Klynt/bot/crates/coding-memory/src/sink/translator.rs`
Expected: a `#[cfg(test)] mod tests` block exists (or in a sibling `*_tests.rs` file). If none, create it at the end of the file.

- [ ] **Step 2: Add the test**

```rust
#[cfg(test)]
mod tests_compression {
    use super::*;

    #[test]
    fn translates_compression_event() {
        let mut t = Translator::new();
        let evt = RuntimeEvent::MidLoopCompressionTriggered {
            before_tokens: 50_000,
            after_tokens: 20_000,
            messages_condensed: 12,
        };
        let out = t.translate(&evt).expect("translate");
        assert_eq!(out.len(), 1);
        let coding_ingest::event::AgentEvent::V1(v1) = &out[0];
        assert_eq!(v1.source, AgentSource::KlyntCli);
        match &v1.kind {
            EventKind::CompressionApplied {
                before_tokens,
                after_tokens,
                messages_condensed,
            } => {
                assert_eq!(*before_tokens, 50_000);
                assert_eq!(*after_tokens, 20_000);
                assert_eq!(*messages_condensed, 12);
            }
            other => panic!("expected CompressionApplied, got {other:?}"),
        }
    }
}
```

- [ ] **Step 3: Run the test**

Run: `cargo nextest run -p coding-memory tests_compression`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/coding-memory/src/sink/translator.rs
git commit -m "test(coding-memory): cover MidLoopCompressionTriggered translation"
```

---

# Phase 2 — Wire turn_handler into the Translator

## Task 3: Create the runtime_event_translator module

**Files:**
- Create: `crates/app-core/src/coding/runtime_event_translator.rs`
- Modify: `crates/app-core/src/coding/mod.rs`

- [ ] **Step 1: Create the new module file**

Write to `crates/app-core/src/coding/runtime_event_translator.rs`:

```rust
//! Maps runtime `agent::AgentEvent` into `coding_memory::sink::translator::RuntimeEvent`.
//!
//! Returns `None` for runtime-only variants that have no ingest counterpart
//! per the active spec §10 (IterationStart, ToolCallStreamChunk, PowerModeToggled,
//! ReasoningChunk, Done, TurnComplete, UsageReport, Error).

use agent::events::AgentEvent;
use coding_memory::sink::translator::RuntimeEvent;

#[must_use]
pub fn agent_event_to_runtime_event(evt: &AgentEvent) -> Option<RuntimeEvent> {
    match evt {
        AgentEvent::ContentChunk { data, .. } => Some(RuntimeEvent::ContentChunk {
            text: data.clone(),
        }),
        AgentEvent::ToolStart {
            call_id,
            name,
            args,
            ..
        } => Some(RuntimeEvent::ToolStart {
            call_id: call_id.clone(),
            name: name.clone(),
            args: args.clone(),
        }),
        AgentEvent::ToolEnd {
            call_id,
            success,
            output,
            duration_ms,
            ..
        } => Some(RuntimeEvent::ToolEnd {
            call_id: call_id.clone(),
            success: *success,
            output: output.clone(),
            duration_ms: *duration_ms,
        }),
        AgentEvent::ContextCompressed {
            before_tokens,
            after_tokens,
            messages_condensed,
            ..
        } => Some(RuntimeEvent::MidLoopCompressionTriggered {
            before_tokens: *before_tokens,
            after_tokens: *after_tokens,
            messages_condensed: *messages_condensed,
        }),
        AgentEvent::FileEditWithSymbols {
            path,
            op,
            bytes,
            anchored_symbols,
            lsp_diagnostics_delta,
            ..
        } => Some(RuntimeEvent::FileEditWithSymbols {
            path: path.clone(),
            op: op.clone(),
            bytes: *bytes,
            anchored_symbols: anchored_symbols.clone(),
            lsp_diagnostics_delta: lsp_diagnostics_delta.clone(),
        }),
        // Runtime-only — no ingest counterpart:
        AgentEvent::ReasoningChunk { .. }
        | AgentEvent::Done { .. }
        | AgentEvent::TurnComplete { .. }
        | AgentEvent::UsageReport { .. }
        | AgentEvent::Error { .. } => None,
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_content_chunk() {
        let ag = AgentEvent::ContentChunk {
            data: "hello".to_string(),
            ..Default::default()
        };
        match agent_event_to_runtime_event(&ag) {
            Some(RuntimeEvent::ContentChunk { text }) => assert_eq!(text, "hello"),
            other => panic!("expected ContentChunk, got {other:?}"),
        }
    }

    #[test]
    fn returns_none_for_done() {
        let ag = AgentEvent::Done {
            ..Default::default()
        };
        assert!(agent_event_to_runtime_event(&ag).is_none());
    }
}
```

- [ ] **Step 2: Wire it into the coding module**

In `crates/app-core/src/coding/mod.rs`, add:

```rust
pub mod runtime_event_translator;
```

- [ ] **Step 3: Build to verify**

Run: `cargo build -p app-core`
Expected: clean build (some compile errors expected if `Default` impls are missing on `AgentEvent` variants — adapt the test inputs to the real fields if so).

- [ ] **Step 4: Run the unit tests**

Run: `cargo nextest run -p app-core runtime_event_translator`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/coding/runtime_event_translator.rs crates/app-core/src/coding/mod.rs
git commit -m "feat(coding): add agent::AgentEvent → RuntimeEvent mapper"
```

---

## Task 4: Wire the translator + Distiller into turn_handler.rs

**Files:**
- Modify: `crates/app-core/src/coding/turn_handler.rs`

- [ ] **Step 1: Locate the bridge task and Distiller handle**

Run: `grep -n "Bridge AgentEvent → ThreadEvent\|distiller\|Distiller" /Users/jayden/Projects/Klynt/bot/crates/app-core/src/coding/turn_handler.rs`
Expected: bridge marker at ~line 163. Confirm whether `distiller: Arc<Distiller>` is already in scope. If not, plumb it through the calling function's signature.

- [ ] **Step 2: Add imports at the top of the file**

In `crates/app-core/src/coding/turn_handler.rs`, add to the import block:

```rust
use crate::coding::runtime_event_translator::agent_event_to_runtime_event;
use coding_memory::sink::translator::Translator as IngestTranslator;
use coding_memory::distiller::Distiller;
use std::sync::Arc;
```

- [ ] **Step 3: Construct an ingest translator alongside the existing one**

Just before the `while let Some(ag_evt) = event_rx.recv().await {` loop (around line 160), add:

```rust
let mut ingest_translator = IngestTranslator::new();
```

Ensure a `distiller: Arc<Distiller>` is in scope at this point. If absent, add a parameter to the function signature and update the caller in `crates/app-core/src/handlers/coding/*.rs`.

- [ ] **Step 4: Add the parallel ingest path inside the loop**

Inside the existing `while` loop, immediately after the existing `match` that emits `ThreadEvent`s (so it doesn't perturb the existing UI flow), add:

```rust
if let Some(rt_evt) = agent_event_to_runtime_event(&ag_evt) {
    match ingest_translator.translate(&rt_evt) {
        Ok(ingest_events) => {
            for mut evt in ingest_events {
                if let coding_ingest::event::AgentEvent::V1(ref mut v1) = evt {
                    v1.session_id = session_id.clone();
                    v1.turn_id = Some(turn_id.clone());
                    v1.cwd = cwd.clone();
                }
                if let Err(e) = distiller.accept_event(evt).await {
                    tracing::warn!(?e, "distiller rejected ingest event; continuing");
                }
            }
        }
        Err(e) => tracing::warn!(?e, "translator error; skipping event"),
    }
}
```

- [ ] **Step 5: Build to verify**

Run: `cargo build -p app-core`
Expected: clean build. If `Distiller::accept_event` signature differs, adapt the call.

- [ ] **Step 6: Run existing turn_handler tests**

Run: `cargo nextest run -p app-core turn_handler`
Expected: PASS — existing tests should not regress.

- [ ] **Step 7: Commit**

```bash
git add crates/app-core/src/coding/turn_handler.rs
git commit -m "feat(coding): emit AgentEvents to Distiller via Translator"
```

---

# Phase 3 — Build KlyntTracingProvider

## Task 5: Create the klynt provider module skeleton

**Files:**
- Create: `crates/app-core/src/tracing/providers/klynt/mod.rs`
- Create: `crates/app-core/src/tracing/providers/klynt/provider_impl.rs`
- Modify: `crates/app-core/src/tracing/providers/mod.rs`

- [ ] **Step 1: Read the kimi provider's mod.rs as reference**

Run: `cat /Users/jayden/Projects/Klynt/bot/crates/app-core/src/tracing/providers/kimi/mod.rs`
Expected: 16 lines, declares 11 sub-modules, re-exports `KimiTracingProvider`.

- [ ] **Step 2: Create the klynt provider mod.rs**

Write `crates/app-core/src/tracing/providers/klynt/mod.rs`:

```rust
//! Klynt-cli TracingProvider implementation.

pub mod context_loader;
pub mod discovery;
pub mod loader;
mod provider_impl;
pub mod state_loader;
pub mod stats;
pub mod subagent_loader;
pub mod summary;

pub use provider_impl::KlyntTracingProvider;
```

- [ ] **Step 3: Create the provider_impl skeleton**

Write `crates/app-core/src/tracing/providers/klynt/provider_impl.rs`:

```rust
//! KlyntTracingProvider — reads from the existing SQLite store.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use common::Result;
use storage::Repos;

use crate::tracing::provider::TracingProvider;
use crate::tracing::types::{
    ContextMessage, Scope, SessionDetail, SessionState, SessionSummary, StatsBundle,
    SubagentSummary,
};

pub struct KlyntTracingProvider {
    repos: Arc<Repos>,
}

impl KlyntTracingProvider {
    pub fn new(repos: Arc<Repos>) -> Self {
        Self { repos }
    }
}

#[async_trait]
impl TracingProvider for KlyntTracingProvider {
    fn id(&self) -> &'static str {
        "klynt"
    }

    fn display_name(&self) -> &'static str {
        "Klynt"
    }

    async fn list_sessions(&self) -> Result<Vec<SessionSummary>> {
        super::discovery::list_klynt_sessions(&self.repos).await
    }

    async fn load_session(
        &self,
        session_id: &str,
        scope: Scope,
    ) -> Result<SessionDetail> {
        super::loader::load_klynt_session(&self.repos, session_id, scope).await
    }

    async fn load_context(
        &self,
        session_id: &str,
        scope: Scope,
    ) -> Result<Vec<ContextMessage>> {
        super::context_loader::load_klynt_context(&self.repos, session_id, scope).await
    }

    async fn load_state(&self, session_id: &str) -> Result<SessionState> {
        super::state_loader::load_klynt_state(&self.repos, session_id).await
    }

    async fn list_subagents(&self, session_id: &str) -> Result<Vec<SubagentSummary>> {
        super::subagent_loader::list_klynt_subagents(&self.repos, session_id).await
    }

    async fn import_from_file(&self, _path: &Path) -> Result<String> {
        Err(common::KlyntbotError::Unsupported(
            "import not supported for klynt provider".into(),
        ))
    }

    async fn open_dir(&self, _session_id: &str) -> Result<PathBuf> {
        // Klynt sessions live in SQLite, not on disk per session.
        // Return the data dir.
        Ok(self.repos.data_dir().to_path_buf())
    }

    async fn stats(&self) -> Result<StatsBundle> {
        super::stats::aggregate_stats(&self.repos).await
    }

    async fn session_summary(&self, session_id: &str) -> Result<SessionSummary> {
        super::summary::session_summary(&self.repos, session_id).await
    }

    async fn load_subagent_session(
        &self,
        session_id: &str,
        agent_id: &str,
    ) -> Result<SessionDetail> {
        super::loader::load_klynt_session(
            &self.repos,
            session_id,
            Scope::Subagent {
                agent_id: agent_id.to_string(),
            },
        )
        .await
    }

    async fn load_subagent_context(
        &self,
        session_id: &str,
        agent_id: &str,
    ) -> Result<Vec<ContextMessage>> {
        super::context_loader::load_klynt_context(
            &self.repos,
            session_id,
            Scope::Subagent {
                agent_id: agent_id.to_string(),
            },
        )
        .await
    }
}
```

- [ ] **Step 4: Wire into providers/mod.rs**

In `crates/app-core/src/tracing/providers/mod.rs`, add:

```rust
pub mod klynt;
```

- [ ] **Step 5: Build to verify (will fail until sub-modules exist)**

Run: `cargo build -p app-core 2>&1 | tail -30`
Expected: failures pointing to missing `discovery`, `loader`, etc. — that's fine; we add them in subsequent tasks.

- [ ] **Step 6: Commit**

```bash
git add crates/app-core/src/tracing/providers/klynt/mod.rs \
        crates/app-core/src/tracing/providers/klynt/provider_impl.rs \
        crates/app-core/src/tracing/providers/mod.rs
git commit -m "feat(tracing): scaffold KlyntTracingProvider"
```

---

## Task 6: Implement discovery (list_klynt_sessions)

**Files:**
- Create: `crates/app-core/src/tracing/providers/klynt/discovery.rs`

- [ ] **Step 1: Write the discovery module**

Write `crates/app-core/src/tracing/providers/klynt/discovery.rs`:

```rust
//! list_klynt_sessions — query coding-mode sessions from the SQLite store.

use std::sync::Arc;

use common::Result;
use storage::Repos;

use crate::tracing::types::{SessionMetadata, SessionSummary};

pub async fn list_klynt_sessions(repos: &Arc<Repos>) -> Result<Vec<SessionSummary>> {
    let pool = repos.pool();
    let rows = sqlx::query!(
        r#"
        SELECT
          s.id as "session_id!",
          s.title,
          s.cwd,
          s.repo_id,
          s.created_at as "created_at!",
          s.updated_at as "updated_at!",
          (SELECT COUNT(*) FROM coding_ingest_events e
           WHERE e.session_id = s.id AND e.source = 'klynt-cli') as "event_count!",
          (SELECT COUNT(*) FROM coding_ingest_events e
           WHERE e.session_id = s.id AND e.source = 'klynt-cli'
             AND e.kind = 'TurnBegin') as "turn_count!"
        FROM sessions s
        WHERE s.conversation_type = 'coding'
        ORDER BY s.updated_at DESC
        LIMIT 1000
        "#
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| SessionSummary {
            session_id: r.session_id.clone(),
            session_dir: format!("klynt://{}", r.session_id),
            work_dir: r.cwd.clone(),
            work_dir_hash: r.repo_id.clone().unwrap_or_default(),
            title: r.title.unwrap_or_else(|| "Untitled session".to_string()),
            last_updated: r.updated_at,
            has_wire: r.event_count > 0,
            has_context: true,
            has_state: true,
            wire_size: r.event_count as u64,
            context_size: 0,
            state_size: 0,
            total_size: 0,
            turns: r.turn_count as u32,
            metadata: Some(SessionMetadata {
                session_id: r.session_id,
                title: String::new(),
                title_generated: false,
                archived: false,
                archived_at: None,
                auto_archive_exempt: false,
                wire_mtime: Some(r.updated_at),
            }),
            imported: false,
            subagent_count: 0,
        })
        .collect())
}
```

- [ ] **Step 2: Confirm the query column names match the schema**

Run: `find /Users/jayden/Projects/Klynt/bot/crates -path "*/migrations/*" -name "*.sql" | xargs grep -l "coding_ingest_events"` and inspect the relevant migration file. Adjust column names in the query if they differ (e.g., `kind` may be `event_kind`).

- [ ] **Step 3: Adjust types if `SessionSummary` field shapes differ**

Run: `grep -n "pub struct SessionSummary" /Users/jayden/Projects/Klynt/bot/crates/app-core/src/tracing/types.rs`
Inspect the actual fields and adapt the construction above to match.

- [ ] **Step 4: Build**

Run: `cargo build -p app-core`
Expected: discovery module compiles. Sub-modules below still missing.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/tracing/providers/klynt/discovery.rs
git commit -m "feat(tracing): klynt provider discovery query"
```

---

## Task 7: Implement loader (load_klynt_session)

**Files:**
- Create: `crates/app-core/src/tracing/providers/klynt/loader.rs`

- [ ] **Step 1: Write the loader module**

Write `crates/app-core/src/tracing/providers/klynt/loader.rs`:

```rust
//! load_klynt_session — read events for a single session.

use std::sync::Arc;

use common::Result;
use storage::Repos;

use crate::tracing::types::{Scope, SessionDetail, TraceEvent};

pub async fn load_klynt_session(
    repos: &Arc<Repos>,
    session_id: &str,
    scope: Scope,
) -> Result<SessionDetail> {
    let pool = repos.pool();
    let agent_id_filter: Option<String> = match &scope {
        Scope::Main => None,
        Scope::Subagent { agent_id } => Some(agent_id.clone()),
    };

    let rows = sqlx::query!(
        r#"
        SELECT
          seq as "seq!",
          kind as "raw_kind!",
          payload as "payload!",
          occurred_at as "occurred_at!",
          turn_id,
          parent_subagent_id
        FROM coding_ingest_events
        WHERE session_id = ?1 AND source = 'klynt-cli'
          AND (?2 IS NULL OR parent_subagent_id = ?2)
        ORDER BY seq ASC
        LIMIT 5000
        "#,
        session_id,
        agent_id_filter,
    )
    .fetch_all(pool)
    .await?;

    let total = rows.len();
    let events = rows
        .into_iter()
        .map(|r| TraceEvent {
            seq: r.seq as u64,
            provider_id: "klynt".to_string(),
            raw_kind: r.raw_kind,
            payload: serde_json::from_str(&r.payload).unwrap_or(serde_json::Value::Null),
            occurred_at: r.occurred_at,
            category: categorize(&r.raw_kind),
            turn_index: None,
            step_index: None,
            parent_subagent_id: r.parent_subagent_id,
        })
        .collect();

    Ok(SessionDetail {
        session_id: session_id.to_string(),
        provider_id: "klynt".to_string(),
        scope,
        stats: serde_json::Map::new().into(),
        events,
        truncated: total >= 5000,
        total_event_count: total as u32,
    })
}

fn categorize(kind: &str) -> String {
    match kind {
        "TurnBegin" | "TurnEnd" => "turn".to_string(),
        "ToolCall" | "ToolResult" => "tool".to_string(),
        "ContentChunk" | "AssistantMsg" => "content".to_string(),
        "ApprovalDecision" => "approval".to_string(),
        "FileEditEnriched" => "edit".to_string(),
        "ProviderCall" => "provider".to_string(),
        "CompressionApplied" => "compaction".to_string(),
        _ => "other".to_string(),
    }
}
```

- [ ] **Step 2: Build**

Run: `cargo build -p app-core 2>&1 | tail -20`
Expected: column-name or type mismatches surface. Fix per the actual schema. (Common fix: replace `r.payload` with `r.payload_json` if that's the column name.)

- [ ] **Step 3: Commit**

```bash
git add crates/app-core/src/tracing/providers/klynt/loader.rs
git commit -m "feat(tracing): klynt provider event loader"
```

---

## Task 8: Implement context loader, state loader, subagent loader

**Files:**
- Create: `crates/app-core/src/tracing/providers/klynt/context_loader.rs`
- Create: `crates/app-core/src/tracing/providers/klynt/state_loader.rs`
- Create: `crates/app-core/src/tracing/providers/klynt/subagent_loader.rs`

- [ ] **Step 1: Write context_loader.rs**

```rust
//! load_klynt_context — read chat_messages for a session.

use std::sync::Arc;

use common::Result;
use storage::Repos;

use crate::tracing::types::{ContextMessage, Scope};

pub async fn load_klynt_context(
    repos: &Arc<Repos>,
    session_id: &str,
    _scope: Scope,
) -> Result<Vec<ContextMessage>> {
    let pool = repos.pool();
    let rows = sqlx::query!(
        r#"
        SELECT
          rowid as "index!: i64",
          role as "role!",
          content
        FROM chat_messages
        WHERE session_id = ?1
        ORDER BY rowid ASC
        "#,
        session_id,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| ContextMessage {
            index: r.index as u32,
            role: r.role,
            content: r.content.and_then(|s| serde_json::from_str(&s).ok()),
        })
        .collect())
}
```

- [ ] **Step 2: Write state_loader.rs**

```rust
//! load_klynt_state — coding-mode session state (cwd, mode, profile).

use std::sync::Arc;

use common::Result;
use storage::Repos;

use crate::tracing::types::SessionState;

pub async fn load_klynt_state(repos: &Arc<Repos>, session_id: &str) -> Result<SessionState> {
    let pool = repos.pool();
    let row = sqlx::query!(
        r#"
        SELECT
          cwd,
          repo_id,
          repo_branch,
          tool_profile,
          approval_mode,
          total_cost_usd,
          total_tokens
        FROM sessions
        WHERE id = ?1 AND conversation_type = 'coding'
        "#,
        session_id,
    )
    .fetch_optional(pool)
    .await?;

    let mut state = SessionState::default();
    if let Some(r) = row {
        state.set("cwd", r.cwd);
        state.set("repo_id", r.repo_id);
        state.set("repo_branch", r.repo_branch);
        state.set("tool_profile", r.tool_profile);
        state.set("approval_mode", Some(r.approval_mode));
        state.set("total_cost_usd", Some(r.total_cost_usd.to_string()));
        state.set("total_tokens", Some(r.total_tokens.to_string()));
    }
    Ok(state)
}
```

- [ ] **Step 3: Write subagent_loader.rs**

```rust
//! list_klynt_subagents — extract subagent records from the events table.

use std::sync::Arc;

use common::Result;
use storage::Repos;

use crate::tracing::types::SubagentSummary;

pub async fn list_klynt_subagents(
    repos: &Arc<Repos>,
    session_id: &str,
) -> Result<Vec<SubagentSummary>> {
    let pool = repos.pool();
    let rows = sqlx::query!(
        r#"
        SELECT DISTINCT
          parent_subagent_id as "agent_id!"
        FROM coding_ingest_events
        WHERE session_id = ?1
          AND source = 'klynt-cli'
          AND parent_subagent_id IS NOT NULL
        "#,
        session_id,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| SubagentSummary {
            agent_id: r.agent_id,
            subagent_type: "subagent".to_string(),
            status: "completed".to_string(),
            description: None,
            created_at: 0,
            updated_at: 0,
            event_count: 0,
        })
        .collect())
}
```

- [ ] **Step 4: Adjust field shapes against `SessionState`, `ContextMessage`, `SubagentSummary` definitions**

Run: `grep -n "pub struct SessionState\|pub struct ContextMessage\|pub struct SubagentSummary" /Users/jayden/Projects/Klynt/bot/crates/app-core/src/tracing/types.rs`
Adapt the constructors to match real fields.

- [ ] **Step 5: Build**

Run: `cargo build -p app-core 2>&1 | tail -20`
Expected: clean build for these files (other sub-modules — summary, stats — still missing).

- [ ] **Step 6: Commit**

```bash
git add crates/app-core/src/tracing/providers/klynt/context_loader.rs \
        crates/app-core/src/tracing/providers/klynt/state_loader.rs \
        crates/app-core/src/tracing/providers/klynt/subagent_loader.rs
git commit -m "feat(tracing): klynt provider context, state, subagent loaders"
```

---

## Task 9: Implement summary + stats

**Files:**
- Create: `crates/app-core/src/tracing/providers/klynt/summary.rs`
- Create: `crates/app-core/src/tracing/providers/klynt/stats.rs`

- [ ] **Step 1: Write summary.rs**

```rust
//! session_summary — per-session aggregate stats.

use std::sync::Arc;

use common::Result;
use storage::Repos;

use crate::tracing::types::SessionSummary;

pub async fn session_summary(
    repos: &Arc<Repos>,
    session_id: &str,
) -> Result<SessionSummary> {
    let pool = repos.pool();
    let row = sqlx::query!(
        r#"
        SELECT
          s.title,
          s.cwd,
          s.repo_id,
          s.created_at as "created_at!",
          s.updated_at as "updated_at!",
          (SELECT COUNT(*) FROM coding_ingest_events e
           WHERE e.session_id = s.id AND e.source = 'klynt-cli') as "event_count!",
          (SELECT COUNT(*) FROM coding_ingest_events e
           WHERE e.session_id = s.id AND e.source = 'klynt-cli'
             AND e.kind = 'TurnBegin') as "turn_count!",
          (SELECT COUNT(*) FROM coding_ingest_events e
           WHERE e.session_id = s.id AND e.source = 'klynt-cli'
             AND e.kind = 'ToolCall') as "tool_count!"
        FROM sessions s
        WHERE s.id = ?1 AND s.conversation_type = 'coding'
        "#,
        session_id,
    )
    .fetch_one(pool)
    .await?;

    Ok(SessionSummary {
        session_id: session_id.to_string(),
        session_dir: format!("klynt://{}", session_id),
        work_dir: row.cwd,
        work_dir_hash: row.repo_id.unwrap_or_default(),
        title: row.title.unwrap_or_else(|| "Untitled session".to_string()),
        last_updated: row.updated_at,
        has_wire: row.event_count > 0,
        has_context: true,
        has_state: true,
        wire_size: row.event_count as u64,
        context_size: 0,
        state_size: 0,
        total_size: 0,
        turns: row.turn_count as u32,
        metadata: None,
        imported: false,
        subagent_count: 0,
    })
}
```

- [ ] **Step 2: Write stats.rs**

```rust
//! aggregate_stats — cross-session stats for the klynt provider.

use std::sync::Arc;

use common::Result;
use storage::Repos;

use crate::tracing::types::{StatsBundle, ToolUsageRow};

pub async fn aggregate_stats(repos: &Arc<Repos>) -> Result<StatsBundle> {
    let pool = repos.pool();
    let tool_rows = sqlx::query!(
        r#"
        SELECT
          json_extract(payload, '$.function.name') as "tool!: String",
          COUNT(*) as "call_count!",
          0 as "error_count!"
        FROM coding_ingest_events
        WHERE source = 'klynt-cli' AND kind = 'ToolCall'
        GROUP BY tool
        ORDER BY call_count DESC
        LIMIT 50
        "#
    )
    .fetch_all(pool)
    .await?;

    let tool_usage = tool_rows
        .into_iter()
        .map(|r| ToolUsageRow {
            tool: r.tool,
            call_count: r.call_count as u32,
            error_count: r.error_count as u32,
        })
        .collect();

    Ok(StatsBundle {
        per_project: vec![],
        tool_usage,
        errors_by_tool: vec![],
        token_series: vec![],
        subagent_types: vec![],
        cache_hit_pct: 0.0,
    })
}
```

- [ ] **Step 3: Adapt to actual `StatsBundle` and `ToolUsageRow` shapes**

Run: `grep -n "pub struct StatsBundle\|pub struct ToolUsageRow" /Users/jayden/Projects/Klynt/bot/crates/app-core/src/tracing/types.rs`
Adjust constructors to match.

- [ ] **Step 4: Build**

Run: `cargo build -p app-core`
Expected: clean build.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/tracing/providers/klynt/summary.rs \
        crates/app-core/src/tracing/providers/klynt/stats.rs
git commit -m "feat(tracing): klynt provider summary and stats"
```

---

# Phase 4 — Registration + Tauri command

## Task 10: Register KlyntTracingProvider at AppCore init

**Files:**
- Modify: `crates/app-core/src/init/*.rs` (exact file determined in this task)

- [ ] **Step 1: Find where KimiTracingProvider is registered**

Run: `grep -rn "KimiTracingProvider\|tracing_registry" /Users/jayden/Projects/Klynt/bot/crates/app-core/src/init/ /Users/jayden/Projects/Klynt/bot/crates/app-core/src/lib.rs`
Expected: at least one `register(...)` call adding the kimi provider. Note the file and line.

- [ ] **Step 2: Add the klynt registration alongside**

Edit the file from Step 1; immediately after the kimi registration:

```rust
registry.register(Arc::new(
    crate::tracing::providers::klynt::KlyntTracingProvider::new(repos.clone()),
));
```

- [ ] **Step 3: Build to verify**

Run: `cargo build -p app-core`
Expected: clean.

- [ ] **Step 4: Run the registry test**

Run: `cargo nextest run -p app-core tracing::registry`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/init/<filename>.rs
git commit -m "feat(tracing): register KlyntTracingProvider at app init"
```

---

## Task 11: Add `tracing_list_providers` Tauri command

**Files:**
- Modify: `crates/app-core/src/tracing_handlers.rs`
- Modify: `crates/desktop/src/commands/tracing.rs`
- Modify: `crates/desktop/src/specta_builder.rs`

- [ ] **Step 1: Add the AppCore handler**

In `crates/app-core/src/tracing_handlers.rs`, add:

```rust
#[tracing::instrument(skip(self), err)]
pub async fn tracing_list_providers(&self) -> Result<Vec<crate::tracing::types::ProviderInfo>> {
    self.tracing_registry.list_providers().await
}
```

- [ ] **Step 2: Add the Tauri command shell**

In `crates/desktop/src/commands/tracing.rs`, add:

```rust
#[klynt_command]
pub async fn tracing_list_providers(
    state: State<'_, AppCore>,
) -> Result<Vec<klynt_app_core::tracing::types::ProviderInfo>> {
    state.tracing_list_providers().await
}
```

- [ ] **Step 3: List the new command in specta_builder.rs**

Run: `grep -n "klynt_collect_commands" /Users/jayden/Projects/Klynt/bot/crates/desktop/src/specta_builder.rs`
Add `commands::tracing::tracing_list_providers` to the list inside the macro.

- [ ] **Step 4: Build**

Run: `cargo build -p desktop`
Expected: clean.

- [ ] **Step 5: Regenerate frontend bindings**

Run: `cd /Users/jayden/Projects/Klynt/bot && cargo tauri dev` for ~10 seconds, then Ctrl+C.
Verify `desktop-ui/src/bindings.ts` now includes `tracing_list_providers`.

- [ ] **Step 6: Commit**

```bash
git add crates/app-core/src/tracing_handlers.rs \
        crates/desktop/src/commands/tracing.rs \
        crates/desktop/src/specta_builder.rs \
        desktop-ui/src/bindings.ts
git commit -m "feat(tracing): add tracing_list_providers Tauri command"
```

---

# Phase 5 — Frontend provider selector

## Task 12: Replace hardcoded PROVIDER_ID with selector

**Files:**
- Modify: `desktop-ui/src/tracing/lib/api.ts`

- [ ] **Step 1: Replace the constant with helpers**

In `desktop-ui/src/tracing/lib/api.ts`, replace line 7 (`const PROVIDER_ID = "kimi";`) with:

```typescript
let _providerId: string | null = null;

export function getCurrentProviderId(): string {
  if (_providerId) return _providerId;
  const params = new URLSearchParams(window.location.search);
  _providerId = params.get("provider") ?? "klynt";
  return _providerId;
}

export function setCurrentProviderId(id: string): void {
  _providerId = id;
  const url = new URL(window.location.href);
  url.searchParams.set("provider", id);
  window.history.pushState({}, "", url.toString());
  apiCache.clear();
}

export interface ProviderInfo {
  id: string;
  display_name: string;
  session_count: number;
}

export async function listProviders(): Promise<ProviderInfo[]> {
  return invoke<ProviderInfo[]>("tracing_list_providers");
}
```

- [ ] **Step 2: Replace every `providerId: PROVIDER_ID` reference**

Run: `grep -n "PROVIDER_ID" /Users/jayden/Projects/Klynt/bot/desktop-ui/src/tracing/lib/api.ts`
For each match, replace `providerId: PROVIDER_ID` with `providerId: getCurrentProviderId()`.

- [ ] **Step 3: Add `apiCache.clear` method if absent**

Run: `grep -n "clear" /Users/jayden/Projects/Klynt/bot/desktop-ui/src/tracing/lib/cache.ts`
If no `clear` method exists, add:

```typescript
clear(): void {
  this._cache.clear();
}
```

- [ ] **Step 4: Typecheck**

Run: `cd /Users/jayden/Projects/Klynt/bot/desktop-ui && bun run typecheck`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/tracing/lib/api.ts desktop-ui/src/tracing/lib/cache.ts
git commit -m "feat(tracing-ui): replace hardcoded PROVIDER_ID with selector"
```

---

## Task 13: Build the ProviderChips component

**Files:**
- Create: `desktop-ui/src/tracing/components/provider-chips.tsx`
- Create: `desktop-ui/src/tracing/components/provider-chips.test.tsx`
- Modify: `desktop-ui/src/tracing/index.tsx`

- [ ] **Step 1: Write the component**

Create `desktop-ui/src/tracing/components/provider-chips.tsx`:

```tsx
import { useEffect, useState } from "react";
import {
  getCurrentProviderId,
  listProviders,
  setCurrentProviderId,
  type ProviderInfo,
} from "@/tracing/lib/api";

export function ProviderChips({ onChange }: { onChange?: () => void }) {
  const [providers, setProviders] = useState<ProviderInfo[]>([]);
  const current = getCurrentProviderId();

  useEffect(() => {
    listProviders().then(setProviders).catch(console.error);
  }, []);

  if (providers.length <= 1) return null;

  return (
    <div className="flex gap-1 px-3 py-1 border-b">
      {providers.map((p) => (
        <button
          key={p.id}
          type="button"
          onClick={() => {
            setCurrentProviderId(p.id);
            onChange?.();
          }}
          className={`text-xs px-2 py-1 rounded-md transition-colors ${
            current === p.id
              ? "bg-accent text-foreground"
              : "text-muted-foreground hover:bg-accent/50"
          }`}
        >
          {p.display_name}{" "}
          <span className="opacity-60">({p.session_count})</span>
        </button>
      ))}
    </div>
  );
}
```

- [ ] **Step 2: Write the test**

Create `desktop-ui/src/tracing/components/provider-chips.test.tsx`:

```tsx
import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { ProviderChips } from "./provider-chips";

vi.mock("@/tracing/lib/api", () => ({
  getCurrentProviderId: vi.fn(() => "klynt"),
  setCurrentProviderId: vi.fn(),
  listProviders: vi.fn(async () => [
    { id: "klynt", display_name: "Klynt", session_count: 3 },
    { id: "kimi", display_name: "Kimi CLI", session_count: 12 },
  ]),
}));

describe("ProviderChips", () => {
  it("renders one chip per provider with session count", async () => {
    render(<ProviderChips />);
    await waitFor(() => {
      expect(screen.getByText("Klynt")).toBeInTheDocument();
      expect(screen.getByText("Kimi CLI")).toBeInTheDocument();
      expect(screen.getByText("(3)")).toBeInTheDocument();
      expect(screen.getByText("(12)")).toBeInTheDocument();
    });
  });

  it("calls onChange when a chip is clicked", async () => {
    const onChange = vi.fn();
    render(<ProviderChips onChange={onChange} />);
    await waitFor(() => screen.getByText("Kimi CLI"));
    fireEvent.click(screen.getByText("Kimi CLI"));
    expect(onChange).toHaveBeenCalled();
  });

  it("hides itself when only one provider exists", async () => {
    const api = await import("@/tracing/lib/api");
    vi.spyOn(api, "listProviders").mockResolvedValueOnce([
      { id: "klynt", display_name: "Klynt", session_count: 3 },
    ]);
    const { container } = render(<ProviderChips />);
    await waitFor(() => {
      expect(container.firstChild).toBeNull();
    });
  });
});
```

- [ ] **Step 3: Run the tests**

Run: `cd /Users/jayden/Projects/Klynt/bot/desktop-ui && bun run test --run provider-chips`
Expected: PASS.

- [ ] **Step 4: Mount the component in the tracing root**

In `desktop-ui/src/tracing/index.tsx`, immediately after the `<header>` block (around line 480), add:

```tsx
<ProviderChips
  onChange={() => {
    setSessionId(null);
    setSessions([]);
    listSessions(true).then(setSessions).catch(() => {});
  }}
/>
```

And add the import at the top:

```tsx
import { ProviderChips } from "@/tracing/components/provider-chips";
```

- [ ] **Step 5: Lint + typecheck**

Run: `cd /Users/jayden/Projects/Klynt/bot/desktop-ui && bun run lint && bun run typecheck`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/tracing/components/provider-chips.tsx \
        desktop-ui/src/tracing/components/provider-chips.test.tsx \
        desktop-ui/src/tracing/index.tsx
git commit -m "feat(tracing-ui): provider chip-row in tracing header"
```

---

# Phase 6 — Property test extension + manual verification

## Task 14: Extend cross_cli_normalization proptest with KlyntCli source

**Files:**
- Modify: `crates/coding-ingest/tests/cross_cli_normalization.rs`

- [ ] **Step 1: Read the existing test to find the source generator**

Run: `grep -n "AgentSource\|prop_oneof" /Users/jayden/Projects/Klynt/bot/crates/coding-ingest/tests/cross_cli_normalization.rs`
Expected: a `prop_oneof![ ... AgentSource::ClaudeCode, AgentSource::Codex, AgentSource::KimiCli, AgentSource::OpenCode ... ]` strategy.

- [ ] **Step 2: Add KlyntCli to the strategy**

Add `Just(AgentSource::KlyntCli)` to the `prop_oneof!` list.

- [ ] **Step 3: Run the test**

Run: `cargo nextest run -p coding-ingest cross_cli_normalization --no-capture`
Expected: PASS — round-trip property holds across all five sources including KlyntCli.

- [ ] **Step 4: Commit**

```bash
git add crates/coding-ingest/tests/cross_cli_normalization.rs
git commit -m "test(coding-ingest): include KlyntCli in cross-CLI normalization proptest"
```

---

## Task 15: Manual end-to-end verification

**Files:** none.

- [ ] **Step 1: Start the dev environment**

```bash
cd /Users/jayden/Projects/Klynt/bot/desktop-ui && bun run dev:vite &
cd /Users/jayden/Projects/Klynt/bot && KLYNTBOT_HOME=~/.klyntbot-dev cargo tauri dev
```

- [ ] **Step 2: Send a coding-mode message**

In the desktop app, switch to coding mode in the composer. Send: "Read the README.md and tell me what this project does."

Wait for the agent loop to complete (or stream tool calls).

- [ ] **Step 3: Open the Tracing page**

Navigate to the Tracing tab in the desktop app.

- [ ] **Step 4: Verify the provider chip-row**

Expected: chips for "Klynt", "Kimi CLI" (and any other registered providers). Default selection: Klynt.

- [ ] **Step 5: Verify the session list**

Expected: the session you just created appears with a real (or "Untitled") title, last_updated within seconds, and turn count = 1.

- [ ] **Step 6: Open the session and verify the wire viewer**

Expected: ContentChunk → ToolCall → ToolResult events in chronological order. ToolCall and ToolResult are correctly grouped with `linkedToolCallId`.

- [ ] **Step 7: Switch to Kimi provider, verify Kimi sessions still load**

Expected: provider chip-row click switches the displayed sessions; existing Kimi functionality unaffected.

- [ ] **Step 8: Clean up**

Stop the dev server. Confirm no orphaned `cargo tauri dev` processes.

---

## Self-review checklist (perform after Task 15)

- [ ] **Spec coverage:** Every section of the spec is implemented in some task above. Gaps: none expected.
- [ ] **No placeholders:** Search the plan for "TODO", "TBD", "fill in" — none should remain.
- [ ] **Type consistency:** `KlyntTracingProvider` constructor takes `Arc<Repos>` everywhere it's used; `getCurrentProviderId` referenced consistently; column names in queries match the schema verified in Task 6 Step 2.
- [ ] **Verification all green:**
  - `cargo nextest run --workspace` passes.
  - `cargo clippy --workspace --all-targets --all-features` produces zero warnings.
  - `cd desktop-ui && bun run lint && bun run typecheck && bun run test` passes.
  - Manual verification (Task 15) completes without errors.

---

*End of plan.*
