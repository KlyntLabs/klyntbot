# Platform-Wide Real-Time Event Migration Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Migrate all feature pages from manual `useEvent("entity:updated") + refetch()` to the declarative `invalidateOn` pattern on `useQuery`, making the entire platform update in real-time when mutations happen from any source (MCP, cron, agent tools, direct UI).

**Architecture:** Pure frontend migration. Replace each `useEvent("entity:updated", handler)` + manual `refetch()` with `invalidateOn: ["entity:updated"]` + `invalidateFilter` on the relevant `useQuery` calls. No backend changes — `entity:updated` events are already emitted from the chat relay (for agent tool mutations) and from the MCP server (for external tool calls). The `useMutation` hook also dispatches `entity:updated` on the frontend for direct UI mutations.

**Tech Stack:** TypeScript/React (desktop-ui only)

**Spec:** `docs/superpowers/specs/2026-03-31-realtime-event-layer-design.md` (Section: "Platform-Wide Adoption")

---

## File Structure

All changes are in `desktop-ui/src/`. No files created — only modifications.

| File | Current Pattern | Migration |
|---|---|---|
| `features/tasks/hooks/useTasks.ts` | `useEvent` + conditional refetch by kind | `invalidateOn` with `invalidateFilter` per query |
| `features/notes/pages/KnowledgeBasePage.tsx` | `useEvent` + refetch + `invalidateQueries()` | `invalidateOn` per query |
| `features/notes/hooks/useBacklinks.ts` | `useEvent` + refetch | `invalidateOn` |
| `features/notes/hooks/useUnlinkedMentions.ts` | `useEvent` + refetch | `invalidateOn` |
| `features/notes/hooks/useInbox.ts` | `useEvent` + refetch | `invalidateOn` |
| `features/finance/pages/CashFlowPage.tsx` | `useEvent` + `refetchAll()` | `invalidateOn` per query |
| `features/finance/pages/FinanceOverviewPage.tsx` | `useEvent` + `refetchAll()` | `invalidateOn` per query |
| `features/finance/pages/InvestmentsPage.tsx` | `useEvent` + `refetchAll()` | `invalidateOn` per query |
| `features/finance/pages/TargetsPage.tsx` | `useEvent` + refetch | `invalidateOn` per query |
| `features/projects/contexts/ProjectContext.tsx` | `useEvent` + conditional refetch | `invalidateOn` with filter |
| `features/dashboard/components/DayCalendarView.tsx` | `useEvent` + conditional refetch | `invalidateOn` with filter |
| `features/dashboard/components/DayColumnsView.tsx` | `useEvent` + refetch | `invalidateOn` |
| `features/dashboard/components/productivity/GoalsProgress.tsx` | `useEvent` + refetch | `invalidateOn` |
| `features/dashboard/components/productivity/ActivityFeed.tsx` | `useEvent` + refetch | `invalidateOn` |
| `features/tray/pages/SystemTrayPage.tsx` | `useEvent` + refetch | `invalidateOn` |

---

## Task 1: Migrate Tasks Feature

**Files:**
- Modify: `desktop-ui/src/features/tasks/hooks/useTasks.ts`

- [ ] **Step 1: Read current implementation**

The file currently has (around lines 37-79):
```typescript
const { data: tasks, refetch: refetchTasks } = useQuery("task_list", filters, []);
const { data: projects, refetch: refetchProjects } = useQuery("project_list", ...);
const { data: areas, refetch: refetchAreas } = useQuery("area_list", ...);

useEvent<{ entityKind: string; id: string }>("entity:updated", (payload) => {
  const kind = payload?.entityKind;
  if (!kind) { refetchTasks(); refetchProjects(); refetchAreas(); return; }
  if (kind === "task" || kind === "area") refetchTasks();
  if (kind === "project") refetchProjects();
  if (kind === "area") refetchAreas();
});
```

- [ ] **Step 2: Add `invalidateOn` to each query and remove the `useEvent` block**

Replace the three `useQuery` calls with `invalidateOn` + `invalidateFilter`:

```typescript
const { data: tasks, refetch: refetchTasks } = useQuery("task_list", filters, [], {
  invalidateOn: ["entity:updated"],
  invalidateFilter: (p) => {
    const kind = (p as { entityKind?: string })?.entityKind;
    return !kind || kind === "task" || kind === "area";
  },
});
const { data: projects, refetch: refetchProjects } = useQuery("project_list", projectFilter, [], {
  invalidateOn: ["entity:updated"],
  invalidateFilter: (p) => {
    const kind = (p as { entityKind?: string })?.entityKind;
    return !kind || kind === "project";
  },
});
const { data: areas, refetch: refetchAreas } = useQuery("area_list", undefined, [], {
  invalidateOn: ["entity:updated"],
  invalidateFilter: (p) => {
    const kind = (p as { entityKind?: string })?.entityKind;
    return !kind || kind === "area";
  },
});
```

Delete the entire `useEvent<{ entityKind: string; id: string }>("entity:updated", ...)` block.

Remove the `useEvent` import if it's no longer used in this file.

- [ ] **Step 3: Verify**

Run: `cd desktop-ui && bunx biome check src/features/tasks/hooks/useTasks.ts`
Expected: No errors.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/tasks/hooks/useTasks.ts
git commit -m "refactor(tasks): migrate to invalidateOn for real-time updates"
```

---

## Task 2: Migrate Notes Feature (KnowledgeBasePage)

**Files:**
- Modify: `desktop-ui/src/features/notes/pages/KnowledgeBasePage.tsx`

- [ ] **Step 1: Read current implementation**

The file has (around lines 65-69, 526-540):
```typescript
const { data: notebooks, refetch: refetchNotebooks } = useQuery("notebook_list", ...);
const { data: notes, refetch: refetchNotes } = useQuery("note_list", ...);

useEvent<{ entityKind: string }>("entity:updated", (payload) => {
  if (payload.entityKind === "note") {
    refetchNotes();
    invalidateQueries("note_backlinks");
    invalidateQueries("note_links_all");
    invalidateQueries("note_suggestions");
  }
  if (payload.entityKind === "notebook") refetchNotebooks();
  if (payload.entityKind === "inbox") invalidateQueries("inbox_list");
});
```

- [ ] **Step 2: Add `invalidateOn` to each query and remove the `useEvent` block**

```typescript
const { data: notebooks, refetch: refetchNotebooks } = useQuery("notebook_list", params, [], {
  invalidateOn: ["entity:updated"],
  invalidateFilter: (p) => (p as { entityKind?: string })?.entityKind === "notebook",
});
const { data: notes, refetch: refetchNotes } = useQuery("note_list", noteParams, [], {
  invalidateOn: ["entity:updated"],
  invalidateFilter: (p) => (p as { entityKind?: string })?.entityKind === "note",
});
```

Delete the `useEvent` block entirely. The `invalidateQueries("note_backlinks")` etc. calls are no longer needed here — those hooks (`useBacklinks`, `useUnlinkedMentions`) will get their own `invalidateOn` in Tasks 3-4.

Remove the `useEvent` and `invalidateQueries` imports if no longer used.

- [ ] **Step 3: Verify**

Run: `cd desktop-ui && bunx biome check src/features/notes/pages/KnowledgeBasePage.tsx`
Expected: No errors.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/notes/pages/KnowledgeBasePage.tsx
git commit -m "refactor(notes): migrate KnowledgeBasePage to invalidateOn"
```

---

## Task 3: Migrate Notes Hooks (useBacklinks, useUnlinkedMentions, useInbox)

**Files:**
- Modify: `desktop-ui/src/features/notes/hooks/useBacklinks.ts`
- Modify: `desktop-ui/src/features/notes/hooks/useUnlinkedMentions.ts`
- Modify: `desktop-ui/src/features/notes/hooks/useInbox.ts`

- [ ] **Step 1: Migrate useBacklinks**

Current:
```typescript
const result = useQuery("note_backlinks", { noteId }, []);
useEvent<{ entityKind: string }>("entity:updated", (payload) => {
  if (payload.entityKind === "note") result.refetch();
});
```

Replace with:
```typescript
const result = useQuery("note_backlinks", { noteId }, [], {
  invalidateOn: ["entity:updated"],
  invalidateFilter: (p) => (p as { entityKind?: string })?.entityKind === "note",
});
```

Delete the `useEvent` block and its import.

- [ ] **Step 2: Migrate useUnlinkedMentions**

Current:
```typescript
const result = useQuery("note_unlinked_mentions", { noteId }, []);
useEvent<{ entityKind: string }>("entity:updated", (payload) => {
  if (payload.entityKind === "note") result.refetch();
});
```

Replace with:
```typescript
const result = useQuery("note_unlinked_mentions", { noteId }, [], {
  invalidateOn: ["entity:updated"],
  invalidateFilter: (p) => (p as { entityKind?: string })?.entityKind === "note",
});
```

Delete the `useEvent` block and its import.

- [ ] **Step 3: Migrate useInbox**

Current:
```typescript
const { data, refetch, ... } = useQuery("inbox_list", undefined, []);
useEvent<{ entityKind: string }>("entity:updated", (payload) => {
  if (payload.entityKind === "inbox") refetch();
});
```

Replace with:
```typescript
const { data, refetch, ... } = useQuery("inbox_list", undefined, [], {
  invalidateOn: ["entity:updated"],
  invalidateFilter: (p) => (p as { entityKind?: string })?.entityKind === "inbox",
});
```

Delete the `useEvent` block and its import.

- [ ] **Step 4: Verify all three**

Run: `cd desktop-ui && bunx biome check src/features/notes/hooks/useBacklinks.ts src/features/notes/hooks/useUnlinkedMentions.ts src/features/notes/hooks/useInbox.ts`
Expected: No errors.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/notes/hooks/useBacklinks.ts desktop-ui/src/features/notes/hooks/useUnlinkedMentions.ts desktop-ui/src/features/notes/hooks/useInbox.ts
git commit -m "refactor(notes): migrate hooks to invalidateOn for real-time updates"
```

---

## Task 4: Migrate Finance Pages

**Files:**
- Modify: `desktop-ui/src/features/finance/pages/CashFlowPage.tsx`
- Modify: `desktop-ui/src/features/finance/pages/FinanceOverviewPage.tsx`
- Modify: `desktop-ui/src/features/finance/pages/InvestmentsPage.tsx`
- Modify: `desktop-ui/src/features/finance/pages/TargetsPage.tsx`

All finance pages follow the same pattern: catch ALL `entity:updated` events and call `refetchAll()`. Since finance entities share a single `EntityKind::Finance`, the filter is simple.

- [ ] **Step 1: Migrate CashFlowPage**

Find all `useQuery` calls and add `invalidateOn`. Find the `useEvent` + `refetchAll` block and delete it.

For every finance `useQuery` in this file, add:
```typescript
{
  invalidateOn: ["entity:updated"],
  invalidateFilter: (p) => {
    const kind = (p as { entityKind?: string })?.entityKind;
    return !kind || kind === "finance";
  },
}
```

Delete the `useEvent` block and the `refetchAll` function if it becomes unused. Remove the `useEvent` import.

- [ ] **Step 2: Migrate FinanceOverviewPage**

Same pattern — add `invalidateOn` to all `useQuery` calls with the same finance filter. Delete the `useEvent` block and `refetchAll`.

- [ ] **Step 3: Migrate InvestmentsPage**

Same pattern.

- [ ] **Step 4: Migrate TargetsPage**

Same pattern.

- [ ] **Step 5: Verify all four**

Run: `cd desktop-ui && bunx biome check src/features/finance/pages/CashFlowPage.tsx src/features/finance/pages/FinanceOverviewPage.tsx src/features/finance/pages/InvestmentsPage.tsx src/features/finance/pages/TargetsPage.tsx`
Expected: No errors.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/finance/pages/
git commit -m "refactor(finance): migrate all finance pages to invalidateOn"
```

---

## Task 5: Migrate Projects Context

**Files:**
- Modify: `desktop-ui/src/features/projects/contexts/ProjectContext.tsx`

- [ ] **Step 1: Read current implementation**

Current (around lines 28-49):
```typescript
const { data: project, refetch: refetchProject } = useProject(projectId);
const { data: objectives, refetch: refetchObjectives } = useProjectObjectives(projectId);
const { data: tasks, refetch: refetchTasks } = useProjectTasks(projectId);

useEvent<{ entityKind: string }>("entity:updated", (payload) => {
  const kind = payload?.entityKind;
  if (kind === "project") refetchProject();
  if (kind === "objective" || kind === "key_result") refetchObjectives();
  if (kind === "task") refetchTasks();
});
```

- [ ] **Step 2: Migrate**

The sub-hooks (`useProject`, `useProjectObjectives`, `useProjectTasks`) internally use `useQuery`. The `invalidateOn` needs to be added inside those hooks, not at the context level. However, if those hooks don't accept options, the simplest approach is to keep the `useEvent` pattern but only at the context level.

**Check if the sub-hooks accept options.** If not, keep the `useEvent` handler here — it's the coordination point for project-scoped refetches.

Alternative: If the sub-hooks return `refetch`, keep the current `useEvent` pattern but simplify by adding a comment noting this is intentional (the sub-hooks are wrappers that don't expose `invalidateOn`).

**Decision: Keep this file as-is if sub-hooks don't accept `UseQueryOptions`.** This is a coordination context that manages multiple sub-hooks. The `useEvent` pattern here is not a simple migration candidate — it coordinates refetches across hooks that may not support `invalidateOn` directly.

- [ ] **Step 3: Verify no regressions**

Run: `cd desktop-ui && bunx biome check src/features/projects/contexts/ProjectContext.tsx`
Expected: No errors.

---

## Task 6: Migrate Dashboard Components

**Files:**
- Modify: `desktop-ui/src/features/dashboard/components/DayCalendarView.tsx`
- Modify: `desktop-ui/src/features/dashboard/components/DayColumnsView.tsx`
- Modify: `desktop-ui/src/features/dashboard/components/productivity/GoalsProgress.tsx`
- Modify: `desktop-ui/src/features/dashboard/components/productivity/ActivityFeed.tsx`

- [ ] **Step 1: Migrate DayCalendarView**

Current has complex conditional refetch based on entity kind (focus_session, task, note, transaction, productivity). Add `invalidateOn` to each `useQuery` with appropriate filter:

```typescript
// timeline_query — refreshes on task/focus/productivity changes
const { data: timeline, refetch: refetchTimeline } = useQuery("timeline_query", params, [], {
  invalidateOn: ["entity:updated"],
  invalidateFilter: (p) => {
    const kind = (p as { entityKind?: string })?.entityKind;
    return !kind || ["task", "focus_session", "note", "productivity"].includes(kind ?? "");
  },
});
```

Apply similar patterns to `productivity_today` and `productivity_summary_range` queries. Delete the `useEvent` block.

- [ ] **Step 2: Migrate DayColumnsView**

Current listens to both `"activity:switch"` and `"entity:updated"`. The `"activity:switch"` event is NOT an entity event — it's a different event type. Keep `"activity:switch"` as a separate `useEvent` listener, but migrate the `"entity:updated"` part to `invalidateOn`:

```typescript
const { data: timeline, refetch: refetchTimeline } = useQuery("productivity_timeline", params, [], {
  invalidateOn: ["entity:updated", "activity:switch"],
  invalidateFilter: (p) => {
    const kind = (p as { entityKind?: string })?.entityKind;
    return !kind || kind === "productivity";
  },
});
```

Note: `invalidateOn` accepts any event name, not just `entity:updated`. So `"activity:switch"` can be included directly. The filter won't apply to `activity:switch` payloads (they won't have `entityKind`), but the `!kind` fallback handles this — when there's no entity kind, it invalidates unconditionally. This is correct behavior since `activity:switch` should always trigger a refresh.

Delete both `useEvent` handlers.

- [ ] **Step 3: Migrate GoalsProgress**

```typescript
const { data: goals, refetch } = useQuery("productivity_goals", undefined, [], {
  invalidateOn: ["entity:updated"],
  invalidateFilter: (p) => (p as { entityKind?: string })?.entityKind === "productivity",
});
```

Delete the `useEvent` block. Keep the manual `refetch()` calls in mutation handlers — those are for immediate optimistic feedback.

- [ ] **Step 4: Migrate ActivityFeed**

```typescript
const { data: feed, refetch } = useQuery("productivity_activity_feed", params, [], {
  invalidateOn: ["entity:updated", "activity:switch"],
  invalidateFilter: (p) => {
    const kind = (p as { entityKind?: string })?.entityKind;
    return !kind || kind === "productivity";
  },
});
```

Delete both `useEvent` handlers.

- [ ] **Step 5: Verify all four**

Run: `cd desktop-ui && bunx biome check src/features/dashboard/components/DayCalendarView.tsx src/features/dashboard/components/DayColumnsView.tsx src/features/dashboard/components/productivity/GoalsProgress.tsx src/features/dashboard/components/productivity/ActivityFeed.tsx`
Expected: No errors.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/dashboard/components/
git commit -m "refactor(dashboard): migrate to invalidateOn for real-time updates"
```

---

## Task 7: Migrate System Tray

**Files:**
- Modify: `desktop-ui/src/features/tray/pages/SystemTrayPage.tsx`

- [ ] **Step 1: Migrate**

Current:
```typescript
const { data: tasks, refetch: refetchTasks } = useQuery("today_tasks", ...);
useEvent("entity:updated", () => refetchTasks());
```

Replace with:
```typescript
const { data: tasks, refetch: refetchTasks } = useQuery("today_tasks", params, [], {
  invalidateOn: ["entity:updated"],
});
```

No filter needed — the tray should refresh on any entity change (tasks, focus sessions, etc. all affect the daily view).

Delete the `useEvent` block and its import.

- [ ] **Step 2: Verify**

Run: `cd desktop-ui && bunx biome check src/features/tray/pages/SystemTrayPage.tsx`
Expected: No errors.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/tray/pages/SystemTrayPage.tsx
git commit -m "refactor(tray): migrate to invalidateOn for real-time updates"
```

---

## Task 8: Full Build Verification

**Files:** None (verification only)

- [ ] **Step 1: Run frontend lint**

Run: `cd desktop-ui && bun run lint`
Expected: No new errors (pre-existing ones in other files are OK).

- [ ] **Step 2: Run frontend tests**

Run: `cd desktop-ui && bun run test`
Expected: All tests pass.

- [ ] **Step 3: Verify unused imports cleaned up**

Run: `cd desktop-ui && bunx biome check src/features/tasks/hooks/useTasks.ts src/features/notes/ src/features/finance/pages/ src/features/dashboard/components/ src/features/tray/pages/SystemTrayPage.tsx`
Expected: No errors. Specifically verify no unused `useEvent` imports remain.

---

## Task 9: Browser Verification

- [ ] **Step 1: Start dev environment and open browser**

```bash
cargo tauri dev  # terminal 1
cd desktop-ui && bun run dev  # terminal 2
```

Open `http://localhost:1420` in Chrome.

- [ ] **Step 2: Test Tasks page**

1. Open /tasks in browser
2. In another terminal: `curl -s -X POST http://127.0.0.1:3456/api/task_create -H "Content-Type: application/json" -d '{"title":"Real-time test task"}'`
3. Observe: task should appear in the list without refreshing

- [ ] **Step 3: Test Notes page**

1. Open /notes in browser
2. Create a note via MCP or curl
3. Observe: note should appear without refreshing

- [ ] **Step 4: Test Finance page**

1. Open /finance in browser
2. Verify existing data loads
3. If possible, add a transaction via MCP — verify it appears without refresh

- [ ] **Step 5: Test Dashboard**

1. Open /day view
2. Complete a task or start a focus session
3. Verify the dashboard updates in real-time
