# ADR-0101: Closed MCP-server builtins outside Tool policy

Context: `get_status` and `agent` are advertised by the MCP server handler but are not ToolRegistry entries (`agent` uses AgentBridge).

Decision: Treat them as a closed, server-owned builtin set (typed in `mcp`) outside Tool exposure policy; do not register domain tools through that set. New builtins need a separate architecture decision.

Why: Preserves historical MCP advertisements without pretending synthetics are Tools or recreating a second exposure catalog.
