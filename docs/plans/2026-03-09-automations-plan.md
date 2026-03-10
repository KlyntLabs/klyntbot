# Automations Page Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add an "Automations" page to the desktop UI for managing cron jobs with proper backend classification.

**Architecture:** Add `CronOrigin` enum to the scheduling crate + migration, create app-core handlers + Tauri commands (thin adapter pattern), and build a React page with inline-expand rows, origin filter tabs, and a guided schedule builder.

**Tech Stack:** Rust (scheduling, storage, app-core, desktop, desktop-shared crates), React 19, TypeScript, Tailwind v4 CSS tokens, lucide-react icons.

---

### Task 1: Add `CronOrigin` enum and update `CronJob` struct

**Files:**
- Modify: `crates/scheduling/src/types.rs:87-113` (CronJob struct)
- Modify: `crates/storage/src/rows/cron.rs:1-22` (CronJobRow)
- Modify: `crates/storage/migrations/001_initial.sql:257-270` (cron_jobs DDL)

**Step 1: Add CronOrigin enum to types.rs**

Add above the `CronJob` struct (after line 85):

```rust
/// Origin of a cron job — who created it
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CronOrigin {
    System,
    User,
    Ai,
    Plugin,
}

impl Default for CronOrigin {
    fn default() -> Self {
        Self::System
    }
}
```

**Step 2: Add `origin` field to `CronJob`**

Add after the `enabled` field (line 95):

```rust
    #[serde(default)]
    pub origin: CronOrigin,
```

**Step 3: Update `CronJob::new()` to accept origin**

Change the `new` method signature to include `origin: CronOrigin` and set it on the struct.

**Step 4: Add `origin` to `CronJobRow`**

In `crates/storage/src/rows/cron.rs`, add:

```rust
    pub origin: String,
```

**Step 5: Update `CronRepo` SQL queries**

In `crates/storage/src/repos/cron.rs`:
- Add `origin` to the `upsert` INSERT/UPDATE columns and bindings
- The `list`, `list_active`, and `get` queries use `SELECT *` so they auto-include it

**Step 6: Update migration DDL**

In `crates/storage/migrations/001_initial.sql`, add to the `cron_jobs` table:

```sql
    origin           TEXT NOT NULL DEFAULT 'system',
```

**Step 7: Update `job_to_row` and `row_to_job` in CronService**

In `crates/scheduling/src/service/mod.rs:391-428`:
- `job_to_row`: add `origin: job.origin.to_string()` (serialize enum to lowercase string)
- `row_to_job`: add `origin: serde_json::from_value(serde_json::Value::String(row.origin)).unwrap_or_default()`

Use simpler approach: `origin: match row.origin.as_str() { "user" => CronOrigin::User, "ai" => CronOrigin::Ai, "plugin" => CronOrigin::Plugin, _ => CronOrigin::System }`

**Step 8: Update `add_job` to accept origin**

In `crates/scheduling/src/service/mod.rs:278-311`, add `origin: CronOrigin` parameter to `add_job()`.

**Step 9: Update existing tests**

Update all test calls to `add_job` and `CronJob::new` to include the new `origin` parameter (use `CronOrigin::System` or `CronOrigin::User` as appropriate).

**Step 10: Add serde test for CronOrigin**

In `crates/scheduling/src/types.rs` tests:

```rust
#[test]
fn test_cron_origin_serde() {
    let origin = CronOrigin::Ai;
    let json = serde_json::to_value(&origin).unwrap();
    assert_eq!(json, "ai");
    let deserialized: CronOrigin = serde_json::from_value(json).unwrap();
    assert_eq!(deserialized, CronOrigin::Ai);
}
```

**Step 11: Run tests**

```bash
cargo nextest run -p scheduling -p storage
```

**Step 12: Run clippy**

```bash
cargo clippy --workspace --all-targets --all-features
```

**Step 13: Commit**

```
feat(scheduling): add CronOrigin enum to classify job sources
```

---

### Task 2: Update all CronJob creation points to set origin

**Files:**
- Modify: `crates/app-core/src/init.rs:444-835` (ensure_cron_jobs — system jobs)
- Modify: `crates/tools/src/cron_tool.rs:135-207` (CronTool — AI-created jobs)
- Modify: `crates/agent/src/cron_handler_adapter.rs:31-55` (adapter — pass origin through)
- Modify: `crates/agent/src/agent_loop/builder.rs` (plugin job registration)

**Step 1: Update ensure_cron_jobs to pass CronOrigin::System**

All calls to `cron_service.add_job(...)` in `ensure_cron_jobs` should pass `CronOrigin::System`.

**Step 2: Update CronHandler trait to include origin**

In `crates/tools/src/cron_tool.rs`, add `origin` field to `AddCronJobParams`:

```rust
pub struct AddCronJobParams {
    pub name: String,
    pub schedule: CronSchedule,
    pub message: String,
    pub enabled: bool,
    pub channel: Option<String>,
    pub to: Option<String>,
    pub internal: bool,
    pub origin: scheduling::CronOrigin, // NEW
}
```

Wait — `tools` crate can't depend on `scheduling` (layer violation: tools is L4, scheduling is L3). Instead, add a string field:

```rust
    pub origin: String, // "system", "user", "ai", "plugin"
```

**Step 3: Update CronHandlerAdapter to map origin**

In `crates/agent/src/cron_handler_adapter.rs`, convert `params.origin` string to `CronOrigin` enum:

```rust
let origin = match params.origin.as_str() {
    "user" => scheduling::CronOrigin::User,
    "ai" => scheduling::CronOrigin::Ai,
    "plugin" => scheduling::CronOrigin::Plugin,
    _ => scheduling::CronOrigin::System,
};
```

Pass it to `self.service.add_job(...)`.

**Step 4: Set origin = "ai" in CronTool execute**

In `crates/tools/src/cron_tool.rs:169-177`, set `origin: "ai".to_string()` on the `AddCronJobParams`.

**Step 5: Set origin = "plugin" for plugin jobs**

In `crates/agent/src/agent_loop/builder.rs`, find where `AddCronJobParams` is constructed for plugin jobs and set `origin: "plugin".to_string()`.

**Step 6: Run tests**

```bash
cargo nextest run --workspace
```

**Step 7: Commit**

```
feat(scheduling): set CronOrigin at all job creation points
```

---

### Task 3: Add desktop-shared types for cron IPC

**Files:**
- Modify: `crates/desktop-shared/src/types.rs` (add cron response/params types)
- Modify: `crates/desktop-shared/src/types.rs` (add `EntityKind::Cron` variant if needed)

**Step 1: Add CronJobResponse type**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CronJobResponse {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub origin: String,
    pub schedule: serde_json::Value,
    pub payload: CronPayloadResponse,
    pub state: CronJobStateResponse,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub delete_after_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CronPayloadResponse {
    pub kind: String,
    pub message: String,
    pub deliver: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CronJobStateResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_run_at_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_run_at_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}
```

**Step 2: Add CronJobCreateParams and CronJobUpdateParams**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CronJobCreateParams {
    pub name: String,
    pub schedule: serde_json::Value,
    pub message: String,
    #[serde(default)]
    pub deliver: bool,
    pub channel: Option<String>,
    pub to: Option<String>,
    #[serde(default)]
    pub delete_after_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CronJobUpdateParams {
    pub id: String,
    pub name: Option<String>,
    pub schedule: Option<serde_json::Value>,
    pub message: Option<String>,
    pub deliver: Option<bool>,
    pub channel: Option<Option<String>>,
    pub to: Option<Option<String>>,
}
```

**Step 3: Add CronStatusResponse**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CronStatusResponse {
    pub enabled: bool,
    pub jobs: usize,
    pub next_wake_at_ms: Option<i64>,
}
```

**Step 4: Commit**

```
feat(desktop-shared): add cron IPC types
```

---

### Task 4: Add app-core cron handlers

**Files:**
- Create: `crates/app-core/src/handlers/cron.rs`
- Modify: `crates/app-core/src/handlers/mod.rs` (add `pub mod cron;`)

**Step 1: Create the handler file**

Follow the pattern from `handlers/notes.rs`: row converters at top, read-only methods, then mutating methods returning `HandlerResult<T>`.

```rust
//! Cron job handlers — transport-agnostic business logic.

use desktop_shared::types::{
    CronJobCreateParams, CronJobResponse, CronJobStateResponse,
    CronJobUpdateParams, CronPayloadResponse, CronStatusResponse,
};

use crate::state::{AppCore, EntityUpdate, HandlerResult};
use desktop_shared::errors::ApiError;

/// Convert a scheduling::CronJob to the IPC response type.
fn to_response(job: &scheduling::CronJob) -> CronJobResponse {
    CronJobResponse {
        id: job.id.clone(),
        name: job.name.clone(),
        enabled: job.enabled,
        origin: serde_json::to_value(&job.origin)
            .and_then(|v| serde_json::from_value(v))
            .unwrap_or_else(|_| "system".to_string()),
        schedule: serde_json::to_value(&job.schedule).unwrap_or_default(),
        payload: CronPayloadResponse {
            kind: job.payload.kind.clone(),
            message: job.payload.message.clone(),
            deliver: job.payload.deliver,
            channel: job.payload.channel.clone(),
            to: job.payload.to.clone(),
        },
        state: CronJobStateResponse {
            next_run_at_ms: job.state.next_run_at_ms,
            last_run_at_ms: job.state.last_run_at_ms,
            last_status: job.state.last_status.clone(),
            last_error: job.state.last_error.clone(),
        },
        created_at_ms: job.created_at_ms,
        updated_at_ms: job.updated_at_ms,
        delete_after_run: job.delete_after_run,
    }
}

impl AppCore {
    pub async fn cron_list(&self, include_disabled: bool) -> Result<Vec<CronJobResponse>, ApiError> {
        let jobs = self.cron_service.list_jobs(include_disabled).await;
        Ok(jobs.iter().map(to_response).collect())
    }

    pub async fn cron_status(&self) -> Result<CronStatusResponse, ApiError> {
        let status = self.cron_service.status().await;
        Ok(CronStatusResponse {
            enabled: status["enabled"].as_bool().unwrap_or(false),
            jobs: status["jobs"].as_u64().unwrap_or(0) as usize,
            next_wake_at_ms: status["nextWakeAtMs"].as_i64(),
        })
    }

    pub async fn cron_enable(&self, id: String, enabled: bool) -> HandlerResult<CronJobResponse> {
        let job = self.cron_service.enable_job(&id, enabled).await
            .map_err(|e| ApiError::new("CRON_ERROR", &e.to_string()))?
            .ok_or_else(|| ApiError::new("NOT_FOUND", &format!("cron job '{id}'")))?;
        Ok((to_response(&job), vec![]))
    }

    pub async fn cron_run(&self, id: String) -> Result<bool, ApiError> {
        self.cron_service.run_job(&id, true).await
            .map_err(|e| ApiError::new("CRON_ERROR", &e.to_string()))
    }

    pub async fn cron_delete(&self, id: String) -> Result<bool, ApiError> {
        // Protect system jobs
        let jobs = self.cron_service.list_jobs(true).await;
        if let Some(job) = jobs.iter().find(|j| j.id == id) {
            if job.origin == scheduling::CronOrigin::System {
                return Err(ApiError::new("FORBIDDEN", "Cannot delete system cron jobs"));
            }
        }
        self.cron_service.remove_job(&id).await
            .map_err(|e| ApiError::new("CRON_ERROR", &e.to_string()))
    }

    pub async fn cron_create(&self, params: CronJobCreateParams) -> HandlerResult<CronJobResponse> {
        let schedule: scheduling::CronSchedule = serde_json::from_value(params.schedule)
            .map_err(|e| ApiError::new("INVALID_PARAMS", &format!("invalid schedule: {e}")))?;

        let job = self.cron_service.add_job(
            params.name,
            schedule,
            params.message,
            params.deliver,
            params.channel,
            params.to,
            params.delete_after_run,
            scheduling::CronOrigin::User,
        ).await.map_err(|e| ApiError::new("CRON_ERROR", &e.to_string()))?;

        Ok((to_response(&job), vec![]))
    }

    pub async fn cron_update(&self, params: CronJobUpdateParams) -> HandlerResult<CronJobResponse> {
        // Protect system jobs from full edit
        let jobs = self.cron_service.list_jobs(true).await;
        let existing = jobs.iter().find(|j| j.id == params.id)
            .ok_or_else(|| ApiError::new("NOT_FOUND", &format!("cron job '{}'", params.id)))?;

        if existing.origin == scheduling::CronOrigin::System {
            return Err(ApiError::new("FORBIDDEN", "Cannot edit system cron jobs"));
        }

        // Remove old job and recreate with updated fields
        // (CronService has no update method — remove + add is the pattern)
        let name = params.name.unwrap_or_else(|| existing.name.clone());
        let schedule = match params.schedule {
            Some(s) => serde_json::from_value(s)
                .map_err(|e| ApiError::new("INVALID_PARAMS", &format!("invalid schedule: {e}")))?,
            None => existing.schedule.clone(),
        };
        let message = params.message.unwrap_or_else(|| existing.payload.message.clone());
        let deliver = params.deliver.unwrap_or(existing.payload.deliver);
        let channel = match params.channel {
            Some(c) => c,
            None => existing.payload.channel.clone(),
        };
        let to = match params.to {
            Some(t) => t,
            None => existing.payload.to.clone(),
        };
        let origin = existing.origin.clone();

        self.cron_service.remove_job(&params.id).await
            .map_err(|e| ApiError::new("CRON_ERROR", &e.to_string()))?;

        let job = self.cron_service.add_job(
            name, schedule, message, deliver, channel, to,
            existing.delete_after_run, origin,
        ).await.map_err(|e| ApiError::new("CRON_ERROR", &e.to_string()))?;

        Ok((to_response(&job), vec![]))
    }
}
```

**Step 2: Add `pub mod cron;` to handlers/mod.rs**

**Step 3: Run tests and clippy**

```bash
cargo clippy --workspace --all-targets --all-features
```

**Step 4: Commit**

```
feat(app-core): add cron job handlers
```

---

### Task 5: Add Tauri commands and dev server dispatch for cron

**Files:**
- Create: `crates/desktop/src/commands/cron.rs`
- Modify: `crates/desktop/src/commands/mod.rs` (add `pub mod cron;`)
- Modify: `crates/desktop/src/main.rs:142+` (register commands)
- Modify: `crates/desktop/src/dev_server.rs:145+` (add dispatch_dev call)

**Step 1: Create commands/cron.rs**

Follow the pattern from `commands/notes.rs`:

```rust
use std::sync::Arc;
use app_core::AppCore;
use desktop_shared::errors::ApiError;
use desktop_shared::types::{
    CronJobCreateParams, CronJobResponse, CronJobUpdateParams, CronStatusResponse,
};
use tauri::State;

#[tauri::command]
pub async fn cron_list(
    state: State<'_, Arc<AppCore>>,
    include_disabled: Option<bool>,
) -> Result<Vec<CronJobResponse>, ApiError> {
    state.cron_list(include_disabled.unwrap_or(true)).await
}

#[tauri::command]
pub async fn cron_status(
    state: State<'_, Arc<AppCore>>,
) -> Result<CronStatusResponse, ApiError> {
    state.cron_status().await
}

#[tauri::command]
pub async fn cron_enable(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
    enabled: bool,
) -> Result<CronJobResponse, ApiError> {
    let (result, updates) = state.cron_enable(id, enabled).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn cron_run(
    state: State<'_, Arc<AppCore>>,
    id: String,
) -> Result<bool, ApiError> {
    state.cron_run(id).await
}

#[tauri::command]
pub async fn cron_delete(
    state: State<'_, Arc<AppCore>>,
    id: String,
) -> Result<bool, ApiError> {
    state.cron_delete(id).await
}

#[tauri::command]
pub async fn cron_create(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: CronJobCreateParams,
) -> Result<CronJobResponse, ApiError> {
    let (result, updates) = state.cron_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn cron_update(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: CronJobUpdateParams,
) -> Result<CronJobResponse, ApiError> {
    let (result, updates) = state.cron_update(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

// ── Dev server dispatch ──

#[cfg(debug_assertions)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "cron_list", "cron_status", "cron_enable", "cron_run",
    "cron_delete", "cron_create", "cron_update",
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "cron_list" => dev::val(core.cron_list(dev::get(body, "includeDisabled").unwrap_or(true)).await),
        "cron_status" => dev::val(core.cron_status().await),
        "cron_enable" => {
            let id = try_field!(dev::get_str(body, "id"));
            let enabled = try_field!(dev::require::<bool>(body, "enabled"));
            dev::val_rh(core.cron_enable(id, enabled).await)
        }
        "cron_run" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val(core.cron_run(id).await)
        }
        "cron_delete" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val(core.cron_delete(id).await)
        }
        "cron_create" => dev::val_rh(core.cron_create(try_field!(dev::parse_params(body))).await),
        "cron_update" => dev::val_rh(core.cron_update(try_field!(dev::parse_params(body))).await),
        _ => return None,
    })
}
```

**Step 2: Add `pub mod cron;` to commands/mod.rs**

**Step 3: Register commands in main.rs**

After the Cognitive section (around line 300+), add:

```rust
            // Cron / Automations
            commands::cron::cron_list,
            commands::cron::cron_status,
            commands::cron::cron_enable,
            commands::cron::cron_run,
            commands::cron::cron_delete,
            commands::cron::cron_create,
            commands::cron::cron_update,
```

**Step 4: Add dispatch_dev call in dev_server.rs**

After the last `dispatch_dev` call (around line 192):

```rust
    if let Some(r) = commands::cron::dispatch_dev(cmd, core, &body).await {
        return into_api_result(r);
    }
```

**Step 5: Build and verify**

```bash
cargo build --workspace
```

**Step 6: Commit**

```
feat(desktop): add Tauri commands and dev dispatch for cron CRUD
```

---

### Task 6: Add frontend TypeScript types

**Files:**
- Modify: `desktop-ui/src/lib/types.ts` (add cron types)

**Step 1: Add cron types**

After the existing type definitions (around line 905), add:

```typescript
// ── Cron / Automations ──────────────────────────────────────────────────

export type CronOrigin = "system" | "user" | "ai" | "plugin";

export type CronSchedule =
  | { kind: "at"; atMs: number }
  | { kind: "every"; everyMs: number }
  | { kind: "cron"; expr: string; tz?: string };

export interface CronPayload {
  kind: string;
  message: string;
  deliver: boolean;
  channel?: string;
  to?: string;
}

export interface CronJobState {
  nextRunAtMs?: number;
  lastRunAtMs?: number;
  lastStatus?: string;
  lastError?: string;
}

export interface CronJob {
  id: string;
  name: string;
  enabled: boolean;
  origin: CronOrigin;
  schedule: CronSchedule;
  payload: CronPayload;
  state: CronJobState;
  createdAtMs: number;
  updatedAtMs: number;
  deleteAfterRun: boolean;
}

export interface CronJobCreateParams {
  name: string;
  schedule: CronSchedule;
  message: string;
  deliver?: boolean;
  channel?: string;
  to?: string;
  deleteAfterRun?: boolean;
}

export interface CronJobUpdateParams {
  id: string;
  name?: string;
  schedule?: CronSchedule;
  message?: string;
  deliver?: boolean;
  channel?: string | null;
  to?: string | null;
}

export interface CronStatusResponse {
  enabled: boolean;
  jobs: number;
  nextWakeAtMs?: number;
}
```

**Step 2: Add "Automations" to SidebarItem union**

Update the `SidebarItem` type (line 895):

```typescript
export type SidebarItem =
  | "Chat"
  | "Tasks"
  | "OKR"
  | "Calendar"
  | "Notes"
  | "Finance"
  | "Productivity"
  | "Automations"
  | "Debug"
  | "Settings";
```

**Step 3: Commit**

```
feat(desktop-ui): add cron/automations TypeScript types
```

---

### Task 7: Add cron utility functions

**Files:**
- Create: `desktop-ui/src/lib/cron.ts`

**Step 1: Create humanization utilities**

```typescript
import type { CronJob, CronOrigin, CronSchedule } from "./types";

/** Humanize a CronSchedule to a readable string */
export function humanizeSchedule(schedule: CronSchedule): string {
  switch (schedule.kind) {
    case "at":
      return new Date(schedule.atMs).toLocaleString(undefined, {
        month: "short", day: "numeric", hour: "numeric", minute: "2-digit",
      });
    case "every": {
      const ms = schedule.everyMs;
      if (ms < 60_000) return `Every ${Math.round(ms / 1000)}s`;
      if (ms < 3_600_000) return `Every ${Math.round(ms / 60_000)} min`;
      if (ms < 86_400_000) {
        const h = ms / 3_600_000;
        return h === 1 ? "Every hour" : `Every ${h} hours`;
      }
      const d = ms / 86_400_000;
      return d === 1 ? "Every day" : `Every ${d} days`;
    }
    case "cron":
      return humanizeCronExpr(schedule.expr, schedule.tz);
  }
}

/** Best-effort humanization of common cron expressions */
function humanizeCronExpr(expr: string, tz?: string): string {
  const parts = expr.trim().split(/\s+/);
  // Handle both 5-field and 6-field (with seconds) cron
  const [min, hour, dom, _mon, dow] =
    parts.length === 6 ? parts.slice(1) : parts;

  const tzSuffix = tz && tz !== "UTC" ? ` (${tz})` : "";

  if (min !== undefined && hour !== undefined && dom === "*" && dow === "*") {
    const h = Number.parseInt(hour);
    const m = Number.parseInt(min);
    if (!Number.isNaN(h) && !Number.isNaN(m)) {
      const time = formatTime(h, m);
      return `Daily at ${time}${tzSuffix}`;
    }
  }

  if (min !== undefined && hour !== undefined && dom === "*" && dow !== undefined && dow !== "*") {
    const h = Number.parseInt(hour);
    const m = Number.parseInt(min);
    if (!Number.isNaN(h) && !Number.isNaN(m)) {
      const time = formatTime(h, m);
      const dayName = dayOfWeek(dow);
      return dayName ? `${dayName}s at ${time}${tzSuffix}` : `${expr}${tzSuffix}`;
    }
  }

  return `${expr}${tzSuffix}`;
}

function formatTime(h: number, m: number): string {
  const ampm = h >= 12 ? "PM" : "AM";
  const h12 = h % 12 || 12;
  return m === 0 ? `${h12} ${ampm}` : `${h12}:${String(m).padStart(2, "0")} ${ampm}`;
}

function dayOfWeek(dow: string): string | null {
  const days: Record<string, string> = {
    "0": "Sunday", "1": "Monday", "2": "Tuesday", "3": "Wednesday",
    "4": "Thursday", "5": "Friday", "6": "Saturday", "7": "Sunday",
  };
  return days[dow] ?? null;
}

/** Humanize a job name by stripping prefixes and converting to title case */
export function humanizeJobName(name: string): string {
  return name
    .replace(/^__klyntbot_/, "")
    .replace(/^todo_/, "")
    .replace(/^plugin:.*?:/, "")
    .replace(/_/g, " ")
    .replace(/\b\w/g, (c) => c.toUpperCase());
}

/** Format relative time: "in 28 min", "2h ago", etc. */
export function relativeTime(ms: number): string {
  const now = Date.now();
  const diff = ms - now;
  const abs = Math.abs(diff);
  const suffix = diff > 0 ? "" : " ago";
  const prefix = diff > 0 ? "in " : "";

  if (abs < 60_000) return "just now";
  if (abs < 3_600_000) return `${prefix}${Math.round(abs / 60_000)} min${suffix}`;
  if (abs < 86_400_000) return `${prefix}${Math.round(abs / 3_600_000)}h${suffix}`;
  return `${prefix}${Math.round(abs / 86_400_000)}d${suffix}`;
}

/** Origin badge config */
export const ORIGIN_STYLES: Record<CronOrigin, { label: string; className: string }> = {
  system: { label: "System", className: "bg-blue-500/20 text-blue-400" },
  ai: { label: "AI", className: "bg-purple-500/20 text-purple-400" },
  user: { label: "User", className: "bg-emerald-500/20 text-emerald-400" },
  plugin: { label: "Plugin", className: "bg-amber-500/20 text-amber-400" },
};
```

**Step 2: Commit**

```
feat(desktop-ui): add cron utility functions
```

---

### Task 8: Build the AutomationsPage component

**Files:**
- Create: `desktop-ui/src/components/views/AutomationsPage.tsx`

**Step 1: Build the main page**

This is the largest task. Build the page with:
- Header with title + "+ New Job" button
- Origin filter tabs (All / System / AI / User / Plugin)
- Search input with debounce
- Job table with expandable rows
- Inline create form
- Toggle switch, run now, delete actions
- Loading skeleton
- Empty state

Reference `FinanceTransactions.tsx` for the filter + search + table pattern.
Reference `Sidebar.tsx` for the glass-button tab pattern.

Use `useQuery("cron_list", { includeDisabled: true })` for data fetching.
Use `useMutation` for enable/disable, run, delete, create, update.
Client-side filtering by origin and search query.

Key components in this single file (keep it in one file unless it exceeds ~500 lines):
- `AutomationsPage` — main container
- `AutomationRow` — table row
- `AutomationExpandedRow` — expanded detail view
- `AutomationCreateForm` — inline form for creating jobs
- `JobScheduleBuilder` — schedule type radio + inputs
- `OriginBadge` — colored pill

Design tokens to use:
- `glass-input` for form inputs
- `glass-button` / `glass-button-active` for filter tabs
- `bg-white/[0.04]` for expanded row background
- `border-white/[0.08]` for borders
- `text-muted`, `text-dim`, `text-primary`, `text-brand`
- `animate-pulse bg-white/[0.08]` for skeleton

**Step 2: Commit**

```
feat(desktop-ui): add AutomationsPage component
```

---

### Task 9: Wire up routing and sidebar navigation

**Files:**
- Modify: `desktop-ui/src/components/layout/Sidebar.tsx:1-10,22-29` (add Timer icon + Automations item)
- Modify: `desktop-ui/src/components/layout/AppShell.tsx:38-46` (add route→sidebar mapping)
- Modify: `desktop-ui/src/App.tsx` (add lazy import + route)

**Step 1: Add Automations to Sidebar items**

In `Sidebar.tsx`, add `Timer` to the lucide-react imports and add the item:

```typescript
{ key: "Automations", icon: Timer, path: "/automations" },
```

Add it after the Productivity entry.

**Step 2: Add route mapping in AppShell**

In the `activeSidebarItem` useMemo (line 38-46), add before the default return:

```typescript
if (path.startsWith("/automations")) return "Automations";
```

**Step 3: Add lazy route in App.tsx**

Add lazy import:

```typescript
const AutomationsPage = lazy(() =>
  import("./components/views/AutomationsPage").then((m) => ({ default: m.AutomationsPage })),
);
```

Add route inside AppShell children:

```typescript
{ path: "/automations", element: <AutomationsPage /> },
```

**Step 4: Lint**

```bash
cd desktop-ui && bun run lint:fix
```

**Step 5: Commit**

```
feat(desktop-ui): wire automations page into sidebar and router
```

---

### Task 10: Final integration test

**Step 1: Build everything**

```bash
cargo build --workspace
cargo clippy --workspace --all-targets --all-features
cargo fmt --all --check
```

**Step 2: Run all Rust tests**

```bash
cargo nextest run --workspace
cargo test --workspace --doc
```

**Step 3: Build frontend**

```bash
cd desktop-ui && bun run build
```

**Step 4: Lint frontend**

```bash
cd desktop-ui && bun run lint:fix
```

**Step 5: Manual smoke test**

Start dev mode with `cargo run -p dev-api` + `cd desktop-ui && bun run dev`, open `localhost:1420`, navigate to Automations page, verify:
- Jobs load and display
- Origin badges render correctly
- Filter tabs work
- Search filters by name
- Toggle enable/disable works
- Expand row shows details
- Create form works (for user jobs)
- System jobs cannot be edited/deleted

**Step 6: Commit (if any fixes needed)**

```
fix: address integration issues
```
