# Kimi CLI Poll-Only Ingest — Design

**Date:** 2026-04-29
**Author:** brainstorming session
**Status:** Draft — pending review

## Problem

The Plugins → Coding Memory → Kimi tab is empty ("No sessions in the last 14 days") even though the user has active kimi-cli sessions on disk. The Kimi adapter scaffolding exists but no transport delivers events to it at runtime:

1. **No auto-install at startup.** Only Claude Code is auto-installed (`crates/app-core/src/init/mod.rs:1307-1326`). Kimi requires a manual UI toggle, and the user's `~/.kimi/config.toml` confirms `hooks = []` — never installed.
2. **Tier-2 Wire is hardcoded off.** `kimi_wire_socket: None` at `crates/app-core/src/init/mod.rs:1083`.
3. **The Wire transport assumption is wrong.** The existing adapter (`crates/coding-ingest/src/adapters/kimi_cli/wire.rs`) expects a global Unix socket at `~/.kimi/wire.sock`, but kimi-cli writes per-session JSONL files at `~/.kimi/sessions/<work_dir_hash>/<session_uuid>/wire.jsonl`. No socket exists on disk.
4. **The frame schema in `wire.rs` doesn't match what kimi writes.** Adapter expects `{t, session_id, payload}`. Kimi writes `{type:"metadata",protocol_version:"1.9"}` then `{timestamp, message:{type, payload}}`.

Goal: make Kimi events first-class peers of Claude Code/Codex/opencode in the Coding Memory pipeline (ingest → Distiller → semantic facts → Reforge → cross-CLI recall), feeding the existing Sessions and Reforge tabs.

## Decision

Build a Rust poll-only adapter modelled on `CodexPoller`, gated by `coding_memory.cli.kimi_cli.enabled`. Delete the never-functional Tier-2 wire-socket adapter and the unused `KimiInstaller`. This follows the recent precedent in `8b4e2789 feat(coding-memory): switch codex to poll-only ingest, drop hook installs`.

## Architecture

A `KimiPoller` mirroring `CodexPoller`, wired into `IngestDaemonConfig` via a new `kimi_sessions_dir` field. Same daemon, same `event_tx`, same `IngestEventLogRepo` — Kimi events flow through Distiller → semantic facts → Reforge automatically once they land in `ingest_event_log` with `source = "kimi-cli"`.

## Components

### New files

- `crates/coding-ingest/src/adapters/kimi_cli/poller.rs` — `KimiPoller` (sessions dir, 1s tick, in-memory per-file byte offsets seeded to EOF on startup). Walks `<sessions_dir>/<work_dir_hash>/<session_uuid>/wire.jsonl` plus `subagents/<id>/wire.jsonl` recursively.
- `crates/coding-ingest/src/adapters/kimi_cli/wire_file.rs` — Rust port of `kimi_cli.wire.file::parse_wire_file_line` from the kimi-cli repo (Apache-2.0; attribute in module header). Parses both the `{type:"metadata",protocol_version}` first-line header and the `{timestamp, message:{type, payload}}` body shape. Includes a `collect_events` helper that recursively unwraps `SubagentEvent`.
- `crates/coding-ingest/src/adapters/kimi_cli/workdir.rs` — reads `~/.kimi/kimi.json`, builds `<md5_hash> → cwd` map. Refreshable on cache miss.
- `crates/coding-ingest/tests/kimi_poller.rs` — fixture-based integration test.

### Modified files

- `crates/coding-ingest/src/adapters/kimi_cli/mod.rs` — drop the `wire`, `dispatch`, `payload` modules and `spawn_wire`; drop `KimiAdapter` (the hook-payload parser). Re-export `KimiPoller` only.
- `crates/coding-ingest/src/daemon.rs` — replace `kimi_wire_socket` field with `kimi_sessions_dir: Option<PathBuf>` and `kimi_poll_interval: Option<Duration>`. Spawn `KimiPoller` instead of `spawn_wire`.
- `crates/coding-ingest/src/lib.rs` — drop `KimiAdapter` re-export, add `KimiPoller`.
- `crates/app-core/src/init/mod.rs` — set `kimi_sessions_dir = Some(home.join(".kimi/sessions"))` when `cfg.coding_memory.cli.kimi_cli.enabled`. Remove the `kimi_wire_socket: None` line.
- `crates/app-core/src/coding_memory/mod.rs` — drop the `"kimi-cli"` arms in `set_cli_enabled` and `coding_memory_diagnose_cli`. The toggle still flips `cfg.coding_memory.cli.kimi_cli.enabled` and the daemon picks it up on hot-reload, identical to Codex.

### Deleted files

- `crates/app-core/src/coding_memory/kimi_installer.rs`
- `crates/app-core/tests/kimi_installer.rs`
- `crates/coding-ingest/src/adapters/kimi_cli/wire.rs`
- `crates/coding-ingest/src/adapters/kimi_cli/dispatch.rs`
- `crates/coding-ingest/src/adapters/kimi_cli/payload.rs`
- `crates/coding-ingest/tests/kimi_wire_tier2.rs`
- `crates/coding-ingest/tests/kimi_adapter_tier1.rs`

## Data flow

1. **Startup**
   - Load hash→cwd map from `~/.kimi/kimi.json` (`work_dirs[].path`, hashed via md5; entries with `kaos != "local"` use prefix `<kaos>_<hash>`).
   - Walk `~/.kimi/sessions/`. Seed per-file byte offsets to current file size (no backfill).
2. **Tick** (default 1 s)
   - For each known and newly-discovered `wire.jsonl`, read from the last offset to EOF, accepting only fully-newline-terminated lines.
   - For each line:
     - Skip metadata header.
     - Parse envelope `{timestamp, message:{type, payload}}`.
     - Recursively unwrap `SubagentEvent`, capturing the inner-event chain along with the originating `<subagent_id>` (the `subagents/<id>` directory name).
     - Resolve `cwd` from the hash cache. On miss, re-read `kimi.json` once; if still unknown, fall back to `"/"`.
     - Build `AgentEventV1 { id: Uuid::new_v4(), source: AgentSource::KimiCli, session_id: <uuid dir>, turn_id: subagent_id_or_none, cwd, repo: resolve_scope(&cwd), occurred_at: Timestamp::from_second(timestamp.trunc() as i64), kind }`.
     - `repo.insert(&event)` then forward on `event_tx`.

## Message-type mapping (initial)

| Kimi `message.type`                | → `EventKind`                                                                |
|------------------------------------|------------------------------------------------------------------------------|
| (first line: `metadata`)           | once per `session_id`, emit `SessionStart { model, source_reason:"kimi-cli" }`. `model` read from `<session_dir>/state.json`; missing → `None`. |
| `TurnBegin` (`payload.user_input`) | `UserPrompt { text, attachments: [] }`. `user_input` may be a string or a list of `{type:"text", text}` blocks — both are handled. |
| `AssistantText` / `AssistantMessage` | `AssistantMsg { text, truncated:false, token_usage }`. `token_usage` populated when present in payload. |
| `ToolCallBegin`                    | buffered per `call_id` in a per-session map (no event emitted yet)           |
| `ToolCallEnd`                      | join with the buffered Begin and emit one `EventKind::ToolCall { tool, args_preview, ok, duration_ms, result_preview }`. `EventKind::ToolCall` already carries both invocation and result — it is emitted once per call, not paired. |
| `SubagentEvent`                    | recurse via `collect_events`; tag inner event with `turn_id = <subagent_id>` |
| Other (`StepBegin`, `StepEnd`, `TurnEnd`, …) | warn-once per type, skip (parity with Codex)                       |

Exact wire field names are pinned during implementation by reading the kimi-cli wire schema source under `src/kimi_cli/wire/`. The table above is the contract; if a field shape diverges, the implementation plan adjusts the mapping but not the policy.

## Error handling

- Missing `~/.kimi/kimi.json` → `cwd = "/"`; info-log once at startup.
- Unknown `<work_dir_hash>` → re-read `kimi.json`; still unknown → `cwd = "/"`.
- Partial trailing line (file mid-write) → break out of read loop; retry next tick.
- Bad JSON line → `warn!`, advance offset past the line, continue.
- Disk read errors on a file → `warn!`, skip that file this tick.

## Testing

- **Unit** (in `wire_file.rs` and `workdir.rs`): happy + error paths for `parse_line`, `SubagentEvent` unwrap recursion, metadata-header skip, hash→cwd resolution from a fixture `kimi.json` (covering `kaos = "local"` and a non-local kaos prefix).
- **Integration** (`crates/coding-ingest/tests/kimi_poller.rs`): write a fixture session dir under tempdir with a metadata line + `TurnBegin` + `AssistantText` + a `SubagentEvent` wrapping an inner `ToolCallBegin`. Spawn the poller against an in-memory `StoragePool`. Assert events arrive on the `mpsc::UnboundedReceiver` and `ingest_event_log` rows are present with `source = "kimi-cli"`.
- **Cross-CLI normalization** (`crates/coding-ingest/tests/cross_cli_normalization.rs`): the existing proptest already covers `AgentSource::KimiCli` roundtrip — must keep passing after the legacy adapter delete.
- **Regression**: removing the `"kimi-cli"` arms in `set_cli_enabled` and `coding_memory_diagnose_cli` must not break the other CLI arms. Existing tests in those areas continue to cover claude-code/codex/opencode.

## Frontend impact

None. The existing Kimi tab in `desktop-ui/src/features/plugins/coding-memory/` filters by `source = "kimiCli"` (translated to `kimi-cli` on the backend in `provider_id_to_db_slug`). Once rows land in `ingest_event_log` with that source slug, the existing `session_list` query surfaces them. No new bindings.

## Out of scope

- Realtime mirror-panel latency tuning. 1 s polling parity with Codex is acceptable.
- "Open in Kimi Vis" deep-link button. Parked for a later iteration if demand emerges.
- Backfill of pre-existing sessions. Matches Codex precedent (seed offsets to current EOF).
- Hooks-based ingest (the rejected Approach 3). The Codex commit `8b4e2789` removed hooks for the same reason.

## Open questions for implementation

- Confirm the exact wire field name for assistant text. The kimi-cli source has both `AssistantText` and `AssistantMessage` historically — read `src/kimi_cli/wire/` to pick the canonical one before writing the mapping table in code.
- Confirm whether `ToolCallEnd` carries the tool name plus args or only an id back-reference. The implementation will keep a per-session `<call_id> → (tool, args_preview, started_at)` map either way, since `EventKind::ToolCall` requires both invocation and result fields at emit time.
- Confirm token-usage location (per-message vs per-turn). If it's emitted on `TurnEnd`, we attach it to the previous `AssistantMsg` rather than dropping the `TurnEnd` silently.
