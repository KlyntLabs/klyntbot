# ADR-0103: Entity-update intent is a dedicated app-core projection

Context: Chat and MCP each invented tool→kind and mutate heuristics for `entity:updated`, and MCP consulted AiFeatureRegistry plus a fallback table — risking a second exposure-like catalog.

Decision: Successful chat `ToolEnd` and MCP bridge-executed tools share a pure `app-core` entity-update intent module (tool name + action → kind-level `"*"` intents via per-tool read-only denylists). AiFeatureRegistry, Tool exposure policy, session `tool_domain`, params/results, and desktop `useMutation` stay out of that module.

Why: One testable emit classification without turning recall metadata or Tool policy into UI-refresh membership.
