# Design: Tool exposure policy

Feature code: EXPO
Status: Approved
Date: 2026-09-02
Requirements: ./requirements.md

## Context

Today MCP default membership is invented outside the live tool catalog: `AiPipelinePlugin::post_init` fills empty `exposed_tools` from `AiFeatureRegistry::tool_names() ∪ EXPLICIT_TOOL_ALLOWLIST`, while agent/subagent visibility use separate `Tool` methods (`allowed_channels`, `subagent_visible`). Diagnostics reprint that union without booting the agent registry. Synthetic MCP names `get_status` and `agent` are handler-owned, not `ToolRegistry` entries.

The binding constraint is **declaration/policy locality**: one cohesive exposure policy on each `Tool`, projections from the final agent `ToolRegistry`, and a Forbidden wall for unreviewed MCP exposure — without a parallel exposure catalog and without `app-core → klyntbot-server`. That rules out keeping AiFeature∪allowlist as runtime authority, and rules out registering domain tools as “builtins.”

Fresh retrieval: `docs/specs/catalog/agent-runtime.md` contains only **EXPO** — no neighbor CODEs to reuse (`owns_coverage`: 1/1 for this shard). Scan digest: `.skills/EXPO/scan.md`. No `docs/architecture/` spine.

## Decisions

1. Cohesive exposure policy on `Tool` / derive (ADR-0100); live agent `ToolRegistry` is catalog for registry-backed tools.
2. MCP `Default` / `OptIn` / `Forbidden`; unreviewed tools Forbidden (ADR-0102); OptIn only via per-tool review.
3. Single cutover in `AiPipelinePlugin::post_init` reading `app.agent.tool_registry()`; delete old union in the same releasable change.
4. Closed MCP-server builtins outside Tool policy (ADR-0101): `get_status` mandatory; `agent` configurable default-on; typed identities in `mcp`.
5. Shared pure validator/projection in `mcp::server::exposure`; `app-core` depends on `mcp`; `klyntbot-server` keeps handlers/schemas and serves validated set; diagnostics boot final registry + same validator.
6. Persistent embedded-MCP status in desktop (distinct from client servers); invalid config fails MCP subsystem only.
7. Classification: historical union → Default (registry-backed); eight EXPO-2.3 stubs → Forbidden; `agent` is builtin not Default Tool; all else Forbidden.
8. Exact Rust field/derive names below are design choices; product semantics stay as requirements.

## Architecture

### ExposurePolicy on Tool (`tools-core`)

Satisfies: EXPO-1.1, EXPO-1.2, EXPO-1.3, EXPO-1.4, EXPO-1.5, EXPO-1.6, EXPO-1.7, EXPO-2.3, EXPO-2.4, EXPO-6.3, EXPO-7.1, EXPO-8.1, EXPO-8.2
Reuse: rung 2 — extend `tools-core` `Tool` trait + `tools-core-macros` `#[derive(Tool)]` / `#[tool(...)]` attrs (today already carry `allowed_channels`)
Surface:
- `Tool::allowed_channels` / `Tool::subagent_visible` call sites (`agent` channel filter, `subagent` projection, tests) — **replace** (thin accessors over policy; no second authority)
- `#[tool(allowed_channels = ...)]` authors — **replace** (map into policy field; keep literal values `all` | `desktop_only`)
- Hand-rolled `Tool` impls overriding channel/subagent — **replace** (return policy axes)
Interface: `ExposurePolicy { llm_channels: ChannelMask, subagent: bool, mcp: McpExposure }` with `McpExposure::{Default, OptIn, Forbidden}`; `Tool::exposure_policy(&self) -> ExposurePolicy`; derive attrs e.g. `mcp_exposure = "default"|"opt_in"|"forbidden"`, `subagent = true`, existing channel attr; defaults ALL / false / Forbidden (except annotated Defaults for historical union tools).
Depth: n/a — extends `Tool` / derive
Locality: change lands in `tools-core` + macros + per-tool annotations; neighbor `agent`/`subagent` **extend** to call `exposure_policy()` (or accessors); `ToolMetadata` **leave**

`McpExposure` default for unspecified tools is **Forbidden** (ADR-0102). Historical **registry-backed Default** names (annotate in code/attrs): `tasks`, `productivity`, `notes`, `learning`, `language_practice`, `memory`, `annotate`, `cron`, `alarm`, `mirror`, `temporal`, `launcher`. The eight EXPO-2.3 stub names stay Forbidden (named removals). Builtin `agent` is not a Tool Default.

### MCP exposure projection (`mcp::server::exposure`)

Satisfies: EXPO-3.1, EXPO-3.2, EXPO-3.3, EXPO-3.4, EXPO-3.5, EXPO-3.6, EXPO-3.7, EXPO-3.10, EXPO-3.11, EXPO-4.2, EXPO-7.1
Reuse: rung 7 — new module inside existing `mcp` crate (already depends on `tools-core`; no `app-core`)
Surface:
- Future callers (`app-core` post_init, `klyntbot-server` diagnostic/stdio, tests) — new readers only (omit Surface inventory of pre-existing readers)
Interface: `validate_mcp_exposure(input: ExposureInput) -> ExposureValidation` where `ExposureInput` carries `registry: &ToolRegistry`, `server_enabled: bool`, `override_tools: Option<&[String]>` (None/empty = auto-default), and builtin rules are read from the typed `BuiltinId` table inside the module. `ExposureValidation` carries `runtime_state` (`Ready` when enabled+valid, `Disabled` when `server_enabled` is false, `Invalid` when enabled+rejected entries), `requested`, `effective_registry_tools`, `effective_builtins`, `rejected: [{name, reason: Unknown|Forbidden}]`. Callers never pass AiFeature records or allowlist slices.
Depth: Callers must still know their `ToolRegistry` handle, whether the embedded server is enabled, and the user’s override list; they must not know Default/OptIn/Forbidden projection rules, builtin mandatory/default-on tables, or rejection classification.
Locality: new code under `crates/mcp/src/server/exposure.rs` (+ `mod` export); `mcp::server` **extend**; `app-core` **extend** (new **crate** dep on `mcp` — distinct from `app_core::handlers::settings::mcp` module)

Does not execute tools or own HTTP/stdio servers.

### Cutover in AppCore (`AiPipelinePlugin::post_init`)

Satisfies: EXPO-2.1, EXPO-2.2, EXPO-2.6, EXPO-3.8, EXPO-8.4, EXPO-7.2
Reuse: rung 2 — extend `AiPipelinePlugin::post_init` and AppCore-held status handle
Surface:
- `config.mcp.server.exposed_tools` auto-fill path — **replace** (policy-derived defaults; delete AiFeature∪allowlist write)
- `EXPLICIT_TOOL_ALLOWLIST` runtime consumers — **replace** (delete from fill/diagnostic reconstruction; may remain only inside migration test evidence fixture)
- `AiFeatureRegistry::tool_names` MCP fill — **replace** (stop using for exposure)
- In-memory `exposed_tools_auto_filled` flag — **compat** until settings/diagnostics stop relying on it; follow-up: derive “auto vs override” from empty-override detection only
Interface: post_init reads `app.agent.tool_registry()` (not host tool clone); calls `mcp::server::exposure::validate_mcp_exposure` with `server_enabled` + override; **always** stores the full `ExposureValidation` on AppCore (embedded MCP status source for UI/desktop). When override is empty and state is Ready, also writes `effective_registry_tools` into in-memory `config.mcp.server.exposed_tools` and sets `exposed_tools_auto_filled` so existing whitelist snapshots keep working. When Invalid, leaves override untouched, stores Invalid status, and does not rewrite defaults. Desktop `main` consults AppCore status before binding embedded HTTP (does not start server when Invalid/Disabled); never `process::exit` on the Tauri app path.
Depth: n/a — extends plugin post_init
Locality: `crates/app-core/src/plugins/ai_pipeline.rs` + AppCore status field; add crate `mcp` to `app-core` Cargo.toml; desktop start path **extend**; **leave** AiFeature recall registration

### klyntbot-server (handlers + diagnostics)

Satisfies: EXPO-3.2, EXPO-3.3, EXPO-3.7, EXPO-3.9, EXPO-4.1, EXPO-4.2, EXPO-4.3, EXPO-4.4, EXPO-8.8, EXPO-8.9
Reuse: rung 2 — extend `KlyntbotServerHandler` / `print_exposed_tools` / stdio entry
Surface:
- `print_exposed_tools` AiFeature∪allowlist reconstruction — **replace**
- Handler synthetic `get_status` / `agent` advertising — **replace** (driven by `BuiltinId` + validation result; schemas stay colocated with handlers)
- Whitelist ∩ registry listing — **replace** (serve validated effective set)
Interface: handler receives `ExposureValidation` (or effective sets); lists registry tools from effective set; always includes `GetStatus`; includes `Agent` when selected; diagnostic boots AppCore (or equivalent final registry), runs shared validator, prints structured fields, exits non-zero on Invalid.
Depth: n/a — extends server crate
Locality: `crates/klyntbot-server/`; uses `mcp` typed builtins; **leave** AgentBridge execution semantics

### Agent / subagent projection readers

Satisfies: EXPO-6.1, EXPO-6.2, EXPO-6.3, EXPO-6.4, EXPO-8.6, EXPO-8.7
Reuse: rung 2 — extend existing filters in `agent_loop` and `subagent`
Surface:
- Channel filter on `get_definitions` — **replace** (read policy / accessor)
- `subagent_visible` filter — **replace** (read policy / accessor)
Interface: unchanged external behavior; internal reads `exposure_policy().llm_channels` / `.subagent`.
Depth: n/a — extends agent
Locality: `crates/agent/src/agent_loop/mod.rs`, `subagent.rs`; **leave** ToolKitBuilder cwd rebuild rules

### Desktop embedded MCP status

Satisfies: EXPO-5.1, EXPO-5.2, EXPO-5.3, EXPO-5.4, EXPO-5.5, EXPO-5.6, EXPO-7.3, EXPO-8.10, EXPO-8.11, EXPO-8.13
Reuse: rung 2 — extend MCP settings route + thin Tauri command/DTO from AppCore status
Surface:
- `McpServersSettings` (client servers) — **extend** (add separated embedded-server section; do not reuse client connection badges as embedded status)
- Config settings MCP handlers (client list) — **leave** for client servers; **extend** with embedded status read API
- Desktop MCP start path — **extend** (honor Invalid → do not bind server; set status)
Interface: DTO e.g. `{ state: ready|disabled|invalid, requested, effective, rejected: [{name, reason}] }`; persistent UI section “Embedded MCP server” with state chip + expandable rejection list; external servers remain a separate list.
Depth: n/a — extends settings UI + commands
Locality: `desktop-ui/.../McpServersSettings.tsx` (+ small presentational bits); `crates/desktop/src/commands/` thin adapter; AppCore status **extend**

## UI design

Grounding: user locks (persistent status, distinct from client servers, rejection summary reachable, no dedicated panel) → `docs/standards/design-tokens.md` + `@klyntbot/design-system` tokens (`text-fg`, `text-muted`, `bg-glass`, `border-*`, semantic good/warn/danger) → choices below.

### Embedded MCP status (settings)

Layout: On the existing MCP settings page, a top section “Embedded MCP server” above the external-servers list; vertical stack with gap from space tokens; status row then optional details.
Components: rung 2 — extend `McpServersSettings`; reuse existing settings section chrome / badges if present; new (rung 7) only a small status chip + rejection list if no existing status pattern fits.
States:
- ready — success/neutral chip “Ready”; effective tool count summary
- disabled — muted chip “Disabled” (config off)
- invalid — danger/warn chip “Invalid”; rejection list visible or one click away (not toast-only)
- loading — optional skeleton while status fetch pending
- focus — visible focus ring on interactive summary control
Type & color: `text-ui` / `text-body` for labels; state via semantic tokens (success/warn/danger), never color alone; mono for tool names (`text-ui-sm` + mono stack).
A11y: section heading; chip text includes state word; rejection list keyboard reachable; focus visible; do not rely on color-only differentiation (EXPO-7.3).

## Seams for testing

| Seam | Kind | Covers |
|---|---|---|
| `ExposurePolicy` defaults + derive expansion | unit | EXPO-1.1–1.5, EXPO-1.7, EXPO-2.3–2.4, EXPO-8.1–8.2 |
| `mcp::server::exposure::validate_mcp_exposure` | unit (new primary seam) | EXPO-3.1–3.7, EXPO-3.10, EXPO-4.2, EXPO-7.1 |
| Canonical migration fixture (historical set → expected defaults/advertised) | unit/integration | EXPO-2.5, EXPO-2.7, EXPO-2.3 removals |
| `AiPipelinePlugin::post_init` with fake/final registry | integration | EXPO-2.1–2.2, EXPO-2.6, EXPO-3.8, EXPO-8.4, EXPO-7.2 |
| Diagnostic command / print path | integration | EXPO-4.1–4.4, EXPO-3.9 (stdio) |
| Agent channel filter + subagent projection | unit | EXPO-6.1–6.4, EXPO-8.6–8.7 |
| Embedded status DTO → settings UI | component / e2e light | EXPO-5.1–5.6, EXPO-7.3, EXPO-8.11 |

## Coverage check

| ID | Satisfies section |
|---|---|
| EXPO-1.1–1.7 | ExposurePolicy on Tool |
| EXPO-2.1–2.2, 2.6 | Cutover in AppCore |
| EXPO-2.3–2.5, 2.7 | ExposurePolicy + migration seam |
| EXPO-3.1–3.7, 3.10–3.11 | mcp::server::exposure |
| EXPO-3.8 | Cutover in AppCore |
| EXPO-3.9 | klyntbot-server |
| EXPO-4.* | klyntbot-server (+ validator unit) |
| EXPO-5.* | Desktop embedded MCP status |
| EXPO-6.* | Agent / subagent projection |
| EXPO-7.1 | ExposurePolicy + validator |
| EXPO-7.2 | Cutover |
| EXPO-7.3 | Desktop UI |
| EXPO-8.1–8.2 | ExposurePolicy |
| EXPO-8.3 | deliberately unmapped as Satisfies of a module that only **guards** last-write-wins — covered by registry **leave** + guard criterion; no structural change — listed under ExposurePolicy locality as leave registry semantics |
| EXPO-8.4 | Cutover |
| EXPO-8.5 | deliberately unmapped to a change module — Out of Scope / guard only (client MCP config leave) |
| EXPO-8.6–8.7 | Agent / subagent |
| EXPO-8.8–8.9 | klyntbot-server |
| EXPO-8.10–8.11, 8.13 | Desktop |
| EXPO-8.12 | deliberately unmapped — AiFeature recall **leave** (guard only) |

EXPO-8.3, EXPO-8.5, EXPO-8.12 are guard-only (no new module Satisfies); they remain binding acceptance criteria verified by existing or regression tests.
