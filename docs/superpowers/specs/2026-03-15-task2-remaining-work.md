# Task2 Integration — Status Tracker

**Date:** 2026-03-15
**Last updated:** 2026-03-15
**Context:** The task2 UI is wired to real data. This document tracks what's done and what's still stubbed or placeholder.

---

## Completed Features

### ✅ Core Data Integration (2026-03-14)

All core task views are wired to real backend data:
- Task list, board, grid views via `task_list`
- Task detail view via `task_get`
- Sub-issues via `task_list_children`
- Create / update / delete / toggle-complete mutations
- Projects and areas from real data
- Tags/labels from task tags
- Real-time updates via `entity:updated` events (Tauri) + refetch after mutations (browser dev)

### ✅ Status Workflow Integration (2026-03-15)

**What was done:**
- `StatusWorkflowProvider` context fetches real workflow labels per-project via `useEffectiveLabels(projectId)`
- `matchIcon()` maps known status names to SVG icons; unknown names get a colored circle fallback
- All 7+ consumer components (`StatusSelector`, `SidebarProperties`, `IssueContextMenu`, `IssueBoard`, `AllIssues`, `CreateIssueModal`, `Filter`) use dynamic statuses from context
- Board columns derive from workflow labels
- Drag-and-drop sends `status` + `statusLabelId` in mutations
- Graceful degradation: tasks with unmatched statuses are bucketed by `statusGroup`
- `resolveStatus(task, labels)` is a pure function accepting `StatusLabel[]`

**Key files:**
- `desktop-ui/src/features/tasks2/contexts/StatusWorkflowContext.tsx`
- `desktop-ui/src/features/tasks2/lib/status-icons.tsx` — icon registry + `matchIcon()`
- `desktop-ui/src/features/tasks2/lib/mappers.ts` — `resolveStatus()`, `statusToMutationParams()`

### ✅ Focus Session Controls (2026-03-15)

**What was done:**
- `FocusSession` interface replaced — no fake data, nullable quality fields
- `buildFocusSession(task)` returns real `startedAt`/`elapsed`/`totalTracked`, null for quality metrics
- Stop button wired to `task_end_focus` mutation
- Quality score, distraction count, flow state, sparkline sections hidden when null
- Pause button disabled with "Coming soon" tooltip

**Key files:**
- `desktop-ui/src/features/tasks2/lib/mappers.ts` — `FocusSession`, `buildFocusSession()`
- `desktop-ui/src/features/tasks2/hooks/useIssueDetail.ts` — `stopFocus` callback
- `desktop-ui/src/features/tasks2/components/detail/SidebarWorkState.tsx`

### ✅ Activity Tab Enrichment (2026-03-15)

**What was done:**
- Backend: `task_update` diffs old vs new, emits `TaskStatusChanged`, `TaskPriorityChanged`, `TaskFieldUpdated` domain events with actor info
- Backend: `normalize_domain_event` handles new events → timeline entries with metadata
- Backend: actor stored in `metadata.actor` (`"user"` / `"agent"` / `"system"`)
- Frontend: `timelineToActivity()` enriched with `resolveActor()` and `buildActivityDetail()`
- Activity shows "You changed status: todo → in_progress", "You changed priority: Medium → High"

**Key files:**
- `crates/bus/src/domain_events.rs` — 3 new `DomainEvent` variants
- `crates/app-core/src/handlers/tasks/crud.rs` — diff + emit in `task_update`
- `crates/app-core/src/handlers/timeline.rs` — `normalize_domain_event` handlers
- `desktop-ui/src/features/tasks2/lib/mappers.ts` — `resolveActor()`, `buildActivityDetail()`

---

## Remaining Work (Placeholder / Stubbed)

### 1. AI Suggestions — High effort

**Current state:** Returns `[]`. Shows static "Why This Task Now?" card with hardcoded reasons.

**What's needed:**
- New backend handler in `app-core` that uses the agent pipeline to analyze a task and produce suggestions (break into subtasks, related issues, priority recommendations)
- Suggestion apply/dismiss persistence
- Confidence scoring

**Frontend is ready:** `SidebarAiInsights.tsx` is fully built with apply/dismiss UI, confidence badges, expandable list. Just needs real data from `useIssueDetail.ts → suggestions`.

**Backend:** No endpoint exists yet.

### 2. Task Memory (Cognitive Bridge) — Medium effort

**Current state:** Returns `null`. "Task Memory" section doesn't render.

**What's needed:**
- Bridge `crates/cognitive/` memory system to per-task context
- Endpoint to fetch memory entries related to a specific task
- Surface last session summary, continuity notes, and related facts

**Frontend is ready:** `WhatAiLearned` and `TaskMemorySection` components in `SidebarAiInsights.tsx` are built, guarded by null checks. Wire `taskMemory` in `useIssueDetail.ts` when endpoint is ready.

**Backend:** Cognitive memory system exists. Needs a query interface filtered by task entity.

### 3. Focus Quality Metrics — Medium effort

**Current state:** Quality score, distraction count, flow state, and sparkline are hidden (fields return `null`).

**What's needed:**
- Backend tracking system for focus quality (quality score per 5-minute bucket, distraction events, flow state detection)
- Endpoint or extension to `task_get` response with quality data
- Pause/resume focus support (new DB fields + Tauri command)

**Frontend is ready:** `SidebarWorkState.tsx` has quality score display, distraction counter, `FlowBadge`, and `QualitySparkline` components. All null-guarded — they render automatically when data is non-null.

### 4. Project Icons — Low effort

**Current state:** All projects show a generic `Folder` icon.

**What's needed:**
- Map project color to a distinct icon, or add an `icon` field to the `Project` backend type
- Show colored dot + project name in `ProjectBadge.tsx`

**Scope:** Cosmetic. Low priority.

### 5. Activity — Comments & Agent Actions — Medium effort

**Current state:** Activity tab shows task lifecycle events and field changes. No user comments, no agent action logging.

**What's needed:**
- Comment-type timeline entries (user-authored text) — needs a comments system
- Agent action logging (suggestions applied, linked issues) — depends on AI Suggestions being built first

---

## Priority for Next Session

| # | Feature | Impact | Effort | Dependencies |
|---|---------|--------|--------|-------------|
| 1 | AI Suggestions | Medium — differentiating feature | High | Agent pipeline |
| 2 | Task Memory | Medium — continuity across sessions | Medium | Cognitive system bridge |
| 3 | Focus Quality Metrics | Medium — enriches focus tracking | Medium | Quality tracking backend |
| 4 | Project Icons | Low — cosmetic | Low | None |
| 5 | Activity Comments | Low — nice to have | Medium | Comments system |

---

## Design & Plan References

- **Design spec:** `docs/superpowers/specs/2026-03-15-task2-remaining-b-design.md`
- **Implementation plan:** `docs/superpowers/plans/2026-03-15-task2-remaining-b.md`
- **Original integration spec:** `docs/superpowers/specs/2026-03-14-task2-integration-design.md`
- **Detail view spec:** `docs/superpowers/specs/2026-03-13-task-detail-view-design.md`
