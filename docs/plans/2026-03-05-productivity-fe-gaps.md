# Productivity Frontend Gaps Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Close all frontend gaps where backend productivity features exist but have no UI — goal CRUD, category management, time entry logging, learned distraction rules, and score breakdown.

**Architecture:** Each gap follows the same pattern: (1) add Tauri command + dev-server dispatch entry if missing, (2) add FE types if missing, (3) build the UI component using existing design patterns (glass-panel dialogs, `useQuery`/`useMutation` hooks, `useEvent` for reactivity). All new components follow the existing dark-theme design system with `bg-surface-base`, `text-primary`, `border-border` tokens.

**Tech Stack:** Rust (Tauri commands, desktop-shared DTOs), TypeScript/React (desktop-ui components), Tailwind v4 CSS tokens, lucide-react icons.

---

## Design Reference

All components use the existing design system observed in the screenshots:
- **Cards:** `bg-surface-base rounded-xl p-4` with `text-[13px] font-medium text-secondary` headings
- **Dialogs:** `glass-panel` overlay with `bg-surface-low rounded-[var(--glass-radius-inner)]` inner panel, header/body/footer sections separated by `border-b border-border`
- **Inputs:** `w-full px-3 py-1.5 text-[13px] bg-surface-base border border-border rounded-md text-primary placeholder:text-dim focus:outline-none focus:border-brand/50`
- **Buttons (primary):** `px-4 py-1.5 text-[12px] rounded-md bg-brand text-white hover:bg-brand-hover`
- **Buttons (ghost):** `px-3 py-1.5 text-[12px] text-muted hover:text-secondary rounded-md hover:bg-surface-base`
- **Destructive actions:** `text-muted hover:text-destructive` on icon buttons
- **Progress bars:** `h-1.5 rounded-full bg-surface-raised` track with `bg-brand` or `bg-success` fill
- **Font sizes:** headings `text-[13px]`, labels `text-[12px]`, small text `text-[11px]`, data values `text-[10px]`
- **Status colors:** met/success = `text-success`, in-progress = `text-brand`, destructive = `text-destructive`

---

## Task 1: Backend — Goal CRUD Tauri Commands

**Why:** `GoalRepo` has `insert`, `delete`, `set_enabled` but no Tauri IPC commands expose them. The FE needs these to create/delete/toggle goals.

**Files:**
- Modify: `crates/desktop-shared/src/commands.rs` (add request params struct)
- Modify: `crates/desktop/src/commands/productivity.rs` (add 3 Tauri commands)
- Modify: `crates/desktop/src/main.rs` (register commands)
- Modify: `crates/desktop/src/dev_server.rs` (add dispatch entries)

**Step 1: Add shared param type to desktop-shared**

In `crates/desktop-shared/src/commands.rs`, after the `GoalProgressResponse` struct (~line 450), add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoalCreateParams {
    pub goal_type: String,
    pub metric: String,
    pub target_value: f64,
}
```

**Step 2: Add Tauri commands to productivity.rs**

In `crates/desktop/src/commands/productivity.rs`, after the `productivity_time_entries` command, add:

```rust
#[tauri::command]
pub async fn productivity_goal_create(
    state: State<'_, Arc<AppCore>>,
    goal_type: String,
    metric: String,
    target_value: f64,
) -> Result<GoalProgressResponse, ApiError> {
    let repos = state.productivity_repos()?;
    let gt: feature_productivity::types::GoalType = goal_type
        .parse()
        .map_err(|_| ApiError::validation("Invalid goal_type. Use: daily, weekly"))?;
    let gm: feature_productivity::types::GoalMetric = metric
        .parse()
        .map_err(|_| ApiError::validation("Invalid metric. Use: productive_hours, focus_sessions, productivity_score, max_distracting_mins"))?;
    let goal = feature_productivity::types::ProductivityGoal {
        id: None,
        goal_type: gt,
        metric: gm,
        target_value,
        enabled: true,
        created_at: Utc::now(),
    };
    let id = repos.goals.insert(&goal).await.map_err(map_prod_err)?;
    Ok(GoalProgressResponse {
        id,
        goal_type: goal.goal_type.to_string(),
        metric: goal.metric.to_string(),
        target_value: goal.target_value,
        current_value: 0.0,
        met: false,
    })
}

#[tauri::command]
pub async fn productivity_goal_delete(
    state: State<'_, Arc<AppCore>>,
    id: i64,
) -> Result<(), ApiError> {
    let repos = state.productivity_repos()?;
    repos.goals.delete(id).await.map_err(map_prod_err)?;
    Ok(())
}

#[tauri::command]
pub async fn productivity_goal_toggle(
    state: State<'_, Arc<AppCore>>,
    id: i64,
    enabled: bool,
) -> Result<(), ApiError> {
    let repos = state.productivity_repos()?;
    repos.goals.set_enabled(id, enabled).await.map_err(map_prod_err)?;
    Ok(())
}
```

Note: You will need to add `GoalProgressResponse` to the imports at the top of the file (it's already in `desktop_shared::commands`).

**Step 3: Register in main.rs**

In `crates/desktop/src/main.rs`, after the `productivity_time_entries` line (~186), add:

```rust
            commands::productivity::productivity_goal_create,
            commands::productivity::productivity_goal_delete,
            commands::productivity::productivity_goal_toggle,
```

**Step 4: Add dev-server dispatch entries**

In `crates/desktop/src/dev_server.rs`, after the `productivity_time_entries` block, add matching dispatch arms. Follow the existing pattern — extract params with `get_str`/`get`, call repos, return JSON. The pattern is identical to what exists for other commands.

For `productivity_goal_create`:
```rust
"productivity_goal_create" => {
    let goal_type = match get_str(&body, "goal_type") { Ok(v) => v, Err(e) => return err(e) };
    let metric = match get_str(&body, "metric") { Ok(v) => v, Err(e) => return err(e) };
    let target_value: f64 = get(&body, "target_value").unwrap_or(0.0);
    let repos = match core.productivity_repos() { Ok(r) => r, Err(e) => return err(e) };
    let gt: feature_productivity::types::GoalType = match goal_type.parse() {
        Ok(v) => v, Err(_) => return err(ApiError::validation("Invalid goal_type")),
    };
    let gm: feature_productivity::types::GoalMetric = match metric.parse() {
        Ok(v) => v, Err(_) => return err(ApiError::validation("Invalid metric")),
    };
    let goal = feature_productivity::types::ProductivityGoal {
        id: None, goal_type: gt, metric: gm, target_value, enabled: true, created_at: Utc::now(),
    };
    match repos.goals.insert(&goal).await {
        Ok(id) => ok(GoalProgressResponse {
            id, goal_type: goal.goal_type.to_string(), metric: goal.metric.to_string(),
            target_value, current_value: 0.0, met: false,
        }),
        Err(e) => err(prod_err(e)),
    }
}
```

For `productivity_goal_delete`:
```rust
"productivity_goal_delete" => {
    let id: i64 = get(&body, "id").unwrap_or(0);
    let repos = match core.productivity_repos() { Ok(r) => r, Err(e) => return err(e) };
    match repos.goals.delete(id).await {
        Ok(_) => ok(()),
        Err(e) => err(prod_err(e)),
    }
}
```

For `productivity_goal_toggle`:
```rust
"productivity_goal_toggle" => {
    let id: i64 = get(&body, "id").unwrap_or(0);
    let enabled: bool = get(&body, "enabled").unwrap_or(true);
    let repos = match core.productivity_repos() { Ok(r) => r, Err(e) => return err(e) };
    match repos.goals.set_enabled(id, enabled).await {
        Ok(_) => ok(()),
        Err(e) => err(prod_err(e)),
    }
}
```

**Step 5: Verify it compiles**

Run: `cargo build -p desktop -p desktop-shared -p dev-api 2>&1 | tail -5`
Expected: successful compilation (0 errors)

**Step 6: Commit**

```bash
git add crates/desktop-shared/src/commands.rs crates/desktop/src/commands/productivity.rs crates/desktop/src/main.rs crates/desktop/src/dev_server.rs
git commit -m "feat(desktop): add goal CRUD Tauri commands"
```

---

## Task 2: Backend — Time Entry & Category Mutation Commands

**Why:** `TimeEntryRepo::insert` and `ActivityCategoryRepo::upsert` exist but have no Tauri mutation commands. The FE needs these to log time and manage categories.

**Files:**
- Modify: `crates/desktop/src/commands/productivity.rs` (add commands)
- Modify: `crates/desktop/src/main.rs` (register)
- Modify: `crates/desktop/src/dev_server.rs` (dispatch)

**Step 1: Add time entry create command**

In `crates/desktop/src/commands/productivity.rs`:

```rust
#[tauri::command]
pub async fn productivity_time_entry_create(
    state: State<'_, Arc<AppCore>>,
    description: String,
    duration_mins: i64,
    category_id: Option<String>,
    project_id: Option<String>,
) -> Result<TimeEntryResponse, ApiError> {
    let repos = state.productivity_repos()?;
    let now = Utc::now();
    let started_at = now - chrono::Duration::minutes(duration_mins);
    let entry = feature_productivity::types::TimeEntry {
        id: None,
        description: description.clone(),
        category_id: category_id.clone(),
        project_id: project_id.clone(),
        started_at,
        duration_secs: duration_mins * 60,
        source: "manual".to_string(),
        created_at: now,
    };
    let id = repos.time_entries.insert(&entry).await.map_err(map_prod_err)?;
    Ok(TimeEntryResponse {
        id,
        description,
        category_id,
        project_id,
        started_at,
        duration_secs: duration_mins * 60,
        source: "manual".to_string(),
    })
}

#[tauri::command]
pub async fn productivity_time_entry_delete(
    state: State<'_, Arc<AppCore>>,
    id: i64,
) -> Result<(), ApiError> {
    let repos = state.productivity_repos()?;
    repos.time_entries.delete(id).await.map_err(map_prod_err)?;
    Ok(())
}
```

**Step 2: Add category upsert command**

```rust
#[tauri::command]
pub async fn productivity_category_upsert(
    state: State<'_, Arc<AppCore>>,
    id: String,
    name: String,
    category_type: String,
    color: Option<String>,
    icon: Option<String>,
) -> Result<ActivityCategoryResponse, ApiError> {
    let repos = state.productivity_repos()?;
    let ct: feature_productivity::types::CategoryType = category_type
        .parse()
        .map_err(|_| ApiError::validation("Invalid category_type. Use: productive, neutral, distracting"))?;
    let cat = feature_productivity::types::ActivityCategory {
        id: id.clone(),
        name: name.clone(),
        category_type: ct,
        color: color.clone(),
        icon: icon.clone(),
        rules: None,
        is_system: false,
    };
    repos.categories.upsert(&cat).await.map_err(map_prod_err)?;
    Ok(ActivityCategoryResponse {
        id,
        name,
        category_type: cat.category_type.to_string(),
        color,
        icon,
        is_system: false,
    })
}
```

**Step 3: Register in main.rs**

After the goal commands registration:
```rust
            commands::productivity::productivity_time_entry_create,
            commands::productivity::productivity_time_entry_delete,
            commands::productivity::productivity_category_upsert,
```

**Step 4: Add dev-server dispatch entries**

Follow the same pattern as Task 1, Step 4 for each new command.

**Step 5: Verify compilation**

Run: `cargo build -p desktop -p dev-api 2>&1 | tail -5`
Expected: successful compilation

**Step 6: Commit**

```bash
git add crates/desktop/src/commands/productivity.rs crates/desktop/src/main.rs crates/desktop/src/dev_server.rs
git commit -m "feat(desktop): add time entry and category mutation commands"
```

---

## Task 3: Frontend — Goal CRUD UI (GoalsProgress upgrade)

**Why:** The `GoalsProgress` component currently shows "No goals set" with no way to create goals. This is the highest-value gap.

**Files:**
- Modify: `desktop-ui/src/lib/types.ts` (add `GoalCreateParams`)
- Create: `desktop-ui/src/components/productivity/AddGoalDialog.tsx`
- Modify: `desktop-ui/src/components/productivity/GoalsProgress.tsx` (add create/delete UI)

**Step 1: Add FE types**

In `desktop-ui/src/lib/types.ts`, after the `TimeEntry` interface (~line 525), add:

```typescript
export interface GoalCreateParams {
  goal_type: string;
  metric: string;
  target_value: number;
}

export interface LearnedRule {
  id: number;
  pattern: string;
  patternType: string;
  classification: string;
  confidence: number;
  hitCount: number;
  lastUsedAt: string;
  createdAt: string;
}
```

**Step 2: Create AddGoalDialog**

Create `desktop-ui/src/components/productivity/AddGoalDialog.tsx`:

```tsx
import { X } from "lucide-react";
import { useState } from "react";

interface AddGoalDialogProps {
  open: boolean;
  onClose: () => void;
  onAdd: (params: { goal_type: string; metric: string; target_value: number }) => void;
}

const METRICS = [
  { value: "productive_hours", label: "Productive hours", unit: "hours", placeholder: "6" },
  { value: "focus_sessions", label: "Focus sessions", unit: "sessions", placeholder: "4" },
  { value: "productivity_score", label: "Productivity score", unit: "/100", placeholder: "70" },
  { value: "max_distracting_mins", label: "Max distracting minutes", unit: "mins", placeholder: "30" },
] as const;

export function AddGoalDialog({ open, onClose, onAdd }: AddGoalDialogProps) {
  const [goalType, setGoalType] = useState<"daily" | "weekly">("daily");
  const [metric, setMetric] = useState(METRICS[0].value);
  const [targetValue, setTargetValue] = useState("");

  if (!open) return null;

  const selectedMetric = METRICS.find((m) => m.value === metric) ?? METRICS[0];
  const canSubmit = targetValue.trim() !== "" && Number(targetValue) > 0;

  const handleSubmit = () => {
    onAdd({ goal_type: goalType, metric, target_value: Number(targetValue) });
    setTargetValue("");
    onClose();
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="glass-panel w-[400px]">
        <div className="bg-surface-low rounded-[var(--glass-radius-inner)]">
          <div className="flex items-center justify-between px-5 py-4 border-b border-border">
            <h3 className="text-[14px] font-medium text-primary">Add Goal</h3>
            <button
              onClick={onClose}
              className="w-7 h-7 rounded-md flex items-center justify-center text-muted hover:text-secondary hover:bg-surface-base transition-colors"
            >
              <X className="w-4 h-4" />
            </button>
          </div>

          <div className="px-5 py-4 space-y-4">
            {/* Goal type */}
            <div>
              <label className="block text-[12px] text-muted mb-1.5">Period</label>
              <div className="flex gap-2">
                {(["daily", "weekly"] as const).map((t) => (
                  <button
                    key={t}
                    onClick={() => setGoalType(t)}
                    className={`flex-1 py-1.5 text-[12px] rounded-md border transition-colors capitalize ${
                      goalType === t
                        ? "border-brand/50 text-brand bg-brand/5"
                        : "border-border text-muted bg-surface-base hover:bg-surface-raised"
                    }`}
                  >
                    {t}
                  </button>
                ))}
              </div>
            </div>

            {/* Metric */}
            <div>
              <label className="block text-[12px] text-muted mb-1.5">Metric</label>
              <div className="flex flex-col gap-1.5">
                {METRICS.map((m) => (
                  <button
                    key={m.value}
                    onClick={() => setMetric(m.value)}
                    className={`px-3 py-2 text-[12px] text-left rounded-md border transition-colors ${
                      metric === m.value
                        ? "border-brand/50 text-brand bg-brand/5"
                        : "border-border text-muted bg-surface-base hover:bg-surface-raised"
                    }`}
                  >
                    {m.label}
                  </button>
                ))}
              </div>
            </div>

            {/* Target value */}
            <div>
              <label className="block text-[12px] text-muted mb-1.5">
                Target <span className="text-dim">({selectedMetric.unit})</span>
              </label>
              <input
                type="number"
                value={targetValue}
                onChange={(e) => setTargetValue(e.target.value)}
                placeholder={selectedMetric.placeholder}
                min={0}
                step={metric === "productive_hours" ? 0.5 : 1}
                className="w-full px-3 py-1.5 text-[13px] bg-surface-base border border-border rounded-md text-primary placeholder:text-dim focus:outline-none focus:border-brand/50"
              />
            </div>
          </div>

          <div className="flex items-center justify-end gap-2 px-5 py-3 border-t border-border">
            <button
              onClick={onClose}
              className="px-3 py-1.5 text-[12px] text-muted hover:text-secondary rounded-md hover:bg-surface-base transition-colors"
            >
              Cancel
            </button>
            <button
              onClick={handleSubmit}
              disabled={!canSubmit}
              className="px-4 py-1.5 text-[12px] rounded-md bg-brand text-white hover:bg-brand-hover transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
            >
              Add goal
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
```

**Step 3: Upgrade GoalsProgress with CRUD**

Replace `desktop-ui/src/components/productivity/GoalsProgress.tsx` with:

```tsx
import { Plus, Trash2 } from "lucide-react";
import { useState } from "react";
import { useEvent } from "../../hooks/useEvent";
import { useMutation } from "../../hooks/useMutation";
import { useQuery } from "../../hooks/useQuery";
import type { GoalProgress } from "../../lib/types";
import { AddGoalDialog } from "./AddGoalDialog";

function metricLabel(metric: string): string {
  switch (metric) {
    case "productive_hours":
      return "productive hours";
    case "focus_sessions":
      return "focus sessions";
    case "productivity_score":
      return "score";
    case "max_distracting_mins":
      return "distracting mins";
    default:
      return metric;
  }
}

function formatValue(metric: string, value: number): string {
  if (metric === "productive_hours") return `${value.toFixed(1)}h`;
  if (metric === "max_distracting_mins") return `${Math.round(value)}m`;
  return `${Math.round(value)}`;
}

export function GoalsProgress() {
  const { data: goals, refetch } = useQuery<GoalProgress[]>("productivity_goals", undefined, []);
  const [showAdd, setShowAdd] = useState(false);
  const { mutate: createGoal } = useMutation("productivity_goal_create");
  const { mutate: deleteGoal } = useMutation("productivity_goal_delete");

  useEvent<{ entityKind: string }>("entity:updated", (payload) => {
    if (payload?.entityKind === "productivity") refetch();
  });

  const handleAdd = async (params: { goal_type: string; metric: string; target_value: number }) => {
    await createGoal(params as any);
    refetch();
  };

  const handleDelete = async (id: number) => {
    await deleteGoal({ id } as any);
    refetch();
  };

  return (
    <>
      <div className="bg-surface-base rounded-xl p-4 flex flex-col gap-3">
        <div className="flex items-center justify-between">
          <h2 className="text-[13px] font-medium text-secondary">Goals</h2>
          <button
            onClick={() => setShowAdd(true)}
            className="w-6 h-6 rounded-md flex items-center justify-center text-muted hover:text-brand hover:bg-surface-raised transition-colors"
          >
            <Plus className="w-3.5 h-3.5" />
          </button>
        </div>

        {goals.length === 0 ? (
          <p className="text-[12px] font-light text-dim">No goals set</p>
        ) : (
          <div className="flex flex-col gap-2">
            {goals.map((g) => {
              const pct = g.targetValue > 0 ? Math.min((g.currentValue / g.targetValue) * 100, 100) : 0;
              return (
                <div key={g.id} className="group flex flex-col gap-1">
                  <div className="flex items-center justify-between text-[11px] font-light">
                    <div className="flex items-center gap-2">
                      <span className={g.met ? "text-success" : "text-brand"}>
                        {g.met ? "MET" : "IN PROGRESS"}
                      </span>
                      <span className="text-primary">
                        {formatValue(g.metric, g.targetValue)} {metricLabel(g.metric)}
                      </span>
                      <span className="text-dim">({g.goalType})</span>
                    </div>
                    <div className="flex items-center gap-2">
                      <span className="text-muted tabular-nums">
                        {formatValue(g.metric, g.currentValue)} / {formatValue(g.metric, g.targetValue)}
                      </span>
                      <button
                        onClick={() => handleDelete(g.id)}
                        className="w-5 h-5 rounded flex items-center justify-center text-transparent group-hover:text-muted hover:!text-destructive transition-colors"
                      >
                        <Trash2 className="w-3 h-3" />
                      </button>
                    </div>
                  </div>
                  <div className="h-1.5 rounded-full bg-surface-raised overflow-hidden">
                    <div
                      className={`h-full rounded-full transition-all ${g.met ? "bg-success" : "bg-brand"}`}
                      style={{ width: `${pct}%` }}
                    />
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>

      <AddGoalDialog open={showAdd} onClose={() => setShowAdd(false)} onAdd={handleAdd} />
    </>
  );
}
```

**Step 4: Verify in browser**

Run: `cd desktop-ui && bun run dev`
Navigate to `http://localhost:1420/#/productivity/day/2026-03-05`
Expected: Goals card now shows a `+` button. Clicking opens the AddGoalDialog. Creating a goal shows it with a progress bar and a delete button on hover.

**Step 5: Lint**

Run: `cd desktop-ui && bun run lint:fix`

**Step 6: Commit**

```bash
git add desktop-ui/src/lib/types.ts desktop-ui/src/components/productivity/AddGoalDialog.tsx desktop-ui/src/components/productivity/GoalsProgress.tsx
git commit -m "feat(desktop-ui): add goal CRUD UI with add dialog and delete"
```

---

## Task 4: Frontend — Time Entry Logging UI

**Why:** Manual time entry exists in backend but has zero FE. Users should be able to log time from the Day view.

**Files:**
- Create: `desktop-ui/src/components/productivity/TimeEntrySection.tsx`
- Modify: `desktop-ui/src/components/productivity/DayView.tsx` (add TimeEntrySection)

**Step 1: Create TimeEntrySection**

Create `desktop-ui/src/components/productivity/TimeEntrySection.tsx`:

```tsx
import { Clock, Plus, Trash2 } from "lucide-react";
import { useState } from "react";
import { useEvent } from "../../hooks/useEvent";
import { useMutation } from "../../hooks/useMutation";
import { useQuery } from "../../hooks/useQuery";
import type { ActivityCategory, TimeEntry } from "../../lib/types";

function formatDuration(secs: number): string {
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  return h > 0 ? `${h}h ${m}m` : `${m}m`;
}

interface TimeEntrySectionProps {
  date: string;
}

export function TimeEntrySection({ date }: TimeEntrySectionProps) {
  const { data: entries, refetch } = useQuery<TimeEntry[]>(
    "productivity_time_entries",
    { date },
    [],
  );
  const { data: categories } = useQuery<ActivityCategory[]>("productivity_categories", undefined, []);
  const { mutate: createEntry } = useMutation("productivity_time_entry_create");
  const { mutate: deleteEntry } = useMutation("productivity_time_entry_delete");

  const [showForm, setShowForm] = useState(false);
  const [description, setDescription] = useState("");
  const [durationMins, setDurationMins] = useState("");
  const [categoryId, setCategoryId] = useState("");

  useEvent<{ entityKind: string }>("entity:updated", (payload) => {
    if (payload?.entityKind === "productivity") refetch();
  });

  const handleAdd = async () => {
    if (!description.trim() || !durationMins) return;
    await createEntry({
      description: description.trim(),
      duration_mins: Number(durationMins),
      category_id: categoryId || undefined,
    } as any);
    setDescription("");
    setDurationMins("");
    setCategoryId("");
    setShowForm(false);
    refetch();
  };

  const handleDelete = async (id: number) => {
    await deleteEntry({ id } as any);
    refetch();
  };

  return (
    <div className="bg-surface-base rounded-xl p-4 flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <h2 className="text-[13px] font-medium text-secondary flex items-center gap-2">
          <Clock className="w-3.5 h-3.5 text-muted" />
          Time Entries
        </h2>
        <button
          onClick={() => setShowForm(!showForm)}
          className="w-6 h-6 rounded-md flex items-center justify-center text-muted hover:text-brand hover:bg-surface-raised transition-colors"
        >
          <Plus className="w-3.5 h-3.5" />
        </button>
      </div>

      {/* Inline add form */}
      {showForm && (
        <div className="flex flex-col gap-2 p-3 bg-surface-lowest rounded-lg border border-border">
          <input
            type="text"
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            placeholder="What did you work on?"
            className="w-full px-3 py-1.5 text-[13px] bg-surface-base border border-border rounded-md text-primary placeholder:text-dim focus:outline-none focus:border-brand/50"
          />
          <div className="flex gap-2">
            <input
              type="number"
              value={durationMins}
              onChange={(e) => setDurationMins(e.target.value)}
              placeholder="Minutes"
              min={1}
              className="w-24 px-3 py-1.5 text-[13px] bg-surface-base border border-border rounded-md text-primary placeholder:text-dim focus:outline-none focus:border-brand/50"
            />
            <select
              value={categoryId}
              onChange={(e) => setCategoryId(e.target.value)}
              className="flex-1 px-3 py-1.5 text-[13px] bg-surface-base border border-border rounded-md text-primary focus:outline-none focus:border-brand/50"
            >
              <option value="">No category</option>
              {categories.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.name}
                </option>
              ))}
            </select>
            <button
              onClick={handleAdd}
              disabled={!description.trim() || !durationMins}
              className="px-3 py-1.5 text-[12px] rounded-md bg-brand text-white hover:bg-brand-hover transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
            >
              Log
            </button>
          </div>
        </div>
      )}

      {/* Entry list */}
      {entries.length === 0 && !showForm ? (
        <p className="text-[12px] font-light text-dim">No manual entries today</p>
      ) : (
        <div className="flex flex-col gap-1.5">
          {entries.map((e) => (
            <div
              key={e.id}
              className="group flex items-center justify-between py-1.5 text-[12px] font-light"
            >
              <div className="flex items-center gap-2">
                <span className="text-primary">{e.description}</span>
                {e.categoryId && (
                  <span className="text-dim">
                    {categories.find((c) => c.id === e.categoryId)?.name}
                  </span>
                )}
              </div>
              <div className="flex items-center gap-2">
                <span className="text-muted tabular-nums">{formatDuration(e.durationSecs)}</span>
                <button
                  onClick={() => handleDelete(e.id)}
                  className="w-5 h-5 rounded flex items-center justify-center text-transparent group-hover:text-muted hover:!text-destructive transition-colors"
                >
                  <Trash2 className="w-3 h-3" />
                </button>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
```

**Step 2: Add to DayView**

In `desktop-ui/src/components/productivity/DayView.tsx`, add import:
```tsx
import { TimeEntrySection } from "./TimeEntrySection";
```

Then in the center column (after `<TopApps apps={...} />`), add:
```tsx
<TimeEntrySection date={date} />
```

**Step 3: Verify in browser**

Navigate to Day view. Center column should now show "Time Entries" card with `+` button. Clicking opens an inline form. Logging an entry shows it in the list.

**Step 4: Lint and commit**

```bash
cd desktop-ui && bun run lint:fix
git add desktop-ui/src/components/productivity/TimeEntrySection.tsx desktop-ui/src/components/productivity/DayView.tsx
git commit -m "feat(desktop-ui): add time entry logging UI to day view"
```

---

## Task 5: Frontend — Category Management in CategoriesList

**Why:** Users can't re-categorize apps from the UI. The existing `CategoriesList` component is read-only.

**Files:**
- Modify: `desktop-ui/src/components/productivity/CategoriesList.tsx` (add edit capability)

**Step 1: Read existing CategoriesList**

Read `desktop-ui/src/components/productivity/CategoriesList.tsx` to understand the current implementation.

**Step 2: Add inline category type toggle**

Upgrade the component so each category row has a clickable category-type indicator (colored dot). Clicking it cycles through `productive → neutral → distracting`. This uses the `productivity_category_upsert` mutation.

The key change is adding an `onCategoryChange` interaction to each row:

```tsx
// Inside each category row, replace the static colored dot with:
<button
  onClick={() => handleCycleType(cat)}
  className="w-2 h-2 rounded-full flex-shrink-0 transition-colors"
  style={{ backgroundColor: typeColor(cat.categoryType) }}
  title={`Click to change: ${cat.categoryType}`}
/>
```

Where `handleCycleType` cycles the type and calls `productivity_category_upsert`. The categories list should be fetched via `useQuery("productivity_categories")` to get the full category objects (with `id` and `categoryType`), rather than relying solely on the `topCategories` prop.

**Step 3: Verify and commit**

```bash
cd desktop-ui && bun run lint:fix
git add desktop-ui/src/components/productivity/CategoriesList.tsx
git commit -m "feat(desktop-ui): add category type toggle to categories list"
```

---

## Task 6: Frontend — Learned Distraction Rules Management

**Why:** The backend has `distraction_learned_rules` and `distraction_delete_rule` commands but no UI to view or manage them.

**Files:**
- Create: `desktop-ui/src/components/productivity/LearnedRulesCard.tsx`
- Modify: `desktop-ui/src/components/productivity/DayView.tsx` (add the card)

**Step 1: Create LearnedRulesCard**

Create `desktop-ui/src/components/productivity/LearnedRulesCard.tsx`:

```tsx
import { Shield, Trash2 } from "lucide-react";
import { useMutation } from "../../hooks/useMutation";
import { useQuery } from "../../hooks/useQuery";
import type { LearnedRule } from "../../lib/types";

export function LearnedRulesCard() {
  const { data: rules, refetch } = useQuery<LearnedRule[]>("distraction_learned_rules", undefined, []);
  const { mutate: deleteRule } = useMutation("distraction_delete_rule");

  if (rules.length === 0) return null;

  const handleDelete = async (id: number) => {
    await deleteRule({ id } as any);
    refetch();
  };

  return (
    <div className="bg-surface-base rounded-xl p-4 flex flex-col gap-3">
      <h2 className="text-[13px] font-medium text-secondary flex items-center gap-2">
        <Shield className="w-3.5 h-3.5 text-muted" />
        Learned Rules
        <span className="text-[10px] text-dim font-light ml-auto">{rules.length} rules</span>
      </h2>

      <div className="flex flex-col gap-1.5 max-h-48 overflow-y-auto">
        {rules.map((r) => (
          <div
            key={r.id}
            className="group flex items-center justify-between py-1.5 text-[12px] font-light"
          >
            <div className="flex items-center gap-2 min-w-0">
              <span
                className={`text-[10px] px-1.5 py-0.5 rounded ${
                  r.classification === "educational" || r.classification === "work_research"
                    ? "bg-success/10 text-success"
                    : "bg-destructive/10 text-destructive"
                }`}
              >
                {r.classification.replace("_", " ")}
              </span>
              <span className="text-primary truncate">{r.pattern}</span>
              <span className="text-dim flex-shrink-0">×{r.hitCount}</span>
            </div>
            <button
              onClick={() => handleDelete(r.id)}
              className="w-5 h-5 rounded flex items-center justify-center text-transparent group-hover:text-muted hover:!text-destructive transition-colors flex-shrink-0"
            >
              <Trash2 className="w-3 h-3" />
            </button>
          </div>
        ))}
      </div>
    </div>
  );
}
```

**Step 2: Add to DayView**

In `desktop-ui/src/components/productivity/DayView.tsx`, import and add after the `AiSummaryCard`:

```tsx
import { LearnedRulesCard } from "./LearnedRulesCard";
```

In the right column, after `<AiSummaryCard ... />`:
```tsx
<LearnedRulesCard />
```

**Step 3: Lint and commit**

```bash
cd desktop-ui && bun run lint:fix
git add desktop-ui/src/components/productivity/LearnedRulesCard.tsx desktop-ui/src/components/productivity/DayView.tsx
git commit -m "feat(desktop-ui): add learned distraction rules management card"
```

---

## Task 7: Frontend — Productivity Score Breakdown

**Why:** The `ProductivityScoreRing` shows only the overall score. The backend computes 4 sub-scores (productive ratio, focus quality, distraction, continuity) but these aren't exposed to the FE yet.

**Files:**
- Modify: `desktop-ui/src/components/productivity/ProductivityScoreRing.tsx` (add breakdown)

**Step 1: Add score breakdown below the ring**

The `DailySummary` already contains the raw data to derive sub-scores. Rather than adding a new backend endpoint, compute the sub-scores client-side from the existing `ProductivitySummary` data.

Update `ProductivityScoreRing` to accept optional breakdown props:

```tsx
interface ProductivityScoreRingProps {
  score: number;
  size?: number;
  summary?: {
    productiveSecs: number;
    neutralSecs: number;
    distractingSecs: number;
    totalActiveSecs: number;
    avgSessionQuality: number | null;
    focusSessionsCount: number;
    contextSwitches: number;
  } | null;
}
```

Then, below the ring and label, render a compact breakdown when `summary` is provided:

```tsx
{summary && summary.totalActiveSecs > 0 && (
  <div className="w-full flex flex-col gap-1.5 mt-2">
    <ScoreBar label="Focus" value={summary.productiveSecs / summary.totalActiveSecs} />
    <ScoreBar label="Quality" value={summary.avgSessionQuality ?? 0} />
    <ScoreBar label="Low distraction" value={1 - summary.distractingSecs / Math.max(summary.totalActiveSecs, 1)} />
    <ScoreBar label="Continuity" value={summary.contextSwitches > 0 ? Math.max(0, 1 - summary.contextSwitches / 100) : 1} />
  </div>
)}
```

Where `ScoreBar` is a small inline component:

```tsx
function ScoreBar({ label, value }: { label: string; value: number }) {
  const pct = Math.round(Math.min(value, 1) * 100);
  return (
    <div className="flex items-center gap-2 text-[10px] font-light">
      <span className="w-20 text-muted text-right">{label}</span>
      <div className="flex-1 h-1 rounded-full bg-surface-raised overflow-hidden">
        <div
          className="h-full rounded-full bg-brand/60 transition-all"
          style={{ width: `${pct}%` }}
        />
      </div>
      <span className="w-8 text-dim tabular-nums">{pct}%</span>
    </div>
  );
}
```

**Step 2: Pass summary from DayView**

In `DayView.tsx`, update the `ProductivityScoreRing` usage:

```tsx
<ProductivityScoreRing score={summary?.productivityScore ?? 0} summary={summary} />
```

**Step 3: Lint and commit**

```bash
cd desktop-ui && bun run lint:fix
git add desktop-ui/src/components/productivity/ProductivityScoreRing.tsx desktop-ui/src/components/productivity/DayView.tsx
git commit -m "feat(desktop-ui): show productivity score breakdown bars"
```

---

## Task 8: Frontend — DistractionBanner wire to IPC

**Why:** The `DistractionBanner` dismiss is `useState` only — refreshing re-shows it. It should call `distraction_dismiss` and persist.

**Files:**
- Modify: `desktop-ui/src/components/productivity/DistractionBanner.tsx`

**Step 1: Read existing component**

Read `desktop-ui/src/components/productivity/DistractionBanner.tsx`.

**Step 2: Wire dismiss to IPC**

Add `useMutation("distraction_dismiss")` and call it on dismiss along with the local state:

```tsx
const { mutate: dismissDistraction } = useMutation("distraction_dismiss");

const handleDismiss = () => {
  setDismissed(true);
  dismissDistraction({ app_name: topDistractingCategory } as any);
};
```

**Step 3: Lint and commit**

```bash
cd desktop-ui && bun run lint:fix
git add desktop-ui/src/components/productivity/DistractionBanner.tsx
git commit -m "fix(desktop-ui): wire distraction banner dismiss to IPC"
```

---

## Task 9: Final — Lint, Test, Verify

**Step 1: Run Rust compilation**

```bash
cargo build -p desktop -p desktop-shared -p dev-api 2>&1 | tail -10
```

**Step 2: Run Rust clippy**

```bash
cargo clippy -p desktop -p desktop-shared -p dev-api --all-targets --all-features 2>&1 | tail -10
```

**Step 3: Run FE lint**

```bash
cd desktop-ui && bun run lint
```

**Step 4: Visual verification**

Open `http://localhost:1420/#/productivity/day/2026-03-05` and verify:
1. Goals card shows `+` button, can create goals, delete on hover
2. Time Entries card appears in center column with `+` to log time
3. Score ring shows breakdown bars underneath
4. Learned Rules card appears in right column (if rules exist)
5. Distraction banner dismiss persists across refresh
6. Week view still shows goals with `+` button

**Step 5: Final commit**

```bash
git add -A
git commit -m "chore(desktop-ui): lint fixes for productivity gap features"
```
