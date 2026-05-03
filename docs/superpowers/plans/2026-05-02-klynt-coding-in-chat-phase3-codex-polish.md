# Klynt Coding-in-Chat — Phase 3 Codex-Polish (Retrospective)

> **For agentic workers:** This is a retrospective plan doc for work that already landed in commit `3ca997eda`. All tasks are marked complete.

**Goal:** Three hardening crates landed as a single commit: ghost-commit snapshots, process hardening, and structured output truncation. This doc backfills the plan that was deferred during Phase 2.

**Tech Stack:** Rust 1.93, Tauri 2, `walkdir`, `tempfile`, `libc`, `serde`.

---

## Task 1: `klynt-git-utils` — Ghost commits

Ghost commits replace BLOB content addressing for git-tracked files. `create_ghost_commit` captures the working tree state as a real git commit object (via `commit-tree`) without touching any branch refs. `restore_ghost_commit` restores the worktree to the snapshotted state, deleting files that appeared after the snapshot while preserving pre-existing untracked files.

**Files:**
- `crates/klynt-git-utils/src/lib.rs` — crate root, re-exports `GhostCommit`, `GhostSnapshotConfig`
- `crates/klynt-git-utils/src/ghost_commits.rs` — `create_ghost_commit`, `restore_ghost_commit`, temp-index isolation, file filtering (size/dir-count/exclude), 6 unit tests
- `crates/klynt-git-utils/src/repo.rs` — `get_git_repo_root` helper
- `crates/klynt-git-utils/src/errors.rs` — `GitToolingError` enum
- `crates/klynt-core/src/snapshots/repo.rs` — `try_record_with_ghost` (line 90): chooses ghost vs BLOB based on whether the file lives in a git repo
- `crates/klynt-core/src/tools/{edit,write,apply_patch,notebook_edit}.rs` — call sites for `try_record_with_ghost`

- [x] **Step 1: Ghost commit creation**
  - Temp index isolation (doesn't touch user's staging area)
  - Parent SHA recording (HEAD at snapshot time)
  - Pre-existing untracked file tracking
  - `GhostSnapshotConfig` with max_file_bytes (10 MiB), max_dir_entries (200), excluded path components

- [x] **Step 2: Ghost commit restoration**
  - `git restore --source <sha> --worktree` for the bulk restore
  - Walk worktree to delete post-snapshot files (respecting pre-existing untracked set)
  - Round-trip test: snapshot → mutate → restore → verify

- [x] **Step 3: Integration with snapshot system**
  - `try_record_with_ghost` in `crates/klynt-core/src/snapshots/repo.rs` auto-detects git repos
  - Falls back to BLOB for non-git directories
  - Wired into all file-editing tools (edit, write, apply_patch, notebook_edit)

---

## Task 2: `klynt-process-hardening` — Pre-main hardening

`pre_main_hardening()` is called as the first statement in `crates/desktop/src/main.rs`. It sets `RLIMIT_CORE = 0` (no core dumps), calls `ptrace(PT_DENY_ATTACH)` on macOS (debuggers cannot attach to a release build), and scrubs `LD_*`/`DYLD_*`/`MallocStackLogging*` env vars.

**Files:**
- `crates/klynt-process-hardening/src/lib.rs` — `pre_main_hardening()` with platform-specific branches (Linux/macOS/BSD/Windows), `set_core_file_size_limit_to_zero`, `remove_env_vars_with_prefix`, 2 unit tests
- `crates/desktop/src/main.rs` — first statement in `fn main()`

- [x] **Step 1: RLIMIT_CORE = 0**
  - `setrlimit(RLIMIT_CORE, {0, 0})` on all Unix platforms
  - Exits with code 7 on failure

- [x] **Step 2: ptrace(PT_DENY_ATTACH) on macOS**
  - Blocks debugger attachment on release builds
  - Exits with code 6 on failure
  - Linux uses `prctl(PR_SET_DUMPABLE, 0)` instead (exits with code 5)

- [x] **Step 3: Env var scrubbing**
  - Removes all `LD_*` vars on Linux/BSD
  - Removes all `DYLD_*`, `MallocStackLogging*`, `MallocLogFile*` vars on macOS
  - Handles non-UTF-8 keys correctly (OsString, not String)
  - Windows branch is a no-op TODO (out of scope)

---

## Task 3: `klynt-truncation` — Structured output truncation

`TruncationPolicy::Bytes(n)` / `Tokens(n)` middle-chops with a "Total output lines: N" prefix. Replaces the deferred Claude-Code-style content-replacement design (spec line 680-682).

**Files:**
- `crates/klynt-truncation/src/lib.rs` — `TruncationPolicy` enum, `truncate_middle_chars`, `formatted_truncate_text`, `truncate_text`, `truncate_function_output_items`, `ContentItem` enum, 12 unit tests

- [x] **Step 1: TruncationPolicy enum**
  - `Bytes(usize)` and `Tokens(usize)` variants
  - `byte_budget()` / `token_budget()` conversion (4 bytes ≈ 1 token heuristic)

- [x] **Step 2: Middle-char truncation**
  - Keeps head + tail halves, replaces middle with `[...] omitted N bytes [...]`
  - UTF-8 safe — uses `floor_char_boundary` to never split multibyte chars
  - Handles edge cases: empty input, budget zero, pathological small budgets

- [x] **Step 3: Formatted truncation**
  - `formatted_truncate_text` prepends `"Total output lines: N\n\n"` when truncated
  - `truncate_text` omits the prefix (for transport-layer caps where model never sees it)

- [x] **Step 4: Function output item truncation**
  - `truncate_function_output_items` handles `ContentItem::Text` / `ContentItem::Image`
  - Images always survive; text items truncated in order with `[omitted N text items ...]` sentinel
