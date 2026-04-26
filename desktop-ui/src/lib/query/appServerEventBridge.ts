import type { QueryClient } from "@tanstack/react-query";
import { subscribeAppServerEvents } from "@/services/events";
import { qk } from "./queryKeys";

interface AppServerEventLike {
  type: string;
  workspaceId?: string;
}

export function startAppServerEventBridge(client: QueryClient): () => void {
  const unsubscribe = subscribeAppServerEvents((eventRaw) => {
    const event = eventRaw as unknown as AppServerEventLike;
    if (!event || typeof event.type !== "string") return;

    switch (event.type) {
      case "SkillsUpdateAvailable":
        if (event.workspaceId) {
          client.invalidateQueries({
            queryKey: qk.skills.list(event.workspaceId),
          });
        }
        return;
      case "AppListUpdated":
        if (event.workspaceId) {
          // Prefix-invalidate qk.apps.all() — TQ matches by prefix, so this
          // covers every (workspaceId, threadId) variant under "apps".
          client.invalidateQueries({ queryKey: qk.apps.all() });
        }
        return;
      case "PromptsUpdateAvailable":
        if (event.workspaceId) {
          client.invalidateQueries({
            queryKey: qk.prompts.list(event.workspaceId),
          });
        }
        return;
      case "ConfigChanged":
        client.invalidateQueries({ queryKey: qk.settings.app() });
        // Codex config.toml drives default-model resolution; refetch so the
        // settings panel reflects backend edits (CLI, file watcher, etc.).
        client.invalidateQueries({ queryKey: ["settings", "defaultModels"] });
        return;
      default:
        return;
    }
  });

  return unsubscribe;
}
