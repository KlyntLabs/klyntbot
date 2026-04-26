# Real-time Data Layer Foundation + Tray Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the current per-component `useState`+`ipc` pattern in `desktop-ui` with a universal TanStack Query data layer; add a Tauri event bridge that auto-invalidates queries when the backend emits canonical events (`entity:updated`, `focus:*`, `chat:*`, `mcp:*`, `productivity:*`); migrate the tray feature end-to-end as proof.

**Architecture:** TanStack Query v5 as the universal cache. Each Tauri webview gets its own `QueryClient` instance scoped to its React tree. A single `tauriEventBridge.ts` per webview subscribes to canonical events and calls `queryClient.invalidateQueries({ queryKey: [...] })`. Mutations flow through `useTauriMutation`, which auto-invalidates by entity-kind prefix using a static `entityKindMap`. The new IPC primitive remains `utils/tauri-bridge.ts` (`ipc`, `listen`); TanStack Query just orchestrates.

**Tech Stack:** `@tanstack/react-query@^5`, `@tanstack/react-query-devtools@^5`, existing `vitest`/`@testing-library/react`/`jsdom`, project's `utils/tauri-bridge.ts`.

**Master plan context:** Plan 1 of 4. Plans 2–4 cover remaining features (launcher, distraction, settings, etc.), the MCP cross-process entity-bridge socket, and the Distiller domain-event work respectively. Plan 1 alone produces a real-time tray that reflects mutations made from any other window instantly.

---

## File Structure

### New files

| Path | Responsibility |
|---|---|
| `desktop-ui/src/lib/query/entityKindMap.ts` | Static map: command-name prefix (`task_`, `note_`) → `EntityKind` enum value. Pure data + a tiny lookup function. |
| `desktop-ui/src/lib/query/queryKeys.ts` | Typed query-key factory. Single source of truth for cache keys (e.g. `qk.tasks.today()`, `qk.focus.status()`). |
| `desktop-ui/src/lib/query/client.ts` | `createQueryClient()` factory. One `QueryClient` per webview, sane defaults (no window-focus refetch, no retries on Tauri errors). |
| `desktop-ui/src/lib/query/QueryProvider.tsx` | Provider component that mounts `QueryClientProvider`, starts the event bridge, and (in dev) mounts devtools. |
| `desktop-ui/src/lib/query/tauriEventBridge.ts` | Subscribes to canonical Tauri events; routes each to `invalidateQueries`. The brain of cross-window real-time. |
| `desktop-ui/src/lib/query/useTauriQuery.ts` | Thin wrapper around `useQuery` — derives `queryFn` from a Tauri command, accepts a fallback. |
| `desktop-ui/src/lib/query/useTauriMutation.ts` | Wrapper around `useMutation` — auto-invalidates by entity-kind prefix; supports opt-in optimistic updates. |
| `desktop-ui/src/lib/query/index.ts` | Barrel export. |
| `desktop-ui/src/lib/query/tests/entityKindMap.test.ts` | Unit tests for the lookup. |
| `desktop-ui/src/lib/query/tests/queryKeys.test.ts` | Unit tests for the key factory (key shape stability). |
| `desktop-ui/src/lib/query/tests/client.test.ts` | Unit test for client defaults. |
| `desktop-ui/src/lib/query/tests/tauriEventBridge.test.ts` | Verifies an emitted event triggers `invalidateQueries` with the right keys. |
| `desktop-ui/src/lib/query/tests/useTauriQuery.test.tsx` | Verifies cache hit + fetch + fallback semantics. |
| `desktop-ui/src/lib/query/tests/useTauriMutation.test.tsx` | Verifies auto-invalidation + optimistic patches. |
| `desktop-ui/src/features/tray/tests/Tray.realtime.test.tsx` | Phase-B integration test: emit `entity:updated{kind:"task"}` → tray refetches today_tasks. |

### Files to modify

| Path | Change |
|---|---|
| `desktop-ui/package.json` | Add `@tanstack/react-query` + devtools to deps. |
| `desktop-ui/src/utils/tauri-bridge.ts` | Export `listen` already does. No change unless we discover gaps. (Verified: it does.) |
| `desktop-ui/src/App.tsx` | Wrap **every** route branch (main / launcher / tray / distraction / about) in `<QueryProvider>`. |
| `desktop-ui/src/features/tray/components/Tray.tsx` | Replace `useTrayQuery`/`useTrayMutation` imports + usages with `useTauriQuery`/`useTauriMutation`. |
| `desktop-ui/src/features/tray/components/FocusControl.tsx` | Same migration. |
| `desktop-ui/src/features/tray/hooks/useFocusTimer.ts` | Same migration. |

### Files to delete (end of Phase B)

- `desktop-ui/src/features/tray/hooks/useTrayQuery.ts`
- `desktop-ui/src/features/tray/hooks/useTrayMutation.ts`

---

## Phase A — Foundation

### Task A1: Install dependencies

**Files:**
- Modify: `desktop-ui/package.json`

- [ ] **Step 1: Add the dependencies via bun**

```bash
cd desktop-ui && bun add @tanstack/react-query@^5 @tanstack/react-query-devtools@^5
```

Expected: `package.json` and `bun.lockb` updated; install completes with no errors.

- [ ] **Step 2: Verify**

```bash
cd desktop-ui && grep '@tanstack/react-query' package.json
```

Expected: two lines, `@tanstack/react-query` and `@tanstack/react-query-devtools`, both `^5.x`.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/package.json desktop-ui/bun.lockb
git commit -m "chore(desktop-ui): add @tanstack/react-query for universal data layer"
```

---

### Task A2: `entityKindMap.ts` — command prefix → entity kind

**Files:**
- Create: `desktop-ui/src/lib/query/entityKindMap.ts`
- Test: `desktop-ui/src/lib/query/tests/entityKindMap.test.ts`

The backend's `EntityKind` enum (`crates/desktop-shared/src/types.rs:48-64`) defines which entity types fire `entity:updated` events. This file maps Tauri-command name prefixes to those kinds so `useTauriMutation` can infer which queries to invalidate without each callsite spelling it out.

- [ ] **Step 1: Write the failing test**

Create `desktop-ui/src/lib/query/tests/entityKindMap.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { entityKindForCommand, type EntityKind } from "../entityKindMap";

describe("entityKindForCommand", () => {
	it.each<[string, EntityKind | null]>([
		["task_create", "task"],
		["task_toggle_complete", "task"],
		["project_archive", "project"],
		["note_update", "note"],
		["notebook_create", "notebook"],
		["finance_transaction_add", "finance"],
		["focus_session_start", "focusSession"],
		["unknown_cmd", null],
		["", null],
	])("maps %s -> %s", (cmd, kind) => {
		expect(entityKindForCommand(cmd)).toBe(kind);
	});
});
```

- [ ] **Step 2: Run the test to verify it fails**

```bash
cd desktop-ui && bun run test src/lib/query/tests/entityKindMap.test.ts
```

Expected: FAIL with module-not-found for `../entityKindMap`.

- [ ] **Step 3: Write the implementation**

Create `desktop-ui/src/lib/query/entityKindMap.ts`:

```ts
// Mirrors crates/desktop-shared/src/types.rs#EntityKind. Strings match the
// backend's serde camelCase encoding so we can compare directly against
// EntityUpdatedPayload.entityKind without translation.
export type EntityKind =
	| "task"
	| "project"
	| "objective"
	| "area"
	| "keyResult"
	| "focusSession"
	| "productivity"
	| "note"
	| "notebook"
	| "finance"
	| "source"
	| "conversation"
	| "mirrorSnippet"
	| "brainVersion"
	| "pendingMemory";

// Ordered longest-prefix-first so "notebook_" wins over "note_".
const PREFIX_TABLE: ReadonlyArray<readonly [string, EntityKind]> = [
	["notebook_", "notebook"],
	["note_", "note"],
	["task_", "task"],
	["project_", "project"],
	["objective_", "objective"],
	["area_", "area"],
	["key_result_", "keyResult"],
	["focus_", "focusSession"],
	["productivity_", "productivity"],
	["finance_", "finance"],
	["source_", "source"],
	["conversation_", "conversation"],
];

export function entityKindForCommand(cmd: string): EntityKind | null {
	for (const [prefix, kind] of PREFIX_TABLE) {
		if (cmd.startsWith(prefix)) return kind;
	}
	return null;
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd desktop-ui && bun run test src/lib/query/tests/entityKindMap.test.ts
```

Expected: 9 passing tests.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/lib/query/entityKindMap.ts desktop-ui/src/lib/query/tests/entityKindMap.test.ts
git commit -m "feat(desktop-ui): add entityKindMap — command prefix → EntityKind lookup"
```

---

### Task A3: `queryKeys.ts` — typed key factory

**Files:**
- Create: `desktop-ui/src/lib/query/queryKeys.ts`
- Test: `desktop-ui/src/lib/query/tests/queryKeys.test.ts`

A central, typed key factory eliminates string-key drift across callsites. TanStack Query keys are arrays — by convention the first element is the entity domain, then sub-namespaces, then args. Drift between writers (`['tasks', 'today']`) and readers (`['task', 'today']`) is the #1 cause of "invalidation didn't work" bugs in TQ apps; the factory eliminates it.

- [ ] **Step 1: Write the failing test**

Create `desktop-ui/src/lib/query/tests/queryKeys.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { qk } from "../queryKeys";

describe("queryKeys", () => {
	it("tasks.today is stable", () => {
		expect(qk.tasks.today()).toEqual(["tasks", "today"]);
	});

	it("tasks.byId encodes id", () => {
		expect(qk.tasks.byId("abc")).toEqual(["tasks", "byId", "abc"]);
	});

	it("focus.status has no args", () => {
		expect(qk.focus.status()).toEqual(["focus", "status"]);
	});

	it("flashcards.dueCount is namespaced", () => {
		expect(qk.flashcards.dueCount()).toEqual(["flashcards", "dueCount"]);
	});

	it("calendar.eventsForDate encodes date", () => {
		expect(qk.calendar.eventsForDate("2026-04-26")).toEqual([
			"calendar",
			"events",
			"2026-04-26",
		]);
	});

	it("focus.todaySessions is stable", () => {
		expect(qk.focus.todaySessions()).toEqual(["focus", "todaySessions"]);
	});
});
```

- [ ] **Step 2: Run the test to verify it fails**

```bash
cd desktop-ui && bun run test src/lib/query/tests/queryKeys.test.ts
```

Expected: FAIL — module not found.

- [ ] **Step 3: Write the implementation**

Create `desktop-ui/src/lib/query/queryKeys.ts`:

```ts
// Single source of truth for query keys. Add new domains here, never inline
// raw arrays at callsites — see docs/superpowers/plans/2026-04-26-realtime-
// data-layer-phase-1.md "Type consistency" section for why.
export const qk = {
	tasks: {
		all: () => ["tasks"] as const,
		today: () => ["tasks", "today"] as const,
		byId: (id: string) => ["tasks", "byId", id] as const,
	},
	calendar: {
		all: () => ["calendar"] as const,
		eventsForDate: (date: string) => ["calendar", "events", date] as const,
	},
	focus: {
		all: () => ["focus"] as const,
		status: () => ["focus", "status"] as const,
		todaySessions: () => ["focus", "todaySessions"] as const,
	},
	flashcards: {
		all: () => ["flashcards"] as const,
		dueCount: () => ["flashcards", "dueCount"] as const,
	},
} as const;

export type QueryKey = ReturnType<
	(typeof qk)[keyof typeof qk][keyof (typeof qk)[keyof typeof qk]]
>;
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd desktop-ui && bun run test src/lib/query/tests/queryKeys.test.ts
```

Expected: 6 passing tests.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/lib/query/queryKeys.ts desktop-ui/src/lib/query/tests/queryKeys.test.ts
git commit -m "feat(desktop-ui): add typed queryKeys factory"
```

---

### Task A4: `client.ts` — `QueryClient` factory

**Files:**
- Create: `desktop-ui/src/lib/query/client.ts`
- Test: `desktop-ui/src/lib/query/tests/client.test.ts`

Defaults matter: in a Tauri desktop app, browser-style `refetchOnWindowFocus` causes a stampede when the user alt-tabs (every webview re-fires every query). We disable it. Retries also default to 3 attempts with backoff — but since Tauri command failures are usually deterministic (typo, missing handler), one retry is plenty.

- [ ] **Step 1: Write the failing test**

Create `desktop-ui/src/lib/query/tests/client.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { createQueryClient } from "../client";

describe("createQueryClient", () => {
	it("disables refetchOnWindowFocus to avoid alt-tab stampede", () => {
		const client = createQueryClient();
		const defaults = client.getDefaultOptions().queries;
		expect(defaults?.refetchOnWindowFocus).toBe(false);
	});

	it("uses 30s default staleTime", () => {
		const client = createQueryClient();
		expect(client.getDefaultOptions().queries?.staleTime).toBe(30_000);
	});

	it("retries once on failure (Tauri errors are usually deterministic)", () => {
		const client = createQueryClient();
		expect(client.getDefaultOptions().queries?.retry).toBe(1);
	});

	it("each invocation returns an independent client", () => {
		const a = createQueryClient();
		const b = createQueryClient();
		expect(a).not.toBe(b);
	});
});
```

- [ ] **Step 2: Run the test to verify it fails**

```bash
cd desktop-ui && bun run test src/lib/query/tests/client.test.ts
```

Expected: FAIL — module not found.

- [ ] **Step 3: Write the implementation**

Create `desktop-ui/src/lib/query/client.ts`:

```ts
import { QueryClient } from "@tanstack/react-query";

// One QueryClient per webview. Exposed as a factory (not a singleton) so each
// Tauri window's React tree gets its own cache; events broadcast across
// windows reach all clients via tauriEventBridge.
export function createQueryClient(): QueryClient {
	return new QueryClient({
		defaultOptions: {
			queries: {
				// Tauri webviews trigger "focus" events constantly during dev
				// reload + multi-window setups; the browser-default refetch
				// would stampede.
				refetchOnWindowFocus: false,
				// 30s mirrors the .bak's default. Push events drive freshness;
				// staleTime is just the "no events seen, we'd better double-check"
				// safety net.
				staleTime: 30_000,
				// Tauri command failures are usually deterministic (handler not
				// registered, type mismatch). Retrying 3× wastes time.
				retry: 1,
			},
			mutations: {
				retry: 0,
			},
		},
	});
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd desktop-ui && bun run test src/lib/query/tests/client.test.ts
```

Expected: 4 passing tests.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/lib/query/client.ts desktop-ui/src/lib/query/tests/client.test.ts
git commit -m "feat(desktop-ui): add QueryClient factory with Tauri-tuned defaults"
```

---

### Task A5: `tauriEventBridge.ts` — events → invalidation

**Files:**
- Create: `desktop-ui/src/lib/query/tauriEventBridge.ts`
- Test: `desktop-ui/src/lib/query/tests/tauriEventBridge.test.ts`

This is the core of cross-window real-time. The function subscribes to canonical Tauri events and calls `queryClient.invalidateQueries` for the affected keys. Because Tauri's `app.emit()` broadcasts to **every** open webview (per `crates/desktop/src/app_core.rs:26`), this single subscription per webview makes the launcher, tray, and main app all stay in sync automatically.

The mapping is data-driven (a const table) so adding a new event ↔ key route is a one-line change.

- [ ] **Step 1: Write the failing test**

Create `desktop-ui/src/lib/query/tests/tauriEventBridge.test.ts`:

```ts
import { QueryClient } from "@tanstack/react-query";
import { describe, expect, it, vi } from "vitest";
import { qk } from "../queryKeys";
import { startTauriEventBridge } from "../tauriEventBridge";

type Handler = (payload: unknown) => void;

function fakeListenFactory() {
	const subs = new Map<string, Handler>();
	const listen = vi.fn(async (event: string, handler: Handler) => {
		subs.set(event, handler);
		return () => subs.delete(event);
	});
	const fire = (event: string, payload: unknown) =>
		subs.get(event)?.(payload);
	return { listen, fire, subs };
}

describe("tauriEventBridge", () => {
	it("entity:updated{entityKind:'task'} invalidates tasks.all", async () => {
		const client = new QueryClient();
		const spy = vi.spyOn(client, "invalidateQueries");
		const { listen, fire } = fakeListenFactory();

		const stop = await startTauriEventBridge(client, listen);
		fire("entity:updated", { entityKind: "task", id: "t1" });

		expect(spy).toHaveBeenCalledWith({ queryKey: qk.tasks.all() });
		stop();
	});

	it("focus:phase_changed invalidates focus.status", async () => {
		const client = new QueryClient();
		const spy = vi.spyOn(client, "invalidateQueries");
		const { listen, fire } = fakeListenFactory();

		const stop = await startTauriEventBridge(client, listen);
		fire("focus:phase_changed", { phase: "break" });

		expect(spy).toHaveBeenCalledWith({ queryKey: qk.focus.status() });
		stop();
	});

	it("entity:updated with unknown kind invalidates nothing", async () => {
		const client = new QueryClient();
		const spy = vi.spyOn(client, "invalidateQueries");
		const { listen, fire } = fakeListenFactory();

		const stop = await startTauriEventBridge(client, listen);
		fire("entity:updated", { entityKind: "unknownKind", id: "x" });

		expect(spy).not.toHaveBeenCalled();
		stop();
	});

	it("returns a cleanup that unsubscribes all events", async () => {
		const client = new QueryClient();
		const { listen, subs, fire } = fakeListenFactory();
		const spy = vi.spyOn(client, "invalidateQueries");

		const stop = await startTauriEventBridge(client, listen);
		expect(subs.size).toBeGreaterThan(0);
		stop();
		expect(subs.size).toBe(0);

		fire("entity:updated", { entityKind: "task", id: "t1" });
		expect(spy).not.toHaveBeenCalled();
	});
});
```

- [ ] **Step 2: Run the test to verify it fails**

```bash
cd desktop-ui && bun run test src/lib/query/tests/tauriEventBridge.test.ts
```

Expected: FAIL — module not found.

- [ ] **Step 3: Write the implementation**

Create `desktop-ui/src/lib/query/tauriEventBridge.ts`:

```ts
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
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd desktop-ui && bun run test src/lib/query/tests/tauriEventBridge.test.ts
```

Expected: 4 passing tests.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/lib/query/tauriEventBridge.ts desktop-ui/src/lib/query/tests/tauriEventBridge.test.ts
git commit -m "feat(desktop-ui): add tauriEventBridge — Tauri events → query invalidation"
```

---

### Task A6: `useTauriQuery.ts` — query hook

**Files:**
- Create: `desktop-ui/src/lib/query/useTauriQuery.ts`
- Test: `desktop-ui/src/lib/query/tests/useTauriQuery.test.tsx`

Thin wrapper around `useQuery`. The hook signature mirrors `useTrayQuery`'s style (`(cmd, args, fallback)`) so migration callsites change minimally — but underneath it gets the full TanStack cache + dedup + events.

- [ ] **Step 1: Write the failing test**

Create `desktop-ui/src/lib/query/tests/useTauriQuery.test.tsx`:

```tsx
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("@/utils/tauri-bridge", () => ({
	ipc: vi.fn(),
}));

import { ipc } from "@/utils/tauri-bridge";
import { qk } from "../queryKeys";
import { useTauriQuery } from "../useTauriQuery";

const mockedIpc = vi.mocked(ipc);

afterEach(() => {
	mockedIpc.mockReset();
});

function wrapper(client: QueryClient) {
	return ({ children }: { children: ReactNode }) => (
		<QueryClientProvider client={client}>{children}</QueryClientProvider>
	);
}

describe("useTauriQuery", () => {
	it("calls the matching ipc command and returns its data", async () => {
		mockedIpc.mockResolvedValueOnce([{ id: "1" }]);
		const client = new QueryClient({
			defaultOptions: { queries: { retry: 0 } },
		});

		const { result } = renderHook(
			() =>
				useTauriQuery({
					queryKey: qk.tasks.today(),
					command: "today_tasks",
				}),
			{ wrapper: wrapper(client) },
		);

		await waitFor(() => expect(result.current.data).toEqual([{ id: "1" }]));
		expect(mockedIpc).toHaveBeenCalledWith("today_tasks", undefined);
	});

	it("returns the fallback while loading the first time", async () => {
		mockedIpc.mockImplementation(() => new Promise(() => {})); // never resolves
		const client = new QueryClient({
			defaultOptions: { queries: { retry: 0 } },
		});

		const { result } = renderHook(
			() =>
				useTauriQuery({
					queryKey: qk.tasks.today(),
					command: "today_tasks",
					fallback: [],
				}),
			{ wrapper: wrapper(client) },
		);

		expect(result.current.data).toEqual([]);
		expect(result.current.isLoading).toBe(true);
	});

	it("forwards args to ipc", async () => {
		mockedIpc.mockResolvedValueOnce([]);
		const client = new QueryClient({
			defaultOptions: { queries: { retry: 0 } },
		});

		renderHook(
			() =>
				useTauriQuery({
					queryKey: qk.calendar.eventsForDate("2026-04-26"),
					command: "productivity_calendar_events",
					args: { date: "2026-04-26" },
				}),
			{ wrapper: wrapper(client) },
		);

		await waitFor(() =>
			expect(mockedIpc).toHaveBeenCalledWith(
				"productivity_calendar_events",
				{ date: "2026-04-26" },
			),
		);
	});
});
```

- [ ] **Step 2: Run the test to verify it fails**

```bash
cd desktop-ui && bun run test src/lib/query/tests/useTauriQuery.test.tsx
```

Expected: FAIL — module not found.

- [ ] **Step 3: Write the implementation**

Create `desktop-ui/src/lib/query/useTauriQuery.ts`:

```ts
import {
	type QueryKey,
	useQuery,
	type UseQueryResult,
} from "@tanstack/react-query";
import { ipc } from "@/utils/tauri-bridge";

export interface TauriQueryOptions<TData> {
	queryKey: QueryKey;
	command: string;
	args?: Record<string, unknown>;
	/** Returned as `data` until the first successful fetch. */
	fallback?: TData;
	/** Disable the query (e.g. wait for a prerequisite). */
	enabled?: boolean;
	/** Override the cache stale time for this query. Default 30s (client.ts). */
	staleTime?: number;
}

export function useTauriQuery<TData>(
	opts: TauriQueryOptions<TData>,
): UseQueryResult<TData> & { data: TData } {
	const result = useQuery<TData>({
		queryKey: opts.queryKey,
		queryFn: () => ipc<TData>(opts.command, opts.args),
		enabled: opts.enabled,
		staleTime: opts.staleTime,
		placeholderData: opts.fallback,
	});

	return {
		...result,
		// `placeholderData` keeps the fallback as `data` until the query
		// succeeds, so the cast is safe.
		data: (result.data ?? opts.fallback) as TData,
	};
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd desktop-ui && bun run test src/lib/query/tests/useTauriQuery.test.tsx
```

Expected: 3 passing tests.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/lib/query/useTauriQuery.ts desktop-ui/src/lib/query/tests/useTauriQuery.test.tsx
git commit -m "feat(desktop-ui): add useTauriQuery — TanStack Query wrapper for ipc"
```

---

### Task A7: `useTauriMutation.ts` — auto-invalidating mutation hook

**Files:**
- Create: `desktop-ui/src/lib/query/useTauriMutation.ts`
- Test: `desktop-ui/src/lib/query/tests/useTauriMutation.test.tsx`

The hook auto-invalidates queries based on the mutation command name's entity prefix. Optimistic patches are opt-in via the `optimistic` option — if provided, the cache is patched immediately, the network call fires, and on error the patch is rolled back.

- [ ] **Step 1: Write the failing test**

Create `desktop-ui/src/lib/query/tests/useTauriMutation.test.tsx`:

```tsx
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("@/utils/tauri-bridge", () => ({
	ipc: vi.fn(),
}));

import { ipc } from "@/utils/tauri-bridge";
import { qk } from "../queryKeys";
import { useTauriMutation } from "../useTauriMutation";

const mockedIpc = vi.mocked(ipc);

afterEach(() => {
	mockedIpc.mockReset();
});

function wrap(client: QueryClient) {
	return ({ children }: { children: ReactNode }) => (
		<QueryClientProvider client={client}>{children}</QueryClientProvider>
	);
}

describe("useTauriMutation", () => {
	it("calls ipc with the cmd + args", async () => {
		mockedIpc.mockResolvedValueOnce({ ok: true });
		const client = new QueryClient();
		const { result } = renderHook(
			() => useTauriMutation({ command: "task_toggle_complete" }),
			{ wrapper: wrap(client) },
		);

		await act(async () => {
			await result.current.mutate({ id: "t1" });
		});

		expect(mockedIpc).toHaveBeenCalledWith("task_toggle_complete", { id: "t1" });
	});

	it("auto-invalidates tasks.all after a task_* mutation", async () => {
		mockedIpc.mockResolvedValueOnce({ ok: true });
		const client = new QueryClient();
		const spy = vi.spyOn(client, "invalidateQueries");

		const { result } = renderHook(
			() => useTauriMutation({ command: "task_toggle_complete" }),
			{ wrapper: wrap(client) },
		);

		await act(async () => {
			await result.current.mutate({ id: "t1" });
		});

		expect(spy).toHaveBeenCalledWith({ queryKey: ["tasks"] });
	});

	it("applies optimistic patch + rolls back on error", async () => {
		const client = new QueryClient();
		client.setQueryData(qk.tasks.today(), [
			{ id: "t1", completed: false },
		]);
		mockedIpc.mockRejectedValueOnce(new Error("boom"));

		const { result } = renderHook(
			() =>
				useTauriMutation<unknown, { id: string }>({
					command: "task_toggle_complete",
					optimistic: {
						queryKey: qk.tasks.today(),
						update: (vars, prev: Array<{ id: string; completed: boolean }>) =>
							prev.map((t) =>
								t.id === vars.id ? { ...t, completed: true } : t,
							),
					},
				}),
			{ wrapper: wrap(client) },
		);

		await act(async () => {
			await result.current.mutate({ id: "t1" }).catch(() => {});
		});

		// Roll-back: cache restored to pre-mutation state
		expect(client.getQueryData(qk.tasks.today())).toEqual([
			{ id: "t1", completed: false },
		]);
	});

	it("optimistic patch survives a successful mutation", async () => {
		const client = new QueryClient();
		client.setQueryData(qk.tasks.today(), [
			{ id: "t1", completed: false },
		]);
		mockedIpc.mockResolvedValueOnce({ ok: true });

		const { result } = renderHook(
			() =>
				useTauriMutation<unknown, { id: string }>({
					command: "task_toggle_complete",
					optimistic: {
						queryKey: qk.tasks.today(),
						update: (vars, prev: Array<{ id: string; completed: boolean }>) =>
							prev.map((t) =>
								t.id === vars.id ? { ...t, completed: true } : t,
							),
					},
				}),
			{ wrapper: wrap(client) },
		);

		await act(async () => {
			await result.current.mutate({ id: "t1" });
		});

		await waitFor(() =>
			expect(client.getQueryData(qk.tasks.today())).toEqual([
				{ id: "t1", completed: true },
			]),
		);
	});
});
```

- [ ] **Step 2: Run the test to verify it fails**

```bash
cd desktop-ui && bun run test src/lib/query/tests/useTauriMutation.test.tsx
```

Expected: FAIL — module not found.

- [ ] **Step 3: Write the implementation**

Create `desktop-ui/src/lib/query/useTauriMutation.ts`:

```ts
import {
	type QueryKey,
	useMutation,
	useQueryClient,
} from "@tanstack/react-query";
import { ipc } from "@/utils/tauri-bridge";
import { entityKindForCommand } from "./entityKindMap";

export interface OptimisticConfig<TVars, TPrev> {
	queryKey: QueryKey;
	update: (vars: TVars, prev: TPrev) => TPrev;
}

export interface TauriMutationOptions<TData, TVars> {
	command: string;
	/**
	 * Override the auto-derived invalidation. Pass an empty array to skip.
	 * Default: invalidates the entity-domain bucket inferred from the command.
	 */
	invalidates?: QueryKey[];
	/** Opt-in optimistic patch. Rolls back on error. */
	// biome-ignore lint/suspicious/noExplicitAny: TPrev is opaque to the hook
	optimistic?: OptimisticConfig<TVars, any>;
	onSuccess?: (data: TData, vars: TVars) => void;
	onError?: (error: unknown, vars: TVars) => void;
}

export function useTauriMutation<TData = unknown, TVars = void>(
	opts: TauriMutationOptions<TData, TVars>,
) {
	const client = useQueryClient();

	const mutation = useMutation<
		TData,
		unknown,
		TVars,
		{ rollback?: () => void }
	>({
		mutationFn: (vars) =>
			ipc<TData>(opts.command, vars as Record<string, unknown> | undefined),

		onMutate: async (vars) => {
			if (!opts.optimistic) return {};
			const { queryKey, update } = opts.optimistic;
			await client.cancelQueries({ queryKey });
			const prev = client.getQueryData(queryKey);
			client.setQueryData(queryKey, (old: unknown) => update(vars, old));
			return { rollback: () => client.setQueryData(queryKey, prev) };
		},

		onError: (err, vars, ctx) => {
			ctx?.rollback?.();
			opts.onError?.(err, vars);
		},

		onSuccess: (data, vars) => {
			opts.onSuccess?.(data, vars);
		},

		onSettled: () => {
			const overrides = opts.invalidates;
			if (overrides) {
				for (const key of overrides) client.invalidateQueries({ queryKey: key });
				return;
			}
			const kind = entityKindForCommand(opts.command);
			if (kind) {
				// Broad-prefix invalidation: queries starting with [kindRoot]
				// match. e.g. ["tasks"] invalidates ["tasks","today"], etc.
				const root = kind === "task" ? "tasks" : kind;
				client.invalidateQueries({ queryKey: [root] });
			}
		},
	});

	return {
		mutate: mutation.mutateAsync,
		isLoading: mutation.isPending,
		error: mutation.error,
	};
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd desktop-ui && bun run test src/lib/query/tests/useTauriMutation.test.tsx
```

Expected: 4 passing tests.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/lib/query/useTauriMutation.ts desktop-ui/src/lib/query/tests/useTauriMutation.test.tsx
git commit -m "feat(desktop-ui): add useTauriMutation with auto-invalidation + optimistic patches"
```

---

### Task A8: `QueryProvider.tsx` — provider component

**Files:**
- Create: `desktop-ui/src/lib/query/QueryProvider.tsx`

The provider mounts the `QueryClientProvider`, starts `tauriEventBridge` once, and (in `import.meta.env.DEV`) mounts the devtools floating button. One-line wrap at every webview entry point in `App.tsx`.

- [ ] **Step 1: Write the implementation**

Create `desktop-ui/src/lib/query/QueryProvider.tsx`:

```tsx
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ReactQueryDevtools } from "@tanstack/react-query-devtools";
import { type ReactNode, useEffect, useRef } from "react";
import { createQueryClient } from "./client";
import { startTauriEventBridge } from "./tauriEventBridge";

interface QueryProviderProps {
	children: ReactNode;
	/** For tests: inject a pre-configured client. */
	client?: QueryClient;
}

export function QueryProvider({ children, client }: QueryProviderProps) {
	// One client per component instance (per webview React tree).
	const clientRef = useRef<QueryClient>();
	if (!clientRef.current) clientRef.current = client ?? createQueryClient();

	useEffect(() => {
		let stop: (() => void) | null = null;
		let cancelled = false;

		startTauriEventBridge(clientRef.current!).then((s) => {
			if (cancelled) s();
			else stop = s;
		});

		return () => {
			cancelled = true;
			stop?.();
		};
	}, []);

	return (
		<QueryClientProvider client={clientRef.current}>
			{children}
			{import.meta.env.DEV && (
				<ReactQueryDevtools initialIsOpen={false} buttonPosition="bottom-left" />
			)}
		</QueryClientProvider>
	);
}
```

- [ ] **Step 2: Sanity-typecheck**

```bash
cd desktop-ui && bunx tsc --noEmit 2>&1 | tail -20 && echo "---DONE---"
```

Expected: only `---DONE---` (no errors).

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/lib/query/QueryProvider.tsx
git commit -m "feat(desktop-ui): add QueryProvider component"
```

---

### Task A9: `index.ts` — barrel export

**Files:**
- Create: `desktop-ui/src/lib/query/index.ts`

- [ ] **Step 1: Write the file**

Create `desktop-ui/src/lib/query/index.ts`:

```ts
export { createQueryClient } from "./client";
export { QueryProvider } from "./QueryProvider";
export { qk } from "./queryKeys";
export type { QueryKey } from "./queryKeys";
export { startTauriEventBridge } from "./tauriEventBridge";
export { useTauriQuery } from "./useTauriQuery";
export type { TauriQueryOptions } from "./useTauriQuery";
export { useTauriMutation } from "./useTauriMutation";
export type {
	OptimisticConfig,
	TauriMutationOptions,
} from "./useTauriMutation";
export { entityKindForCommand } from "./entityKindMap";
export type { EntityKind } from "./entityKindMap";
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/lib/query/index.ts
git commit -m "feat(desktop-ui): barrel-export the query module"
```

---

### Task A10: Wire `QueryProvider` into every webview

**Files:**
- Modify: `desktop-ui/src/App.tsx`

Each webview (main, launcher, tray, distraction-overlay, about) gets its own QueryClient via the same `<QueryProvider>` wrap. Tauri broadcasts events globally, so each webview's bridge subscribes once and they all stay in sync.

- [ ] **Step 1: Read current App.tsx structure**

```bash
cd desktop-ui && cat src/App.tsx
```

Note the existing route branches that return `<MainApp/>`, `<Launcher/>`, `<Tray/>`, `<DistractionOverlay/>`, `<AboutView/>`.

- [ ] **Step 2: Wrap every branch in `<QueryProvider>`**

Edit `desktop-ui/src/App.tsx`. Add the import:

```tsx
import { QueryProvider } from "@/lib/query";
```

Replace each branch's return value. Pattern — for the launcher branch:

```tsx
if (realLabel === "launcher" || windowLabel === "launcher") {
	return (
		<QueryProvider>
			<Suspense fallback={null}>
				<Launcher />
			</Suspense>
		</QueryProvider>
	);
}
```

Apply the same wrapper to every other branch (`tray`, `distraction-overlay`, `about`, and the default `<MainApp/>` return at the bottom).

- [ ] **Step 3: Typecheck**

```bash
cd desktop-ui && bunx tsc --noEmit 2>&1 | tail -10 && echo "---DONE---"
```

Expected: only `---DONE---`.

- [ ] **Step 4: Run all existing tests to confirm no regression**

```bash
cd desktop-ui && bun run test
```

Expected: all suites pass. New `lib/query/tests/*` and pre-existing tests both green.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/App.tsx
git commit -m "feat(desktop-ui): wrap every webview entry in QueryProvider"
```

---

### Task A11: Smoke-verify the foundation in dev

**Files:** none modified — this is a manual verification task.

- [ ] **Step 1: Start the desktop in dev mode**

In one terminal:
```bash
cd desktop-ui && bun run dev
```
In another:
```bash
cargo tauri dev
```

- [ ] **Step 2: Open the React Query devtools floating button**

In the main window, you should see a floating "TanStack" logo at bottom-left (DEV-only). Click it — the devtools panel opens. The query list should be empty (no migrations yet).

- [ ] **Step 3: Open the tray window**

Click the menu-bar icon. The tray opens. Check the tray's devtools (separate floating button — each webview has its own client). Still empty (tray hasn't migrated yet either).

- [ ] **Step 4: Confirm no console errors**

In each webview's web inspector (Cmd+Opt+I in macOS Tauri), the Console tab should show no errors related to QueryProvider or TanStack.

- [ ] **Step 5: Commit anything created (devtools, etc.) — usually nothing to commit here**

Skip if no diff.

---

## Phase B — Tray migration (proof of foundation)

### Task B1: Migrate `useFocusTimer.ts`

**Files:**
- Modify: `desktop-ui/src/features/tray/hooks/useFocusTimer.ts`

Replace `useTrayQuery` and `useTrayMutation` with the new hooks. Keep `useEvent` subscriptions untouched — they're a valid push channel for transient UI state (warnings, dnd hints) that doesn't belong in the cache.

- [ ] **Step 1: Locate the migration sites**

```bash
cd desktop-ui && grep -nE "useTrayQuery|useTrayMutation" src/features/tray/hooks/useFocusTimer.ts
```

Expected: ~13 lines (1 query, 1 query, ~9 mutation declarations).

- [ ] **Step 2: Edit imports**

Replace:
```ts
import { useTrayMutation } from "./useTrayMutation";
import { useTrayQuery } from "./useTrayQuery";
```
with:
```ts
import { qk, useTauriMutation, useTauriQuery } from "@/lib/query";
```

- [ ] **Step 3: Replace the query call**

Find:
```ts
const initialStatusQuery = useTrayQuery<FocusSessionStatus>(
	"focus_session_status",
	undefined,
	{ active: false, sync: null, session: null },
);
const initialStatus = initialStatusQuery.data;
const refetch = initialStatusQuery.refetch;
```
Replace with:
```ts
const initialStatusQuery = useTauriQuery<FocusSessionStatus>({
	queryKey: qk.focus.status(),
	command: "focus_session_status",
	fallback: { active: false, sync: null, session: null },
});
const initialStatus = initialStatusQuery.data;
const refetch = () => initialStatusQuery.refetch();
```

- [ ] **Step 4: Replace each mutation declaration**

Pattern — find:
```ts
const startMut = useTrayMutation<FocusSession, Record<string, unknown>>(
	"focus_session_start",
);
```
Replace with:
```ts
const startMut = useTauriMutation<FocusSession, Record<string, unknown>>({
	command: "focus_session_start",
});
```

Repeat for: `stopMut`, `pauseMut`, `resumeMut`, `extendMut`, `extendWorkMut`, `skipBreakMut`, `takeBreakMut`, `startBreakMut`, `logDistractionMut`, and the `todaySessionsQuery` (which is also `useTrayQuery`).

Specifically for `todaySessionsQuery`:
```ts
const todaySessionsQuery = useTauriQuery<...>({
	queryKey: qk.focus.todaySessions(),
	command: "focus_today_sessions",
	fallback: [...],
});
```

- [ ] **Step 5: Confirm `mutate` callsites still type-check**

The new hook returns `{ mutate, isLoading, error }`. The old hook returned `{ mutate, loading }`. Find every `.loading` access:

```bash
cd desktop-ui && grep -n "Mut\.loading\|Query\.loading" src/features/tray/hooks/useFocusTimer.ts
```

For each match, rename `.loading` → `.isLoading` (mutations) or use `.isLoading` (queries — same field). Mutations: 0 occurrences expected since `useFocusTimer` only checks `loading` on the query.

- [ ] **Step 6: Typecheck**

```bash
cd desktop-ui && bunx tsc --noEmit 2>&1 | grep useFocusTimer | head -10
```

Expected: no output (clean).

- [ ] **Step 7: Run existing tray tests if any**

```bash
cd desktop-ui && bun run test src/features/tray
```

Expected: all pass (or no tests collected — that's fine; B5 adds the new integration test).

- [ ] **Step 8: Commit**

```bash
git add desktop-ui/src/features/tray/hooks/useFocusTimer.ts
git commit -m "refactor(desktop-ui): migrate useFocusTimer to useTauriQuery/useTauriMutation"
```

---

### Task B2: Migrate `Tray.tsx`

**Files:**
- Modify: `desktop-ui/src/features/tray/components/Tray.tsx`

- [ ] **Step 1: Replace imports**

Find:
```ts
import { useTrayMutation } from "../hooks/useTrayMutation";
import { useTrayQuery } from "../hooks/useTrayQuery";
```
Replace with:
```ts
import { qk, useTauriMutation, useTauriQuery } from "@/lib/query";
```

- [ ] **Step 2: Replace the `today_tasks` query**

Find:
```ts
const todayTasksQuery = useTrayQuery<TodayTask[]>(
	"today_tasks",
	undefined,
	[],
);
const todayTasks = todayTasksQuery.data;
```
Replace with:
```ts
const todayTasksQuery = useTauriQuery<TodayTask[]>({
	queryKey: qk.tasks.today(),
	command: "today_tasks",
	fallback: [],
});
const todayTasks = todayTasksQuery.data;
```

- [ ] **Step 3: Replace the `productivity_calendar_events` query**

Find:
```ts
const calendarQuery = useTrayQuery<CalendarEvent[]>(
	"productivity_calendar_events",
	{ date: todayISO() },
	[],
);
const calendarEvents = calendarQuery.data;
```
Replace with:
```ts
const dateKey = todayISO();
const calendarQuery = useTauriQuery<CalendarEvent[]>({
	queryKey: qk.calendar.eventsForDate(dateKey),
	command: "productivity_calendar_events",
	args: { date: dateKey },
	fallback: [],
});
const calendarEvents = calendarQuery.data;
```

- [ ] **Step 4: Replace the toggle-complete mutation with optimistic patch**

Find:
```ts
const toggleComplete = useTrayMutation<TodayTask, { id: string }>(
	"task_toggle_complete",
);
```
Replace with:
```ts
const toggleComplete = useTauriMutation<TodayTask, { id: string }>({
	command: "task_toggle_complete",
	optimistic: {
		queryKey: qk.tasks.today(),
		update: (vars, prev: TodayTask[] = []) =>
			prev.map((t) =>
				t.id === vars.id ? { ...t, completed: !t.completed } : t,
			),
	},
});
```

- [ ] **Step 5: Drop the local `completedIds` Set workaround**

Now that mutations patch the cache directly, the local Set is redundant.

Find:
```ts
const [completedIds, toggleCompletedId] = useSetToggle();
```
Delete that line.

Find:
```ts
const handleToggleTask = async (taskId: string) => {
	toggleCompletedId(taskId);
	await toggleComplete.mutate({ id: taskId });
};
```
Replace with:
```ts
const handleToggleTask = async (taskId: string) => {
	await toggleComplete.mutate({ id: taskId });
};
```

Find:
```ts
const isTaskCompleted = useCallback(
	(t: TodayTask) => t.completed || completedIds.has(t.id),
	[completedIds],
);
```
Replace with:
```ts
const isTaskCompleted = useCallback(
	(t: TodayTask) => t.completed,
	[],
);
```

- [ ] **Step 6: Remove the now-unused `useSetToggle` import**

```bash
cd desktop-ui && grep -n "useSetToggle" src/features/tray/components/Tray.tsx
```

Delete the import line if `useSetToggle` is no longer referenced.

- [ ] **Step 7: Typecheck**

```bash
cd desktop-ui && bunx tsc --noEmit 2>&1 | grep "Tray.tsx" | head -10
```

Expected: no output.

- [ ] **Step 8: Commit**

```bash
git add desktop-ui/src/features/tray/components/Tray.tsx
git commit -m "refactor(desktop-ui): migrate Tray to useTauriQuery and add optimistic toggle"
```

---

### Task B3: Migrate `FocusControl.tsx`

**Files:**
- Modify: `desktop-ui/src/features/tray/components/FocusControl.tsx`

- [ ] **Step 1: Replace imports**

Find:
```ts
import { useTrayQuery } from "../hooks/useTrayQuery";
```
Replace with:
```ts
import { qk, useTauriQuery } from "@/lib/query";
```

- [ ] **Step 2: Replace the flashcard query**

Find:
```ts
const { data: dueCount } = useTrayQuery<number>(
	"flashcard_total_due",
	undefined,
	0,
);
```
Replace with:
```ts
const { data: dueCount } = useTauriQuery<number>({
	queryKey: qk.flashcards.dueCount(),
	command: "flashcard_total_due",
	fallback: 0,
});
```

- [ ] **Step 3: Typecheck**

```bash
cd desktop-ui && bunx tsc --noEmit 2>&1 | grep "FocusControl.tsx" | head -5
```

Expected: no output.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/tray/components/FocusControl.tsx
git commit -m "refactor(desktop-ui): migrate FocusControl flashcard query"
```

---

### Task B4: Delete obsolete hooks

**Files:**
- Delete: `desktop-ui/src/features/tray/hooks/useTrayQuery.ts`
- Delete: `desktop-ui/src/features/tray/hooks/useTrayMutation.ts`

- [ ] **Step 1: Confirm no remaining callers**

```bash
cd desktop-ui && grep -rE "useTrayQuery|useTrayMutation" src/
```

Expected: no output. If anything matches outside the two doomed files, fix it before proceeding.

- [ ] **Step 2: Delete the files**

```bash
rm desktop-ui/src/features/tray/hooks/useTrayQuery.ts
rm desktop-ui/src/features/tray/hooks/useTrayMutation.ts
```

- [ ] **Step 3: Typecheck**

```bash
cd desktop-ui && bunx tsc --noEmit 2>&1 | tail -10 && echo "---DONE---"
```

Expected: only `---DONE---`.

- [ ] **Step 4: Run all tests**

```bash
cd desktop-ui && bun run test
```

Expected: all green.

- [ ] **Step 5: Commit**

```bash
git add -A desktop-ui/src/features/tray/hooks/
git commit -m "chore(desktop-ui): remove obsolete useTrayQuery/useTrayMutation"
```

---

### Task B5: Integration test — multi-window invalidation

**Files:**
- Create: `desktop-ui/src/features/tray/tests/Tray.realtime.test.tsx`

This test is the proof. It renders `<Tray/>` inside a `<QueryProvider/>`, mocks `ipc` and `listen`, fires a fake `entity:updated{kind:"task"}` event, and verifies that `today_tasks` is re-fetched.

- [ ] **Step 1: Write the failing test**

Create `desktop-ui/src/features/tray/tests/Tray.realtime.test.tsx`:

```tsx
import { render, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { QueryProvider } from "@/lib/query";

type Handler = (payload: unknown) => void;

// Capture every listener so we can fire events from the test.
const subs = new Map<string, Handler>();
const fakeListen = vi.fn(async (event: string, handler: Handler) => {
	subs.set(event, handler);
	return () => subs.delete(event);
});
const fakeIpc = vi.fn();

vi.mock("@/utils/tauri-bridge", () => ({
	ipc: (...args: unknown[]) => fakeIpc(...args),
	isTauri: () => true,
	getCurrentWindow: () => ({
		label: "tray",
		hide: vi.fn(),
		show: vi.fn(),
		setFocus: vi.fn(),
	}),
	getWindowByLabel: () => ({
		label: "main",
		hide: vi.fn(),
		show: vi.fn(),
		setFocus: vi.fn(),
	}),
	emit: vi.fn(),
	listen: (...args: Parameters<typeof fakeListen>) => fakeListen(...args),
	currentWindowLabel: () => "tray",
}));

import { Tray } from "../components/Tray";

afterEach(() => {
	fakeIpc.mockReset();
	fakeListen.mockClear();
	subs.clear();
});

describe("Tray real-time", () => {
	it("refetches today_tasks when entity:updated{kind:'task'} fires", async () => {
		fakeIpc.mockImplementation(async (cmd: string) => {
			if (cmd === "today_tasks") return [];
			if (cmd === "productivity_calendar_events") return [];
			if (cmd === "focus_session_status")
				return { active: false, sync: null, session: null };
			if (cmd === "focus_today_sessions") return [];
			if (cmd === "flashcard_total_due") return 0;
			return null;
		});

		render(
			<QueryProvider>
				<Tray />
			</QueryProvider>,
		);

		// Wait for initial fetch.
		await waitFor(() =>
			expect(fakeIpc).toHaveBeenCalledWith("today_tasks", undefined),
		);
		const initialCallCount = fakeIpc.mock.calls.filter(
			([cmd]) => cmd === "today_tasks",
		).length;

		// Fire a fake event from "another window".
		const fire = subs.get("entity:updated");
		expect(fire).toBeDefined();
		fire?.({ entityKind: "task", id: "t1" });

		// today_tasks should refetch.
		await waitFor(() => {
			const after = fakeIpc.mock.calls.filter(
				([cmd]) => cmd === "today_tasks",
			).length;
			expect(after).toBeGreaterThan(initialCallCount);
		});
	});
});
```

- [ ] **Step 2: Run the test**

```bash
cd desktop-ui && bun run test src/features/tray/tests/Tray.realtime.test.tsx
```

Expected: 1 passing test.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/tray/tests/Tray.realtime.test.tsx
git commit -m "test(desktop-ui): verify tray refetches on entity:updated event"
```

---

### Task B6: Manual end-to-end verification

**Files:** none.

- [ ] **Step 1: Start dev**

```bash
cd desktop-ui && bun run dev
# new terminal:
cargo tauri dev
```

- [ ] **Step 2: Open both the main window and the tray**

Click the menu-bar icon to open the tray. Both windows are now visible.

- [ ] **Step 3: Note today's tasks shown in the tray**

Memorise the count and contents.

- [ ] **Step 4: Edit a task in the main window** — mark it complete via the main app's task UI (or create a new task).

- [ ] **Step 5: Switch focus to the tray (no clicking — just look)**

Expected: the tray's task list updates **without a manual refresh**. The completed task disappears from the active list (it's now done). New tasks appear automatically.

- [ ] **Step 6: Repeat the reverse**: toggle a task's checkbox in the tray.

Expected: the visual change in the tray is **immediate** (optimistic patch). The main window's task UI also updates within ~200ms (round-trip + invalidation).

- [ ] **Step 7: Open the React Query devtools in the tray**

Click the floating "TanStack" button. You should see queries `["tasks","today"]`, `["calendar","events","2026-04-26"]`, `["focus","status"]`, `["focus","todaySessions"]`, `["flashcards","dueCount"]`. Each shows its data, status, and last-updated timestamp.

- [ ] **Step 8: Trigger a stale state** — disconnect from Tauri (close the desktop process briefly with Cmd+Q, then reopen).

The tray's cache survives the reload because the QueryProvider lives in the in-process React tree; it's recreated on reload but the first re-fetch happens immediately.

- [ ] **Step 9: Final integrity commit if any cosmetic tweaks were necessary**

```bash
cd /Users/jayden/Projects/Klynt/bot && git status
```

If clean, no commit needed — the foundation + tray ship as-is.

---

## Self-Review

(Run after writing the plan. Findings reported below; fixes applied inline.)

**1. Spec coverage:**
- Foundation primitives (client, provider, bridge, query hook, mutation hook, key factory, entity map, barrel) → Tasks A2–A9 ✓
- DEV devtools mounted → Task A8 ✓
- Multi-window wiring → Task A10 ✓
- Tray migration end-to-end → Tasks B1–B4 ✓
- Real-time invalidation tested → Task B5 ✓
- Manual cross-window verification → Task B6 ✓
- Optimistic update on toggle → Task B2 step 4 ✓

**2. Placeholder scan:** No "TBD"/"TODO"/"add error handling" wording. All test code is concrete, all commands are real, all file paths are absolute.

**3. Type consistency:**
- `useTauriQuery` opts: `{ queryKey, command, args?, fallback?, enabled?, staleTime? }` — used identically in B1, B2, B3 ✓
- `useTauriMutation` opts: `{ command, invalidates?, optimistic?, onSuccess?, onError? }` — used identically in B1, B2 ✓
- `qk.tasks.today()`, `qk.calendar.eventsForDate(date)`, `qk.focus.status()`, `qk.focus.todaySessions()`, `qk.flashcards.dueCount()` — appear in queryKeys.ts (Task A3) and in callsites B1, B2, B3 ✓
- `EntityKind` strings (`"task"`, `"focusSession"`) match between entityKindMap.ts (A2) and tauriEventBridge.ts (A5) ✓

---

## Out-of-scope notes (Plans 2-4)

- Launcher / distraction / settings / git / threads / composer migrations: **Plan 2**.
- MCP cross-process bridge socket so Claude Code edits propagate live: **Plan 3**.
- Distiller domain events + `PRAGMA data_version` polling fallback: **Plan 4**.

---

## Definition of Done (Plan 1)

- All 11 + 6 = 17 tasks committed.
- `bun run test` green; new tests for `entityKindMap`, `queryKeys`, `client`, `tauriEventBridge`, `useTauriQuery`, `useTauriMutation`, `Tray.realtime` all passing.
- `bunx tsc --noEmit` clean.
- Manual verification (B6) confirms cross-window real-time.
- `useTrayQuery.ts` and `useTrayMutation.ts` are deleted.
- React Query devtools visible in dev for every webview.
