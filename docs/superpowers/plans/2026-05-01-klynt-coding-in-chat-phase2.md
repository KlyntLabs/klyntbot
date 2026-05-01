# Klynt Coding-in-Chat — Phase 2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship Phase 2 of the chat-based coding agent: Mirror-learned approval (Layer 3), file snapshots + `/sessions rewind`, real `tool_search`, `/sessions export` + `/sessions fork`, the `/dead-ends` / `/mirror` / `/permissions clear-mirror` slash commands, per-thread cost ceiling with `MirrorAlert`, Settings hooks display + skill install-from-URL polish, and a performance pass to hit the 800 ms p95 first-token gate.

**Architecture:** Phase 1 left every Phase 2 hook in place — `ApprovalLayer::Layer3Mirror` enum variant, the `// Phase 2; skipped here.` line in `guard.rs:115`, the `tool_search` no-op stub, the absent `coding_sessions_*` commands. Phase 2 fills the seats. Layer 3 = a 7th `MirrorSignalSource` (`ApprovalHistorySource`) that subscribes to `ApprovalResolved` events and a small evaluator function called from `guard::evaluate`. Snapshots = a new `coding_snapshots` table written *before* `EditTool`/`WriteTool`/`ApplyPatchTool` mutate disk, replayed by a rewind handler. Cost ceiling = a Mirror alert (per spec §10 `CostThresholdCrossed` was deliberately *not* added to `AgentEvent`). Slash commands ride the existing `useSlashCommands` registry. Skill install-from-URL is mostly UI-complete; we promote the helper to `klynt-skill-loader` for reuse.

**Tech Stack:** Rust 1.93 stable, `cargo nextest`, `proptest`, SQLite (via existing `StoragePool`), Tauri 2 + specta, React 19, Vitest, plain CSS, `bun`, `tracing::instrument`. Rules: zero clippy warnings, `#[tracing::instrument(skip(self), err)]` on every new `AppCore` handler method, additive migrations consolidated per CLAUDE.md pre-release policy.

---

## File Structure

### New files
- `crates/cognitive/src/mirror/sources/approval_history.rs` — 7th `MirrorSignalSource`; ingests `ApprovalResolved`, writes per-`(tool, args_hash, repo_id)` history rows.
- `crates/cognitive/src/mirror/sources/cost_ceiling.rs` — 8th `MirrorSignalSource`; subscribes to `AgentEvent::UsageReport`, fires `MirrorAlert` when per-thread or per-session cost crosses a threshold.
- `crates/klynt-core/src/approval/layer3.rs` — pure evaluator for Mirror-learned auto-approve (input: history + config; output: `ApprovalDecision`).
- `crates/klynt-core/src/snapshots/mod.rs` — `SnapshotService` trait + struct; called from edit-family tools before disk mutation.
- `crates/klynt-core/src/snapshots/repo.rs` — `SnapshotRepo` (CRUD over `coding_snapshots`).
- `crates/storage/src/repos/coding_approval_history.rs` — `CodingApprovalHistoryRepo`.
- `crates/desktop/src/commands/coding_sessions_v2.rs` — new Tauri commands: `coding_sessions_export`, `coding_sessions_fork`, `coding_sessions_rewind`, `coding_permissions_clear_mirror`.
- `desktop-ui/src/features/settings/components/sections/coding/HooksSubsection.tsx`
- `desktop-ui/src/features/coding/components/CostCeilingBanner.tsx`
- `tests/integration/coding_in_chat/property_k10_mirror_cache_poisoning.rs` — K10 proptest.
- `tests/integration/coding_in_chat/property_k11_starred_retention.rs` — K11 proptest.
- `tests/integration/coding_in_chat/scenario_mirror_auto_approve.rs` — scenario test.
- `tests/integration/coding_in_chat/scenario_rewind.rs` — scenario test.
- `crates/agent/benches/chat_send_to_first_token.rs` — Phase-2 perf gate bench.

### Modified files
- `crates/storage/migrations/001_initial.sql` — add `coding_snapshots` + `coding_approval_history` tables (consolidated per pre-release policy).
- `crates/storage/src/repos/session.rs` — add `fork_session`, `export_session_md`, `export_session_json`, `rewind_to_message`, `decrement_starred_prune`.
- `crates/storage/src/repos/mod.rs` — register the new repo.
- `crates/klynt-core/src/approval/guard.rs` — replace the `// Phase 2; skipped here.` line with a real Layer 3 call.
- `crates/klynt-core/src/approval/decision.rs` — add `ApprovalDecision::AutoMirror { reason }` constructor (or repurpose existing).
- `crates/klynt-core/src/tools/edit.rs` / `write.rs` / `apply_patch.rs` — pre-mutation snapshot hook.
- `crates/klynt-core/src/tools/tool_search.rs` — replace stub with real reranker.
- `crates/klynt-core/src/tools/mod.rs` — re-exports.
- `crates/klynt-core/src/lib.rs` — re-export `snapshots`.
- `crates/cognitive/src/mirror/engine.rs:34` — add the 7th + 8th sources to `MirrorEngine::start`.
- `crates/cognitive/src/mirror/sources/mod.rs` — pub mod entries.
- `crates/agent/src/output/cost_tracker.rs` — extend with per-thread ledger keyed by `session_key`; new `record_for_session` + `check_session_ceiling`.
- `crates/agent/src/agent_runtime/runtime.rs:811` — call `record_for_session` alongside `record`.
- `crates/app-core/src/state.rs` — handler methods for the 4 new Tauri commands; add a `cost_ceiling_per_thread_usd` config getter.
- `crates/app-core/src/init/mod.rs:530` — pass new repos + bus to `MirrorEngine::start`.
- `crates/app-core/src/coding/skills_handler.rs:134` — delegate `install_from_url` to `klynt-skill-loader::load_from_url`.
- `crates/klynt-skill-loader/src/lib.rs` — new public `load_from_url` function.
- `crates/desktop/src/lib.rs` — register the 4 new commands in `klynt_collect_commands![...]`.
- `crates/desktop-shared/src/lib.rs` — IPC payload types: `SessionExportArgs`, `SessionExportResult`, `SessionForkArgs`, `SessionRewindArgs`, `ClearMirrorCacheArgs`.
- `crates/config/src/schema/coding.rs` — `permissions.mirrorLearning` (already accepted in Phase 1; widen its semantics if needed) + new `costCeiling.perThreadUsd`.
- `desktop-ui/src/features/coding/slash/registry.ts` — new entries for `/sessions rewind`, `/sessions export`, `/sessions fork`, `/dead-ends`, `/mirror`, `/permissions clear-mirror`.
- `desktop-ui/src/features/settings/components/sections/SettingsCodingSection.tsx` — register Hooks tab.
- `desktop-ui/src/features/settings/components/sections/coding/SkillsSubsection.tsx` — URL-validation feedback.
- `desktop-ui/src/features/coding/components/ApprovalCard.tsx` — render Mirror history line + auto-approval reason.
- `desktop-ui/src/features/chat/components/ChatHeader.tsx` (or equivalent) — mount `CostCeilingBanner`.
- `desktop-ui/src/bindings.ts` — auto-regenerated by `cargo tauri dev`.
- `docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md` — flip Phase 2 deliverables from "deferred" to "shipped" once green; not part of code tasks.

---

## Conventions used in this plan

- Every new `AppCore` handler method gets `#[tracing::instrument(skip(self), err)]`.
- Every Tauri command in `commands/` is a thin `#[klynt_command]` adapter delegating to the AppCore handler.
- After adding a Tauri command, run `cargo tauri dev` once to regenerate `desktop-ui/src/bindings.ts` (or `cargo test -p desktop registration_drift` to confirm). Don't edit `bindings.ts` by hand.
- Tests use `StoragePool::connect_in_memory()`. No filesystem mutation outside `TempDir`.
- Test naming: integration tests in `tests/integration/coding_in_chat/`; property tests in same folder, prefix `property_`; scenarios prefix `scenario_`.
- Commits are conventional: `feat(scope): …` / `fix(scope): …` / `test(scope): …` / `refactor(scope): …`.

---

# Workstream A — Mirror-Learned Approval (Layer 3)

Spec anchors: §7 "Layer 3 — Mirror-learned (Phase 2; opt-in)" lines 780-797; §13 deliverable line 1390; K10 invariant.

### Task A1: Schema — `coding_approval_history` table

**Files:**
- Modify: `crates/storage/migrations/001_initial.sql`

- [ ] **Step 1: Append the table definition**

Append after the existing `sessions` table block:

```sql
-- Phase 2: Mirror-learned approval cache (Layer 3)
-- Append-only log of every ApprovalResolved event keyed by (tool, args_hash, repo_id).
CREATE TABLE IF NOT EXISTS coding_approval_history (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    tool            TEXT NOT NULL,
    args_hash       TEXT NOT NULL,
    repo_id         TEXT NOT NULL DEFAULT '',
    decision        TEXT NOT NULL,           -- 'allow' | 'deny'
    decided_by      TEXT NOT NULL,           -- 'user' | 'auto_allow' | 'auto_deny' | 'timeout' | 'cancelled'
    layer           TEXT NOT NULL,           -- which layer fired
    created_at      INTEGER NOT NULL DEFAULT (cast(strftime('%s','now') as integer))
);

CREATE INDEX IF NOT EXISTS idx_coding_approval_history_key
  ON coding_approval_history(tool, args_hash, repo_id);

CREATE INDEX IF NOT EXISTS idx_coding_approval_history_clear
  ON coding_approval_history(tool, repo_id);
```

- [ ] **Step 2: Run migrations smoke test**

Run: `cargo nextest run -p storage`
Expected: PASS (existing migration tests should auto-pick up the new DDL via `connect_in_memory`).

- [ ] **Step 3: Commit**

```bash
git add crates/storage/migrations/001_initial.sql
git commit -m "feat(storage): add coding_approval_history table for Mirror Layer 3"
```

### Task A2: `CodingApprovalHistoryRepo`

**Files:**
- Create: `crates/storage/src/repos/coding_approval_history.rs`
- Modify: `crates/storage/src/repos/mod.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/storage/src/repos/coding_approval_history.rs` and put a `#[cfg(test)]` module at the bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::StoragePool;

    #[tokio::test]
    async fn record_and_summary_round_trip() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = CodingApprovalHistoryRepo::new(pool.clone());
        repo.record(HistoryEntry { tool: "bash".into(), args_hash: "abc".into(), repo_id: "r1".into(),
            decision: "allow".into(), decided_by: "user".into(), layer: "ask".into() }).await.unwrap();
        repo.record(HistoryEntry { tool: "bash".into(), args_hash: "abc".into(), repo_id: "r1".into(),
            decision: "allow".into(), decided_by: "user".into(), layer: "ask".into() }).await.unwrap();
        let summary = repo.summary("bash", "abc", "r1").await.unwrap();
        assert_eq!(summary.approval_count, 2);
        assert_eq!(summary.denial_count, 0);
    }

    #[tokio::test]
    async fn single_denial_marks_history_poisoned() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = CodingApprovalHistoryRepo::new(pool.clone());
        for _ in 0..10 {
            repo.record(HistoryEntry { tool: "bash".into(), args_hash: "x".into(), repo_id: "r".into(),
                decision: "allow".into(), decided_by: "user".into(), layer: "ask".into() }).await.unwrap();
        }
        repo.record(HistoryEntry { tool: "bash".into(), args_hash: "x".into(), repo_id: "r".into(),
            decision: "deny".into(), decided_by: "user".into(), layer: "ask".into() }).await.unwrap();
        let s = repo.summary("bash", "x", "r").await.unwrap();
        assert_eq!(s.denial_count, 1);
        assert!(s.poisoned());
    }

    #[tokio::test]
    async fn clear_for_tool_empties_summary() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = CodingApprovalHistoryRepo::new(pool.clone());
        repo.record(HistoryEntry { tool: "bash".into(), args_hash: "z".into(), repo_id: "r".into(),
            decision: "allow".into(), decided_by: "user".into(), layer: "ask".into() }).await.unwrap();
        repo.clear_for_tool("bash", Some("r")).await.unwrap();
        let s = repo.summary("bash", "z", "r").await.unwrap();
        assert_eq!(s.approval_count, 0);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p storage coding_approval_history`
Expected: FAIL — symbols not defined.

- [ ] **Step 3: Implement the repo**

Replace the file body (above the test module) with:

```rust
use crate::StoragePool;
use common::Result;
use sqlx::Row;

#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub tool: String,
    pub args_hash: String,
    pub repo_id: String,
    pub decision: String,
    pub decided_by: String,
    pub layer: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApprovalHistorySummary {
    pub approval_count: u32,
    pub denial_count: u32,
    pub last_decided_at: Option<i64>,
}

impl ApprovalHistorySummary {
    pub fn poisoned(&self) -> bool { self.denial_count > 0 }
}

#[derive(Clone)]
pub struct CodingApprovalHistoryRepo { pool: StoragePool }

impl CodingApprovalHistoryRepo {
    pub fn new(pool: StoragePool) -> Self { Self { pool } }

    #[tracing::instrument(skip(self), err)]
    pub async fn record(&self, entry: HistoryEntry) -> Result<()> {
        sqlx::query(
            "INSERT INTO coding_approval_history \
             (tool, args_hash, repo_id, decision, decided_by, layer) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&entry.tool).bind(&entry.args_hash).bind(&entry.repo_id)
        .bind(&entry.decision).bind(&entry.decided_by).bind(&entry.layer)
        .execute(self.pool.inner()).await
        .map_err(common::KlyntbotError::from)?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn summary(&self, tool: &str, args_hash: &str, repo_id: &str) -> Result<ApprovalHistorySummary> {
        let row = sqlx::query(
            "SELECT \
                SUM(CASE WHEN decision = 'allow' THEN 1 ELSE 0 END) AS allow_count, \
                SUM(CASE WHEN decision = 'deny'  THEN 1 ELSE 0 END) AS deny_count, \
                MAX(created_at) AS last_at \
             FROM coding_approval_history WHERE tool = ? AND args_hash = ? AND repo_id = ?",
        )
        .bind(tool).bind(args_hash).bind(repo_id)
        .fetch_one(self.pool.inner()).await
        .map_err(common::KlyntbotError::from)?;
        Ok(ApprovalHistorySummary {
            approval_count: row.try_get::<Option<i64>, _>("allow_count").unwrap_or(None).unwrap_or(0) as u32,
            denial_count:   row.try_get::<Option<i64>, _>("deny_count").unwrap_or(None).unwrap_or(0) as u32,
            last_decided_at: row.try_get::<Option<i64>, _>("last_at").unwrap_or(None),
        })
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn clear_for_tool(&self, tool: &str, repo_id: Option<&str>) -> Result<u64> {
        let res = match repo_id {
            Some(rid) => sqlx::query(
                "DELETE FROM coding_approval_history WHERE tool = ? AND repo_id = ?",
            ).bind(tool).bind(rid).execute(self.pool.inner()).await,
            None => sqlx::query(
                "DELETE FROM coding_approval_history WHERE tool = ?",
            ).bind(tool).execute(self.pool.inner()).await,
        }.map_err(common::KlyntbotError::from)?;
        Ok(res.rows_affected())
    }
}
```

In `crates/storage/src/repos/mod.rs` add:

```rust
pub mod coding_approval_history;
pub use coding_approval_history::{CodingApprovalHistoryRepo, HistoryEntry, ApprovalHistorySummary};
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo nextest run -p storage coding_approval_history`
Expected: 3 PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/storage/src/repos/coding_approval_history.rs crates/storage/src/repos/mod.rs
git commit -m "feat(storage): CodingApprovalHistoryRepo for Mirror Layer 3"
```

`★ Insight ─────────────────────────────────────`
- We **don't** add `parent_session_id` FK constraints (CLAUDE.md gotcha repeats this — pre-release policy means consolidated migrations, no incremental files). The `coding_approval_history` table follows the same convention: indexes for hot lookups, no FKs.
- `args_hash_for_relevance` (spec §7 line 797) — Phase 2 hashes the *normalized* args (volatile fields stripped). For `bash` we hash the command's first non-flag token; for `edit`/`write` we hash the path. Implementation lives in `klynt-core/src/approval/layer3.rs` (Task A4).
`─────────────────────────────────────────────────`

### Task A3: Config — `mirrorLearning` cooldown + clear semantics

**Files:**
- Modify: `crates/config/src/schema/coding.rs`

The field `permissions.mirrorLearning: bool` already exists in Phase 1. Add the supporting tunables.

- [ ] **Step 1: Write the failing test**

Append to `crates/config/src/schema/coding.rs` test module:

```rust
#[test]
fn permissions_defaults_include_mirror_cooldown() {
    let p: CodingPermissions = serde_json::from_str("{}").unwrap();
    assert!(!p.mirror_learning);
    assert_eq!(p.mirror_min_approvals, 5);
    assert_eq!(p.mirror_cooldown_hours, 24);
}
```

- [ ] **Step 2: Run** — Expected: FAIL (fields missing).

`cargo nextest run -p config -E 'test(permissions_defaults_include_mirror_cooldown)'`

- [ ] **Step 3: Implement**

Add to the `CodingPermissions` struct in the same file (preserve `serde(rename_all = "camelCase")`):

```rust
#[serde(default = "default_mirror_min_approvals")]
pub mirror_min_approvals: u32,
#[serde(default = "default_mirror_cooldown_hours")]
pub mirror_cooldown_hours: u32,
```

And the helpers near the file's other defaults:

```rust
fn default_mirror_min_approvals() -> u32 { 5 }
fn default_mirror_cooldown_hours() -> u32 { 24 }
```

- [ ] **Step 4: Run** — Expected: PASS.

`cargo nextest run -p config`

- [ ] **Step 5: Commit**

```bash
git add crates/config/src/schema/coding.rs
git commit -m "feat(config): add mirror_min_approvals + cooldown_hours for Layer 3"
```

### Task A4: `klynt-core` Layer 3 evaluator

**Files:**
- Create: `crates/klynt-core/src/approval/layer3.rs`
- Modify: `crates/klynt-core/src/approval/mod.rs`

- [ ] **Step 1: Write the failing test**

Create the file with both impl + tests:

```rust
use storage::repos::ApprovalHistorySummary;

pub struct Layer3Config {
    pub enabled: bool,
    pub min_approvals: u32,
    pub cooldown_seconds: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Layer3Outcome {
    AutoAllow { reason: String },
    Ask { reason: String },
    FallThrough,
}

pub fn evaluate(cfg: &Layer3Config, summary: &ApprovalHistorySummary, now_unix: i64) -> Layer3Outcome {
    if !cfg.enabled { return Layer3Outcome::FallThrough; }
    if summary.denial_count >= 1 {
        return Layer3Outcome::Ask { reason: "mirror: prior denial — always confirm".into() };
    }
    if summary.approval_count < cfg.min_approvals {
        return Layer3Outcome::FallThrough;
    }
    let last = summary.last_decided_at.unwrap_or(0);
    if now_unix - last < cfg.cooldown_seconds {
        return Layer3Outcome::FallThrough;
    }
    Layer3Outcome::AutoAllow {
        reason: format!("mirror: {}+ prior approvals, no denials", summary.approval_count),
    }
}

pub fn args_hash_for_relevance(tool: &str, args_json: &str) -> String {
    use blake3::Hasher;
    let mut h = Hasher::new();
    h.update(tool.as_bytes());
    h.update(b"\0");
    let normalized = match tool {
        "bash" => normalize_bash(args_json),
        "edit" | "write" | "apply_patch" => normalize_path(args_json),
        _ => args_json.to_string(),
    };
    h.update(normalized.as_bytes());
    h.finalize().to_hex().to_string()
}

fn normalize_bash(args_json: &str) -> String {
    let v: serde_json::Value = serde_json::from_str(args_json).unwrap_or(serde_json::Value::Null);
    let cmd = v.get("command").and_then(|c| c.as_str()).unwrap_or("");
    cmd.split_whitespace().next().unwrap_or("").to_string()
}

fn normalize_path(args_json: &str) -> String {
    let v: serde_json::Value = serde_json::from_str(args_json).unwrap_or(serde_json::Value::Null);
    v.get("path").and_then(|p| p.as_str()).unwrap_or("").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn cfg(enabled: bool) -> Layer3Config {
        Layer3Config { enabled, min_approvals: 5, cooldown_seconds: 86400 }
    }
    fn s(ok: u32, deny: u32, last: Option<i64>) -> ApprovalHistorySummary {
        ApprovalHistorySummary { approval_count: ok, denial_count: deny, last_decided_at: last }
    }

    #[test] fn disabled_falls_through() {
        assert_eq!(evaluate(&cfg(false), &s(100, 0, Some(0)), 999_999), Layer3Outcome::FallThrough);
    }
    #[test] fn single_denial_locks_to_ask() {
        match evaluate(&cfg(true), &s(100, 1, Some(0)), 999_999) {
            Layer3Outcome::Ask { .. } => {}, other => panic!("got {other:?}"),
        }
    }
    #[test] fn under_threshold_falls_through() {
        assert_eq!(evaluate(&cfg(true), &s(4, 0, Some(0)), 999_999), Layer3Outcome::FallThrough);
    }
    #[test] fn cooldown_falls_through() {
        // 5 approvals but last decided 1 hour ago → still in cooldown
        assert_eq!(evaluate(&cfg(true), &s(5, 0, Some(999_000)), 999_999 + 3600), Layer3Outcome::FallThrough);
    }
    #[test] fn five_approvals_post_cooldown_auto_allow() {
        match evaluate(&cfg(true), &s(5, 0, Some(0)), 90_000 + 999_999) {
            Layer3Outcome::AutoAllow { .. } => {},
            other => panic!("got {other:?}"),
        }
    }
    #[test] fn args_hash_strips_command_args() {
        let a = args_hash_for_relevance("bash", r#"{"command":"git status"}"#);
        let b = args_hash_for_relevance("bash", r#"{"command":"git status --short"}"#);
        assert_eq!(a, b, "trailing flags should not change the relevance hash");
    }
}
```

In `crates/klynt-core/src/approval/mod.rs` add `pub mod layer3;` and re-export the names.

If `blake3` isn't already in `klynt-core/Cargo.toml`, add it under `[dependencies]`: `blake3 = { workspace = true }` (it's already a workspace dep elsewhere; if not, add `blake3 = "1"`).

- [ ] **Step 2: Run** — Expected: FAIL until impl compiles, then 6 PASS.

`cargo nextest run -p klynt-core layer3`

- [ ] **Step 3: Confirm green and commit**

```bash
git add crates/klynt-core/src/approval/layer3.rs crates/klynt-core/src/approval/mod.rs crates/klynt-core/Cargo.toml
git commit -m "feat(klynt-core): Layer 3 Mirror approval evaluator"
```

### Task A5: Wire Layer 3 into `guard::evaluate`

**Files:**
- Modify: `crates/klynt-core/src/approval/guard.rs:115`

- [ ] **Step 1: Add the failing test**

Append to `crates/klynt-core/tests/approval_guard.rs`:

```rust
#[tokio::test]
async fn layer3_auto_allows_after_5_prior_approvals_when_enabled() {
    use storage::repos::{CodingApprovalHistoryRepo, HistoryEntry};
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    let history = CodingApprovalHistoryRepo::new(pool.clone());
    for _ in 0..5 {
        history.record(HistoryEntry {
            tool: "bash".into(),
            args_hash: klynt_core::approval::layer3::args_hash_for_relevance("bash", r#"{"command":"echo hi"}"#),
            repo_id: "test-repo".into(),
            decision: "allow".into(),
            decided_by: "user".into(),
            layer: "ask".into(),
        }).await.unwrap();
    }
    // Build GuardCtx with mirror_learning=true and a clock 25h in the future
    let ctx = test_helpers::guard_ctx_with_history(history.clone(), /*mirror_learning*/ true, /*now_offset_h*/ 25);
    let decision = klynt_core::approval::guard::evaluate(ctx, "bash", r#"{"command":"echo hi"}"#).await;
    assert!(matches!(decision.layer(), klynt_core::approval::ApprovalLayer::Layer3Mirror));
    assert!(decision.is_allow(), "expected auto-allow, got {decision:?}");
}
```

(`test_helpers::guard_ctx_with_history` — add a small helper at the top of the file mirroring how the existing tests build `GuardCtx`. Existing tests in `crates/klynt-core/tests/approval_guard.rs` already do this; copy that pattern and inject the optional `Arc<CodingApprovalHistoryRepo>` field you'll add in Step 3.)

- [ ] **Step 2: Run** — Expected: FAIL (`Layer3Mirror` never produced today).

`cargo nextest run -p klynt-core layer3_auto_allows`

- [ ] **Step 3: Add `history_repo` to `GuardCtx` and call Layer 3**

Edit `crates/klynt-core/src/approval/guard.rs` — extend `GuardCtx` (around `:16`):

```rust
pub struct GuardCtx<'a> {
    /* ...existing fields... */
    pub history_repo: Option<std::sync::Arc<storage::repos::CodingApprovalHistoryRepo>>,
    pub repo_id: String,             // empty string for non-repo sessions
    pub mirror_learning_enabled: bool,
    pub mirror_min_approvals: u32,
    pub mirror_cooldown_seconds: i64,
    pub now_unix: i64,
}
```

Replace the `// 3. Layer 3 Mirror-learned — Phase 2; skipped here.` block with:

```rust
// 3. Layer 3 — Mirror-learned (opt-in)
if let Some(repo) = ctx.history_repo.as_ref() {
    let cfg = crate::approval::layer3::Layer3Config {
        enabled: ctx.mirror_learning_enabled,
        min_approvals: ctx.mirror_min_approvals,
        cooldown_seconds: ctx.mirror_cooldown_seconds,
    };
    let hash = crate::approval::layer3::args_hash_for_relevance(tool, payload);
    let summary = repo.summary(tool, &hash, &ctx.repo_id).await
        .unwrap_or_default();
    match crate::approval::layer3::evaluate(&cfg, &summary, ctx.now_unix) {
        crate::approval::layer3::Layer3Outcome::AutoAllow { reason } => {
            return ApprovalDecision::auto_allow(ApprovalLayer::Layer3Mirror, reason);
        }
        crate::approval::layer3::Layer3Outcome::Ask { reason } => {
            return ApprovalDecision::ask(ApprovalLayer::Layer3Mirror, reason);
        }
        crate::approval::layer3::Layer3Outcome::FallThrough => { /* continue */ }
    }
}
```

If `ApprovalDecision::auto_allow` / `::ask` constructors don't exist with those exact names, add them next to the existing constructors in `decision.rs` — match the existing style. The decision returned must carry `ApprovalLayer::Layer3Mirror` so the K8 round-trip identity test (Phase 1) keeps working.

Update every existing call-site of `GuardCtx { ... }` (search workspace for `GuardCtx {` — there should be ~5 hits in tests + `crates/klynt-core/src/registry/builder.rs` + the two bash/exec tools). Each call site fills the new fields with sensible defaults: `history_repo: None, repo_id: String::new(), mirror_learning_enabled: false, mirror_min_approvals: 5, mirror_cooldown_seconds: 86400, now_unix: jiff::Timestamp::now().as_second()`.

- [ ] **Step 4: Run** — Expected: PASS, plus all Phase 1 approval tests still green.

`cargo nextest run -p klynt-core` then `cargo nextest run --workspace` to spot any breakage in callers.

- [ ] **Step 5: Commit**

```bash
git add crates/klynt-core/src/approval/ crates/klynt-core/tests/approval_guard.rs
git commit -m "feat(klynt-core): plug Layer 3 Mirror evaluator into guard pipeline"
```

`★ Insight ─────────────────────────────────────`
- We deliberately keep Layer 3 *additive* on `GuardCtx` rather than introducing a separate `MirrorAwareGuardCtx`. Phase 1 already proved this is the cleanest place to inject — and it lets Layer 3 remain trivially `None` during unit tests of unrelated tools.
- The decision returned uses `ApprovalLayer::Layer3Mirror`. This means `ToolEvent::ApprovalRequested.layer == "layer3-mirror"` will appear on the wire; the React `ApprovalCard` already has a generic "Layer:" line that will render it for free. We add the human-friendly Mirror history line in a later task.
`─────────────────────────────────────────────────`

### Task A6: 7th `MirrorSignalSource` — `ApprovalHistorySource`

**Files:**
- Create: `crates/cognitive/src/mirror/sources/approval_history.rs`
- Modify: `crates/cognitive/src/mirror/sources/mod.rs`
- Modify: `crates/cognitive/src/mirror/engine.rs:34`

- [ ] **Step 1: Write the failing test**

Create `crates/cognitive/tests/mirror_approval_history_source.rs`:

```rust
use storage::StoragePool;
use storage::repos::CodingApprovalHistoryRepo;
use cognitive::mirror::sources::approval_history::ApprovalHistorySource;
use ai_core::events::tool::ToolEvent;

#[tokio::test]
async fn records_resolved_approval_into_history_repo() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repo = CodingApprovalHistoryRepo::new(pool.clone());
    let source = ApprovalHistorySource::new(repo.clone());

    // Synthesize a paired ApprovalRequested + ApprovalResolved as a fake stream
    let req = ToolEvent::ApprovalRequested {
        request_id: "r1".into(), tool: "bash".into(),
        args_hash: "hash-of-bash-args".into(), layer: "ask".into(),
        rule_matched: None, mirror_history: None, requires_user_input: true,
        args: r#"{"command":"git status"}"#.into(),
        cwd: "/tmp".into(), layer_reason: "ask".into(),
    };
    let res = ToolEvent::ApprovalResolved {
        request_id: "r1".into(), decision: "allow".into(),
        decision_reason: "user".into(), latency_ms: 10,
        persisted_rule: None, decided_by: "user".into(),
    };
    source.observe(&req, "test-repo").await;
    source.observe(&res, "test-repo").await;
    let s = repo.summary("bash", "hash-of-bash-args", "test-repo").await.unwrap();
    assert_eq!(s.approval_count, 1);
}
```

(Field names above mirror what Agent #1 reported at `crates/tools-core/src/events.rs:45`. If they drift, follow the actual struct.)

- [ ] **Step 2: Run** — Expected: FAIL.

`cargo nextest run -p cognitive mirror_approval_history_source`

- [ ] **Step 3: Implement the source**

```rust
// crates/cognitive/src/mirror/sources/approval_history.rs
use std::sync::Arc;
use ai_core::events::tool::ToolEvent;
use ai_core::mirror::MirrorSignalSource;
use storage::repos::{CodingApprovalHistoryRepo, HistoryEntry};
use dashmap::DashMap;

pub struct ApprovalHistorySource {
    repo: Arc<CodingApprovalHistoryRepo>,
    pending: DashMap<String, PendingReq>,
}

struct PendingReq { tool: String, args_hash: String, layer: String, repo_id: String }

impl ApprovalHistorySource {
    pub fn new(repo: CodingApprovalHistoryRepo) -> Self {
        Self { repo: Arc::new(repo), pending: DashMap::new() }
    }

    pub async fn observe(&self, ev: &ToolEvent, repo_id: &str) {
        match ev {
            ToolEvent::ApprovalRequested { request_id, tool, args_hash, layer, .. } => {
                self.pending.insert(request_id.clone(), PendingReq {
                    tool: tool.clone(), args_hash: args_hash.clone(),
                    layer: layer.clone(), repo_id: repo_id.to_string(),
                });
            }
            ToolEvent::ApprovalResolved { request_id, decision, decided_by, .. } => {
                if let Some((_, pending)) = self.pending.remove(request_id) {
                    let _ = self.repo.record(HistoryEntry {
                        tool: pending.tool, args_hash: pending.args_hash,
                        repo_id: pending.repo_id, decision: decision.clone(),
                        decided_by: decided_by.clone(), layer: pending.layer,
                    }).await;
                }
            }
            _ => {}
        }
    }
}

#[async_trait::async_trait]
impl MirrorSignalSource for ApprovalHistorySource {
    fn name(&self) -> &'static str { "approval_history" }
    // The trait's main `consume` method is implemented by polling whatever event channel
    // the engine wires in. The bridge function `observe` above is what gets dispatched
    // from the consume loop. Match the existing trait shape — see sources/routing.rs.
}
```

Check `sources/routing.rs` for the exact `MirrorSignalSource` impl shape (subscribe to bus, drain into `observe`). Mirror that pattern.

In `crates/cognitive/src/mirror/sources/mod.rs`:

```rust
pub mod approval_history;
```

In `crates/cognitive/src/mirror/engine.rs:34` (`MirrorEngine::start`), add a new param `approval_history_repo: Option<Arc<CodingApprovalHistoryRepo>>` to the signature, and below the existing 6 source constructions:

```rust
let approval_history_source = approval_history_repo.as_ref().map(|r|
    ApprovalHistorySource::new((**r).clone())
);
```

Push it into the same `consumers` Vec as the other sources.

- [ ] **Step 4: Run tests** — Expected: PASS.

`cargo nextest run -p cognitive mirror_approval_history_source`

- [ ] **Step 5: Update the `MirrorEngine::start` call site**

`crates/app-core/src/init/mod.rs:530` — add the new arg:

```rust
let started = ::cognitive::mirror::MirrorEngine::start(
    repo, narrative_handler, autotuner_bridge, episodic_repo,
    rule_repo, trial_evaluator,
    Some(Arc::new(coding_approval_history_repo.clone())),
)?;
```

(Construct `coding_approval_history_repo` near where the other repos are built — search for `SessionRepo::new` in init/mod.rs.)

- [ ] **Step 6: Workspace build + commit**

```bash
cargo build --workspace
cargo clippy --workspace --all-targets -- -D warnings
git add crates/cognitive/src/mirror/ crates/cognitive/tests/ crates/app-core/src/init/mod.rs
git commit -m "feat(cognitive): 7th MirrorSignalSource — approval history"
```

### Task A7: K10 proptest — Mirror-learned cache poisoning

**Files:**
- Create: `tests/integration/coding_in_chat/property_k10_mirror_cache_poisoning.rs`

- [ ] **Step 1: Write the proptest**

```rust
use proptest::prelude::*;
use storage::StoragePool;
use storage::repos::{CodingApprovalHistoryRepo, HistoryEntry};
use klynt_core::approval::layer3::{evaluate, Layer3Config, Layer3Outcome};

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, .. ProptestConfig::default() })]

    #[test]
    fn k10_single_denial_anywhere_in_history_forces_ask(
        approvals_before in 0u32..50,
        approvals_after  in 0u32..50,
    ) {
        tokio::runtime::Runtime::new().unwrap().block_on(async move {
            let pool = StoragePool::connect_in_memory().await.unwrap();
            let repo = CodingApprovalHistoryRepo::new(pool.clone());
            for _ in 0..approvals_before {
                repo.record(HistoryEntry {
                    tool: "bash".into(), args_hash: "h".into(), repo_id: "r".into(),
                    decision: "allow".into(), decided_by: "user".into(), layer: "ask".into(),
                }).await.unwrap();
            }
            repo.record(HistoryEntry {
                tool: "bash".into(), args_hash: "h".into(), repo_id: "r".into(),
                decision: "deny".into(), decided_by: "user".into(), layer: "ask".into(),
            }).await.unwrap();
            for _ in 0..approvals_after {
                repo.record(HistoryEntry {
                    tool: "bash".into(), args_hash: "h".into(), repo_id: "r".into(),
                    decision: "allow".into(), decided_by: "user".into(), layer: "ask".into(),
                }).await.unwrap();
            }
            let s = repo.summary("bash", "h", "r").await.unwrap();
            let outcome = evaluate(
                &Layer3Config { enabled: true, min_approvals: 5, cooldown_seconds: 0 },
                &s, i64::MAX,
            );
            prop_assert!(matches!(outcome, Layer3Outcome::Ask { .. }),
                "K10 violated: a denial in history must force Ask regardless of allow count, got {outcome:?}");
            Ok(())
        }).unwrap();
    }
}
```

If `tests/integration/coding_in_chat/` is the test-binary form (single `mod.rs` registering individual files), follow the existing pattern from `property_k8_approval_roundtrip.rs`.

- [ ] **Step 2: Register the binary**

Confirm the file is picked up by `cargo nextest list -E 'test(k10_single_denial)'`. If a `mod.rs` is needed to declare the file, add it.

- [ ] **Step 3: Run** — Expected: 256 cases pass.

`cargo nextest run -E 'test(k10_single_denial)'`

- [ ] **Step 4: Commit**

```bash
git add tests/integration/coding_in_chat/property_k10_mirror_cache_poisoning.rs
git commit -m "test(coding): K10 proptest — single denial poisons Mirror cache"
```

### Task A8: `/permissions clear-mirror <tool>` — Tauri command + slash entry

**Files:**
- Modify: `crates/app-core/src/state.rs`
- Modify: `crates/desktop/src/commands/coding_sessions_v2.rs` (created in Workstream B; if not yet, create here)
- Modify: `desktop-ui/src/features/coding/slash/registry.ts`

- [ ] **Step 1: AppCore handler**

Add to `crates/app-core/src/state.rs`:

```rust
#[tracing::instrument(skip(self), err)]
pub async fn coding_permissions_clear_mirror(&self, tool: String, repo_id: Option<String>) -> common::Result<u64> {
    let repo = self.coding_approval_history_repo.clone()
        .ok_or_else(|| common::KlyntbotError::other("approval history repo not initialized"))?;
    repo.clear_for_tool(&tool, repo_id.as_deref()).await
}
```

Ensure `AppCore` carries an `Option<Arc<CodingApprovalHistoryRepo>>` field; initialize in `init/mod.rs` next to the existing repos.

- [ ] **Step 2: Tauri shim**

Create or extend `crates/desktop/src/commands/coding_sessions_v2.rs`:

```rust
use desktop_macros::klynt_command;
use crate::state::AppState;

#[klynt_command]
pub async fn coding_permissions_clear_mirror(state: AppState, tool: String, repo_id: Option<String>) -> u64 {
    state.app_core.coding_permissions_clear_mirror(tool, repo_id).await
}
```

Register in `crates/desktop/src/lib.rs` `klynt_collect_commands![..., commands::coding_sessions_v2::coding_permissions_clear_mirror, ...]`.

- [ ] **Step 3: Slash registry entry**

In `desktop-ui/src/features/coding/slash/registry.ts`:

```ts
"/permissions": {
  kind: "branch",
  children: {
    "clear-mirror": {
      kind: "leaf",
      path: "direct",
      tauriCommand: "coding_permissions_clear_mirror",
      command: "/permissions clear-mirror",
      description: "Reset Mirror-learned approval cache for a tool",
      argHint: "<tool>",
      category: "permissions",
      // Renders userInput confirmation row before executing (per spec §9 risky-direct rule)
      requiresConfirmation: true,
    },
  },
},
```

- [ ] **Step 4: Regenerate bindings + run drift test**

```bash
cargo build -p desktop
cd desktop-ui && bun run typecheck && cd ..
cargo nextest run -p desktop -E 'test(registration_drift) or test(bindings_are_current)'
```

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/state.rs crates/desktop/src/commands/ crates/desktop/src/lib.rs \
    desktop-ui/src/features/coding/slash/registry.ts desktop-ui/src/bindings.ts
git commit -m "feat(coding): /permissions clear-mirror slash command + Tauri wiring"
```

### Task A9: Approval card — render Mirror history + auto-decision reason

**Files:**
- Modify: `desktop-ui/src/features/coding/components/ApprovalCard.tsx`

- [ ] **Step 1: Write the failing Vitest**

Add `desktop-ui/src/features/coding/components/ApprovalCard.test.tsx` (or extend existing):

```tsx
import { render, screen } from "@testing-library/react";
import { ApprovalCard } from "./ApprovalCard";

it("renders Mirror history line when provided", () => {
  render(<ApprovalCard request={{
    requestId: "r1", tool: "bash", argsHash: "x", layer: "layer3-mirror",
    ruleMatched: null,
    mirrorHistory: { approvals: 12, denials: 0 },
    sandboxSummary: "Seatbelt", requiresUserInput: true,
    args: { command: "cargo build" }, cwd: "/x", layerReason: "auto-approved",
  }} onResolve={() => {}} />);
  expect(screen.getByText(/12 approvals, 0 denials/i)).toBeInTheDocument();
});
```

- [ ] **Step 2: Run** — Expected: FAIL.

`cd desktop-ui && bun run test ApprovalCard`

- [ ] **Step 3: Render the line**

In `ApprovalCard.tsx`, where the existing card body is composed, add:

```tsx
{request.mirrorHistory && (
  <div className="approval-card__mirror-history">
    Mirror history: {request.mirrorHistory.approvals} approvals,{" "}
    {request.mirrorHistory.denials} denials in this repo
  </div>
)}
```

- [ ] **Step 4: Run** — Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/coding/components/ApprovalCard.tsx desktop-ui/src/features/coding/components/ApprovalCard.test.tsx
git commit -m "feat(coding): show Mirror history on approval cards"
```

### Task A10: Scenario test — full Layer 3 auto-approve loop

**Files:**
- Create: `tests/integration/coding_in_chat/scenario_mirror_auto_approve.rs`

- [ ] **Step 1: Write the scenario**

Pattern after `tests/integration/coding_in_chat/scenario_bash_happy_path.rs`. Sketch:

1. Build an in-memory pool + the new history repo.
2. Pre-populate 5 `allow` entries for `(bash, hash("git status"), "test-repo")`.
3. Build `BashTool` with a `GuardCtx` carrying `mirror_learning_enabled = true`, `now_unix` past cooldown.
4. Execute `bash {"command": "git status"}`.
5. Assert: `ToolEvent::ApprovalRequested { layer: "layer3-mirror", requires_user_input: false, .. }` followed by `ToolEvent::ApprovalResolved { decided_by: "auto_allow", .. }` — no UI prompt round-trip.
6. Assert: tool actually executed (output captured).

- [ ] **Step 2: Run** — Expected: PASS.

`cargo nextest run -E 'test(scenario_mirror_auto_approve)'`

- [ ] **Step 3: Commit**

```bash
git add tests/integration/coding_in_chat/scenario_mirror_auto_approve.rs
git commit -m "test(coding): scenario — Mirror Layer 3 auto-approves after 5 prior allows"
```

---

# Workstream B — File Snapshots + `/sessions rewind`

Spec anchors: §11 "Snapshots / rewind — Deferred to Phase 2" (line 1268); §13 line 1394 deliverable; K11 invariant.

`★ Insight ─────────────────────────────────────`
- The K11 invariant ("starred sessions are never pruned") really lives at the **session retention** layer, not the snapshots layer. We test it at the `SessionRepo::delete_stale_sessions` boundary — a starred session must survive even if it's age-eligible.
- Rewind is implemented as **file restore + message truncate**, *not* as a re-execution of the prior tool calls. Spec §11 line 1232 is explicit: "The agent loop never replays prior tool calls." We restore disk to the pre-edit state for every snapshot taken between the rewind cursor and HEAD, then truncate `messages` to that index.
`─────────────────────────────────────────────────`

### Task B1: Schema — `coding_snapshots` table

**Files:**
- Modify: `crates/storage/migrations/001_initial.sql`

- [ ] **Step 1: Append the table**

```sql
-- Phase 2: file snapshots for /sessions rewind
CREATE TABLE IF NOT EXISTS coding_snapshots (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    session_key     TEXT NOT NULL,
    message_id      TEXT,                    -- which assistant turn produced the edit (nullable)
    file_path       TEXT NOT NULL,
    content_before  BLOB NOT NULL,           -- raw bytes pre-edit; '' if file did not exist
    file_existed    INTEGER NOT NULL DEFAULT 1,  -- 0 means restoring should delete the file
    content_hash    TEXT NOT NULL,           -- blake3 of content_before
    created_at      INTEGER NOT NULL DEFAULT (cast(strftime('%s','now') as integer))
);

CREATE INDEX IF NOT EXISTS idx_coding_snapshots_session
  ON coding_snapshots(session_key, created_at);
```

- [ ] **Step 2: Run** — Expected: storage tests still pass.

`cargo nextest run -p storage`

- [ ] **Step 3: Commit**

```bash
git add crates/storage/migrations/001_initial.sql
git commit -m "feat(storage): coding_snapshots table for /sessions rewind"
```

### Task B2: `SnapshotRepo`

**Files:**
- Create: `crates/klynt-core/src/snapshots/repo.rs`
- Create: `crates/klynt-core/src/snapshots/mod.rs`
- Modify: `crates/klynt-core/src/lib.rs`

- [ ] **Step 1: Write failing tests**

`crates/klynt-core/src/snapshots/repo.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use storage::StoragePool;

    #[tokio::test]
    async fn snapshot_round_trip() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = SnapshotRepo::new(pool.clone());
        let id = repo.record("sess1", Some("msg1"), "/tmp/foo.txt", b"old", true).await.unwrap();
        let snap = repo.get(id).await.unwrap().expect("exists");
        assert_eq!(snap.content_before, b"old");
        assert!(snap.file_existed);
    }

    #[tokio::test]
    async fn list_after_returns_descending() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = SnapshotRepo::new(pool.clone());
        let _a = repo.record("s", None, "/a", b"1", true).await.unwrap();
        let b = repo.record("s", None, "/b", b"2", true).await.unwrap();
        let snaps = repo.list_for_session("s").await.unwrap();
        assert_eq!(snaps.len(), 2);
        assert_eq!(snaps[0].id, b, "newest first");
    }
}
```

- [ ] **Step 2: Run** — FAIL.

- [ ] **Step 3: Implement**

```rust
use storage::StoragePool;
use common::Result;
use sqlx::Row;

pub struct Snapshot {
    pub id: i64,
    pub session_key: String,
    pub message_id: Option<String>,
    pub file_path: String,
    pub content_before: Vec<u8>,
    pub file_existed: bool,
    pub content_hash: String,
    pub created_at: i64,
}

#[derive(Clone)]
pub struct SnapshotRepo { pool: StoragePool }

impl SnapshotRepo {
    pub fn new(pool: StoragePool) -> Self { Self { pool } }

    #[tracing::instrument(skip(self, content), err)]
    pub async fn record(&self, session_key: &str, message_id: Option<&str>,
                        file_path: &str, content: &[u8], existed: bool) -> Result<i64> {
        let hash = blake3::hash(content).to_hex().to_string();
        let res = sqlx::query(
            "INSERT INTO coding_snapshots \
             (session_key, message_id, file_path, content_before, file_existed, content_hash) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(session_key).bind(message_id).bind(file_path)
        .bind(content).bind(existed as i64).bind(&hash)
        .execute(self.pool.inner()).await
        .map_err(common::KlyntbotError::from)?;
        Ok(res.last_insert_rowid())
    }

    pub async fn get(&self, id: i64) -> Result<Option<Snapshot>> {
        let row = sqlx::query("SELECT * FROM coding_snapshots WHERE id = ?")
            .bind(id).fetch_optional(self.pool.inner()).await
            .map_err(common::KlyntbotError::from)?;
        Ok(row.map(row_to_snapshot))
    }

    pub async fn list_for_session(&self, session_key: &str) -> Result<Vec<Snapshot>> {
        let rows = sqlx::query("SELECT * FROM coding_snapshots WHERE session_key = ? ORDER BY id DESC")
            .bind(session_key).fetch_all(self.pool.inner()).await
            .map_err(common::KlyntbotError::from)?;
        Ok(rows.into_iter().map(row_to_snapshot).collect())
    }

    pub async fn list_after_message(&self, session_key: &str, message_id: &str) -> Result<Vec<Snapshot>> {
        let rows = sqlx::query(
            "SELECT s.* FROM coding_snapshots s \
             WHERE s.session_key = ? AND s.id > COALESCE( \
               (SELECT MAX(id) FROM coding_snapshots WHERE session_key = ? AND message_id = ?), 0 \
             ) ORDER BY s.id ASC",
        ).bind(session_key).bind(session_key).bind(message_id)
        .fetch_all(self.pool.inner()).await
        .map_err(common::KlyntbotError::from)?;
        Ok(rows.into_iter().map(row_to_snapshot).collect())
    }
}

fn row_to_snapshot(row: sqlx::sqlite::SqliteRow) -> Snapshot {
    Snapshot {
        id: row.get("id"), session_key: row.get("session_key"),
        message_id: row.get("message_id"), file_path: row.get("file_path"),
        content_before: row.get("content_before"),
        file_existed: row.get::<i64, _>("file_existed") != 0,
        content_hash: row.get("content_hash"), created_at: row.get("created_at"),
    }
}
```

`crates/klynt-core/src/snapshots/mod.rs`:

```rust
pub mod repo;
pub use repo::{Snapshot, SnapshotRepo};
```

`crates/klynt-core/src/lib.rs`: add `pub mod snapshots;`.

- [ ] **Step 4: Run** — PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/klynt-core/src/snapshots/ crates/klynt-core/src/lib.rs
git commit -m "feat(klynt-core): SnapshotRepo for file snapshots"
```

### Task B3: Pre-mutation snapshot hook in `EditTool`

**Files:**
- Modify: `crates/klynt-core/src/tools/edit.rs:80-82`

- [ ] **Step 1: Add a failing integration test**

`crates/klynt-core/tests/edit_takes_snapshot.rs`:

```rust
use storage::StoragePool;
use klynt_core::snapshots::SnapshotRepo;
use tempfile::TempDir;

#[tokio::test]
async fn edit_records_snapshot_before_writing() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("hello.txt");
    std::fs::write(&path, b"OLD").unwrap();
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let snap_repo = SnapshotRepo::new(pool.clone());
    let tool = test_helpers::edit_tool_with_snapshot(snap_repo.clone(), "session-x");
    tool.execute(serde_json::json!({
        "path": path.to_string_lossy(), "old_text": "OLD", "new_text": "NEW"
    })).await.unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "NEW");
    let snaps = snap_repo.list_for_session("session-x").await.unwrap();
    assert_eq!(snaps.len(), 1);
    assert_eq!(snaps[0].content_before, b"OLD");
    assert!(snaps[0].file_existed);
}
```

(`test_helpers::edit_tool_with_snapshot` — add as needed; build `EditTool` injecting an `Option<Arc<SnapshotRepo>>` + `session_key`.)

- [ ] **Step 2: Run** — FAIL.

- [ ] **Step 3: Inject the snapshot dependency**

In `crates/klynt-core/src/tools/edit.rs`:

```rust
pub struct EditTool {
    /* existing fields */
    pub snapshot_repo: Option<std::sync::Arc<crate::snapshots::SnapshotRepo>>,
    pub session_key: String,
    pub message_id: Option<String>,
}
```

In the `execute` body, *before* writing the file:

```rust
if let Some(repo) = self.snapshot_repo.as_ref() {
    let (content, existed) = match tokio::fs::read(&resolved_path).await {
        Ok(bytes) => (bytes, true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Vec::new(), false),
        Err(e) => return Err(e.into()),
    };
    let _ = repo.record(
        &self.session_key, self.message_id.as_deref(),
        &resolved_path.to_string_lossy(), &content, existed,
    ).await;
}
```

Update `EditTool::new` (and `ToolKitBuilder` in `crates/klynt-core/src/registry/builder.rs`) to thread `snapshot_repo` + `session_key` through.

- [ ] **Step 4: Run** — PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/klynt-core/src/tools/edit.rs crates/klynt-core/src/registry/builder.rs crates/klynt-core/tests/edit_takes_snapshot.rs
git commit -m "feat(klynt-core): EditTool records pre-mutation snapshot"
```

### Task B4: Snapshot hook in `WriteTool`

**Files:**
- Modify: `crates/klynt-core/src/tools/write.rs:55+`

Repeat the Task B3 pattern verbatim for `WriteTool`. Test file: `crates/klynt-core/tests/write_takes_snapshot.rs`. Note: `WriteTool` is the create-or-overwrite path, so the snapshot's `file_existed` flag is the most useful field — rewind must `unlink` files that didn't exist pre-write.

- [ ] **Step 1: Failing test**

```rust
#[tokio::test]
async fn write_records_snapshot_with_existed_false_for_new_file() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("new.txt");
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let snap_repo = SnapshotRepo::new(pool.clone());
    let tool = test_helpers::write_tool_with_snapshot(snap_repo.clone(), "s");
    tool.execute(serde_json::json!({"path": path.to_string_lossy(), "content": "hello"})).await.unwrap();
    let snaps = snap_repo.list_for_session("s").await.unwrap();
    assert!(!snaps[0].file_existed);
    assert!(snaps[0].content_before.is_empty());
}
```

- [ ] **Steps 2-5:** Same pattern as Task B3. Commit.

```bash
git commit -m "feat(klynt-core): WriteTool records pre-mutation snapshot"
```

### Task B5: Snapshot hook in `ApplyPatchTool`

**Files:**
- Modify: `crates/klynt-core/src/tools/apply_patch.rs:56+`

`apply_patch` may touch many files in one call. Snapshot **each** target path before applying.

- [ ] **Step 1: Failing test** — assert `snaps.len() == N` for an N-file patch.
- [ ] **Step 2: Run** — FAIL.
- [ ] **Step 3: Implement** — iterate the parsed patch's affected paths and call `repo.record` per path before the actual `apply()`.
- [ ] **Step 4: Run** — PASS.
- [ ] **Step 5: Commit** — `feat(klynt-core): ApplyPatchTool snapshots all affected files`.

### Task B6: `SessionRepo::rewind_to_message`

**Files:**
- Modify: `crates/storage/src/repos/session.rs`

- [ ] **Step 1: Failing test** — append:

```rust
#[tokio::test]
async fn rewind_truncates_messages_after_anchor() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repo = SessionRepo::new(pool.clone());
    repo.upsert_session("s", "{}").await.unwrap();
    for i in 0..5 {
        repo.add_message("s", &format!("m{i}"), "user", "hi", None, None, None).await.unwrap();
    }
    let removed = repo.rewind_to_message("s", "m2").await.unwrap();
    assert_eq!(removed, 2, "m3 + m4 should be removed");
    assert_eq!(repo.count_messages("s").await.unwrap(), 3);
}
```

- [ ] **Step 2: Run** — FAIL.

- [ ] **Step 3: Implement**

```rust
#[tracing::instrument(skip(self), err)]
pub async fn rewind_to_message(&self, session_key: &str, anchor_id: &str) -> Result<u64> {
    let res = sqlx::query(
        "DELETE FROM messages WHERE session_key = ? AND id IN ( \
            SELECT id FROM messages WHERE session_key = ? \
              AND created_at > (SELECT created_at FROM messages WHERE id = ? AND session_key = ?) \
         )",
    )
    .bind(session_key).bind(session_key).bind(anchor_id).bind(session_key)
    .execute(self.pool.inner()).await
    .map_err(common::KlyntbotError::from)?;
    Ok(res.rows_affected())
}
```

- [ ] **Step 4: Run** — PASS.

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(storage): SessionRepo::rewind_to_message"
```

### Task B7: Rewind orchestrator in `AppCore`

**Files:**
- Modify: `crates/app-core/src/state.rs`

- [ ] **Step 1: Add the handler**

```rust
#[tracing::instrument(skip(self), err)]
pub async fn coding_sessions_rewind(&self, session_key: String, message_id: String) -> common::Result<RewindResult> {
    let snap_repo = self.snapshot_repo.clone()
        .ok_or_else(|| common::KlyntbotError::other("snapshot repo not initialized"))?;
    let snaps = snap_repo.list_after_message(&session_key, &message_id).await?;
    let mut restored: usize = 0;
    let mut deleted: usize = 0;
    // Apply newest-first to undo in reverse order
    for snap in snaps.iter().rev() {
        if snap.file_existed {
            tokio::fs::write(&snap.file_path, &snap.content_before).await?;
            restored += 1;
        } else {
            // file didn't exist before — undo by deleting
            let _ = tokio::fs::remove_file(&snap.file_path).await;
            deleted += 1;
        }
    }
    let removed = self.repos.sessions.rewind_to_message(&session_key, &message_id).await?;
    Ok(RewindResult { messages_removed: removed, files_restored: restored, files_deleted: deleted })
}
```

`RewindResult` lives in `desktop-shared`:

```rust
#[derive(Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct RewindResult {
    pub messages_removed: u64,
    pub files_restored: usize,
    pub files_deleted: usize,
}
```

- [ ] **Step 2: Tauri command + register**

In `crates/desktop/src/commands/coding_sessions_v2.rs`:

```rust
#[klynt_command]
pub async fn coding_sessions_rewind(state: AppState, session_key: String, message_id: String) -> RewindResult {
    state.app_core.coding_sessions_rewind(session_key, message_id).await
}
```

Add to `klynt_collect_commands![...]` in `crates/desktop/src/lib.rs`.

- [ ] **Step 3: Slash registry entry**

```ts
"rewind": {
  kind: "leaf", path: "direct",
  tauriCommand: "coding_sessions_rewind",
  command: "/sessions rewind",
  description: "Restore files + delete messages back to a chosen anchor",
  argHint: "<message-id>",
  category: "sessions",
  requiresConfirmation: true,
},
```

(Place under the existing `/sessions` branch.)

- [ ] **Step 4: Regenerate bindings + run drift gate**

```bash
cargo build -p desktop
cd desktop-ui && bun run typecheck && bun run test && cd ..
cargo nextest run -p desktop -E 'test(registration_drift) or test(bindings_are_current)'
```

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/state.rs crates/desktop/src/commands/coding_sessions_v2.rs \
    crates/desktop/src/lib.rs crates/desktop-shared/src/lib.rs \
    desktop-ui/src/features/coding/slash/registry.ts desktop-ui/src/bindings.ts
git commit -m "feat(coding): /sessions rewind — file restore + message truncate"
```

### Task B8: Scenario test — full rewind round-trip

**Files:**
- Create: `tests/integration/coding_in_chat/scenario_rewind.rs`

- [ ] **Step 1: Write the scenario**

1. In a `TempDir`, create `foo.txt` = `"v1"`.
2. Open in-memory pool; build `EditTool` + `WriteTool` wired to `SnapshotRepo` + `session_key="s"`.
3. Add message `m_user` to `s`.
4. Edit `foo.txt` to `"v2"`; record assistant message `m_a1`.
5. Write new file `bar.txt = "x"`; record assistant message `m_a2`.
6. Call `AppCore::coding_sessions_rewind("s", "m_user")`.
7. Assert `foo.txt` content == `"v1"`; `bar.txt` no longer exists; `count_messages("s") == 1` (only `m_user` remains).

- [ ] **Step 2: Run** — PASS.
- [ ] **Step 3: Commit** — `test(coding): scenario — /sessions rewind restores files and truncates`.

### Task B9: K11 proptest — starred sessions never pruned

**Files:**
- Create: `tests/integration/coding_in_chat/property_k11_starred_retention.rs`
- Modify: `crates/storage/src/repos/session.rs` (extend `delete_stale_sessions` to honor `pinned = 1` if not already)

- [ ] **Step 1: Confirm current behavior**

Read `crates/storage/src/repos/session.rs:454` (`delete_stale_sessions`). If the existing query doesn't include `AND pinned = 0`, add it:

```rust
sqlx::query("DELETE FROM sessions WHERE updated_at < ? AND pinned = 0")
```

- [ ] **Step 2: Write the proptest**

```rust
use proptest::prelude::*;
use storage::StoragePool;
use storage::repos::SessionRepo;

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, .. ProptestConfig::default() })]

    #[test]
    fn k11_starred_session_survives_any_ttl(
        starred_count in 1usize..10,
        unstarred_count in 0usize..10,
        ttl_days in 0u32..365,
    ) {
        tokio::runtime::Runtime::new().unwrap().block_on(async move {
            let pool = StoragePool::connect_in_memory().await.unwrap();
            let repo = SessionRepo::new(pool.clone());
            for i in 0..starred_count {
                let key = format!("starred-{i}");
                repo.upsert_session(&key, "{}").await.unwrap();
                sqlx::query("UPDATE sessions SET pinned = 1, updated_at = 0 WHERE key = ?")
                    .bind(&key).execute(pool.inner()).await.unwrap();
            }
            for i in 0..unstarred_count {
                let key = format!("ephemeral-{i}");
                repo.upsert_session(&key, "{}").await.unwrap();
                sqlx::query("UPDATE sessions SET updated_at = 0 WHERE key = ?")
                    .bind(&key).execute(pool.inner()).await.unwrap();
            }
            repo.delete_stale_sessions(ttl_days as i64).await.unwrap();
            let surviving = repo.count_sessions().await.unwrap();
            prop_assert!(surviving as usize >= starred_count,
                "K11: {starred_count} starred sessions must survive (got {surviving})");
            Ok(())
        }).unwrap();
    }
}
```

- [ ] **Step 3: Run** — Expected: 128 cases pass.
- [ ] **Step 4: Commit** — `test(coding): K11 proptest — starred sessions survive retention pruning`.

---

# Workstream C — Real `tool_search`

Spec anchors: §13 line 1392 ("`tool_search` becomes real"); Phase 1 stub at `crates/klynt-core/src/tools/tool_search.rs:58`.

`★ Insight ─────────────────────────────────────`
- The spec calls for "Mirror per-skill effectiveness reranking" (line 19 of `tool_search.rs` Phase 1 stub comment). For Phase 2 we wire to whatever rank surface coding-memory exposes (`coding_memory_effectiveness_trends` already exists per agent #2's report at `coding_memory.rs:261`). If coding-memory's effectiveness API isn't ready, we fall back to a deterministic registry-traversal scoring (substring + alias match). No silent stub return.
`─────────────────────────────────────────────────`

### Task C1: Define the search interface

**Files:**
- Create: `crates/klynt-core/src/tools/tool_search/mod.rs` (move stub aside)
- Create: `crates/klynt-core/src/tools/tool_search/index.rs`
- Create: `crates/klynt-core/src/tools/tool_search/rerank.rs`

- [ ] **Step 1: Failing test**

```rust
// crates/klynt-core/src/tools/tool_search/index.rs

#[cfg(test)]
mod tests {
    use super::*;
    fn fake_registry() -> Vec<ToolMeta> {
        vec![
            ToolMeta { name: "bash".into(), aliases: vec![], description: "run a shell command".into() },
            ToolMeta { name: "read".into(), aliases: vec![], description: "read a file".into() },
            ToolMeta { name: "edit".into(), aliases: vec![], description: "edit a file in place".into() },
        ]
    }

    #[test] fn substring_query_returns_matches() {
        let idx = ToolIndex::build(&fake_registry());
        let hits = idx.search("file", 10);
        assert_eq!(hits.len(), 2);
        assert!(hits.iter().any(|h| h.name == "read"));
    }

    #[test] fn empty_query_returns_top_n_alphabetical() {
        let idx = ToolIndex::build(&fake_registry());
        let hits = idx.search("", 2);
        assert_eq!(hits.len(), 2);
    }
}
```

- [ ] **Step 2: Run** — FAIL.
- [ ] **Step 3: Implement**

```rust
#[derive(Debug, Clone, serde::Serialize)]
pub struct ToolMeta {
    pub name: String,
    pub aliases: Vec<String>,
    pub description: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchHit {
    pub name: String,
    pub score: f32,
    pub description: String,
}

pub struct ToolIndex { meta: Vec<ToolMeta> }

impl ToolIndex {
    pub fn build(meta: &[ToolMeta]) -> Self { Self { meta: meta.to_vec() } }

    pub fn search(&self, query: &str, top_n: usize) -> Vec<SearchHit> {
        let q = query.trim().to_lowercase();
        let mut hits: Vec<SearchHit> = self.meta.iter().map(|m| {
            let mut score = 0.0_f32;
            if !q.is_empty() {
                if m.name.to_lowercase().contains(&q) { score += 2.0; }
                if m.description.to_lowercase().contains(&q) { score += 1.0; }
                if m.aliases.iter().any(|a| a.to_lowercase().contains(&q)) { score += 1.5; }
            } else {
                score = 1.0;
            }
            SearchHit { name: m.name.clone(), score, description: m.description.clone() }
        }).filter(|h| h.score > 0.0).collect();
        hits.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.name.cmp(&b.name)));
        hits.truncate(top_n);
        hits
    }
}
```

- [ ] **Step 4: Run** — PASS.
- [ ] **Step 5: Commit** — `feat(klynt-core): tool_search lexical index + scoring`.

### Task C2: Mirror reranker (optional, additive)

**Files:**
- Create: `crates/klynt-core/src/tools/tool_search/rerank.rs`

Trait that takes `&[SearchHit]` and a Mirror effectiveness lookup `impl Fn(&str) -> Option<f32>`, applies a multiplicative boost.

- [ ] **Step 1: Failing test** — boost factor lifts a low-score hit above a high-score one when effectiveness is high.
- [ ] **Step 2-4: TDD cycle.**
- [ ] **Step 5: Commit** — `feat(klynt-core): tool_search Mirror effectiveness reranker`.

### Task C3: Replace `tool_search.rs:58` stub with real search

**Files:**
- Modify: `crates/klynt-core/src/tools/tool_search.rs:58` (or `tool_search/tool.rs` if you split).

- [ ] **Step 1: Failing tool-level test** — execute the tool with `{ "query": "file" }`, assert non-empty JSON array with `read` and `edit` entries.
- [ ] **Step 2: Run** — FAIL (today returns `"[]"`).
- [ ] **Step 3: Replace stub body**

```rust
async fn execute(&self, args: serde_json::Value) -> Result<String, ToolError> {
    let query = args.get("query").and_then(|q| q.as_str()).unwrap_or("");
    let top_n = args.get("top_n").and_then(|n| n.as_u64()).unwrap_or(10) as usize;
    let meta: Vec<ToolMeta> = self.registry.list_meta();   // add this getter to ToolRegistry
    let mut hits = ToolIndex::build(&meta).search(query, top_n);
    if let Some(reranker) = self.reranker.as_ref() {
        hits = reranker.apply(&hits);
    }
    Ok(serde_json::to_string(&hits).map_err(|e| ToolError::Internal(e.to_string()))?)
}
```

Add `ToolRegistry::list_meta()` to `crates/klynt-core/src/registry/builder.rs` (it already iterates registered tools — return their `Tool::name()` + a description string).

- [ ] **Step 4: Run** — PASS.
- [ ] **Step 5: Commit** — `feat(klynt-core): tool_search returns real ToolRegistry hits`.

---

# Workstream D — `/sessions export` + `/sessions fork`

### Task D1: `SessionRepo::export_session_md` + `_json`

**Files:**
- Modify: `crates/storage/src/repos/session.rs`

- [ ] **Step 1: Failing test**

```rust
#[tokio::test]
async fn export_md_contains_each_message_role_header() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repo = SessionRepo::new(pool.clone());
    repo.upsert_session("s", "{}").await.unwrap();
    repo.add_message("s", "m1", "user", "hi", None, None, None).await.unwrap();
    repo.add_message("s", "m2", "assistant", "hello", None, None, None).await.unwrap();
    let md = repo.export_session_md("s").await.unwrap();
    assert!(md.contains("### user"));
    assert!(md.contains("### assistant"));
    assert!(md.contains("hi"));
    assert!(md.contains("hello"));
}

#[tokio::test]
async fn export_json_round_trips_via_serde() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repo = SessionRepo::new(pool.clone());
    repo.upsert_session("s", r#"{"title":"x"}"#).await.unwrap();
    repo.add_message("s", "m1", "user", "ping", None, None, None).await.unwrap();
    let j = repo.export_session_json("s").await.unwrap();
    let v: serde_json::Value = serde_json::from_str(&j).unwrap();
    assert_eq!(v["session"]["key"], "s");
    assert_eq!(v["messages"][0]["content"], "ping");
}
```

- [ ] **Step 2-4:** Implement using `get_session` + `get_messages`. JSON exports `{ "session": {...}, "messages": [...] }`. MD exports `# Session <key>\n` then `### <role>\n<content>\n\n` per message, ordered by `created_at`.
- [ ] **Step 5: Commit** — `feat(storage): SessionRepo::export_session_{md,json}`.

### Task D2: `AppCore::coding_sessions_export`

**Files:**
- Modify: `crates/app-core/src/state.rs`

- [ ] **Step 1: Failing test** in `crates/app-core/tests/coding_sessions_export.rs` — round-trip via a temp `KLYNTBOT_HOME`.

- [ ] **Step 2-3: Implement**

```rust
#[tracing::instrument(skip(self), err)]
pub async fn coding_sessions_export(&self, session_key: String, format: ExportFormat)
    -> common::Result<SessionExportResult>
{
    let bytes = match format {
        ExportFormat::Md   => self.repos.sessions.export_session_md(&session_key).await?,
        ExportFormat::Json => self.repos.sessions.export_session_json(&session_key).await?,
    };
    let dir = self.config.data_dir().join("exports");
    tokio::fs::create_dir_all(&dir).await?;
    let ext = match format { ExportFormat::Md => "md", ExportFormat::Json => "json" };
    let path = dir.join(format!("{session_key}.{ext}"));
    tokio::fs::write(&path, &bytes).await?;
    Ok(SessionExportResult { path: path.to_string_lossy().into_owned(), bytes_written: bytes.len() })
}
```

`ExportFormat` + `SessionExportResult` in `desktop-shared`.

- [ ] **Step 4: Run** — PASS.
- [ ] **Step 5: Commit** — `feat(app-core): coding_sessions_export handler`.

### Task D3: Tauri command + slash entry — export

**Files:**
- Modify: `crates/desktop/src/commands/coding_sessions_v2.rs`
- Modify: `crates/desktop/src/lib.rs`
- Modify: `desktop-ui/src/features/coding/slash/registry.ts`

- [ ] **Step 1-3: Wire** — `coding_sessions_export(session_key, format)`; under `/sessions` branch add:

```ts
"export": {
  kind: "leaf", path: "direct",
  tauriCommand: "coding_sessions_export",
  command: "/sessions export",
  description: "Write the thread to a file (md or json)",
  argHint: "[--format md|json]",
  category: "sessions",
},
```

The slash dispatcher must parse `--format md|json` into the `format` arg. Update `dispatchDirect` (in `useSlashCommands.ts`) — extend its arg parsing to handle a `--format` flag specifically for `/sessions export`. Test via Vitest.

- [ ] **Step 4: Drift gate, typecheck.**
- [ ] **Step 5: Commit** — `feat(coding): /sessions export slash command`.

### Task D4: `SessionRepo::fork_session`

**Files:**
- Modify: `crates/storage/src/repos/session.rs`

- [ ] **Step 1: Failing test**

```rust
#[tokio::test]
async fn fork_copies_messages_and_sets_parent() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repo = SessionRepo::new(pool.clone());
    repo.upsert_session("orig", "{}").await.unwrap();
    repo.add_message("orig", "m1", "user", "hi", None, None, None).await.unwrap();
    repo.add_message("orig", "m2", "assistant", "yo", None, None, None).await.unwrap();
    let new_key = repo.fork_session("orig", None /* fork from end */).await.unwrap();
    assert_ne!(new_key, "orig");
    assert_eq!(repo.count_messages(&new_key).await.unwrap(), 2);
    let row = sqlx::query("SELECT parent_session_id FROM sessions WHERE key = ?")
        .bind(&new_key).fetch_one(pool.inner()).await.unwrap();
    let parent: String = row.get("parent_session_id");
    assert_eq!(parent, "orig");
}
```

- [ ] **Step 2-3: Implement**

```rust
#[tracing::instrument(skip(self), err)]
pub async fn fork_session(&self, source_key: &str, up_to_message: Option<&str>) -> Result<String> {
    let new_key = format!("fork-{}", uuid::Uuid::new_v4());
    let metadata = self.get_session(source_key).await?
        .map(|s| s.metadata).unwrap_or_else(|| "{}".into());
    sqlx::query(
        "INSERT INTO sessions (key, metadata, parent_session_id, conversation_type, approval_mode) \
         SELECT ?, ?, key, conversation_type, approval_mode FROM sessions WHERE key = ?"
    ).bind(&new_key).bind(&metadata).bind(source_key)
        .execute(self.pool.inner()).await
        .map_err(common::KlyntbotError::from)?;
    let cutoff_clause = match up_to_message {
        Some(_) => "AND created_at <= (SELECT created_at FROM messages WHERE id = ? AND session_key = ?)",
        None => "",
    };
    let q = format!(
        "INSERT INTO messages (session_key, id, role, content, request_id, tool_calls, metadata, created_at) \
         SELECT ?, id || '-fork', role, content, request_id, tool_calls, metadata, created_at \
         FROM messages WHERE session_key = ? {cutoff_clause} ORDER BY created_at ASC"
    );
    let mut query = sqlx::query(&q).bind(&new_key).bind(source_key);
    if let Some(anchor) = up_to_message {
        query = query.bind(anchor).bind(source_key);
    }
    query.execute(self.pool.inner()).await.map_err(common::KlyntbotError::from)?;
    Ok(new_key)
}
```

- [ ] **Step 4: Run** — PASS.
- [ ] **Step 5: Commit** — `feat(storage): SessionRepo::fork_session`.

### Task D5: `coding_sessions_fork` Tauri command + slash entry

Same wiring pattern as Task D3.

- [ ] **Steps 1-5:** AppCore `coding_sessions_fork(session_key, up_to_message)` → tauri shim → registry entry under `/sessions`. Returns `{ new_session_key }`. Dispatcher then triggers the existing thread-switch UI on result.

```bash
git commit -m "feat(coding): /sessions fork slash command"
```

---

# Workstream E — `/dead-ends`, `/mirror`, agent-routed slash commands

These are spec §9 agent-routed commands — they don't hit Tauri; the dispatcher prepends a system instruction to the user's message and lets the agent loop respond.

### Task E1: Add agent-routed entries

**Files:**
- Modify: `desktop-ui/src/features/coding/slash/registry.ts`
- Modify: `desktop-ui/src/features/coding/slash/transformers.ts` (or wherever `transformAgentRouted` lives — find by grep)

- [ ] **Step 1: Failing Vitest** for the dispatcher

```tsx
import { useSlashCommands } from "./useSlashCommands";
// ...
it("/dead-ends transforms to a system-prefixed message", async () => {
  const r = renderHook(() => useSlashCommands());
  const out = r.result.current.classify("/dead-ends");
  expect(out).toBe("agent");
});
```

- [ ] **Step 2-3:** Add registry entries:

```ts
"/dead-ends": {
  kind: "leaf", path: "agent",
  command: "/dead-ends",
  description: "Surface this repo's known dead-ends from Mirror",
  category: "recall",
  agentTransform: (input) =>
    "[system: invoke check_dead_ends for the current repo and summarize]",
},
"/mirror": {
  kind: "leaf", path: "agent",
  command: "/mirror",
  description: "Show recent Mirror alerts inline",
  category: "recall",
  agentTransform: (input) =>
    "[system: list the agent's recent Mirror alerts]",
},
```

Update `transformAgentRouted` to honor the per-entry `agentTransform` if provided.

- [ ] **Step 4: Test passes.**
- [ ] **Step 5: Commit** — `feat(coding): /dead-ends and /mirror agent-routed slash commands`.

---

# Workstream F — Per-Thread Cost Ceiling + `MirrorAlert`

Spec §13 line 1396 ("Per-thread cost ceiling + `CostThresholdCrossed` Mirror alert") and §10 line 1137 (the variant was *not* added to `AgentEvent`; it flows via Mirror).

### Task F1: Config — `costCeiling.perThreadUsd`

**Files:**
- Modify: `crates/config/src/schema/coding.rs`

- [ ] **Step 1: Failing test** — defaults to `None` (disabled).
- [ ] **Step 2-4:** Add field:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CostCeilingConfig {
    pub per_thread_usd: Option<f64>,
    #[serde(default = "default_cost_alert_pct")]
    pub alert_at_percent: u32,
}
fn default_cost_alert_pct() -> u32 { 80 }
```

Plug into `CodingConfig`.

- [ ] **Step 5: Commit** — `feat(config): coding.costCeiling.perThreadUsd`.

### Task F2: `CostTracker` — per-thread ledger

**Files:**
- Modify: `crates/agent/src/output/cost_tracker.rs:48`

- [ ] **Step 1: Failing tests** — adds `record_for_session("s", usage, ...)`, `total_for_session("s") -> f64`, `check_session_ceiling("s", limit) -> Option<CostAlert>`.
- [ ] **Step 2-4:** Use a `DashMap<String, f64>` keyed by `session_key`. `check_session_ceiling` returns `Some(CostAlert { session_key, spend_usd, ceiling_usd, percent })` once `spend / ceiling >= alert_at_percent / 100.0`. Only emits once per session per crossing (track the last reported percent in a second `DashMap`).
- [ ] **Step 5: Commit** — `feat(agent): CostTracker per-thread ledger + ceiling check`.

### Task F3: Wire ceiling check into the runtime

**Files:**
- Modify: `crates/agent/src/agent_runtime/runtime.rs:811`

- [ ] **Step 1: Failing integration test** — drive a fake usage that crosses 80% of ceiling; assert `MirrorAlert` is published on the bus.

- [ ] **Step 2-3:** After the existing `record(...)` call, also call:

```rust
if let Some(ceiling) = self.config.coding.cost_ceiling.per_thread_usd {
    self.cost_tracker.record_for_session(&session_key, usage, ...);
    if let Some(alert) = self.cost_tracker.check_session_ceiling(&session_key, ceiling) {
        let _ = self.mirror_facade.as_ref().map(|m| m.emit_alert(MirrorAlertKind::CostThresholdCrossed {
            session_key: alert.session_key,
            spend_usd: alert.spend_usd,
            ceiling_usd: alert.ceiling_usd,
            percent: alert.percent,
        }));
    }
}
```

`MirrorAlertKind::CostThresholdCrossed` is added in the next task.

- [ ] **Step 4: Run.**
- [ ] **Step 5: Commit** — `feat(agent): per-thread ceiling check publishes MirrorAlert`.

### Task F4: 8th `MirrorSignalSource` — `CostCeilingSource`

**Files:**
- Create: `crates/cognitive/src/mirror/sources/cost_ceiling.rs`
- Modify: `crates/cognitive/src/mirror/sources/mod.rs`
- Modify: `crates/cognitive/src/mirror/engine.rs:34`
- Modify: `crates/ai-core/src/mirror.rs` — extend `MirrorAlertKind` enum.

- [ ] **Step 1: Failing test** — emits a CostThresholdCrossed alert via the source's `accept_event` (or the established trait method); assert the alert appears in the Mirror sink.
- [ ] **Step 2-3:** Source listens for `AgentEvent::UsageReport` and `AgentEvent::BudgetWarning` on the existing event bus and converts them into `MirrorAlertKind::CostThresholdCrossed`.
- [ ] **Step 4: Run.**
- [ ] **Step 5: Commit** — `feat(cognitive): 8th MirrorSignalSource — cost ceiling alerts`.

### Task F5: Frontend — `CostCeilingBanner`

**Files:**
- Create: `desktop-ui/src/features/coding/components/CostCeilingBanner.tsx`
- Modify: `desktop-ui/src/features/chat/components/ChatHeader.tsx` (or sibling — find the header component for chat threads via grep)

- [ ] **Step 1: Failing Vitest** — when `mirrorAlerts` prop contains a `CostThresholdCrossed` for the current session, banner renders text "Spend $0.85 / $1.00 (85%)".
- [ ] **Step 2-3: Implement.** Subscribes to the existing Mirror alerts feed (`commands.coding_memory_mirror_alerts_feed`). Filter to current `sessionKey` + alert kind. Dismiss = call `commands.coding_memory_mirror_alert_action({ id, action: "dismiss" })`.
- [ ] **Step 4: Test passes; lint, typecheck.**
- [ ] **Step 5: Commit** — `feat(coding): per-thread cost ceiling banner`.

---

# Workstream G — Settings: Hooks Display + Skill Install-from-URL Polish

### Task G1: Promote `install_from_url` to `klynt-skill-loader`

**Files:**
- Create: function in `crates/klynt-skill-loader/src/lib.rs`
- Modify: `crates/app-core/src/coding/skills_handler.rs:134`

- [ ] **Step 1: Failing test in `klynt-skill-loader`** — given a known fixture URL or a mock HTTP server, downloads + extracts to `target_dir`. Use `wiremock` or `httpmock` (whichever the workspace already uses) — grep for it; otherwise add `httpmock` as a dev-dep.

- [ ] **Step 2-3: Implement**

```rust
#[tracing::instrument(skip(target_dir), err)]
pub async fn load_from_url(source: &str, target_dir: &std::path::Path) -> common::Result<()> {
    if !source.starts_with("http://") && !source.starts_with("https://") {
        return Err(common::KlyntbotError::other("expected http(s) URL"));
    }
    // ... lift the existing body of skills_handler::install_from_url verbatim
}
```

`crates/app-core/src/coding/skills_handler.rs:134` becomes:

```rust
klynt_skill_loader::load_from_url(&source, &target_dir).await
```

- [ ] **Step 4: Run** — `cargo nextest run -p klynt-skill-loader` + `-p app-core`.
- [ ] **Step 5: Commit** — `refactor(klynt-skill-loader): promote install_from_url to public API`.

### Task G2: `HooksSubsection.tsx`

**Files:**
- Create: `desktop-ui/src/features/settings/components/sections/coding/HooksSubsection.tsx`
- Modify: `desktop-ui/src/features/settings/components/sections/SettingsCodingSection.tsx`

- [ ] **Step 1: Failing Vitest**

```tsx
import { render, screen, waitFor } from "@testing-library/react";
import { vi } from "vitest";
vi.mock("@/bindings", () => ({
  commands: { codingHooksList: vi.fn().mockResolvedValue({
    hooks: [
      { event: "PreToolUse", matcher: "Bash(*)", command: "scripts/log-bash.sh", timeout_ms: 5000, fail_open: true },
    ],
  })},
}));
import { HooksSubsection } from "./HooksSubsection";

it("renders hook entries from coding_hooks_list", async () => {
  render(<HooksSubsection />);
  await waitFor(() => expect(screen.getByText("PreToolUse")).toBeInTheDocument());
  expect(screen.getByText(/Bash\(\*\)/)).toBeInTheDocument();
  expect(screen.getByText(/log-bash\.sh/)).toBeInTheDocument();
});
```

- [ ] **Step 2: Run** — FAIL.

- [ ] **Step 3: Implement**

```tsx
import { useEffect, useState } from "react";
import { commands } from "@/bindings";

interface HookRow { event: string; matcher?: string; command: string; timeout_ms?: number; fail_open?: boolean; }

export function HooksSubsection() {
  const [hooks, setHooks] = useState<HookRow[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    commands.codingHooksList()
      .then((r: { hooks: HookRow[] }) => setHooks(r.hooks))
      .catch((e: unknown) => setError(String(e)));
  }, []);
  if (error) return <div className="settings-error">{error}</div>;
  if (!hooks) return <div className="settings-empty">Loading hooks…</div>;
  if (hooks.length === 0)
    return <div className="settings-empty">
      No hooks configured. Add some in <code>~/.klyntbot/hooks.toml</code>.
    </div>;
  return (
    <div className="hooks-subsection">
      <table className="hooks-subsection__table">
        <thead><tr>
          <th>Event</th><th>Matcher</th><th>Command</th><th>Timeout</th><th>Fail open</th>
        </tr></thead>
        <tbody>
          {hooks.map((h, i) => (
            <tr key={i}>
              <td>{h.event}</td>
              <td><code>{h.matcher ?? "—"}</code></td>
              <td><code>{h.command}</code></td>
              <td>{h.timeout_ms ?? 30000} ms</td>
              <td>{h.fail_open === false ? "no" : "yes"}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
```

- [ ] **Step 4: Wire into the section tabs**

In `SettingsCodingSection.tsx` extend `tabs`:

```tsx
const tabs = ["General", "Tools", "Permissions", "Sandbox", "Skills", "Sessions", "Hooks"];
// ...
{activeTab === "Hooks" && <HooksSubsection />}
```

- [ ] **Step 5: Add the matching CSS** to `desktop-ui/src/styles/settings.css` (or the sibling sheet that styles other subsections — grep for `hooks-subsection` placeholder; add a new `@import` line in `src/styles/index.css` if you create a new file). Use `var(--fs-base)` / `var(--fs-xs)` per CLAUDE.md typography rule.

- [ ] **Step 6: Test + commit**

```bash
cd desktop-ui && bun run test HooksSubsection && bun run lint && bun run typecheck && cd ..
git add desktop-ui/src/features/settings/ desktop-ui/src/styles/
git commit -m "feat(settings): coding Hooks tab"
```

### Task G3: SkillsSubsection — URL-install feedback

**Files:**
- Modify: `desktop-ui/src/features/settings/components/sections/coding/SkillsSubsection.tsx`

- [ ] **Step 1: Failing Vitest** — submitting a malformed URL renders the validation message; submitting a good URL renders a "Installing…" spinner then a success row.
- [ ] **Step 2-4:** Add `useState<{kind:"idle"|"installing"|"ok"|"err", text?:string}>` next to existing `installSrc`. On submit, validate (`http://` or `https://` or local path), then `await commands.codingSkillsInstall({source})`, surface result.
- [ ] **Step 5: Commit** — `feat(settings): URL-install feedback for /skills install`.

---

# Workstream H — Performance Pass: chat-send → first-token p95 < 800ms

Spec §13 line 1398. Bench target from §14: `bench_chat_send_to_first_token_p95 < 800ms in coding mode (warm cache)`.

### Task H1: Bench harness

**Files:**
- Create: `crates/agent/benches/chat_send_to_first_token.rs`

- [ ] **Step 1: Wire criterion bench**

```rust
use criterion::{Criterion, criterion_group, criterion_main};
// Build a runtime with an in-memory pool, a `_scripted_echo` provider returning a fixed first token after 0ms,
// and an `AgentRuntime` with the curated coding tool registry pre-built.
// Measure: time from `runtime.process(message)` to first AgentEvent::ContentChunk emission.

fn bench_first_token(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    c.bench_function("chat_send_to_first_token_p95", |b| {
        b.to_async(&rt).iter(|| async {
            let runtime = test_helpers::warm_coding_runtime().await;
            let start = std::time::Instant::now();
            let mut stream = runtime.process("Hello").await.unwrap();
            while let Some(ev) = stream.next().await {
                if matches!(ev, agent::events::AgentEvent::ContentChunk { .. }) {
                    return start.elapsed();
                }
            }
            unreachable!()
        })
    });
}

criterion_group!(benches, bench_first_token);
criterion_main!(benches);
```

Add `[[bench]]` entry to `crates/agent/Cargo.toml`.

- [ ] **Step 2: Run baseline**

```bash
cargo bench -p agent --bench chat_send_to_first_token > /tmp/perf_baseline.txt
```

Record the p95 in the commit message.

- [ ] **Step 3: Commit** — `test(agent): bench chat-send-to-first-token`.

### Task H2: Identify hot path

- [ ] **Step 1**: Run `cargo flamegraph -p agent --bench chat_send_to_first_token` (or use `samply` / `pprof-rs`). Record top-3 hottest functions in commit message.
- [ ] **Step 2**: Likely targets — `SkillRouter::select` (full skill walk on every turn?), `ContextEngine::build_system_prompt` (re-reading KLYNTBOT.md from disk every turn?), `ToolRegistry` rebuild. Spec §14 line 1521 says skill discovery should be `< 30ms` for 50 skills; verify. The KLYNTBOT.md soul read is "live-read" per CLAUDE.md — it should be cached with mtime invalidation if it's not already.
- [ ] **Step 3**: Document findings in a small `docs/superpowers/notes/2026-05-XX-coding-perf-pass.md` file (allowed for engineering notes — *not* a CLAUDE.md change).

### Task H3: Apply targeted optimizations

For each optimization, run the bench before + after; commit per-optimization with the delta in the message.

- [ ] **Optimization A**: Memoize `SoulContextSource` reads with mtime check (if not already). TDD: test that `SoulContextSource::build` is called once when mtime is unchanged across 100 invocations.
- [ ] **Optimization B**: Cache the curated tool-registry `Vec<Arc<dyn Tool>>` per-thread; rebuild only on `/power` toggle. TDD: assert `Tool::name` count is stable across 100 turns and the rebuild counter equals 1.
- [ ] **Optimization C**: If skill discovery dominates, add an LRU cache keyed by `(repo_id, file_paths_hash)` to `SkillActivator`.

Stop applying optimizations once the bench reports p95 < 800ms.

- [ ] **Final step**: Commit with the achieved p95 in the commit message:

```bash
git commit -m "perf(coding): chat-send → first-token p95 == NNNms (target 800ms, was MMMms)"
```

---

# Workstream I — Quality Gates + Spec Update

### Task I1: Run all gates

- [ ] **Step 1**:

```bash
cargo build --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
cargo nextest run --workspace
cargo test --workspace --doc
cd desktop-ui && bun run lint && bun run typecheck && bun run test && cd ..
./scripts/run_kca_validation.sh
```

Each must be green. Fix anything yellow before continuing.

- [ ] **Step 2**: Confirm bench:

```bash
cargo bench -p agent --bench chat_send_to_first_token
```

Reports p95 < 800ms. If not, return to Workstream H.

### Task I2: Spec update

- [ ] **Step 1**: Edit `docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md` §13 Phase 2 — replace each deliverable line's "deferred"/"Phase 2" wording with "shipped 2026-05-XX (commit `<sha>`)".
- [ ] **Step 2**: Add a row to the §13 amendment log: `2026-05-XX | Phase 2 shipped: Layer 3, snapshots/rewind, tool_search, /sessions {export, fork}, /dead-ends, /mirror, /permissions clear-mirror, cost ceiling, Settings hooks tab, perf pass.`
- [ ] **Step 3**: Commit — `docs(specs): mark coding-in-chat Phase 2 deliverables shipped`.

---

# Self-Review Checklist (for the executing engineer to read once before starting)

1. **Spec coverage**:
   - Layer 3 (`mirrorLearning: true`) → Workstream A ✓
   - File snapshots → Workstream B Tasks B1-B5 ✓
   - `/sessions rewind` → B6-B9 ✓
   - `tool_search` real → C ✓
   - `/sessions export` → D1-D3 ✓
   - `/sessions fork` → D4-D5 ✓
   - `/dead-ends`, `/mirror`, `/permissions clear-mirror` → A8 + E ✓
   - Per-thread cost ceiling + Mirror alert → F ✓
   - Settings hooks display + skill install URL → G ✓
   - Perf pass < 800ms p95 → H ✓
   - K10, K11 invariants → A7, B9 ✓

2. **Type consistency**: `ApprovalLayer::Layer3Mirror` exists (Phase 1, line 6 of `decision.rs`). `MirrorAlertKind::CostThresholdCrossed` is added in F4 — make sure the variant name used in F3's emission matches F4's enum addition exactly.

3. **No placeholders**: Every step has either a code block, an exact test name, or an explicit shell command. The only deferrals are inside Workstream H (Optimizations A/B/C) where the actual optimization depends on what the flamegraph reveals — that's fine, it's an investigation, not a placeholder.

---

`★ Insight ─────────────────────────────────────`
- The spec's "K11 starred sessions never pruned" invariant is conceptually strange to be Phase 2 — it's a one-line `WHERE pinned = 0` clause. We pay the proptest cost (Task B9) because Phase 1 deferred K11 specifically; the proptest is the gate that *proves* pruning behaves correctly even after Phase 2 retention work touches the cron path.
- Workstream F demonstrates a deliberate spec choice: `CostThresholdCrossed` was *removed* from the runtime `AgentEvent` enum and *re-routed* through Mirror alerts. This means cost ceiling is observable to the user via the existing Mirror alert UI surface (already implemented in coding-memory's `MirrorAlertsFeed`) — no new event channel, no new React store.
- The Tauri command pattern is rigid: every command in `commands/` must be `#[klynt_command]` (or `#[klynt_raw_command]`) per CLAUDE.md "Adding a Tauri command (Plan 6)". Skipping the macro fails the `no_raw_tauri_command_outside_macros` test on CI. Keep this in mind for Tasks A8, B7, D3, D5.
`─────────────────────────────────────────────────`

---

*End of Phase 2 plan.*
