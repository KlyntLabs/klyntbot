import { invoke } from "@/api/client";
import { resolveLeaf } from "./classify";
import type { DispatchResult } from "./types";

export async function dispatchDirect(input: string, sessionKey: string): Promise<DispatchResult> {
  const leaf = resolveLeaf(input);
  if (!leaf || !leaf.tauriCommand) {
    return { kind: "error", message: `unknown direct command: ${input}` };
  }
  const args = input.slice(1).trim().split(/\s+/).slice(leaf.command.split(" ").length);
  try {
    const params = buildParams(leaf.command, args, sessionKey);
    const result = await invoke(leaf.tauriCommand, params);
    return { kind: "render", itemKind: "system", item: { command: leaf.command, result } };
  } catch (e) {
    return { kind: "error", message: String(e) };
  }
}

function buildParams(cmd: string, args: string[], sessionKey: string): Record<string, unknown> {
  switch (cmd) {
    case "skills info":
    case "skills update":
    case "skills uninstall":
    case "skills validate":
      return { name: args[0] ?? "" };
    case "skills install":
      return { source: args[0] ?? "" };
    case "skills toggle":
      return { name: args[0] ?? "", enabled: !args.includes("--off") };
    case "resume":
      return { prefix: args[0] ?? "" };
    case "sessions star":
    case "sessions unstar":
      return { sessionKey };
    case "sessions rewind":
      return { args: { sessionKey, messageId: args[0] ?? "" } };
    case "sessions export":
      return { args: { sessionKey, format: args[0] ?? "md" } };
    case "sessions fork":
      return { args: { sessionKey, upToMessage: args[0] ?? null } };
    case "permissions clear-mirror":
      return { args: { tool: args[0] ?? "", repoId: null } };
    default:
      return {};
  }
}
