# ADR-0102: Unreviewed tools are MCP Forbidden

Context: Migrating off the historical allowlist could accidentally OptIn every registered tool (including external MCP client tools re-exported through the server).

Decision: Historical union names (minus named stub removals and minus builtin `agent`) are Default; coding-memory stub names listed in EXPO-2.3 are Forbidden; all other registered tools are Forbidden until an explicit per-tool review sets OptIn.

Why: Prevents silent MCP surface expansion while still allowing intentional OptIn later.
