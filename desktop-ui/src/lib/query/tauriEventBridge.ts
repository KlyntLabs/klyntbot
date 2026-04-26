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
};

// Static list of (event_name, queryKeys) for non-entity events that still
// need to invalidate something.
const STATIC_ROUTES: ReadonlyArray<readonly [string, QueryKey[]]> = [
	["focus:state_changed", [qk.focus.status()]],
	["focus:phase_changed", [qk.focus.status()]],
	["focus:sync", [qk.focus.status()]],
];

const ALL_EVENTS = ["entity:updated", ...STATIC_ROUTES.map(([n]) => n)];

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

	return () => {
		for (const off of unlisteners) off();
	};
}

export const _internal = { ENTITY_INVALIDATIONS, STATIC_ROUTES, ALL_EVENTS };
