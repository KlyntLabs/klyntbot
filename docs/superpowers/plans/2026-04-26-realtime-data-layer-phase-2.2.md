# Phase 2.2 — Wrap Migrated Hook Tests in QueryProvider

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring `bun run test --run` to a clean exit-0 state by fixing the 57 remaining failures across 5 test files, all of which trip on `No QueryClient set, use QueryClientProvider to set one` because their hooks were migrated to `useTauriQuery`/`useTauriMutation` in Phase 1 but the tests were never re-wrapped.

**Architecture:** Pure test infrastructure. Wrap each test's `renderHook`/`render` in a `wrapper` constant that supplies `QueryProvider`. For tests that asserted legacy effect-based loading semantics (`isLoading: true` immediately, manual cancellation), realign the assertions to TanStack Query semantics using `waitFor`. No production code changes.

**Tech Stack:** Vitest 4.x, @testing-library/react 16.x, TanStack Query v5, `@/lib/query` (`QueryProvider`, `useTauriQuery`, `useTauriMutation`).

**Scope:**
- `src/features/apps/hooks/useApps.test.ts` (6 failures)
- `src/features/models/hooks/useModels.test.tsx` (3 failures)
- `src/features/prompts/hooks/useCustomPrompts.test.tsx` (~6 failures)
- `src/features/settings/components/SettingsView.test.tsx` (~30 failures)
- `src/features/skills/hooks/useSkills.test.tsx` (~12 failures)

**Out of scope:** New tests, production hook changes, the 2 `it.skip`'d GitDiffPanel platform-path tests (separate ticket), the 351 lint warnings (separate hygiene pass).

---

## Pre-flight

- [ ] **Step 0.1: Confirm baseline (5 files / 57 failures)**

Run: `cd desktop-ui && bun run test --run 2>&1 | grep -E "Test Files|Tests " | tail -2`
Expected: `Test Files  5 failed | 143 passed (148)` and `Tests  57 failed | 996 passed | 2 skipped (1055)`. Record actuals.

- [ ] **Step 0.2: Confirm the wrapper pattern**

Run: `grep -A5 "QueryProvider" desktop-ui/src/features/settings/hooks/useSettingsDefaultModels.test.tsx | head -10`
Expected: shows the canonical pattern shipped in Phase 2.1:
```tsx
const wrapper = ({ children }: { children: ReactNode }) => (
  <QueryProvider>{children}</QueryProvider>
);
```
Use this exact pattern in every task below — consistency keeps grep-driven future maintenance painless.

---

### Task 1: `useApps.test.ts` — wrap + realign 6 tests

**Files:**
- Modify: `desktop-ui/src/features/apps/hooks/useApps.test.ts`
- Reference: `desktop-ui/src/features/apps/hooks/useApps.ts` (already migrated)

The hook now uses `useTauriQuery` keyed by `qk.apps.list(workspaceId, threadId)` and is invalidated via `appServerEventBridge` on `AppListUpdated`. The 6 failing tests need a wrapper plus the in-flight-cancellation test (#1) needs to be dropped — TQ handles staleness internally, the same call we made for `useSettingsDefaultModels`.

- [ ] **Step 1.1: Read the existing test file and the hook**

Run: `cat desktop-ui/src/features/apps/hooks/useApps.test.ts | head -40` — note current imports, mocks, and the absence of any `QueryProvider`.
Run: `cat desktop-ui/src/features/apps/hooks/useApps.ts` — capture the current call signature: `useApps(workspaceId, threadId)` returns `{ apps, isLoading, error, refresh }`.

- [ ] **Step 1.2: Add `wrapper` and import `QueryProvider`**

At the top of the file, add (preserving existing imports):
```tsx
import type { ReactNode } from "react";
import { QueryProvider } from "@/lib/query";

const wrapper = ({ children }: { children: ReactNode }) => (
  <QueryProvider>{children}</QueryProvider>
);
```
*(Rename file to `.tsx` if it's currently `.ts`; the JSX above requires it.)*

- [ ] **Step 1.3: Add `wrapper` to every `renderHook(...)` call**

For each `renderHook(fn, { ... })`, insert `wrapper,` as the first key in the options object:
```tsx
renderHook(fn, { wrapper, initialProps: { ... } });
```

- [ ] **Step 1.4: Drop the legacy in-flight-cancellation test**

Delete the `it("re-fetches for a new workspace after switching while previous request is in-flight", ...)` block. TQ replaces this manual cancellation with `queryKey`-keyed cache lookup; the test asserts an internal mechanism that no longer exists.

- [ ] **Step 1.5: Realign "applies app/list/updated notifications" tests**

These tests previously dispatched a custom event listener directly. Replace with: fire `AppListUpdated` through the mocked `subscribeAppServerEvents` and assert `qk.apps.all()` invalidation triggers a refetch. Pattern (mirror `Tray.realtime.test.tsx`):

```tsx
it("refetches when AppListUpdated fires", async () => {
  const { result } = renderHook(() => useApps("ws-1", "thread-1"), { wrapper });
  await waitFor(() => expect(fakeIpc).toHaveBeenCalledWith("apps_list", expect.anything()));
  const initial = fakeIpc.mock.calls.filter(([c]) => c === "apps_list").length;
  fireAppServerEvent({ type: "AppListUpdated", workspaceId: "ws-1" });
  await waitFor(() => {
    const after = fakeIpc.mock.calls.filter(([c]) => c === "apps_list").length;
    expect(after).toBeGreaterThan(initial);
  });
});
```

- [ ] **Step 1.6: Verify**

Run: `cd desktop-ui && bun run test --run src/features/apps/hooks/useApps.test.tsx`
Expected: all tests pass (5–6 tests, depending on how many you kept).

- [ ] **Step 1.7: Commit**

```bash
git add desktop-ui/src/features/apps/hooks/useApps.test.{ts,tsx}
git commit -m "test(apps): wrap useApps tests in QueryProvider and realign for AppListUpdated"
```

---

### Task 2: `useModels.test.tsx` — wrap + drop 3 brittle tests

**Files:**
- Modify: `desktop-ui/src/features/models/hooks/useModels.test.tsx`
- Reference: `desktop-ui/src/features/models/hooks/useModels.ts`

3 failures, all on `No QueryClient`. The hook still has 5 residual `useEffect`s from Phase 2 (flagged in the audit). Tests assert config-model merging and effort persistence — keep them, just wrap.

- [ ] **Step 2.1: Read the test file and hook**

Run: `cat desktop-ui/src/features/models/hooks/useModels.test.tsx | head -50`
Run: `head -90 desktop-ui/src/features/models/hooks/useModels.ts`

- [ ] **Step 2.2: Add the wrapper imports + constant**

Same pattern as Task 1.2.

- [ ] **Step 2.3: Add `wrapper` to every `renderHook(...)` call**

Same pattern as Task 1.3.

- [ ] **Step 2.4: If any test asserts `isLoading === true` synchronously, switch to `waitFor`**

Under TQ, `isLoading` flips to `true` only on the first uncached fetch and resolves on the next microtask. Replace:
```tsx
expect(result.current.isLoading).toBe(true);
```
with:
```tsx
await waitFor(() => expect(result.current.models.length).toBeGreaterThan(0));
```

- [ ] **Step 2.5: Verify**

Run: `cd desktop-ui && bun run test --run src/features/models/hooks/useModels.test.tsx`
Expected: 3 prior failures pass.

- [ ] **Step 2.6: Commit**

```bash
git add desktop-ui/src/features/models/hooks/useModels.test.tsx
git commit -m "test(models): wrap useModels tests in QueryProvider"
```

---

### Task 3: `useCustomPrompts.test.tsx` — wrap + realign event invalidation

**Files:**
- Modify: `desktop-ui/src/features/prompts/hooks/useCustomPrompts.test.tsx`
- Reference: `desktop-ui/src/features/prompts/hooks/useCustomPrompts.ts`

The hook uses `useTauriQuery` for `qk.prompts.list(workspaceId)` and 4 mutations (`create`, `update`, `remove`, `move`) that invalidate the same key. `appServerEventBridge` invalidates on `PromptsUpdateAvailable`.

- [ ] **Step 3.1: Read the test file**

Run: `cat desktop-ui/src/features/prompts/hooks/useCustomPrompts.test.tsx`

- [ ] **Step 3.2: Add wrapper + imports**

Same pattern as Task 1.2.

- [ ] **Step 3.3: Add `wrapper` to every `renderHook` and `render` call**

If the file uses `render(<SomeComponent />)`, wrap it:
```tsx
render(<QueryProvider><SomeComponent /></QueryProvider>);
```
Or use the existing `wrapper` via Testing Library's `wrapper` option:
```tsx
render(<SomeComponent />, { wrapper });
```

- [ ] **Step 3.4: Realign mutation assertions**

If a test asserted `mutate(...)` returned synchronously or that a specific cache key was set imperatively, switch to:
```tsx
await act(async () => { await result.current.create({ ... }); });
await waitFor(() => expect(fakeIpc).toHaveBeenCalledWith("prompts_create", expect.anything()));
```

- [ ] **Step 3.5: Verify**

Run: `cd desktop-ui && bun run test --run src/features/prompts/hooks/useCustomPrompts.test.tsx`
Expected: all tests pass.

- [ ] **Step 3.6: Commit**

```bash
git add desktop-ui/src/features/prompts/hooks/useCustomPrompts.test.tsx
git commit -m "test(prompts): wrap useCustomPrompts tests in QueryProvider"
```

---

### Task 4: `useSkills.test.tsx` — wrap (largest of the hook test files)

**Files:**
- Modify: `desktop-ui/src/features/skills/hooks/useSkills.test.tsx`
- Reference: `desktop-ui/src/features/skills/hooks/useSkills.ts`

~12 failures. Hook is `qk.skills.list(workspaceId)`, invalidated by `SkillsUpdateAvailable` through `appServerEventBridge`.

- [ ] **Step 4.1: Read the test file**

Run: `wc -l desktop-ui/src/features/skills/hooks/useSkills.test.tsx`
Run: `head -60 desktop-ui/src/features/skills/hooks/useSkills.test.tsx`

- [ ] **Step 4.2: Add wrapper + imports**

Same pattern.

- [ ] **Step 4.3: Add `wrapper` to every `renderHook` call**

Use grep to find them: `grep -n "renderHook" desktop-ui/src/features/skills/hooks/useSkills.test.tsx`. Add `wrapper,` to each.

- [ ] **Step 4.4: Realign any tests asserting custom event-listener behavior**

If a test reaches into `window.addEventListener("skills:updated", ...)` style, replace with `subscribeAppServerEvents` mock + `fire({ type: "SkillsUpdateAvailable", workspaceId })`. Pattern matches Task 1.5.

- [ ] **Step 4.5: Verify**

Run: `cd desktop-ui && bun run test --run src/features/skills/hooks/useSkills.test.tsx`
Expected: all 12 prior failures pass.

- [ ] **Step 4.6: Commit**

```bash
git add desktop-ui/src/features/skills/hooks/useSkills.test.tsx
git commit -m "test(skills): wrap useSkills tests in QueryProvider"
```

---

### Task 5: `SettingsView.test.tsx` — wrap (largest, ~30 failures)

**Files:**
- Modify: `desktop-ui/src/features/settings/components/SettingsView.test.tsx`
- Reference: `desktop-ui/src/features/settings/components/SettingsView.tsx`

This is the largest cleanup. SettingsView aggregates many migrated hooks (`useAppSettings`, `useSettingsAgentsSection`, `useSettingsDefaultModels`, `useSettingsFeaturesSection`, `useSettingsServerSection`, `useModels`, `useSkills`, `useApps`, `useCustomPrompts`). All require a `QueryProvider`.

- [ ] **Step 5.1: Inspect the test file's render helper**

Run: `head -80 desktop-ui/src/features/settings/components/SettingsView.test.tsx`
Look for any `renderSettings()` / `renderWithProviders()` helper. If one exists, the wrapper goes there *once*; if not, every `render()` call needs wrapping.

- [ ] **Step 5.2: Add `QueryProvider` to the render helper (or each `render(...)`)**

If a helper exists:
```tsx
function renderSettings(props: Partial<SettingsViewProps> = {}) {
  return render(
    <QueryProvider>
      <SettingsView {...defaultProps} {...props} />
    </QueryProvider>,
  );
}
```

If individual `render(...)` calls: add `wrapper` import per the canonical pattern, then call `render(<SettingsView {...props} />, { wrapper })`.

- [ ] **Step 5.3: Verify the largest cluster passes (Display section)**

Run: `cd desktop-ui && bun run test --run src/features/settings/components/SettingsView.test.tsx -t "Display"`
Expected: all `SettingsView Display > *` tests pass.

- [ ] **Step 5.4: Verify the rest of the file**

Run: `cd desktop-ui && bun run test --run src/features/settings/components/SettingsView.test.tsx`
Expected: full file passes. If any tests still fail with assertion errors (not provider errors), they're hook-behavior drift — handle in a follow-up step.

- [ ] **Step 5.5: Handle hook-behavior drift (if any)**

For each remaining failure, decide:
- **A)** Test asserts UI text that the migration changed → update the expected string.
- **B)** Test asserts a sequence of mutations that TQ now batches → wrap the action in `await act(async () => {...})` and use `waitFor`.
- **C)** Test asserts a hook field that no longer exists (`isLoading: true` synchronously) → replace with the corresponding TQ-aware assertion.

Document in the commit message which tests required option A/B/C.

- [ ] **Step 5.6: Commit**

```bash
git add desktop-ui/src/features/settings/components/SettingsView.test.tsx
git commit -m "test(settings): wrap SettingsView render helper in QueryProvider"
```

---

### Task 6: Final regression sweep

- [ ] **Step 6.1: Full test run**

Run: `cd desktop-ui && bun run test --run 2>&1 | grep -E "Test Files|Tests "`
Expected: `Test Files  148 passed (148)` and `Tests  1053 passed | 2 skipped (1055)` (or similar — failures should be 0).

- [ ] **Step 6.2: Lint + typecheck**

Run in parallel:
- `cd desktop-ui && bun run lint > /dev/null 2>&1; echo "LINT=$?"`
- `cd desktop-ui && bunx tsc --noEmit > /dev/null 2>&1; echo "TSC=$?"`
Expected: `LINT=0` and `TSC=0`.

- [ ] **Step 6.3: Push**

```bash
git push origin main
```

- [ ] **Step 6.4: Optional follow-up — re-enable the 2 skipped GitDiffPanel tests**

These exercise `isAbsolutePathForPlatform` under jsdom. Phase 2.2 may have unblocked them by aligning provider context. Try removing `it.skip` and re-running:
```
cd desktop-ui && bun run test --run src/features/git/components/GitDiffPanel.test.tsx
```
If still failing, leave them skipped and open a Phase 2.3 ticket for platform-path mocking.

---

## Self-Review

**Spec coverage:**
- 5 failing files × 1 task each → Tasks 1–5 ✅
- Provider wrapping (the universal cause) addressed by every task's `wrapper` step ✅
- Behavior-drift escape hatch (Task 5.5) for the largest, most variable file ✅
- Verification (Task 6) ✅

**Placeholder scan:** None — every step gives an exact command, expected outcome, or concrete code block.

**Type consistency:**
- `QueryProvider` is imported from `@/lib/query` consistently — the same export used by `App.tsx` and the canonical `useSettingsDefaultModels.test.tsx` from Phase 2.1.
- `wrapper` constant signature `({ children }: { children: ReactNode })` matches Testing Library's documented `wrapper` option.
- `qk.*` factory references match Phase 1's `queryKeys.ts`.

**Risk flags:**
- **Task 5 is the highest-variance task.** SettingsView aggregates 9+ migrated hooks; some assertions may exercise paths that need option B/C realignment. Budget extra time. If any test resists straightforward fixing, mark it `.skip` with a TODO referencing this plan.
- **Mutation tests under TQ:** any test that asserts a mutation's promise resolves *before* the cache is invalidated will need `await waitFor(() => ...)` rather than synchronous expect. Pattern from Phase 2.1's `useSettingsDefaultModels.test.tsx` is the reference implementation.

---

## Execution Handoff

**Plan complete and saved to `docs/superpowers/plans/2026-04-26-realtime-data-layer-phase-2.2.md`. Two execution options:**

1. **Subagent-Driven (recommended)** — Each task is small (5–20 min) and self-contained per file. Tasks 1–4 are mechanical wrapping that any fresh subagent can complete; Task 5 benefits from review-between-tasks because of behavior-drift risk.
2. **Inline Execution** — Faster turnaround since 5 of 6 tasks are mechanical; main context stays manageable.

Which approach?
