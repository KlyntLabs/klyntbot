# Tracing UI Port Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace klyntbot's existing tracing UI with a verbatim port of the upstream Apache-2.0 tracing visualization SPA found in `references/kimi-cli/vis/`, mounted as a Tailwind v4 island that re-themes via klyntbot's `--ds-*` design tokens. Spec: `docs/superpowers/specs/2026-05-02-tracing-ui-port-design.md`.

**Architecture:** Self-contained Tailwind island at `desktop-ui/src/tracing/` with its own deps (Tailwind v4, shadcn-style Radix primitives, lucide-react, streamdown). The Coding Memory plugin's route renders `<TracingApp />` directly. Adapter layer (`src/tracing/lib/api.ts`) translates upstream-shaped function calls into Tauri `invoke()`s, pinned to `KimiTracingProvider`. Backend gains 3 new commands and an extended `SessionSummary` shape; otherwise unchanged.

**Tech Stack:** Rust (jiff, serde, specta, tokio), Tauri v2, React 19, Vite, Tailwind v4, Radix UI primitives, react-virtuoso, lucide-react, streamdown, bun.

**License compliance:** Apache-2.0 attribution lives in three contained spots: project-root `THIRD_PARTY_NOTICES.md`, `desktop-ui/LICENSES/apache-2.0-tracing-ui.txt`, and a per-file SPDX header (`// SPDX-License-Identifier: Apache-2.0` + `// Derived from upstream Apache-2.0 source. See THIRD_PARTY_NOTICES.md.`). Upstream project name appears nowhere else in the source tree.

**Verbatim-port convention:** Each "Port file X" task copies the upstream file from `references/kimi-cli/vis/src/<path>` to the target path under `src/tracing/<path>`. The engineer:
1. Reads the upstream file to understand its imports + exports.
2. Copies its body verbatim to the target.
3. Prepends the SPDX header (two lines).
4. Adjusts import paths so internal imports resolve to the new structure (e.g., `@/lib/api` → `../../lib/api`, or whichever convention the island standardizes on).
5. Renames any identifier or string that contains the upstream project name to a generic equivalent. (Acceptance: `grep -ri 'kimi\|moonshot' src/tracing/` returns zero matches outside the SPDX header.)

The plan does not embed upstream component code — the source of truth is `references/kimi-cli/vis/`.

---

## Phase 1 — Backend types: SessionMetadataInfo + SessionSummary expansion

### Task 1.1: Add `SessionMetadataInfo` struct

**Files:**
- Modify: `crates/app-core/src/tracing/types.rs`

- [ ] **Step 1: Append the new struct at the end of types.rs**

```rust
// ── Tracing UI port: SessionMetadataInfo ────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionMetadataInfo {
    pub session_id: String,
    pub title: String,
    pub title_generated: bool,
    pub archived: bool,
    pub archived_at: Option<i64>,
    pub auto_archive_exempt: bool,
    pub wire_mtime: Option<i64>,
}

#[cfg(test)]
mod metadata_tests {
    use super::*;
    #[test]
    fn metadata_serializes_camel_case() {
        let m = SessionMetadataInfo {
            session_id: "s1".into(),
            title: "t".into(),
            title_generated: false,
            archived: false,
            archived_at: None,
            auto_archive_exempt: false,
            wire_mtime: Some(1_700_000_000),
        };
        let s = serde_json::to_string(&m).unwrap();
        assert!(s.contains(r#""sessionId":"s1""#));
        assert!(s.contains(r#""titleGenerated":false"#));
        assert!(s.contains(r#""wireMtime":1700000000"#));
    }
}
```

- [ ] **Step 2: Run the new test**

Run: `cargo nextest run -p app-core tracing::types::metadata_tests`
Expected: PASS (1 test).

- [ ] **Step 3: Commit**

```bash
git add crates/app-core/src/tracing/types.rs
git commit -m "feat(tracing): add SessionMetadataInfo DTO"
```

### Task 1.2: Extend `SessionSummary` with port-required fields

**Files:**
- Modify: `crates/app-core/src/tracing/types.rs`

- [ ] **Step 1: Replace the existing `SessionSummary` struct**

Locate `pub struct SessionSummary` (around line 132) and replace it with:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub session_id: String,
    pub provider_id: String,
    pub source_dir: PathBuf,
    pub cwd: Option<PathBuf>,
    pub project_basename: Option<String>,
    pub custom_title: Option<String>,
    #[specta(type = desktop_shared::specta_helpers::Timestamp)]
    pub started_at: Timestamp,
    #[specta(type = desktop_shared::specta_helpers::Timestamp)]
    pub last_event_at: Timestamp,
    pub size_bytes: u64,
    pub turn_count: u32,
    pub step_count: u32,
    pub tool_call_count: u32,
    pub error_count: u32,
    pub subagent_count: u32,
    pub has_wire: bool,
    pub has_context: bool,
    pub imported: bool,

    // ── Tracing UI port additions ──
    pub work_dir_hash: String,
    pub has_state: bool,
    pub wire_size: u64,
    pub context_size: u64,
    pub state_size: u64,
    pub total_size: u64,
    pub metadata: Option<SessionMetadataInfo>,
}
```

- [ ] **Step 2: Run all type tests**

Run: `cargo nextest run -p app-core tracing::types`
Expected: PASS (existing tests).

- [ ] **Step 3: Update construction sites**

Run: `cargo build -p app-core 2>&1 | grep "missing field" | head -20`
Expected: a list of construction sites missing the new fields (likely in `providers/kimi/loader.rs` and `providers/kimi/provider_impl.rs`).

For each construction site, populate the new fields. Use these defaults at every site (real values land in Phase 2):

```rust
work_dir_hash: String::new(),
has_state: false,
wire_size: 0,
context_size: 0,
state_size: 0,
total_size: 0,
metadata: None,
```

- [ ] **Step 4: Verify the crate compiles**

Run: `cargo build -p app-core`
Expected: success.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/tracing/
git commit -m "feat(tracing): extend SessionSummary with port-required fields"
```

### Task 1.3: Add new methods to the `TracingProvider` trait

**Files:**
- Modify: `crates/app-core/src/tracing/provider.rs`

- [ ] **Step 1: Add three methods to the trait**

Append to the existing `#[async_trait] pub trait TracingProvider`:

```rust
    /// Per-session aggregate summary (deeper than `list_sessions` row).
    async fn session_summary(&self, session_id: &str) -> common::Result<SessionSummary>;

    /// Wire events for a single subagent within a session.
    async fn load_subagent_session(
        &self,
        session_id: &str,
        agent_id: &str,
    ) -> common::Result<SessionDetail>;

    /// Context messages for a single subagent within a session.
    async fn load_subagent_context(
        &self,
        session_id: &str,
        agent_id: &str,
    ) -> common::Result<Vec<ContextMessage>>;
```

- [ ] **Step 2: Verify compile fails on the impl**

Run: `cargo build -p app-core 2>&1 | grep "not all trait items"`
Expected: error pointing to `KimiTracingProvider` (the next task fixes it).

- [ ] **Step 3: Commit**

```bash
git add crates/app-core/src/tracing/provider.rs
git commit -m "feat(tracing): add three new methods to TracingProvider trait"
```

---

## Phase 2 — Kimi provider: implement the new methods

### Task 2.1: Implement `session_summary` (TDD)

**Files:**
- Create: `crates/app-core/src/tracing/providers/kimi/summary.rs`
- Modify: `crates/app-core/src/tracing/providers/kimi/mod.rs`
- Test: same file (`#[cfg(test)] mod tests`)

- [ ] **Step 1: Wire the new submodule**

In `crates/app-core/src/tracing/providers/kimi/mod.rs`, add `pub mod summary;`.

- [ ] **Step 2: Write the failing test**

Create `summary.rs` with:

```rust
//! Per-session aggregate summary computation.

use crate::tracing::providers::kimi::loader;
use crate::tracing::providers::kimi::workdir_resolver::WorkdirResolver;
use crate::tracing::types::{SessionMetadataInfo, SessionSummary};
use common::Result;
use std::path::Path;

pub async fn compute(sessions_root: &Path, session_id: &str) -> Result<SessionSummary> {
    todo!("compute aggregate")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/kimi/sessions")
    }

    #[tokio::test]
    async fn computes_summary_for_existing_fixture() {
        let summary = compute(&fixture_root(), "sess-fixture-001").await.unwrap();
        assert_eq!(summary.session_id, "sess-fixture-001");
        assert_eq!(summary.provider_id, "kimi");
        assert!(summary.has_wire);
        assert!(summary.has_context);
        assert!(summary.has_state);
        assert!(summary.wire_size > 0);
        assert!(summary.total_size > 0);
        assert_eq!(summary.subagent_count, 1);
        assert!(summary.metadata.is_some());
    }
}
```

- [ ] **Step 3: Run the test, expect failure**

Run: `cargo nextest run -p app-core tracing::providers::kimi::summary`
Expected: FAIL with "not yet implemented" panic.

- [ ] **Step 4: Implement `compute`**

Replace the `todo!()` body with:

```rust
pub async fn compute(sessions_root: &Path, session_id: &str) -> Result<SessionSummary> {
    let session_dir = sessions_root.join(session_id);
    if !session_dir.exists() {
        return Err(common::KlyntbotError::StorageNotFound(format!(
            "session {} not found",
            session_id
        )));
    }

    let wire_path = session_dir.join("wire.jsonl");
    let context_path = session_dir.join("context.jsonl");
    let state_path = session_dir.join("state.json");
    let subagents_dir = session_dir.join("subagents");

    let wire_size = file_len(&wire_path).await;
    let context_size = file_len(&context_path).await;
    let state_size = file_len(&state_path).await;
    let total_size = wire_size + context_size + state_size;

    let detail = loader::load_session_events(&session_dir).await?;
    let stats = &detail.stats;

    let subagent_count = if subagents_dir.is_dir() {
        tokio::fs::read_dir(&subagents_dir)
            .await
            .ok()
            .map(|mut rd| {
                let mut n = 0;
                while let Ok(Some(_)) = futures::executor::block_on(rd.next_entry()) {
                    n += 1;
                }
                n
            })
            .unwrap_or(0)
    } else {
        0
    };

    let metadata = SessionMetadataInfo {
        session_id: session_id.to_string(),
        title: session_id.to_string(),
        title_generated: false,
        archived: false,
        archived_at: None,
        auto_archive_exempt: false,
        wire_mtime: tokio::fs::metadata(&wire_path)
            .await
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64),
    };

    Ok(SessionSummary {
        session_id: session_id.to_string(),
        provider_id: "kimi".into(),
        source_dir: session_dir.clone(),
        cwd: None,
        project_basename: None,
        custom_title: None,
        started_at: jiff::Timestamp::UNIX_EPOCH,
        last_event_at: jiff::Timestamp::UNIX_EPOCH,
        size_bytes: total_size,
        turn_count: stats.turn_count,
        step_count: stats.step_count,
        tool_call_count: stats.tool_call_count,
        error_count: stats.error_count,
        subagent_count: subagent_count as u32,
        has_wire: wire_path.exists(),
        has_context: context_path.exists(),
        imported: false,

        work_dir_hash: String::new(),
        has_state: state_path.exists(),
        wire_size,
        context_size,
        state_size,
        total_size,
        metadata: Some(metadata),
    })
}

async fn file_len(p: &Path) -> u64 {
    tokio::fs::metadata(p).await.map(|m| m.len()).unwrap_or(0)
}
```

- [ ] **Step 5: Run the test, expect pass**

Run: `cargo nextest run -p app-core tracing::providers::kimi::summary`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/app-core/src/tracing/providers/kimi/summary.rs crates/app-core/src/tracing/providers/kimi/mod.rs
git commit -m "feat(tracing): kimi summary aggregator"
```

### Task 2.2: Implement `load_subagent_session` (TDD)

**Files:**
- Modify: `crates/app-core/src/tracing/providers/kimi/loader.rs` (add `load_subagent_events`)
- Modify: `crates/app-core/src/tracing/providers/kimi/provider_impl.rs`

- [ ] **Step 1: Write the failing test in `provider_impl.rs`**

In the existing `#[cfg(test)] mod tests` block, append:

```rust
#[tokio::test]
async fn loads_subagent_wire_events() {
    let provider = test_provider();
    let detail = provider
        .load_subagent_session("sess-fixture-001", "sub-aaa")
        .await
        .unwrap();
    assert_eq!(detail.session_id, "sess-fixture-001");
    assert!(matches!(detail.scope, Scope::Subagent { .. }));
    assert!(!detail.events.is_empty());
}
```

- [ ] **Step 2: Run the test, expect failure**

Run: `cargo nextest run -p app-core tracing::providers::kimi::provider_impl::tests::loads_subagent_wire_events`
Expected: FAIL (method not yet on impl).

- [ ] **Step 3: Add a `load_subagent_events` helper to `loader.rs`**

Append a public function:

```rust
pub async fn load_subagent_events(
    session_dir: &Path,
    agent_id: &str,
) -> Result<SessionDetail> {
    let subagent_dir = session_dir.join("subagents").join(agent_id);
    if !subagent_dir.exists() {
        return Err(common::KlyntbotError::StorageNotFound(format!(
            "subagent {} not found",
            agent_id
        )));
    }
    let wire_path = subagent_dir.join("wire.jsonl");
    let mut detail = stream_wire_file(&wire_path).await?;
    detail.scope = Scope::Subagent { agent_id: agent_id.to_string() };
    Ok(detail)
}
```

(Reuse the existing `stream_wire_file` helper that the main session loader uses; if it's private, lift it to `pub(super)`.)

- [ ] **Step 4: Implement the trait method**

In `provider_impl.rs`'s `impl TracingProvider for KimiTracingProvider`, add:

```rust
async fn load_subagent_session(
    &self,
    session_id: &str,
    agent_id: &str,
) -> Result<SessionDetail> {
    let session_dir = self.sessions_root.join(session_id);
    loader::load_subagent_events(&session_dir, agent_id).await
}
```

- [ ] **Step 5: Run the test, expect pass**

Run: `cargo nextest run -p app-core tracing::providers::kimi::provider_impl`
Expected: PASS (all tests including the new one).

- [ ] **Step 6: Commit**

```bash
git add crates/app-core/src/tracing/providers/kimi/loader.rs crates/app-core/src/tracing/providers/kimi/provider_impl.rs
git commit -m "feat(tracing): KimiTracingProvider::load_subagent_session"
```

### Task 2.3: Implement `load_subagent_context` (TDD)

**Files:**
- Modify: `crates/app-core/src/tracing/providers/kimi/context_loader.rs`
- Modify: `crates/app-core/src/tracing/providers/kimi/provider_impl.rs`

- [ ] **Step 1: Write the failing test**

Append to `provider_impl.rs` tests:

```rust
#[tokio::test]
async fn loads_subagent_context() {
    let provider = test_provider();
    let messages = provider
        .load_subagent_context("sess-fixture-001", "sub-aaa")
        .await
        .unwrap();
    assert!(!messages.is_empty());
}
```

(If the fixture's subagent has no `context.jsonl`, add one with two messages — see Phase 7 for fixture work — or make the test tolerate empty.)

- [ ] **Step 2: Run, expect fail**

Run: `cargo nextest run -p app-core loads_subagent_context`
Expected: FAIL.

- [ ] **Step 3: Add `load_subagent_context_messages` to `context_loader.rs`**

```rust
pub async fn load_subagent_context_messages(
    session_dir: &Path,
    agent_id: &str,
) -> Result<Vec<ContextMessage>> {
    let path = session_dir
        .join("subagents")
        .join(agent_id)
        .join("context.jsonl");
    if !path.exists() {
        return Ok(Vec::new());
    }
    load_messages_from_path(&path).await
}
```

(Use whatever the main `load_context` helper is named in this file; rename if needed for consistency. Keep `load_context` as-is — call into the same low-level helper.)

- [ ] **Step 4: Implement the trait method**

```rust
async fn load_subagent_context(
    &self,
    session_id: &str,
    agent_id: &str,
) -> Result<Vec<ContextMessage>> {
    let session_dir = self.sessions_root.join(session_id);
    context_loader::load_subagent_context_messages(&session_dir, agent_id).await
}
```

- [ ] **Step 5: Run, expect pass**

Run: `cargo nextest run -p app-core loads_subagent_context`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/app-core/src/tracing/providers/kimi/context_loader.rs crates/app-core/src/tracing/providers/kimi/provider_impl.rs
git commit -m "feat(tracing): KimiTracingProvider::load_subagent_context"
```

### Task 2.4: Implement `session_summary` on the trait

**Files:**
- Modify: `crates/app-core/src/tracing/providers/kimi/provider_impl.rs`

- [ ] **Step 1: Add the trait method**

```rust
async fn session_summary(&self, session_id: &str) -> Result<SessionSummary> {
    summary::compute(&self.sessions_root, session_id).await
}
```

Add `use super::summary;` at the top of the file if not already present.

- [ ] **Step 2: Verify whole crate compiles**

Run: `cargo build -p app-core`
Expected: success.

- [ ] **Step 3: Run the entire tracing test suite**

Run: `cargo nextest run -p app-core tracing`
Expected: PASS (all existing tests + 3 new ones).

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/tracing/providers/kimi/provider_impl.rs
git commit -m "feat(tracing): wire session_summary into KimiTracingProvider"
```

---

## Phase 3 — AppCore handlers

### Task 3.1: Add three handlers to `tracing_handlers.rs`

**Files:**
- Modify: `crates/app-core/src/tracing_handlers.rs`

- [ ] **Step 1: Append three methods to `impl AppCore`**

```rust
    #[tracing::instrument(skip(self), err)]
    pub async fn tracing_session_summary(
        &self,
        provider_id: String,
        session_id: String,
    ) -> common::Result<SessionSummary> {
        let provider = self
            .tracing_registry
            .get(&provider_id)
            .ok_or_else(|| common::KlyntbotError::StorageNotFound(format!("provider {}", provider_id)))?;
        provider.session_summary(&session_id).await
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn tracing_load_subagent_session(
        &self,
        provider_id: String,
        session_id: String,
        agent_id: String,
    ) -> common::Result<SessionDetail> {
        let provider = self
            .tracing_registry
            .get(&provider_id)
            .ok_or_else(|| common::KlyntbotError::StorageNotFound(format!("provider {}", provider_id)))?;
        provider.load_subagent_session(&session_id, &agent_id).await
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn tracing_load_subagent_context(
        &self,
        provider_id: String,
        session_id: String,
        agent_id: String,
    ) -> common::Result<Vec<ContextMessage>> {
        let provider = self
            .tracing_registry
            .get(&provider_id)
            .ok_or_else(|| common::KlyntbotError::StorageNotFound(format!("provider {}", provider_id)))?;
        provider.load_subagent_context(&session_id, &agent_id).await
    }
```

Add `use crate::tracing::types::SessionSummary;` to the import block if needed.

- [ ] **Step 2: Verify compile**

Run: `cargo build -p app-core`
Expected: success.

- [ ] **Step 3: Commit**

```bash
git add crates/app-core/src/tracing_handlers.rs
git commit -m "feat(tracing): three new AppCore handlers for port"
```

---

## Phase 4 — Tauri command adapters

### Task 4.1: Add three `#[klynt_command]` adapters

**Files:**
- Modify: `crates/desktop/src/commands/tracing.rs`

- [ ] **Step 1: Add three command shells**

```rust
#[klynt_command]
pub async fn tracing_session_summary(
    app: AppHandle,
    provider_id: String,
    session_id: String,
) -> Result<SessionSummary> {
    app_core(&app)
        .tracing_session_summary(provider_id, session_id)
        .await
}

#[klynt_command]
pub async fn tracing_load_subagent_session(
    app: AppHandle,
    provider_id: String,
    session_id: String,
    agent_id: String,
) -> Result<SessionDetail> {
    app_core(&app)
        .tracing_load_subagent_session(provider_id, session_id, agent_id)
        .await
}

#[klynt_command]
pub async fn tracing_load_subagent_context(
    app: AppHandle,
    provider_id: String,
    session_id: String,
    agent_id: String,
) -> Result<Vec<ContextMessage>> {
    app_core(&app)
        .tracing_load_subagent_context(provider_id, session_id, agent_id)
        .await
}
```

Add `SessionSummary` to the import block if not already present.

- [ ] **Step 2: Verify compile**

Run: `cargo build -p desktop --no-run 2>&1 | tail -5`
Expected: success.

- [ ] **Step 3: Commit**

```bash
git add crates/desktop/src/commands/tracing.rs
git commit -m "feat(tracing): three new Tauri command adapters"
```

### Task 4.2: Register commands in `specta_builder.rs`

**Files:**
- Modify: `crates/desktop/src/specta_builder.rs`

- [ ] **Step 1: Find the existing tracing command list**

Run: `grep -n "tracing_" crates/desktop/src/specta_builder.rs`
Expected: a `klynt_collect_commands![...]` block listing the existing 10 tracing commands.

- [ ] **Step 2: Add the three new entries to the macro list**

In alphabetical order within the tracing block:

```
crate::commands::tracing::tracing_load_subagent_context,
crate::commands::tracing::tracing_load_subagent_session,
crate::commands::tracing::tracing_session_summary,
```

- [ ] **Step 3: Run the registration drift test**

Run: `cargo nextest run -p desktop registration_drift`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/desktop/src/specta_builder.rs
git commit -m "feat(tracing): register three new commands in specta builder"
```

### Task 4.3: Regenerate `bindings.ts`

**Files:**
- Auto-modified: `desktop-ui/src/bindings.ts`

- [ ] **Step 1: Run the dev tauri build to regen bindings**

Run: `cargo tauri dev` in one terminal until specta logs show bindings written; then Ctrl+C. Alternatively:
Run: `cargo nextest run -p desktop bindings_are_current 2>&1 | tail -20`

If that test fails with "bindings out of date", run the dev cycle once.

- [ ] **Step 2: Verify bindings include the three new commands**

Run: `grep "tracing_load_subagent_session\|tracing_load_subagent_context\|tracing_session_summary" desktop-ui/src/bindings.ts | wc -l`
Expected: 3 (one match per command).

- [ ] **Step 3: Run the full `desktop` test suite**

Run: `cargo nextest run -p desktop`
Expected: PASS, including `bindings_are_current` and `no_raw_tauri_command_outside_macros`.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/bindings.ts
git commit -m "chore(tracing): regenerate bindings for three new commands"
```

---

## Phase 5 — Fixture enrichment

### Task 5.1: Create rich fixture skeleton

**Files:**
- Create: `crates/app-core/tests/fixtures/kimi/sessions/abc123hash/sess-fixture-rich/`
  - `wire.jsonl`
  - `context.jsonl`
  - `state.json`
  - `subagents/sub-rich-a/{meta.json, prompt.txt, wire.jsonl, context.jsonl}`
  - `subagents/sub-rich-b/{meta.json, prompt.txt, wire.jsonl, context.jsonl}`

- [ ] **Step 1: Create the directory tree**

Run:
```bash
mkdir -p crates/app-core/tests/fixtures/kimi/sessions/abc123hash/sess-fixture-rich/subagents/sub-rich-a
mkdir -p crates/app-core/tests/fixtures/kimi/sessions/abc123hash/sess-fixture-rich/subagents/sub-rich-b
```

- [ ] **Step 2: Generate `wire.jsonl` (~150 events)**

The fixture needs realistic event diversity for visual smoke. The format mirrors the existing `sess-fixture-001/wire.jsonl` (one JSON object per line, fields: `index`, `timestamp`, `type`, `payload`).

Required event mix (compose by hand or via a small Rust script in `tests/common`):
- 5 turns, 30 steps, average 4-5 tool calls per turn
- 2 errors (tool result with `is_error: true`)
- 1 compaction marker (`type: "CompactionBegin"`, then `CompactionEnd`)
- 4 subagent invocations referencing `sub-rich-a` or `sub-rich-b`
- A spread of timestamps spanning 3 days for daily-aggregate stats
- Mixed `ContentPart` text + thinking blocks
- A handful of `StatusUpdate` events with todos

Write the file. Aim for 140-160 lines.

- [ ] **Step 3: Generate `context.jsonl`**

10-15 lines: a `system` message, alternating `user` / `assistant`, two with `tool_calls` arrays, two with `tool_call_id`. Format matches existing fixtures.

- [ ] **Step 4: Generate `state.json`**

```json
{
  "custom_title": "Rich fixture session",
  "plan_mode": false,
  "archived": false,
  "todos": [
    { "title": "Wire up dashboard", "status": "in_progress" },
    { "title": "Write tests", "status": "completed" }
  ]
}
```

- [ ] **Step 5: Populate subagent files**

For each subagent (`sub-rich-a`, `sub-rich-b`):
- `meta.json`: `{"agent_id": "sub-rich-a", "subagent_type": "researcher", "status": "completed", "description": "Looked up X", "created_at": ..., "updated_at": ...}`
- `prompt.txt`: 3-4 lines
- `wire.jsonl`: 8-10 events
- `context.jsonl`: 4-6 messages

- [ ] **Step 6: Add an integration test that loads the rich fixture**

Create `crates/app-core/tests/kimi_tracing_rich.rs`:

```rust
use app_core::tracing::providers::kimi::KimiTracingProvider;
use app_core::tracing::TracingProvider;

#[tokio::test]
async fn rich_fixture_loads_and_aggregates() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/kimi/sessions");
    let provider = KimiTracingProvider::new(root);

    let summary = provider.session_summary("sess-fixture-rich").await.unwrap();
    assert!(summary.turn_count >= 3);
    assert!(summary.tool_call_count >= 8);
    assert!(summary.error_count >= 1);
    assert_eq!(summary.subagent_count, 2);

    let detail = provider.load_session("sess-fixture-rich").await.unwrap();
    assert!(detail.events.len() >= 100);

    let subs = provider.list_subagents("sess-fixture-rich").await.unwrap();
    assert_eq!(subs.len(), 2);
}
```

- [ ] **Step 7: Run the test**

Run: `cargo nextest run -p app-core --test kimi_tracing_rich`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/app-core/tests/fixtures/kimi/sessions/abc123hash/sess-fixture-rich/ crates/app-core/tests/kimi_tracing_rich.rs
git commit -m "test(tracing): rich fixture for visual smoke + aggregate verification"
```

---

## Phase 6 — Tailwind island setup

### Task 6.1: Add npm dependencies

**Files:**
- Modify: `desktop-ui/package.json`

- [ ] **Step 1: Add deps**

Run from `desktop-ui/`:

```bash
cd desktop-ui && bun add \
  tailwindcss@^4 \
  @tailwindcss/vite@^4 \
  @radix-ui/react-collapsible \
  @radix-ui/react-scroll-area \
  @radix-ui/react-select \
  @radix-ui/react-separator \
  @radix-ui/react-tabs \
  @radix-ui/react-tooltip \
  @radix-ui/react-alert-dialog \
  radix-ui \
  lucide-react \
  streamdown \
  class-variance-authority \
  clsx \
  tailwind-merge \
  tw-animate-css \
  @fontsource-variable/inter
```

- [ ] **Step 2: Verify install**

Run: `cd desktop-ui && grep -E '"tailwindcss"|"lucide-react"|"streamdown"' package.json`
Expected: 3 matches.

- [ ] **Step 3: Verify nothing pre-existing broke**

Run: `cd desktop-ui && bun run typecheck`
Expected: PASS (no `src/tracing/` exists yet, so this validates the rest of the app still builds).

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/package.json desktop-ui/bun.lockb
git commit -m "chore(tracing): add Tailwind v4 + Radix + supporting deps"
```

### Task 6.2: Create `src/tracing/` skeleton

**Files:**
- Create: `desktop-ui/src/tracing/` (empty subdirs)

- [ ] **Step 1: Create the structure**

Run:
```bash
cd desktop-ui/src && mkdir -p tracing/{components/ui,features/{sessions-explorer,wire-viewer,agents-panel,context-viewer,dual-view,state-viewer,statistics,session-picker},hooks,lib,styles}
```

- [ ] **Step 2: Add a `.gitkeep` so the empty dirs are tracked, and a placeholder index**

Create `desktop-ui/src/tracing/index.tsx`:

```tsx
// SPDX-License-Identifier: Apache-2.0
// Derived from upstream Apache-2.0 source. See THIRD_PARTY_NOTICES.md.

export function TracingApp() {
  return <div className="tracing-root">Tracing port — under construction</div>;
}
```

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/tracing/
git commit -m "chore(tracing): skeleton folders + placeholder TracingApp"
```

### Task 6.3: Create Tailwind config scoped to the island

**Files:**
- Create: `desktop-ui/src/tracing/tailwind.config.ts`

- [ ] **Step 1: Write the config**

```ts
// SPDX-License-Identifier: Apache-2.0
// Derived from upstream Apache-2.0 source. See THIRD_PARTY_NOTICES.md.

import type { Config } from "tailwindcss";

const config: Config = {
  content: ["./src/tracing/**/*.{ts,tsx,css}"],
  darkMode: "class",
  theme: { extend: {} },
};

export default config;
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/tracing/tailwind.config.ts
git commit -m "chore(tracing): scoped Tailwind config"
```

### Task 6.4: Create the theme bridge CSS

**Files:**
- Create: `desktop-ui/src/tracing/styles/theme-bridge.css`

- [ ] **Step 1: Write the bridge**

```css
/* SPDX-License-Identifier: Apache-2.0 */
/* Derived from upstream Apache-2.0 source. See THIRD_PARTY_NOTICES.md. */

.tracing-root {
  --color-background: var(--ds-bg);
  --color-foreground: var(--ds-fg);
  --color-card: var(--ds-surface);
  --color-card-foreground: var(--ds-fg);
  --color-popover: var(--ds-surface, var(--ds-surface));
  --color-popover-foreground: var(--ds-fg);
  --color-primary: var(--ds-accent);
  --color-primary-foreground: var(--ds-bg);
  --color-secondary: var(--ds-bg-elevated, var(--ds-surface));
  --color-secondary-foreground: var(--ds-fg);
  --color-muted: var(--ds-bg-muted, var(--ds-surface));
  --color-muted-foreground: var(--ds-fg-muted, var(--ds-fg));
  --color-accent: var(--ds-accent);
  --color-accent-foreground: var(--ds-bg);
  --color-destructive: var(--ds-error, #ef4444);
  --color-destructive-foreground: #fff;
  --color-border: var(--ds-border);
  --color-input: var(--ds-border);
  --color-ring: var(--ds-accent);

  --radius: 6px;
  font-family: "Inter Variable", system-ui, sans-serif;
}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/tracing/styles/theme-bridge.css
git commit -m "feat(tracing): theme-bridge mapping ds-tokens -> tailwind"
```

### Task 6.5: Create the Tailwind entry CSS

**Files:**
- Create: `desktop-ui/src/tracing/styles/tracing.css`

- [ ] **Step 1: Write the entry**

```css
/* SPDX-License-Identifier: Apache-2.0 */

@import "tailwindcss";
@import "tw-animate-css";
@import "@fontsource-variable/inter";
@import "./theme-bridge.css";

@theme inline {
  --color-background: var(--color-background);
  --color-foreground: var(--color-foreground);
  --color-card: var(--color-card);
  --color-card-foreground: var(--color-card-foreground);
  --color-popover: var(--color-popover);
  --color-popover-foreground: var(--color-popover-foreground);
  --color-primary: var(--color-primary);
  --color-primary-foreground: var(--color-primary-foreground);
  --color-secondary: var(--color-secondary);
  --color-secondary-foreground: var(--color-secondary-foreground);
  --color-muted: var(--color-muted);
  --color-muted-foreground: var(--color-muted-foreground);
  --color-accent: var(--color-accent);
  --color-accent-foreground: var(--color-accent-foreground);
  --color-destructive: var(--color-destructive);
  --color-destructive-foreground: var(--color-destructive-foreground);
  --color-border: var(--color-border);
  --color-input: var(--color-input);
  --color-ring: var(--color-ring);
  --radius-md: var(--radius);
}

.tracing-root {
  background: var(--color-background);
  color: var(--color-foreground);
  min-height: 100%;
}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/tracing/styles/tracing.css
git commit -m "feat(tracing): tailwind entry + @theme bridge"
```

### Task 6.6: Wire `@tailwindcss/vite` into the Vite config

**Files:**
- Modify: `desktop-ui/vite.config.ts`

- [ ] **Step 1: Add the plugin**

At the top of `vite.config.ts`:

```ts
import tailwindcss from "@tailwindcss/vite";
```

In the `plugins: [...]` array, append `tailwindcss()`.

- [ ] **Step 2: Verify the dev server starts and the Tailwind utilities resolve**

Update `desktop-ui/src/tracing/index.tsx` to include a Tailwind class that exercises the theme bridge:

```tsx
export function TracingApp() {
  return (
    <div className="tracing-root bg-background text-foreground p-4">
      Tracing port — under construction
    </div>
  );
}
```

Add the entry CSS import in `desktop-ui/src/main.tsx`:

```ts
import "./tracing/styles/tracing.css";
```

(This import only loads the Tailwind utilities for classes that appear inside `src/tracing/**`. Other parts of the app are unaffected because the `content` glob excludes them.)

- [ ] **Step 3: Smoke-test**

Run: `cd desktop-ui && bun run typecheck`
Expected: PASS.

Run: `cd desktop-ui && bun run dev:vite` and open `localhost:1420` (need a temporary mount point to actually see the placeholder; otherwise visual verification waits until Phase 13).

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/vite.config.ts desktop-ui/src/main.tsx desktop-ui/src/tracing/index.tsx
git commit -m "feat(tracing): wire @tailwindcss/vite plugin"
```

---

## Phase 7 — Attribution files

### Task 7.1: Create project-root `THIRD_PARTY_NOTICES.md`

**Files:**
- Create: `THIRD_PARTY_NOTICES.md`

- [ ] **Step 1: Write the file**

```markdown
# Third-Party Notices

This project includes software developed by third parties. Their licenses
and attributions are listed below.

## Tracing UI (`desktop-ui/src/tracing/`)

Portions of `desktop-ui/src/tracing/` are derived from the kimi-cli
project (https://github.com/MoonshotAI/kimi-cli), licensed under the
Apache License, Version 2.0.

Copyright 2024-2025 Moonshot AI

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.

A copy of the Apache License, Version 2.0 is included at
`desktop-ui/LICENSES/apache-2.0-tracing-ui.txt`.

The ported source has been modified: identifiers, file paths, theme
tokens, and integration glue have been adapted for klyntbot. See
git history for the change record.
```

- [ ] **Step 2: Commit**

```bash
git add THIRD_PARTY_NOTICES.md
git commit -m "docs: third-party notices for tracing UI port"
```

### Task 7.2: Add the Apache-2.0 license copy

**Files:**
- Create: `desktop-ui/LICENSES/apache-2.0-tracing-ui.txt`

- [ ] **Step 1: Copy the upstream license text**

```bash
mkdir -p desktop-ui/LICENSES
cp references/kimi-cli/LICENSE desktop-ui/LICENSES/apache-2.0-tracing-ui.txt
```

- [ ] **Step 2: Verify the file is the standard Apache-2.0 text**

Run: `head -5 desktop-ui/LICENSES/apache-2.0-tracing-ui.txt`
Expected: Apache License header.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/LICENSES/apache-2.0-tracing-ui.txt
git commit -m "docs: include Apache-2.0 license copy for tracing UI"
```

---

## Phase 8 — Adapter layer (lib/utils, lib/cache, lib/api)

### Task 8.1: Port `lib/utils.ts`

**Files:**
- Source: `references/kimi-cli/vis/src/lib/utils.ts`
- Create: `desktop-ui/src/tracing/lib/utils.ts`

- [ ] **Step 1: Read the upstream file**

Run: `cat references/kimi-cli/vis/src/lib/utils.ts`
This file is small (utility helpers like `cn` for class merging, time formatting, etc.).

- [ ] **Step 2: Copy verbatim, prepend SPDX header, fix imports**

Create `desktop-ui/src/tracing/lib/utils.ts` with the SPDX header at the top:

```ts
// SPDX-License-Identifier: Apache-2.0
// Derived from upstream Apache-2.0 source. See THIRD_PARTY_NOTICES.md.
```

Then copy the upstream body. Adjust any internal imports (none expected for utils).

- [ ] **Step 3: Verify identifiers contain no upstream project name**

Run: `grep -i 'kimi\|moonshot' desktop-ui/src/tracing/lib/utils.ts`
Expected: 0 matches.

- [ ] **Step 4: Typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/tracing/lib/utils.ts
git commit -m "feat(tracing): port lib/utils"
```

### Task 8.2: Port `lib/cache.ts`

**Files:**
- Source: `references/kimi-cli/vis/src/lib/cache.ts`
- Create: `desktop-ui/src/tracing/lib/cache.ts`

- [ ] **Step 1: Copy verbatim with SPDX header**

Same procedure as 8.1.

- [ ] **Step 2: Acceptance check**

Run: `grep -i 'kimi\|moonshot' desktop-ui/src/tracing/lib/cache.ts`
Expected: 0 matches.

- [ ] **Step 3: Typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/tracing/lib/cache.ts
git commit -m "feat(tracing): port lib/cache"
```

### Task 8.3: Write the adapter `lib/api.ts` — type definitions

**Files:**
- Create: `desktop-ui/src/tracing/lib/api.ts`

- [ ] **Step 1: Write file with SPDX header + types**

```ts
// SPDX-License-Identifier: Apache-2.0
// Derived from upstream Apache-2.0 source. See THIRD_PARTY_NOTICES.md.

import { invoke } from "@/api/client";
import { apiCache } from "./cache";

const PROVIDER_ID = "kimi";

// ── Upstream-shaped types the ported components import ───────────────

export interface SessionMetadataInfo {
  session_id: string;
  title: string;
  title_generated: boolean;
  archived: boolean;
  archived_at: number | null;
  auto_archive_exempt: boolean;
  wire_mtime: number | null;
}

export interface SessionInfo {
  session_id: string;
  session_dir: string;
  work_dir: string | null;
  work_dir_hash: string;
  title: string;
  last_updated: number;
  has_wire: boolean;
  has_context: boolean;
  has_state: boolean;
  metadata: SessionMetadataInfo | null;
  wire_size: number;
  context_size: number;
  state_size: number;
  total_size: number;
  turns: number;
  imported?: boolean;
  subagent_count?: number;
}

export interface SessionSummary {
  turns: number;
  steps: number;
  tool_calls: number;
  errors: number;
  compactions: number;
  duration_sec: number;
  input_tokens: number;
  output_tokens: number;
  wire_size: number;
  context_size: number;
  state_size: number;
  total_size: number;
}

export interface WireEvent {
  index: number;
  timestamp: number;
  type: string;
  payload: Record<string, unknown>;
}

export interface WireResponse {
  total: number;
  events: WireEvent[];
}

export interface ContentPart {
  type: string;
  text?: string;
  think?: string;
  thinking?: string;
  encrypted?: string;
  image_url?: { url: string; id?: string };
  audio_url?: { url: string; id?: string };
  video_url?: { url: string; id?: string };
  [key: string]: unknown;
}

export interface ToolCallItem {
  id: string;
  type: string;
  function: { name: string; arguments: string };
  extras?: Record<string, unknown>;
}

export interface ContextMessage {
  index: number;
  role: string;
  content?: ContentPart[] | string;
  tool_calls?: ToolCallItem[];
  tool_call_id?: string;
  name?: string;
  partial?: boolean;
  token_count?: number;
  id?: number;
  [key: string]: unknown;
}

export interface ContextResponse {
  total: number;
  messages: ContextMessage[];
}

export interface AggregateStats {
  total_sessions: number;
  total_turns: number;
  total_tokens: { input: number; output: number };
  total_duration_sec: number;
  tool_usage: { name: string; count: number; error_count: number }[];
  daily_usage: { date: string; sessions: number; turns: number }[];
  per_project: { work_dir: string; sessions: number; turns: number }[];
}

export interface VisCapabilities {
  open_in_supported: boolean;
}

export type SubagentStatus =
  | "idle"
  | "running_foreground"
  | "running_background"
  | "completed"
  | "failed"
  | "killed";

export interface SubagentInfo {
  agent_id: string;
  subagent_type: string;
  status: SubagentStatus;
  description: string;
  created_at: number;
  updated_at: number;
  last_task_id: string | null;
  wire_size: number;
  context_size: number;
  launch_spec: Record<string, unknown>;
}

export function normalizeContent(
  content: ContentPart[] | string | undefined | null,
): ContentPart[] {
  if (!content) return [];
  if (typeof content === "string") return [{ type: "text", text: content }];
  if (Array.isArray(content)) return content;
  return [];
}
```

- [ ] **Step 2: Typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: PASS (file is types-only; nothing exports yet that the rest of the app uses).

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/tracing/lib/api.ts
git commit -m "feat(tracing): adapter type definitions"
```

### Task 8.4: Adapter — `listSessions`

**Files:**
- Modify: `desktop-ui/src/tracing/lib/api.ts`

- [ ] **Step 1: Append the function**

```ts
type BackendSessionSummary = {
  sessionId: string;
  providerId: string;
  sourceDir: string;
  cwd: string | null;
  projectBasename: string | null;
  customTitle: string | null;
  startedAt: string;
  lastEventAt: string;
  sizeBytes: number;
  turnCount: number;
  stepCount: number;
  toolCallCount: number;
  errorCount: number;
  subagentCount: number;
  hasWire: boolean;
  hasContext: boolean;
  imported: boolean;
  workDirHash: string;
  hasState: boolean;
  wireSize: number;
  contextSize: number;
  stateSize: number;
  totalSize: number;
  metadata: {
    sessionId: string;
    title: string;
    titleGenerated: boolean;
    archived: boolean;
    archivedAt: number | null;
    autoArchiveExempt: boolean;
    wireMtime: number | null;
  } | null;
};

function reshapeSession(b: BackendSessionSummary): SessionInfo {
  return {
    session_id: b.sessionId,
    session_dir: b.sourceDir,
    work_dir: b.cwd,
    work_dir_hash: b.workDirHash,
    title: b.customTitle ?? b.metadata?.title ?? b.sessionId,
    last_updated: new Date(b.lastEventAt).getTime() / 1000,
    has_wire: b.hasWire,
    has_context: b.hasContext,
    has_state: b.hasState,
    metadata: b.metadata
      ? {
          session_id: b.metadata.sessionId,
          title: b.metadata.title,
          title_generated: b.metadata.titleGenerated,
          archived: b.metadata.archived,
          archived_at: b.metadata.archivedAt,
          auto_archive_exempt: b.metadata.autoArchiveExempt,
          wire_mtime: b.metadata.wireMtime,
        }
      : null,
    wire_size: b.wireSize,
    context_size: b.contextSize,
    state_size: b.stateSize,
    total_size: b.totalSize,
    turns: b.turnCount,
    imported: b.imported,
    subagent_count: b.subagentCount,
  };
}

export async function listSessions(forceRefresh = false): Promise<SessionInfo[]> {
  if (forceRefresh) apiCache.invalidate("sessions");
  return apiCache.get(
    "sessions",
    async () => {
      const rows = await invoke<BackendSessionSummary[]>("tracing_list_sessions", {
        providerId: PROVIDER_ID,
      });
      return rows.map(reshapeSession);
    },
    30_000,
  );
}
```

- [ ] **Step 2: Typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/tracing/lib/api.ts
git commit -m "feat(tracing): adapter listSessions"
```

### Task 8.5: Adapter — `getWireEvents` + `normalizeWireEvents` helper

**Files:**
- Modify: `desktop-ui/src/tracing/lib/api.ts`

- [ ] **Step 1: Append**

```ts
const CONTENT_PART_MAP: Record<string, string> = {
  text: "TextPart",
  think: "ThinkPart",
};

function normalizeWireEvents(res: WireResponse): WireResponse {
  return {
    ...res,
    events: res.events.map((e) => {
      if (e.type === "ContentPart" && typeof (e.payload as any).type === "string") {
        const mapped = CONTENT_PART_MAP[(e.payload as any).type];
        if (mapped) return { ...e, type: mapped };
      }
      if (e.type === "SubagentEvent" && (e.payload as any).event && typeof (e.payload as any).event === "object") {
        const inner = (e.payload as any).event as Record<string, unknown>;
        if (inner.type === "ContentPart" && inner.payload && typeof inner.payload === "object") {
          const innerPayload = inner.payload as Record<string, unknown>;
          const mapped = CONTENT_PART_MAP[innerPayload.type as string];
          if (mapped) {
            return { ...e, payload: { ...e.payload, event: { ...inner, type: mapped } } };
          }
        }
      }
      return e;
    }),
  };
}

type BackendTraceEvent = {
  seq: number;
  providerId: string;
  rawKind: string;
  payload: Record<string, unknown>;
  occurredAt: string;
  category: string;
  turnIndex: number | null;
  stepIndex: number | null;
  parentSubagentId: string | null;
};

type BackendSessionDetail = {
  sessionId: string;
  providerId: string;
  scope: { kind: "main" } | { kind: "subagent"; agentId: string };
  stats: Record<string, unknown>;
  events: BackendTraceEvent[];
  truncated: boolean;
  totalEventCount: number;
};

function reshapeWire(detail: BackendSessionDetail): WireResponse {
  return {
    total: detail.totalEventCount,
    events: detail.events.map((ev) => ({
      index: ev.seq,
      timestamp: new Date(ev.occurredAt).getTime() / 1000,
      type: ev.rawKind,
      payload: ev.payload,
    })),
  };
}

export function getWireEvents(sessionId: string, forceRefresh = false): Promise<WireResponse> {
  const key = `wire:${sessionId}`;
  if (forceRefresh) apiCache.invalidate(key);
  return apiCache.get(key, async () => {
    const detail = await invoke<BackendSessionDetail>("tracing_load_session", {
      providerId: PROVIDER_ID,
      sessionId,
    });
    return normalizeWireEvents(reshapeWire(detail));
  });
}
```

- [ ] **Step 2: Typecheck + commit**

Run: `cd desktop-ui && bun run typecheck`
Expected: PASS.

```bash
git add desktop-ui/src/tracing/lib/api.ts
git commit -m "feat(tracing): adapter getWireEvents + normalizeWireEvents"
```

### Task 8.6: Adapter — `getContextMessages`

**Files:**
- Modify: `desktop-ui/src/tracing/lib/api.ts`

- [ ] **Step 1: Append**

```ts
type BackendContextMessage = {
  index: number;
  role: string;
  content: unknown;
};

function reshapeContext(rows: BackendContextMessage[]): ContextResponse {
  return {
    total: rows.length,
    messages: rows.map((m) => ({
      index: m.index,
      role: m.role,
      content: m.content as ContextMessage["content"],
    })),
  };
}

export function getContextMessages(
  sessionId: string,
  forceRefresh = false,
): Promise<ContextResponse> {
  const key = `context:${sessionId}`;
  if (forceRefresh) apiCache.invalidate(key);
  return apiCache.get(key, async () => {
    const rows = await invoke<BackendContextMessage[]>("tracing_load_context", {
      providerId: PROVIDER_ID,
      sessionId,
    });
    return reshapeContext(rows);
  });
}
```

- [ ] **Step 2: Typecheck + commit**

```bash
cd desktop-ui && bun run typecheck
git add desktop-ui/src/tracing/lib/api.ts
git commit -m "feat(tracing): adapter getContextMessages"
```

### Task 8.7: Adapter — `getSessionState`, `getSessionSummary`

**Files:**
- Modify: `desktop-ui/src/tracing/lib/api.ts`

- [ ] **Step 1: Append**

```ts
export function getSessionState(
  sessionId: string,
  forceRefresh = false,
): Promise<Record<string, unknown>> {
  const key = `state:${sessionId}`;
  if (forceRefresh) apiCache.invalidate(key);
  return apiCache.get(key, () =>
    invoke<Record<string, unknown>>("tracing_load_state", {
      providerId: PROVIDER_ID,
      sessionId,
    }),
  );
}

export function getSessionSummary(
  sessionId: string,
  forceRefresh = false,
): Promise<SessionSummary> {
  const key = `summary:${sessionId}`;
  if (forceRefresh) apiCache.invalidate(key);
  return apiCache.get(key, async () => {
    const b = await invoke<BackendSessionSummary>("tracing_session_summary", {
      providerId: PROVIDER_ID,
      sessionId,
    });
    return {
      turns: b.turnCount,
      steps: b.stepCount,
      tool_calls: b.toolCallCount,
      errors: b.errorCount,
      compactions: 0,
      duration_sec: 0,
      input_tokens: 0,
      output_tokens: 0,
      wire_size: b.wireSize,
      context_size: b.contextSize,
      state_size: b.stateSize,
      total_size: b.totalSize,
    };
  });
}
```

- [ ] **Step 2: Typecheck + commit**

```bash
cd desktop-ui && bun run typecheck
git add desktop-ui/src/tracing/lib/api.ts
git commit -m "feat(tracing): adapter getSessionState + getSessionSummary"
```

### Task 8.8: Adapter — subagent functions

**Files:**
- Modify: `desktop-ui/src/tracing/lib/api.ts`

- [ ] **Step 1: Append**

```ts
type BackendSubagentSummary = {
  agentId: string;
  subagentType: string;
  status: string;
  description: string | null;
  createdAt: string;
  updatedAt: string;
  eventCount: number;
};

function reshapeSubagent(b: BackendSubagentSummary): SubagentInfo {
  return {
    agent_id: b.agentId,
    subagent_type: b.subagentType,
    status: b.status as SubagentStatus,
    description: b.description ?? "",
    created_at: new Date(b.createdAt).getTime() / 1000,
    updated_at: new Date(b.updatedAt).getTime() / 1000,
    last_task_id: null,
    wire_size: 0,
    context_size: 0,
    launch_spec: {},
  };
}

export function getSubagents(sessionId: string, forceRefresh = false): Promise<SubagentInfo[]> {
  const key = `subagents:${sessionId}`;
  if (forceRefresh) apiCache.invalidate(key);
  return apiCache.get(key, async () => {
    const rows = await invoke<BackendSubagentSummary[]>("tracing_list_subagents", {
      providerId: PROVIDER_ID,
      sessionId,
    });
    return rows.map(reshapeSubagent);
  });
}

export function getSubagentWireEvents(
  sessionId: string,
  agentId: string,
  forceRefresh = false,
): Promise<WireResponse> {
  const key = `subagent-wire:${sessionId}:${agentId}`;
  if (forceRefresh) apiCache.invalidate(key);
  return apiCache.get(key, async () => {
    const detail = await invoke<BackendSessionDetail>("tracing_load_subagent_session", {
      providerId: PROVIDER_ID,
      sessionId,
      agentId,
    });
    return normalizeWireEvents(reshapeWire(detail));
  });
}

export function getSubagentContextMessages(
  sessionId: string,
  agentId: string,
  forceRefresh = false,
): Promise<ContextResponse> {
  const key = `subagent-context:${sessionId}:${agentId}`;
  if (forceRefresh) apiCache.invalidate(key);
  return apiCache.get(key, async () => {
    const rows = await invoke<BackendContextMessage[]>("tracing_load_subagent_context", {
      providerId: PROVIDER_ID,
      sessionId,
      agentId,
    });
    return reshapeContext(rows);
  });
}
```

- [ ] **Step 2: Typecheck + commit**

```bash
cd desktop-ui && bun run typecheck
git add desktop-ui/src/tracing/lib/api.ts
git commit -m "feat(tracing): adapter subagent functions"
```

### Task 8.9: Adapter — `getAggregateStats`, `getVisCapabilities`, misc

**Files:**
- Modify: `desktop-ui/src/tracing/lib/api.ts`

- [ ] **Step 1: Append**

```ts
type BackendStats = {
  perProject: { projectBasename: string; cwd: string; sessionCount: number; turnCount: number; toolCallCount: number; errorCount: number; totalInputTokens: number; totalOutputTokens: number; cacheReadTokens: number }[];
  toolUsage: { tool: string; callCount: number; errorCount: number }[];
  errorsByTool: { tool: string; errorCount: number }[];
  tokenSeries: { day: string; inputTokens: number; outputTokens: number }[];
  subagentTypes: { subagentType: string; count: number }[];
  cacheHitPct: number;
};

export async function getAggregateStats(forceRefresh = false): Promise<AggregateStats> {
  const key = "aggregate-stats";
  if (forceRefresh) apiCache.invalidate(key);
  return apiCache.get(key, async () => {
    const b = await invoke<BackendStats>("tracing_stats", { providerId: PROVIDER_ID });
    const totalSessions = b.perProject.reduce((s, p) => s + p.sessionCount, 0);
    const totalTurns = b.perProject.reduce((s, p) => s + p.turnCount, 0);
    const totalInput = b.perProject.reduce((s, p) => s + p.totalInputTokens, 0);
    const totalOutput = b.perProject.reduce((s, p) => s + p.totalOutputTokens, 0);
    return {
      total_sessions: totalSessions,
      total_turns: totalTurns,
      total_tokens: { input: totalInput, output: totalOutput },
      total_duration_sec: 0,
      tool_usage: b.toolUsage.map((t) => ({
        name: t.tool,
        count: t.callCount,
        error_count: t.errorCount,
      })),
      daily_usage: b.tokenSeries.map((d) => ({
        date: d.day,
        sessions: 0,
        turns: 0,
      })),
      per_project: b.perProject.map((p) => ({
        work_dir: p.cwd,
        sessions: p.sessionCount,
        turns: p.turnCount,
      })),
    };
  }, 60_000);
}

export function getVisCapabilities(_forceRefresh = false): Promise<VisCapabilities> {
  return Promise.resolve({ open_in_supported: true });
}

export function getSessionDownloadUrl(_sessionId: string): string {
  // Download not supported in the desktop port.
  return "";
}

export async function openInPath(_app: "finder", path: string): Promise<void> {
  await invoke("tracing_open_dir", { path });
}

export async function importSession(file: File): Promise<{ session_id: string; work_dir_hash: string }> {
  const arrayBuffer = await file.arrayBuffer();
  const bytes = Array.from(new Uint8Array(arrayBuffer));
  const result = await invoke<{ sessionId: string; workDirHash: string }>("tracing_import", {
    providerId: PROVIDER_ID,
    bytes,
    fileName: file.name,
  });
  apiCache.invalidate("sessions");
  return { session_id: result.sessionId, work_dir_hash: result.workDirHash };
}

export async function deleteSession(_sessionId: string): Promise<void> {
  throw new Error("Session deletion is not supported in the desktop port.");
}
```

- [ ] **Step 2: Typecheck + commit**

```bash
cd desktop-ui && bun run typecheck
git add desktop-ui/src/tracing/lib/api.ts
git commit -m "feat(tracing): adapter stats + capabilities + import/open"
```

### Task 8.10: Acceptance — adapter is upstream-call-compatible

- [ ] **Step 1: Verify exports match what the upstream SPA imports**

Run: `grep "from \"@/lib/api\"\|from \"@/lib/api.ts\"" references/kimi-cli/vis/src -r | sed -E 's/.*\{(.+)\}.*/\1/' | tr ',' '\n' | sed 's/^ *//;s/ *$//' | sort -u`

Expected: a list of imported names. Verify every name appears as an `export` in `desktop-ui/src/tracing/lib/api.ts`. Add any missing exports as stubs that throw.

- [ ] **Step 2: Verify no upstream project name leaked into the adapter**

Run: `grep -i 'kimi\|moonshot' desktop-ui/src/tracing/lib/api.ts`
Expected: 0 matches.

- [ ] **Step 3: Commit any fixups**

```bash
git add desktop-ui/src/tracing/lib/api.ts
git commit -m "chore(tracing): adapter export-surface parity"
```

---

## Phase 9 — UI primitives port

### Task 9.1: Port `components/ui/select.tsx`

**Files:**
- Source: `references/kimi-cli/vis/src/components/ui/select.tsx`
- Create: `desktop-ui/src/tracing/components/ui/select.tsx`

- [ ] **Step 1: Read upstream + copy with SPDX header**

Run: `cat references/kimi-cli/vis/src/components/ui/select.tsx`

Copy verbatim. Prepend SPDX header. Adjust imports: `@/lib/utils` → `@/tracing/lib/utils` (or relative path — pick one convention and stick with it for the whole island; recommendation: `@/tracing/...` alias matching the existing `@/` setup).

If the alias `@/tracing/...` doesn't resolve, add it to `desktop-ui/vite.config.ts` and `desktop-ui/tsconfig.json` `paths` block:

```json
"@/tracing/*": ["./src/tracing/*"]
```

- [ ] **Step 2: Acceptance — no upstream project name**

Run: `grep -i 'kimi\|moonshot' desktop-ui/src/tracing/components/ui/select.tsx`
Expected: 0 matches.

- [ ] **Step 3: Typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/tracing/components/ui/select.tsx
git commit -m "feat(tracing): port components/ui/select"
```

### Task 9.2: Port `components/ui/tooltip.tsx`

Same procedure as 9.1 with `tooltip.tsx`.

```bash
git commit -m "feat(tracing): port components/ui/tooltip"
```

### Task 9.3: Port `components/ui/alert-dialog.tsx`

Same procedure.

```bash
git commit -m "feat(tracing): port components/ui/alert-dialog"
```

### Task 9.4: Port `components/markdown.tsx`

**Files:**
- Source: `references/kimi-cli/vis/src/components/markdown.tsx`
- Create: `desktop-ui/src/tracing/components/markdown.tsx`

Same port procedure. This file uses `streamdown`. Verify:

Run: `grep "streamdown" desktop-ui/package.json`
Expected: 1 match.

```bash
git commit -m "feat(tracing): port components/markdown"
```

### Task 9.5: Port `hooks/use-theme.ts`

**Files:**
- Source: `references/kimi-cli/vis/src/hooks/use-theme.ts`
- Create: `desktop-ui/src/tracing/hooks/use-theme.ts`

The upstream hook reads/writes a `dark` class on `document.documentElement`. We do **not** want this to fight klyntbot's existing theme switcher — the island theme flows through the bridge instead.

- [ ] **Step 1: Port verbatim**

Same procedure as 9.1.

- [ ] **Step 2: Neutralize the side-effect**

Edit the ported file's effect that toggles `document.documentElement.classList`. Replace the body of any `useEffect` that writes to `document.documentElement.classList.add("dark")` / `.remove("dark")` with a no-op. Leave the hook's API unchanged so consumers still compile.

Suggested pattern: keep the hook's state-tracking machinery (returns the current theme name) but skip the DOM mutation.

```ts
useEffect(() => {
  // Theme is governed by klyntbot's app-wide theme system; the island
  // re-renders via CSS variables in the theme-bridge. Do not toggle a
  // `dark` class on documentElement here.
}, [theme]);
```

- [ ] **Step 3: Acceptance + typecheck**

Run: `grep -i 'kimi\|moonshot' desktop-ui/src/tracing/hooks/use-theme.ts`
Expected: 0 matches.

Run: `cd desktop-ui && bun run typecheck`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/tracing/hooks/use-theme.ts
git commit -m "feat(tracing): port use-theme hook (DOM mutation neutralized)"
```

---

## Phase 10 — Sessions explorer + session picker

### Task 10.1: Port `features/session-picker/session-picker.tsx`

Standard port procedure. After porting:

```bash
git commit -m "feat(tracing): port session-picker"
```

### Task 10.2: Port `features/sessions-explorer/explorer-toolbar.tsx`

```bash
git commit -m "feat(tracing): port explorer-toolbar"
```

### Task 10.3: Port `features/sessions-explorer/project-group.tsx`

```bash
git commit -m "feat(tracing): port project-group"
```

### Task 10.4: Port `features/sessions-explorer/session-card.tsx`

```bash
git commit -m "feat(tracing): port session-card"
```

### Task 10.5: Port `features/sessions-explorer/sessions-explorer.tsx`

After this lands, run:

Run: `cd desktop-ui && bun run typecheck`
Expected: PASS — sessions-explorer subgraph is now type-complete.

```bash
git commit -m "feat(tracing): port sessions-explorer"
```

---

## Phase 11 — Wire viewer leaf components

### Task 11.1: Port `features/wire-viewer/turn-tree.tsx`

```bash
git commit -m "feat(tracing): port turn-tree"
```

### Task 11.2: Port `features/wire-viewer/wire-event-card.tsx`

This is the largest single file (~800 lines). It dispatches per event type. After porting, no functional changes — only SPDX header + import path adjustments + identifier renames.

```bash
git commit -m "feat(tracing): port wire-event-card"
```

### Task 11.3: Port `features/wire-viewer/tool-call-detail.tsx`

```bash
git commit -m "feat(tracing): port tool-call-detail"
```

### Task 11.4: Port `features/wire-viewer/wire-filters.tsx`

```bash
git commit -m "feat(tracing): port wire-filters"
```

---

## Phase 12 — Wire viewer analytics

### Task 12.1: Port `features/wire-viewer/usage-chart.tsx`

```bash
git commit -m "feat(tracing): port usage-chart"
```

### Task 12.2: Port `features/wire-viewer/turn-efficiency.tsx`

```bash
git commit -m "feat(tracing): port turn-efficiency"
```

### Task 12.3: Port `features/wire-viewer/tool-stats-dashboard.tsx`

```bash
git commit -m "feat(tracing): port tool-stats-dashboard"
```

### Task 12.4: Port `features/wire-viewer/decision-path.tsx`

```bash
git commit -m "feat(tracing): port decision-path"
```

### Task 12.5: Port `features/wire-viewer/integrity-check.tsx`

```bash
git commit -m "feat(tracing): port integrity-check"
```

### Task 12.6: Port `features/wire-viewer/timeline-view.tsx`

(Largest analytics file, ~1200 lines.)

```bash
git commit -m "feat(tracing): port timeline-view"
```

### Task 12.7: Port `features/wire-viewer/wire-viewer.tsx` (top-level)

After this lands:

Run: `cd desktop-ui && bun run typecheck`
Expected: PASS.

```bash
git commit -m "feat(tracing): port wire-viewer top-level"
```

---

## Phase 13 — Agents / context / dual / state / statistics

### Task 13.1: Port `features/agents-panel/agent-scope-bar.tsx`

```bash
git commit -m "feat(tracing): port agent-scope-bar"
```

### Task 13.2: Port `features/agents-panel/agents-panel.tsx`

```bash
git commit -m "feat(tracing): port agents-panel"
```

### Task 13.3: Port `features/context-viewer/user-message.tsx`

```bash
git commit -m "feat(tracing): port context-viewer/user-message"
```

### Task 13.4: Port `features/context-viewer/assistant-message.tsx`

```bash
git commit -m "feat(tracing): port context-viewer/assistant-message"
```

### Task 13.5: Port `features/context-viewer/tool-call-block.tsx`

```bash
git commit -m "feat(tracing): port context-viewer/tool-call-block"
```

### Task 13.6: Port `features/context-viewer/context-space-map.tsx`

```bash
git commit -m "feat(tracing): port context-viewer/context-space-map"
```

### Task 13.7: Port `features/context-viewer/context-viewer.tsx`

```bash
git commit -m "feat(tracing): port context-viewer top-level"
```

### Task 13.8: Port `features/dual-view/dual-view.tsx`

(Scroll-locked sync is out of scope per spec §12; if upstream has sync logic, leave it intact — it just won't be exercised.)

```bash
git commit -m "feat(tracing): port dual-view"
```

### Task 13.9: Port `features/state-viewer/state-viewer.tsx`

```bash
git commit -m "feat(tracing): port state-viewer"
```

### Task 13.10: Port `features/statistics/statistics-view.tsx`

```bash
git commit -m "feat(tracing): port statistics-view"
```

---

## Phase 14 — App shell + mount

### Task 14.1: Port `App.tsx` → `TracingApp`

**Files:**
- Source: `references/kimi-cli/vis/src/App.tsx`
- Modify: `desktop-ui/src/tracing/index.tsx` (replace placeholder)

- [ ] **Step 1: Read upstream**

Run: `cat references/kimi-cli/vis/src/App.tsx`

- [ ] **Step 2: Port and rename root component**

Replace `desktop-ui/src/tracing/index.tsx`:

```tsx
// SPDX-License-Identifier: Apache-2.0
// Derived from upstream Apache-2.0 source. See THIRD_PARTY_NOTICES.md.

// (paste upstream App.tsx body here, with these adjustments:)
//   - rename the root component to TracingApp
//   - wrap the entire returned JSX in <div className="tracing-root">
//   - adjust internal imports to @/tracing/*
//   - rename any identifier or string containing the upstream project name

// export at the end:
export function TracingApp() { /* ... */ }
```

- [ ] **Step 3: Acceptance — full island has zero upstream project name leaks**

Run: `grep -ri 'kimi\|moonshot' desktop-ui/src/tracing/ | grep -v "SPDX-License-Identifier" | grep -v "Derived from upstream"`
Expected: 0 matches.

- [ ] **Step 4: Typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/tracing/index.tsx
git commit -m "feat(tracing): port App shell as TracingApp"
```

### Task 14.2: Mount `<TracingApp />` in the Coding Memory route

**Files:**
- Modify: `desktop-ui/src/features/plugins/coding-memory/CodingMemoryPlugin.tsx`

- [ ] **Step 1: Replace the file with a minimal shell**

```tsx
import { TracingApp } from "@/tracing";

export default function CodingMemoryPlugin() {
  return <TracingApp />;
}
```

(Adjust `export default` vs named export to match the existing module's consumer in the plugin registry — check how it's imported elsewhere.)

- [ ] **Step 2: Typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/plugins/coding-memory/CodingMemoryPlugin.tsx
git commit -m "feat(tracing): mount TracingApp as the Coding Memory route"
```

---

## Phase 15 — Cleanup of legacy files

### Task 15.1: Delete legacy tracing folder

**Files:**
- Delete: `desktop-ui/src/features/plugins/coding-memory/tracing/` (entire folder)

- [ ] **Step 1: Verify nothing imports from it anymore**

Run: `grep -r "features/plugins/coding-memory/tracing" desktop-ui/src/`
Expected: 0 matches.

- [ ] **Step 2: Delete**

```bash
rm -rf desktop-ui/src/features/plugins/coding-memory/tracing/
```

- [ ] **Step 3: Typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add -A desktop-ui/src/features/plugins/coding-memory/tracing/
git commit -m "chore(tracing): remove legacy tracing folder"
```

### Task 15.2: Delete legacy `WireViewer.tsx` + `TurnTree.tsx`

- [ ] **Step 1: Verify no consumers**

Run: `grep -r "coding-memory/WireViewer\|coding-memory/TurnTree" desktop-ui/src/`
Expected: 0 matches.

- [ ] **Step 2: Delete**

```bash
rm desktop-ui/src/features/plugins/coding-memory/WireViewer.tsx
rm desktop-ui/src/features/plugins/coding-memory/TurnTree.tsx
```

- [ ] **Step 3: Typecheck + commit**

```bash
cd desktop-ui && bun run typecheck
git add -A desktop-ui/src/features/plugins/coding-memory/
git commit -m "chore(tracing): remove legacy WireViewer and TurnTree"
```

### Task 15.3: Drop `tracing.css` from the global stylesheet

**Files:**
- Modify: `desktop-ui/src/styles/index.css`
- Delete: `desktop-ui/src/styles/tracing.css`

- [ ] **Step 1: Remove the `@import "./tracing.css"` line**

Run: `sed -i.bak '/tracing\.css/d' desktop-ui/src/styles/index.css && rm desktop-ui/src/styles/index.css.bak`

- [ ] **Step 2: Delete the file**

```bash
rm desktop-ui/src/styles/tracing.css
```

- [ ] **Step 3: Typecheck + commit**

```bash
cd desktop-ui && bun run typecheck
git add -A desktop-ui/src/styles/
git commit -m "chore(tracing): remove legacy tracing.css from global stylesheet"
```

---

## Phase 16 — Backend cleanup

### Task 16.1: Fix the ~16 clippy warnings inside `crates/app-core/src/tracing/`

**Files:**
- Modify: per warning.

- [ ] **Step 1: List warnings**

Run: `cargo clippy -p app-core --all-targets 2>&1 | grep "src/tracing" | head -40`

Expected hits (per audit):
- Unused `PathBuf` import in `import.rs:5`
- Unused `Path` import in `stats.rs:8`
- `identity_op` in `loader.rs` test (`2490 + 9216 + 0`)
- `sort_by_key` simplification in loader, stats, discovery
- `map_or` simplification in `discovery.rs:41,57,94`
- `field assignment outside of initializer` in `loader.rs:47-48`
- `function has too many arguments (8/7)` in loader and stats — bundle into a struct or apply `#[allow(clippy::too_many_arguments)]` with a comment justifying the boundary

- [ ] **Step 2: Fix each warning**

Use the exact change clippy suggests. For too-many-arguments, prefer a struct refactor; if that ripples too far, allow it with a one-line justification.

- [ ] **Step 3: Verify zero warnings inside `tracing/`**

Run: `cargo clippy -p app-core --all-targets 2>&1 | grep "src/tracing"`
Expected: 0 matches.

- [ ] **Step 4: Verify tests still pass**

Run: `cargo nextest run -p app-core tracing`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/tracing/
git commit -m "chore(tracing): clear clippy warnings"
```

---

## Phase 17 — Verification gates

### Task 17.1: Backend full test sweep

- [ ] **Step 1**

Run: `cargo nextest run -p app-core`
Expected: PASS.

- [ ] **Step 2**

Run: `cargo nextest run -p desktop`
Expected: PASS, including `bindings_are_current`, `registration_drift`, `no_raw_tauri_command_outside_macros`.

- [ ] **Step 3**

Run: `cargo clippy -p app-core --all-targets`
Run: `cargo clippy -p desktop --all-targets`
Expected: zero warnings inside `crates/app-core/src/tracing/` and `crates/desktop/src/commands/tracing.rs` (pre-existing warnings outside this scope are acceptable).

- [ ] **Step 4**

Run: `cargo fmt --all --check`
Expected: PASS. If not, run `cargo fmt --all` and commit.

### Task 17.2: Frontend full test sweep

- [ ] **Step 1**

Run: `cd desktop-ui && bun run typecheck`
Expected: PASS.

- [ ] **Step 2**

Run: `cd desktop-ui && bun run lint 2>&1 | grep "src/tracing/"`
Expected: 0 matches (zero lint errors in the island).

- [ ] **Step 3**

Run: `cd desktop-ui && bun run test`
Expected: PASS.

### Task 17.3: Manual smoke against the rich fixture

- [ ] **Step 1: Point dev mode at the fixture**

Set `KLYNTBOT_HOME=$PWD/crates/app-core/tests/fixtures` (or copy `sess-fixture-rich/` into `~/.klyntbot-dev/`'s tracing source path) so `KimiTracingProvider` finds it on launch.

- [ ] **Step 2: Run the desktop app**

In two terminals:
```bash
cd desktop-ui && bun run dev:vite
cargo tauri dev
```

- [ ] **Step 3: Walk every surface**

Open Coding Memory in the desktop app. Verify:
- Sessions explorer lists `sess-fixture-rich` and `sess-fixture-001`
- Clicking a session opens wire-viewer with hierarchical turn tree, turn-by-turn navigation
- Wire-filters: kind chips, presets, search, error nav, view modes (chart / events / timeline / decisions) all functional
- Tool call detail shows args + result + (paired) tool result
- Agents panel shows two subagents with Gantt rows; clicking switches scope
- Context viewer renders user/assistant/tool-call blocks
- Dual view shows wire + context side-by-side
- State viewer shows todos
- Statistics renders charts (token series, subagent type breakdown)
- Theme switch (dark/light/dim/system in klyntbot's settings) re-themes the tracing surface

- [ ] **Step 4: Document any deltas**

Note any visual or behavioral drift from upstream in `docs/superpowers/specs/2026-05-02-tracing-ui-port-design.md` under "Risks / known issues" if discovered. Most expected drift: theme tokens that don't have a klyntbot equivalent (mitigation: add to `ds-tokens.css`).

### Task 17.4: License compliance audit

- [ ] **Step 1: Verify attribution files**

Run: `ls THIRD_PARTY_NOTICES.md desktop-ui/LICENSES/apache-2.0-tracing-ui.txt`
Expected: both exist.

- [ ] **Step 2: Verify SPDX headers on every ported file**

Run: `find desktop-ui/src/tracing -name '*.tsx' -o -name '*.ts' -o -name '*.css' | xargs grep -L "SPDX-License-Identifier"`
Expected: empty output (no files missing the header).

- [ ] **Step 3: Verify no upstream project name appears outside attribution files**

Run: `grep -ri 'kimi\|moonshot' desktop-ui/src/tracing/ | grep -v "SPDX-License-Identifier" | grep -v "Derived from upstream"`
Expected: 0 matches.

- [ ] **Step 4: Commit any final fixes**

```bash
git add -A
git commit -m "chore(tracing): final license compliance audit"
```

### Task 17.5: Open the PR

```bash
git push -u origin feature/tracing
gh pr create --title "feat(tracing): port upstream tracing UI" --body "$(cat <<'EOF'
## Summary
- Replaces the existing tracing UI with a verbatim port of the upstream Apache-2.0 tracing visualization SPA, mounted as a Tailwind v4 island at `desktop-ui/src/tracing/`.
- Adds three Tauri commands (`tracing_session_summary`, `tracing_load_subagent_session`, `tracing_load_subagent_context`) and extends `SessionSummary` with port-required fields.
- Re-themes via klyntbot's `--ds-*` design tokens; attribution lives in `THIRD_PARTY_NOTICES.md` and `desktop-ui/LICENSES/apache-2.0-tracing-ui.txt`.

## Test plan
- [ ] `cargo nextest run -p app-core` green
- [ ] `cargo nextest run -p desktop` green
- [ ] `cargo clippy -p app-core --all-targets` shows 0 warnings inside `src/tracing/`
- [ ] `cd desktop-ui && bun run typecheck && bun run lint && bun run test` green
- [ ] Manual: walk every surface against the rich fixture, theme switches re-theme the island

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

## Self-review notes

- Spec §6 backend extensions: covered by Phases 1-4.
- Spec §7 adapter layer: covered by Phase 8.
- Spec §8 theme mapping: covered by Phase 6 (Tasks 6.4-6.5).
- Spec §9 deletion list: covered by Phase 15.
- Spec §10 attribution scheme: covered by Phase 7 + per-file SPDX header convention enforced in Tasks 17.4.2-17.4.3.
- Spec §11 fixture additions: covered by Phase 5.
- Spec §12 out of scope: respected — no live tail, dual-view scroll-sync left intact-but-unused, download URL stubbed, delete throws, `archived` always false, single provider.
- Spec §13 risks: addressed by `content` glob (risk 1), island-only utility usage (risk 2), token-bridge rather than hardcode (risk 3), stubbed metadata (risk 4), explicit bindings regen step in Phase 4 (risk 5), Tailwind v4 JIT keeps bundle small (risk 6).
- Spec §14 phasing: this plan is the detailed expansion.
- Spec §15 verification gates: covered by Phase 17.

Type consistency check:
- `SessionSummary` field set is consistent across Tasks 1.2, 8.4, 8.7.
- `SessionDetail` shape consistent in Tasks 2.2, 8.5, 8.8.
- Adapter cache keys consistent across Tasks 8.4-8.8.
- `PROVIDER_ID = "kimi"` constant used consistently across all adapter functions.

Placeholder scan: no TBD/TODO; every step has either exact code or an explicit run command with expected output. Component-port tasks intentionally don't embed upstream code — the convention paragraph at the top establishes the procedure once.
