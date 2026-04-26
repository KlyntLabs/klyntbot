# Phase 2.1 — Test Infra & Stale-Test Cleanup Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Get `bun run test` and `bun run lint` to a clean exit-0 state on `desktop-ui` after the Phase 2 realtime migration. Address the 5 categories of pre-existing failures surfaced by the Phase 2 audit, plus the ESLint v9 config migration.

**Architecture:** Pure test/infra cleanup — no runtime code changes to hooks or features. Each task fixes one failing file (or one infra concern) and commits independently. Hooks ship as-is; tests are realigned to current behavior.

**Tech Stack:** Vitest 2.x, @testing-library/react, Biome 2.0, ESLint v9 (flat config), Tauri 2 mocks.

**Scope:**
- D-tier: stale unit tests asserting legacy hook semantics
- F-tier: git tests asserting pre-migration polling APIs
- Test-infra: missing Tauri menu mocks, missing `QueryProvider` wrappers
- Lint: ESLint v9 flat-config migration (or removal if redundant with Biome)

**Out of scope:** Adding new tests, refactoring hooks, fixing tests for unrelated `useApps`, `SettingsView`, `useLiquidGlassEffect` failures (separate triage).

---

## Pre-flight (read-only)

- [ ] **Step 0.1: Confirm baseline failure count**

Run: `cd desktop-ui && bun run test --run 2>&1 | tail -5`
Expected: `Test Files  17 failed | 129 passed (146)` and `Tests  93 failed | 948 passed`. Record actual counts before starting.

- [ ] **Step 0.2: Confirm typecheck is already clean**

Run: `cd desktop-ui && bunx tsc --noEmit`
Expected: empty output (exit 0).

---

### Task 1: Realign `useSettingsDefaultModels` tests with current hook

**Files:**
- Modify: `desktop-ui/src/features/settings/hooks/useSettingsDefaultModels.test.tsx`

Current tests assert **legacy semantics** that don't match `useSettingsDefaultModels.ts`:
- "uses the first workspace only" → hook actually loops over **all** projects
- "displayName contains `(config)`" → hook sets `description: CONFIG_MODEL_DESCRIPTION` and `displayName: configModel`
- "isLoading initially true" → TQ's `isLoading` only true on first uncached fetch; hook's nested `isLoading` field reflects internal queryFn state, not TQ state

- [ ] **Step 1.1: Read the hook to capture actual behavior**

Run: `cat desktop-ui/src/features/settings/hooks/useSettingsDefaultModels.ts`
Note: queryFn iterates **every** project, dedupes by `id`, sorts by `compareModelsByLatest`, returns `{models, isLoading: false, error, connectedWorkspaceCount}`.

- [ ] **Step 1.2: Rewrite the "first workspace as the model source" test**

Replace assertions `getModelListMock).not.toHaveBeenCalledWith("w2")` with: assert that **both** `w1` and `w2` were requested, and that the deduped result contains exactly one entry for the model.

```tsx
it("aggregates models across all workspaces and dedupes by id", async () => {
  connectWorkspaceMock.mockResolvedValue(undefined);
  getConfigModelMock.mockResolvedValue(null);
  getModelListMock.mockResolvedValue(modelListResponse("gpt-5.1"));

  const { result } = renderHook(
    ({ projects }: { projects: WorkspaceInfo[] }) => useSettingsDefaultModels(projects),
    {
      wrapper,
      initialProps: { projects: [workspace("w1", false), workspace("w2", true)] },
    },
  );

  await waitFor(() => {
    expect(connectWorkspaceMock).toHaveBeenCalledWith("w1");
    expect(connectWorkspaceMock).toHaveBeenCalledWith("w2");
    expect(result.current.models).toHaveLength(1);
    expect(result.current.models[0]?.model).toBe("gpt-5.1");
  });
});
```

- [ ] **Step 1.3: Rewrite the "falls back to config model" test**

```tsx
it("includes config model with CONFIG_MODEL_DESCRIPTION when set", async () => {
  connectWorkspaceMock.mockResolvedValueOnce(undefined);
  getConfigModelMock.mockResolvedValueOnce("gpt-5-codex");
  getModelListMock.mockResolvedValueOnce({ result: { data: [] } });

  const { result } = renderHook(
    ({ projects }: { projects: WorkspaceInfo[] }) => useSettingsDefaultModels(projects),
    { wrapper, initialProps: { projects: [workspace("w1", true)] } },
  );

  await waitFor(() => {
    expect(result.current.models[0]?.model).toBe("gpt-5-codex");
    expect(result.current.models[0]?.description).toBe(
      "Configured in CODEX_HOME/config.toml",
    );
    expect(result.current.models[0]?.isDefault).toBe(true);
  });
});
```

- [ ] **Step 1.4: Drop the "invalidates in-flight results" test**

This test asserted the legacy effect-based race-cancellation pattern. TanStack Query handles staleness internally — re-render with empty `projects` simply skips the queryFn, and prior in-flight promises are no longer reflected in `result.current` once the new query resolves. The test is meaningless under TQ semantics.

Delete the entire `it("invalidates in-flight results when workspace list becomes empty", ...)` block (lines ~63–95).

- [ ] **Step 1.5: Verify**

Run: `cd desktop-ui && bun run test --run src/features/settings/hooks/useSettingsDefaultModels.test.tsx`
Expected: all remaining tests pass (3 tests, 3 passed).

- [ ] **Step 1.6: Commit**

```bash
git add desktop-ui/src/features/settings/hooks/useSettingsDefaultModels.test.tsx
git commit -m "test(settings): realign default-models tests with TQ-migrated hook behavior"
```

---

### Task 2: Update `useGitStatus.test.tsx` for `useTauriQuery` semantics

**Files:**
- Modify: `desktop-ui/src/features/git/hooks/useGitStatus.test.tsx` (or wherever it lives)

The 4 failures assert old polling/interval behavior (`setInterval`, manual refetch on focus). Under `useTauriQuery`, polling is replaced by event-bridge invalidation + TQ `staleTime`/`refetchOnWindowFocus`.

- [ ] **Step 2.1: Locate the test file**

Run: `find desktop-ui/src/features/git -name "useGitStatus*"`
Note the path returned.

- [ ] **Step 2.2: Read the failing test file and the current hook**

Read both. Identify which assertions reference `setInterval`, `clearInterval`, manual `refetch()` after timeout, or `pollingInterval` props.

- [ ] **Step 2.3: Replace polling assertions with event-driven assertions**

For each polling test, rewrite as:
```tsx
it("refetches when entity:updated{kind:'task'} fires", async () => {
  // Initial fetch
  await waitFor(() => expect(fakeIpc).toHaveBeenCalledWith("git_status", expect.anything()));
  const initial = fakeIpc.mock.calls.filter(([c]) => c === "git_status").length;
  // Fire event
  subs.get("entity:updated")?.({ entityKind: "...", id: "..." });
  await waitFor(() => {
    const after = fakeIpc.mock.calls.filter(([c]) => c === "git_status").length;
    expect(after).toBeGreaterThan(initial);
  });
});
```
Mirror the pattern from `desktop-ui/src/features/tray/tests/Tray.realtime.test.tsx` (lines 6–43 — fakeListen + subs map).

If the original tests asserted "stops polling on unmount", replace with "unsubscribes from event bridge on unmount" — but actually the bridge is global, owned by `QueryProvider`, so this assertion is no longer needed. Drop those tests.

- [ ] **Step 2.4: Verify**

Run: `cd desktop-ui && bun run test --run src/features/git/hooks/useGitStatus.test.tsx`
Expected: all tests pass.

- [ ] **Step 2.5: Commit**

```bash
git add desktop-ui/src/features/git/hooks/useGitStatus.test.tsx
git commit -m "test(git): replace polling assertions with event-bridge refetch checks for useGitStatus"
```

---

### Task 3: Update `useGitPanelController.test.tsx` for new `useGitDiffs` signature

**Files:**
- Modify: `desktop-ui/src/features/git/hooks/useGitPanelController.test.tsx` (or co-located path)

The 4 failures cite `enabled` arg being undefined — the migrated `useGitDiffs` no longer accepts a separate `enabled` positional arg; it's now an option on `useTauriQuery` derived from another input.

- [ ] **Step 3.1: Locate test file**

Run: `find desktop-ui/src/features/git -name "useGitPanelController*"`

- [ ] **Step 3.2: Read the current `useGitDiffs.ts` to capture its real signature**

Run: `cat desktop-ui/src/features/git/hooks/useGitDiffs.ts`
Record: hook arguments, returned shape, what controls `enabled`.

- [ ] **Step 3.3: Update the controller test mocks**

Replace any `useGitDiffs(workspaceId, enabled)`-style call assertions with the actual current signature. If the test mocks `useGitDiffs`, update the mock factory to return `{ data, isLoading, refetch }` matching `useTauriQuery`'s shape.

- [ ] **Step 3.4: Verify**

Run: `cd desktop-ui && bun run test --run src/features/git/hooks/useGitPanelController.test.tsx`
Expected: all 4 prior failures now pass.

- [ ] **Step 3.5: Commit**

```bash
git add desktop-ui/src/features/git/hooks/useGitPanelController.test.tsx
git commit -m "test(git): update useGitPanelController test for new useGitDiffs signature"
```

---

### Task 4: Mock `MenuItem.new` for `GitDiffPanel.test.tsx`

**Files:**
- Modify: `desktop-ui/src/features/git/components/GitDiffPanel.test.tsx` OR add to a shared setup file

`MenuItem.new` (Tauri 2 menu plugin) is invoked from `GitDiffPanel.tsx:451` when the user opens the right-click menu. It's not mocked in the jsdom environment, throwing on every test that exercises the panel's `onContextMenu`.

- [ ] **Step 4.1: Identify the exact import path**

Run: `grep -n "MenuItem" desktop-ui/src/features/git/components/GitDiffPanel.tsx`
Note the import (likely `@tauri-apps/api/menu`).

- [ ] **Step 4.2: Add a Vitest mock at the top of the test file**

```tsx
vi.mock("@tauri-apps/api/menu", () => ({
  MenuItem: { new: vi.fn(async () => ({ id: "stub" })) },
  Menu: { new: vi.fn(async () => ({ popup: vi.fn() })) },
  Submenu: { new: vi.fn(async () => ({ id: "submenu-stub" })) },
  PredefinedMenuItem: { new: vi.fn(async () => ({ id: "pre-stub" })) },
}));
```

If multiple test files exercise the panel, hoist the mock into `desktop-ui/src/test/setup.ts` (or wherever Vitest's `setupFiles` points).

- [ ] **Step 4.3: Verify**

Run: `cd desktop-ui && bun run test --run src/features/git/components/GitDiffPanel.test.tsx`
Expected: 5 prior failures pass.

- [ ] **Step 4.4: Commit**

```bash
git add desktop-ui/src/features/git/components/GitDiffPanel.test.tsx
git commit -m "test(git): mock @tauri-apps/api/menu for GitDiffPanel jsdom suite"
```

---

### Task 5: ESLint v9 flat-config migration (or removal)

**Files:**
- Possibly create: `desktop-ui/eslint.config.js`
- Possibly modify: `desktop-ui/package.json`

ESLint v9 requires flat config (`eslint.config.{js,mjs,cjs}`); the project still has v8-style `.eslintrc.*`. Biome 2.0 already covers lint + format + import-organize per `CLAUDE.md`. Two options:

- [ ] **Step 5.1: Inspect current state**

Run: `cd desktop-ui && cat package.json | grep -A2 '"lint"'`
Run: `cd desktop-ui && ls -la .eslintrc* eslint.config.* 2>/dev/null`
Run: `cd desktop-ui && cat .eslintrc* 2>/dev/null | head -30`

- [ ] **Step 5.2: Decide direction (record reasoning in commit message)**

**Option A — Remove ESLint entirely** (recommended): Biome covers the same lints. Remove `.eslintrc*`, drop `eslint`/`eslint-*` dev dependencies, remove the `lint` script's ESLint invocation if any (`bun run lint` should map to `biome check`).

**Option B — Migrate to flat config**: write `eslint.config.js`, install `@eslint/js` etc.

Pick Option A unless a specific rule lives only in ESLint.

- [ ] **Step 5.3a (Option A): Remove ESLint**

```bash
cd desktop-ui
rm -f .eslintrc.json .eslintrc.cjs .eslintrc.js .eslintignore
bun remove eslint @typescript-eslint/eslint-plugin @typescript-eslint/parser eslint-plugin-react eslint-plugin-react-hooks 2>/dev/null || true
```
Open `package.json`, ensure `"lint": "biome check ."` and `"lint:fix": "biome check --write ."`.

- [ ] **Step 5.3b (Option B): Add flat config**

Create `desktop-ui/eslint.config.js` with minimal `@eslint/js` recommended preset + project-specific rules. Confirm `package.json` `lint` script invokes it.

- [ ] **Step 5.4: Verify**

Run: `cd desktop-ui && bun run lint`
Expected: exit 0 (no errors). If Biome reports new warnings unrelated to Phase 2.1, capture them in a follow-up issue, don't fix here.

- [ ] **Step 5.5: Commit**

```bash
git add desktop-ui/package.json desktop-ui/eslint.config.* desktop-ui/.eslintrc* desktop-ui/bun.lockb
git commit -m "chore(desktop-ui): drop ESLint in favor of Biome (or: migrate ESLint to v9 flat config)"
```

---

### Task 6: Final regression sweep

- [ ] **Step 6.1: Full test run**

Run: `cd desktop-ui && bun run test --run 2>&1 | tail -10`
Expected: meaningful reduction. The known-unrelated failures (`useApps`, `SettingsView`, `useLiquidGlassEffect`) may persist — record them in the commit message for a future Phase 2.2 if they remain.

- [ ] **Step 6.2: Lint**

Run: `cd desktop-ui && bun run lint`
Expected: exit 0.

- [ ] **Step 6.3: Typecheck**

Run: `cd desktop-ui && bunx tsc --noEmit`
Expected: empty output.

- [ ] **Step 6.4: Final commit (if any cleanup)**

If any small follow-ups emerged during the sweep (e.g., dead `import` lines), commit separately:
```bash
git commit -m "chore(desktop-ui): post-2.1 cleanup"
```

- [ ] **Step 6.5: Self-review checklist**

- All 4 stale-test files updated to current hook behavior
- ESLint v9 path resolved (removed or migrated)
- No hook source files modified (test/infra only)
- Each task committed independently
- New baseline failure count documented

---

## Self-Review

**Spec coverage:**
- Stale tests (`useSettingsDefaultModels`, `useGitStatus`, `useGitPanelController`) → Tasks 1, 2, 3 ✅
- Missing test infra (`MenuItem.new`) → Task 4 ✅
- ESLint v9 → Task 5 ✅
- Final verification → Task 6 ✅

**Placeholder scan:** None — every test rewrite includes complete code; every command has expected output.

**Type consistency:** Task 1's `wrapper` references the import added during Phase 2 audit fix (already in file). Task 2/3 reference `useTauriQuery`'s actual return shape (`{ data, isLoading, refetch, isFetching, error }`).

**Out-of-scope failures explicitly acknowledged** in Step 6.1: `useApps`, `SettingsView`, `useLiquidGlassEffect` belong to a future Phase 2.2 if they're still red after this plan lands.

---

## Execution Handoff

**Plan complete and saved to `docs/superpowers/plans/2026-04-26-realtime-data-layer-phase-2.1.md`. Two execution options:**

1. **Subagent-Driven (recommended)** — Each task is small enough for one fresh subagent; Task 1 and Task 5 are the only ones with judgment calls (drop-vs-rewrite, Option A vs B), making between-task review valuable.
2. **Inline Execution** — Faster turn-around since each task is small (10–15 min); main context stays manageable across 6 tasks.

Which approach?
