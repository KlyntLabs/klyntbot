import { commands } from "@/bindings";
import type { SessionListArgs } from "@/features/plugins/coding-memory/types";

export async function listCodingSessions(args: SessionListArgs) {
  const r = await commands.codingMemorySessionList(args);
  if (r.status !== "ok") throw new Error(r.error.message ?? "session list failed");
  return r.data;
}

export async function fetchSessionWire(
  sessionId: string,
  limit = 500,
  offset = 0,
) {
  const r = await commands.codingMemorySessionReplayTyped(sessionId, limit, offset);
  if (r.status !== "ok") throw new Error(r.error.message ?? "replay failed");
  return r.data;
}

export async function fetchCliHealth() {
  const r = await commands.codingMemoryCliHealth();
  if (r.status !== "ok") throw new Error(r.error.message ?? "health failed");
  return r.data;
}
