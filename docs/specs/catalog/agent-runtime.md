# agent-runtime catalog

Recognized feature cards for this domain. Add a row **before** writing
`requirements.md` for a new feature. Status is one of
`Draft | Approved | In-progress | Implemented | Shipped | Recognized`.

| Code | Feature | Spec | Status | Roadmap item | Match terms | Surface roots |
|---|---|---|---|---|---|---|
| EXPO | Tool exposure policy | ../2026-09-02-tool-exposure-policy/ | Implemented | — | exposure policy, MCP Default OptIn Forbidden, ToolRegistry projection, MCP-server builtin, embedded MCP status | `crates/tools-core/`, `crates/mcp/`, `crates/app-core/`, `crates/klyntbot-server/`, `crates/agent/`, `desktop-ui/` |
| EUPI | Entity-update intent projection | ../2026-09-02-entity-update-intent/ | Approved | — | entity-update intent, entity:updated, ToolEnd, MCP bridge emit, session_tool_domain, per-tool read-only | `crates/app-core/`, `crates/klyntbot-server/` |
