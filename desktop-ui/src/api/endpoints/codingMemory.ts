import { commands } from "@/bindings";
import type { SessionListArgs } from "@/features/plugins/coding-memory/types";

export async function listCodingSessions(args: SessionListArgs) {
  const r = await commands.codingMemorySessionList(args);
  if (r.status !== "ok") throw new Error(r.error.message ?? "session list failed");
  return r.data;
}

export async function fetchSessionWire(sessionId: string, limit = 500, offset = 0) {
  const r = await commands.codingMemorySessionReplayTyped(sessionId, limit, offset);
  if (r.status !== "ok") throw new Error(r.error.message ?? "replay failed");
  return r.data;
}

export async function fetchCliHealth() {
  const r = await commands.codingMemoryCliHealth();
  if (r.status !== "ok") throw new Error(r.error.message ?? "health failed");
  return r.data;
}

export async function fetchRecallOverlay(sessionId: string) {
  const r = await commands.codingMemorySessionReplayRecallOverlay({
    sessionId,
    limit: 200,
    offset: 0,
  });
  if (r.status !== "ok") throw new Error(String(r.error));
  return r.data;
}

export async function fetchMirrorAlerts(args: { repo?: string; severity?: string }) {
  const r = await commands.codingMemoryMirrorAlertsFeed({
    kind: null,
    severity: args.severity ?? null,
    repo: args.repo ?? null,
    limit: 50,
  });
  if (r.status !== "ok") throw new Error(String(r.error));
  return r.data;
}

export async function actMirrorAlert(id: string, action: "approve" | "reject" | "snooze") {
  const r = await commands.codingMemoryMirrorAlertAction({ id, action });
  if (r.status !== "ok") throw new Error(String(r.error));
  return r.data;
}

export async function fetchRecallFacts(ids: string[]) {
  const r = await commands.codingMemoryRecallFetch({
    ids,
    includeProvenance: true,
    includeCausalGraph: true,
  });
  if (r.status !== "ok") throw new Error(String(r.error));
  return r.data;
}

export async function listReforgeCycles() {
  const r = await commands.codingMemoryReforgeCycleList();
  if (r.status !== "ok") throw new Error(String(r.error));
  return r.data;
}

export async function fetchReforgeCycleDiff(args: {
  repoId: string;
  artifact: string;
  beforeCycleId?: string;
  afterCycleId?: string;
}) {
  const r = await commands.codingMemoryReforgeCycleDiff({
    repoId: args.repoId,
    artifact: args.artifact,
    beforeCycleId: args.beforeCycleId ?? null,
    afterCycleId: args.afterCycleId ?? null,
  });
  if (r.status !== "ok") throw new Error(String(r.error));
  return r.data;
}

export async function fetchEffectivenessTrends(patternId: string) {
  const r = await commands.codingMemoryEffectivenessTrends(patternId);
  if (r.status !== "ok") throw new Error(String(r.error));
  return r.data;
}
