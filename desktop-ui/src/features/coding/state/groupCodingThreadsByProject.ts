import type { ChatThread } from "@/features/chat/types";
import type { WorkspaceInfo } from "@/types";

export interface ProjectThreadGroup {
  workspaceId: string | null;
  name: string;
  path: string | null;
  threads: ChatThread[];
}

const ORPHAN_GROUP_ID = "__no_project__";

function basenameFromPath(path: string): string {
  const trimmed = path.replace(/\/+$/, "");
  const slash = trimmed.lastIndexOf("/");
  return slash >= 0 ? trimmed.slice(slash + 1) : trimmed;
}

function workspaceLabel(workspace: WorkspaceInfo): string {
  if (workspace.name && workspace.name.trim().length > 0) return workspace.name;
  return basenameFromPath(workspace.path);
}

export function groupCodingThreadsByProject(
  sessions: ChatThread[],
  workspaceIdByThread: ReadonlyMap<string, string>,
  workspaces: ReadonlyArray<WorkspaceInfo>,
): ProjectThreadGroup[] {
  const wsById = new Map(workspaces.map((w) => [w.id, w] as const));
  const buckets = new Map<string, ProjectThreadGroup>();

  // Pre-seed a bucket for every known workspace so empty projects still show
  // (matches Codex's "<project> — No chats" pattern).
  for (const ws of workspaces) {
    buckets.set(ws.id, {
      workspaceId: ws.id,
      name: workspaceLabel(ws),
      path: ws.path,
      threads: [],
    });
  }

  for (const t of sessions) {
    const wsId = workspaceIdByThread.get(t.sessionKey);
    const ws = wsId ? wsById.get(wsId) : undefined;
    const key = ws ? ws.id : ORPHAN_GROUP_ID;
    let bucket = buckets.get(key);
    if (!bucket) {
      bucket = {
        workspaceId: ws ? ws.id : null,
        name: ws ? workspaceLabel(ws) : "Other chats",
        path: ws ? ws.path : null,
        threads: [],
      };
      buckets.set(key, bucket);
    }
    bucket.threads.push(t);
  }

  // Stable order: workspaces in the order they appear in the input list, orphans last.
  const ordered: ProjectThreadGroup[] = [];
  for (const ws of workspaces) {
    const bucket = buckets.get(ws.id);
    if (bucket) ordered.push(bucket);
  }
  // Any thread whose workspaceId isn't in the workspaces list (stale/missing) → still grouped by id.
  for (const [key, bucket] of buckets) {
    if (key === ORPHAN_GROUP_ID) continue;
    if (!wsById.has(key)) ordered.push(bucket);
  }
  const orphans = buckets.get(ORPHAN_GROUP_ID);
  if (orphans) ordered.push(orphans);
  return ordered;
}
