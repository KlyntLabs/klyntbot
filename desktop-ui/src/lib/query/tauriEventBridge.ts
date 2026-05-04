import type { QueryClient, QueryKey } from "@tanstack/react-query";
import { listen as defaultListen } from "@/utils/tauri-bridge";
import type { EntityKind } from "./entityKindMap";
import { qk } from "./queryKeys";

type ListenFn = typeof defaultListen;

interface EntityUpdatedPayload {
  entityKind: string;
  id?: string;
}

// Maps an entity kind to the query keys that should refetch. Keep narrow:
// `tasks.all()` invalidates today + byId(*) too, because TQ matches keys by
// prefix.
const ENTITY_INVALIDATIONS: Record<EntityKind, QueryKey[]> = {
  task: [qk.tasks.all()],
  project: [qk.tasks.all()], // project changes affect task lists
  objective: [],
  area: [],
  keyResult: [],
  focusSession: [qk.focus.todaySessions(), qk.focus.status()],
  productivity: [],
  note: [],
  notebook: [],
  finance: [],
  source: [],
  conversation: [],
  mirrorSnippet: [],
  brainVersion: [],
  pendingMemory: [],
  codingFact: [qk.codingMemory.all()],
  codingEpisode: [qk.codingMemory.all()],
};

// Static list of (event_name, queryKeys) for non-entity events that still
// need to invalidate something.
const STATIC_ROUTES: ReadonlyArray<readonly [string, QueryKey[]]> = [
  ["focus:state_changed", [qk.focus.status(), qk.launcher.dndActive()]],
  ["focus:phase_changed", [qk.focus.status()]],
  ["focus:sync", [qk.focus.status()]],
  ["chat:thread_created", [qk.threads.list()]],
  ["chat:thread_updated", [qk.threads.list()]],
  ["chat:thread_deleted", [qk.threads.all()]],
  ["chat:message_added", [qk.threads.list()]],
  ["mcp:server_status", [qk.system.mcpServers()]],
  ["mcp:startup_complete", [qk.system.mcpServers()]],
  ["productivity:nudge", [qk.launcher.dashboard()]],
  ["score:updated", [qk.launcher.dashboard()]],
  ["bucket:completed", [qk.launcher.dashboard()]],
];

const ALL_EVENTS = ["entity:updated", "data:version_bumped", ...STATIC_ROUTES.map(([n]) => n)];

export async function startTauriEventBridge(
  client: QueryClient,
  listen: ListenFn = defaultListen,
): Promise<() => void> {
  const unlisteners: Array<() => void> = [];

  const offEntity = await listen("entity:updated", (payload) => {
    const p = payload as EntityUpdatedPayload;
    const keys = ENTITY_INVALIDATIONS[p.entityKind as EntityKind];
    if (!keys) return; // unknown kind — ignore
    for (const queryKey of keys) {
      client.invalidateQueries({ queryKey });
    }
  });
  unlisteners.push(offEntity);

  for (const [event, keys] of STATIC_ROUTES) {
    const off = await listen(event, () => {
      for (const queryKey of keys) {
        client.invalidateQueries({ queryKey });
      }
    });
    unlisteners.push(off);
  }

  // Phase 4 broad-invalidate fallback. Fired by the desktop's
  // `start_data_version_watcher` when a foreign connection wrote and
  // we never saw the matching `entity:updated`. We invalidate the
  // most common broad keys rather than the entire cache to avoid a
  // thundering herd.
  const offBroad = await listen("data:version_bumped", () => {
    client.invalidateQueries({ queryKey: qk.threads.all() });
    client.invalidateQueries({ queryKey: qk.settings.all() });
    client.invalidateQueries({ queryKey: qk.settings.workspaces() });
    client.invalidateQueries({ queryKey: qk.tasks.all() });
    client.invalidateQueries({ queryKey: qk.focus.all() });
    client.invalidateQueries({ queryKey: qk.system.all() });
    client.invalidateQueries({ queryKey: qk.agents.all() });
    client.invalidateQueries({ queryKey: qk.models.all() });
    client.invalidateQueries({ queryKey: qk.git.all() });
    client.invalidateQueries({ queryKey: qk.dashboard.all() });
    client.invalidateQueries({ queryKey: qk.codingMemory.all() });
  });
  unlisteners.push(offBroad);

  return () => {
    for (const off of unlisteners) off();
  };
}

export const _internal = { ENTITY_INVALIDATIONS, STATIC_ROUTES, ALL_EVENTS };
