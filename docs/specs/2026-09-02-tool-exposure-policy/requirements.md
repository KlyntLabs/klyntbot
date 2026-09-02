# Requirements: Tool exposure policy

Feature code: EXPO
Status: In-progress
Date: 2026-09-02

## 1. Declare cohesive exposure policy on a tool

**Story:** As a tool author, I want to declare LLM-channel, subagent, and MCP membership in one exposure-policy value on the tool, so that I do not edit separate registration paths to define who may see the tool.

- **EXPO-1.1** THE SYSTEM SHALL expose a cohesive exposure-policy value on each `Tool` with exactly three independent axes: LLM channel visibility, subagent projection, and MCP exposure.
- **EXPO-1.2** THE SYSTEM SHALL default LLM channel visibility to all chat-channel categories (preserving today’s `ChannelMask::ALL` behavior) when the author does not override it.
- **EXPO-1.3** THE SYSTEM SHALL default subagent projection to false when the author does not override it.
- **EXPO-1.4** THE SYSTEM SHALL represent MCP exposure as one of `Default`, `OptIn`, or `Forbidden` on that cohesive value.
- **EXPO-1.5** THE SYSTEM SHALL NOT place approval class, concurrency safety, timeout, discovery metadata, category, tags, or cost inside the exposure-policy value.
- **EXPO-1.6** WHEN a tool is registered in the live `ToolRegistry` THE SYSTEM SHALL treat that registry as the authoritative in-process catalog for registry-backed tools’ exposure policy.
- **EXPO-1.7** WHILE the single releasable cutover is in progress THE SYSTEM SHALL allow `allowed_channels()` and `subagent_visible()` only as thin accessors over the cohesive exposure policy, not as independent authorities.

## 2. Cut over MCP defaults to registry-backed policy

**Story:** As a maintainer, I want MCP default tool membership to come from registered tools’ MCP exposure policy at final-registry time, so that AiFeature and string allowlists stop owning exposure.

- **EXPO-2.1** WHEN `AiPipelinePlugin::post_init` runs with an empty user `exposed_tools` override THE SYSTEM SHALL derive the registry-backed MCP default set from tools present in `app.agent.tool_registry()` whose MCP exposure is `Default`.
- **EXPO-2.2** WHEN that post_init cutover ships THE SYSTEM SHALL remove runtime use of `AiFeatureRegistry::tool_names() ∪ EXPLICIT_TOOL_ALLOWLIST` for MCP default membership in the same releasable change.
- **EXPO-2.3** THE SYSTEM SHALL classify MCP exposure for existing tools as: historical MCP union names remain `Default`; these eight coding-memory stub tool names are `Forbidden` and recorded as named intentional removals: `recall_index`, `recall_timeline`, `recall_fetch`, `trace_causes`, `check_dead_ends`, `recall_facts_as_of`, `recall_change_history`, `recall_decision_points`; every other registered tool is `Forbidden` unless an explicit per-tool review sets `OptIn`.
- **EXPO-2.4** THE SYSTEM SHALL NOT use `OptIn` as a catch-all migration default for unreviewed tools.
- **EXPO-2.5** WHEN the canonical migration fixture constructs the historical MCP tool set THE SYSTEM SHALL make the policy-derived registry-backed default set equal the previous union minus `agent` and the eight named coding-memory stub removals listed in EXPO-2.3 — verified by an automated migration test.
- **EXPO-2.6** AT runtime, WHEN no user override is set THE SYSTEM SHALL make the effective registry-backed default set equal the tools actually present in the live registry whose MCP policy is `Default`.
- **EXPO-2.7** THE SYSTEM SHALL make the complete advertised MCP tool set equal mandatory builtins plus selected configurable builtins plus the registry-backed defaults, matching the pre-cutover advertised set minus the eight named coding-memory stub removals listed in EXPO-2.3 — verified by the migration test (and runtime assertion where applicable).

## 3. Validate overrides and MCP-server builtins

**Story:** As an operator, I want invalid MCP exposure configuration rejected with explicit reasons while valid overrides and builtins behave predictably, so that typos and Forbidden tools cannot silently shape the MCP surface.

- **EXPO-3.1** THE SYSTEM SHALL treat MCP-server builtins as a closed, server-owned capability set outside `Tool` exposure policy, and SHALL NOT use that set to register domain tools or duplicate registry-tool policy.
- **EXPO-3.2** THE SYSTEM SHALL always advertise the mandatory builtin `get_status`, unaffected by tool overrides.
- **EXPO-3.3** THE SYSTEM SHALL advertise the configurable builtin `agent` by default, and SHALL allow an explicit override to include or exclude `agent`.
- **EXPO-3.4** WHEN validating an override name THE SYSTEM SHALL accept: a live-registry tool with MCP `Default` or `OptIn`; or the configurable builtin `agent`.
- **EXPO-3.5** IF an override names a live-registry tool with MCP `Forbidden` THEN THE SYSTEM SHALL reject it as invalid with reason `forbidden`.
- **EXPO-3.6** IF an override names a tool absent from both the live registry and the closed builtin set THEN THE SYSTEM SHALL reject it as invalid with reason `unknown`.
- **EXPO-3.7** THE SYSTEM SHALL share one server-owned typed source for builtin identities and default/mandatory semantics between validation and handlers (no duplicated string list as a second authority).
- **EXPO-3.8** WHEN embedded MCP configuration is invalid THE SYSTEM SHALL refuse to start only the embedded MCP subsystem, keep the desktop application usable, and expose every invalid entry with its tool name and reason.
- **EXPO-3.9** WHEN stdio MCP mode encounters invalid configuration THE SYSTEM SHALL exit unsuccessfully because MCP is that process’s sole purpose.
- **EXPO-3.10** THE SYSTEM SHALL place pure registry-backed projection and override validation in the `mcp` crate (e.g. `mcp::server::exposure`), accepting live `ToolRegistry`, user override, and policy data, returning one structured validation/projection result, without depending on `app-core` or `agent`.
- **EXPO-3.11** THE SYSTEM SHALL NOT introduce an `app-core` → `klyntbot-server` dependency for this feature.

## 4. Diagnose MCP exposure with the shared validator

**Story:** As a maintainer, I want a first-class diagnostic command that prints requested, effective, and rejected exposure using the live registry and the same validator, so that I can prove MCP membership without reconstructing the old allowlist.

- **EXPO-4.1** WHEN the MCP exposure diagnostic command runs THE SYSTEM SHALL boot enough to obtain the final live `ToolRegistry` and invoke the same shared validator used at runtime.
- **EXPO-4.2** WHEN that diagnostic runs THE SYSTEM SHALL print configured/enabled state, runtime state (`ready`, `disabled`, or `invalid`), requested override, effective exposed-tool set, and rejected entries with tool name and reason (`unknown` or `forbidden`).
- **EXPO-4.3** IF the validated configuration is invalid THEN THE SYSTEM SHALL make the diagnostic command exit non-zero.
- **EXPO-4.4** THE SYSTEM SHALL remove the diagnostic path that independently reconstructs `AiFeatureRegistry::tool_names() ∪ EXPLICIT_TOOL_ALLOWLIST`.

## 5. See embedded MCP status in the desktop

**Story:** As a desktop user, I want persistent embedded-MCP status with access to rejection reasons, so that I know whether the in-process MCP server is ready without reading logs.

- **EXPO-5.1** THE SYSTEM SHALL persistently show the embedded MCP server state as ready, disabled, or invalid in the desktop UI.
- **EXPO-5.2** WHEN embedded MCP is invalid THE SYSTEM SHALL provide access to the rejection summary (tool name and reason) from that persistent status surface.
- **EXPO-5.3** THE SYSTEM SHALL distinguish the KlyntBot embedded MCP server status from status of external MCP servers connected by KlyntBot as a client.
- **EXPO-5.4** THE SYSTEM SHALL NOT treat a transient toast or log line alone as satisfying the embedded status requirement.
- **EXPO-5.5** THE SYSTEM SHALL NOT require a dedicated diagnostics panel or tool-classification editor for this feature.
- **EXPO-5.6** WHERE backend and UI land on a non-releasable stack THE SYSTEM SHALL still require the persistent embedded status UI before the change is releasable.

## 6. Project agent and subagent tool sets from exposure policy

**Story:** As a chat user, I want agent and subagent tool visibility to follow the cohesive exposure policy, so that channel and subagent membership stay consistent with the declared contract.

- **EXPO-6.1** WHEN the agent builds the LLM-visible tool definition list THE SYSTEM SHALL include a tool only if the exposure policy’s LLM channel visibility allows the current chat channel.
- **EXPO-6.2** WHEN a subagent toolkit is projected from the parent registry THE SYSTEM SHALL include a parent tool only if the exposure policy’s subagent projection axis is enabled.
- **EXPO-6.3** THE SYSTEM SHALL CONTINUE TO treat `memory` as the production tool that opts into subagent projection unless an explicit later reclassification changes it.
- **EXPO-6.4** THE SYSTEM SHALL CONTINUE TO keep cwd-scoped kit primitives out of parent-registry subagent projection by default (subagent projection default false).

## 7. Quality attributes

**Section-kind:** nfr

**Story:** As a stakeholder, I want measurable quality targets for this feature, so that how-well is not left implicit.

- **Performance:** None — no standing product metrics doc; this change is init-time validation and projection, not a measured interactive latency surface.
- **Security:** **EXPO-7.1** THE SYSTEM SHALL ensure unreviewed registry tools remain MCP `Forbidden` and that Forbidden or unknown override names cannot enter the effective MCP exposed set — verified by unit tests of the shared validator and the migration test’s no-silent-expansion assertions. (Consult: `docs/security/threat-model.md` Absent — no greppable TB/THR IDs invented.)
- **Reliability:** **EXPO-7.2** WHEN embedded MCP configuration is invalid THE SYSTEM SHALL keep the desktop application process running while only the embedded MCP subsystem fails to start — verified by an automated test or scripted desktop init assertion. (Consult: `docs/ops/reliability.md` Absent — target from frame-change lock.)
- **Accessibility:** **EXPO-7.3** WHEN the embedded MCP status is shown THE SYSTEM SHALL expose state and rejection summary through keyboard-reachable UI (not color alone) — verified by manual keyboard check on the status surface. (Consult: `docs/standards/accessibility.md` Absent — no WCAG ID invented.)

## 8. Existing behavior guards

Touched surfaces and guards:

- `crates/tools-core/src/lib.rs` — **EXPO-8.1** (guard) WHEN tools declare channel or subagent visibility THE SYSTEM SHALL CONTINUE TO default channels to all and subagent projection to false except preserved opt-ins.
- `crates/tools-core-macros/src/tool_derive.rs` — **EXPO-8.2** (guard) WHEN existing `#[tool]` attrs (name, description, params, approval, concurrency, channels) are used THE SYSTEM SHALL CONTINUE TO compile and generate those behaviors.
- `crates/tools-core/src/registry.rs` — **EXPO-8.3** (guard) WHEN tools are registered by name THE SYSTEM SHALL CONTINUE TO last-write-wins on duplicate names (including recall stub shadowing).
- `crates/app-core/src/plugins/ai_pipeline.rs` — **EXPO-8.4** (guard) WHEN the user has a non-empty `exposed_tools` override THE SYSTEM SHALL CONTINUE TO treat that list as the requested override input (subject to new validation rules in §3).
- `crates/config/src/schema/mcp.rs` — **EXPO-8.5** (guard) WHEN MCP client server config (`servers`, transports, OAuth) is loaded THE SYSTEM SHALL CONTINUE TO deserialize and apply those client settings unchanged by this feature.
- `crates/agent/src/subagent.rs` — **EXPO-8.6** (guard) WHEN a subagent is spawned THE SYSTEM SHALL CONTINUE TO project only opted-in parent tools plus cwd-scoped kit tools built for that subagent.
- `crates/agent/src/agent_loop/mod.rs` — **EXPO-8.7** (guard) WHEN filtering tool definitions for a channel THE SYSTEM SHALL CONTINUE TO omit tools whose channel visibility does not allow that channel.
- `crates/klyntbot-server/src/handler.rs` — **EXPO-8.8** (guard) WHEN listing MCP tools THE SYSTEM SHALL CONTINUE TO advertise `get_status` as a server capability.
- `crates/klyntbot-server/src/bridge/` — **EXPO-8.9** (guard) WHEN executing a registry-backed MCP tool name THE SYSTEM SHALL CONTINUE TO dispatch through the live agent `ToolRegistry` bridge.
- `crates/desktop/src/main.rs` — **EXPO-8.10** (guard) WHEN embedded MCP is disabled in config THE SYSTEM SHALL CONTINUE TO leave the desktop app usable without starting the embedded server.
- `desktop-ui/src/features/settings/pages/McpServersSettings.tsx` — **EXPO-8.11** (guard) WHEN managing external MCP client servers THE SYSTEM SHALL CONTINUE TO allow enable/disable and server configuration for those client connections.
- `crates/ai-core/src/registry.rs` — **EXPO-8.12** (guard) WHEN recall-domain features register `FeatureRecord`s THE SYSTEM SHALL CONTINUE TO support domain/skill/entity recall registration unrelated to MCP tool exposure.
- `crates/mcp/src/client/` — no behavior to guard for this feature’s MCP **server** exposure cutover (client allowlists remain separate).
- `crates/app-core/src/handlers/settings/mcp.rs` — **EXPO-8.13** (guard) WHEN settings APIs expose MCP client server lists THE SYSTEM SHALL CONTINUE TO serve client-server configuration independently of embedded-server status.

## Out of Scope

- Redesigning tool execution, approval gates, RoutingContext / FullCtx projection, concurrency safety, timeouts, or discovery metadata.
- Forcing all tool construction into one module.
- Requiring identical tool sets across agent, subagent, and MCP surfaces.
- A separately maintained exposure-policy catalog or using `AiFeatureRegistry` as a general tool-exposure proxy.
- Expanding MCP `Default`/`OptIn` for unreviewed tools; per-tool OptIn security reviews beyond the locked Forbidden wall (including treating the eight EXPO-2.3 stub names as Default).
- Killing the whole desktop application on invalid embedded MCP config.
- Dedicated diagnostics panel or classification editor UI.
- Auditing/rewriting approval enforcement via `ToolRegistryBridge` (separate security change).
- Adding new MCP-server builtins beyond `get_status` and `agent` without a separate architecture decision.
- Fixing CLAUDE.md / coding-mode docs drift except where this feature’s comments must not reintroduce AiFeature∪allowlist as MCP authority.
- Introducing `app-core` → `klyntbot-server` dependency.

## Open Questions

- Exact `ExposurePolicy` / MCP-state / validation-result / rejection / builtin type names and derive-annotation syntax — owner: design-solution — date: 2026-09-02 — forbid-guess: do not invent product semantics beyond locks; naming is design-only.
- Internal DTO/event representation and desktop component placement for embedded MCP status — owner: design-solution — date: 2026-09-02 — forbid-guess: must satisfy EXPO-5.*; layout is design-only.
- Whether diagnostics group builtins vs registry-backed tools visually — owner: implementer after design — date: 2026-09-02 — forbid-guess: optional weigh, not a product fork.
