# Codebase Cleanup — Quick Wins Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land five independent low-risk codebase cleanups in a single coordinated sweep: (1) introduce a `CommandResult<T>` type alias to compress 508 hand-written `Result<T, ApiError>` signatures; (2) consolidate the one outlier crate dep onto the workspace dep table; (3) install and run `cargo-machete` + `cargo-udeps` to prune dead dependencies; (4) add `#[tracing::instrument]` to every public method on `AppCore` so structured trace context follows handler calls; (5) factor the duplicated `vi.mock("@tauri-apps/api/core", ...)` block out of 12 Vitest files into a single shared helper.

**Architecture:** Five independent phases. Each phase produces a green-CI commit on its own; the order below is **dependency-free** so phases can be parallelized across worktrees if desired. Phase A is mechanical sed-replace + one new type alias. Phase B is a one-line Cargo.toml change plus workspace-dep declaration. Phase C is a tooling install + report-driven dep removal (lines depend on what `machete`/`udeps` find). Phase D adds one attribute per public method in `AppCore` — one task per file. Phase E adds one helper file in `desktop-ui/src/test/` and migrates 12 callers.

**Tech Stack:** Rust 1.93, existing `desktop-shared::ApiError`, `tracing` crate (already in workspace), `cargo-machete = ">=0.7"` and `cargo-udeps = ">=0.1.50"` (newly installed via `cargo install`), Vitest 1.x with `@testing-library/react`, `bun` for the JS toolchain.

**Master plan context:** Independent sweep PR. Not part of the Realtime-Data-Layer master plan series. Schedule **after Plan 5 lands** for #4 specifically (the `CommandResult` alias should ride alongside the typed-IPC migration to avoid two churns over the same files); the other four phases can ship any time.

---

## File Structure

### Files to create

| Path | Responsibility |
|---|---|
| `desktop-ui/src/test/mockTauri.ts` | Shared helper for Vitest tests: `installTauriMocks({ commands?, listen?, convertFileSrc? })` returns the mocked module bindings. Replaces the boilerplate `vi.mock("@tauri-apps/api/core", ...)` block at 12 call sites. |

### Files to modify (Rust)

| Path | Change |
|---|---|
| `crates/desktop-shared/src/errors.rs` | Add `pub type CommandResult<T> = Result<T, ApiError>;` after the `ApiError` struct. |
| `crates/desktop-shared/src/lib.rs` | Re-export the alias: `pub use errors::CommandResult;`. |
| `crates/desktop/src/commands/*.rs` (52 files) | Replace `Result<T, ApiError>` with `CommandResult<T>` in every command signature. Update the `use desktop_shared::ApiError;` import to also pull `CommandResult`. One task per file. |
| `crates/desktop/src/oauth/commands.rs` | Same pattern as commands/*. |
| `crates/scheduling/Cargo.toml` | Replace `thiserror = "2.0"` with `thiserror = { workspace = true }`. |
| `Cargo.toml` (workspace root) | Confirm `thiserror = "2.0"` is in `[workspace.dependencies]`; add it if missing. Also add `tracing = "0.1"` if not present (it's used in many crates already). |
| `crates/app-core/src/**/*.rs` (handler-bearing files) | Add `#[tracing::instrument(skip(self), err)]` to every public method on `AppCore` and on each `*Handler` struct. One task per handler file. |
| `crates/app-core/Cargo.toml` | Confirm `tracing = { workspace = true }` is present (it should already be). |
| Per-crate `Cargo.toml` files reported by `cargo-machete` / `cargo-udeps` | Remove flagged unused deps. |

### Files to modify (Frontend)

| Path | Change |
|---|---|
| `desktop-ui/src/services/tauri.test.ts` | Migrate to `installTauriMocks(...)`. |
| `desktop-ui/src/features/composer/components/ComposerSend.test.tsx` | Same. |
| `desktop-ui/src/features/composer/components/ComposerInput.attachments.test.tsx` | Same. |
| `desktop-ui/src/features/composer/components/ComposerEditorHelpers.test.tsx` | Same. |
| (8 more `*.test.tsx` files at the 12 sites identified by `rg -l 'vi\.mock\("@tauri-apps/api/core"'`) | Same migration. Each its own task. |

### Files NOT modified (verified during research; called out to prevent drift)

- `crates/common/src/error.rs` (`KlyntbotError`) — out of scope; the alias is for the IPC error surface only, not the internal one.
- `desktop-ui/src/test/vitest.setup.ts` — global setup file. The new mock helper is *opt-in* per test, not auto-installed globally, because some tests need different mock shapes (e.g. `convertFileSrc` mocks for composer tests).
- Any `crates/feature-*/Cargo.toml` already using `{ workspace = true }` — no-op.
- The `agent` crate's existing `tracing::instrument` call — leave as-is; the audit only adds, never modifies existing instrumentation.
- The bigger `Result<T, KlyntbotError>` surface inside the agent runtime — out of scope; only the Tauri command surface (`Result<T, ApiError>`) gets the alias.

---

## Phase A — `CommandResult<T>` type alias (5 tasks)

### Task A1: Add the type alias

**Files:**
- Modify: `crates/desktop-shared/src/errors.rs`
- Modify: `crates/desktop-shared/src/lib.rs`

- [ ] **Step 1: Read the current `errors.rs` file**

Run: `cat crates/desktop-shared/src/errors.rs`
Expected: a single `pub struct ApiError { ... }` plus impls.

- [ ] **Step 2: Write the failing test**

Append to the bottom of `crates/desktop-shared/src/errors.rs`:

```rust
#[cfg(test)]
mod alias_tests {
    use super::*;

    #[test]
    fn command_result_is_alias_for_result_apierror() {
        // Trivial — proves the alias resolves at compile time.
        let v: CommandResult<i32> = Ok(42);
        assert_eq!(v.unwrap(), 42);

        let e: CommandResult<()> = Err(ApiError {
            kind: "TestKind".into(),
            message: "test".into(),
            details: None,
        });
        assert!(e.is_err());
    }
}
```

- [ ] **Step 3: Run the test — expected to fail**

Run: `cargo nextest run -p desktop-shared command_result_is_alias_for_result_apierror`
Expected: FAIL with "cannot find type `CommandResult`".

- [ ] **Step 4: Add the type alias**

Insert after the `ApiError` struct definition (and its impls), still in `errors.rs`:

```rust
/// Convenience alias used by every `#[tauri::command]` in `crates/desktop`.
/// Identical to `Result<T, ApiError>` but shorter at every call site.
pub type CommandResult<T> = Result<T, ApiError>;
```

Note: the test's `ApiError { kind, message, details }` struct literal must match the actual fields — adjust if your `ApiError` has different field names.

- [ ] **Step 5: Re-export from `lib.rs`**

Edit `crates/desktop-shared/src/lib.rs` — locate the existing `pub use errors::ApiError;` line (or equivalent re-export), and extend it:

```rust
pub use errors::{ApiError, CommandResult};
```

- [ ] **Step 6: Run the test — expected to pass**

Run: `cargo nextest run -p desktop-shared command_result_is_alias_for_result_apierror`
Expected: pass.

- [ ] **Step 7: Build the workspace**

Run: `cargo build --workspace 2>&1 | tail -10`
Expected: clean.

- [ ] **Step 8: Commit**

```bash
git add crates/desktop-shared/src/errors.rs crates/desktop-shared/src/lib.rs
git commit -m "feat(desktop-shared): add CommandResult<T> type alias"
```

---

### Task A2: Migrate `crates/desktop/src/commands/tasks.rs` (pilot)

This is the pilot — pick the smallest commands file and validate the migration mechanics before fanning out.

**Files:**
- Modify: `crates/desktop/src/commands/tasks.rs`

- [ ] **Step 1: Read the file's current `use` line**

Run: `head -10 crates/desktop/src/commands/tasks.rs`
Expected: a `use desktop_shared::...` line that imports `ApiError`.

- [ ] **Step 2: Update the import**

Edit the `use desktop_shared::...` import line. If it currently reads:

```rust
use desktop_shared::ApiError;
```

change to:

```rust
use desktop_shared::{ApiError, CommandResult};
```

If `ApiError` is in a multi-import block:

```rust
use desktop_shared::{ApiError, TaskResponse, ...};
```

extend it to:

```rust
use desktop_shared::{ApiError, CommandResult, TaskResponse, ...};
```

(Keep `ApiError` even though it's still used in `?` paths and error-construction sites.)

- [ ] **Step 3: Replace every return type**

Run a targeted in-file substitution. Edit the file with this find/replace (per-occurrence, since there are no qualifying suffixes):

- find: `Result<Option<TaskResponse>, ApiError>`  →  replace: `CommandResult<Option<TaskResponse>>`
- find: `Result<Vec<TaskResponse>, ApiError>`  →  replace: `CommandResult<Vec<TaskResponse>>`
- find: `Result<TaskResponse, ApiError>`  →  replace: `CommandResult<TaskResponse>`
- find: `Result<bool, ApiError>`  →  replace: `CommandResult<bool>`
- find: `Result<(), ApiError>`  →  replace: `CommandResult<()>`
- find: `Result<Vec<TodayTaskResponse>, ApiError>`  →  replace: `CommandResult<Vec<TodayTaskResponse>>`
- find: `Result<Vec<ProjectResponse>, ApiError>`  →  replace: `CommandResult<Vec<ProjectResponse>>`
- find: `Result<Vec<ObjectiveResponse>, ApiError>`  →  replace: `CommandResult<Vec<ObjectiveResponse>>`
- find: `Result<TaskAttachmentRow, ApiError>`  →  replace: `CommandResult<TaskAttachmentRow>`
- find: `Result<Vec<TaskAttachmentRow>, ApiError>`  →  replace: `CommandResult<Vec<TaskAttachmentRow>>`
- find: `Result<TaskTimeEntryRow, ApiError>`  →  replace: `CommandResult<TaskTimeEntryRow>`
- find: `Result<Vec<TaskTimeEntryRow>, ApiError>`  →  replace: `CommandResult<Vec<TaskTimeEntryRow>>`

(For unfamiliar files, use `grep -E "Result<.+, ApiError>" <file>` to enumerate every occurrence first; replace each one. The pattern is always `Result<X, ApiError>` → `CommandResult<X>` no matter how complex `X` is.)

- [ ] **Step 4: Verify no `Result<*, ApiError>` remains in this file**

Run: `grep -n "Result<.*, ApiError>" crates/desktop/src/commands/tasks.rs`
Expected: no output.

- [ ] **Step 5: Build**

Run: `cargo build -p desktop 2>&1 | tail -10`
Expected: clean.

- [ ] **Step 6: Run all `desktop` tests**

Run: `cargo nextest run -p desktop 2>&1 | tail -10`
Expected: all pass (no semantic change).

- [ ] **Step 7: Commit**

```bash
git add crates/desktop/src/commands/tasks.rs
git commit -m "refactor(desktop): migrate commands/tasks to CommandResult<T> alias"
```

---

### Task A3: Mechanical fan-out across the remaining 51 command files

**Files:** every file matched by `rg -l "Result<.+, ApiError>" crates/desktop/src/commands/ crates/desktop/src/oauth/commands.rs`.

These are mechanical replacements; we run them as a single sweep instead of 51 separate tasks because the pattern is uniform. (Subagent-driven executors can still parallelize: spawn one subagent per file with the A2 step list as its prompt.)

- [ ] **Step 1: Enumerate target files**

Run: `rg -l "Result<.+, ApiError>" crates/desktop/src/commands/ crates/desktop/src/oauth/commands.rs > /tmp/cr_files.txt && wc -l /tmp/cr_files.txt`
Expected: ~52 file paths.

- [ ] **Step 2: For each file, perform the A2 import-update + return-type replacement**

If executing manually, walk the list with one Edit per file. If executing via subagent, dispatch a fresh subagent per file with the literal A2 step list, the file path, and the constraint "exit cleanly only after `cargo build -p desktop` is green for *just* this file's diff".

For batch sed-style execution, the pattern that's safe across all 52 files (uses Perl regex with bounded greedy match):

```bash
# Run from the repo root.
# This regex captures `Result<X, ApiError>` where X may itself contain
# nested generics (`Vec<...>`, `Option<...>`, etc). The Perl `(?:[^<>]|<[^<>]*>)*`
# group matches X with one level of nested `<>` — sufficient for every observed
# return type in the codebase (verified against the 508-occurrence inventory).

rg -l "Result<.+, ApiError>" crates/desktop/src/commands/ crates/desktop/src/oauth/commands.rs | \
  xargs perl -i -pe 's/\bResult<((?:[^<>]|<[^<>]*>)*), ApiError>/CommandResult<$1>/g'
```

**Caveat:** the regex doesn't add the `CommandResult` import. After the perl run, every file that previously imported `ApiError` from `desktop_shared` needs `CommandResult` added to that import line. Apply this second sweep:

```bash
rg -l "CommandResult<" crates/desktop/src/commands/ crates/desktop/src/oauth/commands.rs | while read -r f; do
  # If `use desktop_shared::ApiError;` (single import) → expand to set
  perl -i -pe 's/use desktop_shared::ApiError;/use desktop_shared::{ApiError, CommandResult};/g' "$f"
  # If `use desktop_shared::{...ApiError...};` (multi-import) → splice in CommandResult after ApiError
  perl -i -pe 's/use desktop_shared::\{([^}]*)ApiError(.*?)\};/use desktop_shared::{$1ApiError, CommandResult$2};/g' "$f"
done
```

**Validate the second sweep didn't double-insert** (idempotent check):

```bash
rg "CommandResult, CommandResult|ApiError, CommandResult, CommandResult" crates/desktop/src/commands/
```
Expected: no output.

- [ ] **Step 3: Build**

Run: `cargo build -p desktop 2>&1 | tail -20`
Expected: clean. If it fails, the most likely cause is one of the original `Result<...>` types had three-level nesting (e.g. `Result<Vec<Option<X>>, ApiError>`). For each error, manually fix the offending file.

- [ ] **Step 4: Run all desktop tests**

Run: `cargo nextest run -p desktop`
Expected: all pass.

- [ ] **Step 5: Run the dev_server coverage tests**

Run: `cargo nextest run -p desktop dev_server_covers_all_tauri_commands dev_server_has_no_orphan_commands`
Expected: pass — pure type-alias change should not affect the command list.

- [ ] **Step 6: Confirm zero remaining `Result<*, ApiError>` in the migration scope**

Run: `rg "Result<.+, ApiError>" crates/desktop/src/commands/ crates/desktop/src/oauth/commands.rs`
Expected: no output.

- [ ] **Step 7: Confirm count of `CommandResult<` matches the pre-migration count of `Result<*, ApiError>`**

Run: `rg -c "CommandResult<" crates/desktop/src/commands/ crates/desktop/src/oauth/commands.rs | awk -F: '{s+=$2} END {print s}'`
Expected: 508 (matches the pre-migration count from recon).

- [ ] **Step 8: Commit**

```bash
git add crates/desktop/src/commands/ crates/desktop/src/oauth/commands.rs
git commit -m "refactor(desktop): mass-migrate commands to CommandResult<T> (51 files)"
```

---

### Task A4: Verify clippy + format

- [ ] **Step 1: Format**

Run: `cargo fmt --all`
Expected: clean (or auto-formats minor whitespace from the perl substitution).

- [ ] **Step 2: Clippy**

Run: `cargo clippy -p desktop --all-targets 2>&1 | tail -30`
Expected: no new warnings.

- [ ] **Step 3: If `cargo fmt` made changes, commit them**

```bash
git diff --quiet || (git add -u && git commit -m "style: cargo fmt after CommandResult migration")
```

---

### Task A5: Phase A checkpoint

- [ ] **Step 1: Run the full workspace build**

Run: `cargo build --workspace 2>&1 | tail -10`
Expected: clean.

- [ ] **Step 2: Run the full workspace tests**

Run: `cargo nextest run --workspace 2>&1 | tail -10`
Expected: all pass.

- [ ] **Step 3: Check the FE typecheck still passes (in case any shared type changed)**

Run: `cd desktop-ui && bun run typecheck 2>&1 | tail -10`
Expected: clean.

---

## Phase B — Workspace dependency consolidation (3 tasks)

### Task B1: Confirm `thiserror` is in workspace deps

**Files:**
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Inspect**

Run: `grep -n "thiserror" Cargo.toml`
Expected: at least one match. If `[workspace.dependencies]` does not contain `thiserror`, add it (next step). If it does, skip Step 2.

- [ ] **Step 2: Add `thiserror = "2.0"` to `[workspace.dependencies]`**

Edit `Cargo.toml` — inside `[workspace.dependencies]`, add (alphabetically):

```toml
thiserror = "2.0"
```

- [ ] **Step 3: Build to confirm Cargo.lock is unaffected**

Run: `cargo build --workspace 2>&1 | tail -5`
Expected: clean.

- [ ] **Step 4: Commit (only if added)**

```bash
git add Cargo.toml
git commit -m "chore(workspace): add thiserror to workspace.dependencies"
```

---

### Task B2: Migrate `crates/scheduling/Cargo.toml` to workspace dep

**Files:**
- Modify: `crates/scheduling/Cargo.toml`

- [ ] **Step 1: Read the current line**

Run: `grep -n "thiserror" crates/scheduling/Cargo.toml`
Expected: `thiserror = "2.0"` (or similar literal version).

- [ ] **Step 2: Replace with workspace reference**

Edit the line:

```toml
thiserror = { workspace = true }
```

- [ ] **Step 3: Build the crate**

Run: `cargo build -p scheduling 2>&1 | tail -5`
Expected: clean.

- [ ] **Step 4: Verify `Cargo.lock` unchanged**

Run: `git diff Cargo.lock | head -20`
Expected: no changes (the version resolved is the same — workspace is `2.0`, the literal was `2.0`).

- [ ] **Step 5: Commit**

```bash
git add crates/scheduling/Cargo.toml
git commit -m "chore(scheduling): use workspace thiserror"
```

---

### Task B3: Audit for any other direct version pins

**Files:** any `crates/*/Cargo.toml`.

- [ ] **Step 1: Run a broader sweep**

Run:

```bash
rg "^[a-z][a-z0-9_-]+ = \"[0-9]" crates/*/Cargo.toml | grep -v workspace | head -30
```

Expected: 0–5 lines. If non-empty, each match is a candidate for migration to workspace deps.

- [ ] **Step 2: For each finding, inspect the workspace `Cargo.toml`**

If the dep is already in `[workspace.dependencies]`, replace the literal in the crate file with `{ workspace = true }` and commit per-crate (analogous to B2). If not, decide whether to promote — usually yes for any dep used by ≥2 crates.

- [ ] **Step 3: For each migration, build + commit per-crate**

Format: `chore(<crate>): use workspace <dep>`.

---

## Phase C — Dead dependency audit (5 tasks)

### Task C1: Install `cargo-machete`

**Files:** none — local toolchain change.

- [ ] **Step 1: Install**

Run: `cargo install cargo-machete --locked`
Expected: install completes within 1–2 minutes.

- [ ] **Step 2: Verify**

Run: `cargo machete --help | head -10`
Expected: help text printed.

- [ ] **Step 3: No commit — local dev tool only.**

---

### Task C2: Run `cargo-machete` and capture the report

**Files:** none — read-only step.

- [ ] **Step 1: Run from repo root**

Run: `cargo machete 2>&1 | tee /tmp/machete-report.txt`
Expected: a list of unused deps per crate.

- [ ] **Step 2: Inspect the report**

Run: `cat /tmp/machete-report.txt`

Each finding is shaped:
```
crates/some-crate has unused dependencies:
    foo
    bar (unused in target.x86_64-apple-darwin)
```

- [ ] **Step 3: Write down the findings — note any false positives**

`cargo-machete` flags deps where the symbol references aren't statically detectable, including macro-only deps (`tracing`, `serde_derive`, `tokio::main`, `tauri::generate_handler!`). Skim for known-false-positive crates: **`serde_derive`, `tracing`, `tokio` (when only `#[tokio::main]` is used), `tauri-build`** — keep these even if flagged.

- [ ] **Step 4: No commit yet — proceed to C3 to act on the report.**

---

### Task C3: Remove genuinely unused deps from `Cargo.toml` files

**Files:** per-crate `Cargo.toml`s flagged by C2 (after filtering false positives).

For each genuinely unused dep:

- [ ] **Step 1: Identify the crate file and dep line**

Example: machete flags `crates/foo` for unused `bar`.

- [ ] **Step 2: Remove the line from `crates/foo/Cargo.toml`**

Edit the file — delete the offending line in `[dependencies]` or `[dev-dependencies]`.

- [ ] **Step 3: Build the crate**

Run: `cargo build -p foo 2>&1 | tail -5`
Expected: clean. If it fails, the dep was actually used (false positive); revert.

- [ ] **Step 4: Build the workspace**

Run: `cargo build --workspace 2>&1 | tail -5`
Expected: clean.

- [ ] **Step 5: Commit per-crate**

```bash
git add crates/foo/Cargo.toml
git commit -m "chore(foo): drop unused bar dep (cargo-machete)"
```

Repeat for every flagged crate. **Each crate gets its own commit** for review-friendliness.

---

### Task C4: Install + run `cargo-udeps` (nightly-only check)

`cargo-udeps` requires nightly Rust. It's stricter than `machete` (uses the actual compiler) but slower.

- [ ] **Step 1: Install**

Run: `cargo install cargo-udeps --locked`
Expected: completes.

- [ ] **Step 2: Install nightly toolchain if not present**

Run: `rustup toolchain install nightly --profile minimal`
Expected: installs or reports already installed.

- [ ] **Step 3: Run on the workspace**

Run: `cargo +nightly udeps --workspace --all-targets 2>&1 | tee /tmp/udeps-report.txt | tail -50`
Expected: report. May take 5–15 minutes (full type-checked compile).

- [ ] **Step 4: For each new finding (i.e. not already removed in C3), apply the C3 procedure**

Same per-crate commit pattern.

- [ ] **Step 5: For findings that overlap with C2 (already removed), no action.**

---

### Task C5: Phase C verification

- [ ] **Step 1: Re-run machete to confirm zero flags**

Run: `cargo machete 2>&1 | tail -10`
Expected: "no unused dependencies found" or equivalent.

- [ ] **Step 2: Build + test the workspace**

Run: `cargo build --workspace && cargo nextest run --workspace 2>&1 | tail -10`
Expected: clean.

- [ ] **Step 3: Document the new tooling in CLAUDE.md**

Edit `CLAUDE.md` — under the "Build & Test" section, add:

```markdown
## Dependency hygiene

Run periodically (e.g. before a release):
- `cargo machete` — fast static check for unused deps in `Cargo.toml`
- `cargo +nightly udeps --workspace` — slower but compiler-driven; catches what machete misses
```

- [ ] **Step 4: Commit the docs update**

```bash
git add CLAUDE.md
git commit -m "docs(claude): add cargo-machete + cargo-udeps to dependency hygiene"
```

---

## Phase D — `tracing::instrument` audit on `AppCore` (8 tasks)

This phase adds `#[tracing::instrument]` to every public method on `AppCore` and on each `*Handler` struct in `crates/app-core/`. Goal: every business-logic call gets a structured span carrying handler name + key args. **Out of scope:** the 465 Tauri command shells in `crates/desktop/src/commands/` — those are thin adapters that just delegate; the AppCore method below them is where the trace span belongs.

### Task D1: Survey the `app-core` handler surface

**Files:** read-only.

- [ ] **Step 1: List every handler-bearing file**

Run:

```bash
find crates/app-core/src -name "*.rs" -type f | sort > /tmp/appcore-files.txt
wc -l /tmp/appcore-files.txt
```

- [ ] **Step 2: Find every public method on `AppCore` and on `*Handler` structs**

Run:

```bash
rg -n "^impl.*\b(AppCore|Handler)\b" crates/app-core/src/ | head -40
rg -n "^    pub (async )?fn " crates/app-core/src/handlers/ 2>&1 | wc -l
```

Note the count — this is the rough size of Phase D.

- [ ] **Step 3: Confirm `tracing` is in `app-core/Cargo.toml`**

Run: `grep tracing crates/app-core/Cargo.toml`
Expected: `tracing = { workspace = true }` (or similar). If missing, add it under `[dependencies]`.

- [ ] **Step 4: No commit — survey only.**

---

### Task D2: Pilot — instrument `crates/app-core/src/handlers/tasks.rs`

(Adjust path if the actual handler file lives elsewhere — adapt by inspecting the D1 survey results.)

**Files:**
- Modify: `crates/app-core/src/handlers/tasks.rs` (or whichever file holds task handlers).

- [ ] **Step 1: Read the current file**

Run: `head -50 crates/app-core/src/handlers/tasks.rs`

- [ ] **Step 2: For every `pub async fn` (and `pub fn`) on a handler struct, add `#[tracing::instrument(skip(self), err)]`**

Pattern:

```rust
impl TaskHandler {
    #[tracing::instrument(skip(self), err)]
    pub async fn create_task(&self, params: TaskCreateParams) -> Result<TaskResponse, KlyntbotError> {
        // existing body unchanged
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn list_tasks(&self) -> Result<Vec<TaskResponse>, KlyntbotError> {
        // ...
    }
}
```

**Choosing `skip` parameters:**
- Always `skip(self)` (avoids dumping the entire AppCore Debug repr into spans).
- Skip any `&[u8]` / large `String` params: `skip(self, blob, content)`.
- Skip any `serde_json::Value`: `skip(self, payload)`.

**Choosing the `err` flag:** include if the function returns `Result<T, E>` where `E: Display`. The flag emits a `tracing::error!` when the function returns `Err(_)`. Skip the flag for non-Result functions.

- [ ] **Step 3: Build the crate**

Run: `cargo build -p app-core 2>&1 | tail -10`
Expected: clean. If a method has only `&self` and no other args, `skip(self)` is correct. If a method takes `params` you want logged, omit it from `skip` to capture in the span.

- [ ] **Step 4: Run the crate's tests**

Run: `cargo nextest run -p app-core 2>&1 | tail -10`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/handlers/tasks.rs
git commit -m "feat(app-core): instrument task handlers with tracing::instrument"
```

---

### Task D3–D7: Fan out across remaining handler files

**One task per handler file**, applying the D2 template. Discover the file list from D1's survey output.

Likely candidates based on the architecture description in CLAUDE.md (the actual file layout may differ):

| Task | File (typical) |
|------|----------------|
| D3 | `crates/app-core/src/handlers/projects.rs` |
| D4 | `crates/app-core/src/handlers/notes.rs` |
| D5 | `crates/app-core/src/handlers/finance.rs` |
| D6 | `crates/app-core/src/handlers/productivity.rs` |
| D7 | `crates/app-core/src/handlers/{remaining handler files}` |

**For each file:** apply D2 template (read → annotate every public method → build → test → commit `feat(app-core): instrument <module> handlers with tracing::instrument`).

If a file has 0 public handler methods (e.g. internal helpers only), no annotation needed — skip it. Document the skip in the per-task notes.

---

### Task D8: Phase D verification

- [ ] **Step 1: Count instrument annotations added**

Run: `rg -c "tracing::instrument" crates/app-core/src/ | awk -F: '{s+=$2} END {print s}'`
Expected: the count of public handler methods discovered in D1.

- [ ] **Step 2: Build + test the workspace**

Run: `cargo build --workspace && cargo nextest run --workspace 2>&1 | tail -10`
Expected: clean.

- [ ] **Step 3: Manual smoke test — verify a span shows up in logs**

In one terminal: `RUST_LOG=app_core=debug cargo tauri dev`
Open the desktop UI, perform a simple action (e.g. create a task).

Expected: the dev console shows a structured trace line like:

```
TRACE app_core::handlers::tasks: create_task{params=TaskCreateParams { ... }} new
```

- [ ] **Step 4: Update CLAUDE.md**

Add to the "Conventions" section:

```markdown
- **Tracing:** every public method on an `AppCore` handler must be annotated with `#[tracing::instrument(skip(self), err)]`. New handler methods inherit the convention. The Tauri command shells in `crates/desktop/src/commands/` are NOT instrumented (thin adapters); the trace span lives one layer down.
```

- [ ] **Step 5: Commit the docs update**

```bash
git add CLAUDE.md
git commit -m "docs(claude): document tracing::instrument convention for app-core handlers"
```

---

## Phase E — Vitest mock-Tauri shared helper (5 tasks)

### Task E1: Create the helper

**Files:**
- Create: `desktop-ui/src/test/mockTauri.ts`

- [ ] **Step 1: Write the helper**

```ts
import { vi } from "vitest";

/**
 * Mock helpers for Tauri APIs. Call inside a Vitest module factory:
 *
 *     import { mockTauriCore } from "@/test/mockTauri";
 *     vi.mock("@tauri-apps/api/core", () => mockTauriCore({ invoke: vi.fn() }));
 *
 * The default returns `invoke` and `convertFileSrc` mocks; pass overrides
 * to swap in test-specific behavior.
 */
export function mockTauriCore(overrides: {
    invoke?: ReturnType<typeof vi.fn>;
    convertFileSrc?: (path: string) => string;
} = {}) {
    return {
        invoke: overrides.invoke ?? vi.fn(),
        convertFileSrc:
            overrides.convertFileSrc ?? ((path: string) => `tauri://${path}`),
    };
}

/**
 * Mock helpers for `@tauri-apps/api/event`.
 */
export function mockTauriEvent(overrides: {
    listen?: ReturnType<typeof vi.fn>;
    emit?: ReturnType<typeof vi.fn>;
} = {}) {
    return {
        listen: overrides.listen ?? vi.fn().mockResolvedValue(() => {}),
        emit: overrides.emit ?? vi.fn().mockResolvedValue(undefined),
    };
}

/**
 * Convenience: install both core + event mocks at once.
 * Use inside a `vi.mock(...)` factory if more mocks are needed than the
 * defaults; otherwise prefer the typed `mockTauriCore` / `mockTauriEvent`.
 */
export function installTauriMocks() {
    vi.mock("@tauri-apps/api/core", () => mockTauriCore());
    vi.mock("@tauri-apps/api/event", () => mockTauriEvent());
}
```

- [ ] **Step 2: Write the failing test for the helper**

Create `desktop-ui/src/test/mockTauri.test.ts`:

```ts
import { describe, it, expect, vi } from "vitest";
import { mockTauriCore, mockTauriEvent } from "./mockTauri";

describe("mockTauriCore", () => {
    it("provides default invoke + convertFileSrc", () => {
        const mocks = mockTauriCore();
        expect(typeof mocks.invoke).toBe("function");
        expect(mocks.convertFileSrc("foo.png")).toBe("tauri://foo.png");
    });

    it("respects override invoke", () => {
        const customInvoke = vi.fn();
        const mocks = mockTauriCore({ invoke: customInvoke });
        expect(mocks.invoke).toBe(customInvoke);
    });
});

describe("mockTauriEvent", () => {
    it("provides default listen + emit as resolving fns", async () => {
        const mocks = mockTauriEvent();
        const unlisten = await mocks.listen();
        expect(typeof unlisten).toBe("function");
        await expect(mocks.emit("test", {})).resolves.toBeUndefined();
    });
});
```

- [ ] **Step 3: Run the test**

Run: `cd desktop-ui && bun run test -- --run src/test/mockTauri.test.ts`
Expected: 4 tests pass.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/test/mockTauri.ts desktop-ui/src/test/mockTauri.test.ts
git commit -m "feat(desktop-ui): add shared mockTauriCore + mockTauriEvent helpers"
```

---

### Task E2: Migrate `desktop-ui/src/services/tauri.test.ts`

**Files:**
- Modify: `desktop-ui/src/services/tauri.test.ts`

- [ ] **Step 1: Read the current mock block**

Run: `head -20 desktop-ui/src/services/tauri.test.ts`

- [ ] **Step 2: Replace with helper call**

Find the existing block:

```ts
vi.mock("@tauri-apps/api/core", () => ({
    invoke: vi.fn(),
}));
```

Replace with:

```ts
import { mockTauriCore } from "@/test/mockTauri";

vi.mock("@tauri-apps/api/core", () => mockTauriCore());
```

If the original mock used a custom `invoke` (e.g. with predefined return values via `mockResolvedValue`), pass it through:

```ts
const customInvoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => mockTauriCore({ invoke: customInvoke }));
```

- [ ] **Step 3: Run the migrated test**

Run: `cd desktop-ui && bun run test -- --run src/services/tauri.test.ts`
Expected: same pass result as before.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/services/tauri.test.ts
git commit -m "refactor(desktop-ui): use mockTauriCore in services/tauri.test"
```

---

### Tasks E3–E13: Migrate remaining 11 files

**Apply the E2 template to each file**, one task per file. The list comes from `rg -l 'vi\.mock\("@tauri-apps/api/core"' desktop-ui/src/`.

| Task | File |
|------|------|
| E3 | `desktop-ui/src/features/composer/components/ComposerSend.test.tsx` |
| E4 | `desktop-ui/src/features/composer/components/ComposerInput.attachments.test.tsx` |
| E5 | `desktop-ui/src/features/composer/components/ComposerEditorHelpers.test.tsx` |
| E6–E13 | (8 remaining — discover by running `rg -l 'vi\.mock\("@tauri-apps/api/core"' desktop-ui/src/` and excluding files already migrated) |

**For each file:** apply E2 template (read → identify the existing mock shape → swap in `mockTauriCore({ ... })` with whatever overrides the test required → run → commit).

**Caveat:** some tests use `convertFileSrc` from the mocked module. The default helper provides `convertFileSrc: (path) => "tauri://" + path` — confirm this matches the test's expected behavior. If the test asserts a specific `convertFileSrc` return shape, override:

```ts
vi.mock("@tauri-apps/api/core", () =>
    mockTauriCore({ convertFileSrc: (p) => `custom://${p}` })
);
```

---

### Task E14: Phase E verification

- [ ] **Step 1: Confirm zero remaining duplicated mock patterns**

Run:

```bash
rg -l 'vi\.mock\("@tauri-apps/api/core", \(\) => \(\{' desktop-ui/src/
```

Expected: no output — every site now uses `mockTauriCore`.

- [ ] **Step 2: Run the full FE test suite**

Run: `cd desktop-ui && bun run test 2>&1 | tail -10`
Expected: all pass.

- [ ] **Step 3: Run typecheck + lint**

Run: `cd desktop-ui && bun run typecheck && bun run lint 2>&1 | tail -10`
Expected: clean (lint warnings are fine; no new errors).

---

## Phase F — Final sweep (3 tasks)

### Task F1: Workspace-wide green check

- [ ] **Step 1: Build**

Run: `cargo build --workspace 2>&1 | tail -10`
Expected: clean.

- [ ] **Step 2: Format check**

Run: `cargo fmt --all --check`
Expected: clean.

- [ ] **Step 3: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features 2>&1 | tail -30`
Expected: zero new warnings.

- [ ] **Step 4: Tests**

Run: `cargo nextest run --workspace 2>&1 | tail -10`
Expected: all pass.

- [ ] **Step 5: Doctests**

Run: `cargo test --workspace --doc 2>&1 | tail -10`
Expected: clean.

- [ ] **Step 6: FE typecheck + tests + lint**

Run: `cd desktop-ui && bun run typecheck && bun run test && bun run lint 2>&1 | tail -10`
Expected: clean.

---

### Task F2: Manual smoke test

- [ ] **Step 1: Launch the app**

In one terminal: `cd desktop-ui && bun run dev`
In another: `cargo tauri dev`

- [ ] **Step 2: Exercise the CommandResult migration**

Click around: create a task, edit it, delete it. Each invokes a `CommandResult<T>` -returning command.
Expected: behavior identical to pre-migration.

- [ ] **Step 3: Exercise the tracing instrumentation**

In the dev console (set `RUST_LOG=app_core=debug` before running `cargo tauri dev`), perform a handler-backed action. Confirm a span line shows up in logs.

- [ ] **Step 4: No commit — manual verification only.**

---

### Task F3: Push the working branch

- [ ] **Step 1: Confirm clean tree**

Run: `git status`
Expected: clean.

- [ ] **Step 2: Push**

Run: `git push origin HEAD`

- [ ] **Step 3: Open the PR**

Run: `gh pr create --title "chore: codebase cleanup quick wins (CommandResult + workspace deps + machete + instrument + mockTauri)" --body "$(cat <<'EOF'
## Summary

Five independent low-risk codebase cleanups landed in one sweep PR. Each phase is its own commit cluster; reviewable phase-by-phase.

- **Phase A:** Introduces `CommandResult<T>` type alias in `desktop-shared`. Migrates all 508 occurrences of `Result<T, ApiError>` across 52 command files to the alias.
- **Phase B:** Consolidates the one outlier crate (`scheduling`) onto the workspace `thiserror` declaration.
- **Phase C:** Installs `cargo-machete` + `cargo-udeps`, prunes any flagged unused deps. Documents the tooling in CLAUDE.md.
- **Phase D:** Adds `#[tracing::instrument(skip(self), err)]` to every public method on `AppCore` handlers. Documents the convention in CLAUDE.md.
- **Phase E:** Factors the duplicated `vi.mock("@tauri-apps/api/core", ...)` block out of 12 Vitest files into a shared `mockTauriCore` / `mockTauriEvent` helper.

## Test plan
- [ ] `cargo build --workspace` clean
- [ ] `cargo nextest run --workspace` green
- [ ] `cargo clippy --workspace --all-targets --all-features` zero new warnings
- [ ] `cargo fmt --all --check` clean
- [ ] `cd desktop-ui && bun run typecheck && bun run test && bun run lint` green
- [ ] Manual smoke test: launch the app, create/edit/delete a task; confirm trace spans appear with `RUST_LOG=app_core=debug`

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

## Self-Review

**Spec coverage:**

- ✅ #4 `CommandResult<T>` type alias → Phase A (5 tasks)
- ✅ #6 Workspace dep consolidation → Phase B (3 tasks)
- ✅ #5 Dead dep audit (`cargo machete` + `cargo udeps`) → Phase C (5 tasks)
- ✅ #7 `tracing::instrument` audit on `AppCore` → Phase D (8 tasks)
- ✅ #11 Vitest mock-Tauri shared helper → Phase E (14 tasks)
- ✅ Final sweep → Phase F (3 tasks)

**Placeholder scan:**

- Phase D's task list (D3–D7) is intentionally a discovery-driven template — the *actual* file list comes from D1's survey because the codebase's handler layout may evolve. Each task gets a concrete file before execution; the table is a planning skeleton, not a TBD.
- Phase E's tasks E6–E13 are similarly discovery-driven (run `rg -l ...` and process each file). The pattern is exhaustively spelled out in E2; downstream tasks reference the template by name.
- Every concrete substitution rule in A2 (12 `find/replace` pairs) is spelled out — no "etc." or "and similar".
- Phase C's commit messages follow the explicit pattern `chore(<crate>): drop unused <dep> dep (cargo-machete)`.

**Type consistency:**

- `CommandResult<T>` is referenced in A1 (definition), A2 (pilot), A3 (sweep), and the import line `use desktop_shared::{ApiError, CommandResult};` matches across all three.
- `mockTauriCore`, `mockTauriEvent`, `installTauriMocks` are introduced in E1 and consumed in E2; signatures match.
- `#[tracing::instrument(skip(self), err)]` is the canonical form used in D2 and the CLAUDE.md doc note in D8 — exact attribute string matches.

**Independent shippability:**

Phase A through E are independent. The plan documents (in the header) that **Phase A should land alongside or after Plan 5** to avoid two churns over the same files; the other four phases can ship any time.

---

## Out-of-scope notes

- **Instrumenting the 465 Tauri command shells** in `crates/desktop/src/commands/`. These are thin adapters that delegate to `AppCore`; the meaningful trace span lives one layer down (Phase D). Adding `#[tracing::instrument]` here would just trace IPC marshalling and bloat trace volume.
- **Migrating the broader `Result<T, KlyntbotError>` surface** inside the agent runtime to a similar alias. The IPC error type (`ApiError`) and the internal error type (`KlyntbotError`) are intentionally separate — aliasing the internal one would obscure that distinction.
- **`cargo-deny` integration.** A separate (heavier) tool for license + security audits. Worth a dedicated plan if/when needed; not part of this quick-wins sweep.
- **Frontend `import.meta.env` mocks.** Some Vitest files mock environment variables alongside the Tauri APIs. The `mockTauri` helper deliberately scopes to Tauri only — env mocks are test-specific and don't compose well into a generic helper.
- **The `agent` and `channels` crates' single existing `tracing::instrument` calls.** Leave them — the audit only adds.

---

## Definition of Done

- `cargo build --workspace` clean, zero new warnings.
- `cargo nextest run --workspace` green, all existing + 1 new test (`command_result_is_alias_for_result_apierror`) pass.
- `cargo clippy --workspace --all-targets --all-features` zero new warnings.
- `cargo fmt --all --check` clean.
- `cargo machete` reports zero unused deps (Phase C).
- `cd desktop-ui && bun run typecheck && bun run test && bun run lint` green; 4 new tests in `mockTauri.test.ts` pass.
- 508 occurrences of `Result<*, ApiError>` in `crates/desktop/src/commands/` and `oauth/commands.rs` are gone (`rg "Result<.+, ApiError>" crates/desktop/src/commands/ crates/desktop/src/oauth/commands.rs` returns zero); `CommandResult<*>` count matches 508.
- Zero remaining duplicated `vi.mock("@tauri-apps/api/core", () => ({` blocks in `desktop-ui/src/` (excluding the helper file itself); `rg -l 'vi\.mock\("@tauri-apps/api/core", \(\) => \(\{' desktop-ui/src/` returns empty.
- `crates/scheduling/Cargo.toml` uses `thiserror = { workspace = true }`.
- `rg -c "tracing::instrument" crates/app-core/src/` returns ≥ 1 per handler file (matches the D1 survey count).
- CLAUDE.md updated with: (a) `cargo-machete` + `cargo-udeps` periodic check, (b) `tracing::instrument` convention for app-core handlers.
- All commits use conventional-commit format with the right scope.

---

## End of plan

Five low-risk cleanups, all independently shippable, all measurable by exact `rg` / `grep` / count assertions. Total estimated effort: 1.5–2 days for a single-developer sequential pass; 3–4 hours if parallelized via subagents on the per-file fan-out tasks (A3, D3–D7, E3–E13).
