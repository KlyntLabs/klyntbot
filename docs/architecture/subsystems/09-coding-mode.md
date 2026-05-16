# Subsystem 09 — Coding Mode

> **Status:** 🟡 In Progress (4 Reforge phases stubbed at `required_phase: 5`; `lsp-client` all-stubs; `feature-coding-bash` PTY supervisor stable)
> **Status last verified:** 2026-05-16
> **Crates:** `klynt-core`, `coding-agents-md`, `coding-ingest`, `coding-memory`, `feature-coding-bash`, `feature-coding-todo`, `klynt-protocol`, `klynt-hooks`, `klynt-execpolicy`, `klynt-skill-loader`, `klynt-pty`, `klynt-git-utils`, `klynt-truncation`, `lsp-client` *(14 crates — the largest single subsystem)*
> **Parent overview:** [`00-overview.md`](../00-overview.md)

---

## TL;DR

Klyntbot's Claude-Code-style coding experience. **`klynt-core`** registers **21 tools** (7 read-only, 5 mutating, 2 plan-mode, 8 recall stubs). **`coding-ingest`** normalizes events from **5 external CLIs** (Claude Code, Codex, kimi-cli, opencode, git post-commit) into a unified `AgentEvent` stream with 21 `EventKind` variants. **`coding-memory`** runs a **3-phase Distiller** (extractive → LLM synthesis → reconciliation) that writes `episodic_memories` + `semantic_facts` per turn, plus 8 MCP recall tools. **`klynt-execpolicy`** + **`klynt-sandbox`** + **`klynt-hooks`** gate every command. **`klynt-git-utils`** captures undo state via **ghost commits** that never touch branch refs.

Several pieces are scaffolded but not finished: `lsp-client` is all-stubs, the 4 coding-memory Reforge phases all return `NotImplementedInPhase { required_phase: 5 }`, the `InProcess` hook execution mode is typed but dead, and the kimi-cli/opencode adapters are hook-registered but actually poll-only.

---

## Architecture diagram

```mermaid
flowchart TB
    classDef tool fill:#fbe9e7,stroke:#d84315,color:#bf360c
    classDef ingest fill:#fff3e0,stroke:#f57c00,color:#e65100
    classDef mem fill:#fce4ec,stroke:#ad1457,color:#880e4f
    classDef hook fill:#f3e5f5,stroke:#7b1fa2,color:#4a148c
    classDef sandbox fill:#ffcdd2,stroke:#c62828,color:#b71c1c
    classDef util fill:#fffde7,stroke:#fbc02d,color:#f57f17
    classDef stub fill:#f5f5f5,stroke:#999,color:#616161

    KC[klynt-core<br/><i>ToolKitBuilder · 21 tools</i><br/>read · list_dir · glob · grep · web_fetch · ask_user · tool_search<br/>bash · write · edit · apply_patch · notebook_edit<br/>enter_plan_mode · exit_plan_mode<br/>8 recall stubs]:::tool
    FCB[feature-coding-bash<br/><i>PTY supervisor</i><br/>ring-buffer · attach/detach<br/>JobSupervisorHandle impl]:::tool
    FCT[feature-coding-todo<br/><i>TodoWrite scratchpad</i>]:::tool

    CI[coding-ingest<br/><i>5 adapters · AgentEvent::V1<br/>hook_cli::run · OpencodePoller<br/>21 EventKind variants</i>]:::ingest
    CAM[coding-agents-md<br/><i>WorkspaceAgentsSource<br/>walks AGENTS.md chain</i>]:::ingest

    CM[coding-memory<br/><i>Distiller (3-phase)<br/>ReforgeWriter (bi-temporal)<br/>4 Reforge phases (stubbed)<br/>8 MCP recall tools<br/>TreeSitterExtractor</i>]:::mem

    KP[klynt-protocol<br/><i>Op · Submission<br/>13 HookEventName variants<br/>adapted from codex-rs</i>]:::hook
    KH[klynt-hooks<br/><i>HookEngine · subprocess<br/>5s default timeout · fail-open<br/>InProcess mode TYPED-BUT-DEAD</i>]:::hook
    KE[klynt-execpolicy<br/><i>Starlark prefix rules<br/>Decision: Allow/Forbid/Ask/FallThrough</i>]:::sandbox
    KSL[klynt-skill-loader<br/><i>4 discovery roots<br/>higher priority WINS<br/>globset · LRU cache · DynamicWalker</i>]:::sandbox

    KPTY[klynt-pty<br/><i>ChildHandle::Process or Pty<br/>process-group kill</i>]:::util
    KGU[klynt-git-utils<br/><i>create_ghost_commit<br/>restore_ghost_commit<br/>uses GIT_INDEX_FILE</i>]:::util
    KT[klynt-truncation<br/><i>truncate_middle_chars<br/>ported from codex-rs</i>]:::util

    LSP[lsp-client<br/><i>LspServerPool · LspClientHandle<br/>ALL METHODS STUBBED (TODO T5)</i>]:::stub

    CI --> CM
    KC --> KE
    KC --> KH
    KC --> KGU
    FCB --> KPTY
    FCB --> KC
    KC -.uses for recall.-> CM
    CAM -.injected at session start.-> KC
    KH --> KP
    KSL -.discovers skills.-> KC
```

---

## Mental model

Coding mode is **a separate operating mode**, not a flavor of assistant mode. Different runtime (`CodingThreadRuntime`), different soul (`KLYNTBOT-coding.md`), different tool set (`ChannelMask::CODING_ONLY` for mutating tools), different storage tables (`coding_snapshots`, `coding_thread_messages`, `coding_tool_calls`). The agent runtime (`crates/agent`) is shared; everything else is dedicated.

The **6 conceptual layers** in this subsystem:

| Layer | Crates | Role |
|---|---|---|
| **Tools** | `klynt-core`, `feature-coding-bash`, `feature-coding-todo` | What the LLM can do |
| **Ingest** | `coding-ingest`, `coding-agents-md` | What flows in (events from external CLIs + AGENTS.md context) |
| **Memory** | `coding-memory` | What's remembered (Distiller pipeline, recall surface) |
| **Hooks & Protocol** | `klynt-protocol`, `klynt-hooks` | 13-event lifecycle hooks (adapted from codex-rs) |
| **Policy & Discovery** | `klynt-execpolicy`, `klynt-skill-loader` | Pre-execution gates and skill activation |
| **Utilities** | `klynt-pty`, `klynt-git-utils`, `klynt-truncation`, `lsp-client` | PTY, git ops, truncation, LSP (stubbed) |

### Three non-obvious facts to internalize

1. **`klynt-protocol` and `klynt-hooks` are adapted from codex-rs.** Both crate docs say so explicitly. The 13-event hook schema, the `Op`/`Submission` types, the `HookEventName` enum — all trace to upstream codex-rs.
2. **`SkillSource` priority is "higher number wins."** `User=0`, `ReforgePrivate=1`, `ReforgeTeam=2`, `Project=3`, `Mcp=4`, `SkillsMarketplace=5`. So `Project` skills override `User` skills on collision. The reverse of what most file-discovery conventions imply.
3. **Ghost commits never touch branch refs.** `create_ghost_commit` uses `GIT_INDEX_FILE` pointing at a temp file, then `git commit-tree` directly. Your working branch HEAD is never moved. The SHA lives only in `coding_snapshots.ghost_commit_sha`.

---

## Reference

### `klynt-core` — 21 tools in 4 groups

**`register_read_only` (7 tools, `ChannelMask::ALL`)**

| Tool | Purpose |
|---|---|
| `read` | Read file contents |
| `list_dir` | List directory entries |
| `glob` | Glob file matching |
| `grep` | Regex file search |
| `tool_search` | MCP tool discovery |
| `ask_user` | Synchronous user prompt (uses `LONG_RUNNING_TOOL_TIMEOUT = 600s`) |
| `web_fetch` | HTTP fetch — policy-gated, mirror-learning wired |

**`register_mutating` (5 tools, `ChannelMask::CODING_ONLY`)**

| Tool | Purpose | Snapshot-aware? |
|---|---|---|
| `bash` | Shell execution (policy + privacy + mirror) | No (output-only) |
| `write` | Whole-file write | ✅ |
| `edit` | In-place edit | ✅ |
| `apply_patch` | Unified-diff apply | ✅ |
| `notebook_edit` | Jupyter cell edit | ✅ |

**`register_plan_mode` (2 tools, `ChannelMask::CODING_ONLY`)**

| Tool | Purpose |
|---|---|
| `enter_plan_mode` | Switch the policy gate to PlanMode (writes restricted to plan file) |
| `exit_plan_mode` | Exit plan mode |

**`register_recall` (8 stubs)**

`recall_index`, `recall_timeline`, `recall_fetch`, `trace_causes`, `check_dead_ends`, `recall_facts_as_of`, `recall_change_history`, `recall_decision_points` — all delegate to `CodingRecallService` (or return `NotImplementedInPhase` when service is absent). These are the **8 MCP coding-memory tools** that get auto-added to MCP exposure via `EXPLICIT_TOOL_ALLOWLIST` (see [`11-channels-mcp.md`](./11-channels-mcp.md)).

### `ToolKitBuilder` — DI surface

```rust
pub struct ToolKitBuilder {
    pub cwd: PathBuf,
    pub policy: Arc<Policy>,                            // klynt-execpolicy
    pub privacy: Arc<PrivacyGuard>,
    pub bus: Arc<DomainEventBus>,
    pub repos: Repos,
    pub hook_engine: Option<Arc<HookEngine>>,
    pub snapshot_repo: Option<Arc<SnapshotRepo>>,
    pub session_key: SessionKey,
    pub history_repo: Option<...>,
    // mirror-learning fields:
    pub mirror_learning_enabled: bool,
    pub mirror_min_approvals: u32,
    pub mirror_cooldown_seconds: u64,
    pub repo_id: String,
}
```

Sub-agents call `builder.with_cwd(new_cwd)` before `register_all`, so subagent invocations get fresh working directories without rebuilding policy/bus/repos.

### `coding-ingest` — 5 adapters

| Adapter | Mode | Source data | Notes |
|---|---|---|---|
| `claude_code` | Hook-driven | stdin JSON from Claude Code hooks | Sub-10ms hot path |
| `codex` | Hook-driven | stdin JSON from Codex hooks | Same pattern |
| `kimi_cli` | Poll-only | `~/.kimi/sessions/<hash>/<uuid>/wire.jsonl` | **Hook-registered but short-circuits with "poll-only (Phase 7)"** |
| `opencode` | Poll-only | opencode SQLite (`message`/`part` tables, diffed by `time_created`) | `OpencodePoller` is the daemon |
| `git_post_commit` | Hook-driven | stdin JSON from `.git/hooks/post-commit` | Standalone `.rs` file (easy to miss when grepping `adapters/`) |

The `IngestAdapter` trait: `fn parse(&self, hook_event: &str, raw: &[u8]) -> Result<Option<AgentEvent>>`. Stateless.

### `AgentEvent::V1` — full shape

```rust
pub enum AgentEvent { V1(AgentEventV1) }

pub struct AgentEventV1 {
    pub id: Uuid,
    pub source: AgentSource,        // ClaudeCode | Codex | KimiCli | OpenCode | KlyntCli
    pub session_id: String,
    pub turn_id: Option<String>,
    pub cwd: PathBuf,
    pub repo: Option<RepoScope>,
    pub occurred_at: Timestamp,
    pub kind: EventKind,
}
```

**21 `EventKind` variants** total:

| Group | Variants |
|---|---|
| Base (9) | `SessionStart`, `SessionEnd`, `UserPrompt`, `AssistantMsg`, `ToolCall`, `FileEdit`, `TestRun`, `CompactEvent`, `Error` |
| klynt-cli only (10) | `SkillActivated`, `RecallInjected`, `ApprovalDecision`, `SandboxApplied`, `FileEditEnriched`, `TestRunEnriched`, `ProviderCall`, `CompressionApplied`, `MirrorAlert`, `SkillRoutingTrace` |
| Background jobs (2) | `BackgroundJobLifecycle`, `BackgroundJobOutputBisect` |

The 10 klynt-cli-only variants are why Klynt sessions produce richer cognitive signals than external CLI sessions.

### `hook_cli::run()` short-circuit (sub-10ms hot path)

The `klyntbot-hook` binary and `desktop --hook` share this function:

```
1. Dispatch on argv[1]:
   - "status"           → run_status()
   - "context"          → run_context()
   - "git-post-commit"  → run_git_post_commit()
   - else (validates source in {claude-code, codex, kimi-cli, opencode}):
       Read stdin → adapter.parse() → enrich with RepoScope → apply ExcludeSet
       → HookClient::send() (Unix socket first; falls back to ingest-buffer.jsonl)
2. Creates a `new_current_thread` Tokio runtime per invocation — designed to return
   before Claude Code's hook timeout (~3s typical, target sub-10ms).
```

Socket path: `$KLYNTBOT_HOOK_SOCKET` or `~/.klyntbot/ingest.sock`. Buffer fallback path: `~/.klyntbot/ingest-buffer.jsonl`.

### `coding-memory` — the Distiller (3 phases)

```
Distiller::accept_event(event)
   → TurnBuffer::push(event)
   → on TurnBoundary::Fire:
       tokio::spawn(distill_turn())   ← fire-and-forget; failures never block ingest

distill_turn():
   ├── Phase A (extractive, always runs)
   │     phase_a::compute_turn_trace()
   │       → produces TurnTrace { files_read, files_modified, commands, test_outcomes,
   │                              errors, token_usage }
   │       → persisted as EpisodicMemory of kind "turn_trace"
   │     Phase A.5: tree-sitter anchored refactor episodes ("refactor_episode")
   │
   ├── Phase B (LLM synthesis)
   │     phase_b::invoke_llm()
   │       Default provider: claude-haiku-4-5-20251001
   │       Timeout: 30s
   │       Cost-ceiling guard: blocks if cost_ceiling_usd exceeded
   │       Returns Vec<Observation> via record_observation tool
   │       Transient failures → enqueue to DistillationRetryRepo
   │
   └── Phase C (reconciliation)
         phase_c::reconcile() per observation:
           Add                         → write fresh row
           Supersede { predecessor_id } → DistillerWriter::complete_supersede (logical-time)
           Noop                         → skip
         Auto-derive: failed FixAttempt → DeadEndAttempt counterfactual fact
```

### `coding-memory` Reforge phases (all stubbed at `required_phase: 5`)

| Phase | Class | Trigger | Status |
|---|---|---|---|
| 2.5 | `CodingSynthesisPhase` | Reforge cycle | 🔴 `NotImplementedInPhase { required_phase: 5 }` |
| 3.5 | `RuleArtifactGenerationPhase` | Reforge cycle (writes managed blocks into `CLAUDE.md`, `AGENTS.md`, `.cursorrules`, `.continue/rules/klyntbot.md` when patterns reach `confidence ≥ 0.7`, `stability ≥ 0.5`) | 🔴 Stub |
| — | `SessionEndPass` | End of session | 🔴 Stub |
| — | `CrossSessionDedup` | Reforge cycle (via `ReforgeWriter::set_superseded_by`) | 🔴 Stub |

**`ReforgeWriter` is the only sanctioned removal path:**

| Method | Behavior |
|---|---|
| `reject_delete()` | Always returns error |
| `demote_stability()` | Sets `convergence_score → 0.01` |
| `set_superseded_by()` | Bi-temporal: sets `valid_until` + `superseded_by`; both rows remain on disk |

**Two distinct supersede paths** — these are confusing because they share a verb:

| Path | Where | Semantics | When |
|---|---|---|---|
| `DistillerWriter::complete_supersede` | Distiller Phase C | Logical-time: sets `superseded_at` + `superseded_by` | Within-session, per-turn reconciliation |
| `ReforgeWriter::set_superseded_by` | Reforge phases | Bi-temporal: sets `valid_until` + `superseded_by` | Cross-session, batch dedup |

Both keep all rows on disk. **No physical DELETE ever runs through either path.**

### `klynt-hooks` — 13 event types

From `klynt-protocol::HookEventName`:

`PreToolUse`, `PostToolUse`, `SessionStart`, `UserPromptSubmit`, `Stop`, `SessionEnd`, `PreCompact`, `PostCompact`, `PreFileEdit`, `PostFileEdit`, `Notification`, `SubagentSpawn`, `Error`.

**`HookOutcome` variants:** `Allow`, `Block { reason }`, `ModifyArgs { args }`, `LifecycleNoOp`.

**Execution:** Always subprocess (`sh -c <hook.command>`). 5000ms default timeout. **Fail-open is hardcoded** — `Hook.fail_open` field exists in schema but dispatcher always uses fail-open behavior. `HookExecutionMode::InProcess` is **typed but dead** — no dispatch path implements it.

### `klynt-execpolicy` — Decision lattice

| Variant | Semantics |
|---|---|
| `Allow` | Auto-allow, no prompt |
| `Forbid` | Block; surfaced to LLM as error |
| `Ask` | Prompt via `ApprovalGate` |
| `FallThrough` | Defer to next rule / heuristic |

`Evaluation::from_matches` picks `max()` across matched rules. Session-only rules (in `RwLock<Vec<...>>`) take precedence over compiled rules. When no rules match, `heuristics_fallback(cmd)` provides a default.

### `klynt-skill-loader` — 4 discovery roots (priority **higher wins**)

| Priority | Source | Path |
|---:|---|---|
| 0 | `User` | `~/.klyntbot/skills/` |
| 1 | `ReforgePrivate` | `~/.klyntbot/project-skills/<sanitized-repo-id>/` |
| 2 | `ReforgeTeam` | `<repo_root>/.klyntbot/team-skills/` |
| 3 | `Project` | `<repo_root>/.klyntbot/skills/` |
| 4 | `Mcp` | added via `scan_mcp_server` |
| 5 | `SkillsMarketplace` | (added separately) |

**Path-conditional glob activation:** `KlyntFrontmatter.paths: Vec<String>` lists glob patterns. Skills with non-empty `paths` become `ConditionalSkill { glob_set: GlobSet }`. `touch_path(path)` checks all conditional skills against the path; cap = `max_active_skills` (default 30).

**LRU cache:** `path_match_cache: LruCache<PathBuf, Vec<String>>` (capacity 256). Cleared on dynamic-walker discovery of new conditional skills.

**`DynamicWalker`:** walks ancestor dirs above a touched file up to `cwd_boundary` (the repo root), discovering new `.klyntbot/skills/` dirs not seen before. Hot-reloads skill discovery without a restart.

### `feature-coding-bash` — concrete `JobSupervisorHandle`

The only concrete impl of `tools_core::JobSupervisorHandle` in the workspace. Provides:
- Live-job registry (in-memory + SQLite-persisted `BashJobRepo`)
- Ring-buffer I/O for stdout/stderr (`output_delta(since, block, timeout_ms)`)
- Gate classifier for `bash` tool — uses `klynt-execpolicy` + privacy guard
- PTY operations via `klynt-pty` (`write_stdin`, `resize`, `attach`, `detach`)
- Spawns under `klynt_sandbox::MacOsSeatbeltRunner` on macOS, Landlock+bwrap on Linux

### `lsp-client` — all stubs

```rust
// crates/lsp-client/src/lib.rs:42
pub async fn diagnostics_for(...) -> Result<Vec<...>> {
    // TODO(T5): Send textDocument/didOpen, wait for publishDiagnostics, return
    Ok(vec![])
}
```

`document_symbols`, `server_pool::get_or_spawn` (returns server but does nothing), shutdown, cancel — all the same shape. **The crate compiles and is wired but produces no useful output today.**

---

## Workflows

### Coding session lifecycle (end-to-end)

```
1. coding_thread_start(cwd, session_key) called from desktop / agent layer
   ↓
2. WorkspaceAgentsSource::build_bundle() walks ancestor AGENTS.md chain
   → returns formatted <INSTRUCTIONS> blocks
   → injected as synthetic user message at session open
   ↓
3. CodingThreadRuntime::init:
   - Build ToolKitBuilder (policy from ~/.klyntbot/rules/, privacy, sandbox, hooks, snapshots, mirror)
   - register_all(&tool_registry)  ← 21 tools registered
   - klynt-skill-loader::SkillIndex::discover()  ← 4 roots scanned
   - SkillActivator constructed with `always_activate` set
   ↓
4. Per user prompt:
   turn_handler → AgentRuntime → ExecutionCore → LLM streaming
   ↓
5. Per tool call:
   - bash: ApprovalGate (CodingApprovalPolicy::Default)
           → MacOsSeatbeltRunner / Landlock+bwrap
           → feature-coding-bash::JobSupervisorHandle.spawn
           → ring-buffer I/O
   - mutating tool (write/edit/apply_patch/notebook_edit):
           → SnapshotRepo::try_record_with_ghost() BEFORE mutation
           → HookEngine.fire(PreFileEdit)
           → execute
           → HookEngine.fire(PostFileEdit)
   ↓
6. Per tool event:
   coding-ingest::ClaudeCodeAdapter normalizes → AgentEvent::V1
   → IngestEventLogRepo write
   ↓
7. On turn boundary:
   Distiller::accept_event marks turn complete
   → tokio::spawn(distill_turn())
     → Phase A (extractive) writes EpisodicMemory "turn_trace"
     → Phase B (LLM synthesis) writes Observations
     → Phase C (reconciliation) Add/Supersede/Noop per observation
   ↓
8. On session end:
   - SessionEndPass (stub)
   - All AgentEvents flushed
```

### Ghost commit lifecycle

```
1. mutating tool (write/edit/apply_patch/notebook_edit) about to mutate file F
   ↓
2. SnapshotRepo::try_record_with_ghost(session, msg_id, F):
   a. klynt_git_utils::get_git_repo_root(F.parent)
      → If F is inside a git repo:
         klynt_git_utils::create_ghost_commit(&root, &GhostSnapshotConfig::default()):
            - Set GIT_INDEX_FILE = <temp file>
            - git add --force <eligible files> (skips dirs in EXCLUDED:
                                                 node_modules, .venv, target,
                                                 dist, build, .next, .cache;
                                                 size limit 10 MiB;
                                                 dir count limit 200)
            - git write-tree
            - git commit-tree -m "klynt-snapshot" → returns SHA
            - NO branch ref touched
         INSERT INTO coding_snapshots
            (file_path="<ghost>", content_before=X'', content_hash="ghost",
             ghost_commit_sha, ghost_repo_root, ghost_preexisting_untracked_json)
      → If NOT in git repo (or any git error):
         Silently fall back to BLOB storage:
         INSERT INTO coding_snapshots (file_path=F, content_before=BLOB, ...)
   ↓
3. Tool performs mutation
   ↓
4. User invokes rewind:
   restore_ghost_commit(ghost_sha):
      - git restore --source <ghost-sha> --worktree -- .
      - Remove post-snapshot files NOT in the ghost tree AND NOT in preexisting_untracked_files
```

**Silent-fail case:** If `.git/` is deleted between snapshot and rewind, `restore_ghost_commit` fails silently (logs `tracing::error!`, returns error). The snapshot row is still there but the ghost SHA is now an unresolved reference.

### A `bash` tool call (with sandbox + ghost commit + hooks)

```
LLM emits: bash({command: "cargo build"})
   ↓
1. ApprovalGate.check:
   - CodingApprovalPolicy::Default classify("bash", None, args)
   - Glob match: allow `bash(cargo *)` → Allow class=Safe
   - No prompt needed
   ↓
2. HookEngine.fire(PreToolUse, "bash", args):
   - For each matching hook (regex on tool name): sh -c <hook.command> < {JSON args}
   - 5s default timeout
   - HookOutcome::Allow → continue
   - HookOutcome::Block → return error
   - HookOutcome::ModifyArgs → args replaced
   ↓
3. feature-coding-bash::JobSupervisor.spawn(JobSpec):
   - MacOsSeatbeltRunner.build_sandboxed_command("/bin/sh", &["-c", "cargo build"])
   - Spawns under sandbox-exec with generated .sbpl profile
   - PTY-backed via klynt-pty if interactive
   - Returns JobView with job_id
   - Output streams to ring buffer
   ↓
4. JobSupervisor.output_delta(job_id, since=0, block=true, timeout_ms=30_000):
   - Returns ring read (incremental output)
   ↓
5. HookEngine.fire(PostToolUse, "bash", {exit_code, ...})
   ↓
6. Tool returns formatted output (truncated via klynt-truncation if oversized)
   ↓
7. coding-ingest emits AgentEvent::V1 { kind: ToolCall { ... } }
```

---

## Internals

### The Distiller is failure-isolated

`distill_turn()` is spawned via `tokio::spawn` from `accept_event()`. Failures are logged as warnings but never propagate back to the ingestion path. The Distiller can fail entirely and the user-facing coding session continues unaffected.

### `hook_cli::run()` reuses one runtime per invocation

The hook binary creates a fresh `tokio::runtime::Builder::new_current_thread()` per call. Single-threaded, current-thread runtime — minimum overhead. The Tokio handle is dropped at function exit. Designed for sub-10ms wall time.

### `MacOsSeatbeltRunner` profile substitutions

The embedded `.sbpl` template has 3 substitution points: `{{CWD}}`, `{{EXTRA_WRITES}}`, `{{NETWORK}}`. See [`10-sandboxing-security.md`](./10-sandboxing-security.md) for the template body.

### `klynt-execpolicy` session rules in `RwLock<Vec<...>>`

Session-only rules (from "Allow always" approvals in this session) are stored separately in a `RwLock<Vec<(Vec<String>, Decision)>>` and take precedence over compiled `.rules` files. They're inserted via `append_session_allow_prefix` (called by the `chat_save_starlark_rule` Tauri command) and dropped at session end.

### `klynt-skill-loader::DynamicWalker` only walks up to `cwd_boundary`

The walker doesn't traverse the entire filesystem — it stops at the repo root (`cwd_boundary`). New `.klyntbot/skills/` directories appearing under the repo root are discovered automatically; ones outside aren't.

### `coding-ingest` cross-CLI normalization invariant

The proptest at `crates/coding-ingest/tests/cross_cli_normalization.rs` runs 64 cases asserting `parse(serialize(AgentEvent)) == AgentEvent` for all 5 `AgentSource` × 9 base `EventKind` combinations. Labeled **Inv 7**. If you change `AgentEvent::V1` shape, this test fails.

### kimi-cli / opencode are "hook-registered but poll-only"

The hook CLI's USAGE string lists kimi-cli and opencode as valid sources. If you invoke `klyntbot-hook kimi-cli < event.json`, the binary prints `"kimi-cli is poll-only (Phase 7)"` and exits 0. Data collection happens entirely through `OpencodePoller` (and similar for kimi) — background tokio tasks polling the wire files / SQLite DBs.

---

## Dependencies & extension points

### Upstream deps (selected)

- `klynt-protocol` ← codex-rs protocol types (adapted)
- `tree-sitter` (Rust/TS/JS/Python/Go grammars) — symbol extraction
- `globset`, `lru` — skill discovery
- `git2` (vendored libgit2) — git ops
- `rmcp` (transitively via coding-memory's MCP tools)
- `qwen3-asr`, `qwen3-tts-rs` — (in voice-engine, but consumed by coding-memory's interactive features)

### Adding a coding tool

1. Implement `#[derive(Tool)]` + `#[derive(ToolParams)]` in `crates/klynt-core/src/tools/`.
2. Declare `allowed_channels = "coding_only"` (or `all` for read tools).
3. Add to the appropriate group in `ToolKitBuilder::register_*` method.
4. If mutating: integrate `SnapshotRepo::try_record_with_ghost` before mutation.
5. If shell-executing: integrate `klynt-execpolicy::Policy::check` + `klynt-sandbox` spawn.

### Adding a new ingest adapter

1. Create `crates/coding-ingest/src/adapters/<my_cli>/` with `mod.rs`, optionally `mapper.rs`, `poller.rs`.
2. Implement `IngestAdapter` trait (`parse` only).
3. Add to `AgentSource` enum.
4. If hook-driven: add to `hook_cli::run()` source-name dispatch.
5. If poll-only: add a daemon (see `OpencodePoller`) and short-circuit in `hook_cli::run()`.
6. Update the cross-CLI normalization proptest with your `AgentSource` variant.

### Adding a hook event

⚠️ Cross-cutting change. `klynt-protocol::HookEventName` + `klynt-hooks::engine::dispatcher::HookFireInput` + all `*Tool` integration points need updating. Coordinate with the codex-rs upstream if you want to keep things adapted-from-upstream cleanly.

### Adding a new `Decision` variant to `klynt-execpolicy`

⚠️ Cross-cutting. `Evaluation::from_matches` uses `max()` on the variants, so the variant's ordinal matters for precedence. Adding a new variant in the wrong position changes semantics for all existing rules.

---

## Open questions & debt

- **`lsp-client` is fully stubbed.** All `TODO(T5)`. Either implement or remove.
- **4 Reforge phases in `coding-memory`** return `NotImplementedInPhase { required_phase: 5 }`. Wire them up or document a release plan.
- **`HookExecutionMode::InProcess` is typed but dead.** Pick: implement in-process hooks, or remove the variant.
- **Hook fail-open is hardcoded.** `Hook.fail_open` field exists but is ignored. Decide: respect the field, or remove it.
- **kimi-cli / opencode adapters listed in hook USAGE but short-circuit.** Confusing for anyone reading `--help`. Either remove from USAGE or implement the hook path.
- **`SkillSource` priority "higher number wins"** is the opposite of file-precedence convention. Document loudly or invert (and convert all numeric literals).
- **Ghost commit silent-fail** when `.git/` is removed between snapshot + rewind. Consider surfacing as an error or marking the snapshot row "unresolvable."
- **No physical DELETE through `ReforgeWriter`** is by design (bi-temporal). But the `coding_snapshots` table can grow unboundedly. Add a compaction job.
- **The Distiller default model is hardcoded** (`claude-haiku-4-5-20251001`). Should be `config.coding.distiller_provider` (or similar).
- **`klynt-truncation`'s `formatted_truncate_text` is the only truncation utility in the workspace** — used everywhere truncation happens. Should be in `common` for visibility.

See [`TECH_DEBT.md`](../TECH_DEBT.md) categories #1 (TODOs), #2 (stubs), #8 (naming) for specifics.

---

## Cross-references

- [`04-agent-runtime.md`](./04-agent-runtime.md) — `AgentRuntime` runs the coding session loop
- [`05-cognitive-memory.md`](./05-cognitive-memory.md) — coding-memory plugs into cognitive's Reforge cycle as `CodingPhaseRunner`
- [`07-tools-framework.md`](./07-tools-framework.md) — `JobSupervisorHandle` trait; `Tool` trait; `RoutingContext`
- [`10-sandboxing-security.md`](./10-sandboxing-security.md) — execpolicy + sandbox runners + approval gate are companion subsystem
- [`11-channels-mcp.md`](./11-channels-mcp.md) — 8 coding-memory MCP tools exposed via `EXPLICIT_TOOL_ALLOWLIST`
- [`crates/coding-memory.md`](../crates/coding-memory.md) — *(planned)* method-level reference
- [`crates/coding-ingest.md`](../crates/coding-ingest.md) — *(planned)* method-level reference
