# ADR-0100: Exposure policy lives on Tool

Context: MCP and agent surfaces previously invented membership via AiFeatureRegistry tool names and a string allowlist, separate from Tool registration.

Decision: Cohesive exposure policy (LLM channels, subagent projection, MCP Default/OptIn/Forbidden) is declared on each Tool; the live ToolRegistry is the catalog for registry-backed tools.

Why: Keeps one authoring site next to the implementation and makes projections derive from what actually exists in-process, rejecting a parallel exposure catalog.
