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
					// Prefix invalidation: covers all threadId variants.
					client.invalidateQueries({
						queryKey: ["apps", "list", event.workspaceId],
					});
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
				return;
			default:
				return;
		}
	});

	return unsubscribe;
}
