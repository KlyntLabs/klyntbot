# Contextual Query Rewriting Phase 4 — Desktop View Wiring Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire the desktop UI's active view state (which dashboard, which entity, what the user is looking at) into the agent's query rewriter so that queries like "break this down" resolve to the specific widget/entity visible on screen — enabling the desktop differentiator described in Moment 6 of the spec.

**Architecture:** A shared `Arc<tokio::sync::Mutex<Option<ActiveView>>>` is created during app init, threaded through the agent builder into `AgentRuntime`, and read in Step 5.5 when building `RetrievalContext`. The frontend calls a new Tauri command `view_set_active` on route changes to push view state to the backend. No polling — frontend pushes on navigation events only.

**Tech Stack:** Rust (Tauri 2 commands, app-core handlers, agent runtime), TypeScript/React (useEffect on route changes, useMutation for IPC), existing `ActiveView` type re-exported from `context_engine`.

**Spec:** `docs/superpowers/specs/2026-03-23-contextual-query-rewriting-design.md` (ActiveView struct, Moment 6, Phase 4 section)

**Prior phases:** Phase 1 (heuristic), Phase 2 (LLM fallback), Phase 3 (autotuner integration) — all complete. The rewriter already handles `active_view` signals (Priority 2 in heuristic, view context in LLM prompt, confidence boost). This phase just provides the data.

---

## File Map

| File | Action | Responsibility |
|------|--------|---------------|
| `crates/desktop-shared/src/commands/view.rs` | Create | `SetActiveViewParams` + `ActiveViewResponse` IPC types |
| `crates/desktop-shared/src/commands/mod.rs` | Modify | Export `view` module |
| `crates/app-core/Cargo.toml` | Modify | Add `context_engine` dependency (L7 → L3, safe) |
| `crates/app-core/src/state.rs` | Modify | Add `active_view: Option<Arc<Mutex<Option<context_engine::ActiveView>>>>` to AppCore |
| `crates/app-core/src/handlers/view.rs` | Create | `view_set_active()` and `view_get_active()` handlers on AppCore |
| `crates/app-core/src/handlers/mod.rs` | Modify | Declare `view` module |
| `crates/app-core/src/init/agent.rs` | Modify | Create shared `Arc<Mutex<Option<ActiveView>>>`, pass to builder |
| `crates/app-core/src/init/mod.rs` | Modify | Store `active_view` in AppCore assembly |
| `crates/agent/src/agent_loop/builder.rs` | Modify | Add `active_view` field + `with_active_view()` setter, thread to runtime |
| `crates/agent/src/agent_runtime/runtime.rs` | Modify | Add `active_view` field + `with_active_view()`, read in Step 5.5 |
| `crates/desktop/src/commands/view.rs` | Create | Tauri commands `view_set_active`, `view_get_active` |
| `crates/desktop/src/commands/mod.rs` | Modify | Register view module + commands |
| `crates/desktop/src/dev_server/dispatch.rs` | Modify | Add view command dispatch |
| `desktop-ui/src/shared/hooks/useActiveView.ts` | Create | Hook that pushes view state on route changes |
| `desktop-ui/src/app/layouts/AppShell.tsx` | Modify | Wire `useActiveView()` |

---

## Task 1: Add IPC types for active view

**Files:**
- Create: `crates/desktop-shared/src/commands/view.rs`
- Modify: `crates/desktop-shared/src/commands/mod.rs`

- [ ] **Step 1: Create the IPC types module**

```rust
// crates/desktop-shared/src/commands/view.rs
use serde::{Deserialize, Serialize};

/// Parameters for setting the active desktop view.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetActiveViewParams {
    /// Dashboard identifier (e.g., "finance", "tasks", "projects", "notes", "dashboard").
    pub dashboard: String,
    /// Specific entity focused within the dashboard (e.g., "FIRE projection", project ID).
    pub focused_entity: Option<String>,
    /// Human-readable description of what the user is looking at.
    /// Used by the LLM rewriter for context (e.g., "March 2026 FIRE projection with variance highlighted").
    pub description: Option<String>,
}

/// Response when getting the current active view.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveViewResponse {
    pub dashboard: Option<String>,
    pub focused_entity: Option<String>,
    pub description: Option<String>,
}
```

- [ ] **Step 2: Export from commands mod**

In `crates/desktop-shared/src/commands/mod.rs`, add:

```rust
pub mod view;
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p desktop-shared`
Expected: SUCCESS

- [ ] **Step 4: Commit**

```bash
git add crates/desktop-shared/src/commands/view.rs crates/desktop-shared/src/commands/mod.rs
git commit -m "feat(desktop-shared): add SetActiveViewParams IPC types"
```

---

## Task 2: Add active_view shared state to AppCore and agent init

**Files:**
- Modify: `crates/app-core/src/state.rs:74` (add field)
- Modify: `crates/app-core/src/init/agent.rs:64` (create Arc<Mutex>)
- Modify: `crates/app-core/src/init/mod.rs:275` (store in AppCore)

- [ ] **Step 1: Add field to AppCore**

In `crates/app-core/src/state.rs`, after `user_situation` (line 74), add:

```rust
    /// Shared active desktop view for query rewriting context.
    /// Updated by Tauri command on navigation; read by agent runtime in Step 5.5.
    pub active_view:
        Option<Arc<Mutex<Option<context_engine::ActiveView>>>>,
```

- [ ] **Step 2: Create shared state in agent init**

In `crates/app-core/src/init/agent.rs`, after `user_situation` creation (line 64), add:

```rust
    // Pre-create active view (None until frontend pushes a view).
    // Shared with AgentRuntime for RetrievalContext.active_view.
    let active_view: Arc<Mutex<Option<context_engine::ActiveView>>> =
        Arc::new(Mutex::new(None));
```

Pass to agent builder (after `.with_user_situation(user_situation.clone())` at line 84):

```rust
        .with_active_view(active_view.clone())
```

Add `active_view` to the return value. Find the `AgentResult` struct (or the return expression) and add `active_view` alongside `user_situation`.

- [ ] **Step 3: Store in AppCore assembly**

In `crates/app-core/src/init/mod.rs`, in the `AppCore { ... }` struct literal (after `user_situation: Some(user_situation)` at line 275), add:

```rust
            active_view: Some(active_view),
```

Make sure the `active_view` variable is destructured from the agent result above.

- [ ] **Step 4: Add `context_engine` dependency to app-core**

In `crates/app-core/Cargo.toml`, add to `[dependencies]`:

```toml
context_engine = { path = "../context_engine" }
```

- [ ] **Step 5: Verify compilation (partial)**

Run: `cargo check -p app-core`
Expected: FAIL — `AgentLoopBuilder::with_active_view` not yet defined (Task 3). This is expected; the `app-core` types are correct but the agent builder needs its matching setter.

**Note:** Do NOT commit yet — Tasks 2 and 3 are committed together to avoid a broken intermediate state.

---

## Task 3: Thread active_view through agent builder and runtime

**Files:**
- Modify: `crates/agent/src/agent_loop/builder.rs:84,152-158,1409-1414`
- Modify: `crates/agent/src/agent_runtime/runtime.rs:99-101,199-211,480-487`

- [ ] **Step 1: Add field and setter to AgentLoopBuilder**

In `crates/agent/src/agent_loop/builder.rs`, add to the `AgentLoopBuilder` struct (after `user_situation` at line 84):

```rust
    active_view: Option<Arc<tokio::sync::Mutex<Option<context_engine::ActiveView>>>>,
```

Add to `Self { ... }` in `new()` (alongside other `None` initializations):

```rust
            active_view: None,
```

Add setter method (after `with_user_situation` at line 158):

```rust
    pub fn with_active_view(
        mut self,
        view: Arc<tokio::sync::Mutex<Option<context_engine::ActiveView>>>,
    ) -> Self {
        self.active_view = Some(view);
        self
    }
```

- [ ] **Step 2: Pass to AgentRuntime in build()**

In the `build()` method, after `runtime = runtime.with_user_situation(...)` (line 1410), add:

```rust
        // Inject active view for RetrievalContext
        if let Some(ref view) = self.active_view {
            runtime = runtime.with_active_view(Arc::clone(view));
        }
```

- [ ] **Step 3: Add field and setter to AgentRuntime**

In `crates/agent/src/agent_runtime/runtime.rs`, add to the `AgentRuntime` struct (after `task_repo` at line 101):

```rust
    /// Shared active desktop view for query rewriting context.
    active_view: Option<Arc<tokio::sync::Mutex<Option<context_engine::ActiveView>>>>,
```

Add `active_view: None,` to `Self { ... }` in `new()`.

Add setter method (after `with_task_repo` at line 211):

```rust
    pub fn with_active_view(
        mut self,
        view: Arc<tokio::sync::Mutex<Option<context_engine::ActiveView>>>,
    ) -> Self {
        self.active_view = Some(view);
        self
    }
```

- [ ] **Step 4: Read active_view in Step 5.5**

In `process_message()`, in the Step 5.5 block (line 480-487), replace:

```rust
                active_view: None,
```

With:

```rust
                active_view: if let Some(ref view_lock) = self.active_view {
                    view_lock.lock().await.clone()
                } else {
                    None
                },
```

- [ ] **Step 5: Verify compilation**

Run: `cargo check -p agent`
Expected: SUCCESS

- [ ] **Step 6: Run existing tests**

Run: `cargo nextest run -p agent -E 'test(query_rewriter)'`
Expected: ALL PASS (existing active_view tests already exercise the rewriter logic)

- [ ] **Step 7: Commit (includes Task 2 files — kept together for compilable commit)**

```bash
git add crates/app-core/Cargo.toml crates/app-core/src/state.rs crates/app-core/src/init/agent.rs crates/app-core/src/init/mod.rs crates/agent/src/agent_loop/builder.rs crates/agent/src/agent_runtime/runtime.rs
git commit -m "feat(app-core,agent): add active_view shared state and thread to RetrievalContext"
```

---

## Task 4: Add AppCore handler for view updates

**Files:**
- Create: `crates/app-core/src/handlers/view.rs`
- Modify: `crates/app-core/src/handlers/mod.rs`

- [ ] **Step 1: Create the handler module**

```rust
// crates/app-core/src/handlers/view.rs
use desktop_shared::commands::view::{ActiveViewResponse, SetActiveViewParams};
use desktop_shared::errors::ApiError;

use crate::AppCore;

impl AppCore {
    /// Update the shared active view from a desktop navigation event.
    pub async fn view_set_active(&self, params: SetActiveViewParams) -> Result<(), ApiError> {
        let Some(ref view_lock) = self.active_view else {
            return Ok(()); // No-op if active_view not initialized
        };

        let new_view = context_engine::ActiveView {
            dashboard: params.dashboard,
            focused_entity: params.focused_entity,
            description: params.description,
        };

        *view_lock.lock().await = Some(new_view);
        Ok(())
    }

    /// Clear the active view (e.g., when navigating to chat-only page).
    pub async fn view_clear_active(&self) -> Result<(), ApiError> {
        if let Some(ref view_lock) = self.active_view {
            *view_lock.lock().await = None;
        }
        Ok(())
    }

    /// Get the current active view.
    pub async fn view_get_active(&self) -> Result<ActiveViewResponse, ApiError> {
        let view = if let Some(ref view_lock) = self.active_view {
            view_lock.lock().await.clone()
        } else {
            None
        };

        Ok(match view {
            Some(v) => ActiveViewResponse {
                dashboard: Some(v.dashboard),
                focused_entity: v.focused_entity,
                description: v.description,
            },
            None => ActiveViewResponse {
                dashboard: None,
                focused_entity: None,
                description: None,
            },
        })
    }
}
```

- [ ] **Step 2: Declare in handlers mod**

In `crates/app-core/src/handlers/mod.rs`, add:

```rust
mod view;
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p app-core`
Expected: SUCCESS

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/handlers/view.rs crates/app-core/src/handlers/mod.rs
git commit -m "feat(app-core): add view_set_active and view_get_active handlers"
```

---

## Task 5: Add Tauri commands + dev server dispatch

**Files:**
- Create: `crates/desktop/src/commands/view.rs`
- Modify: `crates/desktop/src/commands/mod.rs`
- Modify: `crates/desktop/src/dev_server/dispatch.rs`

- [ ] **Step 1: Create Tauri command module**

```rust
// crates/desktop/src/commands/view.rs
use desktop_shared::commands::view::{ActiveViewResponse, SetActiveViewParams};
use desktop_shared::errors::ApiError;
use std::sync::Arc;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn view_set_active(
    state: State<'_, Arc<AppCore>>,
    params: SetActiveViewParams,
) -> Result<(), ApiError> {
    state.view_set_active(params).await
}

#[tauri::command]
pub async fn view_clear_active(
    state: State<'_, Arc<AppCore>>,
) -> Result<(), ApiError> {
    state.view_clear_active().await
}

#[tauri::command]
pub async fn view_get_active(
    state: State<'_, Arc<AppCore>>,
) -> Result<ActiveViewResponse, ApiError> {
    state.view_get_active().await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "view_set_active",
    "view_clear_active",
    "view_get_active",
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "view_set_active" => dev::val(core.view_set_active(try_field!(dev::parse_params(body))).await),
        "view_clear_active" => dev::val(core.view_clear_active().await),
        "view_get_active" => dev::val(core.view_get_active().await),
        _ => return None,
    })
}
```

- [ ] **Step 2: Register in commands mod.rs**

In `crates/desktop/src/commands/mod.rs`:

1. Add `pub mod view;` to module declarations.
2. Add `view_set_active, view_clear_active, view_get_active` to the Tauri `invoke_handler` registration list.
3. Add `view::DEV_COMMANDS` to the dev commands collection.

- [ ] **Step 3: Add dispatch chain in dev_server/dispatch.rs**

In `crates/desktop/src/dev_server/dispatch.rs`, add alongside other dispatch chains:

```rust
    if let Some(r) = commands::view::dispatch_dev(cmd, core, &body).await {
        return into_api_result(r);
    }
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p desktop`
Expected: SUCCESS

- [ ] **Step 5: Run dev server parity test**

Run: `cargo nextest run -p desktop -E 'test(dev_server_covers_all_tauri_commands)'`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/desktop/src/commands/view.rs crates/desktop/src/commands/mod.rs crates/desktop/src/dev_server/dispatch.rs
git commit -m "feat(desktop): add view_set_active Tauri commands + dev server dispatch"
```

---

## Task 6: Frontend hook — push view state on navigation

**Files:**
- Create: `desktop-ui/src/shared/hooks/useActiveView.ts`
- Modify: `desktop-ui/src/app/layouts/AppShell.tsx`

- [ ] **Step 1: Create the useActiveView hook**

```typescript
// desktop-ui/src/shared/hooks/useActiveView.ts
import { useEffect, useRef } from "react";
import { useLocation } from "react-router";

import { useMutation } from "@shared/hooks/useMutation";

interface ActiveViewParams {
  dashboard: string;
  focusedEntity?: string | null;
  description?: string | null;
}

/**
 * Pushes the active desktop view to the backend on route changes.
 * The agent's query rewriter uses this to enrich vague queries with
 * what the user is currently looking at (Moment 6: dashboard + chat synergy).
 */
export function useActiveView() {
  const { pathname, search } = useLocation();
  const { mutate: setActive } = useMutation<void, ActiveViewParams>(
    "view_set_active",
    "params",
  );
  const { mutate: clearActive } = useMutation<void, Record<string, never>>(
    "view_clear_active",
  );
  const lastRef = useRef<string>("");

  useEffect(() => {
    const key = `${pathname}?${search}`;
    if (key === lastRef.current) return;
    lastRef.current = key;

    const view = deriveActiveView(pathname, search);
    if (view) {
      setActive(view);
    } else {
      clearActive();
    }
  }, [pathname, search, setActive, clearActive]);
}

function deriveActiveView(
  pathname: string,
  search: string,
): ActiveViewParams | null {
  // Finance views
  if (pathname === "/finance")
    return { dashboard: "finance", description: "Finance overview dashboard" };
  const finSub = pathname.match(/^\/finance\/(.+)$/);
  if (finSub) {
    const sub = finSub[1];
    const labels: Record<string, string> = {
      cashflow: "Cash flow analysis",
      investments: "Investment portfolio",
      targets: "Savings and allocation targets",
    };
    return {
      dashboard: "finance",
      focusedEntity: sub,
      description: labels[sub] ?? `Finance ${sub}`,
    };
  }

  // Task views
  if (pathname === "/" || pathname.startsWith("/tasks")) {
    const params = new URLSearchParams(search);
    const tab = params.get("tab");
    return {
      dashboard: "tasks",
      focusedEntity: tab,
      description: tab ? `Tasks ${tab} view` : "Tasks overview",
    };
  }

  // Project detail
  const projectMatch = pathname.match(/^\/project\/(.+?)(?:\/|$)/);
  if (projectMatch)
    return {
      dashboard: "projects",
      focusedEntity: projectMatch[1],
      description: `Project detail view`,
    };

  // Projects list
  if (pathname === "/projects")
    return { dashboard: "projects", description: "Projects list" };

  // Notes / Knowledge base
  if (pathname === "/notes" || pathname.startsWith("/notes"))
    return { dashboard: "notes", description: "Knowledge base" };

  // Learning
  if (pathname.startsWith("/learn"))
    return { dashboard: "learning", description: "Learning and review" };

  // Coaching
  if (pathname.startsWith("/coaching"))
    return { dashboard: "coaching", description: "Coaching overview" };

  // Dashboard (day/week/month/year views)
  if (
    pathname.startsWith("/day/") ||
    pathname.startsWith("/week/") ||
    pathname.startsWith("/month/") ||
    pathname.startsWith("/year/")
  )
    return { dashboard: "dashboard", description: "Daily planner" };

  // OKR / Objectives
  const objMatch = pathname.match(/^\/objective\/(.+)$/);
  if (objMatch)
    return {
      dashboard: "okr",
      focusedEntity: objMatch[1],
      description: "Objective detail",
    };

  // Automations
  if (pathname === "/automations")
    return { dashboard: "automations", description: "Automations overview" };

  // Chat, launcher, tray, settings — no view context
  return null;
}
```

- [ ] **Step 2: Wire in AppShell**

In `desktop-ui/src/app/layouts/AppShell.tsx`, add import and call:

```typescript
import { useActiveView } from "@shared/hooks/useActiveView";

// Inside the AppShell component, after existing hooks:
useActiveView();
```

- [ ] **Step 3: Verify frontend builds**

Run: `cd desktop-ui && bun run build`
Expected: SUCCESS

- [ ] **Step 4: Verify lint passes**

Run: `cd desktop-ui && bun run lint:fix`
Expected: clean or auto-fixed

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/shared/hooks/useActiveView.ts desktop-ui/src/app/layouts/AppShell.tsx
git commit -m "feat(desktop-ui): push active view to backend on route navigation"
```

---

## Task 7: End-to-end test and final verification

- [ ] **Step 1: Write a Rust integration test for the full data path**

In `crates/agent/src/adapters/query_rewriter.rs` tests, add:

```rust
#[tokio::test]
async fn active_view_flows_through_shared_state() {
    use std::sync::Arc;

    // Simulate the shared state pattern
    let view_lock = Arc::new(tokio::sync::Mutex::new(None::<context_engine::ActiveView>));

    // Initially None — rewriter should not inject view signals
    let rewriter = ContextualQueryRewriter::heuristic_only();
    let ctx_no_view = RetrievalContext::default();
    let result = rewriter.rewrite("break this down", &ctx_no_view).await;
    assert!(result.is_none(), "No context → no rewrite");

    // Simulate frontend pushing a view
    *view_lock.lock().await = Some(context_engine::ActiveView {
        dashboard: "finance".into(),
        focused_entity: Some("FIRE projection".into()),
        description: Some("March 2026 FIRE projection with variance".into()),
    });

    // Read from shared state (same as runtime Step 5.5 would)
    let view = view_lock.lock().await.clone();
    let ctx_with_view = RetrievalContext {
        active_view: view,
        ..Default::default()
    };
    let result = rewriter.rewrite("break this down", &ctx_with_view).await;
    assert!(result.is_some(), "View context should produce a rewrite");
    let enriched = result.unwrap().enriched_query.to_lowercase();
    assert!(
        enriched.contains("fire projection"),
        "Enriched query should include the focused entity, got: {enriched}"
    );

    // Simulate navigation away (clear view)
    *view_lock.lock().await = None;
    let view = view_lock.lock().await.clone();
    let ctx_cleared = RetrievalContext {
        active_view: view,
        ..Default::default()
    };
    let result = rewriter.rewrite("break this down", &ctx_cleared).await;
    assert!(result.is_none(), "Cleared view → no rewrite");
}
```

- [ ] **Step 2: Run the test**

Run: `cargo nextest run -p agent -E 'test(active_view_flows)'`
Expected: PASS

- [ ] **Step 3: Run full verification**

Run: `cargo fmt --all --check`
Run: `cargo clippy --workspace --all-targets --all-features`
Run: `cargo nextest run --workspace`
Run: `cargo test --workspace --doc`
Expected: ALL PASS, 0 warnings

- [ ] **Step 4: Run frontend tests**

Run: `cd desktop-ui && bun run test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/agent/src/adapters/query_rewriter.rs
git commit -m "test(agent): end-to-end test for active_view shared state flow"
```

---

## Summary

| Task | Description | Lines | Dependencies |
|------|-------------|-------|-------------|
| 1 | IPC types (`SetActiveViewParams`, `ActiveViewResponse`) | ~25 | None |
| 2 | Shared state in AppCore + agent init | ~15 | None |
| 3 | Thread through agent builder → runtime → Step 5.5 | ~35 | Task 2 |
| 4 | AppCore handlers (`view_set_active`, `view_clear_active`, `view_get_active`) | ~45 | Task 2 |
| 5 | Tauri commands + dev server dispatch | ~55 | Tasks 1, 4 |
| 6 | Frontend hook (`useActiveView`) + AppShell wiring | ~95 | Task 5 |
| 7 | End-to-end test + full verification | ~40 | Tasks 3, 6 |

**Total: ~310 lines of new/changed code**

**What Phase 4 delivers:**
- "break this down" while viewing the FIRE projection → enriched query includes "FIRE projection" and finance context
- "what's wrong here?" on the task board → enriched query includes task domain context
- "explain this" on any dashboard widget → the agent knows what "this" refers to
- Zero latency overhead — frontend pushes on navigation, backend reads from memory (no DB, no IPC per-message)
- Graceful degradation — when no view is set (chat-only, launcher, tray), the rewriter skips the signal as it always has
- The last piece of the contextual query rewriting system: Phase 1 (heuristic) → Phase 2 (LLM) → Phase 3 (autotuner) → **Phase 4 (desktop synergy)**
