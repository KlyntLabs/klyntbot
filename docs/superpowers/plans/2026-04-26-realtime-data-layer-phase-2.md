# Real-time Data Layer Phase 2 — Migrate Remaining FE Features

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Migrate every remaining `desktop-ui` feature off ad-hoc `useState + ipc` patterns onto the `@/lib/query` foundation built in Plan 1. After this plan, every data-fetching surface (launcher, distraction, settings, models/skills/apps/prompts registries, git panel, threads, composer) participates in cross-window real-time invalidation.

**Architecture:** Same as Plan 1 — `useTauriQuery` / `useTauriMutation` / `qk` from `@/lib/query`. New query keys added to `queryKeys.ts` as features are migrated. New invalidation routes added to `tauriEventBridge.ts` for events that didn't matter for the tray. **No new infrastructure** — pure migration.

**Tech Stack:** Existing `@tanstack/react-query` v5 foundation. Vitest for new tests.

**Master plan context:** Plan 2 of 4. Depends on Plan 1 (foundation + tray migrated). Plans 3-4 cover the Rust-side MCP bridge + Distiller events.

---

## File Structure

### Files to extend

| Path | Change |
|---|---|
| `desktop-ui/src/lib/query/queryKeys.ts` | Add domains: `launcher`, `distraction`, `settings`, `models`, `skills`, `apps`, `prompts`, `git`, `github`, `threads`. |
| `desktop-ui/src/lib/query/tauriEventBridge.ts` | Add static routes for `chat:thread_*`, `chat:message_added`, `mcp:server_status`, `mcp:startup_complete`, `productivity:nudge`, `score:updated`, `bucket:completed`. |
| `desktop-ui/src/lib/query/entityKindMap.ts` | Add `note_`, `notebook_` prefixes if missing — verify Plan 1 left them in place. |

### Files to migrate (per phase)

**Phase B (launcher):**
- `desktop-ui/src/features/launcher/hooks/useDashboardData.ts`
- `desktop-ui/src/features/launcher/hooks/useDndActive.ts`
- `desktop-ui/src/features/launcher/hooks/useLauncherSearch.ts`
- `desktop-ui/src/features/launcher/hooks/useExecuteItem.ts`
- `desktop-ui/src/features/launcher/components/ActionMenu.tsx`
- `desktop-ui/src/features/launcher/components/FocusActiveChip.tsx`

**Phase C (distraction):**
- `desktop-ui/src/features/distraction/components/DistractionOverlay.tsx`

**Phase D (settings):**
- `desktop-ui/src/features/settings/hooks/useAppSettings.ts`
- `desktop-ui/src/features/settings/hooks/useSettingsAgentsSection.ts`
- `desktop-ui/src/features/settings/hooks/useSettingsDefaultModels.ts`
- `desktop-ui/src/features/settings/hooks/useSettingsFeaturesSection.ts`
- `desktop-ui/src/features/settings/hooks/useSettingsServerSection.ts`

**Phase E (registries):**
- `desktop-ui/src/features/models/hooks/useModels.ts`
- `desktop-ui/src/features/skills/hooks/useSkills.ts`
- `desktop-ui/src/features/apps/hooks/useApps.ts`
- `desktop-ui/src/features/prompts/hooks/useCustomPrompts.ts`

**Phase F (git):**
- `desktop-ui/src/features/git/hooks/useGitStatus.ts`
- `desktop-ui/src/features/git/hooks/useGitBranches.ts`
- `desktop-ui/src/features/git/hooks/useGitDiffs.ts`
- `desktop-ui/src/features/git/hooks/useGitLog.ts`
- `desktop-ui/src/features/git/hooks/useGitRemote.ts`
- `desktop-ui/src/features/git/hooks/useGitHubIssues.ts`
- `desktop-ui/src/features/git/hooks/useGitHubPullRequests.ts`
- `desktop-ui/src/features/git/hooks/useGitHubPullRequestDiffs.ts`
- `desktop-ui/src/features/git/hooks/useGitHubPullRequestComments.ts`
- `desktop-ui/src/features/git/hooks/useGitHubPanelController.ts`

**Phase G (composer & sync hooks):**
- `desktop-ui/src/features/app/hooks/useTrayRecentThreads.ts` (write-only sync — keep, but route through cache subscriber)
- `desktop-ui/src/features/app/hooks/useTraySessionUsage.ts` (same)
- `desktop-ui/src/features/composer/**/*` (audit `invoke`/`ipc` usage)

---

## Phase A — Foundation extensions

### Task A1: Extend `queryKeys.ts` with new domains

**Files:**
- Modify: `desktop-ui/src/lib/query/queryKeys.ts`
- Modify: `desktop-ui/src/lib/query/tests/queryKeys.test.ts`

- [ ] **Step 1: Add tests for the new keys**

Append to `tests/queryKeys.test.ts`:

```ts
describe("queryKeys — phase 2 domains", () => {
	it("launcher.dashboard is stable", () => {
		expect(qk.launcher.dashboard()).toEqual(["launcher", "dashboard"]);
	});
	it("launcher.search encodes query", () => {
		expect(qk.launcher.search("hi")).toEqual(["launcher", "search", "hi"]);
	});
	it("launcher.dndActive is stable", () => {
		expect(qk.launcher.dndActive()).toEqual(["launcher", "dndActive"]);
	});
	it("settings.app", () => {
		expect(qk.settings.app()).toEqual(["settings", "app"]);
	});
	it("settings.agents", () => {
		expect(qk.settings.agents()).toEqual(["settings", "agents"]);
	});
	it("settings.defaultModels", () => {
		expect(qk.settings.defaultModels()).toEqual([
			"settings",
			"defaultModels",
		]);
	});
	it("settings.features", () => {
		expect(qk.settings.features()).toEqual(["settings", "features"]);
	});
	it("settings.server", () => {
		expect(qk.settings.server()).toEqual(["settings", "server"]);
	});
	it("models.list / models.config", () => {
		expect(qk.models.list()).toEqual(["models", "list"]);
		expect(qk.models.configModel()).toEqual(["models", "configModel"]);
	});
	it("skills.list", () => {
		expect(qk.skills.list()).toEqual(["skills", "list"]);
	});
	it("apps.list", () => {
		expect(qk.apps.list()).toEqual(["apps", "list"]);
	});
	it("prompts.list", () => {
		expect(qk.prompts.list()).toEqual(["prompts", "list"]);
	});
	it("git.status / branches / diffs / log / remote", () => {
		expect(qk.git.status()).toEqual(["git", "status"]);
		expect(qk.git.branches()).toEqual(["git", "branches"]);
		expect(qk.git.diffs()).toEqual(["git", "diffs"]);
		expect(qk.git.log()).toEqual(["git", "log"]);
		expect(qk.git.remote()).toEqual(["git", "remote"]);
	});
	it("github.issues / pulls / diffsForPr / commentsForPr", () => {
		expect(qk.github.issues()).toEqual(["github", "issues"]);
		expect(qk.github.pulls()).toEqual(["github", "pulls"]);
		expect(qk.github.diffsForPr(42)).toEqual([
			"github",
			"pulls",
			42,
			"diffs",
		]);
		expect(qk.github.commentsForPr(42)).toEqual([
			"github",
			"pulls",
			42,
			"comments",
		]);
	});
	it("threads.list / threads.byId", () => {
		expect(qk.threads.list()).toEqual(["threads", "list"]);
		expect(qk.threads.byId("abc")).toEqual(["threads", "byId", "abc"]);
	});
});
```

- [ ] **Step 2: Run failing tests**

```bash
cd desktop-ui && bun run test src/lib/query/tests/queryKeys.test.ts
```

Expected: all new tests fail.

- [ ] **Step 3: Extend the factory**

Append to `desktop-ui/src/lib/query/queryKeys.ts` inside the `qk` object before the closing brace:

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
		agents: () => ["settings", "agents"] as const,
		defaultModels: () => ["settings", "defaultModels"] as const,
		features: () => ["settings", "features"] as const,
		server: () => ["settings", "server"] as const,
	},
	models: {
		all: () => ["models"] as const,
		list: () => ["models", "list"] as const,
		configModel: () => ["models", "configModel"] as const,
	},
	skills: {
		all: () => ["skills"] as const,
		list: () => ["skills", "list"] as const,
	},
	apps: {
		all: () => ["apps"] as const,
		list: () => ["apps", "list"] as const,
	},
	prompts: {
		all: () => ["prompts"] as const,
		list: () => ["prompts", "list"] as const,
	},
	git: {
		all: () => ["git"] as const,
		status: () => ["git", "status"] as const,
		branches: () => ["git", "branches"] as const,
		diffs: () => ["git", "diffs"] as const,
		log: () => ["git", "log"] as const,
		remote: () => ["git", "remote"] as const,
	},
	github: {
		all: () => ["github"] as const,
		issues: () => ["github", "issues"] as const,
		pulls: () => ["github", "pulls"] as const,
		diffsForPr: (n: number) => ["github", "pulls", n, "diffs"] as const,
		commentsForPr: (n: number) => ["github", "pulls", n, "comments"] as const,
	},
	threads: {
		all: () => ["threads"] as const,
		list: () => ["threads", "list"] as const,
		byId: (id: string) => ["threads", "byId", id] as const,
	},
```

- [ ] **Step 4: Run tests — expect green**

```bash
cd desktop-ui && bun run test src/lib/query/tests/queryKeys.test.ts
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/lib/query/queryKeys.ts desktop-ui/src/lib/query/tests/queryKeys.test.ts
git commit -m "feat(desktop-ui): extend queryKeys for launcher/settings/git/threads/registries"
```

---

### Task A2: Extend `tauriEventBridge.ts` with new routes

**Files:**
- Modify: `desktop-ui/src/lib/query/tauriEventBridge.ts`
- Modify: `desktop-ui/src/lib/query/tests/tauriEventBridge.test.ts`

- [ ] **Step 1: Add tests**

Append to `tests/tauriEventBridge.test.ts`:

```ts
it("chat:message_added invalidates threads.list", async () => {
	const client = new QueryClient();
	const spy = vi.spyOn(client, "invalidateQueries");
	const { listen, fire } = fakeListenFactory();
	const stop = await startTauriEventBridge(client, listen);
	fire("chat:message_added", { sessionKey: "s1", source: "user" });
	expect(spy).toHaveBeenCalledWith({ queryKey: qk.threads.list() });
	stop();
});

it("mcp:server_status invalidates settings.server", async () => {
	const client = new QueryClient();
	const spy = vi.spyOn(client, "invalidateQueries");
	const { listen, fire } = fakeListenFactory();
	const stop = await startTauriEventBridge(client, listen);
	fire("mcp:server_status", { serverName: "x", status: "ready" });
	expect(spy).toHaveBeenCalledWith({ queryKey: qk.settings.server() });
	stop();
});

it("score:updated invalidates launcher.dashboard", async () => {
	const client = new QueryClient();
	const spy = vi.spyOn(client, "invalidateQueries");
	const { listen, fire } = fakeListenFactory();
	const stop = await startTauriEventBridge(client, listen);
	fire("score:updated", { score: 0.8, productiveSecs: 100, distractingSecs: 5 });
	expect(spy).toHaveBeenCalledWith({ queryKey: qk.launcher.dashboard() });
	stop();
});
```

- [ ] **Step 2: Run failing tests**

```bash
cd desktop-ui && bun run test src/lib/query/tests/tauriEventBridge.test.ts
```

Expected: 3 new tests fail.

- [ ] **Step 3: Extend `STATIC_ROUTES`**

In `desktop-ui/src/lib/query/tauriEventBridge.ts`, replace `STATIC_ROUTES` with:

```ts
const STATIC_ROUTES: ReadonlyArray<readonly [string, QueryKey[]]> = [
	["focus:state_changed", [qk.focus.status()]],
	["focus:phase_changed", [qk.focus.status()]],
	["focus:sync", [qk.focus.status()]],
	["chat:thread_created", [qk.threads.list()]],
	["chat:thread_updated", [qk.threads.list()]],
	["chat:message_added", [qk.threads.list()]],
	["mcp:server_status", [qk.settings.server()]],
	["mcp:startup_complete", [qk.settings.server()]],
	["productivity:nudge", [qk.launcher.dashboard()]],
	["score:updated", [qk.launcher.dashboard()]],
	["bucket:completed", [qk.launcher.dashboard()]],
];
```

- [ ] **Step 4: Run tests — expect green**

```bash
cd desktop-ui && bun run test src/lib/query/tests/tauriEventBridge.test.ts
```

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/lib/query/tauriEventBridge.ts desktop-ui/src/lib/query/tests/tauriEventBridge.test.ts
git commit -m "feat(desktop-ui): route chat/mcp/productivity events to query invalidations"
```

---

## Phase B — Launcher migration

### Task B1: Migrate `useDashboardData.ts`

**Files:**
- Modify: `desktop-ui/src/features/launcher/hooks/useDashboardData.ts`

The current hook polls every 30 s; with the bridge, it gets real-time invalidation from `score:updated`/`bucket:completed`/`productivity:nudge`. Drop the polling.

- [ ] **Step 1: Read the current implementation to find what command/types it uses**

```bash
cd desktop-ui && cat src/features/launcher/hooks/useDashboardData.ts
```

- [ ] **Step 2: Rewrite the hook**

Replace the entire file with:

```ts
import { qk, useTauriQuery } from "@/lib/query";

interface DashboardData {
	// Re-use whatever shape the BE returns; preserve from prior file.
	[key: string]: unknown;
}

export function useDashboardData() {
	return useTauriQuery<DashboardData | null>({
		queryKey: qk.launcher.dashboard(),
		command: "launcher_dashboard",
		fallback: null,
	});
}
```

If the file currently exports a `DashboardData` type, preserve the original type definition verbatim.

- [ ] **Step 3: Update callers — they previously consumed via Zustand**

```bash
cd desktop-ui && grep -rn "useDashboardData\|setDashboard" src/features/launcher
```

For each caller, switch from the Zustand-store accessor to the hook return. Specifically: components that read `state.dashboard` should now read the hook's `data`. Delete `setDashboard` from the launcher store if no other caller writes to it.

- [ ] **Step 4: Typecheck**

```bash
cd desktop-ui && bunx tsc --noEmit 2>&1 | grep -E "launcher|useDashboardData" | head -10
```

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/launcher
git commit -m "refactor(desktop-ui): migrate launcher dashboard to useTauriQuery"
```

---

### Task B2: Migrate `useDndActive.ts`

**Files:**
- Modify: `desktop-ui/src/features/launcher/hooks/useDndActive.ts`

Drop the 2-second poll; use focus event invalidation instead.

- [ ] **Step 1: Rewrite**

Replace the file body with:

```ts
import { qk, useTauriQuery } from "@/lib/query";
import type { FocusSession } from "@/features/tray/types";

export function useDndActive() {
	return useTauriQuery<FocusSession | null>({
		queryKey: qk.launcher.dndActive(),
		command: "focus_active",
		args: { mode: "dnd" },
		fallback: null,
	});
}
```

- [ ] **Step 2: Add invalidation route**

In `tauriEventBridge.ts`, append to `STATIC_ROUTES`:

```ts
["focus:state_changed", [qk.focus.status(), qk.launcher.dndActive()]],
```

(Replace the existing line that only invalidates `qk.focus.status()`.)

- [ ] **Step 3: Update test in `tauriEventBridge.test.ts` accordingly** — the existing `focus:phase_changed` test still passes, but add:

```ts
it("focus:state_changed also invalidates launcher.dndActive", async () => {
	const client = new QueryClient();
	const spy = vi.spyOn(client, "invalidateQueries");
	const { listen, fire } = fakeListenFactory();
	const stop = await startTauriEventBridge(client, listen);
	fire("focus:state_changed", { state: "active" });
	expect(spy).toHaveBeenCalledWith({ queryKey: qk.launcher.dndActive() });
	stop();
});
```

- [ ] **Step 4: Run tests**

```bash
cd desktop-ui && bun run test src/lib/query
```

Expected: green.

- [ ] **Step 5: Typecheck + commit**

```bash
cd desktop-ui && bunx tsc --noEmit 2>&1 | tail -5 && echo "---DONE---"
git add desktop-ui/src/features/launcher/hooks/useDndActive.ts desktop-ui/src/lib/query
git commit -m "refactor(desktop-ui): migrate useDndActive; route focus:state_changed"
```

---

### Task B3: Migrate `useLauncherSearch.ts`

**Files:**
- Modify: `desktop-ui/src/features/launcher/hooks/useLauncherSearch.ts`

The current hook reimplements debounce + version counter. Replace with `useTauriQuery` keyed by query string + a debounced query setter. TanStack Query handles the rest (dedup, in-flight reuse).

- [ ] **Step 1: Rewrite**

```ts
import { useEffect, useState } from "react";
import { qk, useTauriQuery } from "@/lib/query";
import type { LauncherItem } from "../types";

const DEBOUNCE_MS = 80;

export function useLauncherSearch(rawQuery: string) {
	const [debounced, setDebounced] = useState(rawQuery);
	useEffect(() => {
		const t = setTimeout(() => setDebounced(rawQuery), DEBOUNCE_MS);
		return () => clearTimeout(t);
	}, [rawQuery]);

	return useTauriQuery<LauncherItem[]>({
		queryKey: qk.launcher.search(debounced),
		command: "launcher_search",
		args: { query: debounced },
		fallback: [],
		enabled: debounced.trim().length > 0,
		// Search results are inherently stale-fast; tighter staleTime so a
		// re-query within 5s reuses cache, but anything longer refetches.
		staleTime: 5_000,
	});
}
```

- [ ] **Step 2: Typecheck + commit**

```bash
cd desktop-ui && bunx tsc --noEmit 2>&1 | grep useLauncherSearch | head -5
git add desktop-ui/src/features/launcher/hooks/useLauncherSearch.ts
git commit -m "refactor(desktop-ui): migrate useLauncherSearch — TQ debounced query"
```

---

### Task B4: Migrate `useExecuteItem.ts` (the heavy one — 18+ ipc chains)

**Files:**
- Modify: `desktop-ui/src/features/launcher/hooks/useExecuteItem.ts`

This is **side-effect mutations**, not data fetches. The migration is mechanical: each `await ipc("foo", args)` becomes `await foo.mutate(args)` where `foo = useTauriMutation({ command: "foo" })`. Auto-invalidation handles cache freshness.

- [ ] **Step 1: List every command currently called**

```bash
cd desktop-ui && grep -nE 'await ipc\(' src/features/launcher/hooks/useExecuteItem.ts
```

Capture each command name into a list (call it `CMDS`).

- [ ] **Step 2: For each CMD, declare a `useTauriMutation` at the top of the hook**

Pattern:

```ts
const taskCreate = useTauriMutation<unknown, { title: string }>({
	command: "task_create",
});
// ... one per cmd
```

Place these declarations at the top of the hook (after `const queryClient = useQueryClient()` if needed).

- [ ] **Step 3: Replace each `await ipc(cmd, args)` with `await mutator.mutate(args)`**

Sed-style mechanical edit per line.

- [ ] **Step 4: For commands that don't fit any entity prefix** (e.g. `launcher_open_app`, `open_url`, `quit_app`, `show_dashboard`), add explicit `invalidates: []` to skip auto-invalidation:

```ts
const showDashboard = useTauriMutation<void, void>({
	command: "show_dashboard",
	invalidates: [],
});
```

- [ ] **Step 5: Typecheck**

```bash
cd desktop-ui && bunx tsc --noEmit 2>&1 | grep useExecuteItem | head -10
```

- [ ] **Step 6: Manual smoke test** — open the launcher, execute a few actions (open task, run cron, etc.). Verify no regressions.

- [ ] **Step 7: Commit**

```bash
git add desktop-ui/src/features/launcher/hooks/useExecuteItem.ts
git commit -m "refactor(desktop-ui): migrate useExecuteItem to useTauriMutation"
```

---

### Task B5: Migrate `ActionMenu.tsx` and `FocusActiveChip.tsx`

**Files:**
- Modify: `desktop-ui/src/features/launcher/components/ActionMenu.tsx`
- Modify: `desktop-ui/src/features/launcher/components/FocusActiveChip.tsx`

Direct `ipc()` calls in component bodies should become `useTauriMutation` declarations at the top of the component.

- [ ] **Step 1: For each `ipc(...)` call inside the component, hoist a mutation hook**

Example for `FocusActiveChip.tsx`:

```ts
const extend = useTauriMutation<void, { secs: number }>({ command: "focus_extend" });
const deactivate = useTauriMutation<void, void>({ command: "focus_deactivate" });

// then inside handlers:
const handleExtend = (secs: number) => extend.mutate({ secs });
const handleDeactivate = () => deactivate.mutate();
```

- [ ] **Step 2: Repeat for `ActionMenu.tsx`**

- [ ] **Step 3: Typecheck + commit**

```bash
cd desktop-ui && bunx tsc --noEmit 2>&1 | grep -E "ActionMenu|FocusActiveChip" | head
git add desktop-ui/src/features/launcher/components
git commit -m "refactor(desktop-ui): hoist launcher component ipc calls to useTauriMutation"
```

---

## Phase C — Distraction overlay

### Task C1: Migrate `DistractionOverlay.tsx`

**Files:**
- Modify: `desktop-ui/src/features/distraction/components/DistractionOverlay.tsx`

Three direct `ipc()` mutations: `distraction_dismiss`, `distraction_allow_temp`, `distraction_allow_session`.

- [ ] **Step 1: Add imports**

```ts
import { useTauriMutation } from "@/lib/query";
```

- [ ] **Step 2: Declare mutations near the top of the component**

```ts
const dismiss = useTauriMutation<void, { appName: string }>({
	command: "distraction_dismiss",
	invalidates: [], // distraction events drive their own state via useEvent
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

- [ ] **Step 3: Replace each `await ipc(...)` with `.mutate(...)`**

- [ ] **Step 4: Typecheck + commit**

```bash
cd desktop-ui && bunx tsc --noEmit 2>&1 | grep DistractionOverlay | head
git add desktop-ui/src/features/distraction/components/DistractionOverlay.tsx
git commit -m "refactor(desktop-ui): migrate distraction actions to useTauriMutation"
```

---

## Phase D — Settings

### Task D1: Migrate `useAppSettings.ts`

**Files:**
- Modify: `desktop-ui/src/features/settings/hooks/useAppSettings.ts`

- [ ] **Step 1: Identify the read + write commands**

```bash
cd desktop-ui && grep -nE "getAppSettings|updateAppSettings|invoke|ipc" src/features/settings/hooks/useAppSettings.ts
```

- [ ] **Step 2: Rewrite as a query + mutation pair**

```ts
import { qk, useTauriMutation, useTauriQuery } from "@/lib/query";
import type { AppSettings } from "../types";

const READ_CMD = "get_app_settings"; // verify exact cmd name
const WRITE_CMD = "update_app_settings";

export function useAppSettings() {
	const query = useTauriQuery<AppSettings | null>({
		queryKey: qk.settings.app(),
		command: READ_CMD,
		fallback: null,
	});
	const update = useTauriMutation<AppSettings, Partial<AppSettings>>({
		command: WRITE_CMD,
		invalidates: [qk.settings.app()],
	});
	return {
		settings: query.data,
		isLoading: query.isLoading,
		update: update.mutate,
		isUpdating: update.isLoading,
	};
}
```

- [ ] **Step 3: Audit callers** — `MainApp.tsx` and others previously got the result via prop drilling. Update to either consume the new return shape directly or wrap a small adapter.

- [ ] **Step 4: Typecheck + commit**

```bash
cd desktop-ui && bunx tsc --noEmit 2>&1 | grep useAppSettings | head
git add desktop-ui/src/features/settings/hooks/useAppSettings.ts
git commit -m "refactor(desktop-ui): migrate useAppSettings to query+mutation pair"
```

---

### Task D2: Migrate `useSettingsAgentsSection.ts`

**Files:**
- Modify: `desktop-ui/src/features/settings/hooks/useSettingsAgentsSection.ts`

This file holds 8 separate `useState` flags. Replace each operation with its own `useTauriMutation` (whose `isLoading` replaces the manual flags).

- [ ] **Step 1: Catalogue the operations**

```bash
cd desktop-ui && grep -nE "useState.*[Ll]oading|invoke|ipc" src/features/settings/hooks/useSettingsAgentsSection.ts
```

- [ ] **Step 2: Replace each fetch + each mutation**

Pattern — each "save provider" type operation becomes:

```ts
const saveProvider = useTauriMutation<void, ProviderInput>({
	command: "agents_save_provider",
	invalidates: [qk.settings.agents()],
});
// callsite: `await saveProvider.mutate(input)`; `saveProvider.isLoading` = `savingProvider`.
```

The initial `getAgentsSettings()` becomes a `useTauriQuery({ queryKey: qk.settings.agents(), command: "agents_get" })`.

- [ ] **Step 3: Delete now-unused `useState<boolean>` flags**

- [ ] **Step 4: Typecheck + commit**

```bash
cd desktop-ui && bunx tsc --noEmit 2>&1 | grep useSettingsAgentsSection | head
git add desktop-ui/src/features/settings/hooks/useSettingsAgentsSection.ts
git commit -m "refactor(desktop-ui): migrate settings agents section to query+mutations"
```

---

### Task D3: Migrate `useSettingsDefaultModels.ts`, `useSettingsFeaturesSection.ts`, `useSettingsServerSection.ts`

For each file, follow the same pattern as D1/D2. Each is its own task with its own commit, but the migration shape repeats.

- [ ] **Step D3a: useSettingsDefaultModels**

Migrate. Commit:
```bash
git add desktop-ui/src/features/settings/hooks/useSettingsDefaultModels.ts
git commit -m "refactor(desktop-ui): migrate useSettingsDefaultModels to TQ"
```

- [ ] **Step D3b: useSettingsFeaturesSection**

Migrate. Commit:
```bash
git add desktop-ui/src/features/settings/hooks/useSettingsFeaturesSection.ts
git commit -m "refactor(desktop-ui): migrate useSettingsFeaturesSection to TQ"
```

- [ ] **Step D3c: useSettingsServerSection**

This one has 15+ `useState`s. Each needs its own query or mutation. Take care — break the migration into separate commits if it gets too large in a single diff. End with:
```bash
git add desktop-ui/src/features/settings/hooks/useSettingsServerSection.ts
git commit -m "refactor(desktop-ui): migrate useSettingsServerSection to TQ"
```

---

## Phase E — Cross-cutting registries

### Task E1: Migrate `useModels.ts`

**Files:**
- Modify: `desktop-ui/src/features/models/hooks/useModels.ts`

This hook is high-fanout — multiple components subscribe simultaneously. Migrating to TQ gives instant dedup.

- [ ] **Step 1: Rewrite**

```ts
import { qk, useTauriQuery } from "@/lib/query";
import type { ModelOption } from "../types";

export function useModels() {
	const list = useTauriQuery<ModelOption[]>({
		queryKey: qk.models.list(),
		command: "model_list",
		fallback: [],
	});
	const configModel = useTauriQuery<string | null>({
		queryKey: qk.models.configModel(),
		command: "get_config_model",
		fallback: null,
	});
	return {
		models: list.data,
		configModel: configModel.data,
		isLoading: list.isLoading || configModel.isLoading,
	};
}
```

- [ ] **Step 2: Typecheck + commit**

```bash
git add desktop-ui/src/features/models/hooks/useModels.ts
git commit -m "refactor(desktop-ui): migrate useModels — single TQ source for high-fanout"
```

### Task E2: Migrate `useSkills.ts`, `useApps.ts`, `useCustomPrompts.ts`

Same shape as E1. Three commits:

- [ ] **E2a:** Migrate `useSkills.ts`. Commit `refactor(desktop-ui): migrate useSkills to useTauriQuery`.
- [ ] **E2b:** Migrate `useApps.ts`. Commit `refactor(desktop-ui): migrate useApps to useTauriQuery`.
- [ ] **E2c:** Migrate `useCustomPrompts.ts`. Commit `refactor(desktop-ui): migrate useCustomPrompts to useTauriQuery`.

---

## Phase F — Git panel

### Task F1: Migrate basic git hooks (status / branches / log / remote / diffs)

**Files:**
- Modify: `desktop-ui/src/features/git/hooks/useGitStatus.ts`
- Modify: `desktop-ui/src/features/git/hooks/useGitBranches.ts`
- Modify: `desktop-ui/src/features/git/hooks/useGitDiffs.ts`
- Modify: `desktop-ui/src/features/git/hooks/useGitLog.ts`
- Modify: `desktop-ui/src/features/git/hooks/useGitRemote.ts`

Each follows the same template:

```ts
import { qk, useTauriQuery } from "@/lib/query";
import type { GitStatusState } from "../types";

export function useGitStatus() {
	return useTauriQuery<GitStatusState | null>({
		queryKey: qk.git.status(),
		command: "git_status", // verify
		fallback: null,
	});
}
```

- [ ] **Step F1a: useGitStatus.** Commit individually.
- [ ] **Step F1b: useGitBranches.** Commit.
- [ ] **Step F1c: useGitDiffs.** Commit.
- [ ] **Step F1d: useGitLog.** Commit.
- [ ] **Step F1e: useGitRemote.** Commit.

### Task F2: Migrate GitHub hooks

**Files:**
- Modify: `desktop-ui/src/features/git/hooks/useGitHubIssues.ts`
- Modify: `desktop-ui/src/features/git/hooks/useGitHubPullRequests.ts`
- Modify: `desktop-ui/src/features/git/hooks/useGitHubPullRequestDiffs.ts`
- Modify: `desktop-ui/src/features/git/hooks/useGitHubPullRequestComments.ts`

For each: replace local `useState` with `useTauriQuery({ queryKey: qk.github.<...>, command: <...> })`. Keys with PR numbers use `qk.github.diffsForPr(n)` etc.

- [ ] **Step F2a-d:** one commit each.

### Task F3: Migrate `useGitHubPanelController.ts`

The controller orchestrates the four GitHub queries. After migration its state becomes a thin wrapper over the hook returns; many `useState` calls disappear.

- [ ] **Step 1:** Read controller file.
- [ ] **Step 2:** Replace each piece of state with the matching `useGitHub*` hook return.
- [ ] **Step 3:** Commit `refactor(desktop-ui): simplify useGitHubPanelController via TQ-backed hooks`.

### Task F4: Add git mutation invalidations

Git CLI commands (commit, push, pull, branch, checkout) mutate state. After each successful mutation, the relevant queries should refetch.

- [ ] **Step 1:** Find git mutation hooks (likely `useGitMutations` or similar). For each:
  - Add `invalidates: [qk.git.status(), qk.git.branches(), qk.git.log()]` (or narrower).

- [ ] **Step 2:** Commit `feat(desktop-ui): wire git mutations to invalidate matching queries`.

---

## Phase G — App orchestration & cleanup

### Task G1: Audit `useTrayRecentThreads.ts` and `useTraySessionUsage.ts`

These hooks **write** data to the tray menu-bar icon (not read). They should subscribe to thread / session caches and push deltas.

- [ ] **Step 1: Rewrite as cache subscribers**

```ts
import { useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";
import { qk } from "@/lib/query";
import { setTrayRecentThreads } from "../api/tray";

export function useTrayRecentThreads() {
	const client = useQueryClient();
	useEffect(() => {
		const unsubscribe = client.getQueryCache().subscribe((event) => {
			if (event.type !== "updated") return;
			const key = event.query.queryKey;
			if (key[0] !== "threads" || key[1] !== "list") return;
			const data = event.query.state.data;
			if (data) setTrayRecentThreads(data);
		});
		return unsubscribe;
	}, [client]);
}
```

- [ ] **Step 2:** Same pattern for `useTraySessionUsage.ts`.

- [ ] **Step 3:** Commit `refactor(desktop-ui): tray-sync hooks subscribe to query cache`.

### Task G2: Audit composer + remaining `ipc()` calls

- [ ] **Step 1:** Sweep the codebase:

```bash
cd desktop-ui && grep -rnE "await ipc\(|await invoke\(|ipc<|invoke<" src/features/composer src/features/messages
```

For each result, decide: data fetch → migrate to `useTauriQuery`; mutation → migrate to `useTauriMutation`; one-off side effect (open window, etc.) → leave as-is but consider if it should invalidate something.

- [ ] **Step 2:** Migrate as needed. Each cluster gets its own commit.

### Task G3: Final regression sweep

- [ ] **Step 1: Run all tests**

```bash
cd desktop-ui && bun run test
```

Expected: all green.

- [ ] **Step 2: Typecheck**

```bash
cd desktop-ui && bunx tsc --noEmit 2>&1 | tail -10 && echo "---DONE---"
```

- [ ] **Step 3: Manual end-to-end test** — open all four windows (main, launcher, tray, distraction). Trigger mutations from each. Verify all others reflect changes instantly.

- [ ] **Step 4: Open React Query devtools in each window** — confirm cached queries are sensible (no duplicates, no leftover ad-hoc keys).

- [ ] **Step 5: Final commit if anything cosmetic was tweaked.**

---

## Self-Review

**1. Spec coverage:**
- Foundation extensions (queryKeys + bridge routes) → A1, A2 ✓
- Launcher migration → B1–B5 ✓
- Distraction overlay → C1 ✓
- Settings (5 hooks) → D1, D2, D3a-c ✓
- Models / Skills / Apps / Prompts → E1, E2 ✓
- Git panel (5 + 4 + controller + mutations) → F1, F2, F3, F4 ✓
- Tray sync hooks → G1 ✓
- Composer + remaining ipc → G2 ✓
- Final regression → G3 ✓

**2. Placeholder scan:** A few lines say "verify exact cmd name" — these aren't placeholders, they're explicit instructions to confirm before edit. Acceptable per the skill (the engineer must read the existing file to grab the exact string; there's no way for me to know the cmd name without it).

**3. Type consistency:** `qk.<domain>.<sub>()` factory ensures keys match between writers (A1 declarations) and readers (B–G callsites). `useTauriQuery`/`useTauriMutation` signatures unchanged from Plan 1.

---

## Definition of Done (Plan 2)

- All migrations committed; no `useTrayQuery`/`useTrayMutation` references remain anywhere.
- `grep -rE "await ipc\(|await invoke\(" src/features` returns only valid one-off side effects (open window, quit_app) — not data fetches.
- React Query devtools shows queries from every feature.
- Cross-window mutations propagate within ~200ms (same window) or one event-loop tick (other windows).
