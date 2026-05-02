import { resolveLeaf } from "./classify";
import type { DispatchResult } from "./types";

/** Transform an agent-routed slash command into a system-instruction-prefixed user message. */
export function transformAgentRouted(input: string): DispatchResult {
  const leaf = resolveLeaf(input);
  if (!leaf || leaf.path !== "agent") {
    return { kind: "error", message: `unknown agent-routed command: ${input}` };
  }

  if (leaf.agentTransform) {
    return { kind: "passthrough", text: leaf.agentTransform() };
  }

  // Fallback for legacy commands that don't declare agentTransform
  const trimmed = input.trim();
  if (trimmed.startsWith("/plan")) {
    return { kind: "passthrough", text: "[system: enter plan mode] " };
  }
  if (trimmed.startsWith("/yolo")) {
    return { kind: "passthrough", text: "[system: enter bypass mode] " };
  }
  if (trimmed.startsWith("/power ")) {
    const enable = trimmed.slice("/power ".length).trim() === "on";
    return { kind: "passthrough", text: `[system: power_mode=${enable}] ` };
  }
  if (trimmed.startsWith("/recall ")) {
    const query = trimmed.slice("/recall ".length);
    return {
      kind: "passthrough",
      text: `[system: force recall query="${query.replace(/"/g, '\\"')}"] `,
    };
  }

  return { kind: "error", message: `unknown agent-routed command: ${input}` };
}
