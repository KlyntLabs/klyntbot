# Phase 2 Verification — Agent 6 (Sandboxing, Channels, MCP, Activity)

> **Docs verified:** `10-sandboxing-security.md`, `11-channels-mcp.md`  
> **Crates verified:** `approval`, `klynt-sandbox`, `klynt-sandbox-helper`, `klynt-process-hardening`, `channels`, `notifications`, `mcp`, `mcp-bridge`, `activity-log`  
> **Date:** 2026-05-16

---

## Summary

| Metric | Count |
|---|---|
| Crates inspected | 9 |
| ✅ Accurate claims | ~45 |
| ⚠️ Drift (slightly off) | 5 |
| ❌ Wrong (factually incorrect) | 2 |
| 🔍 Missing (in code, not in docs) | 8 |
| 📋 Tech debt catalogued | 3 |

Overall the two subsystem docs are **well-maintained** and reflect the source code accurately. The most significant issues are: (1) the `approval` file map names two files that do not exist, (2) `klynt-process-hardening` is falsely claimed to have no tests, and (3) several `activity-log` type fields have incorrect types or are omitted in the docs.

---

## Per-Crate Findings

### `approval`

#### ✅ Accurate
- `src/lib.rs` re-exports match the documented public API.
- `ApprovalGate::new`, `with_classify_hooks`, `with_suggester`, `check()` all exist with correct signatures.
- `GateOutcome` enum variants (`Allow`, `Deny { reason }`, `Cancel`) match.
- `ClassifyHook` trait exists with `classify()` and default `scope()`.
- `CodingApprovalPolicy` has three variants (`Default`, `PlanMode`, `YoloMode`).
- `BlockingFallbackChannel::desktop_prompt()` returns the exact message documented.
- `approval_grants` schema matches exactly (columns, CHECK constraints, unique key, `INSERT OR IGNORE`).
- `coding_approval_history` schema matches exactly.
- `check()` future races against `cancel_token` via `tokio::select!`.
- Remote-channel auto-allow logic (`!caps.supports_classes.contains(&req.class)`) is present.
- YoloMode expiry falls through to `matches!(DefaultPolicy::Ask, DefaultPolicy::Allow)` → `false` (ask everything).
- Tool-name normalization strips `_`/`-` and lowercases.

#### ❌ Wrong
- **File map claims `src/hook.rs` and `src/suggester.rs`.** Neither file exists.
  - `ClassifyHook` is defined in `src/policy.rs`.
  - `ApprovalSuggester` is defined in `src/gate.rs`.

#### 🔍 Missing
- `approval_pattern_history` table columns `id` and `occurred_at` are not mentioned in the doc (only `user_id, tool_name, path, decision, pattern_used` are listed).
- `ApprovalGate::set_suggester()` (post-construction setter) exists in code but is not documented.
- `ApprovalGate::purge_session()` exists and is public but not in the doc API block.

#### 📋 Tech Debt
- None in this crate (no `TODO`/`FIXME`/`unimplemented!`).

---

### `klynt-sandbox`

#### ✅ Accurate
- Module tree matches `src/lib.rs` (`runner`, `policy`, `error`, plus platform-gated `seatbelt`, `linux`, `bwrap`, `helper_proto`).
- `MacOsSeatbeltRunner::build_sandboxed_command()` exists and returns `tokio::process::Command`.
- Template substitutions (`{{CWD}}`, `{{EXTRA_WRITES}}`, `{{NETWORK}}`) are exactly the three documented.
- Seatbelt base policy is `(deny default)`.
- Timeout kills child and returns `SandboxError::ChildExit(124)`.
- stdout+stderr are merged into `CommandOutput::stdout`.
- `LinuxSandboxRunner` detects three modes (`WithBwrap`, `LandlockOnly`, `Unavailable`).
- bwrap flags: `--unshare-user --unshare-pid --die-with-parent --new-session`, plus `--unshare-net` for `Block`.
- System dirs are `--ro-bind`ed (`/usr`, `/lib`, `/lib64`, `/bin`, `/sbin`, `/etc`).
- `/proc`, `/dev` virtualized; `/tmp` fresh tmpfs.
- CWD bind mode varies by `FsConstraints` (`--bind` for `WriteCwdReadAll`, `--ro-bind` for `ReadCwdOnly`).
- Helper located by `<parent_exe_dir>/klynt-sandbox-helper` first, then `PATH`.
- Policy is JSON → base64 (`STANDARD_NO_PAD`) → single positional arg.
- 5 test files exist: `bwrap_args.rs`, `helper_locator.rs`, `linux_smoke.rs`, `policy_construct.rs`, `seatbelt_smoke.rs`.

#### ⚠️ Drift
- None significant.

#### 📋 Tech Debt
- None.

---

### `klynt-sandbox-helper`

#### ✅ Accurate
- `main.rs` sequence: parse CLI → `apply_no_new_privs()` → `apply_landlock()` → execvp.
- Exit codes: `2` (non-Linux), `124` (timeout), `125` (Landlock unavailable), `126` (setup failure).
- `EXIT_SANDBOX_UNAVAILABLE = 125` and `EXIT_SANDBOX_SETUP_FAILED = 126` constants match.
- Landlock-only mode exits 125 when `ruleset != FullyEnforced`.
- In `WithBwrap` mode, helper does **not** exit 125 for non-fully-enforced ruleset (doc correctly notes this is only in `LandlockOnly`).
- `VENDOR.md` exists and documents provenance from `codex-rs/linux-sandbox/` under Apache-2.0.
- Stale "Plan 1: stub" comment is present at `main.rs:1-4` (Plan 3 is active).

#### ⚠️ Drift
- Architecture diagram says "no_new_privs + Landlock + seccomp"; doc itself notes seccomp is **not applied** and treats the mention as aspirational. This is acknowledged in the doc, so it is not a hidden inaccuracy.

#### 📋 Tech Debt
- Stale `main.rs:1-4` comment (documented as debt item in the doc itself).

---

### `klynt-process-hardening`

#### ✅ Accurate
- `pre_main_hardening()` dispatches to platform-specific functions exactly as documented.
- macOS: `ptrace(PT_DENY_ATTACH)`, `RLIMIT_CORE=0`, scrubs `DYLD_*`, `MallocStackLogging`, `MallocLogFile`.
- Linux: `prctl(PR_SET_DUMPABLE, 0)`, `RLIMIT_CORE=0`, scrubs `LD_*`.
- BSD: `RLIMIT_CORE=0`, scrubs `LD_*`.
- Windows: empty function with `// TODO: Windows hardening ...`.
- Exit codes: `5` (Linux prctl), `6` (macOS ptrace), `7` (RLIMIT_CORE).
- Called at `desktop/src/main.rs:112`, before `configure_mimalloc()` at line 114.

#### ❌ Wrong
- **Doc claims "security-critical but no tests."** This is factually incorrect.
  - `src/lib.rs` lines 122–162 contain `#[cfg(all(test, unix))]` tests:
    - `env_keys_with_prefix_handles_non_utf8_entries`
    - `env_keys_with_prefix_filters_only_matching_keys`

#### 📋 Tech Debt
- `pre_main_hardening_windows()` is a stub (`lib.rs:95`) — acknowledged in doc.

---

### `channels`

#### ✅ Accurate
- `Channel` trait signature matches exactly (`name`, `start`, `stop`, `send`, `is_allowed`, `send_typing`, `supports_interaction`, `send_interaction`).
- `DynChannel = Arc<dyn Channel>` exists.
- 4 adapters exist: Telegram, Discord, Slack, Email (feature-gated).
- `TelegramApprovalChannel` exists in `src/adapters/telegram_approval.rs`.
- `ChannelManager::new(config, bus)` takes ownership of `bus.take_outbound_rx()`.
- `initialize_channels()` creates adapters from config.
- `start_all()` spawns per-channel tasks and creates per-channel `mpsc::channel(32)` queues.
- `stop_all()` calls `channel.stop()` on all.
- `check_allowlist()` handles compound IDs split on `|`.
- `reconnect_loop()` retries with 5s sleep.

#### 🔍 Missing
- `channels/src/shared.rs` exists but is not mentioned in the doc module tree (doc only lists `adapters`, `formatter`, `manager`, `utils`, `ws_manager`).

#### 📋 Tech Debt
- None in this crate.

---

### `notifications`

#### ✅ Accurate
- `NotificationDispatcher` subscribes to `DomainEvent::AlarmFired`.
- `kind == "held_release"` routes to `handle_held_release`; all other kinds route to `handle_alarm_fired`.
- `QuietHoursPolicy::new(cfg, iana_tz)`, `is_in_quiet_hours(at)`, `next_window_end(at)`, `override_for_urgent()`, `enabled()` all exist.
- `HeldReleaseService::hold()` generates `held_{uuid}` ID, inserts `held_notifications`, schedules `FireSpec { kind: "held_release", ... }`.
- `mark_released(held_id)` sets `released = 1`.
- `notification_log` primary key is `(alarm_id, channel)`.
- `held_notifications` partial index: `ON held_notifications(release_at_ms) WHERE released = 0`.
- Only `os_native` (bit 1) and `tray` (bit 2) are wired; Telegram (4), Discord (8), Email (16) are defined but not registered.
- `mask_to_names` and `names_to_mask` are inverse pairs.

#### 🔍 Missing
- `NotificationDispatcher` publishes `DomainEvent::HeldNotificationReleased` and `DomainEvent::NotificationDeliveryFailed` — not mentioned in workflow docs.

#### 📋 Tech Debt
- `notifications/src/channel/mod.rs:64`: `TODO(4.8 / follow-up)` about wiring Telegram/Discord/Email into the dispatcher registry.

---

### `mcp`

#### ✅ Accurate
- `McpTransport` enum has `Stdio { command, args, env }` and `Http { url, headers }` variants.
- `McpManager::connect_all()` exists and supports both transports.
- `SamplingDelegate` trait exists with `sample(&self, params: CreateMessageRequestParams)`.
- `McpApprovalChannel` is in `src/server/approval.rs` and **always declines**.
- `deny_to_mcp_error()` exists in `src/server/handler.rs` and parses the structured JSON.
- `McpChannelAllowlist` gates per channel as documented.
- `default_exposed_tools()` returns empty `Vec`.
- `EXPLICIT_TOOL_ALLOWLIST` in `crates/config/src/schema/mcp.rs` contains exactly the 16 tools listed.
- MCP tool namespacing convention `mcp_{sanitized_server}_{sanitized_tool}` is implemented in `client/sanitize.rs`.

#### ⚠️ Drift
- **MCP server-side `ToolRegistryBridge` and `AgentBridge` live in `crates/klyntbot-server/src/bridge/`, not in the `mcp` crate.** The doc says "`mcp` server side — `ToolRegistryBridge`" which is slightly misleading because the structs are in a separate crate (`klyntbot-server`). They are re-exported/consumed via the server handler, but the file paths are not `mcp/src/...`.
- **MCP circuit-breaker cooldown is 60 seconds, not 30 seconds.** The architecture diagram in `11-channels-mcp.md` says "cooldown 30s", but `crates/mcp/src/client/manager.rs:75` constructs `McpCircuitBreaker::new(3, 60)`.

#### 📋 Tech Debt
- `McpApprovalChannel` always declines — acknowledged as intentional limitation in both docs.

---

### `mcp-bridge`

#### ✅ Accurate
- `BridgeFrame { event: String, payload: serde_json::Value }` matches exactly.
- `MAX_FRAME_BYTES = 1 << 20` (1 MB) is correct.
- Encoding is 4-byte little-endian length prefix + JSON body.
- Clean EOF before length prefix → `Ok(None)`.
- Oversized frames → `Err(FrameError::TooLarge(...))`.
- Socket path: `${KLYNTBOT_HOME or ~/.klyntbot}/mcp-events.sock` resolved by `bridge_socket_path()` calling `config::loader::config_dir()`.
- `BridgeServer`, `BridgeClient`, `SocketBridgeEmitter` all exist with documented roles.
- `BridgeClient::send` is non-blocking (unbounded mpsc) and drops silently on failure.

#### 🔍 Missing
- `BridgeClient` has `CONNECT_TIMEOUT = 200ms` and `WRITE_TIMEOUT = 200ms` — not documented.

#### 📋 Tech Debt
- None.

---

### `activity-log`

#### ✅ Accurate
- 8 tables exactly as claimed: `unified_activity_log`, `work_contexts`, `work_resources`, `resource_edges`, `work_context_resources`, `work_context_actions`, `context_merges`, `inference_state`.
- 3 normalizers exist: `ChatMessageNormalizer`, `ToolCallNormalizer`, `WindowEventNormalizer`.
- `ChatMessageNormalizer` maps to `source: Chat`, `actor: User|AiAgent`, `action: "prompt"|"reply"`.
- `ToolCallNormalizer` maps to `source: ToolCall`, `actor: AiAgent`, `action: "run"`.
- `WindowEventNormalizer` maps to `source: OsWindow|Browser`, `actor: User`, `action: "view"|"browse"`; idle events return `None`.
- Content SHA-256 hashing and 500-char preview truncation are implemented.
- IDs are ULIDs (time-sortable).
- Inference loop runs on a schedule and its state is stored in `inference_state`.

#### ⚠️ Drift
- **`WorkContext` field types:**
  - Doc says `total_duration_secs: u64` and `event_count: u64`.
  - Code (`types.rs:137-138`) has `total_duration_secs: i64` and `event_count: i64`.
- **`WorkResource.resource_path`:**
  - Doc says `resource_path: String`.
  - Code (`types.rs:221`) has `resource_path: Option<String>`.
- **`ResourceEdge.edge_type`:**
  - Doc says `edge_type: ResourceEdgeType` (an enum with 4 variants).
  - Code (`types.rs:234`) has `edge_type: String`.

#### 🔍 Missing
- `WorkContext` doc omits fields: `description`, `color`, `tags`, `created_at`, `updated_at`.
- `WorkResource` doc omits fields: `first_seen_at`, `last_seen_at`, `embedding_id`.
- `ResourceEdge` doc omits fields: `first_seen_at`, `last_seen_at`.
- `ActivityLogEntry` type (the actual row type) is not mentioned in the doc at all.

#### 📋 Tech Debt
- None.

---

## Cross-Reference Check

All cross-reference links from both subsystem docs were verified against the filesystem.

| Link | Resolved? | Notes |
|---|---|---|
| `../00-overview.md` (from `subsystems/`) | ✅ | Exists at `docs/architecture/00-overview.md` |
| `./01-foundations.md` | ✅ | Exists |
| `./02-storage.md` | ✅ | Exists |
| `./06-scheduling.md` | ✅ | Exists |
| `./07-tools-framework.md` | ✅ | Exists |
| `./09-coding-mode.md` | ✅ | Exists |
| `./10-sandboxing-security.md` | ✅ | Exists |
| `./11-channels-mcp.md` | ✅ | Exists |
| `./13-desktop-frontend.md` | ✅ | Exists |
| `../TECH_DEBT.md` | ✅ | Exists |

---

## Detailed Issue Log

### Issue 1 — `approval` file map lists non-existent files
**Severity:** Medium  
**Location:** `10-sandboxing-security.md` → "`approval` — file map"  
**Claim:** `src/hook.rs` and `src/suggester.rs` exist.  
**Reality:** `ClassifyHook` is in `src/policy.rs`; `ApprovalSuggester` is in `src/gate.rs`.

### Issue 2 — `klynt-process-hardening` falsely claimed test-less
**Severity:** Medium  
**Location:** `10-sandboxing-security.md` → "`klynt-process-hardening`" + "Open questions & debt"  
**Claim:** "No test coverage" / "security-critical but no tests."  
**Reality:** `src/lib.rs` contains two unit tests for `env_keys_with_prefix`.

### Issue 3 — `activity-log` type field mismatches
**Severity:** Low  
**Location:** `11-channels-mcp.md` → "`activity-log` — types"  
**Claims:**
- `WorkContext.total_duration_secs: u64` → actual `i64`
- `WorkContext.event_count: u64` → actual `i64`
- `WorkResource.resource_path: String` → actual `Option<String>`
- `ResourceEdge.edge_type: ResourceEdgeType` → actual `String`

### Issue 4 — `McpCircuitBreaker` cooldown seconds mismatch
**Severity:** Low  
**Location:** `11-channels-mcp.md` architecture diagram  
**Claim:** "cooldown 30s"  
**Reality:** `McpCircuitBreaker::new(3, 60)` in `crates/mcp/src/client/manager.rs:75`.

### Issue 5 — MCP server bridges located in wrong crate
**Severity:** Low  
**Location:** `11-channels-mcp.md` → "`mcp` server side"  
**Claim:** `ToolRegistryBridge` and `AgentBridge` are described under the `mcp` crate.  
**Reality:** They live in `crates/klyntbot-server/src/bridge/registry.rs` and `agent.rs`. The `mcp` crate only contains `McpApprovalChannel` on the server side.
