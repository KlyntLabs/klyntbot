# Tasks: Tool exposure policy

Feature code: EXPO
Status: In-progress
Date: 2026-09-02
Execution-mode: continuous

**Goal:** Cut MCP (and agent/subagent) tool membership over to a cohesive `Tool` exposure policy with a shared `mcp` validator, builtins, diagnostics, and persistent embedded MCP status.

**Architecture:** `ExposurePolicy` on `Tool` (`tools-core`); pure `mcp::server::exposure` validation; `AiPipelinePlugin::post_init` reads `app.agent.tool_registry()`; `klyntbot-server` serves the validated set; desktop shows embedded status. See `design.md`.

**Tech Stack:** Rust workspace (`tools-core`, `tools-core-macros`, `mcp`, `app-core`, `agent`, `klyntbot-server`, `desktop`), React/`desktop-ui`, Vitest/cargo nextest as in CLAUDE.md.

## Global Constraints

- `CLAUDE.md` (sha256: ca0cf12ff9936f076dcf3885828a11f45135f500166ea9d84323720192a7365c) — bun for UI; `cargo nextest` / `cargo clippy --workspace`; path aliases; no raw hex outside tokens; thin Tauri commands.
- `docs/standards/design-tokens.md` (sha256: 261fb67b8e85beb064f8816798e15dc9bff7057a4e4b53ba11ee9a82b507d747) — DS tokens for embedded status UI.
- ADRs `docs/adr/0100-exposure-policy-on-tool.md`, `0101-mcp-server-builtins.md`, `0102-mcp-forbidden-default.md`.
- No `app-core` → `klyntbot-server` dependency. Prefer crate `mcp` over `app_core::handlers::settings::mcp` module name collision.

## File structure

| Path | Responsibility |
|---|---|
| `crates/tools-core/src/exposure.rs` (create) | `ExposurePolicy`, `McpExposure` |
| `crates/tools-core/src/lib.rs` | `Tool::exposure_policy`; thin accessors |
| `crates/tools-core-macros/src/tool_derive.rs` | Derive attrs for policy axes |
| Per-tool sites (historical Defaults + stubs) | Annotate MCP Default / Forbidden |
| `crates/mcp/src/server/exposure.rs` (create) | Validator + `BuiltinId` |
| `crates/mcp/src/server/mod.rs` | Export exposure module |
| `crates/app-core/Cargo.toml` | Direct dep on crate `mcp` |
| `crates/app-core/src/plugins/ai_pipeline.rs` | post_init cutover |
| `crates/app-core/src/state.rs` | Store `ExposureValidation` |
| `crates/config/src/schema/mcp.rs` | Retire runtime `EXPLICIT_TOOL_ALLOWLIST` use |
| `crates/agent/src/agent_loop/mod.rs` | Channel filter via policy |
| `crates/agent/src/subagent.rs` | Subagent filter via policy |
| `crates/klyntbot-server/src/lib.rs` | Diagnostic uses validator |
| `crates/klyntbot-server/src/handler.rs` | Advertise validated set + builtins |
| `crates/desktop/src/main.rs` | Gate embedded start on status |
| `crates/desktop/src/commands/` + shared DTO | Expose embedded status |
| `desktop-ui/.../McpServersSettings.tsx` | Embedded status section |
| `tests/ai_mcp_default_tools_from_registry.rs` | Rewrite as migration fixture |

## Tasks

### Task 1: ExposurePolicy on Tool + derive

**Files:**
- Create: `crates/tools-core/src/exposure.rs`
- Modify: `crates/tools-core/src/lib.rs`, `crates/tools-core-macros/src/tool_derive.rs`, `crates/tools-core-macros/src/tool_actions.rs` (if `#[tool_actions]` tools need `mcp_exposure`), plus Default tool sites (`tasks`, `notes`, `learning`, `productivity`, `language_practice`, `memory`, `annotate`, `cron`, `alarm`, `mirror`, `temporal`, `launcher` — hand-rolled `impl Tool` or attr as each site allows)
- Test: `crates/tools-core` / macros unit tests

**Reuse:** rung 2 — extend `Tool` + `#[tool]` derive (`allowed_channels` pattern)

**Interfaces:**
- Consumes: `ChannelMask`
- Produces: `ExposurePolicy`, `McpExposure`, `Tool::exposure_policy()`; accessors delegate to policy

**Depends-on:** none

**Steps:**
- [ ] Failing tests: defaults ALL / subagent false / mcp Forbidden; derive overrides; accessors match policy
- [ ] Implement types + trait + derive/`tool_actions` support; annotate historical Defaults (channel literals remain `all`|`desktop_only` only)
- [ ] Tests pass; commit `feat(tools-core): add ExposurePolicy on Tool`

_Requirements: EXPO-1.1, EXPO-1.2, EXPO-1.3, EXPO-1.4, EXPO-1.5, EXPO-1.6, EXPO-1.7, EXPO-2.3, EXPO-2.4, EXPO-8.1, EXPO-8.2_

### Task 2: mcp::server::exposure validator

**Files:**
- Create: `crates/mcp/src/server/exposure.rs`
- Modify: `crates/mcp/src/server/mod.rs`
- Test: unit tests beside module

**Reuse:** rung 7 — new module in `mcp` (depends on `tools-core`)

**Interfaces:**
- Consumes: `&ToolRegistry` (callers hold `tokio::sync::RwLock` read guard then pass `&*reg`), `server_enabled`, override list, `ExposurePolicy` via tools
- Produces: `BuiltinId`, `ExposureValidation`, `validate_mcp_exposure`

**Depends-on:** Task 1

**Steps:**
- [ ] Failing tests: empty override → Defaults only; Forbidden/unknown reject; `get_status` always; `agent` default-on / overrideable; Disabled when server off
- [ ] Implement pure validator + typed builtins
- [ ] Pass; commit `feat(mcp): add server exposure validator`

_Requirements: EXPO-3.1, EXPO-3.2, EXPO-3.3, EXPO-3.4, EXPO-3.5, EXPO-3.6, EXPO-3.7, EXPO-3.10, EXPO-3.11, EXPO-4.2, EXPO-7.1_

### Task 3: post_init cutover + migration fixture

**Files:**
- Modify: `crates/app-core/Cargo.toml`, `crates/app-core/src/plugins/ai_pipeline.rs`, `crates/app-core/src/state.rs`, `crates/config/src/schema/mcp.rs`
- Modify/Test: `tests/ai_mcp_default_tools_from_registry.rs` (migration evidence)

**Reuse:** rung 2 — extend `AiPipelinePlugin::post_init`

**Interfaces:**
- Consumes: `validate_mcp_exposure`, `app.agent.tool_registry()`
- Produces: AppCore-stored `ExposureValidation`; in-memory auto-fill of `exposed_tools` when Ready+empty override

**Depends-on:** Task 2

**Steps:**
- [x] Failing migration test: fixture historical set → registry Defaults = union − agent − eight stubs; advertised = builtins + Defaults; no silent add of Forbidden tools
- [x] Wire post_init; delete AiFeature∪allowlist runtime fill; store status on AppCore
- [x] Confirm client MCP config serde and AiFeature recall registration still work; recall stub shadow test still passes
- [x] Pass; commit `feat(app-core): cut over MCP defaults to ToolRegistry policy`

_Requirements: EXPO-2.1, EXPO-2.2, EXPO-2.5, EXPO-2.6, EXPO-2.7, EXPO-3.8, EXPO-7.2, EXPO-8.3, EXPO-8.4, EXPO-8.5, EXPO-8.12_

### Task 4: Agent and subagent read policy

**Files:**
- Modify: `crates/agent/src/agent_loop/mod.rs`, `crates/agent/src/subagent.rs`
- Test: existing/adjacent unit tests for channel + subagent filters

**Reuse:** rung 2 — extend channel/subagent filters

**Interfaces:**
- Consumes: `Tool::exposure_policy()` / thin accessors
- Produces: unchanged external filter behavior

**Depends-on:** Task 1

**Steps:**
- [ ] Failing tests if needed: channel allow + memory subagent true; default false
- [ ] Switch filters to policy accessors
- [ ] Pass; commit `refactor(agent): read exposure policy for projections`

_Requirements: EXPO-6.1, EXPO-6.2, EXPO-6.3, EXPO-6.4, EXPO-8.6, EXPO-8.7_

### Task 5: Server serve + diagnostic

**Files:**
- Modify: `crates/klyntbot-server/src/handler.rs`, `crates/klyntbot-server/src/lib.rs`, `crates/desktop/src/main.rs` (stdio/tools-list entry as needed)
- Test: integration/CLI smoke for diagnostic exit codes

**Reuse:** rung 2 — extend handler + `print_exposed_tools`

**Interfaces:**
- Consumes: `ExposureValidation` / `BuiltinId`
- Produces: advertised MCP list = effective registry + builtins; diagnostic print + non-zero on Invalid

**Depends-on:** Task 2, Task 3

**Steps:**
- [ ] Failing test/diagnostic expectation: no AiFeature∪allowlist reconstruction; Invalid → non-zero
- [ ] Wire handler + diagnostic to validator; keep schemas with handlers
- [ ] Pass; commit `feat(klyntbot-server): serve validated MCP exposure`

_Requirements: EXPO-3.2, EXPO-3.3, EXPO-3.7, EXPO-3.9, EXPO-4.1, EXPO-4.2, EXPO-4.3, EXPO-4.4, EXPO-8.8, EXPO-8.9_

### Task 6: Embedded MCP status UI

**Files:**
- Modify: `crates/desktop/src/main.rs`, desktop command(s) + shared DTO, `desktop-ui/src/features/settings/pages/McpServersSettings.tsx`
- Test: component test for ready/disabled/invalid + rejection list

**Reuse:** rung 2 — extend MCP settings page + thin commands

**Interfaces:**
- Consumes: AppCore `ExposureValidation`
- Produces: persistent embedded status section distinct from client servers

**Depends-on:** Task 3, Task 5

**Steps:**
- [ ] Failing UI/command test: Invalid shows reasons; Ready/Disabled chips; client list still works
- [ ] Gate embedded HTTP start on status; map DTO; add settings section
- [ ] Pass; commit `feat(desktop-ui): show embedded MCP exposure status`

_Requirements: EXPO-5.1, EXPO-5.2, EXPO-5.3, EXPO-5.4, EXPO-5.5, EXPO-5.6, EXPO-7.3, EXPO-8.10, EXPO-8.11, EXPO-8.13_

## Coverage notes

EXPO-8.3 / EXPO-8.5 / EXPO-8.12 cited on Task 3: keep last-write-wins shadow behavior, leave client MCP config and AiFeature recall registration unchanged (regression checks in that task’s steps).
