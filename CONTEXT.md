# KlyntBot

Personal AI agent desktop app (Tauri 2 + Rust workspace): chat, tools, notes, tasks, and local persistence under `~/.klyntbot/`.

## Language

**Exposure policy**:
Cohesive per-tool declaration of three independent axes: (1) LLM channel visibility — which chat-channel categories receive the tool definition (default: all, preserving today’s `ChannelMask::ALL`); (2) subagent projection — whether the tool is copied from the parent registry into a subagent (default: false); (3) MCP exposure — `Default` / `OptIn` / `Forbidden` with locked override validation. Lives on `Tool` via derive metadata; the live `ToolRegistry` is the in-process catalog. Does **not** include approval, concurrency, timeout, or discovery metadata.
_Avoid_: registration path, wiring path, separate exposure catalog, AiFeatureRegistry-as-tool-list, flat enum of surface combos

**Surface**:
A consumer of the tool set that must decide inclusion: agent loop, subagent projection, or MCP exposure. Surfaces may intentionally differ; differences must be explicit in the exposure policy or in a validated user override.
_Avoid_: channel (overloaded with ChannelMask / messaging), layer, boundary

**Projection**:
A derived or validated view of the tool set for one surface, produced from the exposure policy (plus explicit overrides). Not a second place to invent membership.
_Avoid_: copy of the registry, re-registration


**Mcp exposure**:
Per-tool MCP membership: `Default` (auto-fill when no user override), `OptIn` (user may list explicitly after per-tool review), or `Forbidden` (never exposable). Unreviewed tools default to `Forbidden` — `OptIn` is never a migration catch-all. Overrides: registered+Default/OptIn valid; Forbidden or absent invalid → refuse MCP subsystem start (embedded) or non-zero exit (stdio-only); do not kill the whole desktop app.
_Avoid_: allowlist membership, exposed_tools as source of truth, OptIn-as-default


**Embedded MCP status**:
Persistent desktop indication of the in-process KlyntBot MCP server (ready / disabled / invalid), distinct from external MCP servers KlyntBot connects to as a client. Invalid config blocks only this subsystem; rejection summary must be reachable from the status, not only via toast or logs.
_Avoid_: conflating with external MCP server connection status


**MCP-server builtin**:
A closed, server-owned MCP capability outside Tool exposure policy (`get_status` mandatory; `agent` configurable with default-on). Must not be used to register domain tools or duplicate registry-tool policy. New builtins need a separate architecture decision.
_Avoid_: registry tool, allowlist entry, exposure-policy Default

## Relationships

- An **exposure policy** lives on each `Tool` and drives **projections** onto **surfaces**.
- The live **ToolRegistry** is the authoritative in-process catalog for **registry-backed** tools; MCP/agent/subagent projections read policy from registered tools.
- **MCP-server builtins** (`get_status` mandatory; `agent` configurable) are a closed server-owned capability set — not Tool exposure policy and not a place to register domain tools.
- Tool **construction** stays where dependencies/lifecycle need it; construction sites must not invent a second policy home.
- **AiFeatureRegistry** is recall-domain metadata only — not a proxy for general tool exposure.

## Flagged ambiguities

- Ops docs still say "coding mode" / four tool-wiring paths; code uses `SessionMode::{Assistant, Subagent}` and imperative plugin + builder registration. Treat docs as drift until rewritten.
- Exact Rust names / derive syntax for exposure policy are design decisions; the MCP tri-state semantics above are locked.
