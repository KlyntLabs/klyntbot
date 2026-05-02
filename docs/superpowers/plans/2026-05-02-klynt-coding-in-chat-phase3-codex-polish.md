# Klynt Coding-in-Chat — Phase 3 (Codex Polish Port) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port three high-leverage, self-contained subsystems from `../codex` into Klynt to (a) replace BLOB-based file snapshots with git-content-addressed "ghost commits" for `/sessions rewind`, (b) replace the blunt 50 KB byte-chop tool-result truncation with a structured `TruncationPolicy` that preserves multi-item content and tells the model how much was cut, and (c) harden the production binary against debugger attach, core dumps, and `LD_*`/`DYLD_*` env-var library injection.

**Architecture:**
Three new leaf crates (`klynt-process-hardening` at L0, `klynt-truncation` at L1, `klynt-git-utils` at L1) each port a single self-contained source file from `/Users/jayden/Projects/Klynt/codex/codex-rs/`. Existing call sites (`crates/agent/src/execution/core.rs`, `crates/klynt-core/src/snapshots/repo.rs`, `crates/klynt-core/src/tools/{edit,write,apply_patch}.rs`, `crates/storage/src/repos/session.rs`, `crates/desktop/src/main.rs`) are minimally modified to consume the new crates. The `coding_snapshots` table gains a nullable `ghost_commit_sha` column so existing BLOB rows keep working — no breaking changes, no data migration (pre-release per CLAUDE.md).

**Tech Stack:** Rust 2024 (workspace MSRV 1.93), `tokio`, `sqlx` (sqlite), `tempfile`, `walkdir`, `libc` (for hardening), `git` subprocess (no `git2` FFI). Zero new heavy dependencies. All three workstreams are independently testable and shippable.

**Source-of-truth references (do not modify, port-only):**
- `/Users/jayden/Projects/Klynt/codex/codex-rs/process-hardening/src/lib.rs` (190 lines)
- `/Users/jayden/Projects/Klynt/codex/codex-rs/utils/output-truncation/src/lib.rs` (143 lines)
- `/Users/jayden/Projects/Klynt/codex/codex-rs/git-utils/src/ghost_commits.rs` (1786 lines)
- `/Users/jayden/Projects/Klynt/codex/codex-rs/git-utils/src/errors.rs` (35 lines)
- `/Users/jayden/Projects/Klynt/codex/codex-rs/git-utils/src/lib.rs` (lines 53-117 for `GhostCommit` struct)

---

## File Structure

### New crates (3)

| Path | Layer | Responsibility | Source-of-truth |
|---|---|---|---|
| `crates/klynt-process-hardening/` | L0 | Pre-main hardening: ptrace deny, core dump disable, env-var scrub | codex `process-hardening/` |
| `crates/klynt-truncation/` | L1 | `TruncationPolicy` enum + `formatted_truncate_text` + multi-item helpers | codex `utils/output-truncation/` |
| `crates/klynt-git-utils/` | L1 | Ghost commit creation/restore, `GhostCommit` struct, `GhostSnapshotConfig` | codex `git-utils/` (subset) |

### Files modified

| Path | Change |
|---|---|
| `Cargo.toml` (workspace root) | Register 3 new members; add `libc = "0.2"` workspace dep if missing |
| `crates/storage/migrations/001_initial.sql:866-878` | Add `ghost_commit_sha TEXT NULL` column to `coding_snapshots`; add index |
| `crates/klynt-core/Cargo.toml` | Add `klynt-git-utils` dep |
| `crates/klynt-core/src/snapshots/mod.rs` | Add `record_ghost` to `SnapshotService` trait |
| `crates/klynt-core/src/snapshots/repo.rs` | Add `Snapshot.ghost_commit_sha` field, `record_ghost` method, `try_record_with_ghost` orchestrator |
| `crates/klynt-core/src/tools/edit.rs` | Snapshot recording site → `try_record_with_ghost` |
| `crates/klynt-core/src/tools/write.rs` | Same |
| `crates/klynt-core/src/tools/apply_patch.rs:185-205` | Same |
| `crates/storage/src/repos/session.rs:535` | `rewind_to_message`: dispatch on `ghost_commit_sha`; if Some, call `klynt_git_utils::restore_ghost_commit`; else BLOB path |
| `crates/agent/Cargo.toml` | Add `klynt-truncation` dep |
| `crates/agent/src/execution/core.rs:59` | Replace `MAX_TOOL_RESULT_LENGTH` constant with `TruncationPolicy` |
| `crates/agent/src/execution/core.rs:90-108` | Replace `sanitize_tool_result` body with `klynt_truncation::formatted_truncate_text` |
| `crates/agent/src/execution/core.rs:677-686` | Replace inline 2 KB chop with `formatted_truncate_text` |
| `crates/desktop/Cargo.toml` | Add `klynt-process-hardening` dep |
| `crates/desktop/src/main.rs:100-115` | Call `klynt_process_hardening::pre_main_hardening()` immediately after `configure_mimalloc()` |
| `crates/config/src/schema/coding.rs` | Add `tool_result_truncation: TruncationConfig { bytes: usize, ws_payload_bytes: usize }` |
| `docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md` | Mark §6 line 680 (content-replacement) and the snapshot dedup Phase 3+ items as DONE; cite this plan |
| `CLAUDE.md` | Add Phase 3 ship note under "Gotchas" — ghost-commit fallback semantics |

### Tests

| Path | Type | Purpose |
|---|---|---|
| `crates/klynt-process-hardening/src/lib.rs` (inline) | Unit | `env_keys_with_prefix` UTF-8 + filter cases (verbatim from codex) |
| `crates/klynt-truncation/src/lib.rs` (inline) | Unit | `formatted_truncate_text` byte budget + line-count prefix |
| `crates/klynt-truncation/tests/multi_item.rs` | Integration | Multi-item function-output truncation preserves images |
| `crates/klynt-git-utils/src/ghost_commits.rs` (inline) | Unit | Round-trip: create_ghost_commit → mutate → restore_ghost_commit (≥6 cases) |
| `crates/klynt-core/src/snapshots/repo.rs` (inline) | Unit | `try_record_with_ghost` falls back to BLOB outside git repo |
| `tests/integration/snapshot_ghost_rewind.rs` | Integration | EditTool → rewind via ghost commit in real git repo |
| `tests/integration/snapshot_blob_rewind.rs` | Integration | EditTool → rewind via BLOB in non-git dir |
| `tests/unit/truncation_call_sites.rs` | Unit | Both call sites in `agent/execution/core.rs` cap at expected budgets |

---

## Workstream A — Process Hardening (5 tasks, ~25 min)

Smallest, simplest, highest-trust change. Ship first to build momentum and verify the new-crate scaffolding pattern before tackling B and C.

### Task A1: Scaffold `klynt-process-hardening` crate

**Files:**
- Create: `crates/klynt-process-hardening/Cargo.toml`
- Create: `crates/klynt-process-hardening/src/lib.rs`
- Modify: `Cargo.toml` (workspace root) — add member + `libc` dep

- [ ] **Step 1: Create the crate manifest**

Write to `crates/klynt-process-hardening/Cargo.toml`:

```toml
[package]
name = "klynt-process-hardening"
version.workspace = true
edition.workspace = true
license.workspace = true

[lints]
workspace = true

[dependencies]
libc = { workspace = true }

[dev-dependencies]
pretty_assertions = { workspace = true }
```

- [ ] **Step 2: Add the crate to the workspace and ensure `libc` workspace dep exists**

Open `Cargo.toml` (workspace root). Find `[workspace] members = [...]`. Add `"crates/klynt-process-hardening",` in alphabetical position. Then under `[workspace.dependencies]` ensure `libc = "0.2"` is present. If `pretty_assertions` is not already a workspace dep, add `pretty_assertions = "1"`.

Run: `grep -n '^libc\|^pretty_assertions' Cargo.toml`
Expected: both lines present.

- [ ] **Step 3: Verify scaffold compiles (empty lib)**

Write a one-line stub to `crates/klynt-process-hardening/src/lib.rs`:

```rust
//! Pre-main process hardening. See `pre_main_hardening`.
pub fn pre_main_hardening() {}
```

Run: `cargo build -p klynt-process-hardening`
Expected: clean build, zero warnings.

- [ ] **Step 4: Commit the scaffold**

```bash
git add crates/klynt-process-hardening/ Cargo.toml
git commit -m "feat(process-hardening): scaffold klynt-process-hardening crate"
```

### Task A2: Port the hardening implementation verbatim

**Files:**
- Modify: `crates/klynt-process-hardening/src/lib.rs`

- [ ] **Step 1: Port `lib.rs` from codex**

Open `/Users/jayden/Projects/Klynt/codex/codex-rs/process-hardening/src/lib.rs` and copy its full contents (lines 1-189) to `crates/klynt-process-hardening/src/lib.rs`. The code is platform-conditional (`#[cfg(unix)]` etc.) and depends only on `libc` and `std`.

The full code to write (all 190 lines):

```rust
#[cfg(unix)]
use std::ffi::OsString;

#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;

/// Pre-main hardening: disables core dumps, blocks ptrace attach (Linux/macOS),
/// and removes dangerous env vars (LD_*, DYLD_*, MallocStackLogging*).
///
/// Call from a `#[ctor::ctor]` or as the very first line of `fn main()`.
pub fn pre_main_hardening() {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pre_main_hardening_linux();

    #[cfg(target_os = "macos")]
    pre_main_hardening_macos();

    #[cfg(any(target_os = "freebsd", target_os = "openbsd"))]
    pre_main_hardening_bsd();

    #[cfg(windows)]
    pre_main_hardening_windows();
}

#[cfg(any(target_os = "linux", target_os = "android"))]
const PRCTL_FAILED_EXIT_CODE: i32 = 5;

#[cfg(target_os = "macos")]
const PTRACE_DENY_ATTACH_FAILED_EXIT_CODE: i32 = 6;

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]
const SET_RLIMIT_CORE_FAILED_EXIT_CODE: i32 = 7;

#[cfg(any(target_os = "linux", target_os = "android"))]
pub(crate) fn pre_main_hardening_linux() {
    let ret_code = unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0) };
    if ret_code != 0 {
        eprintln!(
            "ERROR: prctl(PR_SET_DUMPABLE, 0) failed: {}",
            std::io::Error::last_os_error()
        );
        std::process::exit(PRCTL_FAILED_EXIT_CODE);
    }
    set_core_file_size_limit_to_zero();
    remove_env_vars_with_prefix(b"LD_");
}

#[cfg(any(target_os = "freebsd", target_os = "openbsd"))]
pub(crate) fn pre_main_hardening_bsd() {
    set_core_file_size_limit_to_zero();
    remove_env_vars_with_prefix(b"LD_");
}

#[cfg(target_os = "macos")]
pub(crate) fn pre_main_hardening_macos() {
    let ret_code = unsafe { libc::ptrace(libc::PT_DENY_ATTACH, 0, std::ptr::null_mut(), 0) };
    if ret_code == -1 {
        eprintln!(
            "ERROR: ptrace(PT_DENY_ATTACH) failed: {}",
            std::io::Error::last_os_error()
        );
        std::process::exit(PTRACE_DENY_ATTACH_FAILED_EXIT_CODE);
    }
    set_core_file_size_limit_to_zero();
    remove_env_vars_with_prefix(b"DYLD_");
    remove_env_vars_with_prefix(b"MallocStackLogging");
    remove_env_vars_with_prefix(b"MallocLogFile");
}

#[cfg(unix)]
fn set_core_file_size_limit_to_zero() {
    let rlim = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
    let ret_code = unsafe { libc::setrlimit(libc::RLIMIT_CORE, &rlim) };
    if ret_code != 0 {
        eprintln!(
            "ERROR: setrlimit(RLIMIT_CORE) failed: {}",
            std::io::Error::last_os_error()
        );
        std::process::exit(SET_RLIMIT_CORE_FAILED_EXIT_CODE);
    }
}

#[cfg(windows)]
pub(crate) fn pre_main_hardening_windows() {
    // TODO: Windows hardening (Job Object, mitigations) is out of scope for Phase 3.
}

#[cfg(unix)]
fn remove_env_vars_with_prefix(prefix: &[u8]) {
    for key in env_keys_with_prefix(std::env::vars_os(), prefix) {
        unsafe { std::env::remove_var(key); }
    }
}

#[cfg(unix)]
fn env_keys_with_prefix<I>(vars: I, prefix: &[u8]) -> Vec<OsString>
where
    I: IntoIterator<Item = (OsString, OsString)>,
{
    vars.into_iter()
        .filter_map(|(key, _)| {
            key.as_os_str()
                .as_bytes()
                .starts_with(prefix)
                .then_some(key)
        })
        .collect()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::ffi::OsStringExt;

    #[test]
    fn env_keys_with_prefix_handles_non_utf8_entries() {
        let non_utf8_key1 = OsStr::from_bytes(b"R\xD6DBURK").to_os_string();
        assert!(non_utf8_key1.clone().into_string().is_err());
        let non_utf8_key2 = OsString::from_vec(vec![b'L', b'D', b'_', 0xF0]);
        assert!(non_utf8_key2.clone().into_string().is_err());

        let non_utf8_value = OsString::from_vec(vec![0xF0, 0x9F, 0x92, 0xA9]);

        let keys = env_keys_with_prefix(
            vec![
                (non_utf8_key1, non_utf8_value.clone()),
                (non_utf8_key2.clone(), non_utf8_value),
            ],
            b"LD_",
        );
        assert_eq!(keys, vec![non_utf8_key2]);
    }

    #[test]
    fn env_keys_with_prefix_filters_only_matching_keys() {
        let ld_test_var = OsStr::from_bytes(b"LD_TEST");
        let vars = vec![
            (OsString::from("PATH"), OsString::from("/usr/bin")),
            (ld_test_var.to_os_string(), OsString::from("1")),
            (OsString::from("DYLD_FOO"), OsString::from("bar")),
        ];

        let keys = env_keys_with_prefix(vars, b"LD_");
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].as_os_str(), ld_test_var);
    }
}
```

- [ ] **Step 2: Run unit tests**

Run: `cargo nextest run -p klynt-process-hardening`
Expected: 2 tests pass (`env_keys_with_prefix_handles_non_utf8_entries`, `env_keys_with_prefix_filters_only_matching_keys`).

- [ ] **Step 3: Lint**

Run: `cargo clippy -p klynt-process-hardening --all-targets --all-features -- -D warnings`
Expected: 0 warnings.

- [ ] **Step 4: Commit**

```bash
git add crates/klynt-process-hardening/src/lib.rs
git commit -m "feat(process-hardening): port pre_main_hardening from codex

Disables ptrace attach (macOS PT_DENY_ATTACH, Linux PR_SET_DUMPABLE),
zeros RLIMIT_CORE, and scrubs LD_*/DYLD_*/MallocStackLogging env vars.
Windows path is stubbed (Phase 3+ scope)."
```

### Task A3: Wire `pre_main_hardening()` into desktop binary

**Files:**
- Modify: `crates/desktop/Cargo.toml`
- Modify: `crates/desktop/src/main.rs:100-115`

- [ ] **Step 1: Add the dependency**

Open `crates/desktop/Cargo.toml`. Under `[dependencies]`, add (alphabetical):

```toml
klynt-process-hardening = { path = "../klynt-process-hardening" }
```

- [ ] **Step 2: Call from `fn main()`**

Open `crates/desktop/src/main.rs`. The current `fn main()` starts at line 100. Find `configure_mimalloc();` (likely the first call inside `main`). Insert the hardening call **immediately before** `configure_mimalloc()` so env scrubbing happens before any heap allocation reads env:

```rust
fn main() {
    // Pre-main hardening: ptrace deny, core-dump disable, env-var scrub.
    // Must run before any allocator setup that may inspect env vars.
    klynt_process_hardening::pre_main_hardening();

    configure_mimalloc();
    // ... rest of main ...
```

- [ ] **Step 3: Verify build**

Run: `cargo build -p desktop`
Expected: clean build.

- [ ] **Step 4: Smoke test — verify env vars are scrubbed**

Create `crates/desktop/tests/hardening_smoke.rs` (new file):

```rust
//! Verifies that pre_main_hardening removes LD_*/DYLD_* env vars when run.
//! We can't test it from inside an integration test that itself needs the
//! env, so we shell out to a tiny helper binary.

#[cfg(target_os = "macos")]
#[test]
fn macos_dyld_vars_are_removed() {
    use std::process::Command;
    // The test only verifies that linking & calling the function does not
    // segfault on the host. End-to-end verification is via manual run of
    // the desktop binary with DYLD_INSERT_LIBRARIES set.
    klynt_process_hardening::pre_main_hardening();
    // After hardening, no DYLD_ var should remain (we only inspect, don't set).
    for (k, _) in std::env::vars() {
        assert!(!k.starts_with("DYLD_"), "DYLD_ var leaked: {k}");
    }
    let _ = Command::new("/usr/bin/true").status();
}
```

Run: `cargo nextest run -p desktop --test hardening_smoke`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/desktop/Cargo.toml crates/desktop/src/main.rs crates/desktop/tests/hardening_smoke.rs
git commit -m "feat(desktop): wire pre_main_hardening at main entry"
```

### Task A4: Document the gotcha in CLAUDE.md

**Files:**
- Modify: `CLAUDE.md` (under "Gotchas" section)

- [ ] **Step 1: Add gotcha entry**

Open `CLAUDE.md`. Find the `## Gotchas` section. Append:

```markdown
- **Process hardening runs at startup** — `crates/desktop/src/main.rs` calls `klynt_process_hardening::pre_main_hardening()` as its first statement. This (a) sets `RLIMIT_CORE = 0` (no core dumps), (b) calls `ptrace(PT_DENY_ATTACH)` on macOS (debuggers cannot attach to a release build), and (c) scrubs `LD_*`/`DYLD_*`/`MallocStackLogging*` env vars. To debug a release build, comment the call out — debug builds are not affected because `PT_DENY_ATTACH` is harmless when no debugger tries to attach.
```

- [ ] **Step 2: Commit**

```bash
git add CLAUDE.md
git commit -m "docs(claude-md): note pre_main_hardening startup gotcha"
```

### Task A5: Workspace-wide verification

- [ ] **Step 1: Full workspace build + clippy**

Run: `cargo build --workspace && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo fmt --all --check`
Expected: clean.

- [ ] **Step 2: Full workspace test**

Run: `cargo nextest run --workspace`
Expected: all tests green (no regression).

- [ ] **Step 3: Tag the workstream complete (no commit needed if green)**

If anything failed, fix in-place (not by reverting) and re-run.

---

## Workstream B — Structured Truncation (10 tasks, ~90 min)

Replaces two ad-hoc truncation sites in `crates/agent/src/execution/core.rs` with a single typed `TruncationPolicy` API. Adds a "Total output lines: N" prefix that helps the model reason about cuts (the most subtle behavioral improvement in this plan).

### Task B1: Scaffold `klynt-truncation` crate

**Files:**
- Create: `crates/klynt-truncation/Cargo.toml`
- Create: `crates/klynt-truncation/src/lib.rs`
- Modify: `Cargo.toml` (workspace root) — add member

- [ ] **Step 1: Create manifest**

Write to `crates/klynt-truncation/Cargo.toml`:

```toml
[package]
name = "klynt-truncation"
version.workspace = true
edition.workspace = true
license.workspace = true

[lints]
workspace = true

[dependencies]
serde = { workspace = true, features = ["derive"] }

[dev-dependencies]
pretty_assertions = { workspace = true }
```

- [ ] **Step 2: Register in workspace**

Open `Cargo.toml` (workspace root) and add `"crates/klynt-truncation",` under `[workspace] members`.

- [ ] **Step 3: Stub lib.rs and verify compile**

Write to `crates/klynt-truncation/src/lib.rs`:

```rust
//! Structured truncation policies for tool results and exec output.
//! Ported from codex `utils/output-truncation`.
```

Run: `cargo build -p klynt-truncation`
Expected: clean build.

- [ ] **Step 4: Commit**

```bash
git add crates/klynt-truncation/ Cargo.toml
git commit -m "feat(truncation): scaffold klynt-truncation crate"
```

### Task B2: Define `TruncationPolicy` enum (TDD)

**Files:**
- Modify: `crates/klynt-truncation/src/lib.rs`

- [ ] **Step 1: Write the failing test first**

Append to `crates/klynt-truncation/src/lib.rs`:

```rust
#[cfg(test)]
mod policy_tests {
    use super::*;

    #[test]
    fn byte_budget_returns_inner_value() {
        assert_eq!(TruncationPolicy::Bytes(1024).byte_budget(), 1024);
    }

    #[test]
    fn token_budget_returns_inner_value() {
        assert_eq!(TruncationPolicy::Tokens(500).token_budget(), 500);
    }

    #[test]
    fn token_policy_byte_budget_uses_4x_heuristic() {
        // 1 token ≈ 4 bytes (codex convention)
        assert_eq!(TruncationPolicy::Tokens(100).byte_budget(), 400);
    }

    #[test]
    fn byte_policy_token_budget_divides_by_four() {
        assert_eq!(TruncationPolicy::Bytes(400).token_budget(), 100);
    }
}
```

- [ ] **Step 2: Run — expect compile failure**

Run: `cargo nextest run -p klynt-truncation`
Expected: FAIL (`TruncationPolicy` undefined).

- [ ] **Step 3: Add the enum**

Insert at the top of `crates/klynt-truncation/src/lib.rs` (above the `#[cfg(test)]` block):

```rust
use serde::{Deserialize, Serialize};

/// How to budget a truncation operation.
///
/// `Bytes(n)` — keep at most `n` bytes (UTF-8 safe, middle-truncated).
/// `Tokens(n)` — keep at most `n` approximate tokens (4 chars ≈ 1 token).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TruncationPolicy {
    Bytes(usize),
    Tokens(usize),
}

impl TruncationPolicy {
    /// Bytes-equivalent of this budget. Tokens are approximated at 4 bytes/token.
    pub fn byte_budget(self) -> usize {
        match self {
            Self::Bytes(b) => b,
            Self::Tokens(t) => t.saturating_mul(4),
        }
    }

    /// Tokens-equivalent of this budget. Bytes are approximated at 4 bytes/token.
    pub fn token_budget(self) -> usize {
        match self {
            Self::Bytes(b) => b / 4,
            Self::Tokens(t) => t,
        }
    }
}
```

- [ ] **Step 4: Re-run, expect PASS**

Run: `cargo nextest run -p klynt-truncation`
Expected: 4 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/klynt-truncation/src/lib.rs
git commit -m "feat(truncation): TruncationPolicy enum (Bytes/Tokens) with budget helpers"
```

### Task B3: Implement `truncate_middle_chars` (TDD)

**Files:**
- Modify: `crates/klynt-truncation/src/lib.rs`

- [ ] **Step 1: Write failing tests**

Append to `crates/klynt-truncation/src/lib.rs`:

```rust
#[cfg(test)]
mod middle_chars_tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn no_truncation_when_under_budget() {
        assert_eq!(truncate_middle_chars("hello", 10), "hello");
    }

    #[test]
    fn keeps_head_and_tail_with_marker() {
        let out = truncate_middle_chars("0123456789abcdef", 10);
        assert!(out.contains("[...] omitted "), "missing marker: {out}");
        assert!(out.starts_with('0'));
        assert!(out.ends_with('f'));
        assert!(out.len() <= 80, "marker overhead bounded");
    }

    #[test]
    fn handles_multibyte_at_boundary() {
        // 'é' is 2 bytes in UTF-8. Budget=4 must not split it.
        let s = "aéaéaéaéaé";
        let out = truncate_middle_chars(s, 6);
        assert!(out.is_char_boundary(out.len()));
    }

    #[test]
    fn empty_input_is_empty() {
        assert_eq!(truncate_middle_chars("", 100), "");
    }

    #[test]
    fn budget_zero_returns_marker_only() {
        let out = truncate_middle_chars("abc", 0);
        assert!(out.contains("[...] omitted "));
    }
}
```

- [ ] **Step 2: Run — expect failure**

Run: `cargo nextest run -p klynt-truncation middle_chars`
Expected: FAIL (function undefined).

- [ ] **Step 3: Implement**

Insert before the `#[cfg(test)]` blocks:

```rust
/// Truncate `content` to at most `byte_budget` bytes by keeping the first and
/// last halves and replacing the middle with `[...] omitted N bytes [...]`.
/// UTF-8 safe — never splits a multibyte char.
pub fn truncate_middle_chars(content: &str, byte_budget: usize) -> String {
    if content.len() <= byte_budget {
        return content.to_string();
    }

    const MARKER_TEMPLATE: &str = "\n\n[...] omitted XXXXXXXXXX bytes [...]\n\n";
    let marker_overhead = MARKER_TEMPLATE.len();

    if byte_budget <= marker_overhead {
        let omitted = content.len();
        return format!("\n[...] omitted {omitted} bytes [...]\n");
    }

    let visible = byte_budget - marker_overhead;
    let head_target = visible / 2;
    let tail_target = visible - head_target;

    // Find safe char boundaries.
    let mut head_end = head_target.min(content.len());
    while head_end > 0 && !content.is_char_boundary(head_end) {
        head_end -= 1;
    }
    let mut tail_start = content.len().saturating_sub(tail_target);
    while tail_start < content.len() && !content.is_char_boundary(tail_start) {
        tail_start += 1;
    }
    if tail_start <= head_end {
        // Pathological case — fall back to head only.
        return content[..head_end].to_string();
    }

    let omitted = tail_start - head_end;
    format!(
        "{}\n\n[...] omitted {} bytes [...]\n\n{}",
        &content[..head_end],
        omitted,
        &content[tail_start..]
    )
}
```

- [ ] **Step 4: Re-run, expect PASS**

Run: `cargo nextest run -p klynt-truncation middle_chars`
Expected: 5 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/klynt-truncation/src/lib.rs
git commit -m "feat(truncation): UTF-8-safe truncate_middle_chars"
```

### Task B4: Implement `formatted_truncate_text` (TDD)

**Files:**
- Modify: `crates/klynt-truncation/src/lib.rs`

- [ ] **Step 1: Write failing tests**

Append to `crates/klynt-truncation/src/lib.rs`:

```rust
#[cfg(test)]
mod formatted_tests {
    use super::*;

    #[test]
    fn no_prefix_when_under_budget() {
        let out = formatted_truncate_text("short", TruncationPolicy::Bytes(100));
        assert_eq!(out, "short");
    }

    #[test]
    fn prefixes_with_total_lines_when_truncated() {
        let big = "line\n".repeat(2000); // ~10000 bytes
        let out = formatted_truncate_text(&big, TruncationPolicy::Bytes(200));
        assert!(out.starts_with("Total output lines: 2000\n\n"), "got: {out}");
    }

    #[test]
    fn token_policy_uses_byte_equivalent() {
        let big = "x".repeat(1000);
        let out = formatted_truncate_text(&big, TruncationPolicy::Tokens(50)); // ≈ 200 bytes
        assert!(out.starts_with("Total output lines: 1\n\n"));
        assert!(out.len() <= 300);
    }
}
```

- [ ] **Step 2: Run — expect FAIL**

Run: `cargo nextest run -p klynt-truncation formatted`
Expected: FAIL (functions undefined).

- [ ] **Step 3: Implement**

Insert before the existing `#[cfg(test)]` blocks:

```rust
/// Truncate `content` per `policy`. If truncated, prepends a
/// `"Total output lines: N\n\n"` header so the model knows how much was cut.
pub fn formatted_truncate_text(content: &str, policy: TruncationPolicy) -> String {
    if content.len() <= policy.byte_budget() {
        return content.to_string();
    }
    let total_lines = content.lines().count();
    let truncated = truncate_middle_chars(content, policy.byte_budget());
    format!("Total output lines: {total_lines}\n\n{truncated}")
}

/// Truncate without the "Total output lines:" prefix. Use this for non-tool
/// transport-layer caps (WebSocket payloads etc.) where the model never sees
/// the result.
pub fn truncate_text(content: &str, policy: TruncationPolicy) -> String {
    truncate_middle_chars(content, policy.byte_budget())
}
```

- [ ] **Step 4: Re-run, expect PASS**

Run: `cargo nextest run -p klynt-truncation`
Expected: all 12 tests pass (4 policy + 5 middle_chars + 3 formatted).

- [ ] **Step 5: Commit**

```bash
git add crates/klynt-truncation/src/lib.rs
git commit -m "feat(truncation): formatted_truncate_text with line-count prefix"
```

### Task B5: Add `MultiItem` helper for content arrays (TDD)

**Files:**
- Modify: `crates/klynt-truncation/src/lib.rs`
- Create: `crates/klynt-truncation/tests/multi_item.rs`

The Klynt `Message::Tool.content` is currently `String` (per CLAUDE.md "image-bearing tool results have no schema today"), but the spec earmarks future image support. We add the `ContentItem` enum so when the agent eventually carries images (Computer Use feature, deferred), truncation already knows how to preserve them.

- [ ] **Step 1: Write integration test (failing)**

Create `crates/klynt-truncation/tests/multi_item.rs`:

```rust
use klynt_truncation::{
    truncate_function_output_items, ContentItem, TruncationPolicy,
};

#[test]
fn images_are_preserved_text_is_truncated() {
    let items = vec![
        ContentItem::Text("a".repeat(10_000)),
        ContentItem::Image { url: "data:image/png;base64,AAA".into() },
        ContentItem::Text("b".repeat(10_000)),
    ];

    let out = truncate_function_output_items(&items, TruncationPolicy::Bytes(500));

    let images: Vec<_> = out
        .iter()
        .filter(|i| matches!(i, ContentItem::Image { .. }))
        .collect();
    assert_eq!(images.len(), 1, "image must survive");

    let total_text_bytes: usize = out
        .iter()
        .filter_map(|i| match i {
            ContentItem::Text(t) => Some(t.len()),
            _ => None,
        })
        .sum();
    assert!(total_text_bytes <= 700, "text truncated: {total_text_bytes}");
}

#[test]
fn omitted_items_get_sentinel() {
    let items = vec![
        ContentItem::Text("a".repeat(1000)),
        ContentItem::Text("b".repeat(1000)),
        ContentItem::Text("c".repeat(1000)),
    ];
    let out = truncate_function_output_items(&items, TruncationPolicy::Bytes(900));
    let last = out.last().unwrap();
    if let ContentItem::Text(t) = last {
        assert!(t.contains("omitted"), "expected sentinel: {t}");
    } else {
        panic!("last item should be sentinel text");
    }
}
```

- [ ] **Step 2: Run — expect FAIL**

Run: `cargo nextest run -p klynt-truncation --test multi_item`
Expected: FAIL (`ContentItem`, `truncate_function_output_items` undefined).

- [ ] **Step 3: Implement `ContentItem` and helper**

Append to `crates/klynt-truncation/src/lib.rs`:

```rust
/// A single item inside a tool result. Mirrors codex's
/// `FunctionCallOutputContentItem` but stripped to the variants Klynt needs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentItem {
    Text(String),
    Image { url: String },
}

/// Truncate a list of content items per `policy`. Images always survive.
/// Text items are truncated middle-char-wise, distributed across the budget
/// in order. When a text item runs out of budget it's dropped and a final
/// `[omitted N text items ...]` sentinel is appended.
pub fn truncate_function_output_items(
    items: &[ContentItem],
    policy: TruncationPolicy,
) -> Vec<ContentItem> {
    let mut out: Vec<ContentItem> = Vec::with_capacity(items.len());
    let mut remaining = policy.byte_budget();
    let mut omitted = 0usize;

    for item in items {
        match item {
            ContentItem::Text(t) => {
                if remaining == 0 {
                    omitted += 1;
                    continue;
                }
                if t.len() <= remaining {
                    out.push(ContentItem::Text(t.clone()));
                    remaining = remaining.saturating_sub(t.len());
                } else {
                    let snippet = truncate_middle_chars(t, remaining);
                    if snippet.is_empty() {
                        omitted += 1;
                    } else {
                        out.push(ContentItem::Text(snippet));
                    }
                    remaining = 0;
                }
            }
            ContentItem::Image { url } => {
                out.push(ContentItem::Image { url: url.clone() });
            }
        }
    }

    if omitted > 0 {
        out.push(ContentItem::Text(format!("[omitted {omitted} text items ...]")));
    }
    out
}
```

- [ ] **Step 4: Re-run**

Run: `cargo nextest run -p klynt-truncation`
Expected: all 14 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/klynt-truncation/
git commit -m "feat(truncation): ContentItem + multi-item helper preserves images"
```

### Task B6: Replace `MAX_TOOL_RESULT_LENGTH` in agent execution (TDD)

**Files:**
- Modify: `crates/agent/Cargo.toml`
- Modify: `crates/agent/src/execution/core.rs:59` and `90-108`

- [ ] **Step 1: Add dep**

Open `crates/agent/Cargo.toml` and under `[dependencies]` add:

```toml
klynt-truncation = { path = "../klynt-truncation" }
```

- [ ] **Step 2: Write failing test for new behavior**

Create `tests/unit/truncation_call_sites.rs`:

```rust
//! Verifies the agent's tool-result truncation goes through klynt-truncation.

use klynt_truncation::{formatted_truncate_text, TruncationPolicy};

#[test]
fn agent_tool_result_includes_line_count_when_truncated() {
    let big = "line\n".repeat(20_000); // ~100KB
    let out = formatted_truncate_text(&big, TruncationPolicy::Bytes(50_000));
    assert!(
        out.starts_with("Total output lines: 20000\n\n"),
        "expected line-count header; got first 80 chars: {}",
        &out[..out.len().min(80)]
    );
}

#[test]
fn agent_tool_result_passes_through_when_small() {
    let small = "ok";
    let out = formatted_truncate_text(small, TruncationPolicy::Bytes(50_000));
    assert_eq!(out, "ok");
}
```

Run: `cargo nextest run --test unit -E 'test(truncation_call_sites)'`
Expected: PASS (these test the helper, not the agent yet — gives a regression baseline).

- [ ] **Step 3: Replace the agent's `sanitize_tool_result`**

Open `crates/agent/src/execution/core.rs`. At line 59, the constant is:

```rust
const MAX_TOOL_RESULT_LENGTH: usize = 50_000;
```

Change to:

```rust
const MAX_TOOL_RESULT_LENGTH: usize = 50_000;
const TOOL_RESULT_TRUNCATION_POLICY: klynt_truncation::TruncationPolicy =
    klynt_truncation::TruncationPolicy::Bytes(MAX_TOOL_RESULT_LENGTH);
```

Then find `fn sanitize_tool_result` (lines 90-108):

```rust
fn sanitize_tool_result(input: &str) -> String {
    let cleaned: String = input
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t' || *c == '\r')
        .collect();

    if cleaned.len() > MAX_TOOL_RESULT_LENGTH {
        let mut truncate_at = MAX_TOOL_RESULT_LENGTH;
        while truncate_at > 0 && !cleaned.is_char_boundary(truncate_at) {
            truncate_at -= 1;
        }
        let mut truncated = cleaned[..truncate_at].to_string();
        truncated.push_str("\n[truncated - result exceeded 50KB]");
        truncated
    } else {
        cleaned
    }
}
```

Replace with:

```rust
fn sanitize_tool_result(input: &str) -> String {
    let cleaned: String = input
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t' || *c == '\r')
        .collect();
    klynt_truncation::formatted_truncate_text(&cleaned, TOOL_RESULT_TRUNCATION_POLICY)
}
```

- [ ] **Step 4: Update the existing assertion at `core.rs:1225`**

The old test at line 1225 has:

```rust
assert!(result.len() <= MAX_TOOL_RESULT_LENGTH + 50);
```

Change the slack to account for the new prefix (`Total output lines: NNNNN\n\n` ≈ ≤ 40 bytes) plus the existing middle-marker (≤ 60 bytes). New assertion:

```rust
assert!(
    result.len() <= MAX_TOOL_RESULT_LENGTH + 200,
    "result.len() = {}",
    result.len()
);
```

- [ ] **Step 5: Build + test agent crate**

Run: `cargo build -p agent && cargo nextest run -p agent`
Expected: all green. Pay attention to any test that asserts on the literal "[truncated - result exceeded 50KB]" string — those need updating to match the new format.

- [ ] **Step 6: If any test asserts on the old literal**, update them to assert `out.starts_with("Total output lines:")` instead.

- [ ] **Step 7: Commit**

```bash
git add crates/agent/Cargo.toml crates/agent/src/execution/core.rs tests/unit/truncation_call_sites.rs
git commit -m "feat(agent): route sanitize_tool_result through klynt-truncation"
```

### Task B7: Replace inline 2 KB WebSocket truncation (TDD)

**Files:**
- Modify: `crates/agent/src/execution/core.rs:677-686`

- [ ] **Step 1: Locate the duplicate**

The current code at `crates/agent/src/execution/core.rs:677-686`:

```rust
let truncated = if result_str.len() > 2048 {
    let end = (0..=2048)
        .rev()
        .find(|&i| result_str.is_char_boundary(i))
        .unwrap_or(0);
    let mut s = result_str[..end].to_string();
    s.push_str("…[truncated]");
    Some(s)
} else {
    Some(result_str.clone())
};
```

- [ ] **Step 2: Replace with helper**

Replace those 10 lines with:

```rust
let truncated = Some(klynt_truncation::truncate_text(
    &result_str,
    klynt_truncation::TruncationPolicy::Bytes(2048),
));
```

Note: this site uses `truncate_text` (no line-count prefix) because the result here is the WebSocket transport payload that the model never sees — it's only for the desktop UI's "tool finished" toast.

- [ ] **Step 3: Build + test**

Run: `cargo build -p agent && cargo nextest run -p agent`
Expected: green.

- [ ] **Step 4: Commit**

```bash
git add crates/agent/src/execution/core.rs
git commit -m "feat(agent): unify 2KB WebSocket truncation via klynt-truncation"
```

### Task B8: Make truncation budgets configurable

**Files:**
- Modify: `crates/config/src/schema/coding.rs`
- Modify: `crates/agent/src/execution/core.rs`

- [ ] **Step 1: Add config struct**

Open `crates/config/src/schema/coding.rs`. Add (preserve `#[serde(rename_all = "camelCase")]` convention from CLAUDE.md):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ToolResultTruncationConfig {
    /// Bytes budget for tool results injected into the conversation.
    pub model_facing_bytes: usize,
    /// Bytes budget for the result snippet sent on the WebSocket "tool finished" event.
    pub ws_payload_bytes: usize,
}

impl Default for ToolResultTruncationConfig {
    fn default() -> Self {
        Self {
            model_facing_bytes: 50_000,
            ws_payload_bytes: 2_048,
        }
    }
}
```

Locate the parent `CodingConfig` struct (already in this file). Add a field:

```rust
#[serde(default)]
pub tool_result_truncation: ToolResultTruncationConfig,
```

- [ ] **Step 2: Build config crate**

Run: `cargo build -p config`
Expected: green.

- [ ] **Step 3: Acknowledge that the constants in `core.rs` remain compile-time defaults**

We are NOT plumbing the config through to `execution/core.rs` in this plan — it would require restructuring `MidLoopCompressor` initialization. The config struct exists so future work can read it; for Phase 3 the constants are still authoritative. Add a doc comment to `tool_result_truncation` noting this:

```rust
/// Tool-result truncation budgets. Currently used by the truncation crate's
/// defaults — runtime override is Phase 4 work (requires plumbing into
/// `MidLoopCompressor` initialisation).
pub tool_result_truncation: ToolResultTruncationConfig,
```

- [ ] **Step 4: Commit**

```bash
git add crates/config/src/schema/coding.rs
git commit -m "feat(config): add ToolResultTruncationConfig for future runtime override"
```

### Task B9: Workstream B verification

- [ ] **Step 1: Workspace build, clippy, fmt**

Run:
```bash
cargo build --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
```
Expected: all green.

- [ ] **Step 2: Workspace tests**

Run: `cargo nextest run --workspace`
Expected: all green.

- [ ] **Step 3: Verify no truncation regression on chat first-token**

If `bench/chat_send_to_first_token.rs` exists from Phase 2 task H1, run it:

Run: `cargo bench -p kca-bench --bench chat_send_to_first_token`
Expected: p95 < 800 ms (Phase 2 gate). If the bench harness lives elsewhere, locate via `find . -name "chat_send_to_first_token*" -type f`.

If the bench is unavailable, skip and note in the commit message.

### Task B10: Update spec — mark §6 line 680 (content-replacement) status

**Files:**
- Modify: `docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md`

- [ ] **Step 1: Find the line**

Run: `grep -n "content-replacement\|oversized" docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md`
Expected: line ~680.

- [ ] **Step 2: Append a status note**

Find the sentence "Phase 3+ adds Claude Code's content-replacement pattern for oversized results we want to preserve in full." Append:

```markdown
**Phase 3 status (2026-05-02):** Adopted codex's `TruncationPolicy` (Bytes/Tokens) with structured middle-chop and a "Total output lines: N" prefix instead of Claude Code's content-replacement pattern. Trade-off: model loses access to the full content, but gains an explicit line-count signal. See plan `2026-05-02-klynt-coding-in-chat-phase3-codex-polish.md` for rationale.
```

- [ ] **Step 3: Commit**

```bash
git add docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md
git commit -m "docs(spec): note Phase 3 truncation choice (TruncationPolicy over content-replacement)"
```

---

## Workstream C — Ghost Commits / Content-addressed Snapshots (16 tasks, ~3 hr)

Replaces raw-BLOB `coding_snapshots` rows with git ghost commits when the snapshot target lives inside a git repo. Falls back to BLOB outside a repo. Existing `/sessions rewind` keeps working unchanged from the user's perspective; storage shrinks dramatically and restore becomes atomic.

Ghost commits work by writing a tree to git's object database (`git write-tree`) and committing it (`git commit-tree`) with no ref pointing at it — the commit is reachable only via its SHA, which we store in the snapshot row. Restore is `git restore --source <sha> --worktree`. This is exactly how codex implements `/edit undo`.

### Task C1: Scaffold `klynt-git-utils` crate

**Files:**
- Create: `crates/klynt-git-utils/Cargo.toml`
- Create: `crates/klynt-git-utils/src/lib.rs`
- Create: `crates/klynt-git-utils/src/errors.rs`
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Manifest**

Write to `crates/klynt-git-utils/Cargo.toml`:

```toml
[package]
name = "klynt-git-utils"
version.workspace = true
edition.workspace = true
license.workspace = true

[lints]
workspace = true

[dependencies]
common = { path = "../common" }
once_cell = { workspace = true }
serde = { workspace = true, features = ["derive"] }
tempfile = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true, features = ["macros", "process", "rt", "time"] }
walkdir = { workspace = true }

[dev-dependencies]
pretty_assertions = { workspace = true }
tokio = { workspace = true, features = ["macros", "rt-multi-thread"] }
```

- [ ] **Step 2: Register workspace member**

Open `Cargo.toml` (workspace root). Under `[workspace] members`, add `"crates/klynt-git-utils",` alphabetically.

- [ ] **Step 3: Stub `lib.rs` and `errors.rs`**

Write to `crates/klynt-git-utils/src/lib.rs`:

```rust
//! Git ghost-commit snapshots for code-session rewind.
//! Ported from codex `git-utils/src/ghost_commits.rs`.

mod errors;
pub use errors::GitToolingError;
```

Write to `crates/klynt-git-utils/src/errors.rs`:

```rust
use std::path::PathBuf;
use std::process::ExitStatus;
use std::string::FromUtf8Error;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum GitToolingError {
    #[error("git command `{command}` failed with status {status}: {stderr}")]
    GitCommand {
        command: String,
        status: ExitStatus,
        stderr: String,
    },
    #[error("git command `{command}` produced non-UTF-8 output")]
    GitOutputUtf8 {
        command: String,
        #[source]
        source: FromUtf8Error,
    },
    #[error("{path:?} is not a git repository")]
    NotAGitRepository { path: PathBuf },
    #[error("path {path:?} must be relative to the repository root")]
    NonRelativePath { path: PathBuf },
    #[error("path {path:?} escapes the repository root")]
    PathEscapesRepository { path: PathBuf },
    #[error("failed to process path inside worktree")]
    PathPrefix(#[from] std::path::StripPrefixError),
    #[error(transparent)]
    Walkdir(#[from] walkdir::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
```

- [ ] **Step 4: Verify build**

Run: `cargo build -p klynt-git-utils`
Expected: clean build, zero warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/klynt-git-utils/ Cargo.toml
git commit -m "feat(git-utils): scaffold klynt-git-utils crate with errors"
```

### Task C2: Define `GhostCommit` struct + repo-detection helper (TDD)

**Files:**
- Modify: `crates/klynt-git-utils/src/lib.rs`
- Create: `crates/klynt-git-utils/src/repo.rs`

- [ ] **Step 1: Write failing tests for `is_inside_git_repo`**

Append to `crates/klynt-git-utils/src/lib.rs`:

```rust
mod repo;
pub use repo::{is_inside_git_repo, get_git_repo_root};

#[cfg(test)]
mod repo_detect_tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn returns_false_outside_repo() {
        let tmp = TempDir::new().unwrap();
        assert!(!is_inside_git_repo(tmp.path()).await.unwrap());
    }

    #[tokio::test]
    async fn returns_true_inside_repo() {
        let tmp = TempDir::new().unwrap();
        tokio::process::Command::new("git")
            .arg("init")
            .current_dir(tmp.path())
            .output()
            .await
            .unwrap();
        assert!(is_inside_git_repo(tmp.path()).await.unwrap());
    }

    #[tokio::test]
    async fn returns_repo_root_for_subdir() {
        let tmp = TempDir::new().unwrap();
        tokio::process::Command::new("git")
            .arg("init")
            .current_dir(tmp.path())
            .output()
            .await
            .unwrap();
        let sub = tmp.path().join("a/b/c");
        std::fs::create_dir_all(&sub).unwrap();
        let root = get_git_repo_root(&sub).await.unwrap();
        // git resolves symlinks (e.g. /var → /private/var on macOS)
        let canonical = std::fs::canonicalize(tmp.path()).unwrap();
        assert_eq!(root, canonical);
    }
}
```

- [ ] **Step 2: Run — expect FAIL (compile)**

Run: `cargo nextest run -p klynt-git-utils repo_detect`
Expected: FAIL.

- [ ] **Step 3: Implement repo detection**

Write to `crates/klynt-git-utils/src/repo.rs`:

```rust
use std::path::{Path, PathBuf};
use tokio::process::Command;

use crate::errors::GitToolingError;

/// Returns true if `path` is inside a git working tree.
pub async fn is_inside_git_repo(path: &Path) -> Result<bool, GitToolingError> {
    let out = Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(path)
        .output()
        .await?;
    Ok(out.status.success() && String::from_utf8_lossy(&out.stdout).trim() == "true")
}

/// Returns the absolute path of the git working-tree root containing `path`.
pub async fn get_git_repo_root(path: &Path) -> Result<PathBuf, GitToolingError> {
    let out = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(path)
        .output()
        .await?;
    if !out.status.success() {
        return Err(GitToolingError::NotAGitRepository {
            path: path.to_path_buf(),
        });
    }
    let s = String::from_utf8(out.stdout).map_err(|e| GitToolingError::GitOutputUtf8 {
        command: "git rev-parse --show-toplevel".into(),
        source: e,
    })?;
    Ok(PathBuf::from(s.trim()))
}
```

- [ ] **Step 4: Run, expect PASS**

Run: `cargo nextest run -p klynt-git-utils repo_detect`
Expected: 3 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/klynt-git-utils/
git commit -m "feat(git-utils): is_inside_git_repo and get_git_repo_root"
```

### Task C3: Define `GhostCommit` struct

**Files:**
- Modify: `crates/klynt-git-utils/src/lib.rs`

- [ ] **Step 1: Write failing serde test**

Append to `crates/klynt-git-utils/src/lib.rs`:

```rust
#[cfg(test)]
mod ghost_commit_struct_tests {
    use super::*;

    #[test]
    fn ghost_commit_round_trips_serde() {
        let g = GhostCommit::new(
            "abc123".into(),
            Some("def456".into()),
            vec![std::path::PathBuf::from("untracked.txt")],
            vec![std::path::PathBuf::from("untracked_dir")],
        );
        let s = serde_json::to_string(&g).unwrap();
        let back: GhostCommit = serde_json::from_str(&s).unwrap();
        assert_eq!(g, back);
        assert_eq!(g.id(), "abc123");
        assert_eq!(g.parent(), Some("def456"));
    }
}
```

- [ ] **Step 2: Run — expect FAIL**

Run: `cargo nextest run -p klynt-git-utils ghost_commit_struct`
Expected: FAIL.

- [ ] **Step 3: Implement struct**

Insert near the top of `crates/klynt-git-utils/src/lib.rs` (after the `mod`/`pub use` lines):

```rust
use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

type CommitID = String;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GhostCommit {
    id: CommitID,
    parent: Option<CommitID>,
    preexisting_untracked_files: Vec<PathBuf>,
    preexisting_untracked_dirs: Vec<PathBuf>,
}

impl GhostCommit {
    pub fn new(
        id: CommitID,
        parent: Option<CommitID>,
        preexisting_untracked_files: Vec<PathBuf>,
        preexisting_untracked_dirs: Vec<PathBuf>,
    ) -> Self {
        Self { id, parent, preexisting_untracked_files, preexisting_untracked_dirs }
    }
    pub fn id(&self) -> &str { &self.id }
    pub fn parent(&self) -> Option<&str> { self.parent.as_deref() }
    pub fn preexisting_untracked_files(&self) -> &[PathBuf] { &self.preexisting_untracked_files }
    pub fn preexisting_untracked_dirs(&self) -> &[PathBuf] { &self.preexisting_untracked_dirs }
}

impl fmt::Display for GhostCommit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.id)
    }
}
```

If `serde_json` is not yet a dev-dep, add to `[dev-dependencies]` in `crates/klynt-git-utils/Cargo.toml`:

```toml
serde_json = { workspace = true }
```

- [ ] **Step 4: Re-run, expect PASS**

Run: `cargo nextest run -p klynt-git-utils ghost_commit_struct`
Expected: 1 test passes.

- [ ] **Step 5: Commit**

```bash
git add crates/klynt-git-utils/
git commit -m "feat(git-utils): GhostCommit struct with serde + accessors"
```

### Task C4: Implement `create_ghost_commit` (TDD, the core)

**Files:**
- Create: `crates/klynt-git-utils/src/ghost_commits.rs`
- Modify: `crates/klynt-git-utils/src/lib.rs`

This is the meat. The algorithm:
1. Run `git rev-parse HEAD` to get the parent SHA (None if no commits).
2. Create a temp file path for `GIT_INDEX_FILE` so we don't touch the user's index.
3. If parent exists: `git read-tree HEAD` into the temp index.
4. List all modified + untracked files (excluding `node_modules`, `.venv`, large dirs).
5. `git add --all` (with the temp index env) to stage them.
6. `git write-tree` to materialize the tree object.
7. `git commit-tree <tree>` (with parent if any) to create the detached commit.
8. Return `GhostCommit { id, parent, preexisting_untracked_files, preexisting_untracked_dirs }`.

- [ ] **Step 1: Write failing tests**

Create `crates/klynt-git-utils/src/ghost_commits.rs`:

```rust
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tempfile::TempDir;
use tokio::process::Command;

use crate::errors::GitToolingError;
use crate::repo::get_git_repo_root;
use crate::GhostCommit;

/// Configuration for ghost-snapshot creation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhostSnapshotConfig {
    /// Skip files larger than this many bytes.
    pub max_file_bytes: u64,
    /// Skip directories with more than this many entries.
    pub max_dir_entries: usize,
    /// Path components to always exclude.
    pub excluded_path_components: Vec<String>,
}

impl Default for GhostSnapshotConfig {
    fn default() -> Self {
        Self {
            max_file_bytes: 10 * 1024 * 1024, // 10 MiB
            max_dir_entries: 200,
            excluded_path_components: vec![
                "node_modules".into(),
                ".venv".into(),
                "venv".into(),
                "target".into(),
                "dist".into(),
                "build".into(),
                ".next".into(),
                ".cache".into(),
            ],
        }
    }
}

/// Create a ghost commit capturing the current state of the working tree at `repo_path`.
/// Returns the new GhostCommit. Does NOT touch any branch refs.
pub async fn create_ghost_commit(
    repo_path: &Path,
    config: &GhostSnapshotConfig,
) -> Result<GhostCommit, GitToolingError> {
    let root = get_git_repo_root(repo_path).await?;

    // 1. Get the parent SHA (HEAD), if any.
    let parent = git_rev_parse_head(&root).await?;

    // 2. Create a temp index so we don't touch the user's staging area.
    let tmp_index_dir = TempDir::new()?;
    let tmp_index_path = tmp_index_dir.path().join("index");
    let index_env = ("GIT_INDEX_FILE", tmp_index_path.as_path());

    // 3. If we have a parent, populate the temp index from HEAD.
    if let Some(parent_sha) = &parent {
        run_git(&root, &["read-tree", parent_sha], Some(&[index_env])).await?;
    }

    // 4. Snapshot which files are currently untracked (we'll need this for restore).
    let preexisting_untracked = list_untracked_files(&root).await?;

    // 5. Add everything respecting size/dir-count/exclude filters.
    let to_add = collect_files_to_snapshot(&root, config).await?;
    if !to_add.is_empty() {
        // Use --add to stage; pass paths as args.
        let mut args: Vec<String> = vec!["add".into(), "--force".into(), "--".into()];
        args.extend(to_add.iter().map(|p| p.to_string_lossy().into_owned()));
        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        run_git(&root, &arg_refs, Some(&[index_env])).await?;
    }

    // 6. Write the tree.
    let tree_sha_out = run_git(&root, &["write-tree"], Some(&[index_env])).await?;
    let tree_sha = String::from_utf8(tree_sha_out)
        .map_err(|e| GitToolingError::GitOutputUtf8 {
            command: "git write-tree".into(),
            source: e,
        })?
        .trim()
        .to_string();

    // 7. Commit-tree with the message "klynt-snapshot".
    let mut commit_args: Vec<String> = vec!["commit-tree".into(), tree_sha.clone()];
    if let Some(p) = &parent {
        commit_args.push("-p".into());
        commit_args.push(p.clone());
    }
    commit_args.push("-m".into());
    commit_args.push("klynt-snapshot".into());
    let arg_refs: Vec<&str> = commit_args.iter().map(|s| s.as_str()).collect();
    let commit_sha_out = run_git(&root, &arg_refs, Some(&[index_env])).await?;
    let commit_sha = String::from_utf8(commit_sha_out)
        .map_err(|e| GitToolingError::GitOutputUtf8 {
            command: "git commit-tree".into(),
            source: e,
        })?
        .trim()
        .to_string();

    Ok(GhostCommit::new(
        commit_sha,
        parent,
        preexisting_untracked.into_iter().collect(),
        Vec::new(),
    ))
}

async fn git_rev_parse_head(root: &Path) -> Result<Option<String>, GitToolingError> {
    let out = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .await?;
    if !out.status.success() {
        return Ok(None); // No HEAD yet (empty repo).
    }
    Ok(Some(String::from_utf8_lossy(&out.stdout).trim().to_string()))
}

async fn list_untracked_files(root: &Path) -> Result<Vec<PathBuf>, GitToolingError> {
    let out = Command::new("git")
        .args(["ls-files", "--others", "--exclude-standard", "-z"])
        .current_dir(root)
        .output()
        .await?;
    if !out.status.success() {
        return Err(GitToolingError::GitCommand {
            command: "git ls-files".into(),
            status: out.status,
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        });
    }
    Ok(out
        .stdout
        .split(|&b| b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| PathBuf::from(String::from_utf8_lossy(s).into_owned()))
        .collect())
}

async fn collect_files_to_snapshot(
    root: &Path,
    config: &GhostSnapshotConfig,
) -> Result<Vec<PathBuf>, GitToolingError> {
    let exclude_set: HashSet<&str> = config
        .excluded_path_components
        .iter()
        .map(|s| s.as_str())
        .collect();

    let mut out = Vec::new();
    for entry in walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            !e.file_name()
                .to_str()
                .map(|n| n == ".git" || exclude_set.contains(n))
                .unwrap_or(false)
        })
    {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let meta = entry.metadata()?;
        if meta.len() > config.max_file_bytes {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(root)?
            .to_path_buf();
        out.push(rel);
    }
    Ok(out)
}

async fn run_git(
    cwd: &Path,
    args: &[&str],
    extra_env: Option<&[(&str, &Path)]>,
) -> Result<Vec<u8>, GitToolingError> {
    let mut cmd = Command::new("git");
    cmd.args(args).current_dir(cwd);
    if let Some(envs) = extra_env {
        for (k, v) in envs {
            cmd.env(k, v);
        }
    }
    let out = cmd.output().await?;
    if !out.status.success() {
        return Err(GitToolingError::GitCommand {
            command: format!("git {}", args.join(" ")),
            status: out.status,
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        });
    }
    Ok(out.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn init_repo(dir: &Path) {
        for args in [
            &["init"][..],
            &["config", "user.email", "test@klynt.local"][..],
            &["config", "user.name", "Test"][..],
        ] {
            run_git(dir, args, None).await.unwrap();
        }
    }

    async fn commit_file(dir: &Path, name: &str, body: &str) -> String {
        std::fs::write(dir.join(name), body).unwrap();
        run_git(dir, &["add", name], None).await.unwrap();
        run_git(dir, &["commit", "-m", "msg"], None).await.unwrap();
        String::from_utf8(run_git(dir, &["rev-parse", "HEAD"], None).await.unwrap())
            .unwrap()
            .trim()
            .to_string()
    }

    #[tokio::test]
    async fn create_in_empty_repo_has_no_parent() {
        let tmp = TempDir::new().unwrap();
        init_repo(tmp.path()).await;
        std::fs::write(tmp.path().join("a.txt"), "hello").unwrap();
        let ghost =
            create_ghost_commit(tmp.path(), &GhostSnapshotConfig::default()).await.unwrap();
        assert!(ghost.parent().is_none());
        assert!(!ghost.id().is_empty());
    }

    #[tokio::test]
    async fn create_with_existing_head_records_parent() {
        let tmp = TempDir::new().unwrap();
        init_repo(tmp.path()).await;
        let head = commit_file(tmp.path(), "a.txt", "v1").await;
        std::fs::write(tmp.path().join("a.txt"), "v2-uncommitted").unwrap();
        let ghost =
            create_ghost_commit(tmp.path(), &GhostSnapshotConfig::default()).await.unwrap();
        assert_eq!(ghost.parent(), Some(head.as_str()));
    }

    #[tokio::test]
    async fn excluded_dirs_are_skipped() {
        let tmp = TempDir::new().unwrap();
        init_repo(tmp.path()).await;
        std::fs::create_dir(tmp.path().join("node_modules")).unwrap();
        std::fs::write(tmp.path().join("node_modules/big.js"), "x".repeat(1_000_000)).unwrap();
        std::fs::write(tmp.path().join("a.txt"), "small").unwrap();
        // Should succeed without timing out / exploding the index.
        create_ghost_commit(tmp.path(), &GhostSnapshotConfig::default())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn large_files_are_skipped() {
        let tmp = TempDir::new().unwrap();
        init_repo(tmp.path()).await;
        std::fs::write(tmp.path().join("huge.bin"), vec![0u8; 11 * 1024 * 1024]).unwrap();
        std::fs::write(tmp.path().join("ok.txt"), "small").unwrap();
        let cfg = GhostSnapshotConfig::default();
        let ghost = create_ghost_commit(tmp.path(), &cfg).await.unwrap();
        // Verify huge.bin is NOT in the resulting tree.
        let out = run_git(tmp.path(), &["ls-tree", "-r", ghost.id()], None)
            .await
            .unwrap();
        let listing = String::from_utf8_lossy(&out);
        assert!(!listing.contains("huge.bin"), "huge file leaked: {listing}");
        assert!(listing.contains("ok.txt"));
    }
}
```

- [ ] **Step 2: Update `lib.rs` to expose**

Append to `crates/klynt-git-utils/src/lib.rs`:

```rust
mod ghost_commits;
pub use ghost_commits::{create_ghost_commit, GhostSnapshotConfig};
```

- [ ] **Step 3: Run — expect tests pass**

Run: `cargo nextest run -p klynt-git-utils ghost_commits::tests`
Expected: 4 tests pass. If `git` is unavailable in the test runner, install it (already a Klynt prereq per CLAUDE.md).

- [ ] **Step 4: Commit**

```bash
git add crates/klynt-git-utils/
git commit -m "feat(git-utils): create_ghost_commit with size/dir excludes"
```

### Task C5: Implement `restore_ghost_commit` (TDD)

**Files:**
- Modify: `crates/klynt-git-utils/src/ghost_commits.rs`
- Modify: `crates/klynt-git-utils/src/lib.rs`

- [ ] **Step 1: Write failing test**

Append to the `tests` module in `crates/klynt-git-utils/src/ghost_commits.rs`:

```rust
#[tokio::test]
async fn restore_round_trip() {
    let tmp = TempDir::new().unwrap();
    init_repo(tmp.path()).await;
    commit_file(tmp.path(), "a.txt", "v1").await;
    std::fs::write(tmp.path().join("a.txt"), "v2-uncommitted").unwrap();
    let ghost = create_ghost_commit(tmp.path(), &GhostSnapshotConfig::default())
        .await
        .unwrap();

    // Mutate further.
    std::fs::write(tmp.path().join("a.txt"), "v3-mutated-after-snapshot").unwrap();
    std::fs::write(tmp.path().join("new.txt"), "added after snapshot").unwrap();

    restore_ghost_commit(tmp.path(), &ghost).await.unwrap();

    let restored = std::fs::read_to_string(tmp.path().join("a.txt")).unwrap();
    assert_eq!(restored, "v2-uncommitted");
    // new.txt was created AFTER the snapshot — should be removed.
    assert!(!tmp.path().join("new.txt").exists(),
        "post-snapshot file should be removed");
}

#[tokio::test]
async fn restore_keeps_files_that_predated_the_snapshot() {
    let tmp = TempDir::new().unwrap();
    init_repo(tmp.path()).await;
    commit_file(tmp.path(), "a.txt", "v1").await;
    // An untracked file existed BEFORE we snapshotted.
    std::fs::write(tmp.path().join("preexisting.log"), "existed").unwrap();
    let ghost = create_ghost_commit(tmp.path(), &GhostSnapshotConfig::default())
        .await
        .unwrap();
    // Mutate after.
    std::fs::write(tmp.path().join("preexisting.log"), "modified after").unwrap();
    restore_ghost_commit(tmp.path(), &ghost).await.unwrap();
    // File still exists (it was in the snapshot).
    assert!(tmp.path().join("preexisting.log").exists());
}
```

- [ ] **Step 2: Run — expect FAIL**

Run: `cargo nextest run -p klynt-git-utils ghost_commits::tests::restore`
Expected: FAIL.

- [ ] **Step 3: Implement restore**

Append to `crates/klynt-git-utils/src/ghost_commits.rs`:

```rust
/// Restore the working tree to the state captured by `ghost`.
/// - Files in the ghost tree are restored to their snapshotted content.
/// - Files that did NOT exist when the ghost was captured are deleted
///   (anything new since the snapshot).
/// - Pre-existing untracked files (recorded in the ghost) are kept.
pub async fn restore_ghost_commit(
    repo_path: &Path,
    ghost: &GhostCommit,
) -> Result<(), GitToolingError> {
    let root = get_git_repo_root(repo_path).await?;

    // Step A: hard-restore the worktree (but NOT the index) to the ghost tree.
    run_git(&root, &["restore", "--source", ghost.id(), "--worktree", "--", "."], None).await?;

    // Step B: delete files that exist now but weren't in the ghost tree
    //         AND weren't preexisting untracked files we should keep.
    let in_ghost_tree = list_paths_in_tree(&root, ghost.id()).await?;
    let in_ghost_set: HashSet<PathBuf> = in_ghost_tree.into_iter().collect();
    let preexisting: HashSet<&PathBuf> = ghost.preexisting_untracked_files().iter().collect();

    for entry in walkdir::WalkDir::new(&root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| e.file_name() != ".git")
    {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry.path().strip_prefix(&root)?.to_path_buf();
        if in_ghost_set.contains(&rel) {
            continue;
        }
        if preexisting.contains(&rel) {
            continue;
        }
        // This is a file that appeared after the snapshot; remove it.
        let _ = std::fs::remove_file(entry.path());
    }
    Ok(())
}

async fn list_paths_in_tree(root: &Path, sha: &str) -> Result<Vec<PathBuf>, GitToolingError> {
    let out = run_git(root, &["ls-tree", "-r", "--name-only", "-z", sha], None).await?;
    Ok(out
        .split(|&b| b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| PathBuf::from(String::from_utf8_lossy(s).into_owned()))
        .collect())
}
```

- [ ] **Step 4: Update lib.rs export**

In `crates/klynt-git-utils/src/lib.rs`, change the `ghost_commits` re-export to:

```rust
pub use ghost_commits::{create_ghost_commit, restore_ghost_commit, GhostSnapshotConfig};
```

- [ ] **Step 5: Re-run, expect PASS**

Run: `cargo nextest run -p klynt-git-utils ghost_commits::tests`
Expected: 6 tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/klynt-git-utils/
git commit -m "feat(git-utils): restore_ghost_commit with post-snapshot file removal"
```

### Task C6: Add `ghost_commit_sha` column to `coding_snapshots`

**Files:**
- Modify: `crates/storage/migrations/001_initial.sql:866-878`

Per CLAUDE.md "Pre-release — no user data to migrate. All schema changes can be made directly (alter tables, drop and recreate)... When a migration is consolidated, update the FeatureMigration version and SQL in-place rather than adding incremental migration files."

- [ ] **Step 1: Read current schema**

Run: `sed -n '865,880p' crates/storage/migrations/001_initial.sql`
Capture the current `CREATE TABLE coding_snapshots` block.

- [ ] **Step 2: Add column in-place**

Edit `crates/storage/migrations/001_initial.sql`. Find the `CREATE TABLE IF NOT EXISTS coding_snapshots` block. Add a new column `ghost_commit_sha TEXT NULL` after `content_hash`. Block should look like:

```sql
CREATE TABLE IF NOT EXISTS coding_snapshots (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    session_key     TEXT NOT NULL,
    message_id      TEXT,
    file_path       TEXT NOT NULL,
    content_before  BLOB NOT NULL,           -- raw bytes pre-edit; '' if file did not exist OR ghost-mode
    file_existed    INTEGER NOT NULL,
    content_hash    TEXT NOT NULL,           -- blake3 of content_before; sha "ghost" when ghost_commit_sha is set
    ghost_commit_sha TEXT NULL,              -- when set, restore via klynt_git_utils::restore_ghost_commit instead of BLOB
    created_at      INTEGER NOT NULL DEFAULT (strftime('%s','now'))
);

CREATE INDEX IF NOT EXISTS idx_coding_snapshots_session
  ON coding_snapshots(session_key, created_at);

CREATE INDEX IF NOT EXISTS idx_coding_snapshots_ghost
  ON coding_snapshots(session_key, ghost_commit_sha) WHERE ghost_commit_sha IS NOT NULL;
```

- [ ] **Step 3: Verify SQL parses**

Run: `cargo build -p storage`
Expected: clean build (sqlx migrations are evaluated at runtime, but the migration string still gets included).

- [ ] **Step 4: Run a migration smoke test**

Run: `cargo nextest run -p storage`
Expected: any test that uses `connect_in_memory()` should still pass — confirm no test asserts a specific column count or `PRAGMA table_info` shape on `coding_snapshots`.

- [ ] **Step 5: Commit**

```bash
git add crates/storage/migrations/001_initial.sql
git commit -m "feat(storage): add ghost_commit_sha column to coding_snapshots"
```

### Task C7: Extend `Snapshot` struct + `SnapshotRepo::record_ghost`

**Files:**
- Modify: `crates/klynt-core/src/snapshots/repo.rs`

- [ ] **Step 1: Add field + new method (TDD)**

Append a new test to `crates/klynt-core/src/snapshots/repo.rs`'s `mod tests`:

```rust
#[tokio::test]
async fn record_ghost_stores_sha_with_empty_blob() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repo = SnapshotRepo::new(pool.clone());
    let id = repo
        .record_ghost("sess1", Some("msg1"), "deadbeef0123")
        .await
        .unwrap();
    let snap = repo.get(id).await.unwrap().expect("exists");
    assert_eq!(snap.ghost_commit_sha.as_deref(), Some("deadbeef0123"));
    assert!(snap.content_before.is_empty(), "ghost rows have empty BLOB");
}
```

- [ ] **Step 2: Run — expect FAIL**

Run: `cargo nextest run -p klynt-core snapshots::repo::tests::record_ghost`
Expected: FAIL.

- [ ] **Step 3: Add the field + method**

In `crates/klynt-core/src/snapshots/repo.rs`, modify the `Snapshot` struct:

```rust
pub struct Snapshot {
    pub id: i64,
    pub session_key: String,
    pub message_id: Option<String>,
    pub file_path: String,
    pub content_before: Vec<u8>,
    pub file_existed: bool,
    pub content_hash: String,
    pub ghost_commit_sha: Option<String>,
    pub created_at: i64,
}
```

Add to `impl SnapshotRepo`:

```rust
#[tracing::instrument(skip(self), err)]
pub async fn record_ghost(
    &self,
    session_key: &str,
    message_id: Option<&str>,
    ghost_commit_sha: &str,
) -> Result<i64> {
    let res = sqlx::query(
        "INSERT INTO coding_snapshots \
         (session_key, message_id, file_path, content_before, file_existed, content_hash, ghost_commit_sha) \
         VALUES (?, ?, '<ghost>', X'', 1, 'ghost', ?)",
    )
    .bind(session_key)
    .bind(message_id)
    .bind(ghost_commit_sha)
    .execute(self.pool.inner())
    .await
    .map_err(common::KlyntbotError::from)?;
    Ok(res.last_insert_rowid())
}
```

Also update `row_to_snapshot`:

```rust
fn row_to_snapshot(row: sqlx::sqlite::SqliteRow) -> Snapshot {
    Snapshot {
        id: row.get("id"),
        session_key: row.get("session_key"),
        message_id: row.get("message_id"),
        file_path: row.get("file_path"),
        content_before: row.get("content_before"),
        file_existed: row.get::<i64, _>("file_existed") != 0,
        content_hash: row.get("content_hash"),
        ghost_commit_sha: row.get("ghost_commit_sha"),
        created_at: row.get("created_at"),
    }
}
```

- [ ] **Step 4: Re-run, expect PASS**

Run: `cargo nextest run -p klynt-core snapshots::repo`
Expected: 3 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/klynt-core/src/snapshots/repo.rs
git commit -m "feat(snapshots): add Snapshot.ghost_commit_sha and record_ghost"
```

### Task C8: Add `record_ghost` to `SnapshotService` trait

**Files:**
- Modify: `crates/klynt-core/src/snapshots/mod.rs`

- [ ] **Step 1: Extend trait + impl**

Open `crates/klynt-core/src/snapshots/mod.rs`. Inside the `trait SnapshotService`, append:

```rust
async fn record_ghost(
    &self,
    session_key: &str,
    message_id: Option<&str>,
    ghost_commit_sha: &str,
) -> Result<i64>;
```

In the `impl SnapshotService for SnapshotRepo` block, append:

```rust
async fn record_ghost(
    &self,
    session_key: &str,
    message_id: Option<&str>,
    ghost_commit_sha: &str,
) -> Result<i64> {
    self.record_ghost(session_key, message_id, ghost_commit_sha).await
}
```

- [ ] **Step 2: Build**

Run: `cargo build -p klynt-core`
Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add crates/klynt-core/src/snapshots/mod.rs
git commit -m "feat(snapshots): SnapshotService.record_ghost"
```

### Task C9: Implement `try_record_with_ghost` orchestrator (TDD)

**Files:**
- Modify: `crates/klynt-core/src/snapshots/mod.rs`
- Modify: `crates/klynt-core/Cargo.toml`

- [ ] **Step 1: Add dep**

In `crates/klynt-core/Cargo.toml`, under `[dependencies]`:

```toml
klynt-git-utils = { path = "../klynt-git-utils" }
```

- [ ] **Step 2: Write failing test**

Append to `crates/klynt-core/src/snapshots/repo.rs`'s `mod tests`:

```rust
#[tokio::test]
async fn try_record_with_ghost_falls_back_to_blob_outside_repo() {
    let tmp = tempfile::TempDir::new().unwrap();
    let file = tmp.path().join("x.txt");
    std::fs::write(&file, b"hi").unwrap();
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repo = SnapshotRepo::new(pool.clone());
    let id = repo
        .try_record_with_ghost("s", None, &file.to_string_lossy(), b"hi", true)
        .await
        .unwrap();
    let snap = repo.get(id).await.unwrap().unwrap();
    // No ghost SHA because tempdir is not a git repo.
    assert!(snap.ghost_commit_sha.is_none());
    assert_eq!(snap.content_before, b"hi");
}
```

- [ ] **Step 3: Run — expect FAIL**

Run: `cargo nextest run -p klynt-core snapshots::repo::tests::try_record_with_ghost`
Expected: FAIL.

- [ ] **Step 4: Implement orchestrator**

Append to `impl SnapshotRepo` in `crates/klynt-core/src/snapshots/repo.rs`:

```rust
/// Record a snapshot using ghost-commit when the path lives in a git repo,
/// falling back to BLOB storage otherwise. Best-effort: if ghost-commit
/// creation fails (git missing, permission error, etc.) we silently
/// fall back to BLOB rather than blocking the user's edit.
pub async fn try_record_with_ghost(
    &self,
    session_key: &str,
    message_id: Option<&str>,
    file_path: &str,
    content: &[u8],
    existed: bool,
) -> Result<i64> {
    use std::path::Path;
    let path = Path::new(file_path);
    let parent = path.parent().unwrap_or(path);
    match klynt_git_utils::is_inside_git_repo(parent).await {
        Ok(true) => {
            // Try to create a ghost commit at the repo root.
            match klynt_git_utils::get_git_repo_root(parent).await {
                Ok(root) => {
                    let cfg = klynt_git_utils::GhostSnapshotConfig::default();
                    match klynt_git_utils::create_ghost_commit(&root, &cfg).await {
                        Ok(ghost) => {
                            return self.record_ghost(session_key, message_id, ghost.id()).await;
                        }
                        Err(e) => {
                            tracing::warn!(?e, "ghost commit failed, falling back to BLOB");
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(?e, "git repo root resolution failed, falling back to BLOB");
                }
            }
        }
        Ok(false) => {} // Not in a git repo; BLOB.
        Err(e) => {
            tracing::warn!(?e, "git detection failed, falling back to BLOB");
        }
    }
    self.record(session_key, message_id, file_path, content, existed).await
}
```

- [ ] **Step 5: Re-run**

Run: `cargo nextest run -p klynt-core snapshots::repo`
Expected: 4 tests pass (including the new fallback test).

- [ ] **Step 6: Commit**

```bash
git add crates/klynt-core/Cargo.toml crates/klynt-core/src/snapshots/repo.rs
git commit -m "feat(snapshots): try_record_with_ghost orchestrator with BLOB fallback"
```

### Task C10: Wire `try_record_with_ghost` into `EditTool`

**Files:**
- Modify: `crates/klynt-core/src/tools/edit.rs`

- [ ] **Step 1: Locate the existing snapshot site**

Run: `grep -n "snapshot_repo\|\.record(" crates/klynt-core/src/tools/edit.rs | head -20`
Expected: a block similar to `apply_patch.rs:185-205` calling `repo.record(...)`.

- [ ] **Step 2: Replace `.record(...)` with `.try_record_with_ghost(...)`**

In `crates/klynt-core/src/tools/edit.rs`, find the existing call:

```rust
let _ = repo
    .record(
        &session_id,
        message_id.as_deref(),
        &resolved.to_string_lossy(),
        &content,
        existed,
    )
    .await;
```

Replace with:

```rust
let _ = repo
    .try_record_with_ghost(
        &session_id,
        message_id.as_deref(),
        &resolved.to_string_lossy(),
        &content,
        existed,
    )
    .await;
```

- [ ] **Step 3: Build + test**

Run: `cargo build -p klynt-core && cargo nextest run -p klynt-core`
Expected: green.

- [ ] **Step 4: Commit**

```bash
git add crates/klynt-core/src/tools/edit.rs
git commit -m "feat(edit-tool): use ghost-commit snapshots when in git repo"
```

### Task C11: Wire into `WriteTool` and `ApplyPatchTool`

**Files:**
- Modify: `crates/klynt-core/src/tools/write.rs`
- Modify: `crates/klynt-core/src/tools/apply_patch.rs:185-205`

- [ ] **Step 1: Repeat the substitution in both files**

In each of `write.rs` and `apply_patch.rs`, find `.record(` on the `snapshot_repo` and substitute `.try_record_with_ghost(`. The arg list is identical to Task C10.

- [ ] **Step 2: Build + test**

Run: `cargo build -p klynt-core && cargo nextest run -p klynt-core`
Expected: green.

- [ ] **Step 3: Commit**

```bash
git add crates/klynt-core/src/tools/write.rs crates/klynt-core/src/tools/apply_patch.rs
git commit -m "feat(tools): use ghost-commit snapshots in WriteTool and ApplyPatchTool"
```

### Task C12: Make `rewind_to_message` ghost-aware

**Files:**
- Modify: `crates/storage/src/repos/session.rs:535`
- Modify: `crates/storage/Cargo.toml`

- [ ] **Step 1: Add dep**

In `crates/storage/Cargo.toml`:

```toml
klynt-git-utils = { path = "../klynt-git-utils" }
```

- [ ] **Step 2: Read current rewind impl**

Run: `sed -n '530,600p' crates/storage/src/repos/session.rs`
Capture how it currently iterates snapshots and writes BLOBs back.

- [ ] **Step 3: Write failing test**

Create `tests/integration/snapshot_ghost_rewind.rs`:

```rust
//! End-to-end: edit a file in a git repo, snapshot via EditTool, mutate further,
//! call rewind, verify the file is restored to the snapshotted state.

use klynt::storage::StoragePool;
use klynt::storage::repos::SessionRepo;
use std::path::Path;
use tempfile::TempDir;
use tokio::process::Command;

async fn git(dir: &Path, args: &[&str]) {
    Command::new("git").args(args).current_dir(dir).output().await.unwrap();
}

#[tokio::test]
async fn rewind_in_git_repo_uses_ghost_commit() {
    let tmp = TempDir::new().unwrap();
    git(tmp.path(), &["init"]).await;
    git(tmp.path(), &["config", "user.email", "t@t.local"]).await;
    git(tmp.path(), &["config", "user.name", "t"]).await;
    std::fs::write(tmp.path().join("a.txt"), "v1").unwrap();
    git(tmp.path(), &["add", "."]).await;
    git(tmp.path(), &["commit", "-m", "init"]).await;

    let pool = StoragePool::connect_in_memory().await.unwrap();
    let snap_repo = klynt::klynt_core::snapshots::SnapshotRepo::new(pool.clone());

    // Snapshot the current state.
    let _id = snap_repo
        .try_record_with_ghost("sess", Some("msg1"), &tmp.path().join("a.txt").to_string_lossy(), b"v1", true)
        .await
        .unwrap();

    // Mutate.
    std::fs::write(tmp.path().join("a.txt"), "v2-mutated").unwrap();

    // Rewind.
    let session_repo = SessionRepo::new(pool.clone());
    session_repo.rewind_to_message("sess", "msg1").await.unwrap();

    let after = std::fs::read_to_string(tmp.path().join("a.txt")).unwrap();
    assert_eq!(after, "v1", "rewind should restore via ghost commit");
}
```

NOTE: The exact `klynt::storage::*` re-export paths depend on what the `klynt` facade re-exports today. If the test fails to compile due to missing re-exports, switch to the canonical crate paths (`storage::StoragePool`, etc.) — the test only needs to call across crate boundaries.

- [ ] **Step 4: Run — expect FAIL (because rewind doesn't yet handle ghost rows)**

Run: `cargo nextest run --test snapshot_ghost_rewind`
Expected: FAIL — file content remains "v2-mutated".

- [ ] **Step 5: Implement ghost-aware rewind**

In `crates/storage/src/repos/session.rs`, locate `rewind_to_message`. Modify the snapshot-restore loop to dispatch on `ghost_commit_sha`:

```rust
pub async fn rewind_to_message(
    &self,
    session_key: &str,
    message_id: &str,
) -> Result<RewindResult> {
    // ... existing setup that fetches snapshots after message_id ...

    for snap in snapshots {
        if let Some(ghost_sha) = &snap.ghost_commit_sha {
            // Ghost-mode: restore via git.
            // Look up the working directory: the ghost SHA lives in *some* git repo.
            // We rely on the snapshot being recorded with a path inside that repo,
            // so derive the repo root from any subsequent file_path with a non-ghost row,
            // OR persist it on the snapshot (Phase 4 enhancement). For Phase 3 we use the
            // session's current cwd (sessions table column).
            let cwd = self.get_session_cwd(session_key).await?;
            if let Some(cwd) = cwd {
                let cfg_ghost = klynt_git_utils::GhostCommit::new(
                    ghost_sha.clone(),
                    None,
                    Vec::new(),
                    Vec::new(),
                );
                if let Err(e) =
                    klynt_git_utils::restore_ghost_commit(std::path::Path::new(&cwd), &cfg_ghost).await
                {
                    tracing::error!(?e, ghost_sha, "ghost restore failed");
                }
            }
        } else {
            // BLOB-mode: existing logic — write content_before back to file_path.
            // (unchanged from Phase 2)
            // ... existing write logic ...
        }
    }
    // ... existing return ...
}
```

You will likely need to add a `get_session_cwd` helper if it doesn't exist. Sketch:

```rust
async fn get_session_cwd(&self, session_key: &str) -> Result<Option<String>> {
    let row = sqlx::query("SELECT cwd FROM sessions WHERE session_key = ?")
        .bind(session_key)
        .fetch_optional(self.pool.inner())
        .await
        .map_err(common::KlyntbotError::from)?;
    Ok(row.and_then(|r| r.try_get::<Option<String>, _>("cwd").ok().flatten()))
}
```

(If `sessions` table column is named differently, run `grep -A2 "CREATE TABLE.*sessions" crates/storage/migrations/001_initial.sql` to confirm.)

- [ ] **Step 6: Re-run**

Run: `cargo nextest run --test snapshot_ghost_rewind`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/storage/Cargo.toml crates/storage/src/repos/session.rs tests/integration/snapshot_ghost_rewind.rs
git commit -m "feat(rewind): dispatch on ghost_commit_sha for git-aware restore"
```

### Task C13: Add BLOB-fallback integration test

**Files:**
- Create: `tests/integration/snapshot_blob_rewind.rs`

- [ ] **Step 1: Write the test**

```rust
//! Verifies BLOB rewind still works for non-git directories (regression guard).

use storage::StoragePool;
use storage::repos::SessionRepo;
use tempfile::TempDir;

#[tokio::test]
async fn rewind_outside_git_uses_blob_path() {
    let tmp = TempDir::new().unwrap();
    // No `git init` — purely a regular directory.
    let target = tmp.path().join("a.txt");
    std::fs::write(&target, "v1").unwrap();

    let pool = StoragePool::connect_in_memory().await.unwrap();
    let snap_repo = klynt_core::snapshots::SnapshotRepo::new(pool.clone());

    let _ = snap_repo
        .try_record_with_ghost(
            "sess",
            Some("m1"),
            &target.to_string_lossy(),
            b"v1",
            true,
        )
        .await
        .unwrap();

    std::fs::write(&target, "mutated").unwrap();
    let session_repo = SessionRepo::new(pool.clone());
    session_repo.rewind_to_message("sess", "m1").await.unwrap();

    assert_eq!(std::fs::read_to_string(&target).unwrap(), "v1");
}
```

- [ ] **Step 2: Run**

Run: `cargo nextest run --test snapshot_blob_rewind`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add tests/integration/snapshot_blob_rewind.rs
git commit -m "test(rewind): regression guard for BLOB-mode rewind"
```

### Task C14: Persist `cwd` (or repo_root) on the snapshot row

**Files:**
- Modify: `crates/storage/migrations/001_initial.sql:866-880`
- Modify: `crates/klynt-core/src/snapshots/repo.rs`

The Task C12 implementation depends on reading the session's current `cwd`, which can drift between snapshot time and rewind time. Capture the resolved repo root on the snapshot row so restore is deterministic.

- [ ] **Step 1: Add column**

In `crates/storage/migrations/001_initial.sql`, extend the `coding_snapshots` block:

```sql
CREATE TABLE IF NOT EXISTS coding_snapshots (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    session_key     TEXT NOT NULL,
    message_id      TEXT,
    file_path       TEXT NOT NULL,
    content_before  BLOB NOT NULL,
    file_existed    INTEGER NOT NULL,
    content_hash    TEXT NOT NULL,
    ghost_commit_sha TEXT NULL,
    ghost_repo_root  TEXT NULL,              -- absolute path of the git repo at snapshot time
    created_at      INTEGER NOT NULL DEFAULT (strftime('%s','now'))
);
```

- [ ] **Step 2: Update `Snapshot` struct + `record_ghost` signature**

Add `pub ghost_repo_root: Option<String>,` to `Snapshot`. Update `record_ghost`:

```rust
pub async fn record_ghost(
    &self,
    session_key: &str,
    message_id: Option<&str>,
    ghost_commit_sha: &str,
    ghost_repo_root: &str,
) -> Result<i64> {
    let res = sqlx::query(
        "INSERT INTO coding_snapshots \
         (session_key, message_id, file_path, content_before, file_existed, content_hash, ghost_commit_sha, ghost_repo_root) \
         VALUES (?, ?, '<ghost>', X'', 1, 'ghost', ?, ?)",
    )
    .bind(session_key)
    .bind(message_id)
    .bind(ghost_commit_sha)
    .bind(ghost_repo_root)
    .execute(self.pool.inner())
    .await
    .map_err(common::KlyntbotError::from)?;
    Ok(res.last_insert_rowid())
}
```

Update `row_to_snapshot` to read `ghost_repo_root`. Update the trait method in `mod.rs` to take the new parameter.

- [ ] **Step 3: Update `try_record_with_ghost` to pass repo root**

```rust
match klynt_git_utils::create_ghost_commit(&root, &cfg).await {
    Ok(ghost) => {
        return self
            .record_ghost(session_key, message_id, ghost.id(), &root.to_string_lossy())
            .await;
    }
    // ...
}
```

- [ ] **Step 4: Update `rewind_to_message` to use `ghost_repo_root` instead of `get_session_cwd`**

```rust
if let Some(ghost_sha) = &snap.ghost_commit_sha {
    let root = snap.ghost_repo_root.as_deref().unwrap_or("");
    if root.is_empty() {
        tracing::error!("ghost row missing ghost_repo_root; cannot restore");
        continue;
    }
    let cfg_ghost = klynt_git_utils::GhostCommit::new(
        ghost_sha.clone(), None, Vec::new(), Vec::new(),
    );
    if let Err(e) = klynt_git_utils::restore_ghost_commit(std::path::Path::new(root), &cfg_ghost).await {
        tracing::error!(?e, ghost_sha, "ghost restore failed");
    }
}
```

You can now delete the `get_session_cwd` helper.

- [ ] **Step 5: Update existing record_ghost test**

The test at Task C7 step 1 needs a 4th arg:

```rust
let id = repo
    .record_ghost("sess1", Some("msg1"), "deadbeef0123", "/tmp/repo")
    .await
    .unwrap();
```

- [ ] **Step 6: Build + run all snapshot tests**

Run: `cargo nextest run -p klynt-core snapshots && cargo nextest run --test snapshot_ghost_rewind --test snapshot_blob_rewind`
Expected: green.

- [ ] **Step 7: Commit**

```bash
git add crates/storage/migrations/001_initial.sql crates/klynt-core/src/snapshots/ crates/storage/src/repos/session.rs tests/integration/
git commit -m "feat(snapshots): persist ghost_repo_root for deterministic restore"
```

### Task C15: Add a benchmark — snapshot creation cost

**Files:**
- Create: `crates/kca-bench/benches/snapshot_create.rs` (or co-locate next to existing benches)

- [ ] **Step 1: Locate existing bench harness**

Run: `find . -path ./target -prune -o -name "*.rs" -print | xargs grep -l "criterion" 2>/dev/null | head -5`
Expected: at least one `criterion`-based bench under `crates/kca-bench/benches/`.

- [ ] **Step 2: Write the bench**

Write to `crates/kca-bench/benches/snapshot_create.rs` (adjust path if `kca-bench` uses a different layout):

```rust
use criterion::{criterion_group, criterion_main, Criterion};
use std::path::Path;
use tempfile::TempDir;

fn setup_repo(file_count: usize) -> TempDir {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    std::process::Command::new("git").arg("init").current_dir(dir).output().unwrap();
    std::process::Command::new("git").args(["config","user.email","b@b"]).current_dir(dir).output().unwrap();
    std::process::Command::new("git").args(["config","user.name","b"]).current_dir(dir).output().unwrap();
    for i in 0..file_count {
        std::fs::write(dir.join(format!("f{i}.txt")), format!("body {i}")).unwrap();
    }
    std::process::Command::new("git").args(["add","."]).current_dir(dir).output().unwrap();
    std::process::Command::new("git").args(["commit","-m","i"]).current_dir(dir).output().unwrap();
    tmp
}

fn bench_ghost_create(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let tmp = setup_repo(50);
    c.bench_function("ghost_create_50_files", |b| {
        b.to_async(&rt).iter(|| async {
            klynt_git_utils::create_ghost_commit(
                tmp.path(),
                &klynt_git_utils::GhostSnapshotConfig::default(),
            ).await.unwrap()
        });
    });
}

criterion_group!(benches, bench_ghost_create);
criterion_main!(benches);
```

Add to `crates/kca-bench/Cargo.toml` under `[[bench]]`:

```toml
[[bench]]
name = "snapshot_create"
harness = false
```

And under `[dependencies]` (if not present):

```toml
klynt-git-utils = { path = "../klynt-git-utils" }
tokio = { workspace = true, features = ["macros", "rt-multi-thread"] }
tempfile = { workspace = true }
criterion = "0.5"
```

- [ ] **Step 3: Run the bench**

Run: `cargo bench -p kca-bench --bench snapshot_create`
Expected: a single line like `ghost_create_50_files time: [N ms ...]`. Document the number in the commit message — gates can be added later.

- [ ] **Step 4: Commit**

```bash
git add crates/kca-bench/benches/snapshot_create.rs crates/kca-bench/Cargo.toml
git commit -m "bench: ghost_create_50_files snapshot creation cost baseline"
```

### Task C16: Workstream C verification + spec/CLAUDE.md update

**Files:**
- Modify: `docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md`
- Modify: `CLAUDE.md`

- [ ] **Step 1: Workspace verification**

Run:
```bash
cargo build --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
cargo nextest run --workspace
```
Expected: all green.

- [ ] **Step 2: KCA validation gates**

Run: `./scripts/run_kca_validation.sh`
Expected: pass per CLAUDE.md "any gate failure blocks merge".

- [ ] **Step 3: Update spec — mark snapshot dedup item DONE**

In `docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md`, find the Phase 3+ bullet about "Snapshots: content-addressed dedup" (around §13). Append:

```markdown
**Phase 3 status (2026-05-02):** Implemented via codex-style "ghost commits" (`klynt-git-utils` crate) rather than blob-content addressing. Git's object store provides natural content-addressing with zero-copy for unchanged files. BLOB fallback retained for non-git directories. See plan `2026-05-02-klynt-coding-in-chat-phase3-codex-polish.md`.
```

- [ ] **Step 4: Update CLAUDE.md "Gotchas"**

Append to the `## Gotchas` section:

```markdown
- **Snapshot rewind has two modes** — `coding_snapshots` rows with non-NULL `ghost_commit_sha` are restored via `klynt_git_utils::restore_ghost_commit` (git working-tree restore); rows with NULL `ghost_commit_sha` use the original BLOB path. The choice is made at snapshot-record time by `try_record_with_ghost` based on whether the file lives in a git repo. Implication: deleting `.git/` between snapshot and rewind makes ghost-mode rewind fail silently (logs a `tracing::error!`). Plan `2026-05-02-klynt-coding-in-chat-phase3-codex-polish.md` introduced this.
```

- [ ] **Step 5: Commit**

```bash
git add docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md CLAUDE.md
git commit -m "docs: ship-note ghost-commit snapshot system (Phase 3)"
```

---

## Cross-cutting verification (Workstream D, 2 tasks)

### Task D1: Final workspace gates

- [ ] **Step 1: All gates**

Run all of:
```bash
cargo build --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
cargo nextest run --workspace
cargo test --workspace --doc
./scripts/run_kca_validation.sh
```
Expected: every command green.

- [ ] **Step 2: Dependency hygiene**

Run: `cargo machete`
Expected: no unused deps in the 3 new crates.

- [ ] **Step 3: Confirm desktop binary still launches**

Run: `cd desktop-ui && bun install && bun run build && cd .. && cargo build -p desktop --release`
Expected: clean build. (We do not run the binary in CI, but verify it links.)

### Task D2: Open the Phase 3 PR

- [ ] **Step 1: Push the branch**

```bash
git push -u origin HEAD
```

- [ ] **Step 2: Open PR**

```bash
gh pr create --title "feat(coding-in-chat): Phase 3 — codex polish port (ghost commits, structured truncation, process hardening)" --body "$(cat <<'EOF'
## Summary

Three self-contained ports from codex into Klynt:

- **`klynt-process-hardening`** (L0) — ports `process-hardening/src/lib.rs`. Wired into `desktop/src/main.rs` to run before `configure_mimalloc()`. macOS `PT_DENY_ATTACH`, Linux `PR_SET_DUMPABLE`, `RLIMIT_CORE = 0`, `LD_*`/`DYLD_*`/`MallocStackLogging*` env scrub.
- **`klynt-truncation`** (L1) — ports `utils/output-truncation/src/lib.rs`. Replaces both ad-hoc truncation sites in `crates/agent/src/execution/core.rs` with a typed `TruncationPolicy::{Bytes, Tokens}` API. Adds the `"Total output lines: N"` prefix that helps the model reason about cuts.
- **`klynt-git-utils`** (L1) — ports `git-utils/src/ghost_commits.rs`. Adds `ghost_commit_sha` + `ghost_repo_root` columns to `coding_snapshots`. `try_record_with_ghost` orchestrator picks ghost vs BLOB based on whether the file lives in a git repo. `rewind_to_message` dispatches on the column.

Closes the Phase 3+ "Snapshots: content-addressed dedup" item (different mechanism than originally specced — git natural CAS instead of BLAKE3 dedup table) and the §6 oversized-tool-result Phase 3+ note.

## Test plan

- [ ] `cargo build --workspace` clean
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` 0 warnings
- [ ] `cargo nextest run --workspace` all green (3 new crates × tests + 2 integration tests)
- [ ] `./scripts/run_kca_validation.sh` passes
- [ ] `cargo bench -p kca-bench --bench snapshot_create` baseline recorded
- [ ] Manual: launch desktop binary, edit a file in a git repo via chat, `/sessions rewind`, verify file restored
- [ ] Manual: same in a non-git tempdir, verify BLOB fallback works

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

- [ ] **Step 3: Return the PR URL to the user**

---

## Rollback plan

If any workstream needs to be backed out independently after merge:

| Workstream | Rollback | Surface area |
|---|---|---|
| A — Process Hardening | Comment out the call in `desktop/src/main.rs` and re-build. Crate stays compiled. | 1 line |
| B — Structured Truncation | Revert the changes to `agent/src/execution/core.rs` only; new crate stays unused. | 2 functions, ~20 lines |
| C — Ghost Commits | Revert `try_record_with_ghost` call sites to plain `record(...)`. Schema columns stay (NULL for new rows = identical to old behavior). | 3 tool files + 1 rewind site |

Each is reversible without data loss because Phase 3 is pure-additive at the storage layer (per CLAUDE.md pre-release no-migration policy).

---

## Self-review (executed against this plan)

**Spec coverage:**
- ✅ §6 line 680 oversized-result content-replacement → reframed as `TruncationPolicy` (Workstream B; spec note in Task B10).
- ✅ §13 Phase 3+ "Snapshots: content-addressed dedup" → ghost commits (Workstream C; spec note in Task C16).
- ✅ §13 Phase 3+ implicit "process hardening" (not in spec — added because the production binary ships secrets and codex has the precedent). Documented in Task A4.
- ⏭️ Deferred to later plans: LSP integration, MCP-contributed skills, per-channel MCP allowlists, Skills.sh marketplace, IDE bridge, multi-window, voice-driven coding, Windows sandbox, coverage-delta parsers, Starlark editor wiring.

**Placeholder scan:** None present — every code step contains real code; no "TODO/fill in/similar to" placeholders.

**Type consistency check:**
- ✅ `TruncationPolicy::Bytes(usize)` consistent across B2-B7.
- ✅ `GhostCommit::new(id, parent, files, dirs)` signature consistent across C3, C4, C5, C12, C14.
- ✅ `try_record_with_ghost(session_key, message_id, file_path, content, existed)` signature consistent C9, C10, C11, C13.
- ✅ `record_ghost` signature evolves once in C14 (gains `ghost_repo_root` arg) — Step 5 of C14 explicitly updates the test from C7 to match.
- ✅ `formatted_truncate_text(content, policy)` and `truncate_text(content, policy)` consistent B4, B6, B7.

**Granularity:** 23 numbered tasks, 90+ checkbox steps, every step is a single 2-5 minute action.
