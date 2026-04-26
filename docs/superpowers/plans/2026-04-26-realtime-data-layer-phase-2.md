# Real-time Data Layer Phase 2 — Migrate Remaining FE Features

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Migrate every remaining `desktop-ui` feature off ad-hoc `useState + ipc()`/typed-wrapper patterns onto the `@/lib/query` foundation built in Plan 1. After this plan, every data-fetching surface — launcher (Dashboard, Search, DndActive, ExecuteItem, ActionMenu, FocusActiveChip), distraction overlay, settings (5 hooks), models/skills/apps/prompts registries, git panel (5 git hooks + 4 GitHub hooks + commit/actions controllers + panel controller), and tray-sync — participates in cross-window real-time invalidation.

**Architecture:** Extend `useTauriQuery`/`useTauriMutation` with an optional `queryFn`/`mutationFn` escape hatch so the codebase's typed `@services/tauri` wrappers can be used without round-tripping through string commands. New query keys land in `queryKeys.ts`; new event routes (chat, mcp, productivity, app-server) land in `tauriEventBridge.ts`. Existing `subscribeAppServerEvents` consumers become cache invalidators instead of `setState` callers, so the FE has one cache (TanStack Query) plus two invalidation channels (Tauri events for cross-window, app-server events for live backend deltas) that both converge on the same cache.

**Tech Stack:** Existing `@tanstack/react-query@^5` foundation. Vitest + `@testing-library/react`. Existing `@services/tauri` typed wrappers and `@services/events.subscribeAppServerEvents`.

**Master plan context:** Plan 2 of 4. Depends on Plan 1 (foundation + tray migrated, complete and verified). Plans 3–4 cover the Rust-side MCP cross-process bridge (Plan 3) and Distiller domain events (Plan 4).

**Self-imposed constraints (from CLAUDE.md):**
- TDD where it pays — pure helpers and primitives get tests; rewrites of existing hooks rely on the existing component tests + manual smoke tests.
- One concern per commit. Co-author trailer omitted (matches user's recent commit style).
- No back-compat shims — we delete the old `useState`/polling code and rely on the cache.
- `bunx tsc --noEmit` clean after every task.

---

## File Structure

### Files to extend (foundation, used by every later phase)

| Path | Change |
|---|---|
| `desktop-ui/src/lib/query/useTauriQuery.ts` | Add optional `queryFn` (replaces `command`+`args` if provided). |
| `desktop-ui/src/lib/query/useTauriMutation.ts` | Add optional `mutationFn` (replaces `command` for the network call). |
| `desktop-ui/src/lib/query/queryKeys.ts` | Add domains: `launcher`, `dashboard`, `settings`, `agents`, `models`, `skills`, `apps`, `prompts`, `git`, `github`, `threads`, `system`. |
| `desktop-ui/src/lib/query/tauriEventBridge.ts` | Add static routes for `chat:thread_*`, `chat:message_added`, `mcp:server_status`, `mcp:startup_complete`, `productivity:nudge`, `score:updated`, `bucket:completed`. Extend `focus:state_changed` route to also hit `qk.launcher.dndActive()`. |
| `desktop-ui/src/lib/query/index.ts` | Re-export the new types. |

### New files (cache adapter for app-server events)

| Path | Responsibility |
|---|---|
| `desktop-ui/src/lib/query/appServerEventBridge.ts` | Subscribes to `subscribeAppServerEvents`, routes `SkillsUpdateAvailable`, `AppListUpdated`, `ConfigChanged`, etc. to `qk.skills.list()` / `qk.apps.list()` / `qk.settings.app()` invalidations. |
| `desktop-ui/src/lib/query/tests/appServerEventBridge.test.ts` | Unit test. |

### Files to migrate (one task per file unless trivially identical)

**Phase B — launcher (5 files):**
- `desktop-ui/src/features/launcher/hooks/useDashboardData.ts`
- `desktop-ui/src/features/launcher/hooks/useDndActive.ts`
- `desktop-ui/src/features/launcher/hooks/useLauncherSearch.ts`
- `desktop-ui/src/features/launcher/components/ActionMenu.tsx`
- `desktop-ui/src/features/launcher/components/FocusActiveChip.tsx`

(`useExecuteItem.ts` is intentionally **deferred** — it's not a hook; see Task B6 for why and how.)

**Phase C — distraction (1 file):**
- `desktop-ui/src/features/distraction/components/DistractionOverlay.tsx`

**Phase D — settings (5 hooks):**
- `desktop-ui/src/features/settings/hooks/useAppSettings.ts`
- `desktop-ui/src/features/settings/hooks/useSettingsAgentsSection.ts`
- `desktop-ui/src/features/settings/hooks/useSettingsDefaultModels.ts`
- `desktop-ui/src/features/settings/hooks/useSettingsFeaturesSection.ts`
- `desktop-ui/src/features/settings/hooks/useSettingsServerSection.ts`

**Phase E — registries (4 hooks):**
- `desktop-ui/src/features/models/hooks/useModels.ts`
- `desktop-ui/src/features/skills/hooks/useSkills.ts`
- `desktop-ui/src/features/apps/hooks/useApps.ts`
- `desktop-ui/src/features/prompts/hooks/useCustomPrompts.ts`

**Phase F — git (12 files):**
- `desktop-ui/src/features/git/hooks/useGitStatus.ts`
- `desktop-ui/src/features/git/hooks/useGitBranches.ts`
- `desktop-ui/src/features/git/hooks/useGitDiffs.ts`
- `desktop-ui/src/features/git/hooks/useGitLog.ts`
- `desktop-ui/src/features/git/hooks/useGitRemote.ts`
- `desktop-ui/src/features/git/hooks/useGitActions.ts`
- `desktop-ui/src/features/git/hooks/useGitCommitDiffs.ts`
- `desktop-ui/src/features/git/hooks/useGitRepoScan.ts`
- `desktop-ui/src/features/git/hooks/useGitHubIssues.ts`
- `desktop-ui/src/features/git/hooks/useGitHubPullRequests.ts`
- `desktop-ui/src/features/git/hooks/useGitHubPullRequestDiffs.ts`
- `desktop-ui/src/features/git/hooks/useGitHubPullRequestComments.ts`
- `desktop-ui/src/features/app/hooks/useGitCommitController.ts`
- `desktop-ui/src/features/app/hooks/useGitHubPanelController.ts` (state aggregator — simplifies, doesn't change network calls)

**Phase G — composer + tray-sync + final sweep:**
- `desktop-ui/src/features/app/hooks/useTrayRecentThreads.ts`
- `desktop-ui/src/features/app/hooks/useTraySessionUsage.ts`
- Final regression sweep + manual cross-window verification.

---

## Phase A — Foundation extensions

### Task A1: Add `queryFn` escape hatch to `useTauriQuery`

**Context:** Settings/models/git hooks call typed wrappers like `getGitStatus(workspaceId)` from `@services/tauri`. Those wrappers internally call `invoke(...)`. To avoid duplicating type information by re-routing through string-named `ipc()`, we add an optional `queryFn` on `useTauriQuery`. If provided, it replaces the `command`/`args` path. Both must produce a `Promise<TData>`.

**Files:**
- Modify: `desktop-ui/src/lib/query/useTauriQuery.ts`
- Modify: `desktop-ui/src/lib/query/tests/useTauriQuery.test.tsx`

- [ ] **Step 1: Add a failing test for `queryFn`**

Append to `desktop-ui/src/lib/query/tests/useTauriQuery.test.tsx`:

```tsx
describe("useTauriQuery — queryFn escape hatch", () => {
	it("uses queryFn when provided and ignores command", async () => {
		const queryFn = vi.fn().mockResolvedValue({ id: 1, name: "x" });
		const client = new QueryClient({
			defaultOptions: { queries: { retry: 0 } },
		});

		const { result } = renderHook(
			() =>
				useTauriQuery({
					queryKey: ["custom", "thing"],
					queryFn,
				}),
			{ wrapper: wrapper(client) },
		);

		await waitFor(() =>
			expect(result.current.data).toEqual({ id: 1, name: "x" }),
		);
		expect(queryFn).toHaveBeenCalledTimes(1);
		expect(mockedIpc).not.toHaveBeenCalled();
	});

	it("throws if neither command nor queryFn is provided", () => {
		const client = new QueryClient({
			defaultOptions: { queries: { retry: 0 } },
		});
		expect(() =>
			renderHook(
				() =>
					useTauriQuery({
						queryKey: ["empty"],
					} as never),
				{ wrapper: wrapper(client) },
			),
		).toThrow(/command or queryFn/);
	});
});
```

- [ ] **Step 2: Run — expect FAIL**

```bash
cd desktop-ui && bun run test src/lib/query/tests/useTauriQuery.test.tsx
```

Expected: 2 new tests fail (`useTauriQuery queryFn escape hatch`).

- [ ] **Step 3: Update the implementation**

Replace `desktop-ui/src/lib/query/useTauriQuery.ts` entirely with:

```ts
import {
	type QueryKey,
	useQuery,
	type UseQueryResult,
} from "@tanstack/react-query";
import { ipc } from "@/utils/tauri-bridge";

export interface TauriQueryOptions<TData> {
	queryKey: QueryKey;
	/** Tauri command name. Mutually exclusive with `queryFn`. */
	command?: string;
	args?: Record<string, unknown>;
	/**
	 * Custom fetch function. Use this when the data source is a typed wrapper
	 * (e.g. `@services/tauri`'s `getGitStatus(workspaceId)`) rather than a
	 * string-named ipc command. Mutually exclusive with `command`.
	 */
	queryFn?: () => Promise<TData>;
	/** Returned as `data` until the first successful fetch. */
	fallback?: TData;
	enabled?: boolean;
	staleTime?: number;
}

export function useTauriQuery<TData>(
	opts: TauriQueryOptions<TData>,
): UseQueryResult<TData> & { data: TData } {
	if (!opts.command && !opts.queryFn) {
		throw new Error(
			"useTauriQuery: either `command` or `queryFn` must be provided",
		);
	}

	const result = useQuery<TData>({
		queryKey: opts.queryKey,
		queryFn: opts.queryFn ?? (() => ipc<TData>(opts.command!, opts.args)),
		enabled: opts.enabled,
		staleTime: opts.staleTime,
		placeholderData: opts.fallback as never,
	});

	return {
		...result,
		data: (result.data ?? opts.fallback) as TData,
	};
}
```

- [ ] **Step 4: Run — expect PASS**

```bash
cd desktop-ui && bun run test src/lib/query/tests/useTauriQuery.test.tsx
```

Expected: all 5 tests pass (3 original + 2 new).

- [ ] **Step 5: Typecheck**

```bash
cd desktop-ui && bunx tsc --noEmit 2>&1 | tail -5 && echo "---DONE---"
```

Expected: `---DONE---` only.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/lib/query/useTauriQuery.ts desktop-ui/src/lib/query/tests/useTauriQuery.test.tsx
git commit -m "feat(desktop-ui): add queryFn escape hatch to useTauriQuery"
```

---

### Task A2: Add `mutationFn` escape hatch to `useTauriMutation`

**Files:**
- Modify: `desktop-ui/src/lib/query/useTauriMutation.ts`
- Modify: `desktop-ui/src/lib/query/tests/useTauriMutation.test.tsx`

- [ ] **Step 1: Add a failing test**

Append to `desktop-ui/src/lib/query/tests/useTauriMutation.test.tsx`:

```tsx
describe("useTauriMutation — mutationFn escape hatch", () => {
	it("uses mutationFn when provided", async () => {
		const mutationFn = vi.fn().mockResolvedValue({ ok: true });
		const client = new QueryClient();

		const { result } = renderHook(
			() =>
				useTauriMutation<{ ok: boolean }, { name: string }>({
					mutationFn,
					invalidates: [],
				}),
			{ wrapper: wrap(client) },
		);

		await act(async () => {
			await result.current.mutate({ name: "abc" });
		});

		expect(mutationFn).toHaveBeenCalledWith({ name: "abc" });
		expect(mockedIpc).not.toHaveBeenCalled();
	});

	it("throws if neither command nor mutationFn is provided", () => {
		const client = new QueryClient();
		expect(() =>
			renderHook(() => useTauriMutation({} as never), {
				wrapper: wrap(client),
			}),
		).toThrow(/command or mutationFn/);
	});
});
```

- [ ] **Step 2: Run — expect FAIL**

```bash
cd desktop-ui && bun run test src/lib/query/tests/useTauriMutation.test.tsx
```

Expected: 2 new tests fail.

- [ ] **Step 3: Update the implementation**

Replace `desktop-ui/src/lib/query/useTauriMutation.ts` entirely with:

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
	/** Tauri command name. Mutually exclusive with `mutationFn`. */
	command?: string;
	/** Custom mutation function for typed-wrapper service calls. */
	mutationFn?: (vars: TVars) => Promise<TData>;
	invalidates?: QueryKey[];
	// biome-ignore lint/suspicious/noExplicitAny: TPrev is opaque
	optimistic?: OptimisticConfig<TVars, any>;
	onSuccess?: (data: TData, vars: TVars) => void;
	onError?: (error: unknown, vars: TVars) => void;
}

export function useTauriMutation<TData = unknown, TVars = void>(
	opts: TauriMutationOptions<TData, TVars>,
) {
	if (!opts.command && !opts.mutationFn) {
		throw new Error(
			"useTauriMutation: either `command` or `mutationFn` must be provided",
		);
	}
	const client = useQueryClient();

	const mutation = useMutation<
		TData,
		unknown,
		TVars,
		{ rollback?: () => void }
	>({
		mutationFn: (vars) =>
			opts.mutationFn
				? opts.mutationFn(vars)
				: ipc<TData>(
						opts.command!,
						vars as Record<string, unknown> | undefined,
					),

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
				for (const key of overrides) {
					client.invalidateQueries({ queryKey: key });
				}
				return;
			}
			if (!opts.command) return;
			const kind = entityKindForCommand(opts.command);
			if (kind) {
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

- [ ] **Step 4: Run — expect PASS**

```bash
cd desktop-ui && bun run test src/lib/query/tests/useTauriMutation.test.tsx
```

Expected: all 6 tests pass.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/lib/query/useTauriMutation.ts desktop-ui/src/lib/query/tests/useTauriMutation.test.tsx
git commit -m "feat(desktop-ui): add mutationFn escape hatch to useTauriMutation"
```

---

### Task A3: Extend `queryKeys.ts` with new domains

**Files:**
- Modify: `desktop-ui/src/lib/query/queryKeys.ts`
- Modify: `desktop-ui/src/lib/query/tests/queryKeys.test.ts`

- [ ] **Step 1: Add failing tests**

Append to `desktop-ui/src/lib/query/tests/queryKeys.test.ts`:

```ts
describe("queryKeys — phase 2 domains", () => {
	it("launcher.dashboard / search / dndActive", () => {
		expect(qk.launcher.dashboard()).toEqual(["launcher", "dashboard"]);
		expect(qk.launcher.search("hi")).toEqual(["launcher", "search", "hi"]);
		expect(qk.launcher.dndActive()).toEqual(["launcher", "dndActive"]);
	});
	it("settings keys", () => {
		expect(qk.settings.app()).toEqual(["settings", "app"]);
		expect(qk.settings.codexConfigPath()).toEqual([
			"settings",
			"codexConfigPath",
		]);
		expect(qk.settings.features("ws-1")).toEqual([
			"settings",
			"features",
			"ws-1",
		]);
		expect(qk.settings.tailscaleStatus()).toEqual([
			"settings",
			"tailscaleStatus",
		]);
		expect(qk.settings.tailscaleCommandPreview()).toEqual([
			"settings",
			"tailscaleCommandPreview",
		]);
		expect(qk.settings.tcpDaemonStatus()).toEqual([
			"settings",
			"tcpDaemonStatus",
		]);
		expect(qk.settings.workspaces()).toEqual(["settings", "workspaces"]);
	});
	it("agents keys", () => {
		expect(qk.agents.settings()).toEqual(["agents", "settings"]);
		expect(qk.agents.configToml("foo")).toEqual([
			"agents",
			"configToml",
			"foo",
		]);
	});
	it("models keys", () => {
		expect(qk.models.list("ws-1")).toEqual(["models", "list", "ws-1"]);
		expect(qk.models.configModel("ws-1")).toEqual([
			"models",
			"configModel",
			"ws-1",
		]);
	});
	it("registries", () => {
		expect(qk.skills.list("ws-1")).toEqual(["skills", "list", "ws-1"]);
		expect(qk.apps.list("ws-1", "thread-7")).toEqual([
			"apps",
			"list",
			"ws-1",
			"thread-7",
		]);
		expect(qk.prompts.list("ws-1")).toEqual(["prompts", "list", "ws-1"]);
	});
	it("git keys", () => {
		expect(qk.git.status("ws-1")).toEqual(["git", "status", "ws-1"]);
		expect(qk.git.branches("ws-1")).toEqual(["git", "branches", "ws-1"]);
		expect(qk.git.diffs("ws-1")).toEqual(["git", "diffs", "ws-1"]);
		expect(qk.git.log("ws-1")).toEqual(["git", "log", "ws-1"]);
		expect(qk.git.remote("ws-1")).toEqual(["git", "remote", "ws-1"]);
		expect(qk.git.commitDiffs("ws-1", "abc")).toEqual([
			"git",
			"commitDiffs",
			"ws-1",
			"abc",
		]);
		expect(qk.git.repoScan("ws-1", 2)).toEqual([
			"git",
			"repoScan",
			"ws-1",
			2,
		]);
	});
	it("github keys", () => {
		expect(qk.github.issues("ws-1")).toEqual(["github", "issues", "ws-1"]);
		expect(qk.github.pulls("ws-1")).toEqual(["github", "pulls", "ws-1"]);
		expect(qk.github.diffsForPr("ws-1", 42)).toEqual([
			"github",
			"pulls",
			"ws-1",
			42,
			"diffs",
		]);
		expect(qk.github.commentsForPr("ws-1", 42)).toEqual([
			"github",
			"pulls",
			"ws-1",
			42,
			"comments",
		]);
	});
	it("threads / system", () => {
		expect(qk.threads.list()).toEqual(["threads", "list"]);
		expect(qk.threads.byId("abc")).toEqual(["threads", "byId", "abc"]);
		expect(qk.system.mcpServers()).toEqual(["system", "mcpServers"]);
	});
});
```

- [ ] **Step 2: Run — expect FAIL**

```bash
cd desktop-ui && bun run test src/lib/query/tests/queryKeys.test.ts
```

Expected: new tests fail.

- [ ] **Step 3: Extend the factory**

Open `desktop-ui/src/lib/query/queryKeys.ts`. Inside the `qk` object literal, after the existing `flashcards` entry and before the closing `} as const`, add:

```ts
	launcher: {
		all: () => ["launcher"] as const,
		dashboard: () => ["launcher", "dashboard"] as const,
		search: (query: string) => ["launcher", "search", query] as const,
		dndActive: () => ["launcher", "dndActive"] as const,
	},
	settings: {
		all: () => ["settings"] as const,
		app: () => ["settings", "app"] as const,
		codexConfigPath: () => ["settings", "codexConfigPath"] as const,
		features: (workspaceId: string | null) =>
			["settings", "features", workspaceId ?? "global"] as const,
		tailscaleStatus: () => ["settings", "tailscaleStatus"] as const,
		tailscaleCommandPreview: () =>
			["settings", "tailscaleCommandPreview"] as const,
		tcpDaemonStatus: () => ["settings", "tcpDaemonStatus"] as const,
		workspaces: () => ["settings", "workspaces"] as const,
	},
	agents: {
		all: () => ["agents"] as const,
		settings: () => ["agents", "settings"] as const,
		configToml: (agentName: string) =>
			["agents", "configToml", agentName] as const,
	},
	models: {
		all: () => ["models"] as const,
		list: (workspaceId: string) => ["models", "list", workspaceId] as const,
		configModel: (workspaceId: string) =>
			["models", "configModel", workspaceId] as const,
	},
	skills: {
		all: () => ["skills"] as const,
		list: (workspaceId: string) => ["skills", "list", workspaceId] as const,
	},
	apps: {
		all: () => ["apps"] as const,
		list: (workspaceId: string, threadId: string | null) =>
			["apps", "list", workspaceId, threadId ?? "no-thread"] as const,
	},
	prompts: {
		all: () => ["prompts"] as const,
		list: (workspaceId: string) => ["prompts", "list", workspaceId] as const,
	},
	git: {
		all: () => ["git"] as const,
		status: (workspaceId: string) => ["git", "status", workspaceId] as const,
		branches: (workspaceId: string) =>
			["git", "branches", workspaceId] as const,
		diffs: (workspaceId: string) => ["git", "diffs", workspaceId] as const,
		log: (workspaceId: string) => ["git", "log", workspaceId] as const,
		remote: (workspaceId: string) => ["git", "remote", workspaceId] as const,
		commitDiffs: (workspaceId: string, sha: string) =>
			["git", "commitDiffs", workspaceId, sha] as const,
		repoScan: (workspaceId: string, depth: number) =>
			["git", "repoScan", workspaceId, depth] as const,
	},
	github: {
		all: () => ["github"] as const,
		issues: (workspaceId: string) =>
			["github", "issues", workspaceId] as const,
		pulls: (workspaceId: string) => ["github", "pulls", workspaceId] as const,
		diffsForPr: (workspaceId: string, n: number) =>
			["github", "pulls", workspaceId, n, "diffs"] as const,
		commentsForPr: (workspaceId: string, n: number) =>
			["github", "pulls", workspaceId, n, "comments"] as const,
	},
	threads: {
		all: () => ["threads"] as const,
		list: () => ["threads", "list"] as const,
		byId: (id: string) => ["threads", "byId", id] as const,
	},
	system: {
		all: () => ["system"] as const,
		mcpServers: () => ["system", "mcpServers"] as const,
	},
```

- [ ] **Step 4: Run — expect PASS**

```bash
cd desktop-ui && bun run test src/lib/query/tests/queryKeys.test.ts
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/lib/query/queryKeys.ts desktop-ui/src/lib/query/tests/queryKeys.test.ts
git commit -m "feat(desktop-ui): extend queryKeys with launcher/settings/git/registries"
```

---

### Task A4: Extend `tauriEventBridge.ts` with new static routes

**Files:**
- Modify: `desktop-ui/src/lib/query/tauriEventBridge.ts`
- Modify: `desktop-ui/src/lib/query/tests/tauriEventBridge.test.ts`

- [ ] **Step 1: Add failing tests**

Append to `desktop-ui/src/lib/query/tests/tauriEventBridge.test.ts`:

```ts
it("chat:thread_created invalidates threads.list", async () => {
	const client = new QueryClient();
	const spy = vi.spyOn(client, "invalidateQueries");
	const { listen, fire } = fakeListenFactory();
	const stop = await startTauriEventBridge(client, listen);
	fire("chat:thread_created", { id: "t1" });
	expect(spy).toHaveBeenCalledWith({ queryKey: qk.threads.list() });
	stop();
});

it("chat:thread_updated invalidates threads.list", async () => {
	const client = new QueryClient();
	const spy = vi.spyOn(client, "invalidateQueries");
	const { listen, fire } = fakeListenFactory();
	const stop = await startTauriEventBridge(client, listen);
	fire("chat:thread_updated", { id: "t1" });
	expect(spy).toHaveBeenCalledWith({ queryKey: qk.threads.list() });
	stop();
});

it("chat:message_added invalidates threads.list", async () => {
	const client = new QueryClient();
	const spy = vi.spyOn(client, "invalidateQueries");
	const { listen, fire } = fakeListenFactory();
	const stop = await startTauriEventBridge(client, listen);
	fire("chat:message_added", { sessionKey: "s1" });
	expect(spy).toHaveBeenCalledWith({ queryKey: qk.threads.list() });
	stop();
});

it("mcp:server_status invalidates system.mcpServers", async () => {
	const client = new QueryClient();
	const spy = vi.spyOn(client, "invalidateQueries");
	const { listen, fire } = fakeListenFactory();
	const stop = await startTauriEventBridge(client, listen);
	fire("mcp:server_status", { serverName: "x", status: "ready" });
	expect(spy).toHaveBeenCalledWith({ queryKey: qk.system.mcpServers() });
	stop();
});

it("mcp:startup_complete invalidates system.mcpServers", async () => {
	const client = new QueryClient();
	const spy = vi.spyOn(client, "invalidateQueries");
	const { listen, fire } = fakeListenFactory();
	const stop = await startTauriEventBridge(client, listen);
	fire("mcp:startup_complete", {});
	expect(spy).toHaveBeenCalledWith({ queryKey: qk.system.mcpServers() });
	stop();
});

it("score:updated invalidates launcher.dashboard", async () => {
	const client = new QueryClient();
	const spy = vi.spyOn(client, "invalidateQueries");
	const { listen, fire } = fakeListenFactory();
	const stop = await startTauriEventBridge(client, listen);
	fire("score:updated", { score: 0.8 });
	expect(spy).toHaveBeenCalledWith({ queryKey: qk.launcher.dashboard() });
	stop();
});

it("bucket:completed invalidates launcher.dashboard", async () => {
	const client = new QueryClient();
	const spy = vi.spyOn(client, "invalidateQueries");
	const { listen, fire } = fakeListenFactory();
	const stop = await startTauriEventBridge(client, listen);
	fire("bucket:completed", { bucket: "x" });
	expect(spy).toHaveBeenCalledWith({ queryKey: qk.launcher.dashboard() });
	stop();
});

it("focus:state_changed invalidates dndActive too", async () => {
	const client = new QueryClient();
	const spy = vi.spyOn(client, "invalidateQueries");
	const { listen, fire } = fakeListenFactory();
	const stop = await startTauriEventBridge(client, listen);
	fire("focus:state_changed", { state: "active" });
	expect(spy).toHaveBeenCalledWith({ queryKey: qk.launcher.dndActive() });
	stop();
});
```

- [ ] **Step 2: Run — expect FAIL**

```bash
cd desktop-ui && bun run test src/lib/query/tests/tauriEventBridge.test.ts
```

Expected: 8 new tests fail.

- [ ] **Step 3: Replace `STATIC_ROUTES` in the bridge**

Open `desktop-ui/src/lib/query/tauriEventBridge.ts`. Replace the `STATIC_ROUTES` constant with:

```ts
const STATIC_ROUTES: ReadonlyArray<readonly [string, QueryKey[]]> = [
	["focus:state_changed", [qk.focus.status(), qk.launcher.dndActive()]],
	["focus:phase_changed", [qk.focus.status()]],
	["focus:sync", [qk.focus.status()]],
	["chat:thread_created", [qk.threads.list()]],
	["chat:thread_updated", [qk.threads.list()]],
	["chat:message_added", [qk.threads.list()]],
	["mcp:server_status", [qk.system.mcpServers()]],
	["mcp:startup_complete", [qk.system.mcpServers()]],
	["productivity:nudge", [qk.launcher.dashboard()]],
	["score:updated", [qk.launcher.dashboard()]],
	["bucket:completed", [qk.launcher.dashboard()]],
];
```

- [ ] **Step 4: Run — expect PASS**

```bash
cd desktop-ui && bun run test src/lib/query/tests/tauriEventBridge.test.ts
```

Expected: all tests pass (original 4 + 8 new = 12).

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/lib/query/tauriEventBridge.ts desktop-ui/src/lib/query/tests/tauriEventBridge.test.ts
git commit -m "feat(desktop-ui): route chat/mcp/productivity events to query invalidations"
```

---

### Task A5: Add `appServerEventBridge.ts` for backend WebSocket events

**Context:** `subscribeAppServerEvents` (in `@services/events`) is the FE's existing WebSocket subscription to the backend's app-server. Several hooks (`useSkills`, `useApps`) already listen to events like `SkillsUpdateAvailable`. Instead of each hook independently re-fetching, route those events through one bridge that invalidates the matching cache keys.

**Files:**
- Create: `desktop-ui/src/lib/query/appServerEventBridge.ts`
- Create: `desktop-ui/src/lib/query/tests/appServerEventBridge.test.ts`
- Modify: `desktop-ui/src/lib/query/QueryProvider.tsx` (start the bridge alongside the Tauri one)
- Modify: `desktop-ui/src/lib/query/index.ts` (export)

- [ ] **Step 1: Read the helpers we'll wrap**

```bash
cd desktop-ui && grep -n "isSkillsUpdateAvailable\|isAppListUpdated\|isConfigChanged" src/utils/appServerEvents.ts
```

Note the function names — they're predicate guards over the event payload. Capture exact spellings before writing code.

- [ ] **Step 2: Write the failing test**

Create `desktop-ui/src/lib/query/tests/appServerEventBridge.test.ts`:

```ts
import { QueryClient } from "@tanstack/react-query";
import { describe, expect, it, vi } from "vitest";

vi.mock("@/services/events", () => ({
	subscribeAppServerEvents: vi.fn(),
}));

import { subscribeAppServerEvents } from "@/services/events";
import { startAppServerEventBridge } from "../appServerEventBridge";
import { qk } from "../queryKeys";

const mockedSubscribe = vi.mocked(subscribeAppServerEvents);

function makeFakeSub() {
	let handler: (e: unknown) => void = () => {};
	const unsubscribe = vi.fn();
	mockedSubscribe.mockImplementation((h: (e: unknown) => void) => {
		handler = h;
		return unsubscribe;
	});
	return { fire: (e: unknown) => handler(e), unsubscribe };
}

describe("appServerEventBridge", () => {
	it("invalidates skills.list when SkillsUpdateAvailable fires", () => {
		const { fire } = makeFakeSub();
		const client = new QueryClient();
		const spy = vi.spyOn(client, "invalidateQueries");
		startAppServerEventBridge(client);
		fire({ type: "SkillsUpdateAvailable", workspaceId: "ws-1" });
		expect(spy).toHaveBeenCalledWith({
			queryKey: qk.skills.list("ws-1"),
		});
	});

	it("invalidates apps.list (broad) when AppListUpdated fires", () => {
		const { fire } = makeFakeSub();
		const client = new QueryClient();
		const spy = vi.spyOn(client, "invalidateQueries");
		startAppServerEventBridge(client);
		fire({ type: "AppListUpdated", workspaceId: "ws-1" });
		// Broad invalidation: prefix `["apps","list","ws-1"]` covers any threadId.
		expect(spy).toHaveBeenCalledWith({
			queryKey: ["apps", "list", "ws-1"],
		});
	});

	it("returns a stop function that unsubscribes", () => {
		const { unsubscribe } = makeFakeSub();
		const client = new QueryClient();
		const stop = startAppServerEventBridge(client);
		stop();
		expect(unsubscribe).toHaveBeenCalled();
	});
});
```

- [ ] **Step 3: Run — expect FAIL (module not found)**

```bash
cd desktop-ui && bun run test src/lib/query/tests/appServerEventBridge.test.ts
```

- [ ] **Step 4: Implement**

Create `desktop-ui/src/lib/query/appServerEventBridge.ts`:

```ts
import type { QueryClient } from "@tanstack/react-query";
import { subscribeAppServerEvents } from "@/services/events";
import { qk } from "./queryKeys";

interface AppServerEventLike {
	type: string;
	workspaceId?: string;
}

export function startAppServerEventBridge(client: QueryClient): () => void {
	const unsubscribe = subscribeAppServerEvents((eventRaw) => {
		const event = eventRaw as AppServerEventLike;
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
```

- [ ] **Step 5: Run — expect PASS**

```bash
cd desktop-ui && bun run test src/lib/query/tests/appServerEventBridge.test.ts
```

Expected: 3 passing tests.

- [ ] **Step 6: Wire into `QueryProvider`**

Open `desktop-ui/src/lib/query/QueryProvider.tsx`. Replace the single-bridge `useEffect` with:

```tsx
useEffect(() => {
	let stopTauri: (() => void) | null = null;
	let stopApp: (() => void) | null = null;
	let cancelled = false;

	startTauriEventBridge(clientRef.current!).then((s) => {
		if (cancelled) s();
		else stopTauri = s;
	});
	stopApp = startAppServerEventBridge(clientRef.current!);

	return () => {
		cancelled = true;
		stopTauri?.();
		stopApp?.();
	};
}, []);
```

Add the import at the top:

```tsx
import { startAppServerEventBridge } from "./appServerEventBridge";
```

- [ ] **Step 7: Export from barrel**

Edit `desktop-ui/src/lib/query/index.ts`. Append:

```ts
export { startAppServerEventBridge } from "./appServerEventBridge";
```

- [ ] **Step 8: Typecheck + commit**

```bash
cd desktop-ui && bunx tsc --noEmit 2>&1 | tail -5 && echo "---DONE---"
git add desktop-ui/src/lib/query/appServerEventBridge.ts desktop-ui/src/lib/query/tests/appServerEventBridge.test.ts desktop-ui/src/lib/query/QueryProvider.tsx desktop-ui/src/lib/query/index.ts
git commit -m "feat(desktop-ui): route app-server WebSocket events to query invalidations"
```

---

## Phase B — Launcher migration

### Task B1: Migrate `useDashboardData.ts`

**Current state (from inventory):** Polls `ipc("launcher_dashboard")` every 30s when `mode === "dashboard"`, writes the result through `useLauncherApi().setDashboard`. Now obsolete: the bridge invalidates `qk.launcher.dashboard()` on `score:updated` / `bucket:completed` / `productivity:nudge`.

**Files:**
- Modify: `desktop-ui/src/features/launcher/hooks/useDashboardData.ts`
- Modify: `desktop-ui/src/features/launcher/store.tsx` (remove `setDashboard` from API + reducer)
- Modify: any caller that reads `state.dashboard` (Launcher.tsx)

- [ ] **Step 1: Find every reader of `state.dashboard`**

```bash
cd desktop-ui && grep -rn "state.dashboard\|s\.dashboard\|\.dashboard\b" src/features/launcher
```

Capture the list.

- [ ] **Step 2: Rewrite the hook**

Replace `desktop-ui/src/features/launcher/hooks/useDashboardData.ts` entirely with:

```ts
import { qk, useTauriQuery } from "@/lib/query";
import { useLauncherState } from "../store";
import type { DashboardData } from "../types";

export function useDashboardData() {
	const mode = useLauncherState((s) => s.mode);
	return useTauriQuery<DashboardData | null>({
		queryKey: qk.launcher.dashboard(),
		command: "launcher_dashboard",
		fallback: null,
		enabled: mode === "dashboard",
	});
}
```

- [ ] **Step 3: Drop `dashboard` from the launcher store**

Open `desktop-ui/src/features/launcher/store.tsx`. In the `State` type, delete the line `dashboard: DashboardData | null;`. In the `LauncherStoreApi` interface, delete `setDashboard`. In the reducer's initial state, delete the `dashboard: null,` line. In whatever `case` handles `setDashboard`, delete it.

- [ ] **Step 4: Update readers**

For every grep-hit in step 1, replace `state.dashboard` reads with `useDashboardData().data`. The component that previously called `useDashboardData()` (likely `Launcher.tsx` or a child) now both invokes the hook AND reads its `.data` (instead of two separate calls).

Pattern:
```tsx
// Before:
useDashboardData(); // fire-and-forget
const dashboard = useLauncherState((s) => s.dashboard);

// After:
const { data: dashboard } = useDashboardData();
```

- [ ] **Step 5: Typecheck**

```bash
cd desktop-ui && bunx tsc --noEmit 2>&1 | grep -E "Dashboard|launcher" | head -20
```

Expected: no errors. If there are errors about `setDashboard` not on type, fix the remaining caller.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/launcher
git commit -m "refactor(desktop-ui): migrate useDashboardData to useTauriQuery"
```

---

### Task B2: Migrate `useDndActive.ts`

**Current state:** 2-second polling of `ipc("focus_active", { mode: "dnd" })`. Now obsolete: the `focus:state_changed` route in A4 invalidates `qk.launcher.dndActive()`.

**Files:**
- Modify: `desktop-ui/src/features/launcher/hooks/useDndActive.ts`

- [ ] **Step 1: Rewrite**

Replace the file entirely with:

```ts
import { qk, useTauriQuery } from "@/lib/query";
import type { FocusSession } from "../types";

export interface DndActiveResult {
	data: FocusSession | null;
	refetch: () => void;
}

export function useDndActive(): DndActiveResult {
	const query = useTauriQuery<FocusSession | null>({
		queryKey: qk.launcher.dndActive(),
		command: "focus_active",
		args: { mode: "dnd" },
		fallback: null,
	});
	return {
		data: query.data,
		refetch: () => {
			query.refetch();
		},
	};
}
```

- [ ] **Step 2: Typecheck**

```bash
cd desktop-ui && bunx tsc --noEmit 2>&1 | grep useDndActive | head -5
```

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/launcher/hooks/useDndActive.ts
git commit -m "refactor(desktop-ui): migrate useDndActive to useTauriQuery"
```

---

### Task B3: Migrate `useLauncherSearch.ts`

**Current state:** Custom debounce (30ms) + version counter to ignore stale responses. TanStack Query handles request dedup and abort automatically when the query key changes.

**Files:**
- Modify: `desktop-ui/src/features/launcher/hooks/useLauncherSearch.ts`

- [ ] **Step 1: Rewrite**

Replace the file entirely with:

```ts
import { useEffect } from "react";
import { qk, useTauriQuery } from "@/lib/query";
import { useLauncherApi, useLauncherState } from "../store";
import type { LauncherItem } from "../types";

const DEBOUNCE_MS = 30;

export function useLauncherSearch() {
	const query = useLauncherState((s) => s.query);
	const { setResults, setIsSearching } = useLauncherApi();

	// Debounce the raw query so we don't fire 1 query per keystroke. The
	// queryKey change cancels the in-flight TQ fetch automatically.
	const debounced = useDebounced(query, DEBOUNCE_MS);

	const search = useTauriQuery<LauncherItem[]>({
		queryKey: qk.launcher.search(debounced),
		command: "launcher_search",
		args: { query: debounced },
		fallback: [],
		// Search results are inherently stale-fast; tighter than the global
		// 30s default so an exact-string repeat within ~5s reuses the cache.
		staleTime: 5_000,
	});

	useEffect(() => {
		setIsSearching(search.isFetching);
	}, [search.isFetching, setIsSearching]);

	useEffect(() => {
		if (search.data) setResults(search.data);
	}, [search.data, setResults]);
}

function useDebounced<T>(value: T, ms: number): T {
	const [v, setV] = useStateInit(value);
	useEffect(() => {
		const t = setTimeout(() => setV(value), ms);
		return () => clearTimeout(t);
	}, [value, ms]);
	return v;
}

import { useState as useStateInit } from "react";
```

(Note: the `import { useState as useStateInit } from "react";` line at the bottom is intentional — keeps the helper self-contained at the bottom of the file. Move it to the top with the other imports if you prefer.)

- [ ] **Step 2: Cleanup imports**

Move the trailing `useState` import to the top:

```ts
import { useEffect, useState } from "react";
```

And in `useDebounced`, change `useStateInit` back to `useState`. Final file should have one `useState` import at the top.

- [ ] **Step 3: Typecheck**

```bash
cd desktop-ui && bunx tsc --noEmit 2>&1 | grep useLauncherSearch | head -5
```

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/launcher/hooks/useLauncherSearch.ts
git commit -m "refactor(desktop-ui): migrate useLauncherSearch to useTauriQuery"
```

---

### Task B4: Migrate `FocusActiveChip.tsx`

**Current state (inventory):** Calls `ipc("focus_extend", { mode: "dnd", newEndsAt })` and `ipc("focus_deactivate", { mode: "dnd" })` directly inside button handlers.

**Files:**
- Modify: `desktop-ui/src/features/launcher/components/FocusActiveChip.tsx`

- [ ] **Step 1: Read the file to confirm exact handler shapes**

```bash
cd desktop-ui && cat src/features/launcher/components/FocusActiveChip.tsx
```

- [ ] **Step 2: Hoist mutations to the top of the component**

Add the import:
```tsx
import { useTauriMutation } from "@/lib/query";
```

After `function FocusActiveChip({ endsAt, onDone }: Props) {`, add:

```tsx
const focusExtend = useTauriMutation<void, { mode: "dnd"; newEndsAt: string }>({
	command: "focus_extend",
});
const focusDeactivate = useTauriMutation<void, { mode: "dnd" }>({
	command: "focus_deactivate",
});
```

Replace each `await ipc("focus_extend", { mode: "dnd", newEndsAt })` with `await focusExtend.mutate({ mode: "dnd", newEndsAt })`, and `await ipc("focus_deactivate", { mode: "dnd" })` with `await focusDeactivate.mutate({ mode: "dnd" })`.

Remove the now-unused `import { ipc } from "@/utils/tauri-bridge";` if no other lines use it.

- [ ] **Step 3: Typecheck**

```bash
cd desktop-ui && bunx tsc --noEmit 2>&1 | grep FocusActiveChip | head -5
```

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/launcher/components/FocusActiveChip.tsx
git commit -m "refactor(desktop-ui): hoist FocusActiveChip ipc to useTauriMutation"
```

---

### Task B5: Migrate `ActionMenu.tsx`

**Current state (inventory):** Calls `ipc("launcher_open_app", { path })` inside an `openPath` helper used by multiple action builders. No data fetching.

**Files:**
- Modify: `desktop-ui/src/features/launcher/components/ActionMenu.tsx`

- [ ] **Step 1: Read the file to find every `ipc(` call**

```bash
cd desktop-ui && grep -n "ipc(" src/features/launcher/components/ActionMenu.tsx
```

- [ ] **Step 2: Hoist a single mutation**

Import:
```tsx
import { useTauriMutation } from "@/lib/query";
```

At the top of the component:
```tsx
const launcherOpenApp = useTauriMutation<void, { path: string }>({
	command: "launcher_open_app",
	invalidates: [], // pure side effect, nothing to refetch
});
```

Replace every `ipc("launcher_open_app", { path })` with `launcherOpenApp.mutate({ path })`. (Note: the original may use `await ipc(...)`; if so, use `await launcherOpenApp.mutate({ path })`.)

Remove unused `import { ipc } from "@/utils/tauri-bridge";` if no other lines use it.

- [ ] **Step 3: Typecheck**

```bash
cd desktop-ui && bunx tsc --noEmit 2>&1 | grep ActionMenu | head -5
```

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/launcher/components/ActionMenu.tsx
git commit -m "refactor(desktop-ui): hoist ActionMenu ipc to useTauriMutation"
```

---

### Task B6: `useExecuteItem` — defer or factor

**Important:** From the inventory, `useExecuteItem.ts` does NOT export a hook. It exports a plain function `executeItem(item, options, args)` that takes the store API and dispatches `ipc()` calls based on `item.kind`. It cannot use React hooks.

There are two valid responses; pick one explicitly:

**Option A (deferred):** leave `useExecuteItem.ts` as-is. The launcher's mutations are pure side effects (open app, run script, paste clipboard) — they don't drive cache state, so auto-invalidation isn't needed. The function being non-reactive is fine; the `ipc()` primitive still works.

**Option B (factored):** create a `useLauncherActions()` hook that exposes typed `mutate*` functions, and pass it through `ExecuteItemOptions`. Each `ipc("foo", args)` becomes `options.actions.foo.mutate(args)`. This unlocks per-action loading/error states and auto-invalidation if any of the actions ever start mutating cached entities.

- [ ] **Step 1: Pick the option**

For this plan, **choose Option A**. Rationale: the launcher actions are end-state side effects (open app, paste, focus a window). None of them mutate data the cache holds. Re-routing them through TanStack adds noise without benefit. Document this decision in a one-line comment at the top of `useExecuteItem.ts`:

```ts
// `executeItem` invokes Tauri commands that are pure side effects (open
// app, paste clipboard, focus window). It deliberately does not go through
// useTauriMutation because there's no cache state to invalidate; promoting
// it would just add a layer for no benefit.
```

- [ ] **Step 2: Commit the comment**

```bash
git add desktop-ui/src/features/launcher/hooks/useExecuteItem.ts
git commit -m "docs(desktop-ui): document why useExecuteItem stays on raw ipc"
```

(If the team later decides to switch to Option B, that'd be a follow-up plan.)

---

## Phase C — Distraction overlay

### Task C1: Migrate `DistractionOverlay.tsx`

**Current state (inventory):** Three direct `ipc()` calls — `distraction_dismiss`, `distraction_allow_temp`, `distraction_allow_session`. Three local `useState`s for transient overlay state (`intervention`, `verdict`, `loading`) driven by `useEvent` subscriptions. The `useState`s should stay (transient UI state, not cache); only the mutations migrate.

**Files:**
- Modify: `desktop-ui/src/features/distraction/components/DistractionOverlay.tsx`

- [ ] **Step 1: Read the current file**

```bash
cd desktop-ui && cat src/features/distraction/components/DistractionOverlay.tsx
```

Note the exact handler names (likely `handleAllow`, `handleAllowSession`, `handleDismiss` or similar).

- [ ] **Step 2: Add imports**

At the top:
```tsx
import { useTauriMutation } from "@/lib/query";
```

- [ ] **Step 3: Declare three mutations near the top of the component**

```tsx
const dismiss = useTauriMutation<void, { appName: string }>({
	command: "distraction_dismiss",
	invalidates: [],
});
const allowTemp = useTauriMutation<void, { pattern: string }>({
	command: "distraction_allow_temp",
	invalidates: [],
});
const allowSession = useTauriMutation<
	void,
	{ appName: string; windowTitle: string | null; classification: string }
>({
	command: "distraction_allow_session",
	invalidates: [],
});
```

`invalidates: []` is explicit because `distraction_*` doesn't match any entity prefix; without the override, the mutation hook would no-op silently — which is correct, but explicit is better.

- [ ] **Step 4: Replace each `ipc(...)` call**

For each `await ipc("distraction_dismiss", { appName })` → `await dismiss.mutate({ appName })`. Same pattern for `allow_temp` and `allow_session`.

Remove the now-unused `import { ipc } from "@/utils/tauri-bridge";` if not used elsewhere.

- [ ] **Step 5: Typecheck**

```bash
cd desktop-ui && bunx tsc --noEmit 2>&1 | grep DistractionOverlay | head -5
```

- [ ] **Step 6: Manual smoke test (optional but recommended)**

```bash
cd desktop-ui && bun run dev &
cargo tauri dev
```

Open distraction overlay (trigger via the BE — or skip and rely on existing component tests). Click each button. Verify the BE handlers still fire (check terminal logs or BE state).

- [ ] **Step 7: Commit**

```bash
git add desktop-ui/src/features/distraction/components/DistractionOverlay.tsx
git commit -m "refactor(desktop-ui): migrate distraction actions to useTauriMutation"
```

---

## Phase D — Settings hooks

### Task D1: Migrate `useAppSettings.ts`

**Current state (inventory):** A 200-line hook that loads `getAppSettings()`, normalizes it through `normalizeAppSettings()` + `buildDefaultSettings()`, exposes `setSettings`/`saveSettings`/`doctor`/`isLoading`. The normalize/default logic stays untouched; only the load/save plumbing migrates.

**Files:**
- Modify: `desktop-ui/src/features/settings/hooks/useAppSettings.ts`

- [ ] **Step 1: Read the file to capture exact import list**

```bash
cd desktop-ui && head -25 src/features/settings/hooks/useAppSettings.ts
```

Confirm the imports: `getAppSettings`, `runCodexDoctor`, `updateAppSettings` from `@services/tauri`.

- [ ] **Step 2: Replace the bottom of the file (the hook itself)**

Find the `export function useAppSettings()` block. Replace **only that function** with:

```ts
export function useAppSettings() {
	const defaultSettings = useMemo(() => buildDefaultSettings(), []);

	const query = useTauriQuery<AppSettings>({
		queryKey: qk.settings.app(),
		queryFn: async () => {
			try {
				const response = await getAppSettings();
				return normalizeAppSettings({
					...defaultSettings,
					...response,
				});
			} catch {
				// Fall back to defaults if loading settings fails.
				return defaultSettings;
			}
		},
		fallback: defaultSettings,
	});

	const save = useTauriMutation<AppSettings, AppSettings>({
		mutationFn: async (next) => {
			const normalized = normalizeAppSettings(next);
			const saved = await updateAppSettings(normalized);
			return normalizeAppSettings({
				...defaultSettings,
				...saved,
			});
		},
		invalidates: [qk.settings.app()],
	});

	const setSettings = useCallback(
		(updater: AppSettings | ((prev: AppSettings) => AppSettings)) => {
			const queryClient = save; // placeholder — actual setQueryData below
			const prev = query.data;
			const next =
				typeof updater === "function"
					? (updater as (p: AppSettings) => AppSettings)(prev)
					: updater;
			// Optimistic local update — same UX the original useState provided.
			// The save mutation will overwrite via invalidation when it settles.
			queryClient; // silence unused
			void next;
		},
		[query.data, save],
	);

	const doctor = useCallback(
		async (codexBin: string | null, codexArgs: string | null) => {
			return runCodexDoctor(codexBin, codexArgs);
		},
		[],
	);

	return {
		settings: query.data,
		setSettings,
		saveSettings: save.mutate,
		doctor,
		isLoading: query.isLoading,
	};
}
```

**This `setSettings` placeholder is wrong** — fix it in step 3.

- [ ] **Step 3: Implement `setSettings` against the cache**

Real-life many callers of `useAppSettings` mutate locally before saving (controlled inputs). Use `queryClient.setQueryData` for that local optimistic state.

Replace the `setSettings` block with:

```ts
import { useQueryClient } from "@tanstack/react-query";
// ... at top of file

const queryClient = useQueryClient();
const setSettings = useCallback(
	(updater: AppSettings | ((prev: AppSettings) => AppSettings)) => {
		queryClient.setQueryData<AppSettings>(qk.settings.app(), (prev) => {
			const base = prev ?? defaultSettings;
			return typeof updater === "function"
				? (updater as (p: AppSettings) => AppSettings)(base)
				: updater;
		});
	},
	[queryClient, defaultSettings],
);
```

Remove the `queryClient = save;` placeholder lines.

- [ ] **Step 4: Update imports at the top**

Add:
```ts
import { qk, useTauriMutation, useTauriQuery } from "@/lib/query";
import { useQueryClient } from "@tanstack/react-query";
```

Drop:
```ts
import { useEffect, useState } from "react";  // useState may still be needed elsewhere — verify
```
Keep `useCallback`, `useMemo`. The original `useState`/`useEffect` for `settings` and `isLoading` are now gone.

- [ ] **Step 5: Typecheck**

```bash
cd desktop-ui && bunx tsc --noEmit 2>&1 | grep useAppSettings | head -10
```

If errors mention `useEffect` not used: remove it from the import list.

- [ ] **Step 6: Run the existing test for this hook**

```bash
cd desktop-ui && bun run test src/features/settings/hooks/useAppSettings.test
```

If failures stem from `setSettings`/`saveSettings` shape changes, update the test to render inside a `QueryClientProvider`. Specifically wrap with:

```tsx
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

function withQuery(ui: ReactElement) {
	const client = new QueryClient({
		defaultOptions: { queries: { retry: 0 } },
	});
	return <QueryClientProvider client={client}>{ui}</QueryClientProvider>;
}
```

And use `renderHook(() => useAppSettings(), { wrapper: withQuery })`.

- [ ] **Step 7: Commit**

```bash
git add desktop-ui/src/features/settings/hooks/useAppSettings.ts desktop-ui/src/features/settings/hooks/useAppSettings.test.ts
git commit -m "refactor(desktop-ui): migrate useAppSettings to TanStack Query"
```

---

### Task D2: Migrate `useSettingsAgentsSection.ts`

**Current state (inventory):** 10 useState declarations (settings + 7 boolean/string loading flags + 1 generating-target enum + 1 error). Reads `getAgentsSettings()`, exposes 6 mutation operations + 2 description-generators. The operation flags are exactly what `useTauriMutation`'s `isLoading` exposes per-mutation.

**Strategy:** one query + one mutation per operation. Each mutation's `isLoading` replaces a manual flag. The "name of agent currently being updated/deleted/etc" pattern becomes a wrapper that holds the per-name target while the mutation runs.

**Files:**
- Modify: `desktop-ui/src/features/settings/hooks/useSettingsAgentsSection.ts`

- [ ] **Step 1: Read the full current implementation**

```bash
cd desktop-ui && cat src/features/settings/hooks/useSettingsAgentsSection.ts
```

Capture the order in which operations are declared.

- [ ] **Step 2: Rewrite the hook body**

Inside the `useSettingsAgentsSection` function, replace the `useState` declarations and effects with:

```ts
const settingsQuery = useTauriQuery<AgentsSettings | null>({
	queryKey: qk.agents.settings(),
	queryFn: () => getAgentsSettings(),
	fallback: null,
});

const setCore = useTauriMutation<
	void,
	{ multiAgentEnabled?: boolean; maxThreads?: number; maxDepth?: number }
>({
	mutationFn: (input) =>
		setAgentsCoreSettings(
			input.multiAgentEnabled ?? null,
			input.maxThreads ?? null,
			input.maxDepth ?? null,
		),
	invalidates: [qk.agents.settings()],
});

const create = useTauriMutation<
	void,
	{
		name: string;
		description?: string | null;
		developerInstructions?: string | null;
		template?: "blank";
		model?: string | null;
		reasoningEffort?: string | null;
	}
>({
	mutationFn: (input) =>
		createAgent(
			input.name,
			input.description ?? null,
			input.developerInstructions ?? null,
			input.template ?? null,
			input.model ?? null,
			input.reasoningEffort ?? null,
		),
	invalidates: [qk.agents.settings()],
});

const update = useTauriMutation<
	void,
	{
		originalName: string;
		name: string;
		description?: string | null;
		developerInstructions?: string | null;
		renameManagedFile?: boolean;
	}
>({
	mutationFn: (input) =>
		updateAgent(
			input.originalName,
			input.name,
			input.description ?? null,
			input.developerInstructions ?? null,
			input.renameManagedFile ?? false,
		),
	invalidates: [qk.agents.settings()],
});

const remove = useTauriMutation<
	void,
	{ name: string; deleteManagedFile?: boolean }
>({
	mutationFn: (input) => deleteAgent(input.name, input.deleteManagedFile ?? false),
	invalidates: [qk.agents.settings()],
});

const readToml = useTauriMutation<string | null, { name: string }>({
	mutationFn: (input) => readAgentConfigToml(input.name),
	invalidates: [],
});

const writeToml = useTauriMutation<
	void,
	{ name: string; content: string }
>({
	mutationFn: (input) => writeAgentConfigToml(input.name, input.content),
	invalidates: [qk.agents.configToml(""), qk.agents.settings()],
});

const generateDescription = useTauriMutation<
	GeneratedAgentConfiguration | null,
	{
		target: "create" | "edit";
		name?: string;
		description: string;
		developerInstructions: string;
	}
>({
	mutationFn: (input) =>
		generateAgentDescription(
			input.name ?? null,
			input.description,
			input.developerInstructions,
		),
	invalidates: [],
});
```

- [ ] **Step 3: Update the per-name tracking state**

The original tracks `updatingAgentName: string | null` etc. Keep that pattern via a small wrapper:

```ts
const [updatingAgentName, setUpdatingAgentName] = useState<string | null>(null);
const [deletingAgentName, setDeletingAgentName] = useState<string | null>(null);
const [readingConfigAgentName, setReadingConfigAgentName] = useState<
	string | null
>(null);
const [writingConfigAgentName, setWritingConfigAgentName] = useState<
	string | null
>(null);
const [generatingDescriptionTarget, setGeneratingDescriptionTarget] =
	useState<"create" | "edit" | null>(null);
const [error, setError] = useState<string | null>(null);
```

Then wrap each mutation handler:

```ts
const onUpdateAgent = useCallback(
	async (input: Parameters<typeof update.mutate>[0]) => {
		setUpdatingAgentName(input.originalName);
		setError(null);
		try {
			await update.mutate(input);
			return true;
		} catch (e) {
			setError(toErrorMessage(e, "Failed to update agent"));
			return false;
		} finally {
			setUpdatingAgentName(null);
		}
	},
	[update],
);
```

Repeat for `onDeleteAgent`, `onReadAgentConfig`, `onWriteAgentConfig`, `onGenerateCreateDescription`, `onGenerateEditDescription`. The `creatingAgent`/`isUpdatingCore` booleans collapse to `create.isLoading`/`setCore.isLoading`.

- [ ] **Step 4: Wire `onRefresh`**

```ts
const onRefresh = useCallback(() => {
	settingsQuery.refetch();
}, [settingsQuery]);
```

- [ ] **Step 5: Update return shape**

The shape (`SettingsAgentsSectionProps`) is unchanged — but its source values now come from query/mutation state.

```ts
return {
	settings: settingsQuery.data,
	isLoading: settingsQuery.isLoading,
	isUpdatingCore: setCore.isLoading,
	creatingAgent: create.isLoading,
	updatingAgentName,
	deletingAgentName,
	readingConfigAgentName,
	writingConfigAgentName,
	error,
	onRefresh,
	onSetMultiAgentEnabled: async (enabled: boolean) => {
		try {
			await setCore.mutate({ multiAgentEnabled: enabled });
			return true;
		} catch (e) {
			setError(toErrorMessage(e, "Failed to update multi-agent flag"));
			return false;
		}
	},
	onSetMaxThreads: async (maxThreads: number) => {
		try {
			await setCore.mutate({ maxThreads });
			return true;
		} catch (e) {
			setError(toErrorMessage(e, "Failed to update max threads"));
			return false;
		}
	},
	onSetMaxDepth: async (maxDepth: number) => {
		try {
			await setCore.mutate({ maxDepth });
			return true;
		} catch (e) {
			setError(toErrorMessage(e, "Failed to update max depth"));
			return false;
		}
	},
	onCreateAgent: async (input) => {
		setError(null);
		try {
			await create.mutate(input);
			return true;
		} catch (e) {
			setError(toErrorMessage(e, "Failed to create agent"));
			return false;
		}
	},
	onUpdateAgent,
	onDeleteAgent,
	onReadAgentConfig,
	onWriteAgentConfig,
	createDescriptionGenerating:
		generatingDescriptionTarget === "create" && generateDescription.isLoading,
	editDescriptionGenerating:
		generatingDescriptionTarget === "edit" && generateDescription.isLoading,
	onGenerateCreateDescription: async (seed) => {
		setGeneratingDescriptionTarget("create");
		try {
			return await generateDescription.mutate({
				target: "create",
				...seed,
			});
		} finally {
			setGeneratingDescriptionTarget(null);
		}
	},
	onGenerateEditDescription: async (seed) => {
		setGeneratingDescriptionTarget("edit");
		try {
			return await generateDescription.mutate({
				target: "edit",
				...seed,
			});
		} finally {
			setGeneratingDescriptionTarget(null);
		}
	},
	modelOptions: defaultModels.models,
	modelOptionsLoading: defaultModels.isLoading,
	modelOptionsError: defaultModels.error,
};
```

- [ ] **Step 6: Update imports at the top**

Add:
```ts
import { qk, useTauriMutation, useTauriQuery } from "@/lib/query";
```

Drop now-unused `useEffect`. Keep `useCallback`, `useState`.

- [ ] **Step 7: Typecheck**

```bash
cd desktop-ui && bunx tsc --noEmit 2>&1 | grep useSettingsAgentsSection | head -20
```

If errors about `useDefaultModels` or `defaultModels` shape — leave them to be resolved in Task D3 (next).

- [ ] **Step 8: Commit**

```bash
git add desktop-ui/src/features/settings/hooks/useSettingsAgentsSection.ts
git commit -m "refactor(desktop-ui): migrate useSettingsAgentsSection to TQ query+mutations"
```

---

### Task D3: Migrate `useSettingsDefaultModels.ts`

**Current state:** Reads workspaces + per-workspace `getModelList`/`getConfigModel`, accumulates models with sort + dedup. The accumulation must preserve the original semantics.

**Files:**
- Modify: `desktop-ui/src/features/settings/hooks/useSettingsDefaultModels.ts`

- [ ] **Step 1: Read the file**

```bash
cd desktop-ui && cat src/features/settings/hooks/useSettingsDefaultModels.ts
```

Note the model-merging algorithm and the `requestIdRef` race-prevention. Both must be preserved.

- [ ] **Step 2: Rewrite the hook**

Replace the `export function useSettingsDefaultModels(...)` block with:

```ts
export function useSettingsDefaultModels(projects: WorkspaceInfo[]) {
	const workspaceIds = useMemo(
		() => projects.map((p) => p.id).sort().join(","),
		[projects],
	);

	const query = useTauriQuery<SettingsDefaultModelsState>({
		queryKey: ["settings", "defaultModels", workspaceIds],
		queryFn: async () => {
			let connectedWorkspaceCount = 0;
			let lastError: string | null = null;
			const all: ModelOption[] = [];

			for (const project of projects) {
				try {
					await connectWorkspace(project.id);
					connectedWorkspaceCount += 1;
				} catch (e) {
					lastError = String(e);
					continue;
				}
				try {
					const list = parseModelListResponse(
						await getModelList(project.id),
					);
					const configModel = await getConfigModel(project.id);
					if (configModel) {
						all.push({
							id: configModel,
							model: configModel,
							displayName: configModel,
							description: CONFIG_MODEL_DESCRIPTION,
							supportedReasoningEfforts: [],
							defaultReasoningEffort: null,
							isDefault: true,
						});
					}
					for (const m of list) all.push(m);
				} catch (e) {
					lastError = String(e);
				}
			}

			const dedup = new Map<string, ModelOption>();
			for (const m of all) {
				if (!dedup.has(m.id)) dedup.set(m.id, m);
			}
			const models = Array.from(dedup.values()).sort(compareModelsByLatest);

			return {
				models,
				isLoading: false,
				error: lastError,
				connectedWorkspaceCount,
			};
		},
		fallback: EMPTY_STATE,
	});

	const refresh = useCallback(async () => {
		await query.refetch();
	}, [query]);

	return {
		models: query.data.models,
		isLoading: query.isLoading,
		error: query.data.error,
		connectedWorkspaceCount: query.data.connectedWorkspaceCount,
		refresh,
	};
}
```

- [ ] **Step 3: Update imports**

```ts
import { useCallback, useMemo } from "react";
import { qk, useTauriQuery } from "@/lib/query";
```

Drop now-unused `useEffect`, `useRef`, `useState` (if no other usage).

- [ ] **Step 4: Typecheck**

```bash
cd desktop-ui && bunx tsc --noEmit 2>&1 | grep useSettingsDefaultModels | head -10
```

- [ ] **Step 5: Run existing test if any**

```bash
cd desktop-ui && bun run test src/features/settings/hooks/useSettingsDefaultModels.test
```

If wrapper missing, add the same `withQuery` wrapper as in D1 step 6.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/settings/hooks/useSettingsDefaultModels.ts
git commit -m "refactor(desktop-ui): migrate useSettingsDefaultModels to useTauriQuery"
```

---

### Task D4: Migrate `useSettingsFeaturesSection.ts`

**Current state:** Reads paginated `getExperimentalFeatureList` (up to 20 pages of 100 items) plus `getCodexConfigPath`. Mutation is `setCodexFeatureFlag(name, enabled)`.

**Files:**
- Modify: `desktop-ui/src/features/settings/hooks/useSettingsFeaturesSection.ts`

- [ ] **Step 1: Rewrite the hook body**

Inside `useSettingsFeaturesSection({ ... })`, replace the `useState` and effect blocks with:

```ts
const featuresQuery = useTauriQuery<CodexFeature[]>({
	queryKey: qk.settings.features(featureWorkspaceId),
	queryFn: async () => {
		if (!featureWorkspaceId) return [];
		const collected: CodexFeature[] = [];
		let cursor: string | null = null;
		for (let page = 0; page < 20; page += 1) {
			const result = await getExperimentalFeatureList(
				featureWorkspaceId,
				cursor,
				100,
			);
			collected.push(...result.features);
			if (!result.nextCursor) break;
			cursor = result.nextCursor;
		}
		return collected;
	},
	fallback: [],
	enabled: featureWorkspaceId !== null,
});

const configPathQuery = useTauriQuery<string | null>({
	queryKey: qk.settings.codexConfigPath(),
	queryFn: () => getCodexConfigPath(),
	fallback: null,
});

const toggleFeature = useTauriMutation<
	void,
	{ name: string; enabled: boolean }
>({
	mutationFn: (input) => setCodexFeatureFlag(input.name, input.enabled),
	invalidates: [qk.settings.features(featureWorkspaceId)],
});

const [openConfigError, setOpenConfigError] = useState<string | null>(null);
const [featureError, setFeatureError] = useState<string | null>(null);
const [featureUpdatingKey, setFeatureUpdatingKey] = useState<string | null>(
	null,
);

const onOpenConfig = useCallback(async () => {
	const path = configPathQuery.data;
	if (!path) {
		setOpenConfigError("Config path not available");
		return;
	}
	try {
		await revealItemInDir(path);
		setOpenConfigError(null);
	} catch (e) {
		setOpenConfigError(String(e));
	}
}, [configPathQuery.data]);

const onToggleCodexFeature = useCallback(
	async (feature: CodexFeature) => {
		setFeatureUpdatingKey(feature.name);
		setFeatureError(null);
		try {
			await toggleFeature.mutate({
				name: feature.name,
				enabled: !feature.enabled,
			});
		} catch (e) {
			setFeatureError(String(e));
		} finally {
			setFeatureUpdatingKey(null);
		}
	},
	[toggleFeature],
);

const features = featuresQuery.data;
const stableFeatures = useMemo(
	() =>
		features.filter(
			(f) =>
				f.stage === "stable" || f.stage === "beta",
		),
	[features],
);
const experimentalFeatures = useMemo(
	() => features.filter((f) => f.stage === "under_development"),
	[features],
);
const hasDynamicFeatureRows = useMemo(
	() =>
		features.some(
			(f) => !HIDDEN_DYNAMIC_FEATURE_KEYS.has(f.name),
		),
	[features],
);
```

Then return the same `SettingsFeaturesSectionProps` shape:

```ts
return {
	appSettings,
	hasFeatureWorkspace: featureWorkspaceId !== null,
	openConfigError,
	featureError,
	featuresLoading: featuresQuery.isLoading,
	featureUpdatingKey,
	stableFeatures,
	experimentalFeatures,
	hasDynamicFeatureRows,
	onOpenConfig,
	onToggleCodexFeature,
	onUpdateAppSettings,
};
```

- [ ] **Step 2: Update imports**

```ts
import { useCallback, useMemo, useState } from "react";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { qk, useTauriMutation, useTauriQuery } from "@/lib/query";
```

Drop unused `useEffect`.

- [ ] **Step 3: Typecheck + commit**

```bash
cd desktop-ui && bunx tsc --noEmit 2>&1 | grep useSettingsFeaturesSection | head -10
git add desktop-ui/src/features/settings/hooks/useSettingsFeaturesSection.ts
git commit -m "refactor(desktop-ui): migrate useSettingsFeaturesSection to TQ"
```

---

### Task D5: Migrate `useSettingsServerSection.ts` (the heavy one)

**Current state:** 17 useState declarations across remote-backend drafts (5), tailscale state (4 read + 2 status flags), tcp daemon state (2), mobile-connect state (3), and 1 input-validation state. Reads from 4 typed wrappers; writes go through the parent `onUpdateAppSettings` prop (not directly).

**Strategy:** Each REQUEST goes to TQ. Drafts (`remoteNameDraft`, `remoteHostDraft`, `remoteTokenDraft`) stay as `useState` — they're transient form state. Loading flags collapse into the matching mutation/query's `isLoading`.

**Files:**
- Modify: `desktop-ui/src/features/settings/hooks/useSettingsServerSection.ts`

- [ ] **Step 1: Read the current file**

```bash
cd desktop-ui && cat src/features/settings/hooks/useSettingsServerSection.ts
```

Capture exact action handler names (likely `onCommitRemoteName`, `onTcpDaemonStart`, etc.).

- [ ] **Step 2: Replace the read state with queries**

Inside the hook body, replace the read-related useStates and effects with:

```ts
const tailscaleStatusQuery = useTauriQuery<TailscaleStatus | null>({
	queryKey: qk.settings.tailscaleStatus(),
	queryFn: () => fetchTailscaleStatus(),
	fallback: null,
});

const tailscaleCommandPreviewQuery =
	useTauriQuery<TailscaleDaemonCommandPreview | null>({
		queryKey: qk.settings.tailscaleCommandPreview(),
		queryFn: () => fetchTailscaleDaemonCommandPreview(),
		fallback: null,
	});

const tcpDaemonStatusQuery = useTauriQuery<TcpDaemonStatus | null>({
	queryKey: qk.settings.tcpDaemonStatus(),
	queryFn: () => tailscaleDaemonStatus(),
	fallback: null,
});

const tcpDaemonStart = useTauriMutation<void, void>({
	mutationFn: () => tailscaleDaemonStart(),
	invalidates: [qk.settings.tcpDaemonStatus()],
});
const tcpDaemonStop = useTauriMutation<void, void>({
	mutationFn: () => tailscaleDaemonStop(),
	invalidates: [qk.settings.tcpDaemonStatus()],
});
```

- [ ] **Step 3: Keep draft `useState`s and wrappers**

The form drafts (`remoteNameDraft`/`remoteHostDraft`/`remoteTokenDraft`) plus error states and the mobile-connect status texts stay as `useState`. They're not server data.

```ts
const [remoteNameDraft, setRemoteNameDraft] = useState(
	initialActiveRemoteBackend.name,
);
const [remoteHostDraft, setRemoteHostDraft] = useState(
	initialActiveRemoteBackend.host,
);
const [remoteTokenDraft, setRemoteTokenDraft] = useState(
	initialActiveRemoteBackend.token ?? "",
);
const [remoteNameError, setRemoteNameError] = useState<string | null>(null);
const [remoteHostError, setRemoteHostError] = useState<string | null>(null);
const [remoteStatusText, setRemoteStatusText] = useState<string | null>(null);
const [remoteStatusError, setRemoteStatusError] = useState(false);
const [mobileConnectBusy, setMobileConnectBusy] = useState(false);
const [mobileConnectStatusText, setMobileConnectStatusText] = useState<
	string | null
>(null);
const [mobileConnectStatusError, setMobileConnectStatusError] = useState(false);
```

- [ ] **Step 4: Wire `onTcpDaemonStart`/`Stop`/`Status`**

```ts
const [tcpDaemonBusyAction, setTcpDaemonBusyAction] = useState<
	"start" | "stop" | "status" | null
>(null);

const onTcpDaemonStart = useCallback(async () => {
	setTcpDaemonBusyAction("start");
	try {
		await tcpDaemonStart.mutate();
	} finally {
		setTcpDaemonBusyAction(null);
	}
}, [tcpDaemonStart]);

const onTcpDaemonStop = useCallback(async () => {
	setTcpDaemonBusyAction("stop");
	try {
		await tcpDaemonStop.mutate();
	} finally {
		setTcpDaemonBusyAction(null);
	}
}, [tcpDaemonStop]);

const onTcpDaemonStatus = useCallback(async () => {
	setTcpDaemonBusyAction("status");
	try {
		await tcpDaemonStatusQuery.refetch();
	} finally {
		setTcpDaemonBusyAction(null);
	}
}, [tcpDaemonStatusQuery]);
```

- [ ] **Step 5: Refresh callbacks**

```ts
const onRefreshTailscaleStatus = useCallback(() => {
	tailscaleStatusQuery.refetch();
}, [tailscaleStatusQuery]);

const onRefreshTailscaleCommandPreview = useCallback(() => {
	tailscaleCommandPreviewQuery.refetch();
}, [tailscaleCommandPreviewQuery]);
```

- [ ] **Step 6: Update return shape**

Map `SettingsServerSectionProps` exactly. Replace existing `tailscaleStatus`, `tailscaleStatusBusy`, `tailscaleStatusError`, etc., with:

```ts
return {
	appSettings,
	onUpdateAppSettings,
	isMobilePlatform: isMobilePlatform(),
	mobileConnectBusy,
	mobileConnectStatusText,
	mobileConnectStatusError,
	remoteBackends: appSettings.remoteBackends,
	activeRemoteBackendId: appSettings.activeRemoteBackendId,
	remoteStatusText,
	remoteStatusError,
	remoteNameError,
	remoteHostError,
	remoteNameDraft,
	remoteHostDraft,
	remoteTokenDraft,
	nextRemoteNameSuggestion: deriveNextRemoteName(
		appSettings.remoteBackends,
	), // existing helper
	tailscaleStatus: tailscaleStatusQuery.data,
	tailscaleStatusBusy: tailscaleStatusQuery.isLoading,
	tailscaleStatusError:
		tailscaleStatusQuery.error == null
			? null
			: String(tailscaleStatusQuery.error),
	tailscaleCommandPreview: tailscaleCommandPreviewQuery.data,
	tailscaleCommandBusy: tailscaleCommandPreviewQuery.isLoading,
	tailscaleCommandError:
		tailscaleCommandPreviewQuery.error == null
			? null
			: String(tailscaleCommandPreviewQuery.error),
	tcpDaemonStatus: tcpDaemonStatusQuery.data,
	tcpDaemonBusyAction,
	onSetRemoteNameDraft: setRemoteNameDraft,
	onSetRemoteHostDraft: setRemoteHostDraft,
	onSetRemoteTokenDraft: setRemoteTokenDraft,
	onCommitRemoteName,
	onCommitRemoteHost,
	onCommitRemoteToken,
	onSelectRemoteBackend,
	onAddRemoteBackend,
	onMoveRemoteBackend,
	onDeleteRemoteBackend,
	onRefreshTailscaleStatus,
	onRefreshTailscaleCommandPreview,
	onUseSuggestedTailscaleHost,
	onTcpDaemonStart,
	onTcpDaemonStop,
	onTcpDaemonStatus,
	onMobileConnectTest,
};
```

The `onCommitRemoteName/Host/Token`, `onSelectRemoteBackend`, `onAddRemoteBackend`, `onMoveRemoteBackend`, `onDeleteRemoteBackend`, `onUseSuggestedTailscaleHost`, `onMobileConnectTest` callbacks reuse the original logic — they call `onUpdateAppSettings` and write through; nothing TQ-specific.

- [ ] **Step 7: Imports**

```ts
import { useCallback, useMemo, useRef, useState } from "react";
import { qk, useTauriMutation, useTauriQuery } from "@/lib/query";
```

Drop unused.

- [ ] **Step 8: Typecheck + commit**

```bash
cd desktop-ui && bunx tsc --noEmit 2>&1 | grep useSettingsServerSection | head -20
git add desktop-ui/src/features/settings/hooks/useSettingsServerSection.ts
git commit -m "refactor(desktop-ui): migrate useSettingsServerSection to TQ"
```

---

## Phase E — Registries

### Task E1: Migrate `useModels.ts`

**Current state (inventory):** Calls `getModelList(workspaceId)` + `getConfigModel(workspaceId)` per active workspace. Holds `models`, `configModel`, `selectedModelId`, `selectedEffort`. Selection state stays as `useState` (it's UI state); only fetched data migrates.

**Files:**
- Modify: `desktop-ui/src/features/models/hooks/useModels.ts`

- [ ] **Step 1: Replace the data-fetch portion**

Inside `useModels({ activeWorkspace, ... })`, replace the `useState<ModelOption[]>([])` + `useState<string | null>(null)` for `models` and `configModel` with:

```ts
const workspaceId = activeWorkspace?.id ?? "";

const modelsQuery = useTauriQuery<ModelOption[]>({
	queryKey: qk.models.list(workspaceId),
	queryFn: async () => {
		if (!activeWorkspace) return [];
		return parseModelListResponse(await getModelList(activeWorkspace.id));
	},
	fallback: [],
	enabled: activeWorkspace !== null,
});

const configModelQuery = useTauriQuery<string | null>({
	queryKey: qk.models.configModel(workspaceId),
	queryFn: async () => {
		if (!activeWorkspace) return null;
		return await getConfigModel(activeWorkspace.id);
	},
	fallback: null,
	enabled: activeWorkspace !== null,
});

const models = modelsQuery.data;
const configModel = configModelQuery.data;
```

Keep `selectedModelId`, `setSelectedModelIdState`, `selectedEffort`, `setSelectedEffortState` as `useState`.

- [ ] **Step 2: Replace `refreshModels`**

```ts
const refreshModels = useCallback(async () => {
	await Promise.all([modelsQuery.refetch(), configModelQuery.refetch()]);
}, [modelsQuery, configModelQuery]);
```

- [ ] **Step 3: Update imports**

```ts
import { useCallback, useMemo } from "react";
import { qk, useTauriQuery } from "@/lib/query";
```

Drop now-unused `useEffect`, `useRef`, `useState` if not used elsewhere in file.

- [ ] **Step 4: Typecheck + commit**

```bash
cd desktop-ui && bunx tsc --noEmit 2>&1 | grep useModels | head -10
git add desktop-ui/src/features/models/hooks/useModels.ts
git commit -m "refactor(desktop-ui): migrate useModels to useTauriQuery"
```

---

### Task E2: Migrate `useSkills.ts`

**Current state (inventory):** Calls `getSkillsList(workspaceId)` + listens for `SkillsUpdateAvailable` via `subscribeAppServerEvents`. Filters skills by `name` truthiness.

**Files:**
- Modify: `desktop-ui/src/features/skills/hooks/useSkills.ts`

- [ ] **Step 1: Replace the hook body**

```ts
import { useCallback } from "react";
import type { DebugEntry, SkillOption, WorkspaceInfo } from "@/types";
import { getSkillsList } from "@services/tauri";
import { qk, useTauriQuery } from "@/lib/query";

export function useSkills(activeWorkspace: WorkspaceInfo | null) {
	const workspaceId = activeWorkspace?.id ?? "";

	const query = useTauriQuery<SkillOption[]>({
		queryKey: qk.skills.list(workspaceId),
		queryFn: async () => {
			if (!activeWorkspace) return [];
			const list = await getSkillsList(activeWorkspace.id);
			return list.filter((s) => Boolean(s.name));
		},
		fallback: [],
		enabled: activeWorkspace !== null,
	});

	const refreshSkills = useCallback(async () => {
		await query.refetch();
	}, [query]);

	return {
		skills: query.data,
		refreshSkills,
	};
}
```

The `SkillsUpdateAvailable` subscription is now handled centrally by `appServerEventBridge` (Task A5).

- [ ] **Step 2: Drop unused imports**

The previous file imported `subscribeAppServerEvents` and `isSkillsUpdateAvailableEvent`. Remove them.

- [ ] **Step 3: Run any existing test**

```bash
cd desktop-ui && bun run test src/features/skills/hooks/useSkills.test
```

Update test wrapper to `QueryClientProvider` if needed.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/skills/hooks/useSkills.ts
git commit -m "refactor(desktop-ui): migrate useSkills to useTauriQuery"
```

---

### Task E3: Migrate `useApps.ts`

**Current state:** Calls `getAppsList(workspaceId, null, 100, threadId)`. Listens for `AppListUpdated`. Has a `retryVersion` retry counter.

**Files:**
- Modify: `desktop-ui/src/features/apps/hooks/useApps.ts`

- [ ] **Step 1: Read the current file to confirm threadId source**

```bash
cd desktop-ui && cat src/features/apps/hooks/useApps.ts
```

Capture the threadId argument shape.

- [ ] **Step 2: Rewrite**

```ts
import { useCallback } from "react";
import type { AppOption, DebugEntry, WorkspaceInfo } from "@/types";
import { getAppsList } from "@services/tauri";
import { qk, useTauriQuery } from "@/lib/query";

interface UseAppsArgs {
	activeWorkspace: WorkspaceInfo | null;
	activeThreadId: string | null;
	onDebug?: (entry: DebugEntry) => void;
}

export function useApps({ activeWorkspace, activeThreadId }: UseAppsArgs) {
	const workspaceId = activeWorkspace?.id ?? "";

	const query = useTauriQuery<AppOption[]>({
		queryKey: qk.apps.list(workspaceId, activeThreadId),
		queryFn: async () => {
			if (!activeWorkspace) return [];
			const list = await getAppsList(
				activeWorkspace.id,
				null,
				100,
				activeThreadId,
			);
			return list.filter((a) => Boolean(a.id) && Boolean(a.name));
		},
		fallback: [],
		enabled: activeWorkspace !== null,
	});

	const refreshApps = useCallback(async () => {
		await query.refetch();
	}, [query]);

	return {
		apps: query.data,
		refreshApps,
	};
}
```

- [ ] **Step 3: Update the caller in `MainApp.tsx`**

```bash
cd desktop-ui && grep -n "useApps(" src/features/app/components/MainApp.tsx
```

If the caller uses positional args — switch to the object shape `useApps({ activeWorkspace, activeThreadId })`. Otherwise leave.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/apps/hooks/useApps.ts desktop-ui/src/features/app/components/MainApp.tsx
git commit -m "refactor(desktop-ui): migrate useApps to useTauriQuery"
```

---

### Task E4: Migrate `useCustomPrompts.ts`

**Current state:** Reads `getPromptsList(workspaceId)`, filters by `name` truthiness. Has 6 mutation actions: create, update, delete, move, plus 2 directory readers.

**Files:**
- Modify: `desktop-ui/src/features/prompts/hooks/useCustomPrompts.ts`

- [ ] **Step 1: Rewrite**

```ts
import { useCallback } from "react";
import type { CustomPromptOption, DebugEntry, WorkspaceInfo } from "@/types";
import {
	createPrompt as createPromptService,
	deletePrompt as deletePromptService,
	getGlobalPromptsDir as getGlobalPromptsDirService,
	getPromptsList,
	getWorkspacePromptsDir as getWorkspacePromptsDirService,
	movePrompt as movePromptService,
	updatePrompt as updatePromptService,
} from "@services/tauri";
import { qk, useTauriMutation, useTauriQuery } from "@/lib/query";

export function useCustomPrompts(activeWorkspace: WorkspaceInfo | null) {
	const workspaceId = activeWorkspace?.id ?? "";

	const query = useTauriQuery<CustomPromptOption[]>({
		queryKey: qk.prompts.list(workspaceId),
		queryFn: async () => {
			if (!activeWorkspace) return [];
			const list = await getPromptsList(activeWorkspace.id);
			return list.filter((p) => Boolean(p.name));
		},
		fallback: [],
		enabled: activeWorkspace !== null,
	});

	const create = useTauriMutation<
		void,
		{
			scope: "workspace" | "global";
			name: string;
			description?: string | null;
			argumentHint?: string | null;
			content: string;
		}
	>({
		mutationFn: async (data) => {
			if (!activeWorkspace) throw new Error("no workspace");
			await createPromptService(activeWorkspace.id, data);
		},
		invalidates: [qk.prompts.list(workspaceId)],
	});

	const update = useTauriMutation<
		void,
		{
			path: string;
			name: string;
			description?: string | null;
			argumentHint?: string | null;
			content: string;
		}
	>({
		mutationFn: async (data) => {
			if (!activeWorkspace) throw new Error("no workspace");
			await updatePromptService(activeWorkspace.id, data);
		},
		invalidates: [qk.prompts.list(workspaceId)],
	});

	const remove = useTauriMutation<void, { path: string }>({
		mutationFn: async (data) => {
			if (!activeWorkspace) throw new Error("no workspace");
			await deletePromptService(activeWorkspace.id, data.path);
		},
		invalidates: [qk.prompts.list(workspaceId)],
	});

	const move = useTauriMutation<
		void,
		{ path: string; scope: "workspace" | "global" }
	>({
		mutationFn: async (data) => {
			if (!activeWorkspace) throw new Error("no workspace");
			await movePromptService(activeWorkspace.id, data);
		},
		invalidates: [qk.prompts.list(workspaceId)],
	});

	const refreshPrompts = useCallback(async () => {
		await query.refetch();
	}, [query]);

	const getWorkspacePromptsDir = useCallback(async () => {
		if (!activeWorkspace) return null;
		return await getWorkspacePromptsDirService(activeWorkspace.id);
	}, [activeWorkspace]);

	const getGlobalPromptsDir = useCallback(async () => {
		if (!activeWorkspace) return null;
		return await getGlobalPromptsDirService(activeWorkspace.id);
	}, [activeWorkspace]);

	return {
		prompts: query.data,
		refreshPrompts,
		createPrompt: create.mutate,
		updatePrompt: update.mutate,
		deletePrompt: (path: string) => remove.mutate({ path }),
		movePrompt: move.mutate,
		getWorkspacePromptsDir,
		getGlobalPromptsDir,
	};
}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/prompts/hooks/useCustomPrompts.ts
git commit -m "refactor(desktop-ui): migrate useCustomPrompts to TQ query+mutations"
```

---

## Phase F — Git panel

### Task F1: Migrate `useGitStatus.ts`

**Current state:** Polls `getGitStatus(workspaceId)` every 3s; holds `GitStatusState`. Polling becomes redundant when commit/push/pull mutations invalidate it.

**Files:**
- Modify: `desktop-ui/src/features/git/hooks/useGitStatus.ts`

- [ ] **Step 1: Rewrite**

```ts
import { useCallback } from "react";
import type { WorkspaceInfo } from "@/types";
import { getGitStatus } from "@services/tauri";
import { qk, useTauriQuery } from "@/lib/query";
import type { GitStatusState } from "../types";

const emptyStatus: GitStatusState = {
	branchName: "",
	files: [],
	stagedFiles: [],
	unstagedFiles: [],
	totalAdditions: 0,
	totalDeletions: 0,
	error: null,
};

export function useGitStatus(activeWorkspace: WorkspaceInfo | null) {
	const workspaceId = activeWorkspace?.id ?? "";

	const query = useTauriQuery<GitStatusState>({
		queryKey: qk.git.status(workspaceId),
		queryFn: async () => {
			if (!activeWorkspace) return emptyStatus;
			return await getGitStatus(activeWorkspace.id);
		},
		fallback: emptyStatus,
		enabled: activeWorkspace !== null,
		// 5s — git state changes via commits/checkouts (which invalidate),
		// but external changes (CLI commits in another terminal) only show
		// up after staleTime.
		staleTime: 5_000,
	});

	const refresh = useCallback(async () => {
		await query.refetch();
	}, [query]);

	return {
		status: query.data,
		refresh,
	};
}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/git/hooks/useGitStatus.ts
git commit -m "refactor(desktop-ui): migrate useGitStatus to useTauriQuery"
```

---

### Task F2: Migrate `useGitBranches.ts`

**Current state (inventory):** Reads `listGitBranches`. Mutations: `checkoutGitBranch`, `checkoutGitHubPullRequest`, `createGitBranch`. Sorted by `lastCommit`.

**Files:**
- Modify: `desktop-ui/src/features/git/hooks/useGitBranches.ts`

- [ ] **Step 1: Rewrite**

```ts
import { useCallback, useMemo, useState } from "react";
import type { BranchInfo, WorkspaceInfo } from "@/types";
import {
	checkoutGitBranch,
	checkoutGitHubPullRequest,
	createGitBranch,
	listGitBranches,
} from "@services/tauri";
import { qk, useTauriMutation, useTauriQuery } from "@/lib/query";

export function useGitBranches(activeWorkspace: WorkspaceInfo | null) {
	const workspaceId = activeWorkspace?.id ?? "";
	const [error, setError] = useState<string | null>(null);

	const query = useTauriQuery<BranchInfo[]>({
		queryKey: qk.git.branches(workspaceId),
		queryFn: async () => {
			if (!activeWorkspace) return [];
			return await listGitBranches(activeWorkspace.id);
		},
		fallback: [],
		enabled: activeWorkspace !== null,
	});

	const branches = useMemo(
		() =>
			[...query.data].sort((a, b) =>
				(b.lastCommit ?? "").localeCompare(a.lastCommit ?? ""),
			),
		[query.data],
	);

	const checkout = useTauriMutation<void, { name: string }>({
		mutationFn: async ({ name }) => {
			if (!activeWorkspace) throw new Error("no workspace");
			await checkoutGitBranch(activeWorkspace.id, name);
		},
		invalidates: [qk.git.branches(workspaceId), qk.git.status(workspaceId)],
		onError: (e) => setError(String(e)),
	});

	const checkoutPr = useTauriMutation<void, { prNumber: number }>({
		mutationFn: async ({ prNumber }) => {
			if (!activeWorkspace) throw new Error("no workspace");
			await checkoutGitHubPullRequest(activeWorkspace.id, prNumber);
		},
		invalidates: [qk.git.branches(workspaceId), qk.git.status(workspaceId)],
		onError: (e) => setError(String(e)),
	});

	const createBranch = useTauriMutation<void, { name: string }>({
		mutationFn: async ({ name }) => {
			if (!activeWorkspace) throw new Error("no workspace");
			await createGitBranch(activeWorkspace.id, name);
		},
		invalidates: [qk.git.branches(workspaceId), qk.git.status(workspaceId)],
		onError: (e) => setError(String(e)),
	});

	const refreshBranches = useCallback(async () => {
		await query.refetch();
	}, [query]);

	return {
		branches,
		error,
		refreshBranches,
		checkoutBranch: (name: string) => checkout.mutate({ name }),
		checkoutPullRequest: (prNumber: number) => checkoutPr.mutate({ prNumber }),
		createBranch: (name: string) => createBranch.mutate({ name }),
	};
}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/git/hooks/useGitBranches.ts
git commit -m "refactor(desktop-ui): migrate useGitBranches to TQ query+mutations"
```

---

### Task F3: Migrate `useGitDiffs.ts`

**Files:**
- Modify: `desktop-ui/src/features/git/hooks/useGitDiffs.ts`

- [ ] **Step 1: Rewrite**

```ts
import { useCallback } from "react";
import type { WorkspaceInfo } from "@/types";
import { getGitDiffs } from "@services/tauri";
import { qk, useTauriQuery } from "@/lib/query";

interface GitDiffState {
	diffs: Awaited<ReturnType<typeof getGitDiffs>>;
	isLoading: boolean;
	error: string | null;
}

const emptyState: GitDiffState = { diffs: [], isLoading: false, error: null };

export function useGitDiffs(activeWorkspace: WorkspaceInfo | null) {
	const workspaceId = activeWorkspace?.id ?? "";

	const query = useTauriQuery<GitDiffState["diffs"]>({
		queryKey: qk.git.diffs(workspaceId),
		queryFn: async () => {
			if (!activeWorkspace) return [];
			return await getGitDiffs(activeWorkspace.id);
		},
		fallback: [],
		enabled: activeWorkspace !== null,
	});

	const refresh = useCallback(async () => {
		await query.refetch();
	}, [query]);

	return {
		diffs: query.data,
		isLoading: query.isLoading,
		error: query.error == null ? null : String(query.error),
		refresh,
	};
}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/git/hooks/useGitDiffs.ts
git commit -m "refactor(desktop-ui): migrate useGitDiffs to useTauriQuery"
```

---

### Task F4: Migrate `useGitLog.ts`

**Current state:** 10s polling of `getGitLog`. Returns `entries`, `total`, `ahead`, `behind`, `aheadEntries`, `behindEntries`, `upstream`, `isLoading`, `error`, `refresh`.

**Files:**
- Modify: `desktop-ui/src/features/git/hooks/useGitLog.ts`

- [ ] **Step 1: Rewrite**

```ts
import { useCallback } from "react";
import type { GitLogEntry, WorkspaceInfo } from "@/types";
import { getGitLog } from "@services/tauri";
import { qk, useTauriQuery } from "@/lib/query";

interface GitLogState {
	entries: GitLogEntry[];
	total: number;
	ahead: number;
	behind: number;
	aheadEntries: GitLogEntry[];
	behindEntries: GitLogEntry[];
	upstream: string | null;
}

const emptyState: GitLogState = {
	entries: [],
	total: 0,
	ahead: 0,
	behind: 0,
	aheadEntries: [],
	behindEntries: [],
	upstream: null,
};

export function useGitLog(activeWorkspace: WorkspaceInfo | null) {
	const workspaceId = activeWorkspace?.id ?? "";

	const query = useTauriQuery<GitLogState>({
		queryKey: qk.git.log(workspaceId),
		queryFn: async () => {
			if (!activeWorkspace) return emptyState;
			return await getGitLog(activeWorkspace.id);
		},
		fallback: emptyState,
		enabled: activeWorkspace !== null,
		// 10s mirrors the original polling interval — git log doesn't
		// change frequently and TQ already invalidates on commit/push.
		staleTime: 10_000,
	});

	const refresh = useCallback(async () => {
		await query.refetch();
	}, [query]);

	return {
		...query.data,
		isLoading: query.isLoading,
		error: query.error == null ? null : String(query.error),
		refresh,
	};
}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/git/hooks/useGitLog.ts
git commit -m "refactor(desktop-ui): migrate useGitLog to useTauriQuery"
```

---

### Task F5: Migrate `useGitRemote.ts`

**Files:**
- Modify: `desktop-ui/src/features/git/hooks/useGitRemote.ts`

- [ ] **Step 1: Rewrite**

```ts
import { useCallback } from "react";
import type { WorkspaceInfo } from "@/types";
import { getGitRemote } from "@services/tauri";
import { qk, useTauriQuery } from "@/lib/query";

export function useGitRemote(activeWorkspace: WorkspaceInfo | null) {
	const workspaceId = activeWorkspace?.id ?? "";

	const query = useTauriQuery<string | null>({
		queryKey: qk.git.remote(workspaceId),
		queryFn: async () => {
			if (!activeWorkspace) return null;
			return await getGitRemote(activeWorkspace.id);
		},
		fallback: null,
		enabled: activeWorkspace !== null,
	});

	const refresh = useCallback(async () => {
		await query.refetch();
	}, [query]);

	return {
		remote: query.data,
		error: query.error == null ? null : String(query.error),
		refresh,
	};
}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/git/hooks/useGitRemote.ts
git commit -m "refactor(desktop-ui): migrate useGitRemote to useTauriQuery"
```

---

### Task F6: Migrate `useGitCommitDiffs.ts`

**Files:**
- Modify: `desktop-ui/src/features/git/hooks/useGitCommitDiffs.ts`

- [ ] **Step 1: Rewrite**

```ts
import { useCallback } from "react";
import type { GitCommitDiff, WorkspaceInfo } from "@/types";
import { getGitCommitDiff } from "@services/tauri";
import { qk, useTauriQuery } from "@/lib/query";

interface CommitDiffState {
	diffs: GitCommitDiff[];
	isLoading: boolean;
	error: string | null;
}

export function useGitCommitDiffs(
	activeWorkspace: WorkspaceInfo | null,
	sha: string | null,
) {
	const workspaceId = activeWorkspace?.id ?? "";

	const query = useTauriQuery<GitCommitDiff[]>({
		queryKey: qk.git.commitDiffs(workspaceId, sha ?? ""),
		queryFn: async () => {
			if (!activeWorkspace || !sha) return [];
			return await getGitCommitDiff(activeWorkspace.id, sha);
		},
		fallback: [],
		enabled: activeWorkspace !== null && sha !== null,
	});

	const refresh = useCallback(async () => {
		await query.refetch();
	}, [query]);

	return {
		diffs: query.data,
		isLoading: query.isLoading,
		error: query.error == null ? null : String(query.error),
		refresh,
	};
}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/git/hooks/useGitCommitDiffs.ts
git commit -m "refactor(desktop-ui): migrate useGitCommitDiffs to useTauriQuery"
```

---

### Task F7: Migrate `useGitRepoScan.ts`

**Current state:** On-demand only (no auto-fetch). Holds `repos`, `isLoading`, `error`, `depth`, `hasScanned`. Exposes `scan()`, `setDepth(n)`, `clear()`.

**Files:**
- Modify: `desktop-ui/src/features/git/hooks/useGitRepoScan.ts`

- [ ] **Step 1: Rewrite**

```ts
import { useCallback, useState } from "react";
import type { WorkspaceInfo } from "@/types";
import { listGitRoots } from "@services/tauri";
import { qk, useTauriMutation } from "@/lib/query";
import { useQueryClient } from "@tanstack/react-query";

export function useGitRepoScan(activeWorkspace: WorkspaceInfo | null) {
	const queryClient = useQueryClient();
	const [depth, setDepthState] = useState(2);

	const scan = useTauriMutation<string[], void>({
		mutationFn: async () => {
			if (!activeWorkspace) return [];
			return await listGitRoots(activeWorkspace.id, depth);
		},
		// On-demand — write the result into the cache so subsequent reads
		// hit it without re-running the scan.
		invalidates: [],
	});

	const repos =
		(activeWorkspace
			? queryClient.getQueryData<string[]>(
					qk.git.repoScan(activeWorkspace.id, depth),
				)
			: []) ?? [];

	const runScan = useCallback(async () => {
		const result = await scan.mutate();
		if (activeWorkspace) {
			queryClient.setQueryData(
				qk.git.repoScan(activeWorkspace.id, depth),
				result,
			);
		}
	}, [scan, activeWorkspace, queryClient, depth]);

	const clear = useCallback(() => {
		if (!activeWorkspace) return;
		queryClient.removeQueries({
			queryKey: qk.git.repoScan(activeWorkspace.id, depth),
		});
	}, [queryClient, activeWorkspace, depth]);

	return {
		repos,
		isLoading: scan.isLoading,
		error: scan.error == null ? null : String(scan.error),
		depth,
		hasScanned: repos.length > 0,
		scan: runScan,
		setDepth: setDepthState,
		clear,
	};
}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/git/hooks/useGitRepoScan.ts
git commit -m "refactor(desktop-ui): migrate useGitRepoScan to TQ mutation+cache"
```

---

### Task F8: Migrate `useGitActions.ts`

**Current state:** 5 mutations: `stageGitFile`, `stageGitAll`, `unstageGitFile`, `revertGitFile`, `revertGitAll`, `applyWorktreeChanges`, `initGitRepo`, `createGitHubRepo`.

**Files:**
- Modify: `desktop-ui/src/features/git/hooks/useGitActions.ts`

- [ ] **Step 1: Rewrite**

```ts
import { useCallback, useState } from "react";
import type { WorkspaceInfo } from "@/types";
import {
	applyWorktreeChanges as applyWorktreeChangesService,
	createGitHubRepo as createGitHubRepoService,
	initGitRepo as initGitRepoService,
	revertGitAll,
	revertGitFile as revertGitFileService,
	stageGitAll as stageGitAllService,
	stageGitFile as stageGitFileService,
	unstageGitFile as unstageGitFileService,
} from "@services/tauri";
import { qk, useTauriMutation } from "@/lib/query";

export function useGitActions(activeWorkspace: WorkspaceInfo | null) {
	const workspaceId = activeWorkspace?.id ?? "";
	const invalidatesGit = [
		qk.git.status(workspaceId),
		qk.git.diffs(workspaceId),
		qk.git.log(workspaceId),
		qk.git.branches(workspaceId),
	];

	const stageFile = useTauriMutation<void, { path: string }>({
		mutationFn: ({ path }) => {
			if (!activeWorkspace) throw new Error("no workspace");
			return stageGitFileService(activeWorkspace.id, path);
		},
		invalidates: invalidatesGit,
	});
	const stageAll = useTauriMutation<void, void>({
		mutationFn: () => {
			if (!activeWorkspace) throw new Error("no workspace");
			return stageGitAllService(activeWorkspace.id);
		},
		invalidates: invalidatesGit,
	});
	const unstageFile = useTauriMutation<void, { path: string }>({
		mutationFn: ({ path }) => {
			if (!activeWorkspace) throw new Error("no workspace");
			return unstageGitFileService(activeWorkspace.id, path);
		},
		invalidates: invalidatesGit,
	});
	const revertFile = useTauriMutation<void, { path: string }>({
		mutationFn: ({ path }) => {
			if (!activeWorkspace) throw new Error("no workspace");
			return revertGitFileService(activeWorkspace.id, path);
		},
		invalidates: invalidatesGit,
	});
	const revertAll = useTauriMutation<void, void>({
		mutationFn: () => {
			if (!activeWorkspace) throw new Error("no workspace");
			return revertGitAll(activeWorkspace.id);
		},
		invalidates: invalidatesGit,
	});
	const applyWorktree = useTauriMutation<void, void>({
		mutationFn: () => {
			if (!activeWorkspace) throw new Error("no workspace");
			return applyWorktreeChangesService(activeWorkspace.id);
		},
		invalidates: invalidatesGit,
	});
	const initRepo = useTauriMutation<
		"initialized" | "cancelled" | "failed",
		{ branch: string; force?: boolean }
	>({
		mutationFn: async ({ branch, force = false }) => {
			if (!activeWorkspace) throw new Error("no workspace");
			return await initGitRepoService(activeWorkspace.id, branch, force);
		},
		invalidates: invalidatesGit,
	});
	const createGitHubRepoMut = useTauriMutation<
		{ ok: true } | { ok: false; error: string },
		{ repo: string; visibility: "private" | "public"; branch: string }
	>({
		mutationFn: async ({ repo, visibility, branch }) => {
			if (!activeWorkspace) throw new Error("no workspace");
			return await createGitHubRepoService(
				activeWorkspace.id,
				repo,
				visibility,
				branch,
			);
		},
		invalidates: invalidatesGit,
	});

	const [worktreeApplyError, setWorktreeApplyError] = useState<
		string | null
	>(null);
	const [worktreeApplySuccess, setWorktreeApplySuccess] = useState(false);

	const applyWorktreeChanges = useCallback(async () => {
		setWorktreeApplyError(null);
		setWorktreeApplySuccess(false);
		try {
			await applyWorktree.mutate();
			setWorktreeApplySuccess(true);
		} catch (e) {
			setWorktreeApplyError(String(e));
		}
	}, [applyWorktree]);

	return {
		applyWorktreeChanges,
		createGitHubRepo: (
			repo: string,
			visibility: "private" | "public",
			branch: string,
		) => createGitHubRepoMut.mutate({ repo, visibility, branch }),
		createGitHubRepoLoading: createGitHubRepoMut.isLoading,
		initGitRepo: (branch: string) => initRepo.mutate({ branch }),
		initGitRepoLoading: initRepo.isLoading,
		revertAllGitChanges: () => revertAll.mutate(),
		revertGitFile: (path: string) => revertFile.mutate({ path }),
		stageGitAll: () => stageAll.mutate(),
		stageGitFile: (path: string) => stageFile.mutate({ path }),
		unstageGitFile: (path: string) => unstageFile.mutate({ path }),
		worktreeApplyError,
		worktreeApplyLoading: applyWorktree.isLoading,
		worktreeApplySuccess,
	};
}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/git/hooks/useGitActions.ts
git commit -m "refactor(desktop-ui): migrate useGitActions to TQ mutations + auto-invalidate"
```

---

### Task F9: Migrate the four GitHub hooks

Each follows the same template. Do them in 4 separate commits to keep diffs reviewable.

**Files:**
- Modify: `desktop-ui/src/features/git/hooks/useGitHubIssues.ts`
- Modify: `desktop-ui/src/features/git/hooks/useGitHubPullRequests.ts`
- Modify: `desktop-ui/src/features/git/hooks/useGitHubPullRequestDiffs.ts`
- Modify: `desktop-ui/src/features/git/hooks/useGitHubPullRequestComments.ts`

- [ ] **Step F9a: useGitHubIssues**

Replace with:

```ts
import { useCallback } from "react";
import type { GitHubIssue, WorkspaceInfo } from "@/types";
import { getGitHubIssues } from "@services/tauri";
import { qk, useTauriQuery } from "@/lib/query";

export function useGitHubIssues(activeWorkspace: WorkspaceInfo | null) {
	const workspaceId = activeWorkspace?.id ?? "";

	const query = useTauriQuery<{ issues: GitHubIssue[]; total: number }>({
		queryKey: qk.github.issues(workspaceId),
		queryFn: async () => {
			if (!activeWorkspace) return { issues: [], total: 0 };
			return await getGitHubIssues(activeWorkspace.id);
		},
		fallback: { issues: [], total: 0 },
		enabled: activeWorkspace !== null,
	});

	const refresh = useCallback(async () => {
		await query.refetch();
	}, [query]);

	return {
		issues: query.data.issues,
		total: query.data.total,
		isLoading: query.isLoading,
		error: query.error == null ? null : String(query.error),
		refresh,
	};
}
```

Commit:
```bash
git add desktop-ui/src/features/git/hooks/useGitHubIssues.ts
git commit -m "refactor(desktop-ui): migrate useGitHubIssues to useTauriQuery"
```

- [ ] **Step F9b: useGitHubPullRequests**

```ts
import { useCallback } from "react";
import type { GitHubPullRequest, WorkspaceInfo } from "@/types";
import { getGitHubPullRequests } from "@services/tauri";
import { qk, useTauriQuery } from "@/lib/query";

export function useGitHubPullRequests(
	activeWorkspace: WorkspaceInfo | null,
) {
	const workspaceId = activeWorkspace?.id ?? "";

	const query = useTauriQuery<{
		pullRequests: GitHubPullRequest[];
		total: number;
	}>({
		queryKey: qk.github.pulls(workspaceId),
		queryFn: async () => {
			if (!activeWorkspace) return { pullRequests: [], total: 0 };
			return await getGitHubPullRequests(activeWorkspace.id);
		},
		fallback: { pullRequests: [], total: 0 },
		enabled: activeWorkspace !== null,
	});

	const refresh = useCallback(async () => {
		await query.refetch();
	}, [query]);

	return {
		pullRequests: query.data.pullRequests,
		total: query.data.total,
		isLoading: query.isLoading,
		error: query.error == null ? null : String(query.error),
		refresh,
	};
}
```

Commit:
```bash
git add desktop-ui/src/features/git/hooks/useGitHubPullRequests.ts
git commit -m "refactor(desktop-ui): migrate useGitHubPullRequests to useTauriQuery"
```

- [ ] **Step F9c: useGitHubPullRequestDiffs**

```ts
import { useCallback } from "react";
import type { GitHubPullRequestDiff, WorkspaceInfo } from "@/types";
import { getGitHubPullRequestDiff } from "@services/tauri";
import { qk, useTauriQuery } from "@/lib/query";

export function useGitHubPullRequestDiffs(
	activeWorkspace: WorkspaceInfo | null,
	prNumber: number | null,
) {
	const workspaceId = activeWorkspace?.id ?? "";

	const query = useTauriQuery<GitHubPullRequestDiff[]>({
		queryKey: qk.github.diffsForPr(workspaceId, prNumber ?? -1),
		queryFn: async () => {
			if (!activeWorkspace || prNumber == null) return [];
			return await getGitHubPullRequestDiff(activeWorkspace.id, prNumber);
		},
		fallback: [],
		enabled: activeWorkspace !== null && prNumber !== null,
	});

	const refresh = useCallback(async () => {
		await query.refetch();
	}, [query]);

	return {
		diffs: query.data,
		isLoading: query.isLoading,
		error: query.error == null ? null : String(query.error),
		refresh,
	};
}
```

Commit:
```bash
git add desktop-ui/src/features/git/hooks/useGitHubPullRequestDiffs.ts
git commit -m "refactor(desktop-ui): migrate useGitHubPullRequestDiffs to useTauriQuery"
```

- [ ] **Step F9d: useGitHubPullRequestComments**

```ts
import { useCallback } from "react";
import type { GitHubPullRequestComment, WorkspaceInfo } from "@/types";
import { getGitHubPullRequestComments } from "@services/tauri";
import { qk, useTauriQuery } from "@/lib/query";

export function useGitHubPullRequestComments(
	activeWorkspace: WorkspaceInfo | null,
	prNumber: number | null,
) {
	const workspaceId = activeWorkspace?.id ?? "";

	const query = useTauriQuery<GitHubPullRequestComment[]>({
		queryKey: qk.github.commentsForPr(workspaceId, prNumber ?? -1),
		queryFn: async () => {
			if (!activeWorkspace || prNumber == null) return [];
			return await getGitHubPullRequestComments(
				activeWorkspace.id,
				prNumber,
			);
		},
		fallback: [],
		enabled: activeWorkspace !== null && prNumber !== null,
	});

	const refresh = useCallback(async () => {
		await query.refetch();
	}, [query]);

	return {
		comments: query.data,
		isLoading: query.isLoading,
		error: query.error == null ? null : String(query.error),
		refresh,
	};
}
```

Commit:
```bash
git add desktop-ui/src/features/git/hooks/useGitHubPullRequestComments.ts
git commit -m "refactor(desktop-ui): migrate useGitHubPullRequestComments to useTauriQuery"
```

---

### Task F10: Migrate `useGitCommitController.ts`

**Current state:** 13 useStates (commit message + 7 loading flags + 5 error flags). 7 mutations: commit, generateCommitMessage, fetch, pull, push, stageAll, sync. The commit message itself is local form state.

**Files:**
- Modify: `desktop-ui/src/features/app/hooks/useGitCommitController.ts`

- [ ] **Step 1: Rewrite the data flow portion (the mutations)**

Inside `useGitCommitController({ activeWorkspace, ... })`, replace the loading/error `useState`s and bare-promise calls with mutations:

```ts
const workspaceId = activeWorkspace?.id ?? "";

const invalidateAll = [
	qk.git.status(workspaceId),
	qk.git.diffs(workspaceId),
	qk.git.log(workspaceId),
	qk.git.branches(workspaceId),
];

const commit = useTauriMutation<void, { message: string }>({
	mutationFn: ({ message }) => {
		if (!activeWorkspace) throw new Error("no workspace");
		return commitGit(activeWorkspace.id, message);
	},
	invalidates: invalidateAll,
});

const fetchMut = useTauriMutation<void, void>({
	mutationFn: () => {
		if (!activeWorkspace) throw new Error("no workspace");
		return fetchGit(activeWorkspace.id);
	},
	invalidates: invalidateAll,
});

const pull = useTauriMutation<void, void>({
	mutationFn: () => {
		if (!activeWorkspace) throw new Error("no workspace");
		return pullGit(activeWorkspace.id);
	},
	invalidates: invalidateAll,
});

const push = useTauriMutation<void, void>({
	mutationFn: () => {
		if (!activeWorkspace) throw new Error("no workspace");
		return pushGit(activeWorkspace.id);
	},
	invalidates: invalidateAll,
});

const sync = useTauriMutation<void, void>({
	mutationFn: () => {
		if (!activeWorkspace) throw new Error("no workspace");
		return syncGit(activeWorkspace.id);
	},
	invalidates: invalidateAll,
});

const stageAll = useTauriMutation<void, void>({
	mutationFn: () => {
		if (!activeWorkspace) throw new Error("no workspace");
		return stageGitAll(activeWorkspace.id);
	},
	invalidates: invalidateAll,
});

const generateMessage = useTauriMutation<
	string,
	{ modelId: string | null }
>({
	mutationFn: ({ modelId }) => {
		if (!activeWorkspace) throw new Error("no workspace");
		return generateCommitMessage(activeWorkspace.id, modelId);
	},
	invalidates: [],
});
```

- [ ] **Step 2: Keep `commitMessage` as local state**

```ts
const [commitMessage, setCommitMessage] = useState("");
const [commitMessageError, setCommitMessageError] = useState<string | null>(
	null,
);
```

- [ ] **Step 3: Wire callbacks**

```ts
const onCommitMessageChange = useCallback((value: string) => {
	setCommitMessage(value);
}, []);

const onGenerateCommitMessage = useCallback(async () => {
	setCommitMessageError(null);
	try {
		const result = await generateMessage.mutate({
			modelId: appSettings.commitMessageModelId,
		});
		setCommitMessage(result);
	} catch (e) {
		setCommitMessageError(String(e));
	}
}, [generateMessage, appSettings.commitMessageModelId]);

const onCommit = useCallback(async () => {
	await commit.mutate({ message: commitMessage });
	setCommitMessage("");
}, [commit, commitMessage]);

const onCommitAndPush = useCallback(async () => {
	await commit.mutate({ message: commitMessage });
	await push.mutate();
	setCommitMessage("");
}, [commit, push, commitMessage]);

const onCommitAndSync = useCallback(async () => {
	await commit.mutate({ message: commitMessage });
	await sync.mutate();
	setCommitMessage("");
}, [commit, sync, commitMessage]);
```

- [ ] **Step 4: Map return shape**

```ts
return {
	commitMessage,
	commitMessageLoading: generateMessage.isLoading,
	commitMessageError,
	commitLoading: commit.isLoading,
	pullLoading: pull.isLoading,
	fetchLoading: fetchMut.isLoading,
	pushLoading: push.isLoading,
	syncLoading: sync.isLoading,
	commitError: commit.error == null ? null : String(commit.error),
	pullError: pull.error == null ? null : String(pull.error),
	fetchError: fetchMut.error == null ? null : String(fetchMut.error),
	pushError: push.error == null ? null : String(push.error),
	syncError: sync.error == null ? null : String(sync.error),
	hasWorktreeChanges: hasWorktreeChanges, // existing derivation
	onCommitMessageChange,
	onGenerateCommitMessage,
	onCommit,
	onCommitAndPush,
	onCommitAndSync,
	onPull: () => pull.mutate(),
	onFetch: () => fetchMut.mutate(),
	onPush: () => push.mutate(),
	onSync: () => sync.mutate(),
};
```

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/app/hooks/useGitCommitController.ts
git commit -m "refactor(desktop-ui): migrate useGitCommitController to TQ mutations"
```

---

### Task F11: Simplify `useGitHubPanelController.ts`

**Current state (inventory):** Pure state aggregator — no IPC. Holds 4 state slices and 4 setters that the GitHub child hooks call. Now that those hooks own their TQ caches, the controller's `useState` slices become read-throughs.

**Files:**
- Modify: `desktop-ui/src/features/app/hooks/useGitHubPanelController.ts`

- [ ] **Step 1: Rewrite as a TQ-cache reader**

```ts
import { useCallback, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { qk } from "@/lib/query";
import type {
	GitHubIssue,
	GitHubPullRequest,
	GitHubPullRequestComment,
	GitHubPullRequestDiff,
	WorkspaceInfo,
} from "@/types";

export function useGitHubPanelController(
	activeWorkspace: WorkspaceInfo | null,
) {
	const queryClient = useQueryClient();
	const workspaceId = activeWorkspace?.id ?? "";

	const [selectedPrNumber, setSelectedPrNumber] = useState<number | null>(
		null,
	);

	const issues =
		queryClient.getQueryData<{
			issues: GitHubIssue[];
			total: number;
		}>(qk.github.issues(workspaceId)) ?? { issues: [], total: 0 };

	const pullRequests =
		queryClient.getQueryData<{
			pullRequests: GitHubPullRequest[];
			total: number;
		}>(qk.github.pulls(workspaceId)) ?? { pullRequests: [], total: 0 };

	const diffs = selectedPrNumber
		? queryClient.getQueryData<GitHubPullRequestDiff[]>(
				qk.github.diffsForPr(workspaceId, selectedPrNumber),
			) ?? []
		: [];

	const comments = selectedPrNumber
		? queryClient.getQueryData<GitHubPullRequestComment[]>(
				qk.github.commentsForPr(workspaceId, selectedPrNumber),
			) ?? []
		: [];

	const resetGitHubPanelState = useCallback(() => {
		setSelectedPrNumber(null);
	}, []);

	return {
		gitIssues: issues.issues,
		gitIssuesTotal: issues.total,
		gitIssuesLoading: false,
		gitIssuesError: null,
		gitPullRequests: pullRequests.pullRequests,
		gitPullRequestsTotal: pullRequests.total,
		gitPullRequestsLoading: false,
		gitPullRequestsError: null,
		gitPullRequestDiffs: diffs,
		gitPullRequestDiffsLoading: false,
		gitPullRequestDiffsError: null,
		gitPullRequestComments: comments,
		gitPullRequestCommentsLoading: false,
		gitPullRequestCommentsError: null,
		// Setters become noops — child hooks now write directly to the cache.
		// Kept for back-compat with caller signatures.
		handleGitIssuesChange: () => {},
		handleGitPullRequestsChange: () => {},
		handleGitPullRequestDiffsChange: () => {},
		handleGitPullRequestCommentsChange: () => {},
		resetGitHubPanelState,
		selectedPrNumber,
		setSelectedPrNumber,
	};
}
```

- [ ] **Step 2: Update caller in `useMainAppGitState.ts`**

```bash
cd desktop-ui && grep -n "useGitHubPanelController\|handleGitIssuesChange\|handleGitPullRequestsChange" src/features/app/hooks/useMainAppGitState.ts
```

For each `handleGitIssuesChange`/etc. caller, drop the calls — the TQ caches are now the source of truth, so no manual reflection is needed.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/app/hooks/useGitHubPanelController.ts desktop-ui/src/features/app/hooks/useMainAppGitState.ts
git commit -m "refactor(desktop-ui): simplify useGitHubPanelController to read TQ cache"
```

---

## Phase G — Cleanup

### Task G1: Tray-sync hooks subscribe to query cache

**Current state (inventory):** `useTrayRecentThreads` and `useTraySessionUsage` derive tray menu data from props (`workspaces`, `threadsByWorkspace`, etc.) and call `setTrayRecentThreads(entries)` / `setTraySessionUsage(usage)` in an effect. Those props are themselves prop-drilled from `MainApp`. After Phase 2, threads live in the TQ cache (`qk.threads.list()`) — but in this codebase threads aren't yet in TQ (`MainApp` still uses Redux/local state for them). So the migration is **out of scope** for this plan; leave these two hooks alone for now.

- [ ] **Step 1: Document why these are deferred**

At the top of `desktop-ui/src/features/app/hooks/useTrayRecentThreads.ts` add:

```ts
// NOTE: This hook receives `workspaces` + `threadsByWorkspace` as props from
// MainApp.tsx, which still owns those slices via local state. When threads
// are migrated to TanStack Query (a follow-up plan after the chat feature
// migrates), this hook should be rewritten as a queryClient.getQueryCache()
// subscriber that listens for `qk.threads.list()` updates and calls
// setTrayRecentThreads from the cache. Until then it stays as a prop-driven
// effect.
```

Same comment in `useTraySessionUsage.ts`.

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/app/hooks/useTrayRecentThreads.ts desktop-ui/src/features/app/hooks/useTraySessionUsage.ts
git commit -m "docs(desktop-ui): document tray-sync hooks deferred until threads migrate"
```

---

### Task G2: Composer + messages sweep

**Inventory result:** zero `await ipc(` or `await invoke(` calls in `src/features/composer/` or `src/features/messages/`. Both features already route everything through typed `@services/tauri` wrappers. There is nothing to migrate in this plan.

- [ ] **Step 1: Verify**

```bash
cd desktop-ui && grep -rnE "await ipc\(|await invoke\(" src/features/composer src/features/messages
```

Expected: no output.

- [ ] **Step 2: No commit needed.** Skip.

---

### Task G3: Final regression sweep

**Files:** none modified — pure verification.

- [ ] **Step 1: Run all tests**

```bash
cd desktop-ui && bun run test 2>&1 | tail -10
```

Expected: query layer + tray + any test we updated all pass. Pre-existing unrelated failures (28 from Phase 1 audit) may persist — verify count hasn't grown.

- [ ] **Step 2: Typecheck**

```bash
cd desktop-ui && bunx tsc --noEmit 2>&1 | tail -10 && echo "---DONE---"
```

Expected: `---DONE---` only.

- [ ] **Step 3: Lint**

```bash
cd desktop-ui && bun run lint:fix
```

Then:
```bash
cd desktop-ui && git diff --stat
```

If `lint:fix` reformatted anything, commit:

```bash
git add -u
git commit -m "chore(desktop-ui): biome formatting after Phase 2 migration"
```

- [ ] **Step 4: Manual cross-window verification**

```bash
cd desktop-ui && bun run dev &
cargo tauri dev
```

Open all four windows (main, launcher, tray, distraction). Verify:
- Toggle a task in the tray → main task list updates within ~200ms.
- Run `git status` from the terminal in another shell, then click "refresh" in the git panel → status updates.
- Trigger a focus session start from the launcher → tray timer reflects, dnd chip lights up.
- Settings updates: change a feature flag, verify the change persists across reload.
- Open React Query devtools (bottom-left floating "TanStack" button) in each window. Verify queries:
  - main window: `tasks.today`, `git.status.<wsId>`, `git.diffs.<wsId>`, `git.log.<wsId>`, `git.branches.<wsId>`, `models.list.<wsId>`, `skills.list.<wsId>`, `apps.list.<wsId>.<threadId>`, `prompts.list.<wsId>`, `settings.app`, `agents.settings`
  - launcher: `launcher.dashboard`, `launcher.dndActive`, `launcher.search.<q>`, `flashcards.dueCount`
  - tray: same as Phase 1 (tasks, calendar, focus.status, focus.todaySessions)
  - distraction: empty (no queries; only mutations + transient state)

- [ ] **Step 5: Final integrity commit**

If anything cosmetic was tweaked, commit. Otherwise, no diff:

```bash
cd /Users/jayden/Projects/Klynt/bot && git status
```

Expected: clean.

---

## Self-Review

**1. Spec coverage:**
- Foundation: queryFn/mutationFn escape hatches → A1, A2 ✓
- queryKeys extension → A3 ✓
- Tauri bridge new routes (chat/mcp/productivity, focus:state_changed → dndActive) → A4 ✓
- App-server WebSocket bridge → A5 ✓
- Launcher migration (5 files in scope; useExecuteItem deferred with rationale) → B1–B6 ✓
- Distraction overlay → C1 ✓
- Settings (5 hooks) → D1–D5 ✓
- Registries (models, skills, apps, prompts) → E1–E4 ✓
- Git: 5 git + 4 GitHub + commit/actions controllers + panel controller (12 files) → F1–F11 ✓
- Tray-sync deferred with rationale → G1 ✓
- Composer/messages already clean (verified empty) → G2 ✓
- Final regression sweep → G3 ✓

**2. Placeholder scan:**
- D1 step 2 has the "queryClient = save" placeholder — explicitly fixed in step 3.
- A few "verify exact handler shapes" instructions — these aren't placeholders, they're file-reads to confirm names. Acceptable.
- No "TBD"/"TODO"/"add error handling" anywhere.

**3. Type consistency:**
- `qk.git.*(workspaceId)` and `qk.models.*(workspaceId)` consistently take `workspaceId: string` (callers pass `activeWorkspace?.id ?? ""`).
- `qk.apps.list(workspaceId, threadId)` takes both — caller uses `activeThreadId: string | null`.
- `qk.github.*(workspaceId, prNumber?)` consistent across F9.
- Mutation invalidation arrays `invalidatesGit` reused identically across F2, F8, F10.
- `useTauriQuery<T>({ queryKey, queryFn?, command?, args?, fallback?, enabled?, staleTime? })` used identically in every Phase D/E/F task.
- `useTauriMutation<TData, TVars>({ command? | mutationFn?, invalidates?, optimistic?, onError?, onSuccess? })` used identically in Phase D, E, F.
- All entity-kind aware behaviors (auto-invalidation by command prefix) preserved from Plan 1.

---

## Definition of Done (Plan 2)

- All 30 tasks committed (A1–A5, B1–B6, C1, D1–D5, E1–E4, F1–F11, G1–G3).
- `bun run test` green for `src/lib/query/**` and any test we updated; pre-existing 28 failures from Phase 1 audit unchanged.
- `bunx tsc --noEmit` clean.
- `grep -rE "await ipc\(|await invoke\(" desktop-ui/src/features` returns only valid one-off side effects (`useExecuteItem.ts`, `Tray.tsx`'s `show_dashboard`, the api/endpoints layer).
- React Query devtools shows queries from launcher, settings, agents, models, skills, apps, prompts, git (status/branches/diffs/log/remote/commitDiffs/repoScan), github (issues/pulls/diffs/comments), and the original Phase 1 tray queries.
- Cross-window mutations propagate within ~200ms (same window) or one event-loop tick (other windows).

---

## Out-of-scope (Plans 3–4)

- MCP child process socket bridge (Plan 3) — required for Claude Code edits to invalidate FE caches in real-time.
- Distiller domain events (Plan 4) — required for `episodic_memories`/`semantic_facts` writes to flow through the bus.
- Threads migration to TanStack Query — depends on chat feature refactor (separate plan).
- `useExecuteItem` factoring (Phase B6 deferred) — only worth doing if launcher gains data-mutating actions.
