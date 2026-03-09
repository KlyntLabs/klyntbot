# Focus Timer Smart Upgrade — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Transform the standalone tray Pomodoro timer into a fully integrated smart focus system — linked to tasks, coaching, cognitive memory, distraction tracking, and daily stats.

**Architecture:** 10 upgrade tasks split across 3 layers: (1) Rust backend wiring to connect `FocusTimer` → `AppCore` → `FocusManager` → `DomainEventBus`; (2) New Tauri commands for task linking, distraction logging, and today stats; (3) React frontend for task picker, distraction quick-log, coaching debrief, presets, stats display, and flexible extend. Breaking changes are acceptable — no backwards compatibility needed.

**Tech Stack:** Rust (Tauri 2, tokio, sqlx SQLite), TypeScript React (Vite, Tailwind v4 CSS tokens), existing crates: `feature-productivity`, `feature-todo`, `feature-coaching`, `cognitive`, `bus`, `app-core`, `desktop-shared`.

**Key files reference:**
- Timer engine: `crates/desktop/src/focus_timer.rs`
- Tauri commands: `crates/desktop/src/commands/productivity.rs`
- Command registration: `crates/desktop/src/main.rs` (L262-L300)
- AppCore handlers: `crates/app-core/src/handlers/productivity.rs` (L127-L310)
- FocusManager: `crates/feature-productivity/src/focus.rs`
- IPC events/payloads: `crates/desktop-shared/src/events.rs` (L539-L553)
- IPC response types: `crates/desktop-shared/src/commands.rs` (L710-L733)
- React hook: `desktop-ui/src/hooks/useFocusTimer.ts`
- React component: `desktop-ui/src/components/tray/FocusControl.tsx`
- TS types: `desktop-ui/src/lib/types.ts` (L688-L823)

---

## Task 1: Wire Timer Completion to AppCore (C1)

Currently `on_focus_complete` in `focus_timer.rs:L282-L287` calls `core.productivity_focus_end(None)` at the end of `timer_loop`, but the quality score fetch at L343-L351 happens BEFORE that call — so it reads the score from an unfinished session. Also, `productivity_pomodoro_start` at `commands/productivity.rs:L268` passes `None` for both `action_id` and `project_id`. This task fixes the data flow so domain events (coaching, cognitive memory) fire correctly.

**Files:**
- Modify: `crates/desktop/src/focus_timer.rs:L268-L287` (timer_loop completion block)
- Modify: `crates/desktop/src/focus_timer.rs:L335-L390` (on_focus_complete)
- Modify: `crates/desktop/src/commands/productivity.rs:L251-L283` (focus_timer_start)

**Step 1: Fix completion order in timer_loop**

In `focus_timer.rs`, the completion block (L268-L287) currently calls `on_focus_complete` first, then `productivity_focus_end`. This means `on_focus_complete` reads quality_score from an unfinished session. Reverse the order: end session first, then build notification with the computed quality.

Replace `focus_timer.rs:L268-L287`:

```rust
    // Timer complete
    let is_break = mode == "break";

    if is_break {
        on_break_complete(&app).await;
    } else {
        // End the AppCore session FIRST (computes quality, emits DomainEvent::FocusSessionEnded)
        let ended_session = if let Some(core) = app.try_state::<Arc<AppCore>>() {
            core.productivity_focus_end(None).await.ok().flatten()
        } else {
            None
        };

        on_focus_complete(&app, &mode, total_secs, break_mins, ended_session.as_ref()).await;
    }

    clear_tray_title(&app);

    if let Some(timer) = app.try_state::<Arc<FocusTimer>>() {
        timer.mark_completed().await;
    }
```

**Step 2: Update on_focus_complete to accept ended session**

Replace `focus_timer.rs:L335-L390`:

```rust
async fn on_focus_complete(
    app: &AppHandle,
    mode: &str,
    total_secs: u64,
    break_mins: Option<u64>,
    ended_session: Option<&FocusSessionResponse>,
) {
    let duration_mins = total_secs / 60;
    let quality_score = ended_session.and_then(|s| s.quality_score);

    // Notification
    let body = match (break_mins, quality_score) {
        (Some(brk), Some(q)) => format!(
            "{duration_mins}m done (quality {}%). Take a {brk}m break!",
            (q * 100.0).round() as u32
        ),
        (Some(brk), None) => {
            format!("{duration_mins}m session done. Time for a {brk}m break!")
        }
        (None, Some(q)) => format!(
            "{duration_mins}m session finished. Quality: {}%",
            (q * 100.0).round() as u32
        ),
        (None, None) => format!("{duration_mins}m session finished."),
    };
    let _ = common::utils::notify::send_os_notification("Focus Session Complete", &body).await;

    // Sound
    #[cfg(target_os = "macos")]
    {
        let _ = tokio::process::Command::new("afplay")
            .arg("/System/Library/Sounds/Glass.aiff")
            .spawn();
    }

    // Frontend event
    let _ = app.emit(
        FOCUS_COMPLETED,
        FocusCompletedPayload {
            mode: mode.to_string(),
            duration_mins,
            quality_score,
            break_mins,
        },
    );

    open_tray_window(app);
}
```

**Step 3: Add import for FocusSessionResponse**

At the top of `focus_timer.rs`, ensure this import exists:
```rust
use desktop_shared::commands::FocusSessionResponse;
```

**Step 4: Update focus_timer_start to pass action_id through**

In `commands/productivity.rs:L251-L283`, add `action_id` and `action_title` parameters. The `productivity_pomodoro_start` currently passes `None` for both — wire them through:

```rust
#[tauri::command(rename_all = "snake_case")]
pub async fn focus_timer_start(
    state: State<'_, Arc<AppCore>>,
    timer: State<'_, Arc<FocusTimer>>,
    app: tauri::AppHandle,
    mode: String,
    work_mins: u64,
    break_mins: Option<u64>,
    action_id: Option<String>,
    action_title: Option<String>,
) -> Result<FocusSessionResponse, ApiError> {
    let timer_mode = match mode.as_str() {
        "pomodoro" => TimerMode::Pomodoro,
        _ => TimerMode::Focus,
    };

    // Start the persistent session first
    let session = if timer_mode == TimerMode::Pomodoro {
        state
            .productivity_pomodoro_start_with_action(
                action_id,
                None,
                Some(work_mins as i64),
                break_mins.map(|b| b as i64),
            )
            .await?
    } else {
        state
            .productivity_focus_start(action_id, None, Some(work_mins as i64))
            .await?
    };

    // Then start the desktop timer (tray title + countdown)
    timer
        .start(app, timer_mode, work_mins, break_mins, action_title)
        .await
        .map_err(|e| ApiError::new("TIMER_ERROR", e.to_string()))?;

    Ok(session)
}
```

**Step 5: Add productivity_pomodoro_start_with_action to AppCore**

In `crates/app-core/src/handlers/productivity.rs`, add after `productivity_pomodoro_start` (L298-L308):

```rust
    pub async fn productivity_pomodoro_start_with_action(
        &self,
        action_id: Option<String>,
        project_id: Option<String>,
        work_mins: Option<i64>,
        break_mins: Option<i64>,
    ) -> Result<FocusSessionResponse, ApiError> {
        let focus_mgr = self.focus_manager()?;
        let session = focus_mgr
            .start_pomodoro(action_id, project_id, work_mins, break_mins)
            .await
            .map_err(map_prod_err)?;
        Ok(session_to_response(session))
    }
```

**Step 6: Update FocusTimer::start to accept action_title for tray display**

In `focus_timer.rs`, update `TimerState` (L40-L47):

```rust
struct TimerState {
    mode: TimerMode,
    total_secs: u64,
    break_mins: Option<u64>,
    action_title: Option<String>,
    handle: JoinHandle<()>,
    cmd_tx: mpsc::Sender<TimerCommand>,
}
```

Update `FocusTimer::start` signature (L63) to accept `action_title: Option<String>`:

```rust
pub async fn start(
    &self,
    app: AppHandle,
    mode: TimerMode,
    work_mins: u64,
    break_mins: Option<u64>,
    action_title: Option<String>,
) -> common::Result<()> {
```

Store it in `TimerState` and pass to `timer_loop`. Update `timer_loop` signature and `update_tray_title` to show task name:

```rust
async fn timer_loop(
    app: AppHandle,
    mode: String,
    mut total_secs: u64,
    break_mins: Option<u64>,
    action_title: Option<String>,
    mut cmd_rx: mpsc::Receiver<TimerCommand>,
)
```

Update `update_tray_title` (L292-L304):

```rust
fn update_tray_title(app: &AppHandle, remaining_secs: u64, paused: bool, action_title: Option<&str>) {
    let mins = remaining_secs / 60;
    let secs = remaining_secs % 60;
    let time = if paused {
        format!("⏸ {mins:02}:{secs:02}")
    } else {
        format!("{mins:02}:{secs:02}")
    };
    let title = match action_title {
        Some(t) if !t.is_empty() => {
            let truncated: String = t.chars().take(20).collect();
            format!("{time} · {truncated}")
        }
        _ => time,
    };

    if let Some(tray) = app.tray_by_id("klynt-tray") {
        let _ = tray.set_title(Some(&title));
    }
}
```

Also update `start_break` (L99-L130) to pass `None` for `action_title`.

**Step 7: Update all callers of timer_loop and update_tray_title**

Search for all call sites in `focus_timer.rs` and pass the `action_title` parameter. There are 2 call sites for `timer_loop` (inside `start` and `start_break`) and ~2 for `update_tray_title` (inside `timer_loop`).

**Step 8: Add FocusTickPayload.actionTitle field**

In `desktop-shared/src/events.rs:L539-L544`, add:

```rust
pub struct FocusTickPayload {
    pub remaining_secs: u64,
    pub total_secs: u64,
    pub mode: String,
    pub paused: bool,
    pub action_title: Option<String>,
}
```

Update the emit call in `timer_loop` to include `action_title`.

**Step 9: Update frontend FocusTickPayload type**

In `desktop-ui/src/lib/types.ts:L811-L816`, add:
```ts
export interface FocusTickPayload {
  remainingSecs: number;
  totalSecs: number;
  mode: string;
  paused: boolean;
  actionTitle: string | null;
}
```

**Step 10: Update useFocusTimer to track actionTitle**

In `desktop-ui/src/hooks/useFocusTimer.ts`, add state:
```ts
const [actionTitle, setActionTitle] = useState<string | null>(null);
```

In the `focus:tick` handler (L100-L106), add:
```ts
setActionTitle(payload.actionTitle ?? null);
```

Clear it in `stop` (L186-L197): `setActionTitle(null);`

Include `actionTitle` in the return object (L289-L319).

**Step 11: Build and verify**

Run: `cargo build --workspace`
Expected: Clean build, 0 errors.

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings.

**Step 12: Commit**

```bash
git add crates/desktop/src/focus_timer.rs crates/desktop/src/commands/productivity.rs \
       crates/app-core/src/handlers/productivity.rs crates/desktop-shared/src/events.rs \
       desktop-ui/src/lib/types.ts desktop-ui/src/hooks/useFocusTimer.ts
git commit -m "feat(focus): wire timer completion to AppCore, add action_id + tray task title"
```

---

## Task 2: Loading Guards & Break Pending Stop (C3 + M3)

Prevent double-click race conditions and add a Stop button during the 5s break_pending window.

**Files:**
- Modify: `desktop-ui/src/components/tray/FocusControl.tsx:L244-L374` (bottom bar), `L413-L446` (BreakPendingActions)
- Modify: `desktop-ui/src/hooks/useFocusTimer.ts:L289-L319` (return loading)

**Step 1: Disable all action buttons when loading**

In `FocusControl.tsx`, every `<button>` in the bottom bar and in `BreakPendingActions` that triggers an async action should receive `disabled={timer.loading}` and a disabled style class. Add this utility class near the top of FocusControl.tsx:

```tsx
const disabledIf = (loading: boolean) =>
  loading ? "opacity-50 pointer-events-none" : "";
```

Apply to every action button in:
- Break phase bottom bar (L246-L291): Pause/Resume, Skip, Stop
- Focus phase bottom bar (L293-L339): Pause/Resume, Break, Stop
- Idle phase bottom bar (L340-L373): Start
- BreakPendingActions (L413-L446): "+5m more", "Start Break"
- WarningBanner (L381-L409): extend, stop

**Step 2: Add Stop button to BreakPendingActions**

In `BreakPendingActions` (L413-L446), add a third button after "Start Break":

```tsx
<button
  type="button"
  onClick={() => timer.stop()}
  disabled={timer.loading}
  className={`flex items-center gap-1 px-3 py-1.5 rounded-lg text-xs
    bg-surface-raised/50 text-muted hover:text-foreground transition-colors
    ${disabledIf(timer.loading)}`}
>
  <Square className="w-3 h-3" />
  Stop
</button>
```

Import `Square` from `lucide-react` if not already imported.

**Step 3: Lint and format**

Run: `cd desktop-ui && bun run lint:fix`
Expected: Clean.

**Step 4: Commit**

```bash
git add desktop-ui/src/components/tray/FocusControl.tsx
git commit -m "fix(focus): disable buttons during loading, add stop in break_pending"
```

---

## Task 3: Task Integration — Task Picker UI (H1)

Add a task picker that appears in idle phase, allowing the user to link a focus session to a task.

**Files:**
- Modify: `desktop-ui/src/hooks/useFocusTimer.ts` (add actionId state, pass to startTimer)
- Modify: `desktop-ui/src/components/tray/FocusControl.tsx` (add task picker in idle view)
- Modify: `desktop-ui/src/lib/types.ts` (if needed)

**Step 1: Add task state to useFocusTimer**

In `useFocusTimer.ts`, add state variables after L96:

```ts
const [selectedTaskId, setSelectedTaskId] = useState<string | null>(null);
const [selectedTaskTitle, setSelectedTaskTitle] = useState<string | null>(null);
```

**Step 2: Pass action_id and action_title in start()**

Update `start` function (L166-L184) to include task info in the mutation:

```ts
const start = useCallback(async () => {
    setCompleted(null);
    setPaused(false);
    setPhase("focus");
    let sessions = completedSessions;
    if (sessions >= settings.longBreakAfter) {
      sessions = 0;
      setCompletedSessions(0);
      saveSessions(0);
    }
    const nextIsLongBreak = sessions + 1 >= settings.longBreakAfter;
    const breakMins = nextIsLongBreak ? settings.longBreak : settings.shortBreak;
    await startTimer.mutate({
      mode: "focus",
      work_mins: settings.focusDuration,
      break_mins: breakMins,
      action_id: selectedTaskId ?? undefined,
      action_title: selectedTaskTitle ?? undefined,
    });
    refetch();
  }, [startTimer, refetch, settings, completedSessions, selectedTaskId, selectedTaskTitle]);
```

Update the type parameter of `startTimer` useMutation (L80-L83):

```ts
const startTimer = useMutation<
  FocusSession,
  { mode: string; work_mins: number; break_mins?: number; action_id?: string; action_title?: string }
>("focus_timer_start");
```

**Step 3: Clear task on stop, add to return object**

In `stop` (L186-L197), add after `setPaused(false)`:
```ts
setSelectedTaskId(null);
setSelectedTaskTitle(null);
```

Add to return object:
```ts
selectedTaskId,
selectedTaskTitle,
selectTask: (id: string | null, title: string | null) => {
  setSelectedTaskId(id);
  setSelectedTaskTitle(title);
},
```

**Step 4: Create TaskPicker component in FocusControl.tsx**

Add a `TaskPicker` component inside `FocusControl.tsx`. It queries `today_tasks` (existing IPC command that returns `TodayTask[]`) and shows a compact dropdown:

```tsx
function TaskPicker({
  selectedId,
  onSelect,
}: {
  selectedId: string | null;
  onSelect: (id: string | null, title: string | null) => void;
}) {
  const { data: tasks } = useQuery<TodayTask[]>("today_tasks", undefined, []);
  const [open, setOpen] = useState(false);

  const selected = tasks.find((t) => t.id === selectedId);

  return (
    <div className="relative w-full">
      <button
        type="button"
        onClick={() => setOpen(!open)}
        className="w-full px-3 py-1.5 text-xs text-left rounded-lg bg-surface-raised/50
                   text-muted hover:text-foreground transition-colors truncate"
      >
        {selected ? selected.title : "No task linked"}
      </button>
      {open && (
        <div className="absolute bottom-full left-0 right-0 mb-1 rounded-lg glass-panel
                        border border-border p-1 max-h-40 overflow-y-auto z-50">
          <button
            type="button"
            onClick={() => { onSelect(null, null); setOpen(false); }}
            className="w-full px-2 py-1 text-xs text-left text-muted hover:text-foreground
                       hover:bg-surface-raised/50 rounded"
          >
            No task
          </button>
          {tasks.filter((t) => !t.completed).map((task) => (
            <button
              key={task.id}
              type="button"
              onClick={() => { onSelect(task.id, task.title); setOpen(false); }}
              className={`w-full px-2 py-1 text-xs text-left rounded truncate
                ${task.id === selectedId
                  ? "text-brand bg-brand/10"
                  : "text-muted hover:text-foreground hover:bg-surface-raised/50"
                }`}
            >
              {task.title}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
```

Import `TodayTask` from `../../lib/types` and `useQuery` from `../../hooks/useQuery`.

**Step 5: Render TaskPicker in idle phase**

In `TimerView`, inside the idle bottom bar section (L340-L373), add the `TaskPicker` above the Start button:

```tsx
{/* Task picker — idle only */}
{phase === "idle" && (
  <TaskPicker
    selectedId={timer.selectedTaskId}
    onSelect={timer.selectTask}
  />
)}
```

**Step 6: Show task title inside ring during focus**

In the ring content area (around L162-L222), when `phase === "focus"` and `timer.actionTitle` is set, show it below the time display:

```tsx
{timer.actionTitle && (phase === "focus") && (
  <p className="text-[10px] text-muted truncate max-w-[120px] mt-0.5">
    {timer.actionTitle}
  </p>
)}
```

**Step 7: Lint and format**

Run: `cd desktop-ui && bun run lint:fix`

**Step 8: Build and verify**

Run: `cargo build --workspace && cd desktop-ui && bun run build`

**Step 9: Commit**

```bash
git add desktop-ui/src/hooks/useFocusTimer.ts desktop-ui/src/components/tray/FocusControl.tsx \
       desktop-ui/src/lib/types.ts
git commit -m "feat(focus): add task picker to link focus sessions to tasks"
```

---

## Task 4: Distraction Quick-Log (H3)

Add quick-tap distraction buttons during focus mode. Reuse existing `distraction_dismiss` Tauri command which calls `AppCore::distraction_dismiss` → `FocusManager::record_distraction`.

**Files:**
- Modify: `desktop-ui/src/components/tray/FocusControl.tsx` (add QuickDistractionLog component)
- Modify: `desktop-ui/src/hooks/useFocusTimer.ts` (add logDistraction mutation)

**Step 1: Add logDistraction mutation to hook**

In `useFocusTimer.ts`, add after the other mutations (L88):

```ts
const logDistraction = useMutation<void, { app_name: string }>("distraction_dismiss");
```

Add a `logDistraction` callback:

```ts
const logDistractionCb = useCallback(
  async (category: string) => {
    await logDistraction.mutate({ app_name: category });
  },
  [logDistraction],
);
```

Add to return object:
```ts
logDistraction: logDistractionCb,
```

**Step 2: Create QuickDistractionLog component**

In `FocusControl.tsx`, add:

```tsx
const DISTRACTION_CATEGORIES = [
  { label: "Social", value: "social_media" },
  { label: "Chat", value: "chat" },
  { label: "Email", value: "email" },
  { label: "Tired", value: "fatigue" },
  { label: "Meeting", value: "meeting" },
] as const;

function QuickDistractionLog({ onLog }: { onLog: (cat: string) => void }) {
  return (
    <div className="flex gap-1 justify-center flex-wrap px-2">
      {DISTRACTION_CATEGORIES.map((c) => (
        <button
          key={c.value}
          type="button"
          onClick={() => onLog(c.value)}
          className="px-2 py-0.5 text-[10px] rounded-md bg-surface-raised/30
                     text-muted hover:text-foreground transition-colors"
        >
          {c.label}
        </button>
      ))}
    </div>
  );
}
```

**Step 3: Render QuickDistractionLog during focus phase**

In `TimerView`, show the distraction log below the ring when in focus phase and NOT showing the warning banner:

```tsx
{phase === "focus" && !showWarning && (
  <QuickDistractionLog onLog={timer.logDistraction} />
)}
```

**Step 4: Lint and commit**

Run: `cd desktop-ui && bun run lint:fix`

```bash
git add desktop-ui/src/hooks/useFocusTimer.ts desktop-ui/src/components/tray/FocusControl.tsx
git commit -m "feat(focus): add distraction quick-log during focus sessions"
```

---

## Task 5: Coaching Debrief in Break Pending (H4)

After a focus session completes, the coaching service emits `COACHING_INTERVENTION` via IPC (wired in `crates/desktop/src/app_core.rs:L103-L127`). This fires automatically once Task 1 is done (domain events now flow). Add a UI component to show the coaching debrief.

**Files:**
- Modify: `desktop-ui/src/hooks/useFocusTimer.ts` (subscribe to coaching:intervention)
- Modify: `desktop-ui/src/components/tray/FocusControl.tsx` (render coaching card)

**Step 1: Add coaching intervention state to hook**

In `useFocusTimer.ts`, add state and type:

```ts
interface CoachingIntervention {
  message: string;
  interventionType: string;
}

const [coaching, setCoaching] = useState<CoachingIntervention | null>(null);
```

Subscribe to the event:

```ts
useEvent<CoachingIntervention>("coaching:intervention", (payload) => {
  if (payload?.message) {
    setCoaching(payload);
  }
});
```

Clear coaching on `start`:
```ts
setCoaching(null);
```

Add to return object:
```ts
coaching,
dismissCoaching: useCallback(() => setCoaching(null), []),
```

**Step 2: Create CoachingDebrief component**

In `FocusControl.tsx`:

```tsx
function CoachingDebrief({
  message,
  onDismiss,
}: {
  message: string;
  onDismiss: () => void;
}) {
  return (
    <div className="mx-2 p-2.5 rounded-lg bg-brand/10 border border-brand/20">
      <div className="flex items-start gap-2">
        <Sparkles className="w-3.5 h-3.5 text-brand mt-0.5 shrink-0" />
        <p className="text-xs text-foreground leading-relaxed flex-1">
          {message}
        </p>
        <button
          type="button"
          onClick={onDismiss}
          className="text-muted hover:text-foreground shrink-0"
        >
          <X className="w-3 h-3" />
        </button>
      </div>
    </div>
  );
}
```

Import `Sparkles` and `X` from `lucide-react`.

**Step 3: Render in break_pending phase**

In `TimerView`, show the coaching debrief in the break_pending view, below `BreakPendingActions`:

```tsx
{phase === "break_pending" && timer.coaching && (
  <CoachingDebrief
    message={timer.coaching.message}
    onDismiss={timer.dismissCoaching}
  />
)}
```

**Step 4: Lint and commit**

Run: `cd desktop-ui && bun run lint:fix`

```bash
git add desktop-ui/src/hooks/useFocusTimer.ts desktop-ui/src/components/tray/FocusControl.tsx
git commit -m "feat(focus): show coaching debrief after focus session completion"
```

---

## Task 6: Quick Presets (M1)

Add preset buttons in idle phase for quick session configuration.

**Files:**
- Modify: `desktop-ui/src/components/tray/FocusControl.tsx` (add presets UI)
- Modify: `desktop-ui/src/hooks/useFocusTimer.ts` (add preset type)

**Step 1: Define presets**

In `useFocusTimer.ts`, add after `DEFAULT_SETTINGS` (L25-L31):

```ts
export interface FocusPreset {
  label: string;
  focusDuration: number;
  shortBreak: number;
}

export const FOCUS_PRESETS: FocusPreset[] = [
  { label: "Standard", focusDuration: 25, shortBreak: 5 },
  { label: "Deep Work", focusDuration: 50, shortBreak: 10 },
  { label: "Sprint", focusDuration: 15, shortBreak: 3 },
];
```

Export them from the hook file.

**Step 2: Add activePreset detection to hook**

In the hook, add a derived value:

```ts
const activePreset = FOCUS_PRESETS.find(
  (p) => p.focusDuration === settings.focusDuration && p.shortBreak === settings.shortBreak,
)?.label ?? "Custom";
```

Add to return object:
```ts
activePreset,
applyPreset: useCallback((preset: FocusPreset) => {
  updateSettings({ focusDuration: preset.focusDuration, shortBreak: preset.shortBreak });
}, [updateSettings]),
```

**Step 3: Add preset selector in idle phase**

In `FocusControl.tsx`, in the idle section (around the ring area), add preset chips below the ring when idle:

```tsx
{phase === "idle" && (
  <div className="flex gap-1.5 justify-center">
    {FOCUS_PRESETS.map((preset) => (
      <button
        key={preset.label}
        type="button"
        onClick={() => timer.applyPreset(preset)}
        className={`px-2.5 py-1 text-[10px] rounded-full transition-colors
          ${timer.activePreset === preset.label
            ? "bg-brand/20 text-brand border border-brand/30"
            : "bg-surface-raised/30 text-muted hover:text-foreground border border-transparent"
          }`}
      >
        {preset.label} {preset.focusDuration}/{preset.shortBreak}
      </button>
    ))}
  </div>
)}
```

Import `FOCUS_PRESETS` and `FocusPreset` from the hook file.

**Step 4: Lint and commit**

Run: `cd desktop-ui && bun run lint:fix`

```bash
git add desktop-ui/src/hooks/useFocusTimer.ts desktop-ui/src/components/tray/FocusControl.tsx
git commit -m "feat(focus): add quick preset selector (Standard/Deep Work/Sprint)"
```

---

## Task 7: Mini Stats — Today's Summary (M2)

Show "Today: 3 sessions · 1h 45m" in the idle view. Reuse existing `productivity_sessions` Tauri command.

**Files:**
- Modify: `desktop-ui/src/components/tray/FocusControl.tsx` (add TodayStats component)
- Modify: `desktop-ui/src/hooks/useFocusTimer.ts` (add today stats query)

**Step 1: Add today stats query to hook**

In `useFocusTimer.ts`, add a query for today's sessions. The existing `productivity_sessions` command accepts a `date` string:

```ts
const todayDate = new Date().toISOString().slice(0, 10); // "YYYY-MM-DD"
const { data: todaySessions, refetch: refetchToday } = useQuery<FocusSession[]>(
  "productivity_sessions",
  { date: todayDate },
  [],
);
```

Derive stats:

```ts
const todayStats = {
  sessions: todaySessions.filter((s) => s.completed).length,
  totalMins: todaySessions
    .filter((s) => s.completed)
    .reduce((sum, s) => sum + (s.actualMins ?? 0), 0),
  avgQuality: (() => {
    const scored = todaySessions.filter((s) => s.qualityScore != null);
    if (scored.length === 0) return null;
    return scored.reduce((sum, s) => sum + (s.qualityScore ?? 0), 0) / scored.length;
  })(),
};
```

In the `focus:completed` handler (L109-L128), after `refetch()`, also call `refetchToday()`.

Add to return object:
```ts
todayStats,
```

**Step 2: Create TodayStats component**

In `FocusControl.tsx`:

```tsx
function TodayStats({ stats }: { stats: { sessions: number; totalMins: number; avgQuality: number | null } }) {
  if (stats.sessions === 0) return null;

  const hours = Math.floor(stats.totalMins / 60);
  const mins = stats.totalMins % 60;
  const timeStr = hours > 0 ? `${hours}h ${mins}m` : `${mins}m`;

  return (
    <div className="text-center text-[10px] text-muted">
      Today: {stats.sessions} session{stats.sessions !== 1 ? "s" : ""} · {timeStr}
      {stats.avgQuality != null && (
        <span> · {(stats.avgQuality * 100).toFixed(0)}% quality</span>
      )}
    </div>
  );
}
```

**Step 3: Render in idle phase**

Show `TodayStats` above the ring in idle phase:

```tsx
{phase === "idle" && <TodayStats stats={timer.todayStats} />}
```

**Step 4: Lint and commit**

Run: `cd desktop-ui && bun run lint:fix`

```bash
git add desktop-ui/src/hooks/useFocusTimer.ts desktop-ui/src/components/tray/FocusControl.tsx
git commit -m "feat(focus): show today's session stats in idle view"
```

---

## Task 8: Flexible Extend Options (M5)

Replace single "+5m" / "+30s" buttons with a dropdown offering +5m, +10m, +15m (focus) or +30s, +1m, +2m (break).

**Files:**
- Modify: `desktop-ui/src/components/tray/FocusControl.tsx:L381-L409` (WarningBanner), `L413-L446` (BreakPendingActions)

**Step 1: Update WarningBanner**

Replace the single extend button with multiple options:

```tsx
function WarningBanner({ timer, isFocus }: { timer: Timer; isFocus: boolean }) {
  const extendOptions = isFocus
    ? [
        { label: "+5m", secs: 300 },
        { label: "+10m", secs: 600 },
        { label: "+15m", secs: 900 },
      ]
    : [
        { label: "+30s", secs: 30 },
        { label: "+1m", secs: 60 },
        { label: "+2m", secs: 120 },
      ];

  return (
    <div className="mx-2 p-2 rounded-lg bg-warning/10 border border-warning/20">
      <p className="text-xs text-center text-warning mb-2">
        {isFocus ? "Focus ending soon" : "Break ending soon"}
      </p>
      <div className="flex gap-1.5 justify-center">
        {extendOptions.map((opt) => (
          <button
            key={opt.secs}
            type="button"
            onClick={() => timer.extend(opt.secs)}
            disabled={timer.loading}
            className="px-2 py-1 text-xs rounded-md bg-warning/20 text-warning
                       hover:bg-warning/30 transition-colors disabled:opacity-50"
          >
            {opt.label}
          </button>
        ))}
        <button
          type="button"
          onClick={() => timer.stop()}
          disabled={timer.loading}
          className="px-2 py-1 text-xs rounded-md bg-surface-raised/50
                     text-muted hover:text-foreground transition-colors disabled:opacity-50"
        >
          End now
        </button>
      </div>
    </div>
  );
}
```

**Step 2: Update BreakPendingActions with flexible extend**

Replace the single "+5m more" with options:

```tsx
function BreakPendingActions({ timer, cycleComplete }: { timer: Timer; cycleComplete: boolean }) {
  const breakMins = timer.completed?.breakMins ?? timer.settings.shortBreak;

  return (
    <div className="flex flex-col gap-2 items-center px-4">
      <p className="text-xs text-muted">
        {cycleComplete ? `Cycle done! ${breakMins}m long break` : `${breakMins}m break`}
      </p>
      <div className="flex gap-1.5">
        {[5, 10, 15].map((mins) => (
          <button
            key={mins}
            type="button"
            onClick={() => {
              // extendWork starts a new focus session of N minutes
              timer.extendWork(mins);
            }}
            disabled={timer.loading}
            className="px-2 py-1 text-xs rounded-md bg-surface-raised/50
                       text-muted hover:text-foreground transition-colors disabled:opacity-50"
          >
            +{mins}m work
          </button>
        ))}
      </div>
      <div className="flex gap-1.5">
        <button
          type="button"
          onClick={() => timer.startBreak()}
          disabled={timer.loading}
          className="px-3 py-1.5 text-xs rounded-lg bg-info/20 text-info
                     hover:bg-info/30 transition-colors disabled:opacity-50"
        >
          Start Break
        </button>
        <button
          type="button"
          onClick={() => timer.stop()}
          disabled={timer.loading}
          className="px-3 py-1.5 text-xs rounded-lg bg-surface-raised/50
                     text-muted hover:text-foreground transition-colors disabled:opacity-50"
        >
          Stop
        </button>
      </div>
    </div>
  );
}
```

**Step 3: Update extendWork to accept duration parameter**

In `useFocusTimer.ts`, update `extendWork` (L243-L255) to accept a `mins` parameter:

```ts
const extendWork = useCallback(async (mins: number = 5) => {
    if (autoBreakTimer.current) clearTimeout(autoBreakTimer.current);
    setPhase("focus");
    setCompleted(null);
    setPaused(false);
    const breakMins = completed?.breakMins ?? settings.shortBreak;
    await startTimer.mutate({
      mode: "focus",
      work_mins: mins,
      break_mins: breakMins,
      action_id: selectedTaskId ?? undefined,
      action_title: selectedTaskTitle ?? undefined,
    });
    refetch();
  }, [startTimer, refetch, completed, settings.shortBreak, selectedTaskId, selectedTaskTitle]);
```

**Step 4: Lint and commit**

Run: `cd desktop-ui && bun run lint:fix`

```bash
git add desktop-ui/src/components/tray/FocusControl.tsx desktop-ui/src/hooks/useFocusTimer.ts
git commit -m "feat(focus): flexible extend options (+5/+10/+15m work, +30s/+1m/+2m break)"
```

---

## Task 9: Persist Timer State — Remove localStorage (C2)

Move `completedSessions` tracking from localStorage to SQLite. On mount, derive session count from today's completed sessions query (added in Task 7).

**Files:**
- Modify: `desktop-ui/src/hooks/useFocusTimer.ts` (remove localStorage, derive from query)

**Step 1: Remove localStorage helpers**

Delete `SESSIONS_KEY` constant (L15), `loadSessions` function (L51-L57), and `saveSessions` function (L59-L65).

**Step 2: Derive completedSessions from todaySessions**

Replace the `completedSessions` state (L96) and all `saveSessions` calls with a derived value from the `todaySessions` query (added in Task 7):

```ts
const completedSessions = todaySessions.filter((s) => s.completed).length;
```

Remove `setCompletedSessions` calls from:
- `focus:completed` handler (L120-L124)
- `start` function (L170-L174)
- `takeBreak` function (L222-L226)
- `skipBreak` function (L266-L271)

Instead, the session count is automatically correct because `productivity_focus_end` persists to SQLite, and `refetchToday()` re-queries.

**Step 3: Update resetSessions**

`resetSessions` (L282-L285) can no longer clear localStorage. Since session counting is now derived from SQLite, this function becomes a no-op or should reset the cycle position. For now, remove it from the return object and remove the Reset button from the idle bottom bar.

**Step 4: Fix cycle calculation**

The `start`, `skipBreak`, and `takeBreak` functions calculate break duration based on `completedSessions`. Since `completedSessions` is now derived from `todaySessions.filter(s => s.completed).length`, the cycle position within `longBreakAfter` should use modulo:

```ts
const cyclePosition = completedSessions % settings.longBreakAfter;
```

Update `start`:
```ts
const nextIsLongBreak = cyclePosition + 1 >= settings.longBreakAfter;
```

Update `skipBreak` and `takeBreak` similarly.

**Step 5: Lint, build, and commit**

Run: `cd desktop-ui && bun run lint:fix && bun run build`

```bash
git add desktop-ui/src/hooks/useFocusTimer.ts desktop-ui/src/components/tray/FocusControl.tsx
git commit -m "refactor(focus): replace localStorage sessions with SQLite-derived count"
```

---

## Task 10: Sound & Notification Preferences (N2)

Wire the existing no-op checkboxes in FocusSettingsPanel to actual settings.

**Files:**
- Modify: `desktop-ui/src/hooks/useFocusTimer.ts` (add settings fields)
- Modify: `desktop-ui/src/components/tray/FocusControl.tsx:L531-L549` (wire checkboxes)
- Modify: `crates/desktop/src/focus_timer.rs` (respect settings via IPC)

**Step 1: Add settings fields**

In `useFocusTimer.ts`, update `FocusSettings` interface (L17-L23):

```ts
export interface FocusSettings {
  focusDuration: number;
  shortBreak: number;
  longBreak: number;
  longBreakAfter: number;
  dndEnabled: boolean;
  soundEnabled: boolean;
  notificationEnabled: boolean;
}
```

Update `DEFAULT_SETTINGS` (L25-L31):

```ts
const DEFAULT_SETTINGS: FocusSettings = {
  focusDuration: 25,
  shortBreak: 5,
  longBreak: 15,
  longBreakAfter: 4,
  dndEnabled: false,
  soundEnabled: true,
  notificationEnabled: true,
};
```

**Step 2: Wire checkboxes in FocusSettingsPanel**

In `FocusControl.tsx`, update the notifications tab (L531-L549) to use actual settings:

```tsx
<Checkbox
  checked={settings.soundEnabled}
  onCheckedChange={(v) => onUpdate({ soundEnabled: !!v })}
  label="Sound"
/>
<Checkbox
  checked={settings.notificationEnabled}
  onCheckedChange={(v) => onUpdate({ notificationEnabled: !!v })}
  label="Notification"
/>
```

**Step 3: Pass sound/notification preferences to backend**

This requires adding `sound_enabled` and `notification_enabled` parameters to `focus_timer_start`. However, since the sound and notification logic lives in `on_focus_complete` and `on_break_complete` in `focus_timer.rs`, and those functions read from `AppHandle` state rather than parameters, we need a different approach.

Add these to `FocusTickPayload` is overkill. Instead, store the preferences in `FocusTimer` itself:

In `focus_timer.rs`, add to `TimerState`:
```rust
sound_enabled: bool,
notification_enabled: bool,
```

Add a method to `FocusTimer`:
```rust
pub async fn preferences(&self) -> (bool, bool) {
    let guard = self.state.lock().await;
    guard.as_ref()
        .map(|s| (s.sound_enabled, s.notification_enabled))
        .unwrap_or((true, true))
}
```

Update `focus_timer_start` to accept `sound_enabled: Option<bool>` and `notification_enabled: Option<bool>`, defaulting to `true`.

In `on_focus_complete` and `on_break_complete`, read preferences:
```rust
let (sound_enabled, notification_enabled) = if let Some(timer) = app.try_state::<Arc<FocusTimer>>() {
    timer.preferences().await
} else {
    (true, true)
};

if notification_enabled {
    let _ = common::utils::notify::send_os_notification(...).await;
}

#[cfg(target_os = "macos")]
if sound_enabled {
    let _ = tokio::process::Command::new("afplay")...;
}
```

**Step 4: Update frontend mutation to pass preferences**

In `useFocusTimer.ts`, update `startTimer.mutate` call in `start()` to include:
```ts
sound_enabled: settings.soundEnabled,
notification_enabled: settings.notificationEnabled,
```

Update the `startTimer` mutation type to include these optional fields.

**Step 5: Build, lint, and commit**

Run: `cargo build --workspace && cd desktop-ui && bun run lint:fix && bun run build`

```bash
git add crates/desktop/src/focus_timer.rs crates/desktop/src/commands/productivity.rs \
       desktop-ui/src/hooks/useFocusTimer.ts desktop-ui/src/components/tray/FocusControl.tsx
git commit -m "feat(focus): wire sound and notification preference toggles"
```

---

## Verification Checklist

After all tasks are complete, verify end-to-end:

1. **Start focus with task linked** → tray shows "25:00 · Task name", `focus_sessions` row has `action_id`
2. **Log distraction during focus** → session `interruptions` increments, `distraction_events` grows
3. **Focus completes naturally** → coaching debrief shows in break_pending, `DomainEvent::FocusSessionEnded` fires (check logs)
4. **Click Stop in break_pending** → timer fully stops, tray clears
5. **Select different presets** → settings update, timer starts with correct duration
6. **Check today stats** → shows correct count after sessions complete
7. **Flexible extend** → all extend options (+5m, +10m, +15m in focus; +30s, +1m, +2m in break) work correctly
8. **Disable buttons during loading** → rapid clicking doesn't fire duplicate IPC calls
9. **Toggle sound/notification off** → completion plays no sound, shows no OS notification
10. **App restart mid-session** → timer reconnects (existing `useEffect` on `timerStatus.active`), session count derived from SQLite
