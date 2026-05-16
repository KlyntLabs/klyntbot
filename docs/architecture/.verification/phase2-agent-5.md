# Phase 2 Verification Report — Agent 5

**Subsystem:** 09 — Coding Mode  
**Doc:** `docs/architecture/subsystems/09-coding-mode.md`  
**Crates verified:** 14  
**Date:** 2026-05-16  

---

## Summary

| Metric | Count |
|---|---|
| Crates inspected | 14 |
| ✅ Fully accurate crates | 7 |
| ⚠️ Drift found | 5 |
| ❌ Wrong claims | 6 |
| 🔍 Missing in docs | 4 |
| 📋 TODO/FIXME catalogued | 336 |

**Overall assessment:** The doc is largely accurate on architecture and public API shapes, but contains several factually incorrect claims about stub status, variant counts, and constant locations. The most significant issue is the mischaracterization of `coding-memory` Reforge phases as "all stubbed" when two of the four have full implementations, and the claim that "no physical DELETE ever runs" when `SessionEndPass` performs physical deletions.

---

## Per-Crate Findings

### `klynt-core`

**✅ Accurate**
- `ToolKitBuilder` exists with all documented fields except `non_ui_policy`.
- 21 tools registered in 4 groups verified in `src/registry/builder.rs`:
  - `register_read_only`: 7 tools (read, list_dir, glob, grep, tool_search, ask_user, web_fetch)
  - `register_mutating`: 5 tools (bash, write, edit, apply_patch, notebook_edit)
  - `register_plan_mode`: 2 tools (enter_plan_mode, exit_plan_mode)
  - `register_recall`: 8 stubs (recall_index, recall_timeline, recall_fetch, trace_causes, check_dead_ends, recall_facts_as_of, recall_change_history, recall_decision_points)
- `with_cwd()` builder method exists.
- All claimed modules (`tools/`, `registry/`, `snapshots/`, `privacy/`) exist.

**⚠️ Drift**
- `register_read_only` doc comment says "Six read-only … tools" but registers 7.
- `register_all` doc comment says "thirteen klynt-core primitive tools" but 7+5+2 = 14 primitive tools (plus 8 recall stubs = 22 total registrations).
- Doc omits `non_ui_policy: common::tool_channel::NonUiPolicy` field on `ToolKitBuilder`.
- Doc claims `session_key: SessionKey` but actual type is `String`.

**❌ Wrong**
- Doc claims `ask_user` uses `LONG_RUNNING_TOOL_TIMEOUT = 600s` in klynt-core. The constant exists in `crates/agent/src/execution/core.rs`, not in `klynt-core`.

---

### `coding-agents-md`

**✅ Accurate**
- `WorkspaceAgentsSource` exists with `new()`, `with_global()`, `build_bundle()`, and `walk()` methods.
- `AgentsMdSource` struct has `path`, `dir`, `contents` fields.
- `walk_agents_md()` walks ancestor chain outermost-first.
- Module structure matches `src/lib.rs` only (no submodules).

---

### `coding-ingest`

**✅ Accurate**
- 5 adapters exist: `claude_code/`, `codex/`, `kimi_cli/`, `opencode/`, `git_post_commit.rs`.
- `AgentEvent::V1(AgentEventV1)` shape matches code exactly (all 8 fields).
- `AgentSource` has 5 variants: ClaudeCode, Codex, KimiCli, OpenCode, KlyntCli.
- `hook_cli::run()` dispatch matches: "status", "context", "git-post-commit", else source validation.
- kimi-cli and opencode short-circuit with "poll-only (Phase 7)" message.
- Socket path fallback (`$KLYNTBOT_HOOK_SOCKET` → `~/.klyntbot/ingest.sock`) and buffer fallback (`ingest-buffer.jsonl`) match.
- `tokio::runtime::Builder::new_current_thread()` per invocation confirmed.
- Cross-CLI normalization proptest at `tests/cross_cli_normalization.rs` exists.

**⚠️ Drift**
- `IngestAdapter` trait has an additional `source_name(&self) -> &'static str` method not mentioned in the doc.
- **EventKind count:** The doc claims 21 variants (9 base + 10 klynt-only + 2 background), but the enum has **22 variants**. The doc omits `GitCommit` from the "Base" group; it should be 10 base variants. `GitCommit` is produced by the `git_post_commit` adapter, one of the 5 claimed adapters.

---

### `coding-memory`

**✅ Accurate**
- `Distiller` exists with `accept_event()` → `tokio::spawn(distill_turn())` fire-and-forget semantics.
- Phase A (`phase_a::compute_turn_trace`) produces `TurnTrace` with all documented fields.
- Phase A.5 refactor episode extraction exists.
- Phase B (`phase_b::invoke_llm`) uses default model `claude-haiku-4-5-20251001`, timeout `30s`, and cost-ceiling guard.
- Phase C (`phase_c::reconcile`) returns `Add` / `Supersede { predecessor_id }` / `Noop`.
- `DistillerWriter::complete_supersede` sets `superseded_at` + `superseded_by` (logical-time).
- `ReforgeWriter::set_superseded_by` sets `valid_until` + `superseded_by` (bi-temporal).
- `ReforgeWriter::reject_delete()` always returns error.
- `ReforgeWriter::demote_stability()` sets convergence_score → `0.01`.
- 8 MCP recall tools in `CODING_MEMORY_MCP_TOOLS` const match doc exactly.
- `CodingMemoryToolset::mcp_tools()` dispatches all 8 tools.

**❌ Wrong (Most Serious)**
- **Doc claims:** "4 Reforge phases in `coding-memory` return `NotImplementedInPhase { required_phase: 5 }`" and lists `CodingSynthesisPhase`, `RuleArtifactGenerationPhase`, `SessionEndPass`, `CrossSessionDedup` as all stubbed.
- **Reality:** There are **two sets** of these types:
  1. `coding-memory/src/reforge_phase.rs` — contains stubs that return `Err(phase(5))` for `CodingSynthesisPhase` and `RuleArtifactGenerationPhase`.
  2. `coding-memory/src/reforge/` — contains **fully implemented** versions:
     - `reforge/coding_synthesis.rs`: `CodingSynthesisPhase::run()` fetches fix attempts, workflow patterns, repo context, causal chains, applies promote actions, and writes to `procedural_rules` and `semantic_facts`.
     - `reforge/rule_artifacts.rs`: `RuleArtifactGenerationPhase::run()` discovers repos, builds artifact plans, calls LLM handler, and writes managed blocks to `CLAUDE.md`, `AGENTS.md`, `.cursorrules`, `.continue/rules/klyntbot.md`.
     - `reforge/session_end.rs`: `SessionEndPass::run()` performs Hebbian bump, within-session dedup, causal-edge detection, stale-candidate resolution, community membership, and summary generation. **Fully implemented.**
     - `reforge/cross_session_dedup.rs`: `CrossSessionDedup::run()` performs vector-similarity or exact-match dedup and bi-temporal supersession. **Fully implemented.**
- **Conclusion:** Only 2 of the 4 phases are truly stubbed. The doc's claim that all 4 return `NotImplementedInPhase { required_phase: 5 }` is **factually incorrect**.

- **Doc claims:** "No physical DELETE ever runs through either path" and "Both keep all rows on disk."
- **Reality:** `SessionEndPass::run` calls `ep_repo.delete_by_id(id)` for:
  1. Within-session dedup of duplicate `fix_attempt` episodes (lines 181–186 of `session_end.rs`).
  2. Stale-candidate resolution (`resolve_stale_candidates`, lines 279–286).
  This is a **direct contradiction** of the "no physical DELETE" invariant.

**🔍 Missing**
- `SessionEndPass` performs physical DELETEs — not mentioned in doc.
- `reforge_phase.rs` stub types are exported alongside the real implementations in `reforge/`. The doc does not explain this dual-surface.

---

### `feature-coding-bash`

**✅ Accurate**
- `JobSupervisor` implements `tools_core::JobSupervisorHandle` (line 620 of `supervisor.rs`).
- `RingFile` ring-buffer I/O with bisect-on-overflow exists.
- `GateClassifier` classifies output into `GateResult` with `FailureKind` variants.
- `MacOsSeatbeltRunner` is used for sandboxing.
- `ChildBackend::Pty` supports PTY-backed jobs with resize/attach/detach.
- `BashJobRepo` SQLite persistence exists.
- `output_delta` / `read_delta` incremental reads exist.

**📋 Tech Debt**
- 18 `TODO` / `FIXME` / `unimplemented!` / `todo!()` items found in source.

---

### `feature-coding-todo`

**✅ Accurate**
- `CodingTodoFeature` implements `FeaturePackage` with name `"coding_todo"`.
- `CodingTodoTool` is the single tool exposed.
- `PlanModeInjector` exists.
- `PlanModeView` and `CodingTodoView` exist.

**📋 Tech Debt**
- 312 `TODO` / `FIXME` / `unimplemented!` / `todo!()` items found in source (the highest count in this subsystem).

---

### `klynt-protocol`

**✅ Accurate**
- `HookEventName` enum has exactly 13 variants matching doc.
- `Op` and `Submission` types exist.
- `SessionKey` is re-exported from `common::types`.
- `HookExecutionMode::InProcess` variant exists alongside `Subprocess`.
- `HookRunStatus`, `HookOutputEntry`, `HookRunSummary`, `HookCompletedEvent` all exist.

---

### `klynt-hooks`

**✅ Accurate**
- `HookEngine` exists with `fire()` method dispatching all 13 events.
- `HookFireInput` has 13 variants matching `HookEventName`.
- `HookOutcome` variants: `Allow`, `Block { reason }`, `ModifyArgs { args }`, `LifecycleNoOp`.
- Default timeout is `5000ms` (`hook.timeout_ms.unwrap_or(5000)` in `command_runner.rs`).
- Execution is **always subprocess** (`sh -c <hook.command>`) — no other backend.
- `Hook.fail_open` field exists in `schema.rs` but is **never read** by the dispatcher. Fail-open behavior is hardcoded: empty stdout → `Allow`, parse error → `Allow`, spawn error → returns error but caller in `dispatcher.rs` ignores it and continues.

**⚠️ Drift**
- `HookExecutionMode::InProcess` is defined in `klynt-protocol::HookExecutionMode`, not in `klynt-hooks`. The doc discusses it under the `klynt-hooks` section. The `klynt-hooks` crate never imports or references `HookExecutionMode`.

---

### `klynt-execpolicy`

**✅ Accurate**
- `Decision` enum has `Allow`, `Ask`, `Forbid`, `FallThrough`.
- `Evaluation::from_matches` uses `max()` on matched rules.
- Session-only rules stored in `RwLock<Vec<(Vec<String>, Decision)>>` and checked **before** compiled rules in `matches_for_command_with_options`.
- `append_session_allow_prefix` exists and mutates in-memory rules.
- `heuristics_fallback` is invoked when no rules match.

---

### `klynt-skill-loader`

**✅ Accurate**
- `SkillSource` enum has 6 variants with priorities: User=0, ReforgePrivate=1, ReforgeTeam=2, Project=3, Mcp=4, SkillsMarketplace=5.
- `SkillIndex::insert` uses `existing.source.priority() >= skill.source.priority()` — higher wins.
- `DynamicWalker::discover_above` walks up to `cwd_boundary` and stops.
- `SkillActivator` has `path_match_cache: LruCache<PathBuf, Vec<String>>` with capacity 256.
- `ConditionalSkill` with `glob_set: GlobSet` exists.
- `max_active_skills` default is 30.
- `touch_path()` checks cache, then glob matches, then caps at `max_active_skills`.
- `DiscoveryRoots` has 4 discovery roots (klyntbot_home, repo_id, repo_root, cwd).

---

### `klynt-pty`

**✅ Accurate**
- `ChildHandle::Process` and `ChildHandle::Pty` variants exist.
- `spawn_with_pgrp` calls `libc::setpgid(0, 0)` on Unix.
- `klynt_pty::kill_process_group` is re-exported and used by `feature-coding-bash`.

---

### `klynt-git-utils`

**✅ Accurate**
- `create_ghost_commit` and `restore_ghost_commit` exist.
- `GhostSnapshotConfig::default()` has `max_file_bytes: 10 * 1024 * 1024` (10 MiB) and `max_dir_entries: 200`.
- Uses `GIT_INDEX_FILE` pointing at a temp file.
- Calls `git commit-tree -m "klynt-snapshot"` directly; no branch ref is moved.
- `restore_ghost_commit` calls `git restore --source <ghost-sha> --worktree -- .` and deletes post-snapshot files not in ghost tree and not preexisting untracked.
- Silent-fail on missing `.git/` between snapshot and restore: `get_git_repo_root` returns error, which propagates up and is logged via `tracing::error!`.

**⚠️ Drift**
- Doc lists excluded dirs as: `node_modules`, `.venv`, `target`, `dist`, `build`, `.next`, `.cache`.
- Actual default `excluded_path_components` also includes `"venv"` (without the leading dot). So there are 8 excluded components, not 7.

---

### `klynt-truncation`

**✅ Accurate**
- `truncate_middle_chars` exists and implements UTF-8-safe middle truncation with `[...] omitted N bytes [...]` marker.
- `formatted_truncate_text` prepends `"Total output lines: N\n\n"` header.
- `TruncationPolicy::Bytes` and `TruncationPolicy::Tokens` exist with 4 bytes/token approximation.

---

### `lsp-client`

**✅ Accurate**
- All methods are stubbed with `// TODO(T5)` comments.
- `LspClientHandle::diagnostics_for` returns `Ok(vec![])`.
- `LspClientHandle::document_symbols` returns `Ok(vec![])`.
- `server_pool::get_or_spawn` returns a server handle but does nothing.
- The crate compiles and is wired but produces no useful output.

**📋 Tech Debt**
- 5 `TODO(T5)` items in source (`src/lib.rs:42`, `src/lib.rs:59`, `src/server_pool.rs:24`, `src/server_pool.rs:58`, `src/server_pool.rs:86`).

---

## Cross-Reference Check

| Link in doc | Target | Status |
|---|---|---|
| `../00-overview.md` | `docs/architecture/00-overview.md` | ✅ Exists |
| `./04-agent-runtime.md` | `docs/architecture/subsystems/04-agent-runtime.md` | ✅ Exists |
| `./05-cognitive-memory.md` | `docs/architecture/subsystems/05-cognitive-memory.md` | ✅ Exists |
| `./07-tools-framework.md` | `docs/architecture/subsystems/07-tools-framework.md` | ✅ Exists |
| `./10-sandboxing-security.md` | `docs/architecture/subsystems/10-sandboxing-security.md` | ✅ Exists |
| `./11-channels-mcp.md` | `docs/architecture/subsystems/11-channels-mcp.md` | ✅ Exists |
| `../crates/coding-memory.md` | `docs/architecture/crates/coding-memory.md` | ✅ Exists (doc marked as "planned" but file exists) |
| `../crates/coding-ingest.md` | `docs/architecture/crates/coding-ingest.md` | ✅ Exists (doc marked as "planned" but file exists) |

---

## Priority Fixes for Doc

1. **Correct Reforge phase status** — `SessionEndPass` and `CrossSessionDedup` are fully implemented, not stubbed. Only `CodingSynthesisPhase` and `RuleArtifactGenerationPhase` in `reforge_phase.rs` are stubs.
2. **Fix "no physical DELETE" claim** — `SessionEndPass` performs `delete_by_id` in within-session dedup and stale-candidate resolution.
3. **Fix EventKind count** — Total is 22, not 21. Base group should include `GitCommit` (10 base variants).
4. **Move `LONG_RUNNING_TOOL_TIMEOUT` reference** — It lives in `crates/agent`, not `klynt-core`.
5. **Fix `session_key` type** — `ToolKitBuilder.session_key` is `String`, not `SessionKey`.
6. **Add `venv` to excluded dirs** — Ghost commit default exclusions include `"venv"` as well as `".venv"`.
7. **Clarify `HookExecutionMode`** — The enum lives in `klynt-protocol`, not `klynt-hooks`.
