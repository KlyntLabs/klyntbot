# Task2 Integration — Remaining Work

**Date:** 2026-03-15
**Status:** Tracking
**Context:** The task2 UI is wired to real data. This document tracks features that are still stubbed or placeholder.

---

## 1. Activity Tab — Richer Event Logging

**Current:** Shows `timeline_query` entries filtered by `entityId`. Only system-generated events appear (e.g., "created task"). No status change history, no comments, no agent actions.

**Needed:**
- Log status/priority changes as timeline entries when `task_update` is called
- Log agent actions (suggestions applied, linked issues) as timeline entries
- Support comment-type entries (user-authored text)
- Map actor types properly: "You" for user actions, "Klyntbot" for agent, "System" for automated

**Backend:** `crates/app-core/src/handlers/tasks/crud.rs` — `task_update()` should emit richer `DomainEvent` variants that the timeline system captures.

**Frontend:** `desktop-ui/src/features/tasks2/components/detail/IssueActivityTab.tsx` — already renders `ActivityEntry[]`, just needs richer data.

---

## 2. AI Suggestions

**Current:** Returns empty `[]`. Shows static "Why This Task Now?" card with hardcoded reasons.

**Needed:**
- Backend endpoint to generate task-specific suggestions (break into subtasks, related issues, priority recommendations)
- Suggestion apply/dismiss persistence
- Confidence scoring

**Backend:** No endpoint exists. Would need a new handler in `app-core` that uses the agent pipeline to analyze a task and produce suggestions.

**Frontend:** `desktop-ui/src/features/tasks2/components/detail/SidebarAiInsights.tsx` — fully built, just needs real data. `useIssueDetail.ts` returns empty suggestions — wire to new endpoint when ready.

---

## 3. Task Memory (Cognitive Bridge)

**Current:** Returns `null`. The "Task Memory" section (last session summary, continuity note, related facts) doesn't render.

**Needed:**
- Bridge `crates/cognitive/` memory system to per-task context
- Endpoint to fetch memory entries related to a specific task
- Surface last session summary, continuity notes, and related facts

**Backend:** The cognitive memory system exists in `crates/cognitive/`. Needs a query interface filtered by task entity.

**Frontend:** `desktop-ui/src/features/tasks2/components/detail/SidebarAiInsights.tsx` — `WhatAiLearned` and `TaskMemorySection` components are built, guarded by null checks. Wire `taskMemory` in `useIssueDetail.ts` when endpoint is ready.

---

## 4. Focus Session — Real Metrics + Controls

**Current:** `deriveFocusSession()` returns hardcoded defaults when `task.focusedAt` is set. Pause/Stop buttons are `disabled`.

**Needed:**
- Wire Pause button to pause the focus timer (no backend endpoint yet)
- Wire Stop button to `task_end_focus` mutation
- Fetch real quality metrics (quality score, distraction count, flow state) from the focus tracking system
- Quality history sparkline from real 5-minute bucket data

**Backend:** `task_start_focus` and `task_end_focus` Tauri commands exist. Quality metrics may need a new query or be part of the focus session response.

**Frontend:**
- `desktop-ui/src/features/tasks2/components/detail/SidebarWorkState.tsx` — enable Pause/Stop buttons, wire to mutations
- `desktop-ui/src/features/tasks2/hooks/useIssueDetail.ts` — replace `deriveFocusSession()` with real data query

---

## 5. Status Workflow Integration

**Current:** 6 hardcoded statuses in `lib/status-icons.tsx` with static `backendStatus` mapping. Status pickers show these fixed options regardless of project workflow.

**Needed:**
- Fetch real `StatusWorkflow` labels via `useEffectiveLabels(projectId)` (hook already exists in the old tasks feature)
- Dynamically populate status pickers from workflow labels
- Map workflow label colors/names to icons (option C from spec — match known names, fallback to colored circle)
- Per-project workflow support (different projects can have different status options)

**Backend:** Fully built — `StatusWorkflow`, `StatusLabel`, and CRUD endpoints all exist.

**Frontend:**
- `StatusSelector.tsx`, `SidebarProperties.tsx`, `IssueContextMenu.tsx`, `IssueBoard.tsx`, `GroupIssues.tsx`, `Filter.tsx` — all iterate `status` array from `status-icons.tsx`. Replace with dynamic workflow labels.
- `lib/mappers.ts` — `resolveStatus()` already handles custom labels as fallback. Would become primary path.

---

## 6. Project Icons

**Current:** All projects show a generic `Folder` icon via `projectToDisplayProject()`.

**Needed:**
- Map project color to a distinct icon, or add an `icon` field to the `Project` backend type
- Show colored dot + project name in `ProjectBadge.tsx`

**Scope:** Cosmetic. Low priority.

---

## Priority Order

| # | Feature | Impact | Effort |
|---|---|---|---|
| 1 | Status Workflow Integration | High — correct status options per project | Medium |
| 2 | Focus Session Controls | High — core productivity feature | Low |
| 3 | Activity Tab Enrichment | Medium — better task history | Medium |
| 4 | AI Suggestions | Medium — differentiating feature | High |
| 5 | Task Memory | Medium — continuity across sessions | Medium |
| 6 | Project Icons | Low — cosmetic | Low |
