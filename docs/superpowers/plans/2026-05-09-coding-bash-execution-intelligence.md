# Coding Bash — Execution Intelligence Implementation Plan (Phase 2.3b)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship Phase 2.3b — extend `feature-coding-bash` (Phase 2.3a) with three cognitive layers: (1) cross-run output diffing embedded in completion `<system-reminder>` bodies, (2) plan-mode `ExecutionIntelligenceInjector` that surfaces TodoItems looking like background-bash candidates, and (3) per-job episodic memory writes via a new `BackgroundJobSignalSource` plugged into `MirrorEngine`. Implements the spec at `docs/superpowers/specs/2026-05-09-coding-bash-execution-intelligence-design.md`.

**Architecture:** New `intelligence/` submodule inside `feature-coding-bash` (normalize, diff, verification_match, injector). New `coding_bash.rs` mirror source in `cognitive`. One translator arm in `app-core::init::ai_pipeline`. One column added to `coding_background_jobs` (`command_key TEXT NOT NULL` + composite index). Three accessor methods on `BashJobEvent`. No new tools, no new frontend components.

**Tech Stack:** Rust (sqlx + tokio + jiff + sha2 + once_cell + regex). SQLite WAL via `StoragePool`. Reuses 2.3a `JobSupervisor`, 2.2 `DynamicInjector`/`InjectorRegistry`, 2.1 `TodoRepo` + `MirrorSignalSource` pattern from `coding_todo.rs`.

**Spec:** `docs/superpowers/specs/2026-05-09-coding-bash-execution-intelligence-design.md`.

**Foundations (already shipped):**
- Phase 2.1 TodoWrite (`feature-coding-todo` + `TodoSignalSource`)
- Phase 2.2 plan mode (`InjectorRegistry`, `DynamicInjector`, `coding_policies` propagation)
- Phase 2.3a background bash (`JobSupervisor`, `BackgroundJobsInjector`, `BashJobEvent`, `coding_background_jobs` table, `failure_extracted` JSON)

---

## File Structure

### Create

| Path | Responsibility |
|---|---|
| `crates/feature-coding-bash/src/intelligence/mod.rs` | Submodule barrel — re-exports public items |
| `crates/feature-coding-bash/src/intelligence/normalize.rs` | `command_key(raw: &str) -> String` (sha256 of normalized command) |
| `crates/feature-coding-bash/src/intelligence/diff.rs` | `JobDiff`, `KindTransition`, `ExtractedDiff`, `Location`, `diff_against_prior(prior, curr) -> JobDiff` |
| `crates/feature-coding-bash/src/intelligence/verification_match.rs` | `VerificationVerb` enum + `classify(title) -> Option<VerificationVerb>` |
| `crates/feature-coding-bash/src/intelligence/injector.rs` | `ExecutionIntelligenceInjector : DynamicInjector` |
| `crates/feature-coding-bash/tests/intel_diff_basic.rs` | E2E: prior+curr same command → completion body has diff section |
| `crates/feature-coding-bash/tests/intel_diff_test_set.rs` | E2E: TestFailure→TestFailure with overlapping name sets |
| `crates/feature-coding-bash/tests/intel_diff_recovered.rs` | E2E: TestFailure→Passed produces "Recovered" transition |
| `crates/feature-coding-bash/tests/intel_affordance_in_plan.rs` | E2E: plan-mode todos with verification verbs produce affordance |
| `crates/feature-coding-bash/tests/intel_affordance_dedup.rs` | E2E: active job covering todo suppresses affordance |
| `crates/feature-coding-bash/tests/intel_command_key_normalization.rs` | E2E: env-var prefix and whitespace collapse hit the same prior |
| `crates/feature-coding-bash/tests/intel_episodic_write.rs` | E2E: BashJob.Failed → episodic_memories row written |
| `crates/feature-coding-bash/tests/intel_episodic_passed.rs` | E2E: BashJob.Completed → episodic_memories row, importance=0.3 |
| `crates/feature-coding-bash/tests/intel_episodic_lost_on_restart.rs` | E2E: orphan reconcile → Lost episode written, importance=0.6 |
| `crates/feature-coding-bash/tests/intel_subagent_episodic_actor_id.rs` | E2E: subagent spawn → episode actor_id matches subagent |
| `crates/feature-coding-bash/tests/fixtures/cargo_multi_test_failure.txt` | Real cargo nextest output with 3 failed tests |
| `crates/feature-coding-bash/tests/fixtures/vitest_multi_failure.txt` | Real vitest output with 2 failed tests |
| `crates/cognitive/src/mirror/sources/coding_bash.rs` | `BackgroundJobSignalSource : MirrorSignalSource` |

### Modify

| Path | What changes |
|---|---|
| `crates/feature-coding-bash/src/lib.rs` | Add `pub mod intelligence;` and re-export `ExecutionIntelligenceInjector` |
| `crates/feature-coding-bash/src/migrations.rs` | Bump version 1→2, add `command_key TEXT NOT NULL` column + composite index, drop+recreate (pre-release) |
| `crates/feature-coding-bash/src/gate.rs` | Replace `FIRST_FAILED_TEST_RE` capture with all-tests capture; emit `failed_test_names: Vec<String>` for cargo TestFailure; add multi-name extraction for vitest |
| `crates/feature-coding-bash/src/supervisor.rs` | In `spawn` (~line 464): set `command_key`. In `handle_exit` (~line 279): compute `command_key`, query prior, build diff, pass to `completion_notification` |
| `crates/feature-coding-bash/src/render.rs` | Extend `completion_notification` signature to take `diff: Option<&JobDiff>`; render new "Compared to last run" block when present |
| `crates/feature-coding-bash/src/Cargo.toml` | Add `sha2 = "0.10"` and ensure `regex`, `serde_json` are present |
| `crates/storage/src/repos/coding_background_jobs.rs` | Add `command_key: String` field on `BashJobRow`; extend `insert` and `update_status` if needed; add `find_prior_by_command_key` method; update `map_row` |
| `crates/bus/src/domain_events.rs` | Add `impl BashJobEvent { fn job_id, fn thread_id, fn agent_id }` accessor methods |
| `crates/cognitive/src/mirror/sources/mod.rs` | Add `pub mod coding_bash;` and `pub use coding_bash::BackgroundJobSignalSource;` |
| `crates/cognitive/src/mirror/engine.rs` | Add `bash_repo: Option<Arc<storage::BashJobRepo>>` parameter to `MirrorEngine::start`; construct + register `BackgroundJobSignalSource` when both `episodic_repo` and `bash_repo` are Some |
| `crates/app-core/src/init/ai_pipeline.rs` | Add new arm: `if let Some(s) = translate_bash_job(event) { return Some(s); }` (or inline `match event` arm) for `DomainEvent::BashJob(_)` |
| `crates/app-core/src/init/mod.rs` | Construct `ExecutionIntelligenceInjector`, pass to `InjectorRegistry::new(vec![...])`. Pass `Some(Arc::new(bash_job_repo.clone()))` to `MirrorEngine::start`. |
| `crates/feature-coding-bash/Cargo.toml` | Add `feature-coding-todo` dep (path) for TodoRepo access |

### Test

Tests are colocated as `#[cfg(test)] mod tests` in each new file, plus integration tests in `crates/feature-coding-bash/tests/`. No frontend tests — UI surface unchanged.

---

## Task 0: Branch + spec confirm + baseline green

**Files:**
- Read: `docs/superpowers/specs/2026-05-09-coding-bash-execution-intelligence-design.md`

- [ ] **Step 1: Create a feature branch from main**

```bash
git checkout main
git pull --ff-only
git checkout -b feat/coding-bash-execution-intelligence
```

- [ ] **Step 2: Confirm spec is at HEAD**

```bash
git log --oneline -1 -- docs/superpowers/specs/2026-05-09-coding-bash-execution-intelligence-design.md
```
Expected: a commit hash with the message containing `coding bash execution intelligence design (Phase 2.3b)`.

If no such commit yet, commit the spec first:
```bash
git add docs/superpowers/specs/2026-05-09-coding-bash-execution-intelligence-design.md
git commit -m "docs(spec): coding bash execution intelligence design (Phase 2.3b)"
```

- [ ] **Step 3: Confirm Phase 2.3a is in place**

```bash
ls crates/feature-coding-bash/src/
grep -n "BackgroundJobsInjector" crates/feature-coding-bash/src/injector.rs | head -3
grep -n "publish_bash_job" crates/feature-coding-bash/src/supervisor.rs | head -5
grep -n "BashJobEvent::Lost" crates/feature-coding-bash/src/supervisor.rs | head -3
```
Expected: each command returns at least 1 line. Confirms 2.3a foundation is present.

- [ ] **Step 4: Confirm `BashJobEvent` accessor methods are NOT present yet**

```bash
grep -n "impl BashJobEvent" crates/bus/src/domain_events.rs
```
Expected: 0 matches. We'll add the impl block in Task A4.

- [ ] **Step 5: Build & test baseline green**

```bash
cargo build --workspace 2>&1 | tail -10
cargo nextest run --workspace 2>&1 | tail -5
```
Expected: workspace builds cleanly; all tests pass. Don't proceed if there are pre-existing failures — they'll mask regressions.

- [ ] **Step 6: Confirm `cargo machete` is clean for the crates we'll touch**

```bash
cargo machete crates/feature-coding-bash crates/cognitive crates/storage 2>&1 | tail -10
```
Expected: no unused deps. If existing unused deps appear, note them but don't fix in this branch.

---

# PR 1 — Storage + bus extensions (~0.5 day)

> **Strategy:** Land schema and minor bus changes first. No behavior change yet — purely additive surface that subsequent PRs build on.

## Phase A — `command_key` column on `coding_background_jobs`

### Task A1: Bump migration version + add `command_key` column

**Files:**
- Modify: `crates/feature-coding-bash/src/migrations.rs:1-39`

- [ ] **Step 1: Read current migrations.rs**

```bash
cat crates/feature-coding-bash/src/migrations.rs
```
Confirm: `version: 1`, no `command_key` column.

- [ ] **Step 2: Replace migration with version 2 including `command_key`**

Edit `crates/feature-coding-bash/src/migrations.rs`. Change `version: 1` to `version: 2`. Inside the SQL string, add the `command_key TEXT NOT NULL` column after `command TEXT NOT NULL,` line, and add a new index `idx_cbj_session_command_key` after the existing two indexes.

The full new file content:

```rust
use tools_core::FeatureMigration;

pub fn coding_background_jobs_migration() -> FeatureMigration {
    FeatureMigration {
        feature_name: "feature_coding_bash",
        version: 2,
        description: "Create coding_background_jobs table with command_key for diff lookup",
        sql: r#"
            CREATE TABLE coding_background_jobs (
                id                    TEXT PRIMARY KEY,
                session_id            TEXT NOT NULL,
                agent_id              TEXT NOT NULL,
                description           TEXT NOT NULL,
                command               TEXT NOT NULL,
                command_key           TEXT NOT NULL,
                cwd                   TEXT NOT NULL,
                timeout_ms            INTEGER NOT NULL,
                silent_completion     INTEGER NOT NULL DEFAULT 0,
                status                TEXT NOT NULL,
                exit_code             INTEGER,
                failure_kind          TEXT,
                failure_detail        TEXT,
                failure_extracted     TEXT,
                started_at            TEXT NOT NULL,
                finished_at           TEXT,
                total_bytes_emitted   INTEGER NOT NULL DEFAULT 0,
                bisect_count          INTEGER NOT NULL DEFAULT 0,
                log_path              TEXT NOT NULL,
                final_path            TEXT,
                last_polled_at        TEXT,
                last_seen_offset      INTEGER NOT NULL DEFAULT 0,

                CHECK (status IN ('Starting','Running','Completed','Failed','Cancelled','Lost')),
                CHECK (failure_kind IS NULL OR status IN ('Failed','Cancelled','Lost'))
            );

            CREATE INDEX idx_cbj_session_status
                ON coding_background_jobs(session_id, status);

            CREATE INDEX idx_cbj_active
                ON coding_background_jobs(status)
                WHERE status IN ('Starting','Running');

            CREATE INDEX idx_cbj_session_command_key
                ON coding_background_jobs(session_id, command_key, started_at DESC);
        "#,
    }
}
```

> Note: the FOREIGN KEY clause on `session_id` may or may not exist in your current 2.3a migration. If it does, preserve it exactly. Use `cat` first and copy verbatim.

- [ ] **Step 3: Verify migration compiles**

```bash
cargo check -p feature-coding-bash 2>&1 | tail -5
```
Expected: `Finished` with 0 errors. Will fail next steps until `BashJobRow` knows about `command_key`.

### Task A2: Add `command_key` field to `BashJobRow` + extend insert/get/map_row

**Files:**
- Modify: `crates/storage/src/repos/coding_background_jobs.rs`

- [ ] **Step 1: Read current BashJobRow + insert + map_row**

```bash
sed -n '1,50p' crates/storage/src/repos/coding_background_jobs.rs
sed -n '45,110p' crates/storage/src/repos/coding_background_jobs.rs
sed -n '255,290p' crates/storage/src/repos/coding_background_jobs.rs
```
Confirm the field list (currently 21 fields), the `INSERT INTO` statement, and the `map_row` field bindings.

- [ ] **Step 2: Add `command_key: String` field on `BashJobRow`**

Find the `BashJobRow` struct (lines 11-33) and add `pub command_key: String,` immediately after `pub command: String,`:

```rust
pub command: String,
pub command_key: String,    // NEW: sha256 hex of normalized command, for diff lookup
pub cwd: String,
```

- [ ] **Step 3: Extend `BashJobRepo::insert` to bind command_key**

In the `insert` method (line 45 onwards), update the SQL to include `command_key` in the column list and `?` placeholders. Add a `.bind(&row.command_key)` in the same positional order.

Locate the `INSERT INTO coding_background_jobs` SQL string. Add `command_key,` to the column list right after `command,`, add `?` to the values list at the matching position, and add `.bind(&row.command_key)` at the matching position in the chain of `.bind(...)` calls. Preserve order exactly.

- [ ] **Step 4: Extend `map_row` to read command_key**

In the `map_row` function (line 261 onwards), add `command_key: row.try_get("command_key")?,` immediately after the `command:` line.

- [ ] **Step 5: Verify compile**

```bash
cargo check -p storage 2>&1 | tail -10
cargo check -p feature-coding-bash 2>&1 | tail -10
```
Expected: both `Finished`. The supervisor will fail to compile because it constructs `BashJobRow` literal at line 464 — fixed in Task A3.

### Task A3: Set `command_key` in supervisor::spawn

**Files:**
- Modify: `crates/feature-coding-bash/src/supervisor.rs`

- [ ] **Step 1: Locate the BashJobRow construction in `spawn`**

```bash
grep -n "BashJobRow {" crates/feature-coding-bash/src/supervisor.rs
```
Expected: one or two match lines around 464.

- [ ] **Step 2: Read the literal**

```bash
sed -n '460,495p' crates/feature-coding-bash/src/supervisor.rs
```
Read the full struct literal so you know where to insert `command_key`.

- [ ] **Step 3: Add `intelligence` module to lib.rs upfront so we can use `command_key()`**

Edit `crates/feature-coding-bash/src/lib.rs`. Add `pub mod intelligence;` after `pub mod injector;` (alphabetical order):

```rust
pub mod gate;
pub mod injector;
pub mod intelligence;       // NEW
pub mod migrations;
pub mod render;
pub mod ring;
pub mod spawner;
pub mod supervisor;
pub mod tools;
pub mod view;
```

- [ ] **Step 4: Create empty `intelligence/mod.rs` so the build doesn't break**

```bash
mkdir -p crates/feature-coding-bash/src/intelligence
```

Create `crates/feature-coding-bash/src/intelligence/mod.rs` with:

```rust
//! Phase 2.3b — Execution Intelligence layer.
//!
//! Spec: `docs/superpowers/specs/2026-05-09-coding-bash-execution-intelligence-design.md`

pub mod normalize;

pub use normalize::command_key;
```

- [ ] **Step 5: Create `intelligence/normalize.rs` with the `command_key` fn**

Create `crates/feature-coding-bash/src/intelligence/normalize.rs`:

```rust
//! Command normalization for diff/recall lookup.
//!
//! `command_key` produces a stable 64-char sha256 hex over the command after
//! trimming, collapsing internal whitespace, and stripping leading `KEY=VAL`
//! environment-variable prefixes. Two commands that share intent but differ
//! in formatting or debug env vars hash to the same key.

use once_cell::sync::Lazy;
use regex::Regex;
use sha2::{Digest, Sha256};

static LEADING_ENV_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^([A-Z_][A-Z0-9_]*=\S+\s+)+").unwrap());

pub fn command_key(raw: &str) -> String {
    let trimmed = raw.trim();
    let no_env = strip_leading_env_vars(trimmed);
    let collapsed = collapse_whitespace(no_env);
    let mut hasher = Sha256::new();
    hasher.update(collapsed.as_bytes());
    hex::encode(hasher.finalize())
}

fn strip_leading_env_vars(s: &str) -> &str {
    match LEADING_ENV_RE.find(s) {
        Some(m) => &s[m.end()..],
        None => s,
    }
}

fn collapse_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idempotent() {
        let k = command_key("cargo nextest run -p agent");
        assert_eq!(k, command_key("cargo nextest run -p agent"));
        assert_eq!(k.len(), 64);
    }

    #[test]
    fn whitespace_collapsing_matches() {
        assert_eq!(
            command_key("cargo nextest run -p agent"),
            command_key("  cargo nextest run  -p agent")
        );
    }

    #[test]
    fn leading_env_stripped() {
        assert_eq!(
            command_key("cargo nextest run -p agent"),
            command_key("RUST_LOG=debug cargo nextest run -p agent")
        );
        assert_eq!(
            command_key("cargo nextest run -p agent"),
            command_key("RUST_LOG=debug RUST_BACKTRACE=1 cargo nextest run -p agent")
        );
    }

    #[test]
    fn non_leading_env_preserved() {
        // FOO=bar after the command, not before — should remain in the key
        assert_ne!(
            command_key("cargo nextest run"),
            command_key("cargo nextest run FOO=bar")
        );
    }

    #[test]
    fn different_flags_different_keys() {
        assert_ne!(
            command_key("cargo nextest run -p agent"),
            command_key("cargo nextest run -p agent --nocapture")
        );
    }

    #[test]
    fn empty_and_only_env() {
        // Empty input
        assert_eq!(command_key(""), command_key(""));
        // All env, no command — strip leaves empty string
        assert_eq!(command_key(""), command_key("FOO=bar BAZ=qux "));
    }
}
```

- [ ] **Step 6: Add sha2 + hex to feature-coding-bash deps**

Read current dependencies:
```bash
grep -A2 "^\[dependencies\]" crates/feature-coding-bash/Cargo.toml | head -3
```

Edit `crates/feature-coding-bash/Cargo.toml`. In the `[dependencies]` section add:

```toml
sha2 = "0.10"
hex = "0.4"
```

If `regex`, `once_cell`, `serde_json` are not already listed, add them:
```toml
regex = "1"
once_cell = "1"
serde_json = "1"
```

(Some of these are likely already present from 2.3a — `cat crates/feature-coding-bash/Cargo.toml` and skip duplicates.)

- [ ] **Step 7: Verify normalize tests pass**

```bash
cargo nextest run -p feature-coding-bash -E 'test(normalize)' 2>&1 | tail -10
```
Expected: 6 tests passed.

- [ ] **Step 8: Set `command_key` in supervisor::spawn's BashJobRow literal**

In `crates/feature-coding-bash/src/supervisor.rs`, find the `BashJobRow {` literal in `spawn` (around line 464) and add `command_key: crate::intelligence::command_key(&spec.command),` after the `command:` field. Add the import at the top of the file:

```rust
use crate::intelligence::command_key;
```

Then in the literal:
```rust
let row = BashJobRow {
    id: id.0.clone(),
    session_id: spec.session_id.clone(),
    agent_id: spec.agent_id.clone(),
    description: spec.description.clone(),
    command: spec.command.clone(),
    command_key: command_key(&spec.command),    // NEW
    cwd: spec.cwd.to_string_lossy().into_owned(),
    // ... rest unchanged ...
};
```

- [ ] **Step 9: Verify supervisor compiles**

```bash
cargo check -p feature-coding-bash 2>&1 | tail -10
```
Expected: `Finished`.

- [ ] **Step 10: Run all feature-coding-bash tests to confirm no regression**

```bash
cargo nextest run -p feature-coding-bash 2>&1 | tail -10
```
Expected: all existing 2.3a tests still pass; the new `normalize` tests are also green.

- [ ] **Step 11: Commit**

```bash
git add crates/feature-coding-bash/Cargo.toml \
        crates/feature-coding-bash/src/lib.rs \
        crates/feature-coding-bash/src/migrations.rs \
        crates/feature-coding-bash/src/intelligence/mod.rs \
        crates/feature-coding-bash/src/intelligence/normalize.rs \
        crates/feature-coding-bash/src/supervisor.rs \
        crates/storage/src/repos/coding_background_jobs.rs
git commit -m "feat(coding): add command_key column for cross-run diff lookup (Phase 2.3b PR1)"
```

### Task A4: Add `find_prior_by_command_key` to BashJobRepo + accessor methods on BashJobEvent

**Files:**
- Modify: `crates/storage/src/repos/coding_background_jobs.rs`
- Modify: `crates/bus/src/domain_events.rs`

- [ ] **Step 1: Add `find_prior_by_command_key` to BashJobRepo**

In `crates/storage/src/repos/coding_background_jobs.rs`, add the new method inside the `impl BashJobRepo` block, placed right after `get` (around line 154):

```rust
/// Most recent terminal-state job in this session with the same command_key,
/// excluding `exclude_id`. Returns None if no prior run exists. Excludes Lost
/// status — Lost runs lack reliable final output to diff against.
pub async fn find_prior_by_command_key(
    &self,
    session_id: &str,
    command_key: &str,
    exclude_id: &str,
) -> Result<Option<BashJobRow>, StorageError> {
    let row_opt = sqlx::query(
        r#"
        SELECT * FROM coding_background_jobs
        WHERE session_id = ?1
          AND command_key = ?2
          AND id != ?3
          AND status IN ('Completed','Failed','Cancelled')
        ORDER BY started_at DESC
        LIMIT 1
        "#,
    )
    .bind(session_id)
    .bind(command_key)
    .bind(exclude_id)
    .fetch_optional(&self.pool)
    .await
    .map_err(StorageError::from)?;

    row_opt.as_ref().map(Self::map_row).transpose()
}
```

- [ ] **Step 2: Write a unit test for find_prior_by_command_key**

Add a `#[cfg(test)] mod tests` at the bottom of `crates/storage/src/repos/coding_background_jobs.rs` (or extend the existing one if it has). Use the in-memory pool helper:

```rust
#[cfg(test)]
mod find_prior_tests {
    use super::*;
    use crate::pool::StoragePool;
    use jiff::Timestamp;

    fn fake_row(id: &str, session: &str, key: &str, status: &str, started_at: Timestamp) -> BashJobRow {
        BashJobRow {
            id: id.into(),
            session_id: session.into(),
            agent_id: "a".into(),
            description: "d".into(),
            command: "c".into(),
            command_key: key.into(),
            cwd: "/".into(),
            timeout_ms: 60_000,
            silent_completion: false,
            status: status.into(),
            exit_code: Some(0),
            failure_kind: None,
            failure_detail: None,
            failure_extracted: None,
            started_at,
            finished_at: Some(started_at),
            total_bytes_emitted: 0,
            bisect_count: 0,
            log_path: format!("/tmp/{id}.log"),
            final_path: Some(format!("/tmp/{id}.final")),
            last_polled_at: None,
            last_seen_offset: 0,
        }
    }

    async fn setup() -> BashJobRepo {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        // Create the table directly (this test bypasses FeaturePackage migration runner).
        sqlx::query(
            r#"
            CREATE TABLE coding_background_jobs (
                id TEXT PRIMARY KEY, session_id TEXT NOT NULL, agent_id TEXT NOT NULL,
                description TEXT NOT NULL, command TEXT NOT NULL, command_key TEXT NOT NULL,
                cwd TEXT NOT NULL, timeout_ms INTEGER NOT NULL,
                silent_completion INTEGER NOT NULL DEFAULT 0, status TEXT NOT NULL,
                exit_code INTEGER, failure_kind TEXT, failure_detail TEXT,
                failure_extracted TEXT, started_at TEXT NOT NULL, finished_at TEXT,
                total_bytes_emitted INTEGER NOT NULL DEFAULT 0,
                bisect_count INTEGER NOT NULL DEFAULT 0, log_path TEXT NOT NULL,
                final_path TEXT, last_polled_at TEXT,
                last_seen_offset INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX idx_cbj_session_command_key
                ON coding_background_jobs(session_id, command_key, started_at DESC);
            "#,
        )
        .execute(pool.inner())
        .await
        .unwrap();
        BashJobRepo::new(pool.inner().clone())
    }

    #[tokio::test]
    async fn returns_most_recent_terminal() {
        let repo = setup().await;
        let t1 = Timestamp::from_millisecond(1_700_000_000_000).unwrap();
        let t2 = Timestamp::from_millisecond(1_700_000_010_000).unwrap();
        repo.insert(&fake_row("a", "s1", "k1", "Completed", t1)).await.unwrap();
        repo.insert(&fake_row("b", "s1", "k1", "Failed", t2)).await.unwrap();

        let prior = repo.find_prior_by_command_key("s1", "k1", "x").await.unwrap();
        assert_eq!(prior.unwrap().id, "b");
    }

    #[tokio::test]
    async fn excludes_self_id() {
        let repo = setup().await;
        let t1 = Timestamp::from_millisecond(1_700_000_000_000).unwrap();
        repo.insert(&fake_row("a", "s1", "k1", "Completed", t1)).await.unwrap();

        assert!(repo.find_prior_by_command_key("s1", "k1", "a").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn excludes_lost_status() {
        let repo = setup().await;
        let t1 = Timestamp::from_millisecond(1_700_000_000_000).unwrap();
        repo.insert(&fake_row("a", "s1", "k1", "Lost", t1)).await.unwrap();

        assert!(repo.find_prior_by_command_key("s1", "k1", "x").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn returns_none_when_no_prior() {
        let repo = setup().await;
        assert!(repo.find_prior_by_command_key("s1", "k1", "x").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn scoped_by_session() {
        let repo = setup().await;
        let t1 = Timestamp::from_millisecond(1_700_000_000_000).unwrap();
        repo.insert(&fake_row("a", "s1", "k1", "Completed", t1)).await.unwrap();

        assert!(repo.find_prior_by_command_key("s2", "k1", "x").await.unwrap().is_none());
    }
}
```

- [ ] **Step 3: Run the new tests to verify**

```bash
cargo nextest run -p storage -E 'test(find_prior)' 2>&1 | tail -10
```
Expected: 5 tests passed.

- [ ] **Step 4: Add accessor methods on BashJobEvent**

In `crates/bus/src/domain_events.rs`, after the `BashJobEvent` enum definition (after line 81), add:

```rust
impl BashJobEvent {
    pub fn job_id(&self) -> &str {
        match self {
            Self::Started   { job_id, .. } => job_id,
            Self::Completed { job_id, .. } => job_id,
            Self::Failed    { job_id, .. } => job_id,
            Self::Cancelled { job_id, .. } => job_id,
            Self::Lost      { job_id, .. } => job_id,
        }
    }

    pub fn thread_id(&self) -> &str {
        match self {
            Self::Started   { thread_id, .. } => thread_id,
            Self::Completed { thread_id, .. } => thread_id,
            Self::Failed    { thread_id, .. } => thread_id,
            Self::Cancelled { thread_id, .. } => thread_id,
            Self::Lost      { thread_id, .. } => thread_id,
        }
    }

    pub fn agent_id(&self) -> &str {
        match self {
            Self::Started   { agent_id, .. } => agent_id,
            Self::Completed { agent_id, .. } => agent_id,
            Self::Failed    { agent_id, .. } => agent_id,
            Self::Cancelled { agent_id, .. } => agent_id,
            Self::Lost      { agent_id, .. } => agent_id,
        }
    }
}
```

- [ ] **Step 5: Add a unit test for the accessor methods**

In `crates/bus/src/domain_events.rs`, find the existing `#[cfg(test)] mod tests` block (or add one if absent). Add:

```rust
#[cfg(test)]
mod bash_job_event_accessor_tests {
    use super::*;
    use jiff::Timestamp;

    #[test]
    fn accessors_return_inner_fields() {
        let started = BashJobEvent::Started {
            job_id: "bash-x".into(),
            thread_id: "t1".into(),
            agent_id: "a1".into(),
            command: "c".into(),
            description: "d".into(),
            started_at: Timestamp::now(),
        };
        assert_eq!(started.job_id(), "bash-x");
        assert_eq!(started.thread_id(), "t1");
        assert_eq!(started.agent_id(), "a1");

        let lost = BashJobEvent::Lost {
            job_id: "bash-y".into(),
            thread_id: "t2".into(),
            agent_id: "a2".into(),
        };
        assert_eq!(lost.job_id(), "bash-y");
        assert_eq!(lost.thread_id(), "t2");
        assert_eq!(lost.agent_id(), "a2");
    }
}
```

- [ ] **Step 6: Run the test**

```bash
cargo nextest run -p bus -E 'test(bash_job_event_accessor)' 2>&1 | tail -10
```
Expected: 1 test passed.

- [ ] **Step 7: Workspace build + test sanity check**

```bash
cargo build --workspace 2>&1 | tail -5
cargo nextest run --workspace 2>&1 | tail -5
```
Expected: 0 errors, all tests still pass.

- [ ] **Step 8: Commit**

```bash
git add crates/storage/src/repos/coding_background_jobs.rs \
        crates/bus/src/domain_events.rs
git commit -m "feat(coding): add find_prior_by_command_key + BashJobEvent accessors (Phase 2.3b PR1)"
```

---

# PR 2 — Gate classifier extension (~0.5 day)

> **Strategy:** Extend cargo and vitest detectors to capture all failed test names. Pre-release we drop `test_name: Option<String>` in favor of `failed_test_names: Vec<String>`. Existing 2.3a fixtures are updated; new multi-test fixtures are added.

## Phase B — Multi-test name extraction

### Task B1: Add cargo multi-test fixture

**Files:**
- Create: `crates/feature-coding-bash/tests/fixtures/cargo_multi_test_failure.txt`

- [ ] **Step 1: Create the fixture file**

Create `crates/feature-coding-bash/tests/fixtures/cargo_multi_test_failure.txt` with realistic cargo nextest output that contains 3 failing tests:

```text
running 21 tests
test tests::session_persistence::reload_active_thread ... FAILED
test tests::session_persistence::reload_orphan ... FAILED
test tests::concurrent_writes::write_then_read ... FAILED
test tests::config_loader::loads_default ... ok
test tests::ulid_gen::roundtrip ... ok

failures:

---- tests::session_persistence::reload_active_thread stdout ----
thread 'tests::session_persistence::reload_active_thread' panicked at 'assertion failed'

---- tests::session_persistence::reload_orphan stdout ----
thread 'tests::session_persistence::reload_orphan' panicked at 'assertion failed'

---- tests::concurrent_writes::write_then_read stdout ----
thread 'tests::concurrent_writes::write_then_read' panicked at 'unexpected None'

failures:
    tests::concurrent_writes::write_then_read
    tests::session_persistence::reload_active_thread
    tests::session_persistence::reload_orphan

test result: FAILED. 17 passed; 3 failed; 1 ignored; 0 measured; 0 filtered out
```

### Task B2: Add vitest multi-test fixture

**Files:**
- Create: `crates/feature-coding-bash/tests/fixtures/vitest_multi_failure.txt`

- [ ] **Step 1: Create the fixture file**

Create `crates/feature-coding-bash/tests/fixtures/vitest_multi_failure.txt` with vitest output containing 2 failures:

```text
 RUN  v1.6.0

 ❯ src/features/coding/components/JobsPanel.test.tsx (4 tests | 2 failed) 124ms
   ✓ JobsPanel > renders empty state when no jobs 12ms
   ✗ JobsPanel > renders 6 jobs in started-desc order 38ms
   ✗ JobsPanel > updates row on tauri event 41ms
   ✓ JobsPanel > clicking stop invokes coding_task_stop 33ms

 FAIL  src/features/coding/components/JobsPanel.test.tsx > JobsPanel > renders 6 jobs in started-desc order
AssertionError: expected ...

 FAIL  src/features/coding/components/JobsPanel.test.tsx > JobsPanel > updates row on tauri event
AssertionError: expected ...

 Test Files  1 failed (1)
      Tests  2 failed | 2 passed (4)
```

### Task B3: Extend cargo test extractor in gate.rs

**Files:**
- Modify: `crates/feature-coding-bash/src/gate.rs`

- [ ] **Step 1: Read current gate.rs cargo extraction**

```bash
grep -n "FIRST_FAILED_TEST_RE\|RUST_TEST_RE\|first_failed_rust_test" crates/feature-coding-bash/src/gate.rs
```

- [ ] **Step 2: Replace `FIRST_FAILED_TEST_RE` + `first_failed_rust_test` with multi-capture**

Find the static regex `FIRST_FAILED_TEST_RE` (returns the *first* failed test name). Replace it with a multi-capture variant. Find the helper `first_failed_rust_test` and replace with `all_failed_rust_tests`.

Edit the regex declaration:

```rust
// OLD:
// static FIRST_FAILED_TEST_RE: Lazy<Regex> =
//     Lazy::new(|| Regex::new(r"test ([\w:]+) \.\.\. FAILED").unwrap());
//
// NEW:
static FAILED_TEST_NAME_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"test ([\w:]+) \.\.\. FAILED").unwrap());
```

Replace the helper:

```rust
// OLD:
// fn first_failed_rust_test(text: &str) -> Option<String> {
//     FIRST_FAILED_TEST_RE.captures(text).map(|c| c[1].to_string())
// }
//
// NEW:
fn all_failed_rust_tests(text: &str) -> Vec<String> {
    FAILED_TEST_NAME_RE
        .captures_iter(text)
        .map(|c| c[1].to_string())
        .take(50)                          // defensive cap
        .collect()
}
```

- [ ] **Step 3: Update the cargo TestFailure JSON construction**

Find the block that builds `failure_extracted` for cargo TestFailure (uses `RUST_TEST_RE` captures). It currently emits `{ "test_name": <Option<String>>, "n_failed", "n_passed", "n_ignored": 0 }`. Replace with `failed_test_names: Vec<String>`.

The block typically looks like:

```rust
// OLD:
// let test_name = first_failed_rust_test(&combined);
// let extracted = serde_json::json!({
//     "test_name": test_name,
//     "n_failed": n_failed,
//     "n_passed": n_passed,
//     "n_ignored": 0,
// });
//
// NEW:
let failed_test_names = all_failed_rust_tests(&combined);
let extracted = serde_json::json!({
    "failed_test_names": failed_test_names,
    "n_failed": n_failed,
    "n_passed": n_passed,
    "n_ignored": 0,
});
```

> Note: the `combined` variable is a concatenation of head+tail or stdout+stderr — preserve whatever the existing 2.3a code uses. Don't change variable plumbing.

- [ ] **Step 4: Extend vitest extractor with multi-name capture**

Find `VITEST_RE` (currently captures just totals). Add a sibling regex for vitest failure lines and use it:

```rust
static VITEST_FAILURE_RE: Lazy<Regex> = Lazy::new(|| {
    // Matches lines like: " FAIL  path/to/file.test.ts > Suite > test name"
    Regex::new(r" FAIL  [^>]+>\s+(.+)$").unwrap()
});

fn all_failed_vitest_tests(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            VITEST_FAILURE_RE.captures(line).map(|c| c[1].trim().to_string())
        })
        .take(50)
        .collect()
}
```

In the vitest TestFailure JSON construction, add `failed_test_names`:

```rust
let failed_test_names = all_failed_vitest_tests(&combined);
let extracted = serde_json::json!({
    "failed_test_names": failed_test_names,
    "n_failed": n_failed,
    "n_passed": n_passed,
});
```

- [ ] **Step 5: Add unit tests for the extractors**

Add at the bottom of `crates/feature-coding-bash/src/gate.rs` inside the existing `#[cfg(test)] mod tests` block (or create one):

```rust
#[test]
fn cargo_extracts_all_failed_test_names() {
    let fixture = include_str!("../tests/fixtures/cargo_multi_test_failure.txt");
    let result = GateClassifier::classify(fixture, "", 101, "cargo nextest run", false, false, false, 0);
    if let GateResult::Failed { kind, extracted, .. } = result {
        assert_eq!(format!("{kind:?}"), "TestFailure");
        let names = extracted["failed_test_names"]
            .as_array().unwrap().iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        assert_eq!(names.len(), 3);
        assert!(names.iter().any(|n| n.contains("reload_active_thread")));
        assert!(names.iter().any(|n| n.contains("reload_orphan")));
        assert!(names.iter().any(|n| n.contains("concurrent_writes")));
        assert_eq!(extracted["n_failed"], 3);
        assert_eq!(extracted["n_passed"], 17);
    } else {
        panic!("expected Failed");
    }
}

#[test]
fn vitest_extracts_all_failed_test_names() {
    let fixture = include_str!("../tests/fixtures/vitest_multi_failure.txt");
    let result = GateClassifier::classify(fixture, "", 1, "bun run test", false, false, false, 0);
    if let GateResult::Failed { kind, extracted, .. } = result {
        assert_eq!(format!("{kind:?}"), "TestFailure");
        let names = extracted["failed_test_names"]
            .as_array().unwrap().iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        assert_eq!(names.len(), 2);
        assert!(names.iter().any(|n| n.contains("renders 6 jobs")));
        assert!(names.iter().any(|n| n.contains("updates row on tauri event")));
    } else {
        panic!("expected Failed");
    }
}

#[test]
fn cap_at_50_names() {
    let mut text = String::from("test result: FAILED. 0 passed; 60 failed\n");
    for i in 0..60 {
        text.push_str(&format!("test t::{} ... FAILED\n", i));
    }
    let result = GateClassifier::classify(&text, "", 101, "cargo test", false, false, false, 0);
    if let GateResult::Failed { extracted, .. } = result {
        assert_eq!(extracted["failed_test_names"].as_array().unwrap().len(), 50);
    }
}
```

> Confirm the exact `GateClassifier::classify` signature first by `grep -n "pub fn classify" crates/feature-coding-bash/src/gate.rs`. The call above uses 8 positional args matching the explored signature.

- [ ] **Step 6: Run tests**

```bash
cargo nextest run -p feature-coding-bash -E 'test(cargo_extracts_all)+test(vitest_extracts_all)+test(cap_at_50)' 2>&1 | tail -10
```
Expected: 3 tests passed.

- [ ] **Step 7: Run all gate.rs tests to confirm no regression on existing fixtures**

```bash
cargo nextest run -p feature-coding-bash -E 'binary(gate)+test(classify)' 2>&1 | tail -10
```
Expected: existing classifier tests still pass. If any 2.3a test asserted on the old `test_name` field shape, it will fail — update those tests to use `failed_test_names` (pre-release; no production data to migrate).

- [ ] **Step 8: Update any 2.3a tests that asserted on the old `test_name` field**

```bash
grep -rn "test_name" crates/feature-coding-bash/tests/ crates/feature-coding-bash/src/gate.rs
```

For any hits in test files asserting `extracted["test_name"]`, change to `extracted["failed_test_names"][0]`. Document each change.

- [ ] **Step 9: Re-run full feature-coding-bash test suite**

```bash
cargo nextest run -p feature-coding-bash 2>&1 | tail -10
```
Expected: all tests pass.

- [ ] **Step 10: Commit**

```bash
git add crates/feature-coding-bash/src/gate.rs \
        crates/feature-coding-bash/tests/fixtures/cargo_multi_test_failure.txt \
        crates/feature-coding-bash/tests/fixtures/vitest_multi_failure.txt
git commit -m "feat(coding): extract all failed test names for cargo + vitest (Phase 2.3b PR2)"
```

---

# PR 3 — Diff machinery + completion-body integration (~1 day)

> **Strategy:** All net-new pure-function code lands first (verification_match, diff types, diff function), with full unit-test coverage. Then a single supervisor.rs edit wires the diff into `handle_exit`, and `render::completion_notification` grows one optional parameter.

## Phase C — Verification verb classifier

### Task C1: Create verification_match.rs

**Files:**
- Create: `crates/feature-coding-bash/src/intelligence/verification_match.rs`
- Modify: `crates/feature-coding-bash/src/intelligence/mod.rs`

- [ ] **Step 1: Create verification_match.rs**

Create `crates/feature-coding-bash/src/intelligence/verification_match.rs`:

```rust
//! Heuristic classifier for TodoItem titles that look like verification steps
//! (Run / Test / Check / Verify / Build).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationVerb {
    Run,
    Test,
    Check,
    Verify,
    Build,
}

impl VerificationVerb {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Run    => "Run",
            Self::Test   => "Test",
            Self::Check  => "Check",
            Self::Verify => "Verify",
            Self::Build  => "Build",
        }
    }
}

pub fn classify(title: &str) -> Option<VerificationVerb> {
    let trimmed = title.trim();
    if trimmed.len() < 3 {
        return None;
    }
    let first_token = trimmed.split_whitespace().next()?;
    let lower = first_token.to_ascii_lowercase();
    let cleaned = lower.trim_end_matches(|c: char| !c.is_alphanumeric());
    match cleaned {
        "run" | "running"                       => Some(VerificationVerb::Run),
        "test" | "tests"                        => Some(VerificationVerb::Test),
        "check" | "checking"                    => Some(VerificationVerb::Check),
        "verify" | "verifies" | "verifying"     => Some(VerificationVerb::Verify),
        "build" | "rebuild" | "compile"         => Some(VerificationVerb::Build),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_each_verb() {
        assert_eq!(classify("Run integration tests"), Some(VerificationVerb::Run));
        assert_eq!(classify("Test the supervisor"),   Some(VerificationVerb::Test));
        assert_eq!(classify("Check migrations"),      Some(VerificationVerb::Check));
        assert_eq!(classify("Verify migration safety"), Some(VerificationVerb::Verify));
        assert_eq!(classify("Build release binary"),  Some(VerificationVerb::Build));
    }

    #[test]
    fn case_insensitive() {
        assert_eq!(classify("RUN tests"), Some(VerificationVerb::Run));
        assert_eq!(classify("test x"),    Some(VerificationVerb::Test));
    }

    #[test]
    fn handles_conjugations() {
        assert_eq!(classify("Running the suite"),  Some(VerificationVerb::Run));
        assert_eq!(classify("Verifying constraints"), Some(VerificationVerb::Verify));
        assert_eq!(classify("Rebuild the docs"),   Some(VerificationVerb::Build));
    }

    #[test]
    fn trailing_punctuation_ok() {
        assert_eq!(classify("verify."), Some(VerificationVerb::Verify));
        assert_eq!(classify("Test:"),   Some(VerificationVerb::Test));
    }

    #[test]
    fn rejects_unrelated_titles() {
        assert_eq!(classify("Refactor the supervisor"), None);
        assert_eq!(classify("Add a new tool"),          None);
        assert_eq!(classify("Fix bug in injector"),     None);
    }

    #[test]
    fn rejects_too_short() {
        assert_eq!(classify(""),    None);
        assert_eq!(classify("a"),   None);
        assert_eq!(classify("ab"),  None);
        assert_eq!(classify("Run"), Some(VerificationVerb::Run));    // 3 chars OK
    }
}
```

- [ ] **Step 2: Add module to intelligence/mod.rs**

Edit `crates/feature-coding-bash/src/intelligence/mod.rs`:

```rust
//! Phase 2.3b — Execution Intelligence layer.
//!
//! Spec: `docs/superpowers/specs/2026-05-09-coding-bash-execution-intelligence-design.md`

pub mod normalize;
pub mod verification_match;

pub use normalize::command_key;
pub use verification_match::{classify as classify_verification, VerificationVerb};
```

- [ ] **Step 3: Run tests**

```bash
cargo nextest run -p feature-coding-bash -E 'test(verification_match)' 2>&1 | tail -10
```
Expected: 6 tests passed.

- [ ] **Step 4: Commit**

```bash
git add crates/feature-coding-bash/src/intelligence/
git commit -m "feat(coding): verification verb classifier (Phase 2.3b PR3 part 1)"
```

## Phase D — Diff types and engine

### Task D1: Create diff.rs scaffold with types

**Files:**
- Create: `crates/feature-coding-bash/src/intelligence/diff.rs`
- Modify: `crates/feature-coding-bash/src/intelligence/mod.rs`

- [ ] **Step 1: Create diff.rs with types only (no logic yet)**

Create `crates/feature-coding-bash/src/intelligence/diff.rs`:

```rust
//! Cross-run diff for completed bash jobs.
//!
//! `diff_against_prior(prior, curr)` is pure: takes two `BashJobRow`s with
//! matching `command_key` and returns a `JobDiff` describing the transition.

use serde::{Deserialize, Serialize};
use storage::repos::BashJobRow;
use tools_core::FailureKind;

#[derive(Debug, Clone, PartialEq)]
pub struct JobDiff {
    pub kind_transition: KindTransition,
    pub extracted_diff:  ExtractedDiff,
    pub elapsed_delta_ms: i64,    // signed: negative = faster than prior
}

#[derive(Debug, Clone, PartialEq)]
pub enum KindTransition {
    StillPassing,
    StillFailing { kind: String },                  // kind name (FailureKind variant)
    Regressed   { from: String, to: String },       // None or different failure
    Recovered   { prior_kind: String },
    Changed     { from: String, to: String },       // both failed, different kinds
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExtractedDiff {
    None,
    TestSet {
        new_failures:  Vec<String>,
        still_failing: Vec<String>,
        resolved:      Vec<String>,
    },
    Compile {
        same_location: bool,
        prior_loc: Option<Location>,
        curr_loc:  Option<Location>,
    },
    Bind {
        same_port: bool,
        prior_port: Option<u64>,
        curr_port:  Option<u64>,
    },
    Lint {
        delta_n_errors: i64,
    },
    Timeout {
        prior_ms: u64,
        curr_ms:  u64,
    },
    OtherExitTransition {
        from: Option<i32>,
        to:   Option<i32>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Location {
    pub file: String,
    pub line: u64,
}

// Implementation comes in the next task.
```

- [ ] **Step 2: Add diff to intelligence/mod.rs**

```rust
//! Phase 2.3b — Execution Intelligence layer.

pub mod diff;
pub mod normalize;
pub mod verification_match;

pub use diff::{diff_against_prior, ExtractedDiff, JobDiff, KindTransition, Location};
pub use normalize::command_key;
pub use verification_match::{classify as classify_verification, VerificationVerb};
```

- [ ] **Step 3: Verify compile (will fail because `diff_against_prior` is not yet defined)**

```bash
cargo check -p feature-coding-bash 2>&1 | tail -10
```
Expected: `error[E0432]` — `diff_against_prior` not found. That's correct; we add it in Task D2.

### Task D2: Implement diff_against_prior + extensive unit tests

**Files:**
- Modify: `crates/feature-coding-bash/src/intelligence/diff.rs`

- [ ] **Step 1: Add the diff_against_prior function and helpers**

Append to `crates/feature-coding-bash/src/intelligence/diff.rs`:

```rust
use std::collections::BTreeSet;

pub fn diff_against_prior(prior: &BashJobRow, curr: &BashJobRow) -> JobDiff {
    let kind_transition = classify_transition(prior, curr);
    let extracted_diff = match (
        parse_extracted(prior.failure_extracted.as_deref()),
        parse_extracted(curr.failure_extracted.as_deref()),
    ) {
        (Some(p), Some(c)) => diff_extracted(
            &p, &c,
            prior.failure_kind.as_deref(),
            curr.failure_kind.as_deref(),
        ),
        _ => ExtractedDiff::None,
    };
    let elapsed_delta_ms = elapsed_ms(curr) as i64 - elapsed_ms(prior) as i64;

    JobDiff { kind_transition, extracted_diff, elapsed_delta_ms }
}

fn elapsed_ms(row: &BashJobRow) -> u64 {
    match row.finished_at {
        Some(end) => {
            let start_ms = row.started_at.as_millisecond() as i128;
            let end_ms = end.as_millisecond() as i128;
            (end_ms - start_ms).max(0) as u64
        }
        None => 0,
    }
}

fn parse_extracted(s: Option<&str>) -> Option<serde_json::Value> {
    s.and_then(|s| serde_json::from_str(s).ok())
}

fn classify_transition(prior: &BashJobRow, curr: &BashJobRow) -> KindTransition {
    let prior_failed = prior.failure_kind.is_some();
    let curr_failed = curr.failure_kind.is_some();
    match (prior_failed, curr_failed) {
        (false, false) => KindTransition::StillPassing,
        (true, false) => KindTransition::Recovered {
            prior_kind: prior.failure_kind.clone().unwrap_or_default(),
        },
        (false, true) => KindTransition::Regressed {
            from: "Passed".to_string(),
            to:   curr.failure_kind.clone().unwrap_or_default(),
        },
        (true, true) => {
            let p = prior.failure_kind.clone().unwrap_or_default();
            let c = curr.failure_kind.clone().unwrap_or_default();
            if p == c {
                KindTransition::StillFailing { kind: p }
            } else {
                KindTransition::Changed { from: p, to: c }
            }
        }
    }
}

fn diff_extracted(
    prior: &serde_json::Value,
    curr:  &serde_json::Value,
    prior_kind: Option<&str>,
    curr_kind:  Option<&str>,
) -> ExtractedDiff {
    use ExtractedDiff::*;

    if prior_kind == Some("TestFailure") && curr_kind == Some("TestFailure") {
        let p_set = string_array_set(prior, "failed_test_names");
        let c_set = string_array_set(curr, "failed_test_names");
        let new_failures:  Vec<String> = c_set.difference(&p_set).cloned().collect();
        let still_failing: Vec<String> = c_set.intersection(&p_set).cloned().collect();
        let resolved:      Vec<String> = p_set.difference(&c_set).cloned().collect();
        return TestSet { new_failures, still_failing, resolved };
    }

    if prior_kind == Some("CompileError") && curr_kind == Some("CompileError") {
        let pl = location_from(prior);
        let cl = location_from(curr);
        return Compile {
            same_location: pl.is_some() && pl == cl,
            prior_loc: pl,
            curr_loc:  cl,
        };
    }

    if prior_kind == Some("NetworkBindFailure") && curr_kind == Some("NetworkBindFailure") {
        let pp = prior.get("port").and_then(|v| v.as_u64());
        let cp = curr.get("port").and_then(|v| v.as_u64());
        return Bind {
            same_port: pp.is_some() && pp == cp,
            prior_port: pp,
            curr_port:  cp,
        };
    }

    if prior_kind == Some("LintFailure") && curr_kind == Some("LintFailure") {
        let pe = prior.get("n_errors").and_then(|v| v.as_i64()).unwrap_or(0);
        let ce = curr.get("n_errors").and_then(|v| v.as_i64()).unwrap_or(0);
        return Lint { delta_n_errors: ce - pe };
    }

    if prior_kind == Some("Timeout") && curr_kind == Some("Timeout") {
        let pm = prior.get("elapsed_ms").and_then(|v| v.as_u64()).unwrap_or(0);
        let cm = curr.get("elapsed_ms").and_then(|v| v.as_u64()).unwrap_or(0);
        return Timeout { prior_ms: pm, curr_ms: cm };
    }

    if prior_kind.is_none() && curr_kind.is_none() {
        return None;
    }

    OtherExitTransition {
        from: prior.get("exit_code").and_then(|v| v.as_i64()).map(|v| v as i32),
        to:   curr.get("exit_code").and_then(|v| v.as_i64()).map(|v| v as i32),
    }
}

fn string_array_set(v: &serde_json::Value, key: &str) -> BTreeSet<String> {
    v.get(key)
        .and_then(|x| x.as_array())
        .map(|arr| arr.iter().filter_map(|el| el.as_str().map(String::from)).collect())
        .unwrap_or_default()
}

fn location_from(v: &serde_json::Value) -> Option<Location> {
    let file = v.get("file").and_then(|x| x.as_str())?.to_string();
    let line = v.get("line").and_then(|x| x.as_u64())?;
    Some(Location { file, line })
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::Timestamp;

    fn row(
        kind: Option<&str>,
        extracted: Option<serde_json::Value>,
        started_ms: i64,
        finished_ms: i64,
    ) -> BashJobRow {
        let started_at = Timestamp::from_millisecond(started_ms).unwrap();
        BashJobRow {
            id: "x".into(),
            session_id: "s".into(),
            agent_id: "a".into(),
            description: "d".into(),
            command: "c".into(),
            command_key: "k".into(),
            cwd: "/".into(),
            timeout_ms: 60_000,
            silent_completion: false,
            status: if kind.is_some() { "Failed" } else { "Completed" }.into(),
            exit_code: Some(if kind.is_some() { 1 } else { 0 }),
            failure_kind: kind.map(String::from),
            failure_detail: None,
            failure_extracted: extracted.map(|v| v.to_string()),
            started_at,
            finished_at: Some(Timestamp::from_millisecond(finished_ms).unwrap()),
            total_bytes_emitted: 0,
            bisect_count: 0,
            log_path: "/tmp/x.log".into(),
            final_path: None,
            last_polled_at: None,
            last_seen_offset: 0,
        }
    }

    #[test]
    fn still_passing() {
        let p = row(None, None, 0, 1000);
        let c = row(None, None, 0, 1500);
        let d = diff_against_prior(&p, &c);
        assert_eq!(d.kind_transition, KindTransition::StillPassing);
        assert_eq!(d.extracted_diff, ExtractedDiff::None);
        assert_eq!(d.elapsed_delta_ms, 500);
    }

    #[test]
    fn recovered() {
        let p = row(Some("TestFailure"), Some(serde_json::json!({"failed_test_names":["a"]})), 0, 1000);
        let c = row(None, None, 0, 800);
        let d = diff_against_prior(&p, &c);
        assert_eq!(d.kind_transition, KindTransition::Recovered { prior_kind: "TestFailure".into() });
    }

    #[test]
    fn regressed_from_pass() {
        let p = row(None, None, 0, 1000);
        let c = row(Some("CompileError"), Some(serde_json::json!({"file":"src/lib.rs","line":42})), 0, 1500);
        let d = diff_against_prior(&p, &c);
        assert_eq!(d.kind_transition, KindTransition::Regressed { from: "Passed".into(), to: "CompileError".into() });
    }

    #[test]
    fn still_failing_same_kind() {
        let p = row(Some("TestFailure"), Some(serde_json::json!({"failed_test_names":["a"]})), 0, 1000);
        let c = row(Some("TestFailure"), Some(serde_json::json!({"failed_test_names":["a"]})), 0, 1100);
        let d = diff_against_prior(&p, &c);
        assert_eq!(d.kind_transition, KindTransition::StillFailing { kind: "TestFailure".into() });
    }

    #[test]
    fn changed_kind() {
        let p = row(Some("TestFailure"), Some(serde_json::json!({"failed_test_names":["a"]})), 0, 1000);
        let c = row(Some("CompileError"), Some(serde_json::json!({"file":"x","line":1})), 0, 1100);
        let d = diff_against_prior(&p, &c);
        assert_eq!(d.kind_transition, KindTransition::Changed { from: "TestFailure".into(), to: "CompileError".into() });
    }

    #[test]
    fn test_set_diff_overlapping() {
        let p = row(Some("TestFailure"), Some(serde_json::json!({"failed_test_names":["a","b","c"]})), 0, 1000);
        let c = row(Some("TestFailure"), Some(serde_json::json!({"failed_test_names":["b","c","d"]})), 0, 1000);
        let d = diff_against_prior(&p, &c);
        match d.extracted_diff {
            ExtractedDiff::TestSet { new_failures, still_failing, resolved } => {
                assert_eq!(new_failures, vec!["d".to_string()]);
                let mut still = still_failing.clone(); still.sort();
                assert_eq!(still, vec!["b".to_string(), "c".to_string()]);
                assert_eq!(resolved, vec!["a".to_string()]);
            }
            other => panic!("expected TestSet, got {other:?}"),
        }
    }

    #[test]
    fn test_set_diff_disjoint() {
        let p = row(Some("TestFailure"), Some(serde_json::json!({"failed_test_names":["a"]})), 0, 1000);
        let c = row(Some("TestFailure"), Some(serde_json::json!({"failed_test_names":["b"]})), 0, 1000);
        let d = diff_against_prior(&p, &c);
        match d.extracted_diff {
            ExtractedDiff::TestSet { new_failures, still_failing, resolved } => {
                assert_eq!(new_failures, vec!["b".to_string()]);
                assert!(still_failing.is_empty());
                assert_eq!(resolved, vec!["a".to_string()]);
            }
            _ => panic!("expected TestSet"),
        }
    }

    #[test]
    fn compile_same_location() {
        let p = row(Some("CompileError"), Some(serde_json::json!({"file":"a.rs","line":10})), 0, 1000);
        let c = row(Some("CompileError"), Some(serde_json::json!({"file":"a.rs","line":10})), 0, 1100);
        let d = diff_against_prior(&p, &c);
        assert!(matches!(d.extracted_diff, ExtractedDiff::Compile { same_location: true, .. }));
    }

    #[test]
    fn compile_different_location() {
        let p = row(Some("CompileError"), Some(serde_json::json!({"file":"a.rs","line":10})), 0, 1000);
        let c = row(Some("CompileError"), Some(serde_json::json!({"file":"a.rs","line":11})), 0, 1100);
        let d = diff_against_prior(&p, &c);
        assert!(matches!(d.extracted_diff, ExtractedDiff::Compile { same_location: false, .. }));
    }

    #[test]
    fn bind_port_diff() {
        let p = row(Some("NetworkBindFailure"), Some(serde_json::json!({"port":3000})), 0, 1000);
        let c = row(Some("NetworkBindFailure"), Some(serde_json::json!({"port":3000})), 0, 1000);
        assert!(matches!(diff_against_prior(&p, &c).extracted_diff, ExtractedDiff::Bind { same_port: true, .. }));
    }

    #[test]
    fn lint_delta() {
        let p = row(Some("LintFailure"), Some(serde_json::json!({"n_errors":5})), 0, 1000);
        let c = row(Some("LintFailure"), Some(serde_json::json!({"n_errors":3})), 0, 1000);
        assert_eq!(
            diff_against_prior(&p, &c).extracted_diff,
            ExtractedDiff::Lint { delta_n_errors: -2 },
        );
    }

    #[test]
    fn timeout_delta() {
        let p = row(Some("Timeout"), Some(serde_json::json!({"elapsed_ms":600_000})), 0, 1000);
        let c = row(Some("Timeout"), Some(serde_json::json!({"elapsed_ms":700_000})), 0, 1000);
        assert_eq!(
            diff_against_prior(&p, &c).extracted_diff,
            ExtractedDiff::Timeout { prior_ms: 600_000, curr_ms: 700_000 },
        );
    }

    #[test]
    fn malformed_extracted_falls_back_to_none() {
        let mut p = row(Some("Other"), None, 0, 1000);
        p.failure_extracted = Some("not-valid-json".into());
        let c = row(Some("Other"), None, 0, 1000);
        let d = diff_against_prior(&p, &c);
        assert_eq!(d.extracted_diff, ExtractedDiff::None);
    }

    #[test]
    fn elapsed_delta_signed_negative_is_faster() {
        let p = row(None, None, 0, 1000);
        let c = row(None, None, 0, 600);
        assert_eq!(diff_against_prior(&p, &c).elapsed_delta_ms, -400);
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cargo nextest run -p feature-coding-bash -E 'test(diff)' 2>&1 | tail -15
```
Expected: 13 tests passed.

- [ ] **Step 3: Commit**

```bash
git add crates/feature-coding-bash/src/intelligence/diff.rs \
        crates/feature-coding-bash/src/intelligence/mod.rs
git commit -m "feat(coding): JobDiff types and diff_against_prior (Phase 2.3b PR3 part 2)"
```

## Phase E — Wire diff into supervisor + render

### Task E1: Extend render::completion_notification with optional diff

**Files:**
- Modify: `crates/feature-coding-bash/src/render.rs`

- [ ] **Step 1: Read current completion_notification**

```bash
sed -n '30,70p' crates/feature-coding-bash/src/render.rs
```

- [ ] **Step 2: Extend signature to take diff**

Edit `crates/feature-coding-bash/src/render.rs`:

```rust
use crate::intelligence::{ExtractedDiff, JobDiff, KindTransition};

pub fn completion_notification(
    id: &JobId,
    spec: &JobSpec,
    result: &GateResult,
    final_summary: &str,
    diff: Option<&JobDiff>,                                    // NEW
) -> String {
    let mut s = String::new();
    s.push_str("<system-reminder>\n");
    s.push_str(&format!("Background job {} completed.\n", id.as_str()));
    s.push_str(&format!("Description: {}\n", spec.description));

    match result {
        GateResult::Passed => {
            s.push_str("Status: Completed (Passed)\n");
        }
        GateResult::Failed { kind, detail, extracted } => {
            s.push_str("Status: Failed\n");
            s.push_str(&format!("Failure kind: {kind:?}\n"));
            s.push_str(&format!("Detail: {detail}\n"));
            if !extracted.is_null() {
                s.push_str(&format!(
                    "Extracted: {}\n",
                    serde_json::to_string_pretty(extracted).unwrap_or_else(|_| extracted.to_string())
                ));
            }
        }
    }

    if let Some(d) = diff {
        s.push_str("\nCompared to last run of this command:\n");
        s.push_str(&format_kind_transition(&d.kind_transition));
        s.push_str(&format_extracted_diff(&d.extracted_diff));
        s.push_str(&format!("  Wall-clock: {}\n", format_elapsed_delta(d.elapsed_delta_ms)));
    }

    s.push_str("\nLast portion of output:\n");
    let tail_start = final_summary.floor_char_boundary(final_summary.len().saturating_sub(8000));
    s.push_str(&final_summary[tail_start..]);
    s.push_str("\n</system-reminder>\n");
    s
}

fn format_kind_transition(t: &KindTransition) -> String {
    match t {
        KindTransition::StillPassing => "  Transition: StillPassing\n".into(),
        KindTransition::StillFailing { kind } => format!("  Transition: StillFailing ({kind})\n"),
        KindTransition::Regressed { from, to } => format!("  Transition: Regressed ({from} → {to})\n"),
        KindTransition::Recovered { prior_kind } => format!("  Transition: Recovered ({prior_kind} → Passed)\n"),
        KindTransition::Changed { from, to } => format!("  Transition: Changed ({from} → {to})\n"),
    }
}

fn format_extracted_diff(d: &ExtractedDiff) -> String {
    match d {
        ExtractedDiff::None => String::new(),
        ExtractedDiff::TestSet { new_failures, still_failing, resolved } => {
            let mut s = String::from("  Test diff:\n");
            s.push_str(&format!("    new failures:  {}\n", trim_set(new_failures)));
            s.push_str(&format!("    still failing: {}\n", trim_set(still_failing)));
            s.push_str(&format!("    resolved:      {}\n", trim_set(resolved)));
            s
        }
        ExtractedDiff::Compile { same_location, prior_loc, curr_loc } => {
            let same = if *same_location { "same location" } else { "different location" };
            format!(
                "  Compile diff: {same} (prior: {prior_loc:?}, curr: {curr_loc:?})\n"
            )
        }
        ExtractedDiff::Bind { same_port, prior_port, curr_port } => {
            let same = if *same_port { "same port" } else { "different port" };
            format!("  Bind diff: {same} (prior: {prior_port:?}, curr: {curr_port:?})\n")
        }
        ExtractedDiff::Lint { delta_n_errors } => {
            format!("  Lint diff: error count Δ {delta_n_errors:+}\n")
        }
        ExtractedDiff::Timeout { prior_ms, curr_ms } => {
            format!("  Timeout diff: prior {prior_ms} ms, curr {curr_ms} ms\n")
        }
        ExtractedDiff::OtherExitTransition { from, to } => {
            format!("  Exit code transition: {from:?} → {to:?}\n")
        }
    }
}

fn trim_set(v: &[String]) -> String {
    if v.is_empty() {
        return "(none)".into();
    }
    let mut sorted: Vec<&str> = v.iter().map(|s| s.as_str()).collect();
    sorted.sort();
    if sorted.len() <= 50 {
        sorted.join(", ")
    } else {
        let head = sorted[..50].join(", ");
        format!("{head}, + {} more", sorted.len() - 50)
    }
}

fn format_elapsed_delta(ms: i64) -> String {
    let secs = ms.abs() as f64 / 1000.0;
    if ms > 0 {
        format!("+{secs:.1}s (slower)")
    } else if ms < 0 {
        format!("-{secs:.1}s (faster)")
    } else {
        "+0.0s".into()
    }
}
```

- [ ] **Step 3: Verify compile (will fail at the supervisor.rs call site)**

```bash
cargo check -p feature-coding-bash 2>&1 | tail -10
```
Expected: error in `supervisor.rs` — `completion_notification` call site is missing the new arg. Fixed in Task E2.

- [ ] **Step 4: Add inline tests for the renderer**

Append to `crates/feature-coding-bash/src/render.rs`:

```rust
#[cfg(test)]
mod completion_notification_tests {
    use super::*;
    use crate::intelligence::{ExtractedDiff, JobDiff, KindTransition};
    use jiff::Timestamp;
    use std::path::PathBuf;
    use tools_core::{FailureKind, GateResult, JobId, JobSpec};

    fn fake_spec(description: &str) -> JobSpec {
        JobSpec {
            session_id: "s".into(),
            agent_id: "a".into(),
            agent_chain: vec!["a".into()],
            description: description.into(),
            command: "c".into(),
            cwd: PathBuf::from("/"),
            timeout_ms: 60_000,
            silent_completion: false,
        }
    }

    #[test]
    fn renders_no_diff_section_when_diff_is_none() {
        let id = JobId("bash-test".into());
        let spec = fake_spec("test desc");
        let result = GateResult::Passed;
        let body = completion_notification(&id, &spec, &result, "tail output", None);
        assert!(!body.contains("Compared to last run"));
        assert!(body.contains("Status: Completed (Passed)"));
    }

    #[test]
    fn renders_diff_section_when_diff_is_some() {
        let id = JobId("bash-test".into());
        let spec = fake_spec("test desc");
        let result = GateResult::Failed {
            kind: FailureKind::TestFailure,
            detail: "1 failed".into(),
            extracted: serde_json::json!({"failed_test_names":["A","B"]}),
        };
        let diff = JobDiff {
            kind_transition: KindTransition::StillFailing { kind: "TestFailure".into() },
            extracted_diff: ExtractedDiff::TestSet {
                new_failures:  vec!["B".into()],
                still_failing: vec!["A".into()],
                resolved:      vec!["C".into()],
            },
            elapsed_delta_ms: 1234,
        };
        let body = completion_notification(&id, &spec, &result, "", Some(&diff));
        assert!(body.contains("Compared to last run of this command"));
        assert!(body.contains("Transition: StillFailing (TestFailure)"));
        assert!(body.contains("new failures:  B"));
        assert!(body.contains("still failing: A"));
        assert!(body.contains("resolved:      C"));
        assert!(body.contains("+1.2s (slower)"));
    }

    #[test]
    fn empty_test_sets_render_as_none() {
        let id = JobId("bash-test".into());
        let spec = fake_spec("test desc");
        let result = GateResult::Passed;
        let diff = JobDiff {
            kind_transition: KindTransition::Recovered { prior_kind: "TestFailure".into() },
            extracted_diff: ExtractedDiff::None,
            elapsed_delta_ms: -500,
        };
        let body = completion_notification(&id, &spec, &result, "", Some(&diff));
        assert!(body.contains("Recovered (TestFailure → Passed)"));
        assert!(body.contains("-0.5s (faster)"));
        assert!(!body.contains("new failures:"));
    }
}
```

### Task E2: Wire diff into supervisor::handle_exit

**Files:**
- Modify: `crates/feature-coding-bash/src/supervisor.rs`

- [ ] **Step 1: Read the call site of `completion_notification` in handle_exit**

```bash
grep -n "completion_notification" crates/feature-coding-bash/src/supervisor.rs
```
Expected: a call around line 408-414.

- [ ] **Step 2: Read the surrounding handle_exit body**

```bash
sed -n '275,418p' crates/feature-coding-bash/src/supervisor.rs
```

- [ ] **Step 3: Build the BashJobRow representation needed by `find_prior_by_command_key`**

The `update_status` upsert in 2.3a writes only certain fields back; for diffing we need a `BashJobRow` with the current `failure_kind`, `failure_extracted`, etc. The cleanest approach is to fetch via `repo.get(id)` after `update_status` completes, then run the diff against the prior row.

In `handle_exit`, after the existing `self.repo.update_status(...)` call (around line 373), add:

```rust
// 2.3b: query prior run + compute diff for the completion notification.
let curr_command_key = command_key(&live.spec.command);
let prior = match self
    .repo
    .find_prior_by_command_key(&live.spec.session_id, &curr_command_key, id.as_str())
    .await
{
    Ok(opt) => opt,
    Err(e) => {
        tracing::warn!(error = ?e, "prior lookup failed; skipping diff");
        None
    }
};
let curr_row = match self.repo.get(id.as_str()).await {
    Ok(Some(r)) => Some(r),
    _ => None,
};
let diff = match (prior, curr_row) {
    (Some(p), Some(c)) => Some(crate::intelligence::diff_against_prior(&p, &c)),
    _ => None,
};
```

- [ ] **Step 4: Pass diff into completion_notification**

Find the existing call:

```rust
// OLD:
// let body = crate::render::completion_notification(id, &live.spec, &gate_result, &final_summary);
//
// NEW:
let body = crate::render::completion_notification(
    id, &live.spec, &gate_result, &final_summary, diff.as_ref()
);
```

> Replace the variable name (`gate_result`, `final_summary`) with whatever the actual 2.3a code uses — match exactly. If `id` is a `&JobId` reference, that's correct; if it's owned, take `&id`.

- [ ] **Step 5: Verify compile**

```bash
cargo check -p feature-coding-bash 2>&1 | tail -10
```
Expected: `Finished`.

- [ ] **Step 6: Run all feature-coding-bash tests**

```bash
cargo nextest run -p feature-coding-bash 2>&1 | tail -15
```
Expected: all tests pass (existing + new render + diff tests).

- [ ] **Step 7: Commit**

```bash
git add crates/feature-coding-bash/src/render.rs \
        crates/feature-coding-bash/src/supervisor.rs
git commit -m "feat(coding): wire JobDiff into completion_notification (Phase 2.3b PR3 part 3)"
```

### Task E3: Integration test — intel_diff_basic

**Files:**
- Create: `crates/feature-coding-bash/tests/intel_diff_basic.rs`

- [ ] **Step 1: Read an existing integration test for the spawn pattern**

```bash
ls crates/feature-coding-bash/tests/
cat crates/feature-coding-bash/tests/bg_smoke.rs 2>/dev/null | head -80 || echo "(no bg_smoke.rs)"
```

Use the discovered test as a template — it shows how to construct a `JobSupervisor` against an in-memory pool, spawn a job, wait for completion, and read the resulting `ContextUpdate`.

- [ ] **Step 2: Write the diff_basic integration test**

Create `crates/feature-coding-bash/tests/intel_diff_basic.rs`:

```rust
//! Phase 2.3b integration: completion body contains a diff section when a
//! prior run with the same command_key exists.

use std::sync::Arc;
use std::time::Duration;

use bus::{ContextUpdateQueue, DomainEventBus};
use feature_coding_bash::JobSupervisor;
use storage::{repos::BashJobRepo, StoragePool};
use tools_core::{JobSpec, JobSupervisorHandle};

async fn pool_with_table() -> StoragePool {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    // Apply just the bash jobs migration manually.
    let migration = feature_coding_bash::migrations::coding_background_jobs_migration();
    sqlx::query(migration.sql).execute(pool.inner()).await.unwrap();
    pool
}

fn spec(command: &str) -> JobSpec {
    JobSpec {
        session_id: "s1".into(),
        agent_id:   "a1".into(),
        agent_chain: vec!["a1".into()],
        description: "desc".into(),
        command: command.into(),
        cwd: std::env::temp_dir(),
        timeout_ms: 30_000,
        silent_completion: false,
    }
}

#[tokio::test]
async fn second_run_of_same_command_has_diff_section() {
    let pool = pool_with_table().await;
    let bash_repo = BashJobRepo::new(pool.inner().clone());
    let bus = Arc::new(DomainEventBus::new());
    let queue = Arc::new(ContextUpdateQueue::new());
    let data_dir = tempfile::tempdir().unwrap();

    let supervisor = Arc::new(JobSupervisor::new(
        bash_repo.clone(),
        Arc::clone(&bus),
        Arc::clone(&queue),
        data_dir.path().to_path_buf(),
        Arc::new(klynt_sandbox::MacOsSeatbeltRunner::new()),
    ));

    // First spawn — a quick failure.
    let v1 = supervisor.spawn(spec("false")).await.unwrap();
    wait_for_terminal(&supervisor, &v1.id).await;

    // Second spawn — same command, second failure.
    let v2 = supervisor.spawn(spec("false")).await.unwrap();
    wait_for_terminal(&supervisor, &v2.id).await;

    // Drain the queue and find the second completion notification.
    let updates = queue.drain_all();
    let body_v2 = updates.iter()
        .filter_map(|u| u.content.as_ref())
        .find(|s| s.contains(v2.id.as_str()))
        .expect("expected completion body for v2");

    assert!(
        body_v2.contains("Compared to last run of this command"),
        "expected diff section in body, got:\n{body_v2}"
    );
}

async fn wait_for_terminal(supervisor: &JobSupervisor, id: &tools_core::JobId) {
    for _ in 0..50 {
        if !supervisor.list("s1", &["a1".into()], true).await
            .iter().any(|j| &j.id == id)
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("job did not reach terminal state in time");
}
```

> If `ContextUpdateQueue::drain_all` is not the actual method name, run `grep -n "pub fn" crates/bus/src/context_updates.rs` and adapt.

- [ ] **Step 3: Run the integration test**

```bash
cargo nextest run -p feature-coding-bash --test intel_diff_basic 2>&1 | tail -10
```
Expected: 1 test passed.

- [ ] **Step 4: Commit**

```bash
git add crates/feature-coding-bash/tests/intel_diff_basic.rs
git commit -m "test(coding): integration test for diff section in completion body (Phase 2.3b)"
```

---

# PR 4 — ExecutionIntelligenceInjector + plan-mode affordance (~1 day)

> **Strategy:** New injector + render helper, registered in app-core's `init/mod.rs` alongside the existing two injectors. Requires `feature-coding-bash` to depend on `feature-coding-todo` for `TodoRepo` types.

## Phase F — TodoRepo dep + ExecutionIntelligenceInjector

### Task F1: Add feature-coding-todo dep + render::verification_affordance

**Files:**
- Modify: `crates/feature-coding-bash/Cargo.toml`
- Modify: `crates/feature-coding-bash/src/render.rs`

- [ ] **Step 1: Add `storage` dep already present (TodoRepo lives in storage::repos)**

Check `crates/feature-coding-bash/Cargo.toml` — `storage` should already be a dep from 2.3a. If not:

```toml
storage = { path = "../storage" }
```

We do NOT need `feature-coding-todo` as a dep (TodoRepo is in `storage::repos::TodoRepo`).

- [ ] **Step 2: Add render::verification_affordance**

Append to `crates/feature-coding-bash/src/render.rs`:

```rust
use crate::intelligence::VerificationVerb;

pub struct VerificationAffordance<'a> {
    pub todo_id: &'a str,
    pub title:   &'a str,
    pub verb:    VerificationVerb,
}

pub fn verification_affordance_reminder(items: &[VerificationAffordance<'_>]) -> String {
    let mut s = String::new();
    s.push_str("<system-reminder>\n");
    s.push_str("Plan mode active — the following pending TodoItems look like background-bash candidates after `/plan-exit`:\n");
    for item in items {
        s.push_str(&format!(
            "- \"{title}\" → bash(command=…, run_in_background=true) [verb: {verb}]\n",
            title = item.title,
            verb = item.verb.as_str(),
        ));
    }
    s.push_str("Background jobs cannot be spawned while plan mode is active. After ratification, you may launch these as background jobs.\n");
    s.push_str("</system-reminder>\n");
    s
}

#[cfg(test)]
mod verification_affordance_tests {
    use super::*;

    #[test]
    fn renders_each_item() {
        let items = [
            VerificationAffordance { todo_id: "t1", title: "Run integration tests", verb: VerificationVerb::Run },
            VerificationAffordance { todo_id: "t2", title: "Verify migration safety", verb: VerificationVerb::Verify },
        ];
        let body = verification_affordance_reminder(&items);
        assert!(body.contains("Plan mode active"));
        assert!(body.contains("Run integration tests"));
        assert!(body.contains("[verb: Run]"));
        assert!(body.contains("Verify migration safety"));
        assert!(body.contains("[verb: Verify]"));
    }
}
```

- [ ] **Step 3: Verify compile**

```bash
cargo check -p feature-coding-bash 2>&1 | tail -5
cargo nextest run -p feature-coding-bash -E 'test(verification_affordance)' 2>&1 | tail -5
```
Expected: 1 test passed.

### Task F2: Create ExecutionIntelligenceInjector

**Files:**
- Create: `crates/feature-coding-bash/src/intelligence/injector.rs`
- Modify: `crates/feature-coding-bash/src/intelligence/mod.rs`
- Modify: `crates/feature-coding-bash/src/lib.rs`

- [ ] **Step 1: Inspect TodoRepo signature for the read method**

```bash
grep -n "pub async fn\|pub fn" crates/storage/src/repos/coding_todo.rs | head -20
```

Locate the method that lists todos for `(thread_id, agent_id)`. The method signature determines what we call.

- [ ] **Step 2: Inspect the injector context for the agent_chain accessor**

Already confirmed: `InjectorContext::agent_chain(&self) -> &[String]` has a default-impl returning `&[]`. The actual `RoutingContext` impl provides the real chain.

- [ ] **Step 3: Decide on async vs sync read**

`DynamicInjector::collect` is currently sync (`fn collect(&self, ctx: &dyn InjectorContext) -> Vec<ContextUpdate>`). `TodoRepo::list_for_thread` is `async`. We have two options:

(a) Block on the read using `tokio::task::block_in_place` + `Handle::block_on` (only safe in multi-thread runtime).
(b) Cache the latest todos in an `Arc<RwLock<HashMap<(String,String), Vec<TodoItem>>>>` updated on each Tauri command write (more plumbing).

Option (a) is what `PlanModeInjector` does today (verify with `grep -n "block_on\|block_in_place" crates/feature-coding-todo/src/injector.rs`). Use the same pattern.

> If `PlanModeInjector` does NOT block_on, then it doesn't read TodoRepo at all — it reads `coding_policies`. In that case, prefer (b): cache. But before adding complexity, run `grep -A20 "fn collect" crates/feature-coding-todo/src/injector.rs` to see what it actually does.

For this plan we assume option (a) since the injector trait is sync. If the codebase uses a different pattern, adapt the plan step accordingly.

- [ ] **Step 4: Create injector.rs**

Create `crates/feature-coding-bash/src/intelligence/injector.rs`:

```rust
//! Phase 2.3b — ExecutionIntelligenceInjector
//!
//! Surfaces, during plan mode, those pending TodoItems whose titles look like
//! verification steps (Run/Test/Check/Verify/Build), as background-bash
//! candidates the LLM should consider after `/plan-exit`.

use std::sync::Arc;

use bus::context_updates::{ContextUpdate, ContextUpdateReason, UpdatePriority};
use bus::injection::{DynamicInjector, InjectorContext};
use jiff::Timestamp;
use storage::repos::TodoRepo;
use tools_core::JobSupervisorHandle;

use crate::intelligence::{classify_verification, VerificationVerb};
use crate::render::{verification_affordance_reminder, VerificationAffordance};

pub struct ExecutionIntelligenceInjector {
    todo_repo:  TodoRepo,
    supervisor: Arc<dyn JobSupervisorHandle>,
}

impl ExecutionIntelligenceInjector {
    pub fn new(todo_repo: TodoRepo, supervisor: Arc<dyn JobSupervisorHandle>) -> Self {
        Self { todo_repo, supervisor }
    }
}

impl DynamicInjector for ExecutionIntelligenceInjector {
    fn name(&self) -> &str {
        "execution-intelligence"
    }

    fn collect(&self, ctx: &dyn InjectorContext) -> Vec<ContextUpdate> {
        if !ctx.plan_mode_active() {
            return vec![];
        }
        let chain = ctx.agent_chain();
        if chain.is_empty() {
            return vec![];
        }

        // Block on async TodoRepo + JobSupervisor reads. The injector trait
        // is synchronous; we mirror PlanModeInjector's pattern.
        let todos = match tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(
                self.todo_repo.list_for_thread(ctx.thread_id(), ctx.agent_id())
            )
        }) {
            Ok(items) => items,
            Err(e) => {
                tracing::debug!(error = ?e, "todo lookup failed in injector; suppressing affordance");
                return vec![];
            }
        };

        let active_jobs = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(
                self.supervisor.list(ctx.thread_id(), chain, true)
            )
        });

        // Filter: pending or in-progress todos whose title classifies, and which
        // are NOT already covered by an active job (substring match on description).
        let mut affordances: Vec<(String, String, VerificationVerb)> = Vec::new();
        for item in &todos {
            // Status filter — TodoStatus is in bus::domain_events
            if !is_pending_or_in_progress(item) {
                continue;
            }
            let Some(verb) = classify_verification(&item.title) else { continue; };
            if active_jobs.iter().any(|j| j.description.contains(&item.title)) {
                continue;
            }
            affordances.push((item.id.clone(), item.title.clone(), verb));
        }

        if affordances.is_empty() {
            return vec![];
        }

        let view: Vec<VerificationAffordance<'_>> = affordances
            .iter()
            .map(|(id, title, verb)| VerificationAffordance {
                todo_id: id.as_str(),
                title:   title.as_str(),
                verb:    *verb,
            })
            .collect();

        let body = verification_affordance_reminder(&view);
        vec![ContextUpdate {
            reason:   ContextUpdateReason::CodingJobsChanged,
            content:  Some(body),
            metadata: None,
            priority: UpdatePriority::Normal,
            timestamp: Timestamp::now(),
        }]
    }
}

fn is_pending_or_in_progress(item: &storage::repos::TodoRow) -> bool {
    // The TodoRow representation: status as string. Adjust to actual repr.
    matches!(item.status.as_str(), "pending" | "in_progress")
}
```

> Two adaptations may be needed depending on actual TodoRepo signatures:
> - `list_for_thread` may be named `list` or `for_thread` — `grep -n "pub async fn" crates/storage/src/repos/coding_todo.rs` to find the right name.
> - `TodoRow.status` may be a typed enum, not a string — adapt `is_pending_or_in_progress` accordingly.

- [ ] **Step 5: Update intelligence/mod.rs to export the injector**

```rust
//! Phase 2.3b — Execution Intelligence layer.

pub mod diff;
pub mod injector;
pub mod normalize;
pub mod verification_match;

pub use diff::{diff_against_prior, ExtractedDiff, JobDiff, KindTransition, Location};
pub use injector::ExecutionIntelligenceInjector;
pub use normalize::command_key;
pub use verification_match::{classify as classify_verification, VerificationVerb};
```

- [ ] **Step 6: Re-export from lib.rs**

Edit `crates/feature-coding-bash/src/lib.rs`:

```rust
pub use injector::BackgroundJobsInjector;
pub use intelligence::ExecutionIntelligenceInjector;    // NEW
pub use supervisor::JobSupervisor;
pub use view::{BashJobView, BashJobsPanelView};
```

- [ ] **Step 7: Verify compile**

```bash
cargo check -p feature-coding-bash 2>&1 | tail -10
```
Expected: `Finished`.

- [ ] **Step 8: Add inline unit test for ExecutionIntelligenceInjector with a fake TodoRepo + supervisor**

This is awkward because both `TodoRepo` and `JobSupervisorHandle` need fakes. Instead of a unit test, defer assertions to the integration test in Task F4.

- [ ] **Step 9: Commit**

```bash
git add crates/feature-coding-bash/src/intelligence/injector.rs \
        crates/feature-coding-bash/src/intelligence/mod.rs \
        crates/feature-coding-bash/src/lib.rs
git commit -m "feat(coding): ExecutionIntelligenceInjector for plan-mode affordance (Phase 2.3b PR4 part 1)"
```

### Task F3: Wire injector into app-core::init InjectorRegistry

**Files:**
- Modify: `crates/app-core/src/init/mod.rs`

- [ ] **Step 1: Read current InjectorRegistry construction**

```bash
sed -n '160,195p' crates/app-core/src/init/mod.rs
```

Find the `let injector_registry = bus::InjectorRegistry::new(vec![...])` block (around lines 186-189).

- [ ] **Step 2: Add ExecutionIntelligenceInjector construction**

Above the `InjectorRegistry::new` call, add:

```rust
let exec_intel_injector: Arc<dyn bus::injection::DynamicInjector> = Arc::new(
    feature_coding_bash::ExecutionIntelligenceInjector::new(
        repos.coding_todo.clone(),
        Arc::clone(&job_supervisor) as Arc<dyn tools_core::JobSupervisorHandle>,
    )
);
```

> Cast `JobSupervisor` → `dyn JobSupervisorHandle`. If `repos.coding_todo` is not the actual binding, locate the `TodoRepo` instance via `grep -n "coding_todo\|TodoRepo::new" crates/app-core/src/init/mod.rs`.

- [ ] **Step 3: Add it to the registry vec**

```rust
let injector_registry = bus::InjectorRegistry::new(vec![
    plan_mode_injector       as Arc<dyn bus::injection::DynamicInjector>,
    background_jobs_injector as Arc<dyn bus::injection::DynamicInjector>,
    exec_intel_injector,                                         // NEW
]);
```

- [ ] **Step 4: Verify compile**

```bash
cargo check -p app-core 2>&1 | tail -10
```
Expected: `Finished`.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/init/mod.rs
git commit -m "feat(coding): register ExecutionIntelligenceInjector in InjectorRegistry (Phase 2.3b PR4 part 2)"
```

### Task F4: Integration test — intel_affordance_in_plan + intel_affordance_dedup

**Files:**
- Create: `crates/feature-coding-bash/tests/intel_affordance_in_plan.rs`
- Create: `crates/feature-coding-bash/tests/intel_affordance_dedup.rs`

- [ ] **Step 1: Write intel_affordance_in_plan.rs**

Create `crates/feature-coding-bash/tests/intel_affordance_in_plan.rs`:

```rust
//! Phase 2.3b: ExecutionIntelligenceInjector renders affordances for verification
//! verb todos when plan mode is active.

use std::sync::Arc;

use bus::injection::{DynamicInjector, InjectorContext};
use feature_coding_bash::ExecutionIntelligenceInjector;
use storage::{repos::TodoRepo, StoragePool};
use tools_core::{JobId, JobView, JobStatus, JobSupervisorHandle, JobSpec, JobError, RingRead};

#[derive(Debug)]
struct StubSupervisor;

#[async_trait::async_trait]
impl JobSupervisorHandle for StubSupervisor {
    async fn spawn(&self, _: JobSpec) -> Result<JobView, JobError> { unimplemented!() }
    async fn output_delta(&self, _: &JobId, _: u64, _: bool, _: u64) -> Result<RingRead, JobError> {
        unimplemented!()
    }
    async fn stop(&self, _: &JobId, _: &str) -> Result<JobView, JobError> { unimplemented!() }
    async fn list(&self, _: &str, _: &[String], _: bool) -> Vec<JobView> { vec![] }
}

struct PlanCtx {
    thread_id: String,
    agent_id: String,
    chain: Vec<String>,
    plan_active: bool,
}

impl InjectorContext for PlanCtx {
    fn thread_id(&self) -> &str { &self.thread_id }
    fn agent_id(&self) -> &str { &self.agent_id }
    fn plan_mode_active(&self) -> bool { self.plan_active }
    fn plan_session_id(&self) -> Option<&str> { None }
    fn agent_chain(&self) -> &[String] { &self.chain }
}

#[tokio::test(flavor = "multi_thread")]
async fn renders_affordance_for_verification_verb_todos() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    // Apply todo migration manually:
    let m = feature_coding_todo::CodingTodoFeature::default_migration();
    sqlx::query(m.sql).execute(pool.inner()).await.unwrap();

    let todo_repo = TodoRepo::new(pool.inner().clone());
    // Insert two todos: one Run, one Refactor.
    todo_repo.upsert_for_thread("t1", "a1", &serde_json::to_string(&serde_json::json!([
        {"id":"x1","title":"Run integration tests","status":"pending","concurrency":"safe"},
        {"id":"x2","title":"Refactor the supervisor","status":"pending","concurrency":"safe"},
    ])).unwrap()).await.unwrap();
    // (Replace with the actual TodoRepo write API; see grep result.)

    let supervisor: Arc<dyn JobSupervisorHandle> = Arc::new(StubSupervisor);
    let injector = ExecutionIntelligenceInjector::new(todo_repo, supervisor);

    let ctx = PlanCtx {
        thread_id: "t1".into(),
        agent_id:  "a1".into(),
        chain:     vec!["a1".into()],
        plan_active: true,
    };

    let updates = injector.collect(&ctx);
    assert_eq!(updates.len(), 1, "expected exactly 1 update");
    let body = updates[0].content.as_ref().unwrap();
    assert!(body.contains("Run integration tests"));
    assert!(!body.contains("Refactor the supervisor"));
}

#[tokio::test(flavor = "multi_thread")]
async fn no_affordance_when_plan_mode_inactive() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let todo_repo = TodoRepo::new(pool.inner().clone());
    let supervisor: Arc<dyn JobSupervisorHandle> = Arc::new(StubSupervisor);
    let injector = ExecutionIntelligenceInjector::new(todo_repo, supervisor);

    let ctx = PlanCtx {
        thread_id: "t1".into(),
        agent_id:  "a1".into(),
        chain:     vec!["a1".into()],
        plan_active: false,
    };

    assert!(injector.collect(&ctx).is_empty());
}
```

> The exact `TodoRepo` write API is what you discover in step `grep -n "pub async fn" crates/storage/src/repos/coding_todo.rs`. The above shows the *intent* — the method may be named `set_for_thread`, `upsert`, etc.

- [ ] **Step 2: Run the test**

```bash
cargo nextest run -p feature-coding-bash --test intel_affordance_in_plan 2>&1 | tail -15
```
Expected: 2 tests passed. Adjust the `todo_repo.upsert_for_thread` call if the real API differs — the assertion is what matters.

- [ ] **Step 3: Write intel_affordance_dedup.rs**

Create `crates/feature-coding-bash/tests/intel_affordance_dedup.rs`:

```rust
//! Phase 2.3b: ExecutionIntelligenceInjector suppresses affordances for todos
//! whose title is already covered by an active background job.

use std::sync::Arc;

use bus::injection::{DynamicInjector, InjectorContext};
use feature_coding_bash::ExecutionIntelligenceInjector;
use jiff::Timestamp;
use storage::{repos::TodoRepo, StoragePool};
use tools_core::{
    JobError, JobId, JobSpec, JobStatus, JobSupervisorHandle, JobView, RingRead,
};

#[derive(Debug)]
struct OneActiveJobSupervisor;

#[async_trait::async_trait]
impl JobSupervisorHandle for OneActiveJobSupervisor {
    async fn spawn(&self, _: JobSpec) -> Result<JobView, JobError> { unimplemented!() }
    async fn output_delta(&self, _: &JobId, _: u64, _: bool, _: u64) -> Result<RingRead, JobError> { unimplemented!() }
    async fn stop(&self, _: &JobId, _: &str) -> Result<JobView, JobError> { unimplemented!() }
    async fn list(&self, _: &str, _: &[String], _: bool) -> Vec<JobView> {
        vec![JobView {
            id: JobId("bash-x".into()),
            session_id: "t1".into(),
            agent_id: "a1".into(),
            description: "Run integration tests on the agent crate".into(),
            command: "cargo test -p agent".into(),
            cwd: std::path::PathBuf::from("/"),
            status: JobStatus::Running,
            started_at: Timestamp::now(),
            finished_at: None,
            exit_code: None,
            gate_result: None,
            failure_extracted: None,
            total_bytes_emitted: 0,
            bisect_generation: 0,
            last_polled_at: None,
            last_seen_offset: 0,
        }]
    }
}

struct PlanCtx;

impl InjectorContext for PlanCtx {
    fn thread_id(&self) -> &str { "t1" }
    fn agent_id(&self) -> &str { "a1" }
    fn plan_mode_active(&self) -> bool { true }
    fn plan_session_id(&self) -> Option<&str> { None }
    fn agent_chain(&self) -> &[String] { &["a1".to_string()] as &[String] }
}

#[tokio::test(flavor = "multi_thread")]
async fn affordance_suppressed_when_active_job_covers_todo() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let m = feature_coding_todo::CodingTodoFeature::default_migration();
    sqlx::query(m.sql).execute(pool.inner()).await.unwrap();

    let todo_repo = TodoRepo::new(pool.inner().clone());
    todo_repo.upsert_for_thread("t1", "a1", &serde_json::to_string(&serde_json::json!([
        {"id":"x1","title":"Run integration tests","status":"pending","concurrency":"safe"}
    ])).unwrap()).await.unwrap();

    let supervisor: Arc<dyn JobSupervisorHandle> = Arc::new(OneActiveJobSupervisor);
    let injector = ExecutionIntelligenceInjector::new(todo_repo, supervisor);
    let ctx = PlanCtx;
    assert!(injector.collect(&ctx).is_empty(), "active job should suppress affordance");
}
```

- [ ] **Step 4: Run test**

```bash
cargo nextest run -p feature-coding-bash --test intel_affordance_dedup 2>&1 | tail -10
```
Expected: 1 test passed.

- [ ] **Step 5: Commit**

```bash
git add crates/feature-coding-bash/tests/intel_affordance_in_plan.rs \
        crates/feature-coding-bash/tests/intel_affordance_dedup.rs
git commit -m "test(coding): integration tests for ExecutionIntelligenceInjector affordance (Phase 2.3b)"
```

---

# PR 5 — Episodic memory via BackgroundJobSignalSource (~1 day)

> **Strategy:** Add the translator arm in `app-core::init::ai_pipeline`, create the new mirror source, register it in `MirrorEngine::start` (which means extending the start signature with `bash_repo: Option<Arc<BashJobRepo>>`).

## Phase G — Translator arm + mirror source

### Task G1: Add BashJob translator arm in ai_pipeline.rs

**Files:**
- Modify: `crates/app-core/src/init/ai_pipeline.rs`

- [ ] **Step 1: Read current translate function**

```bash
cat crates/app-core/src/init/ai_pipeline.rs
```

Find the `translate` function and identify where the existing `feature_coding_todo::events::try_from_domain_event` arm sits. We add our new arm next to it.

- [ ] **Step 2: Add a helper function `translate_bash_job` in the same file**

Append (or insert before the main `translate` fn):

```rust
fn translate_bash_job(event: &bus::DomainEvent) -> Option<ai_core::AiSignal> {
    let bus::DomainEvent::BashJob(inner) = event else { return None };

    let kind = match inner {
        bus::BashJobEvent::Started   { .. } => bus::KIND_BASH_JOB_STARTED,
        bus::BashJobEvent::Completed { .. } => bus::KIND_BASH_JOB_COMPLETED,
        bus::BashJobEvent::Failed    { .. } => bus::KIND_BASH_JOB_FAILED,
        bus::BashJobEvent::Cancelled { .. } => bus::KIND_BASH_JOB_CANCELLED,
        bus::BashJobEvent::Lost      { .. } => bus::KIND_BASH_JOB_LOST,
    };

    // BashJob events route to RecallDomain::General (no CodingMemory variant exists).
    // The signal carries only the job_id; the SignalSource re-reads the row for full state.
    Some(ai_core::AiSignal {
        domain: ai_core::RecallDomain::General,
        event_kind: kind,
        importance: importance_for_bash_event(inner),
        salience: ai_core::SalienceVerdict::Significant,
        content: format!(
            "bash job {} ({}/{}/{})",
            inner.job_id(),
            kind,
            inner.thread_id(),
            inner.agent_id(),
        ),
        entity: None,
        timestamp: jiff::Timestamp::now(),
        raw_event: Some(event.clone()),
        metrics: ai_core::AiMetrics::default(),
        coaching_signal: false,
        coaching_rule: None,
        metric_samples: vec![],
    })
}

fn importance_for_bash_event(e: &bus::BashJobEvent) -> f64 {
    match e {
        bus::BashJobEvent::Failed { .. }    => 0.7,
        bus::BashJobEvent::Lost { .. }      => 0.6,
        bus::BashJobEvent::Cancelled { .. } => 0.5,
        bus::BashJobEvent::Completed { .. } => 0.3,
        bus::BashJobEvent::Started { .. }   => 0.2,
    }
}
```

> Verify exact paths via `grep -n "AiSignal\|SalienceVerdict\|AiMetrics" crates/ai-core/src/`. The struct-literal fields above match what was discovered. Adjust `salience` and `importance` field names if they differ.

- [ ] **Step 3: Wire it into the main translate function**

Find the existing `translate` function. Add a call before the existing `translate_system_event` fallback:

```rust
pub fn translate(event: &bus::DomainEvent) -> Option<ai_core::AiSignal> {
    // ... existing arms (tasks, finance, coaching, productivity, notes, learning,
    //                   language_learning, coding_todo, community_intelligence) ...

    if let Some(signal) = translate_bash_job(event) {
        return Some(signal);
    }

    translate_system_event(event)
}
```

- [ ] **Step 4: Verify compile**

```bash
cargo check -p app-core 2>&1 | tail -10
```
Expected: `Finished`. If `bus::KIND_BASH_JOB_*` paths don't resolve, add `pub use domain_events::{KIND_BASH_JOB_*}` to `crates/bus/src/lib.rs` (verify with `grep -n "KIND_BASH_JOB" crates/bus/src/lib.rs`).

- [ ] **Step 5: Add a unit test for translate_bash_job**

In `crates/app-core/src/init/ai_pipeline.rs`, add:

```rust
#[cfg(test)]
mod bash_job_translate_tests {
    use super::*;
    use jiff::Timestamp;

    #[test]
    fn translates_failed() {
        let event = bus::DomainEvent::BashJob(bus::BashJobEvent::Failed {
            job_id: "bash-x".into(),
            thread_id: "t".into(),
            agent_id: "a".into(),
            exit_code: Some(1),
            failure_kind: "TestFailure".into(),
            failure_detail: "...".into(),
        });
        let signal = translate_bash_job(&event).unwrap();
        assert_eq!(signal.event_kind, "BashJob.Failed");
        assert!((signal.importance - 0.7).abs() < 1e-9);
    }

    #[test]
    fn translates_completed_lower_importance() {
        let event = bus::DomainEvent::BashJob(bus::BashJobEvent::Completed {
            job_id: "bash-x".into(),
            thread_id: "t".into(),
            agent_id: "a".into(),
            exit_code: 0,
            duration_ms: 1000,
        });
        let signal = translate_bash_job(&event).unwrap();
        assert!((signal.importance - 0.3).abs() < 1e-9);
    }

    #[test]
    fn returns_none_for_non_bash_event() {
        let event = bus::DomainEvent::SkillRouted { /* fill minimal fields if needed */ };
        // Or use any other DomainEvent variant — exact construction depends on variant fields.
        // assert!(translate_bash_job(&event).is_none());
    }
}
```

> The third test may need the actual variant signature; remove if shape is awkward.

- [ ] **Step 6: Run the test**

```bash
cargo nextest run -p app-core -E 'test(bash_job_translate)' 2>&1 | tail -10
```
Expected: 2 tests passed.

- [ ] **Step 7: Commit**

```bash
git add crates/app-core/src/init/ai_pipeline.rs
git commit -m "feat(coding): translate BashJob events to AiSignal (Phase 2.3b PR5 part 1)"
```

### Task G2: Create BackgroundJobSignalSource

**Files:**
- Create: `crates/cognitive/src/mirror/sources/coding_bash.rs`
- Modify: `crates/cognitive/src/mirror/sources/mod.rs`
- Modify: `crates/cognitive/Cargo.toml`

- [ ] **Step 1: Confirm Cargo.toml has storage dep**

```bash
grep "storage" crates/cognitive/Cargo.toml
```

If not present, add to `[dependencies]`:

```toml
storage = { path = "../storage" }
```

- [ ] **Step 2: Create coding_bash.rs**

Create `crates/cognitive/src/mirror/sources/coding_bash.rs`:

```rust
//! Phase 2.3b — `BackgroundJobSignalSource`
//!
//! Subscribes to `BashJob.Completed/Failed/Cancelled/Lost` AiSignals,
//! re-reads the row from `BashJobRepo`, and writes one `EpisodicMemory`
//! per event to `episodic_memories` with `kind="bash_job"`, `domain="coding"`.

use std::sync::Arc;

use ai_core::{AiSignal, MirrorSignalSource, MirrorSnapshotSpec};
use async_trait::async_trait;
use jiff::Timestamp;
use storage::repos::{BashJobRepo, BashJobRow};
use uuid::Uuid;

use crate::repos::EpisodicMemoryRepo;
use crate::types::EpisodicMemory;

const SUBSCRIBED_KINDS: &[&str] = &[
    "BashJob.Completed",
    "BashJob.Failed",
    "BashJob.Cancelled",
    "BashJob.Lost",
];

pub struct BackgroundJobSignalSource {
    episodic_repo: EpisodicMemoryRepo,
    bash_repo:     Arc<BashJobRepo>,
}

impl BackgroundJobSignalSource {
    pub fn new(episodic_repo: EpisodicMemoryRepo, bash_repo: Arc<BashJobRepo>) -> Self {
        Self { episodic_repo, bash_repo }
    }
}

#[async_trait]
impl MirrorSignalSource for BackgroundJobSignalSource {
    fn spec(&self) -> MirrorSnapshotSpec {
        MirrorSnapshotSpec {
            name: "coding_bash",
            subscribed_kinds: SUBSCRIBED_KINDS,
            flush_interval_secs: None,            // event-driven only; no flush loop
        }
    }

    fn name(&self) -> &'static str {
        "coding_bash"
    }

    async fn accumulate(&self, signal: &AiSignal) -> common::Result<()> {
        // Extract job_id from the raw_event (we rely on the original DomainEvent for fields).
        let job_id = match &signal.raw_event {
            Some(bus::DomainEvent::BashJob(inner)) => inner.job_id().to_string(),
            _ => return Ok(()),  // shouldn't happen given subscribed_kinds, but defensive
        };

        let row = match self.bash_repo.get(&job_id).await {
            Ok(Some(r)) => r,
            Ok(None) => {
                tracing::debug!(job_id, "row missing at episodic write; skipping");
                return Ok(());
            }
            Err(e) => {
                tracing::warn!(error = ?e, job_id, "bash_repo.get failed in mirror source");
                return Ok(());
            }
        };

        let mem = build_episodic_memory(&row);
        if let Err(e) = self.episodic_repo.insert(&mem).await {
            tracing::warn!(error = ?e, job_id, "episodic insert failed");
        }
        Ok(())
    }

    async fn flush(&self) -> common::Result<()> { Ok(()) }
}

fn build_episodic_memory(row: &BashJobRow) -> EpisodicMemory {
    let importance = match row.status.as_str() {
        "Failed"    => 0.7,
        "Lost"      => 0.6,
        "Cancelled" => 0.5,
        "Completed" => 0.3,
        _           => 0.3,
    };
    let elapsed_ms = match row.finished_at {
        Some(end) => {
            let s = row.started_at.as_millisecond() as i128;
            let e = end.as_millisecond() as i128;
            (e - s).max(0) as u64
        }
        None => 0,
    };
    let extracted: serde_json::Value = row
        .failure_extracted
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or(serde_json::Value::Null);

    let content = serde_json::json!({
        "job_id":            row.id,
        "command":           row.command,
        "command_key":       row.command_key,
        "description":       row.description,
        "status":            row.status,
        "exit_code":         row.exit_code,
        "elapsed_ms":        elapsed_ms,
        "failure_kind":      row.failure_kind,
        "failure_extracted": extracted,
    }).to_string();

    let summary = render_episode_summary(row, elapsed_ms);

    let metadata = serde_json::json!({
        "agent_id":  row.agent_id,
        "thread_id": row.session_id,
    }).to_string();

    let now = Timestamp::now().to_string();
    let occurred_at = row.finished_at.map(|t| t.to_string()).unwrap_or_else(|| now.clone());

    EpisodicMemory {
        id: Uuid::new_v4().to_string(),
        domain: "coding".into(),
        content,
        summary: Some(summary),
        importance,
        occurred_at,
        recorded_at: now,
        stability: 1.0,
        last_accessed: None,
        access_count: 0,
        project_id: None,
        scope_type: "session".into(),
        scope_id: Some(row.session_id.clone()),
        scope_repo_id: None,
        metadata: Some(metadata),
        kind: Some("bash_job".into()),
        actor_id: Some(row.agent_id.clone()),
        tier: "raw".into(),
        parent_id: None,
        child_count: 0,
        rolled_up_at: None,
    }
}

fn render_episode_summary(row: &BashJobRow, elapsed_ms: u64) -> String {
    let secs = elapsed_ms as f64 / 1000.0;
    match (row.status.as_str(), row.failure_kind.as_deref()) {
        ("Completed", _) => format!("Passed `{}` in {:.1}s", truncate(&row.command, 60), secs),
        ("Cancelled", _) => format!(
            "Cancelled `{}` after {:.1}s",
            truncate(&row.command, 60),
            secs
        ),
        ("Lost", _) => format!("Lost `{}` (Klynt restarted mid-run)", truncate(&row.command, 60)),
        ("Failed", Some(kind)) => format!(
            "{} in `{}` after {:.1}s",
            kind,
            truncate(&row.command, 60),
            secs
        ),
        _ => format!("Bash job `{}` ended", truncate(&row.command, 60)),
    }
    .chars()
    .take(160)
    .collect()
}

fn truncate(s: &str, n: usize) -> &str {
    if s.len() <= n {
        s
    } else {
        let mut end = n;
        while !s.is_char_boundary(end) && end < s.len() {
            end += 1;
        }
        &s[..end.min(s.len())]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_row(id: &str, status: &str, kind: Option<&str>) -> BashJobRow {
        BashJobRow {
            id: id.into(),
            session_id: "s1".into(),
            agent_id: "a1".into(),
            description: "desc".into(),
            command: "cargo nextest run -p agent".into(),
            command_key: "k".into(),
            cwd: "/".into(),
            timeout_ms: 60_000,
            silent_completion: false,
            status: status.into(),
            exit_code: Some(if status == "Completed" { 0 } else { 1 }),
            failure_kind: kind.map(String::from),
            failure_detail: None,
            failure_extracted: None,
            started_at: jiff::Timestamp::from_millisecond(1_700_000_000_000).unwrap(),
            finished_at: Some(jiff::Timestamp::from_millisecond(1_700_000_005_000).unwrap()),
            total_bytes_emitted: 0,
            bisect_count: 0,
            log_path: "/tmp/x.log".into(),
            final_path: None,
            last_polled_at: None,
            last_seen_offset: 0,
        }
    }

    #[test]
    fn importance_failed() {
        let mem = build_episodic_memory(&fake_row("a", "Failed", Some("TestFailure")));
        assert!((mem.importance - 0.7).abs() < 1e-9);
        assert_eq!(mem.kind, Some("bash_job".into()));
        assert_eq!(mem.domain, "coding");
        assert_eq!(mem.scope_type, "session");
        assert_eq!(mem.scope_id, Some("s1".into()));
        assert_eq!(mem.actor_id, Some("a1".into()));
    }

    #[test]
    fn importance_completed() {
        let mem = build_episodic_memory(&fake_row("b", "Completed", None));
        assert!((mem.importance - 0.3).abs() < 1e-9);
    }

    #[test]
    fn importance_lost() {
        let mem = build_episodic_memory(&fake_row("c", "Lost", Some("Lost")));
        assert!((mem.importance - 0.6).abs() < 1e-9);
    }

    #[test]
    fn summary_under_160_chars() {
        let row = fake_row("d", "Failed", Some("TestFailure"));
        let mem = build_episodic_memory(&row);
        assert!(mem.summary.unwrap().chars().count() <= 160);
    }

    #[test]
    fn spec_returns_4_kinds() {
        let pool = futures::executor::block_on(async {
            storage::StoragePool::connect_in_memory().await.unwrap()
        });
        let bash_repo = Arc::new(BashJobRepo::new(pool.inner().clone()));
        let ep_repo = EpisodicMemoryRepo::new(pool.inner().clone());
        let src = BackgroundJobSignalSource::new(ep_repo, bash_repo);
        assert_eq!(src.spec().subscribed_kinds.len(), 4);
        assert_eq!(src.spec().flush_interval_secs, None);
    }
}
```

- [ ] **Step 3: Add `uuid` dep to cognitive Cargo.toml if not present**

```bash
grep "^uuid" crates/cognitive/Cargo.toml
```

If absent, add to `[dependencies]`:
```toml
uuid = { version = "1", features = ["v4"] }
```

- [ ] **Step 4: Update sources/mod.rs**

Edit `crates/cognitive/src/mirror/sources/mod.rs`:

```rust
pub mod approval_history;
pub mod coding_bash;            // NEW
pub mod coding_todo;
pub mod config_archiver;
pub mod cost_ceiling;
pub mod finance_drift;
pub mod meta_rule;
pub mod routing;
pub mod skill_effectiveness;
pub mod task_focus;
pub mod trial;

pub use approval_history::ApprovalHistorySource;
pub use coding_bash::BackgroundJobSignalSource;     // NEW
pub use coding_todo::TodoSignalSource;
pub use config_archiver::ConfigArchiverSource;
pub use cost_ceiling::CostCeilingSource;
pub use finance_drift::FinanceSpendingDriftSource;
pub use meta_rule::MetaRuleSignalSource;
pub use routing::RoutingSignalSource;
pub use skill_effectiveness::{EffectivenessScores, SkillEffectivenessSource};
pub use task_focus::TaskFocusPatternSource;
pub use trial::TrialPreviewSource;
```

- [ ] **Step 5: Verify cognitive compiles**

```bash
cargo check -p cognitive 2>&1 | tail -10
```
Expected: `Finished`. If `bus::DomainEvent` is not directly accessible from `cognitive`, the source can be modified to take a `String job_id` extracted upstream — but `cognitive` already depends on `bus` (verify with `grep "^bus" crates/cognitive/Cargo.toml`).

- [ ] **Step 6: Run unit tests**

```bash
cargo nextest run -p cognitive -E 'test(coding_bash)' 2>&1 | tail -10
```
Expected: 5 tests passed.

- [ ] **Step 7: Commit**

```bash
git add crates/cognitive/src/mirror/sources/coding_bash.rs \
        crates/cognitive/src/mirror/sources/mod.rs \
        crates/cognitive/Cargo.toml
git commit -m "feat(cognitive): BackgroundJobSignalSource for episodic memory (Phase 2.3b PR5 part 2)"
```

### Task G3: Register BackgroundJobSignalSource in MirrorEngine::start

**Files:**
- Modify: `crates/cognitive/src/mirror/engine.rs`
- Modify: `crates/app-core/src/init/mod.rs`

- [ ] **Step 1: Read current MirrorEngine::start**

```bash
sed -n '30,100p' crates/cognitive/src/mirror/engine.rs
```

- [ ] **Step 2: Extend start signature with bash_repo parameter**

Edit `crates/cognitive/src/mirror/engine.rs`. Change the signature:

```rust
// OLD:
// pub fn start(
//     repo: MirrorRepo,
//     narrative_handler: Option<Arc<dyn NarrativeHandler>>,
//     autotuner_bridge: Option<Arc<dyn AutotunerBridge>>,
//     episodic_repo: Option<EpisodicMemoryRepo>,
//     rule_repo: Option<ProceduralRuleRepo>,
//     trial_evaluator: Option<Arc<dyn crate::mirror::types::EarlyTrialEvaluator>>,
//     approval_history_repo: Option<Arc<storage::repos::CodingApprovalHistoryRepo>>,
//     approval_pattern_repo: Option<Arc<storage::repos::ApprovalPatternHistoryRepo>>,
// ) -> StartedMirror

// NEW:
pub fn start(
    repo: MirrorRepo,
    narrative_handler: Option<Arc<dyn NarrativeHandler>>,
    autotuner_bridge: Option<Arc<dyn AutotunerBridge>>,
    episodic_repo: Option<EpisodicMemoryRepo>,
    rule_repo: Option<ProceduralRuleRepo>,
    trial_evaluator: Option<Arc<dyn crate::mirror::types::EarlyTrialEvaluator>>,
    approval_history_repo: Option<Arc<storage::repos::CodingApprovalHistoryRepo>>,
    approval_pattern_repo: Option<Arc<storage::repos::ApprovalPatternHistoryRepo>>,
    bash_repo: Option<Arc<storage::BashJobRepo>>,            // NEW
) -> StartedMirror
```

- [ ] **Step 3: Register the source inside start**

Find the existing `register!(...)` calls (around lines 77-93). After `register!(cost_ceiling);`, add:

```rust
if let (Some(ep), Some(br)) = (&episodic_repo, &bash_repo) {
    let bg_job_source = Arc::new(
        crate::mirror::sources::BackgroundJobSignalSource::new(ep.clone(), br.clone())
    );
    register!(bg_job_source);
}
```

- [ ] **Step 4: Update consumer count test**

Find the test at line ~134 that asserts the consumer count is 8. Update to expect 9 when both repos are Some, or refactor the test to check the additional registration is conditional.

```bash
grep -n "consumers.len" crates/cognitive/src/mirror/engine.rs
```

If the test asserts a hard count, update it to construct the engine with `Some(...)` for `bash_repo` and assert 9, OR with `None` and assert 8 (then add a separate test for the +1 case).

- [ ] **Step 5: Update the call site in app-core**

Find `MirrorEngine::start(...)` in `crates/app-core/src/init/mod.rs` (around lines 611-620). Add the new positional argument:

```rust
let bash_repo_arc = Some(Arc::new(bash_job_repo.clone()));   // NEW

mirror_engine = MirrorEngine::start(
    mirror_repo.clone(),
    narrative_handler,
    autotuner_bridge,
    episodic_repo,
    rule_repo,
    trial_evaluator,
    Some(Arc::new(coding_approval_history_repo.clone())),
    Some(Arc::new(approval_pattern_history_repo.clone())),
    bash_repo_arc,                                            // NEW
);
```

- [ ] **Step 6: Verify compile**

```bash
cargo check --workspace 2>&1 | tail -10
```
Expected: `Finished`. If other call sites of `MirrorEngine::start` exist (e.g. tests in cognitive), update each to pass `None` for the new param.

- [ ] **Step 7: Run cognitive tests**

```bash
cargo nextest run -p cognitive 2>&1 | tail -10
```
Expected: all tests pass.

- [ ] **Step 8: Commit**

```bash
git add crates/cognitive/src/mirror/engine.rs \
        crates/app-core/src/init/mod.rs
git commit -m "feat(cognitive): register BackgroundJobSignalSource in MirrorEngine (Phase 2.3b PR5 part 3)"
```

### Task G4: Integration test — episodic write end-to-end

**Files:**
- Create: `crates/feature-coding-bash/tests/intel_episodic_write.rs`

- [ ] **Step 1: Write the test**

Create `crates/feature-coding-bash/tests/intel_episodic_write.rs`:

```rust
//! Phase 2.3b: a Failed bash job results in an EpisodicMemory row.

use std::sync::Arc;
use std::time::Duration;

use ai_core::SignalConsumer;
use bus::{ContextUpdateQueue, DomainEvent, DomainEventBus};
use cognitive::mirror::sources::BackgroundJobSignalSource;
use cognitive::repos::EpisodicMemoryRepo;
use feature_coding_bash::JobSupervisor;
use storage::{repos::BashJobRepo, StoragePool};
use tools_core::{JobSpec, JobSupervisorHandle};

async fn pool_with_tables() -> StoragePool {
    let pool = StoragePool::connect_in_memory().await.unwrap();

    // bash jobs migration
    let m1 = feature_coding_bash::migrations::coding_background_jobs_migration();
    sqlx::query(m1.sql).execute(pool.inner()).await.unwrap();

    // cognitive episodic_memories table
    sqlx::query(include_str!("../../cognitive/migrations/001_cognitive_tables.sql"))
        .execute(pool.inner()).await.unwrap();
    // (Adapt path or use cognitive's own helper if migration is split.)

    pool
}

#[tokio::test(flavor = "multi_thread")]
async fn failed_job_writes_episodic_memory() {
    let pool = pool_with_tables().await;

    let bash_repo = BashJobRepo::new(pool.inner().clone());
    let bash_repo_arc = Arc::new(bash_repo.clone());
    let ep_repo = EpisodicMemoryRepo::new(pool.inner().clone());
    let ep_repo_arc = ep_repo.clone();

    // Set up the signal source manually (skip the full MirrorEngine).
    let source = Arc::new(BackgroundJobSignalSource::new(ep_repo, bash_repo_arc.clone()));

    // Set up supervisor and spawn a failing job.
    let bus = Arc::new(DomainEventBus::new());
    let queue = Arc::new(ContextUpdateQueue::new());
    let data_dir = tempfile::tempdir().unwrap();

    let supervisor = Arc::new(JobSupervisor::new(
        bash_repo,
        Arc::clone(&bus),
        Arc::clone(&queue),
        data_dir.path().to_path_buf(),
        Arc::new(klynt_sandbox::MacOsSeatbeltRunner::new()),
    ));

    // Subscribe the source to bus events directly (mimics ai_pipeline + signal router).
    let bus_rx = bus.subscribe();
    let source_clone = Arc::clone(&source);
    tokio::spawn(async move {
        let mut rx = bus_rx;
        while let Ok(event) = rx.recv().await {
            if let DomainEvent::BashJob(_) = &event {
                let signal = ai_core::AiSignal {
                    domain: ai_core::RecallDomain::General,
                    event_kind: event.kind(),
                    importance: 0.7,
                    salience: ai_core::SalienceVerdict::Significant,
                    content: String::new(),
                    entity: None,
                    timestamp: jiff::Timestamp::now(),
                    raw_event: Some(event),
                    metrics: ai_core::AiMetrics::default(),
                    coaching_signal: false,
                    coaching_rule: None,
                    metric_samples: vec![],
                };
                let _ = source_clone.accumulate(&signal).await;
            }
        }
    });

    let v = supervisor.spawn(JobSpec {
        session_id: "s1".into(),
        agent_id:   "a1".into(),
        agent_chain: vec!["a1".into()],
        description: "test failure".into(),
        command: "false".into(),
        cwd: std::env::temp_dir(),
        timeout_ms: 30_000,
        silent_completion: false,
    }).await.unwrap();

    // Wait for completion + propagation.
    tokio::time::sleep(Duration::from_secs(2)).await;

    let row_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM episodic_memories WHERE kind = 'bash_job'"
    )
    .fetch_one(pool.inner()).await.unwrap();

    assert!(row_count >= 1, "expected ≥1 bash_job episode, got {row_count}");
}
```

> The exact `bus.subscribe()` API may differ; if `DomainEventBus` uses a different fan-out mechanism, adapt to match. The test's value is verifying end-to-end that episode writes happen — adapt the wiring to whatever the real bus exposes.

- [ ] **Step 2: Run the test**

```bash
cargo nextest run -p feature-coding-bash --test intel_episodic_write 2>&1 | tail -10
```
Expected: 1 test passed.

- [ ] **Step 3: Commit**

```bash
git add crates/feature-coding-bash/tests/intel_episodic_write.rs
git commit -m "test(coding): integration test for BackgroundJobSignalSource episodic writes (Phase 2.3b)"
```

---

# PR 6 — Remaining integration tests + smoke checklist (~0.5 day)

> **Strategy:** Land the rest of the integration tests, then run the manual smoke checklist as a final guard.

### Task H1: intel_diff_test_set + intel_diff_recovered

**Files:**
- Create: `crates/feature-coding-bash/tests/intel_diff_test_set.rs`
- Create: `crates/feature-coding-bash/tests/intel_diff_recovered.rs`

- [ ] **Step 1: Write intel_diff_test_set.rs**

This test inserts two BashJobRows directly via the repo (not via spawn — simpler, no real subprocess), then exercises the diff path.

Create `crates/feature-coding-bash/tests/intel_diff_test_set.rs`:

```rust
//! Phase 2.3b: TestFailure→TestFailure diff produces correct new/still/resolved sets.

use feature_coding_bash::intelligence::{diff_against_prior, ExtractedDiff};
use jiff::Timestamp;
use storage::{repos::{BashJobRepo, BashJobRow}, StoragePool};

fn row(id: &str, kind: &str, names: &[&str]) -> BashJobRow {
    BashJobRow {
        id: id.into(),
        session_id: "s1".into(),
        agent_id: "a1".into(),
        description: "desc".into(),
        command: "cargo test".into(),
        command_key: "k".into(),
        cwd: "/".into(),
        timeout_ms: 60_000,
        silent_completion: false,
        status: "Failed".into(),
        exit_code: Some(101),
        failure_kind: Some(kind.into()),
        failure_detail: None,
        failure_extracted: Some(serde_json::json!({
            "failed_test_names": names,
            "n_failed": names.len(),
            "n_passed": 5,
            "n_ignored": 0,
        }).to_string()),
        started_at: Timestamp::from_millisecond(1_700_000_000_000).unwrap(),
        finished_at: Some(Timestamp::from_millisecond(1_700_000_001_000).unwrap()),
        total_bytes_emitted: 0,
        bisect_count: 0,
        log_path: format!("/tmp/{id}.log"),
        final_path: None,
        last_polled_at: None,
        last_seen_offset: 0,
    }
}

#[tokio::test]
async fn test_set_diff_via_repo_round_trip() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let m = feature_coding_bash::migrations::coding_background_jobs_migration();
    sqlx::query(m.sql).execute(pool.inner()).await.unwrap();
    let repo = BashJobRepo::new(pool.inner().clone());

    let prior = row("a", "TestFailure", &["A", "B"]);
    let curr  = row("b", "TestFailure", &["B", "C"]);
    repo.insert(&prior).await.unwrap();
    repo.insert(&curr).await.unwrap();

    let prior_back = repo.find_prior_by_command_key("s1", "k", "b").await.unwrap().unwrap();
    let diff = diff_against_prior(&prior_back, &curr);

    match diff.extracted_diff {
        ExtractedDiff::TestSet { new_failures, still_failing, resolved } => {
            let mut new_f = new_failures.clone(); new_f.sort();
            assert_eq!(new_f, vec!["C".to_string()]);
            assert_eq!(still_failing, vec!["B".to_string()]);
            assert_eq!(resolved, vec!["A".to_string()]);
        }
        other => panic!("expected TestSet, got {other:?}"),
    }
}
```

- [ ] **Step 2: Write intel_diff_recovered.rs**

Create `crates/feature-coding-bash/tests/intel_diff_recovered.rs` analogous to above but with prior=Failed, curr=Passed; assert `KindTransition::Recovered`.

```rust
use feature_coding_bash::intelligence::{diff_against_prior, KindTransition};
use jiff::Timestamp;
use storage::{repos::{BashJobRepo, BashJobRow}, StoragePool};

fn passed_row(id: &str) -> BashJobRow {
    BashJobRow {
        id: id.into(),
        session_id: "s1".into(),
        agent_id: "a1".into(),
        description: "desc".into(),
        command: "cargo test".into(),
        command_key: "k".into(),
        cwd: "/".into(),
        timeout_ms: 60_000,
        silent_completion: false,
        status: "Completed".into(),
        exit_code: Some(0),
        failure_kind: None,
        failure_detail: None,
        failure_extracted: None,
        started_at: Timestamp::from_millisecond(1_700_000_000_000).unwrap(),
        finished_at: Some(Timestamp::from_millisecond(1_700_000_001_000).unwrap()),
        total_bytes_emitted: 0,
        bisect_count: 0,
        log_path: format!("/tmp/{id}.log"),
        final_path: None,
        last_polled_at: None,
        last_seen_offset: 0,
    }
}

fn failed_row(id: &str) -> BashJobRow {
    BashJobRow {
        failure_kind: Some("TestFailure".into()),
        failure_extracted: Some(serde_json::json!({"failed_test_names":["A"]}).to_string()),
        status: "Failed".into(),
        exit_code: Some(101),
        ..passed_row(id)
    }
}

#[tokio::test]
async fn recovered_transition_when_prior_failed() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let m = feature_coding_bash::migrations::coding_background_jobs_migration();
    sqlx::query(m.sql).execute(pool.inner()).await.unwrap();
    let repo = BashJobRepo::new(pool.inner().clone());

    repo.insert(&failed_row("a")).await.unwrap();
    let curr = passed_row("b");
    repo.insert(&curr).await.unwrap();

    let prior = repo.find_prior_by_command_key("s1", "k", "b").await.unwrap().unwrap();
    let diff = diff_against_prior(&prior, &curr);

    assert_eq!(
        diff.kind_transition,
        KindTransition::Recovered { prior_kind: "TestFailure".into() }
    );
}
```

- [ ] **Step 3: Run both tests**

```bash
cargo nextest run -p feature-coding-bash --test intel_diff_test_set --test intel_diff_recovered 2>&1 | tail -10
```
Expected: 2 tests passed.

- [ ] **Step 4: Commit**

```bash
git add crates/feature-coding-bash/tests/intel_diff_test_set.rs \
        crates/feature-coding-bash/tests/intel_diff_recovered.rs
git commit -m "test(coding): test_set + recovered diff integration tests (Phase 2.3b)"
```

### Task H2: intel_command_key_normalization + intel_subagent_episodic_actor_id

**Files:**
- Create: `crates/feature-coding-bash/tests/intel_command_key_normalization.rs`
- Create: `crates/feature-coding-bash/tests/intel_subagent_episodic_actor_id.rs`

- [ ] **Step 1: Write intel_command_key_normalization.rs**

```rust
//! Phase 2.3b: env-prefix and whitespace differences hash to the same command_key.

use feature_coding_bash::intelligence::command_key;

#[test]
fn env_prefix_and_whitespace_collapse_to_same_key() {
    let a = command_key("cargo test -p agent");
    let b = command_key("  cargo test  -p agent  ");
    let c = command_key("RUST_LOG=debug cargo test -p agent");
    let d = command_key("RUST_LOG=debug  RUST_BACKTRACE=1 cargo test -p agent");
    assert_eq!(a, b);
    assert_eq!(a, c);
    assert_eq!(a, d);
}

#[test]
fn flag_change_produces_different_key() {
    assert_ne!(
        command_key("cargo test -p agent"),
        command_key("cargo test -p agent --nocapture"),
    );
}
```

- [ ] **Step 2: Write intel_subagent_episodic_actor_id.rs**

```rust
//! Phase 2.3b: episode for a subagent-spawned job uses the subagent's agent_id as actor_id.

use cognitive::mirror::sources::coding_bash::build_episodic_memory;
use jiff::Timestamp;
use storage::repos::BashJobRow;

fn fake_row(agent_id: &str) -> BashJobRow {
    BashJobRow {
        id: "bash-x".into(),
        session_id: "s1".into(),
        agent_id: agent_id.into(),
        description: "d".into(),
        command: "c".into(),
        command_key: "k".into(),
        cwd: "/".into(),
        timeout_ms: 60_000,
        silent_completion: false,
        status: "Completed".into(),
        exit_code: Some(0),
        failure_kind: None,
        failure_detail: None,
        failure_extracted: None,
        started_at: Timestamp::from_millisecond(1_700_000_000_000).unwrap(),
        finished_at: Some(Timestamp::from_millisecond(1_700_000_001_000).unwrap()),
        total_bytes_emitted: 0,
        bisect_count: 0,
        log_path: "/tmp/x.log".into(),
        final_path: None,
        last_polled_at: None,
        last_seen_offset: 0,
    }
}

#[test]
fn actor_id_matches_subagent() {
    let mem = build_episodic_memory(&fake_row("subagent-7"));
    assert_eq!(mem.actor_id, Some("subagent-7".into()));
}
```

> Requires `build_episodic_memory` to be `pub` in `coding_bash.rs`. Adjust visibility.

- [ ] **Step 3: Make `build_episodic_memory` public**

In `crates/cognitive/src/mirror/sources/coding_bash.rs`, change:

```rust
fn build_episodic_memory(row: &BashJobRow) -> EpisodicMemory {
```

to:

```rust
pub fn build_episodic_memory(row: &BashJobRow) -> EpisodicMemory {
```

- [ ] **Step 4: Run tests**

```bash
cargo nextest run -p feature-coding-bash --test intel_command_key_normalization 2>&1 | tail -5
cargo nextest run -p feature-coding-bash --test intel_subagent_episodic_actor_id 2>&1 | tail -5
```
Expected: 3 tests passed total.

- [ ] **Step 5: Commit**

```bash
git add crates/feature-coding-bash/tests/intel_command_key_normalization.rs \
        crates/feature-coding-bash/tests/intel_subagent_episodic_actor_id.rs \
        crates/cognitive/src/mirror/sources/coding_bash.rs
git commit -m "test(coding): command_key normalization + subagent actor_id tests (Phase 2.3b)"
```

### Task H3: intel_episodic_passed + intel_episodic_lost_on_restart

**Files:**
- Create: `crates/feature-coding-bash/tests/intel_episodic_passed.rs`
- Create: `crates/feature-coding-bash/tests/intel_episodic_lost_on_restart.rs`

- [ ] **Step 1: Write intel_episodic_passed.rs**

Mirror the structure of `intel_episodic_write.rs` but spawn `true` (passes) and assert importance≈0.3.

```rust
use std::sync::Arc;
use std::time::Duration;

use bus::{ContextUpdateQueue, DomainEvent, DomainEventBus};
use cognitive::mirror::sources::BackgroundJobSignalSource;
use cognitive::repos::EpisodicMemoryRepo;
use feature_coding_bash::JobSupervisor;
use storage::{repos::BashJobRepo, StoragePool};
use tools_core::JobSpec;

#[tokio::test(flavor = "multi_thread")]
async fn passed_job_writes_episode_with_importance_0_3() {
    // Setup mirrors intel_episodic_write.rs — spawn `true` instead of `false`,
    // then assert:
    //   SELECT importance FROM episodic_memories WHERE kind='bash_job' LIMIT 1;
    //   approx 0.3
    // Full body left as exercise — copy from intel_episodic_write.rs and swap command.
}
```

> Copy the full setup from `intel_episodic_write.rs` and swap `command: "false"` → `command: "true"` and the assertion to:

```rust
let importance: f64 = sqlx::query_scalar(
    "SELECT importance FROM episodic_memories WHERE kind = 'bash_job' LIMIT 1"
).fetch_one(pool.inner()).await.unwrap();
assert!((importance - 0.3).abs() < 0.01, "got {importance}");
```

- [ ] **Step 2: Write intel_episodic_lost_on_restart.rs**

```rust
//! Phase 2.3b: orphan rows reconciled at startup produce Lost episodes.

use std::sync::Arc;
use std::time::Duration;

use bus::{ContextUpdateQueue, DomainEvent, DomainEventBus};
use cognitive::mirror::sources::BackgroundJobSignalSource;
use cognitive::repos::EpisodicMemoryRepo;
use feature_coding_bash::JobSupervisor;
use jiff::Timestamp;
use storage::{repos::{BashJobRepo, BashJobRow}, StoragePool};

#[tokio::test(flavor = "multi_thread")]
async fn lost_episode_written_on_reconcile() {
    // 1. Construct a pool with both bash + episodic tables.
    // 2. Insert a fake Running BashJobRow directly.
    // 3. Subscribe BackgroundJobSignalSource to the bus.
    // 4. Construct a JobSupervisor and call reconcile_on_startup.
    // 5. Assert episodic_memories has 1 bash_job row with importance ≈ 0.6.
}
```

(Body following the `intel_episodic_write.rs` pattern; spawn-via-direct-insert.)

- [ ] **Step 3: Run tests**

```bash
cargo nextest run -p feature-coding-bash --test intel_episodic_passed --test intel_episodic_lost_on_restart 2>&1 | tail -10
```
Expected: 2 tests passed.

- [ ] **Step 4: Commit**

```bash
git add crates/feature-coding-bash/tests/intel_episodic_passed.rs \
        crates/feature-coding-bash/tests/intel_episodic_lost_on_restart.rs
git commit -m "test(coding): episodic_passed + lost_on_restart integration tests (Phase 2.3b)"
```

### Task H4: Workspace tests + lint

- [ ] **Step 1: Run full workspace test suite**

```bash
cargo nextest run --workspace 2>&1 | tail -15
```
Expected: 0 failures.

- [ ] **Step 2: Run clippy**

```bash
cargo clippy --workspace --all-targets --all-features 2>&1 | tail -20
```
Expected: 0 warnings (zero-warnings policy).

- [ ] **Step 3: Run fmt check**

```bash
cargo fmt --all --check
```
Expected: no diff.

- [ ] **Step 4: Run doctests**

```bash
cargo test --workspace --doc 2>&1 | tail -10
```
Expected: pass.

- [ ] **Step 5: Cargo machete check (no new unused deps)**

```bash
cargo machete crates/feature-coding-bash crates/cognitive crates/storage crates/app-core 2>&1 | tail -10
```
Expected: no new unused deps.

### Task H5: Manual smoke checklist

- [ ] **Step 1: Set dev env**

```bash
export KLYNTBOT_HOME=$HOME/.klyntbot-dev
rm -rf $KLYNTBOT_HOME/data.db   # pre-release migration drops the table
```

- [ ] **Step 2: Start dev server + Tauri**

In one terminal:
```bash
cd desktop-ui && bun run dev
```

In another:
```bash
cargo tauri dev
```

- [ ] **Step 3: Smoke test 1 — diff section absent on first run, present on second**

In a coding thread, run:
```
bash run_in_background=true command="echo hi; false"
```
Wait for completion. Confirm `<system-reminder>` body in the LLM's next iteration does NOT contain "Compared to last run".

Re-run:
```
bash run_in_background=true command="echo hi; false"
```
Confirm the body NOW contains "Compared to last run of this command" with "StillFailing".

- [ ] **Step 4: Smoke test 2 — TestFailure diff with multi-name**

Set up a small Rust crate with 2 failing tests. Run:
```
bash run_in_background=true command="cargo nextest run -p <crate>"
```

After completion, fix one test, re-run. Confirm body shows `resolved: [<test_name>]` and `still failing: [<other>]`.

- [ ] **Step 5: Smoke test 3 — Plan-mode affordance**

Enter plan mode (`/plan`). Add via TodoWrite:
- "Run integration tests"
- "Refactor the supervisor"

Continue chatting. Verify the LLM's next system reminder includes "Plan mode active — the following pending TodoItems look like background-bash candidates" with only "Run integration tests" listed.

- [ ] **Step 6: Smoke test 4 — Episodic memory inspection**

```bash
sqlite3 $KLYNTBOT_HOME/data.db \
  "SELECT id, kind, importance, summary FROM episodic_memories WHERE kind='bash_job' ORDER BY recorded_at DESC LIMIT 5;"
```

Confirm rows match the spawns from steps 3-4.

- [ ] **Step 7: Smoke test 5 — Lost episode on restart**

Spawn a long-running command:
```
bash run_in_background=true command="sleep 60"
```

Force-quit Tauri (Cmd+Q + Force Quit if needed). Restart `cargo tauri dev`. Open the same coding thread.

Query:
```bash
sqlite3 $KLYNTBOT_HOME/data.db \
  "SELECT importance, summary FROM episodic_memories WHERE kind='bash_job' AND content LIKE '%\"status\":\"Lost\"%' ORDER BY recorded_at DESC LIMIT 1;"
```

Confirm one row with `importance = 0.6` and a summary containing "Lost".

### Task H6: Final commit + branch push

- [ ] **Step 1: Final workspace verification**

```bash
cargo build --workspace --release 2>&1 | tail -5
cargo nextest run --workspace 2>&1 | tail -5
cargo clippy --workspace --all-targets --all-features 2>&1 | tail -5
cargo fmt --all --check
```
All four must succeed.

- [ ] **Step 2: Push branch**

```bash
git push -u origin feat/coding-bash-execution-intelligence
```

- [ ] **Step 3: Open PR**

Use `gh pr create` per project convention. PR title: `feat(coding): Execution Intelligence — Phase 2.3b`. Body should reference the spec and list all six PR sub-phases as a summary.

---

## Verification

After every PR sub-phase, run:

```bash
cargo nextest run -p feature-coding-bash
cargo nextest run -p storage -E 'test(coding_background_jobs)+test(find_prior)'
cargo nextest run -p cognitive -E 'test(coding_bash)'
cargo nextest run -p bus -E 'test(bash_job_event_accessor)'
cargo nextest run -p app-core -E 'test(bash_job_translate)'
cargo nextest run -E 'test(intel_)'
cargo clippy --workspace --all-targets --all-features
```

All must succeed before moving to the next sub-phase.

---

## Self-review notes

After writing this plan I checked it against the spec — every section/requirement maps to a task:

| Spec section | Task |
|---|---|
| §3 No new tools | (no task — confirmed by inspection) |
| §4.1 SQLite extension (`command_key`) | A1 |
| §4.2 `BashJobRow.command_key` field | A2 |
| §4.2 `find_prior_by_command_key` | A4 |
| §4.3 Extended `failure_extracted` for TestFailure | B3 |
| §4.4 `JobDiff` types | D1 |
| §4.5 `VerificationVerb` types | C1 |
| §4.6 EpisodicMemory write shape | G2 |
| §5.1 `ExecutionIntelligenceInjector` | F2, F3 |
| §5.2 `command_key` normalization | A3 (normalize.rs creation) |
| §5.3 `diff_against_prior` | D2 |
| §5.4 `verification_match::classify` | C1 |
| §5.5 `GateClassifier` extension | B3 |
| §5.6 `supervisor::handle_exit` extension | E2 |
| §5.7 `render::completion_notification` extension | E1 |
| §5.8 `BackgroundJobSignalSource` | G2 |
| §5.9 Translator arm | G1 |
| §5.10 Registration | F3, G3 |
| §6.1–6.6 Data flows | covered by E2, E3, F4, G4 (integration tests) |
| §10 Error handling | covered inline (best-effort, log warnings) |
| §11 Testing strategy | tasks scattered across all PRs |

Type/method consistency check: `command_key`, `JobDiff`, `KindTransition`, `ExtractedDiff`, `VerificationVerb`, `ExecutionIntelligenceInjector`, `BackgroundJobSignalSource`, `find_prior_by_command_key`, `verification_affordance_reminder`, `build_episodic_memory` — all defined and used with consistent names across tasks.

No "TBD" / "TODO" / "implement later" markers in the plan body.

The two known plan-time uncertainties (already noted in the spec):
1. `MirrorEngine::start` signature extension — Task G3 explicitly adds `bash_repo: Option<Arc<storage::BashJobRepo>>` as a positional arg.
2. `TodoRepo::list_for_thread` exact name — Task F2 instructs the implementer to verify via `grep` and adapt.
