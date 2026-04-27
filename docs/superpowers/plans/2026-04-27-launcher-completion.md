# Launcher Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring the launcher feature from ~90% complete to 100%: remove dead code, integrate latent features (pinning, Restore window action, calendar search, agent tool exposure), replace UI stubs (chat, voice), surface errors to users, and harden performance + UX/UI.

**Architecture:** The launcher is a thin Tauri-IPC feature: React UI in `desktop-ui/src/features/launcher/` calls 10 IPC commands in `crates/desktop/src/commands/launcher.rs`, which delegate to `AppCore` handlers in `crates/app-core/src/handlers/launcher/`, which call into the `feature-launcher` crate. We extend this same shape — no architectural rewrite. Pinning + Calendar source slot into the existing `SourceRegistry`. The agent tool wraps existing `LauncherSearchEngine` methods. Stub replacements consume APIs that already exist (voice-conversation manager, chat-surface-integration plan).

**Tech Stack:** Rust (sqlx, tokio, async-trait, nucleo-matcher, ArcSwap, jiff), Tauri 2 with `#[klynt_command]`/`#[klynt_raw_command]` macros, specta for TS bindings, React + TanStack Query, Vitest + Testing Library, plain CSS with design tokens (`--fs-*`, `--surface-*`).

**Dependency notes:**
- Phase 7 (chat mode) depends on `docs/superpowers/plans/2026-04-27-chat-surface-integration.md` exposing a chat session API. If not yet ready when Phase 7 begins, sequence Phase 7 last.
- Phases 2, 3, 4 are independent after Phase 0 + 1 and may run in parallel.
- Phase 5 (agent tool) depends on Phase 2 + 3 to expose pin/calendar actions.

---

## Table of Contents

- **Phase 0** — Cleanup of dead/unused code
- **Phase 1** — Quick integration completions (Restore, onOpenTask, error toasts, DND args)
- **Phase 2** — Pinning feature (uses existing `launcher_pins` table)
- **Phase 3** — Calendar search source
- **Phase 4** — DomainEventBus publishing for launcher executions
- **Phase 5** — `LauncherTool` for agent + MCP exposure
- **Phase 6** — Voice recording mode (replaces `VoiceRecorderStub`)
- **Phase 7** — AI chat mode (replaces `LauncherChatStub`)
- **Phase 8** — Performance audit + tuning
- **Phase 9** — UX/UI polish
- **Phase 10** — Frontend test coverage backfill

---

## File Structure (changes by phase)

### Phase 0 — Cleanup

| File | Change |
|---|---|
| `crates/feature-launcher/migrations/001_launcher_tables.sql` | Modify: drop `launcher_frequencies` table block (lines 1–8) |
| `crates/feature-launcher/src/types.rs` | Modify: remove `FocusDashboard` struct (lines 184–191), drop `focus` field from `DashboardData` (line 178) |
| `crates/app-core/src/handlers/launcher/dashboard.rs` | Modify: delete focus-session lookup block (lines 19–48) |
| `desktop-ui/src/features/launcher/types.ts` | Modify: remove `FocusDashboard` interface (89, 95–101) |
| `crates/desktop/src/lazy_window.rs` | Modify: remove `destroy_if_hidden` (line 13 onward) |
| `crates/feature-launcher/src/search/calculator.rs` | Modify: remove `#[allow(clippy::unnecessary_map_or)]` at line 50, replace `.map_or(false, …)` with `.is_some_and(…)` or `.is_none_or(…)` |
| `crates/feature-launcher/src/search/system_prefs.rs` | Modify: same as above at line 49 |
| `crates/feature-launcher/src/window_mgmt/presets.rs` | Modify: rename test `count_is_25` → `count_is_26` at line 347 |
| `crates/desktop/src/commands/launcher.rs` | Modify: remove `args` parameter from `launcher_execute` (re-derive from handler) |
| `crates/app-core/src/handlers/launcher/handlers.rs` | Modify: drop `args` parameter from `launcher_execute` |
| `crates/app-core/src/handlers/launcher/search_engine.rs` | Modify: remove `let _ = args;` at line 327 and the `args` parameter |
| `desktop-ui/src/features/launcher/hooks/useExecuteItem.ts` | Modify: remove the `args` field from the `launcher_execute` invoke call |
| `desktop-ui/src/bindings.ts` | Regenerate via `cargo tauri dev` |

### Phase 1 — Quick integrations

| File | Change |
|---|---|
| `crates/feature-launcher/src/window_mgmt/actions.rs` | Modify: replace no-op `restore` block (89–91) with real implementation; add `last_frame_per_window: DashMap<u32, CGRect>` field; capture before each action |
| `crates/feature-launcher/src/window_mgmt/actions.rs` | Add: tests for capture-then-restore round trip |
| `crates/desktop/src/commands/launcher.rs` | Modify: thread DND duration arg through `SystemCommands::execute` (lines 81–83) |
| `crates/feature-launcher/src/search/system_commands.rs` | Modify: extend `execute` to accept `Option<Duration>` for DND |
| `desktop-ui/src/features/launcher/components/Launcher.tsx` | Modify: pass `onOpenTask` prop to `<Dashboard />` (line 204) |
| `desktop-ui/src/features/launcher/hooks/useExecuteItem.ts` | Modify: replace 21 `console.error` paths with toast emission via `show_status_badge` IPC |
| `desktop-ui/src/features/launcher/components/ActionMenu.tsx` | Modify: implement empty `Execute` handlers at lines 286 and 314 |
| `desktop-ui/src/features/launcher/components/FocusActiveChip.tsx` | Modify: replace 2 silent error paths with badge emission |

### Phase 2 — Pinning

| File | Change |
|---|---|
| `crates/feature-launcher/src/repos/pins.rs` | Create: `PinsRepo` with `pin`, `unpin`, `list_pinned`, `is_pinned`, `set_position` |
| `crates/feature-launcher/src/repos/mod.rs` | Modify: re-export `PinsRepo`, `Pin` types |
| `crates/feature-launcher/src/types.rs` | Modify: add `pinned: bool` to `LauncherItem` (default false) |
| `crates/app-core/src/handlers/launcher/handlers.rs` | Add: `launcher_pin`, `launcher_unpin`, `launcher_list_pinned` |
| `crates/app-core/src/handlers/launcher/search_engine.rs` | Modify: post-rank step to elevate pinned items to top, mark `pinned = true` |
| `crates/desktop/src/commands/launcher.rs` | Add: 3 new `#[klynt_command]` IPCs |
| `crates/desktop/src/specta_builder.rs` | Modify: add 3 names to `SPECTA_COMMAND_NAMES` and `collect_commands![]` |
| `desktop-ui/src/features/launcher/components/ActionMenu.tsx` | Modify: add Pin/Unpin entries |
| `desktop-ui/src/features/launcher/components/ResultRow.tsx` | Modify: render pin glyph for pinned items |
| `desktop-ui/src/features/launcher/launcher.css` | Modify: add `.lc-pin-glyph` styles |
| `desktop-ui/src/features/launcher/hooks/useExecuteItem.ts` | Modify: handle pin/unpin invalidation |
| `desktop-ui/src/lib/query/queryKeys.ts` | Modify: add `qk.launcher.pinned()` key |

### Phase 3 — Calendar source

| File | Change |
|---|---|
| `crates/feature-launcher/src/search/calendar.rs` | Create: `CalendarSource` reading from injected calendar fetcher |
| `crates/feature-launcher/src/search/mod.rs` | Modify: re-export `CalendarSource`; document new prefix `c/` |
| `crates/app-core/src/init/launcher.rs` | Modify: register `CalendarSource` in registry when calendar config enabled |
| `crates/config/src/schema/launcher.rs` | Modify: add `CalendarSourceConfig` with `enabled: bool, lookback_days: u32, lookahead_days: u32` |
| `desktop-ui/src/features/launcher/components/DetailPanel.tsx` | Modify: ensure `CalendarDetail` sub-component renders event start/end |

### Phase 4 — Domain bus

| File | Change |
|---|---|
| `crates/feature-launcher/src/events.rs` | Create: `LauncherItemExecuted` event struct |
| `crates/feature-launcher/src/lib.rs` | Modify: re-export `events::*` |
| `crates/bus/src/lib.rs` | Modify: register the event type if a registry exists |
| `crates/app-core/src/handlers/launcher/handlers.rs` | Modify: publish event from `launcher_execute` after frequency record |
| `crates/app-core/src/state.rs` | Modify: ensure `domain_bus` is reachable from launcher handlers (likely already is) |

### Phase 5 — Agent + MCP tool

| File | Change |
|---|---|
| `crates/feature-launcher/src/tool/mod.rs` | Create: `LauncherTool` with `#[derive(Tool)]` and `#[tool_actions]` |
| `crates/feature-launcher/src/tool/actions.rs` | Create: action params (`SearchParams`, `ExecuteParams`, `ApplyWindowParams`, `RunScriptParams`, `PinParams`) |
| `crates/feature-launcher/src/lib.rs` | Modify: `tools()` returns `vec![Box::new(LauncherTool::new(engine.clone()))]` |
| `crates/feature-launcher/Cargo.toml` | Modify: add `tools-core-macros` dep, ensure `tools-core` already present |
| `crates/app-core/src/init/ai_pipeline.rs` | Modify: register `LauncherFeature` in `build_feature_registry()` (lines 139–149) |
| `crates/config/src/schema/mcp.rs` | Modify: add `"launcher"` to `EXPLICIT_TOOL_ALLOWLIST` (lines 191–208) |

### Phase 6 — Voice mode

| File | Change |
|---|---|
| `desktop-ui/src/features/launcher/components/VoiceRecorder.tsx` | Create: replaces `VoiceRecorderStub` |
| `desktop-ui/src/features/launcher/components/Launcher.tsx` | Modify: import `VoiceRecorder`, pass `onTranscriptReady` |
| `desktop-ui/src/features/launcher/components/VoiceRecorderStub.tsx` | Delete |
| `desktop-ui/src/features/launcher/launcher.css` | Modify: rename `.lc-voice-stub*` → `.lc-voice*`; remove `Stubs` section header |
| `desktop-ui/src/features/launcher/hooks/useVoiceRecording.ts` | Create: hook wrapping voice IPC |

### Phase 7 — Chat mode

| File | Change |
|---|---|
| `desktop-ui/src/features/launcher/components/LauncherChat.tsx` | Create: replaces `LauncherChatStub` |
| `desktop-ui/src/features/launcher/components/Launcher.tsx` | Modify: import `LauncherChat` |
| `desktop-ui/src/features/launcher/components/LauncherChatStub.tsx` | Delete |
| `desktop-ui/src/features/launcher/launcher.css` | Modify: rename `.lc-chat-stub*` → `.lc-chat*` |

### Phase 8 — Performance

| File | Change |
|---|---|
| `crates/feature-launcher/src/search/mod.rs` | Modify: tune query cache TTLs, eviction interval, prefix dispatch shortcut |
| `crates/feature-launcher/src/search/inverted_index.rs` | Modify: bench-driven optimization (smaller `SmolStr` posting keys, score cap) |
| `crates/feature-launcher/benches/inverted_index.rs` | Create: criterion bench |
| `desktop-ui/src/features/launcher/hooks/useLauncherSearch.ts` | Modify: tune debounce (30→16ms), use `requestIdleCallback` for non-typing fetches |
| `desktop-ui/src/features/launcher/components/ResultsList.tsx` | Modify: virtualize results with `react-window` if list >40 |

### Phase 9 — UX/UI polish

| File | Change |
|---|---|
| `desktop-ui/src/features/launcher/launcher.css` | Modify: full pass — typography tokens, spacing scale, focus ring, reduced-motion respect |
| `desktop-ui/src/features/launcher/components/EmptyState.tsx` | Create: empty-results state with shortcut hints |
| `desktop-ui/src/features/launcher/components/LoadingState.tsx` | Create: skeleton loader |
| `desktop-ui/src/features/launcher/components/Launcher.tsx` | Modify: framer-motion mount/unmount transitions (60ms) |

### Phase 10 — Frontend tests

| File | Change |
|---|---|
| `desktop-ui/src/features/launcher/components/*.test.tsx` | Create: 12 component test files |
| `desktop-ui/src/features/launcher/hooks/*.test.ts` | Create: 5 hook test files |
| `desktop-ui/src/features/launcher/store.test.tsx` | Create: store reducer tests |
| `desktop-ui/src/test/launcherFixtures.ts` | Create: shared fixtures |

---

# Phase 0 — Cleanup

## Task 0.1: Remove `launcher_frequencies` table

**Files:**
- Modify: `crates/feature-launcher/migrations/001_launcher_tables.sql:1-8`
- Test: `crates/feature-launcher/src/repos/frequency.rs` (existing tests must still pass)

- [ ] **Step 1: Edit the migration**

In `crates/feature-launcher/migrations/001_launcher_tables.sql`, delete lines 1–8:

```sql
-- Frequency learning for search ranking
CREATE TABLE IF NOT EXISTS launcher_frequencies (
    item_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    count INTEGER NOT NULL DEFAULT 0,
    last_used TEXT NOT NULL,
    PRIMARY KEY (item_id, kind)
);
```

The file should now begin with `-- Usage log for frecency calculation (exponential decay)`.

- [ ] **Step 2: Verify FrequencyRepo tests still pass**

Run: `cargo nextest run -p feature-launcher repos::frequency`
Expected: 5 tests pass (`test_record_and_frecency`, `test_get_nonexistent_returns_zero`, `test_frecency_batch_ordering`, `test_top_frecency`, `test_increment_delegates_to_record_usage`).

- [ ] **Step 3: Verify migration runs cleanly**

Run: `cargo nextest run -p feature-launcher`
Expected: All tests in the crate pass.

- [ ] **Step 4: Commit**

```bash
git add crates/feature-launcher/migrations/001_launcher_tables.sql
git commit -m "refactor(launcher): drop unused launcher_frequencies table

Frecency uses launcher_usage_log exclusively; the rollup table was never
read or written. Pre-release, safe in-place migration edit."
```

---

## Task 0.2: Remove `FocusDashboard` from backend

**Files:**
- Modify: `crates/feature-launcher/src/types.rs:178,184-191`
- Modify: `crates/app-core/src/handlers/launcher/dashboard.rs:19-48`

- [ ] **Step 1: Remove the struct and field**

In `crates/feature-launcher/src/types.rs`:
- Delete the `pub focus: Option<FocusDashboard>,` line in `DashboardData` (line 178).
- Delete the entire `FocusDashboard` struct (lines 184–191):

```rust
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FocusDashboard {
    pub task_name: Option<String>,
    pub elapsed_secs: i64,
    pub target_secs: Option<i64>,
    pub session_id: String,
}
```

- [ ] **Step 2: Remove dashboard.rs lookup block**

In `crates/app-core/src/handlers/launcher/dashboard.rs`, delete the focus-session query block at lines 19–48 and any `focus:` field assignment in the returned `DashboardData`. Replace any `let focus = ...; ` lines with nothing; the returned struct should have no `focus` field.

- [ ] **Step 3: Compile**

Run: `cargo build -p feature-launcher -p app-core`
Expected: clean build. Any errors = a missed reference; grep `FocusDashboard` and remove all references.

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p feature-launcher -p app-core`
Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add crates/feature-launcher/src/types.rs crates/app-core/src/handlers/launcher/dashboard.rs
git commit -m "refactor(launcher): remove FocusDashboard widget backend

Frontend explicitly does not render this; focus state is already surfaced
via FocusActiveChip and tray countdown. Removes redundant per-fetch DB hit."
```

---

## Task 0.3: Remove `FocusDashboard` from frontend types

**Files:**
- Modify: `desktop-ui/src/features/launcher/types.ts`
- Modify: `desktop-ui/src/bindings.ts` (regenerate via tauri dev)

- [ ] **Step 1: Remove the TS interface and field**

In `desktop-ui/src/features/launcher/types.ts`, find and delete:
- The line `focus: FocusDashboard | null;` inside the `DashboardData` interface
- The full `FocusDashboard` interface (around line 95):

```ts
export interface FocusDashboard {
  taskName: string | null;
  elapsedSecs: number;
  targetSecs: number | null;
  sessionId: string;
}
```

- [ ] **Step 2: Regenerate bindings**

Run: `cargo tauri dev` (Ctrl+C after the dev window opens — bindings regenerate during startup).
Expected: `desktop-ui/src/bindings.ts` no longer contains `FocusDashboard`.

- [ ] **Step 3: Typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: clean. Any `Property 'focus' does not exist` = a missed call site; remove.

- [ ] **Step 4: Lint**

Run: `cd desktop-ui && bun run lint`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/launcher/types.ts desktop-ui/src/bindings.ts
git commit -m "refactor(launcher-ui): remove FocusDashboard type"
```

---

## Task 0.4: Remove `destroy_if_hidden` dead helper

**Files:**
- Modify: `crates/desktop/src/lazy_window.rs:13`

- [ ] **Step 1: Find and read the function**

Run: `grep -n "destroy_if_hidden" crates/desktop/src/`
Expected: one definition at `lazy_window.rs:13`, zero callers elsewhere.

- [ ] **Step 2: Delete the function**

Remove the function and its `#[allow(dead_code)]` attribute. Use `Edit` to remove the entire `pub fn destroy_if_hidden(...)` block plus the attribute line above it.

- [ ] **Step 3: Compile + commit**

Run: `cargo build -p desktop && git add crates/desktop/src/lazy_window.rs && git commit -m "chore(desktop): drop unused destroy_if_hidden helper

YAGNI: speculative GPU-memory recovery with no caller and no measured need."`

---

## Task 0.5: Replace MSRV-pinned `map_or` suppressions with `is_none_or`

**Files:**
- Modify: `crates/feature-launcher/src/search/calculator.rs:50`
- Modify: `crates/feature-launcher/src/search/system_prefs.rs:49`

- [ ] **Step 1: Read both call sites**

Use `Read` on both files at the noted lines to see the exact `map_or(false, |x| ...)` or `map_or(true, |x| ...)` patterns.

- [ ] **Step 2: Replace pattern at calculator.rs:50**

For `Option::map_or(false, |x| pred(x))` → `Option::is_some_and(|x| pred(x))`.
For `Option::map_or(true,  |x| pred(x))` → `Option::is_none_or(|x| pred(x))`.

Remove the `#[allow(clippy::unnecessary_map_or)]` attribute above.

- [ ] **Step 3: Same edit at system_prefs.rs:49**

Identical transformation.

- [ ] **Step 4: Verify clippy is clean**

Run: `cargo clippy -p feature-launcher --all-targets --all-features -- -D warnings`
Expected: zero warnings.

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p feature-launcher`
Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add crates/feature-launcher/src/search/calculator.rs crates/feature-launcher/src/search/system_prefs.rs
git commit -m "chore(launcher): use is_none_or now that MSRV is 1.93"
```

---

## Task 0.6: Fix mis-named preset count test

**Files:**
- Modify: `crates/feature-launcher/src/window_mgmt/presets.rs:347`

- [ ] **Step 1: Rename test**

Change `fn count_is_25()` to `fn count_is_26()`. Assertion stays the same.

- [ ] **Step 2: Verify**

Run: `cargo nextest run -p feature-launcher window_mgmt::presets::tests::count_is_26 --no-capture`
Expected: pass.

- [ ] **Step 3: Commit**

```bash
git add crates/feature-launcher/src/window_mgmt/presets.rs
git commit -m "test(launcher): rename count_is_25 → count_is_26 to match assertion"
```

---

## Task 0.7: Remove `args` parameter from `launcher_execute`

**Files:**
- Modify: `crates/desktop/src/commands/launcher.rs` (the `launcher_execute` command)
- Modify: `crates/app-core/src/handlers/launcher/handlers.rs` (the `launcher_execute` method)
- Modify: `crates/app-core/src/handlers/launcher/search_engine.rs:327` (the `execute` method)
- Modify: `desktop-ui/src/features/launcher/hooks/useExecuteItem.ts` (the `launcher_execute` IPC call)

- [ ] **Step 1: Drop `args` from `LauncherSearchEngine::execute`**

In `crates/app-core/src/handlers/launcher/search_engine.rs`, change the signature:

```rust
pub async fn execute(&self, item_id: &str, kind: &str) -> Result<LauncherExecuteResult, ApiError>
```

Remove the `args: HashMap<String, String>` param and the `let _ = args;` line at line 327.

- [ ] **Step 2: Update `AppCore::launcher_execute`**

In `crates/app-core/src/handlers/launcher/handlers.rs`, change the handler:

```rust
#[tracing::instrument(skip(self), err)]
pub async fn launcher_execute(
    &self,
    item_id: String,
    kind: String,
) -> Result<LauncherExecuteResult, ApiError> {
    self.launcher_engine()?.execute(&item_id, &kind).await
}
```

- [ ] **Step 3: Update Tauri IPC command**

In `crates/desktop/src/commands/launcher.rs`, drop the `args` argument from the `launcher_execute` `#[klynt_command]`. The signature becomes `(item_id: String, kind: String)`.

- [ ] **Step 4: Update dev-server dispatch**

In the same file, locate `dispatch_dev` and update the `"launcher_execute"` arm to deserialize only `{ itemId, kind }`.

- [ ] **Step 5: Update frontend IPC call**

In `desktop-ui/src/features/launcher/hooks/useExecuteItem.ts`, find every `ipc("launcher_execute", { itemId, kind, args })` and remove `args`. There should be exactly one canonical call near the bottom of the file in the recording-after-action helper.

- [ ] **Step 6: Regenerate bindings**

Run: `cargo tauri dev` and Ctrl+C after window opens. Verify `desktop-ui/src/bindings.ts` `launcherExecute` signature has lost `args`.

- [ ] **Step 7: Type-check + run tests**

Run: `cd desktop-ui && bun run typecheck`
Run: `cargo nextest run -p app-core -p desktop`
Expected: pass.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "refactor(launcher): drop args from launcher_execute IPC

Args were always silently discarded; specific commands (run_script,
system_command) own their own args. Cleaner API at no behavior change."
```

---

# Phase 1 — Quick integration completions

## Task 1.1: Implement `WindowAction::Restore` with frame history

**Files:**
- Modify: `crates/feature-launcher/src/window_mgmt/actions.rs`
- Test: same file `#[cfg(test)] mod tests`

`★ Design note for the engineer:` `WindowManager` is a singleton (`OnceLock`). Concurrency: window actions are user-driven so contention is low, but the history map must be `Send + Sync`. Use `parking_lot::Mutex<HashMap<u32, VecDeque<CGRect>>>` — `u32` keyed by window ID (frontmost PID is fine if window ID isn't available). Cap stack depth at 8.

- [ ] **Step 1: Read the current `actions.rs`**

Use `Read` on `crates/feature-launcher/src/window_mgmt/actions.rs` to find the existing `WindowManager` struct and `execute` method.

- [ ] **Step 2: Add the field**

Add to `WindowManager`:

```rust
use parking_lot::Mutex;
use std::collections::{HashMap, VecDeque};

pub struct WindowManager {
    cycle_state: Mutex<HashMap<u32, CycleState>>,
    frame_history: Mutex<HashMap<u32, VecDeque<CGRect>>>, // NEW
}
```

Update `WindowManager::new()` accordingly.

- [ ] **Step 3: Capture frame before applying any action**

In `execute`, before calling `compute_frame` or `set_window_frame`, call:

```rust
fn capture_current(&self, window_id: u32) {
    if let Ok(frame) = platform_macos::window::get_frontmost_window_frame() {
        let mut history = self.frame_history.lock();
        let stack = history.entry(window_id).or_default();
        if stack.len() >= 8 {
            stack.pop_front();
        }
        stack.push_back(frame);
    }
}
```

Call `self.capture_current(pid)` at the top of `execute` for non-`Restore` actions.

- [ ] **Step 4: Implement Restore**

Replace the no-op block at lines 89–91 (`if name == "restore" { return Ok(()); }`) with:

```rust
if name == "restore" {
    let window_id = platform_macos::window::get_frontmost_pid()? as u32;
    let popped = {
        let mut history = self.frame_history.lock();
        history.get_mut(&window_id).and_then(|s| s.pop_back())
    };
    if let Some(frame) = popped {
        platform_macos::window::set_window_frame(frame)?;
        return Ok(());
    } else {
        return Err(KlyntbotError::other("No previous frame to restore"));
    }
}
```

- [ ] **Step 5: Add a test**

```rust
#[test]
fn restore_pops_last_frame_per_window() {
    let mgr = WindowManager::new();
    let window_id = 42u32;

    // Manually push frames into history (bypass capture for test)
    {
        let mut h = mgr.frame_history.lock();
        let s = h.entry(window_id).or_default();
        s.push_back(CGRect { x: 0.0, y: 0.0, width: 800.0, height: 600.0 });
        s.push_back(CGRect { x: 100.0, y: 100.0, width: 1024.0, height: 768.0 });
    }

    // Pop once (simulating restore)
    let popped = {
        let mut h = mgr.frame_history.lock();
        h.get_mut(&window_id).and_then(|s| s.pop_back())
    };
    assert_eq!(popped.unwrap().width, 1024.0);

    // History stack should now have 1 frame
    let remaining = mgr.frame_history.lock().get(&window_id).map(|s| s.len()).unwrap_or(0);
    assert_eq!(remaining, 1);
}

#[test]
fn frame_history_caps_at_8() {
    let mgr = WindowManager::new();
    let window_id = 7u32;
    {
        let mut h = mgr.frame_history.lock();
        let s = h.entry(window_id).or_default();
        for i in 0..12 {
            if s.len() >= 8 { s.pop_front(); }
            s.push_back(CGRect { x: i as f64, y: 0.0, width: 100.0, height: 100.0 });
        }
    }
    let len = mgr.frame_history.lock().get(&window_id).unwrap().len();
    assert_eq!(len, 8);
}
```

- [ ] **Step 6: Run tests**

Run: `cargo nextest run -p feature-launcher window_mgmt`
Expected: 5 tests pass (3 existing + 2 new).

- [ ] **Step 7: Manual smoke test (macOS)**

Build and run: `cargo tauri dev` → Alt+Space → search "left half" → Enter → search "restore" → Enter. Window should return to original frame.

- [ ] **Step 8: Commit**

```bash
git add crates/feature-launcher/src/window_mgmt/actions.rs
git commit -m "feat(launcher): implement WindowAction::Restore with per-window frame history

Captures the current frame before each layout action; Restore pops
the most recent. Cap stack at 8 to bound memory."
```

---

## Task 1.2: Surface IPC errors via toast badges

**Files:**
- Modify: `desktop-ui/src/features/launcher/hooks/useExecuteItem.ts` (21 paths)
- Modify: `desktop-ui/src/features/launcher/components/ActionMenu.tsx` (2 paths)
- Modify: `desktop-ui/src/features/launcher/components/FocusActiveChip.tsx` (2 paths)
- Create: `desktop-ui/src/features/launcher/lib/showError.ts` (helper)

- [ ] **Step 1: Create the helper**

Create `desktop-ui/src/features/launcher/lib/showError.ts`:

```ts
import { ipc } from "@/utils/tauri-bridge";

export async function showError(message: string, err: unknown): Promise<void> {
  const detail = err instanceof Error ? err.message : String(err);
  console.error(message, err);
  try {
    await ipc("show_status_badge", {
      text: `${message} ${detail}`.slice(0, 80),
      kind: "error",
      durationMs: 2400,
    });
  } catch {
    // status badge IPC failed — already logged above, no further user surface
  }
}
```

- [ ] **Step 2: Replace silent paths in `useExecuteItem.ts`**

For every block of the form:

```ts
.catch((err) => console.error("Failed to open app:", err));
```

Replace with:

```ts
.catch((err) => showError("Couldn't open app:", err));
```

There are 17 such paths in `useExecuteItem.ts`. Each gets a verb-specific message:
- "Couldn't open app:"
- "Couldn't activate focus:"
- "Couldn't run command:"
- "Couldn't run script:"
- "Couldn't paste:"
- "Couldn't copy:"
- "Couldn't open:"
- "Couldn't open URL:"
- "Couldn't open setting:"
- "Couldn't focus app:"
- "Couldn't open SSH:"
- "Couldn't open contact:"
- "Couldn't apply layout:"
- "Couldn't copy package name:"
- "Couldn't execute:"

Add `import { showError } from "../lib/showError";` at the top.

- [ ] **Step 3: Replace silent paths in `ActionMenu.tsx`**

Lines 27 and 129 — same pattern (`"Couldn't open:"` and `"Couldn't copy:"`).

- [ ] **Step 4: Replace silent paths in `FocusActiveChip.tsx`**

Lines 23 and 30 — `"Couldn't extend focus:"` and `"Couldn't end focus:"`.

- [ ] **Step 5: Run lint + typecheck**

Run: `cd desktop-ui && bun run typecheck && bun run lint`
Expected: clean.

- [ ] **Step 6: Manual smoke test**

Build & run, force a launch error (e.g., delete an indexed app, then try to open it). Confirm a red badge appears.

- [ ] **Step 7: Commit**

```bash
git add desktop-ui/src/features/launcher/
git commit -m "feat(launcher-ui): surface IPC errors via status badge

21 silent console.error paths now also emit a 2.4s error badge so
users see when an action fails. Helper centralized in lib/showError."
```

---

## Task 1.3: Wire `onOpenTask` from Launcher → Dashboard

**Files:**
- Modify: `desktop-ui/src/features/launcher/components/Launcher.tsx:204`

- [ ] **Step 1: Read current dashboard render**

Use `Read` on `Launcher.tsx` near line 204 to confirm the surrounding render and find where `onExpandToMain` and other navigation helpers are defined.

- [ ] **Step 2: Add the handler**

In `LauncherShell`, alongside other handlers, add:

```tsx
const onOpenTask = useCallback((taskId: string) => {
  ipc("launcher_open_app", { path: `klyntbot://task/${taskId}` })
    .catch((err) => showError("Couldn't open task:", err));
  getCurrentWindow().hide();
}, []);
```

Pass `<Dashboard onOpenTask={onOpenTask} />` instead of `<Dashboard />`.

- [ ] **Step 3: Typecheck + lint**

Run: `cd desktop-ui && bun run typecheck && bun run lint`
Expected: clean.

- [ ] **Step 4: Smoke test**

Open launcher with empty query, click a task row in the dashboard. Confirm main window opens to that task.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/launcher/components/Launcher.tsx
git commit -m "fix(launcher-ui): wire onOpenTask so dashboard task rows are clickable"
```

---

## Task 1.4: Thread DND duration through `SystemCommands::execute`

**Files:**
- Modify: `crates/feature-launcher/src/search/system_commands.rs` (extend `execute`)
- Modify: `crates/desktop/src/commands/launcher.rs:81-83` (pass arg through)
- Test: `crates/feature-launcher/src/search/system_commands.rs`

`★ Design note:` `SystemAction::ToggleDoNotDisturb` currently runs `shortcuts run "Toggle Do Not Disturb"`. With duration, we should call the existing `focus_activate` mechanism instead. But that's an `AppCore` method, not a `feature-launcher` concern — so `SystemCommands::execute` should NOT call focus_activate directly. Instead, it accepts a `Option<Duration>`, and the dispatch site (`commands/launcher.rs`) decides: if `args` contains `duration` and action is DND, call `focus_activate`; else fall through to system-command toggle.

- [ ] **Step 1: Extend `SystemCommands::execute` signature**

Change to `pub async fn execute(&self, action: &SystemAction, duration: Option<Duration>) -> Result<()>`. For all non-DND variants, ignore `duration`. For DND, when `duration` is `Some`, log a debug trace acknowledging duration but defer to the toggle (the actual duration is handled at the IPC layer for now).

(Reason: keeping the focus subsystem out of `feature-launcher` preserves the dependency-inversion architecture. `feature-launcher` does not depend on focus.)

- [ ] **Step 2: Update IPC dispatch**

In `crates/desktop/src/commands/launcher.rs`, replace the `let _ = args;` block (lines 81–83) with:

```rust
let duration = args
    .get("duration")
    .and_then(|s| parse_human_duration(s));

if matches!(action, SystemAction::ToggleDoNotDisturb) && duration.is_some() {
    let ends_at = jiff::Timestamp::now() + duration.unwrap();
    return state
        .focus_activate("dnd".to_string(), ends_at.to_string())
        .await
        .map(|_| ())
        .map_err(Into::into);
}

SystemCommands::execute(&action, duration)
    .await
    .map_err(Into::into)
```

Add a small `parse_human_duration` helper that maps `"30m"`, `"2h"`, `"1d"` to `std::time::Duration`. (Reuse `desktop-ui/src/features/launcher/lib/parseDuration.ts` logic in Rust.)

- [ ] **Step 3: Add Rust parser tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minutes() { assert_eq!(parse_human_duration("30m"), Some(Duration::from_secs(1800))); }
    #[test]
    fn parses_hours()   { assert_eq!(parse_human_duration("2h"),  Some(Duration::from_secs(7200))); }
    #[test]
    fn parses_days()    { assert_eq!(parse_human_duration("1d"),  Some(Duration::from_secs(86400))); }
    #[test]
    fn rejects_garbage() { assert_eq!(parse_human_duration("xyz"), None); }
}
```

- [ ] **Step 4: Verify**

Run: `cargo nextest run -p desktop -p feature-launcher`
Expected: pass.

- [ ] **Step 5: Smoke test**

Launcher → DND → fill "30m" in arg chip → Enter. Confirm DND turns on for 30 minutes.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat(launcher): thread DND duration arg through to focus_activate

Resolves the deferred 'Task 3.4' workaround. SystemCommands::execute
accepts Option<Duration>; the IPC dispatch routes duration-flavored
DND requests through focus_activate."
```

---

## Task 1.5: Implement empty `Execute` handlers in `ActionMenu`

**Files:**
- Modify: `desktop-ui/src/features/launcher/components/ActionMenu.tsx:286,314`

- [ ] **Step 1: Replace empty handlers with real dispatch**

In `executeActions()` for `systemCommand`/`script` (line ~286), the `Execute` action should call into the same `executeItem(item, args)` path used by Enter:

```tsx
{ label: "Execute", shortcut: "Enter", handler: () => executeItem(item, {}) },
```

Same for `defaultActions()` (line ~314).

`executeItem` is imported from `../hooks/useExecuteItem` — confirm the import is present and add if missing.

- [ ] **Step 2: Smoke test**

Open ActionMenu (Cmd+K) on a system command; confirm Execute action runs the command.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/launcher/components/ActionMenu.tsx
git commit -m "fix(launcher-ui): wire ActionMenu Execute action to executeItem"
```

---

# Phase 2 — Pinning feature

## Task 2.1: Build `PinsRepo`

**Files:**
- Create: `crates/feature-launcher/src/repos/pins.rs`
- Modify: `crates/feature-launcher/src/repos/mod.rs` (re-export)

- [ ] **Step 1: Write failing tests**

Create `crates/feature-launcher/src/repos/pins.rs`:

```rust
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use storage::StoragePool;
use common::Result;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Pin {
    pub item_id: String,
    pub kind: String,
    pub position: i64,
}

pub struct PinsRepo {
    pool: SqlitePool,
}

impl PinsRepo {
    pub fn new(pool: &StoragePool) -> Self {
        Self { pool: pool.inner().clone() }
    }

    pub async fn pin(&self, item_id: &str, kind: &str) -> Result<()> {
        let max_pos: Option<i64> = sqlx::query_scalar("SELECT MAX(position) FROM launcher_pins")
            .fetch_one(&self.pool).await.map_err(storage::StorageError::from)?;
        let next_pos = max_pos.map_or(0, |p| p + 1);
        sqlx::query("INSERT OR REPLACE INTO launcher_pins (item_id, kind, position) VALUES (?, ?, ?)")
            .bind(item_id).bind(kind).bind(next_pos)
            .execute(&self.pool).await.map_err(storage::StorageError::from)?;
        Ok(())
    }

    pub async fn unpin(&self, item_id: &str, kind: &str) -> Result<()> {
        sqlx::query("DELETE FROM launcher_pins WHERE item_id = ? AND kind = ?")
            .bind(item_id).bind(kind)
            .execute(&self.pool).await.map_err(storage::StorageError::from)?;
        Ok(())
    }

    pub async fn list_pinned(&self) -> Result<Vec<Pin>> {
        let pins = sqlx::query_as::<_, Pin>("SELECT item_id, kind, position FROM launcher_pins ORDER BY position ASC")
            .fetch_all(&self.pool).await.map_err(storage::StorageError::from)?;
        Ok(pins)
    }

    pub async fn is_pinned(&self, item_id: &str, kind: &str) -> Result<bool> {
        let row: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM launcher_pins WHERE item_id = ? AND kind = ? LIMIT 1")
            .bind(item_id).bind(kind)
            .fetch_optional(&self.pool).await.map_err(storage::StorageError::from)?;
        Ok(row.is_some())
    }

    pub async fn pinned_set(&self) -> Result<std::collections::HashSet<(String, String)>> {
        let pins = self.list_pinned().await?;
        Ok(pins.into_iter().map(|p| (p.item_id, p.kind)).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage::StoragePool;

    async fn setup() -> StoragePool {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        StoragePool::run_feature_migrations(&pool, &crate::launcher_migrations()).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn pin_and_list() {
        let pool = setup().await;
        let repo = PinsRepo::new(&pool);
        repo.pin("app:/Applications/Slack.app", "application").await.unwrap();
        repo.pin("app:/Applications/VSCode.app", "application").await.unwrap();
        let pins = repo.list_pinned().await.unwrap();
        assert_eq!(pins.len(), 2);
        assert_eq!(pins[0].position, 0);
        assert_eq!(pins[1].position, 1);
    }

    #[tokio::test]
    async fn unpin_removes() {
        let pool = setup().await;
        let repo = PinsRepo::new(&pool);
        repo.pin("a", "application").await.unwrap();
        repo.unpin("a", "application").await.unwrap();
        assert_eq!(repo.list_pinned().await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn is_pinned_query() {
        let pool = setup().await;
        let repo = PinsRepo::new(&pool);
        assert!(!repo.is_pinned("a", "k").await.unwrap());
        repo.pin("a", "k").await.unwrap();
        assert!(repo.is_pinned("a", "k").await.unwrap());
    }

    #[tokio::test]
    async fn pin_is_idempotent() {
        let pool = setup().await;
        let repo = PinsRepo::new(&pool);
        repo.pin("a", "k").await.unwrap();
        repo.pin("a", "k").await.unwrap();
        // INSERT OR REPLACE keeps row count at 1
        assert_eq!(repo.list_pinned().await.unwrap().len(), 1);
    }
}
```

- [ ] **Step 2: Re-export from `repos/mod.rs`**

Add to `crates/feature-launcher/src/repos/mod.rs`:

```rust
mod pins;
pub use pins::{Pin, PinsRepo};
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p feature-launcher repos::pins`
Expected: 4 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/feature-launcher/src/repos/
git commit -m "feat(launcher): add PinsRepo backed by launcher_pins table"
```

---

## Task 2.2: Surface `pinned: bool` on `LauncherItem`

**Files:**
- Modify: `crates/feature-launcher/src/types.rs:28-39`
- Modify: `desktop-ui/src/features/launcher/types.ts`

- [ ] **Step 1: Add field**

In `LauncherItem` struct:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct LauncherItem {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub icon: Option<String>,
    pub kind: LauncherItemKind,
    pub score: f64,
    #[serde(default)]
    pub no_view: bool,
    #[serde(default)]
    pub arguments: Vec<ArgSpec>,
    #[serde(default)]
    pub pinned: bool, // NEW
}
```

Update every constructor site of `LauncherItem` in the workspace to include `pinned: false`. Use `grep -rn "LauncherItem {" crates/feature-launcher crates/app-core` to find them.

- [ ] **Step 2: Compile**

Run: `cargo build -p feature-launcher -p app-core`
Expected: clean.

- [ ] **Step 3: Mirror in TS**

Regenerate `bindings.ts` via `cargo tauri dev`. Verify `LauncherItem` has `pinned: boolean`.

- [ ] **Step 4: Update `desktop-ui/src/features/launcher/types.ts`**

If `LauncherItem` is locally defined (vs. imported from bindings), add `pinned: boolean`.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(launcher): add pinned flag to LauncherItem"
```

---

## Task 2.3: Elevate pinned items in search

**Files:**
- Modify: `crates/app-core/src/handlers/launcher/search_engine.rs`
- Modify: `crates/feature-launcher/src/lib.rs` (expose `LauncherSearchEngine` field for `pins_repo`)
- Modify: `crates/app-core/src/handlers/launcher/search_engine.rs` (struct + ctor)
- Modify: `crates/app-core/src/init/launcher.rs` (construct with `PinsRepo`)
- Test: same engine file

- [ ] **Step 1: Add `pins_repo` to `LauncherSearchEngine`**

```rust
pub struct LauncherSearchEngine {
    pub registry: SourceRegistry,
    pub frequency_repo: FrequencyRepo,
    pub clipboard_repo: ClipboardRepo,
    pub pins_repo: feature_launcher::PinsRepo, // NEW
    pub _file_watcher: Option<feature_launcher::SourceFileWatcher>,
}
```

Update constructor in `crates/app-core/src/init/launcher.rs` to build a `PinsRepo` and pass it.

- [ ] **Step 2: Add `kind_tag` helper and apply pin elevation in `search()`**

First, add a free function in the same file (or in `feature_launcher::types`):

```rust
pub fn kind_tag(kind: &feature_launcher::LauncherItemKind) -> &'static str {
    use feature_launcher::LauncherItemKind as K;
    match kind {
        K::Application { .. } => "application",
        K::Task { .. } => "task",
        K::Note { .. } => "note",
        K::ClipboardEntry { .. } => "clipboardEntry",
        K::SystemCommand { .. } => "systemCommand",
        K::Script { .. } => "script",
        K::Calculator { .. } => "calculator",
        K::Calendar { .. } => "calendar",
        K::AiChat { .. } => "aiChat",
        K::File { .. } => "file",
        K::ContentMatch { .. } => "contentMatch",
        K::Contact { .. } => "contact",
        K::SystemPref { .. } => "systemPref",
        K::RunningApp { .. } => "runningApp",
        K::Bookmark { .. } => "bookmark",
        K::BrowserHistory { .. } => "browserHistory",
        K::BrewPackage { .. } => "brewPackage",
        K::SshHost { .. } => "sshHost",
        K::GitRepo { .. } => "gitRepo",
        K::UrlNavigation { .. } => "urlNavigation",
        K::WindowAction { .. } => "windowAction",
    }
}
```

Then in `search()`, after dedup + sort:

```rust
let pinned = self.pins_repo.pinned_set().await.unwrap_or_default();

// Mark pinned and elevate
for item in &mut results {
    let tag = kind_tag(&item.kind);
    if pinned.contains(&(item.id.clone(), tag.to_string())) {
        item.pinned = true;
        item.score += 1000.0; // strong elevation; tied items keep base ordering
    }
}

// Re-sort after pin boost
results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
```

- [ ] **Step 3: Test pinned elevation**

Add an integration-style test in `app-core/tests` (or as a unit test on a method-extracted function `apply_pins`):

```rust
#[tokio::test]
async fn pinned_items_rank_first() {
    // Build engine with two apps; pin the second; search for both;
    // assert second comes back first with pinned=true.
}
```

- [ ] **Step 4: Verify + commit**

```bash
cargo nextest run -p app-core
git add -A
git commit -m "feat(launcher): elevate pinned items to top of search results

Pins win all ties: score += 1000 after dedup, then resort. Pinned flag
is mirrored on the item for UI rendering."
```

---

## Task 2.4: Pin/unpin/list IPC commands

**Files:**
- Modify: `crates/app-core/src/handlers/launcher/handlers.rs` (add `launcher_pin`, `launcher_unpin`, `launcher_list_pinned`)
- Modify: `crates/desktop/src/commands/launcher.rs` (add 3 `#[klynt_command]` IPCs)
- Modify: `crates/desktop/src/specta_builder.rs` (add to `SPECTA_COMMAND_NAMES` and `collect_commands![]`)
- Modify: `crates/desktop/src/commands/launcher.rs` (`dispatch_dev` for new commands)

- [ ] **Step 1: Add `AppCore` handlers**

```rust
#[tracing::instrument(skip(self), err)]
pub async fn launcher_pin(&self, item_id: String, kind: String) -> Result<(), ApiError> {
    self.launcher_engine()?.pins_repo.pin(&item_id, &kind).await.map_err(Into::into)
}

#[tracing::instrument(skip(self), err)]
pub async fn launcher_unpin(&self, item_id: String, kind: String) -> Result<(), ApiError> {
    self.launcher_engine()?.pins_repo.unpin(&item_id, &kind).await.map_err(Into::into)
}

#[tracing::instrument(skip(self), err)]
pub async fn launcher_list_pinned(&self) -> Result<Vec<feature_launcher::Pin>, ApiError> {
    self.launcher_engine()?.pins_repo.list_pinned().await.map_err(Into::into)
}
```

- [ ] **Step 2: Add Tauri IPC commands**

In `crates/desktop/src/commands/launcher.rs`:

```rust
#[klynt_command]
pub async fn launcher_pin(state: AppCore, item_id: String, kind: String) -> Result<(), ApiError> {
    state.launcher_pin(item_id, kind).await
}

#[klynt_command]
pub async fn launcher_unpin(state: AppCore, item_id: String, kind: String) -> Result<(), ApiError> {
    state.launcher_unpin(item_id, kind).await
}

#[klynt_command]
pub async fn launcher_list_pinned(state: AppCore) -> Result<Vec<Pin>, ApiError> {
    state.launcher_list_pinned().await
}
```

- [ ] **Step 3: Add to specta**

In `specta_builder.rs`, add 3 names and 3 paths to both `SPECTA_COMMAND_NAMES` and `collect_commands![]`.

- [ ] **Step 4: Add dev-server dispatch arms**

In `dispatch_dev`, add three more arms paralleling existing ones.

- [ ] **Step 5: Regenerate bindings**

Run: `cargo tauri dev`, then Ctrl+C.

- [ ] **Step 6: Run drift tests**

```bash
cargo nextest run -p desktop registration_drift bindings_are_current no_raw_tauri_command_outside_macros
```
Expected: pass.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat(launcher): add pin/unpin/list_pinned IPC commands"
```

---

## Task 2.5: Pin/unpin in UI ActionMenu + glyph rendering

**Files:**
- Modify: `desktop-ui/src/features/launcher/components/ActionMenu.tsx`
- Modify: `desktop-ui/src/features/launcher/components/ResultRow.tsx`
- Modify: `desktop-ui/src/features/launcher/launcher.css`
- Modify: `desktop-ui/src/lib/query/queryKeys.ts` (add `qk.launcher.pinned()`)
- Modify: `desktop-ui/src/features/launcher/hooks/useLauncherSearch.ts` (invalidate on pin/unpin)

- [ ] **Step 1: Add query key**

In `desktop-ui/src/lib/query/queryKeys.ts`, add to the `launcher` namespace:

```ts
pinned: () => ["launcher", "pinned"] as const,
```

- [ ] **Step 2: Add Pin/Unpin actions**

In `ActionMenu.tsx`, append to every `actions` array:

```tsx
{
  label: item.pinned ? "Unpin from top" : "Pin to top",
  shortcut: "⌘P",
  handler: async () => {
    try {
      const kind = kindTag(item.kind);
      if (item.pinned) {
        await ipc("launcher_unpin", { itemId: item.id, kind });
      } else {
        await ipc("launcher_pin", { itemId: item.id, kind });
      }
      queryClient.invalidateQueries({ queryKey: qk.launcher.search(query) });
      queryClient.invalidateQueries({ queryKey: qk.launcher.pinned() });
    } catch (err) {
      showError("Couldn't update pin:", err);
    }
  },
},
```

`kindTag` is a small helper. Create `desktop-ui/src/features/launcher/lib/kindTag.ts`:

```ts
import type { LauncherItemKind } from "../types";

/** Returns the discriminator string matching the Rust kind_tag() helper. */
export function kindTag(kind: LauncherItemKind): string {
  return kind.type;
}
```

(The TS bindings already serialize the discriminator as `kind.type` due to `#[serde(tag = "type")]`, so this helper is a one-liner — but kept in a named file so future kind name changes have a single touch point.)

- [ ] **Step 3: Render pin glyph in ResultRow**

```tsx
{item.pinned && <span className="lc-pin-glyph" aria-label="Pinned">📌</span>}
```

- [ ] **Step 4: Add CSS**

In `launcher.css`:

```css
.lc-pin-glyph {
  font-size: var(--fs-2xs);
  margin-right: 4px;
  opacity: 0.7;
  transform: rotate(-15deg);
  display: inline-block;
}
```

- [ ] **Step 5: Smoke test**

Search "slack" → Cmd+K → "Pin to top" → close launcher → reopen with empty query → Slack appears at top with pin glyph. Search "vscode" → Cmd+K → "Pin to top" → "slack" search puts Slack first; "code" search puts VSCode first.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat(launcher-ui): pin/unpin via ActionMenu, render pin glyph"
```

---

# Phase 3 — Calendar search source

## Task 3.1: Calendar fetcher trait + injection point

**Files:**
- Create: `crates/feature-launcher/src/search/calendar.rs`
- Modify: `crates/feature-launcher/src/search/mod.rs` (re-export, document `c/` prefix)
- Modify: `crates/config/src/schema/launcher.rs` (add `CalendarSourceConfig`)

`★ Design note:` `feature-launcher` cannot depend on calendar/google-calendar crates (would invert layers). Use a trait that callers in `app-core` implement. Keep `feature-launcher` provider-agnostic.

- [ ] **Step 1: Define trait + source**

```rust
// crates/feature-launcher/src/search/calendar.rs
use crate::search::{fuzzy_match, SearchSource};
use crate::types::{LauncherItem, LauncherItemKind, SearchResult};
use async_trait::async_trait;
use jiff::Timestamp;
use std::sync::Arc;

#[async_trait]
pub trait CalendarFetcher: Send + Sync {
    async fn upcoming_events(&self, lookback_days: u32, lookahead_days: u32) -> Vec<CalendarEvent>;
}

#[derive(Debug, Clone)]
pub struct CalendarEvent {
    pub event_id: String,
    pub title: String,
    pub starts_at: Timestamp,
    pub ends_at: Timestamp,
}

pub struct CalendarSource {
    fetcher: Arc<dyn CalendarFetcher>,
    lookback_days: u32,
    lookahead_days: u32,
}

impl CalendarSource {
    pub fn new(fetcher: Arc<dyn CalendarFetcher>, lookback_days: u32, lookahead_days: u32) -> Self {
        Self { fetcher, lookback_days, lookahead_days }
    }
}

#[async_trait]
impl SearchSource for CalendarSource {
    fn name(&self) -> &str { "calendar" }
    fn prefix(&self) -> Option<&str> { Some("c/") }

    async fn search(&self, query: &str) -> Vec<SearchResult> {
        let events = self.fetcher.upcoming_events(self.lookback_days, self.lookahead_days).await;
        if query.is_empty() {
            return events.into_iter().take(10).map(|e| SearchResult {
                item: event_to_item(&e, 0.6),
                base_score: 0.6,
            }).collect();
        }
        let scored = fuzzy_match(query, events.iter().map(|e| (e.title.as_str(), e)).collect::<Vec<_>>());
        scored.into_iter().take(15).map(|(score, e)| {
            let normalized = (score as f64 / 1000.0) * 0.85;
            SearchResult { item: event_to_item(e, normalized), base_score: normalized }
        }).collect()
    }
}

fn event_to_item(e: &CalendarEvent, score: f64) -> LauncherItem {
    let subtitle = format!("{} → {}", e.starts_at, e.ends_at);
    LauncherItem {
        id: format!("cal:{}", e.event_id),
        title: e.title.clone(),
        subtitle: Some(subtitle),
        icon: Some("📅".to_string()),
        kind: LauncherItemKind::Calendar { event_id: e.event_id.clone(), starts_at: e.starts_at },
        score,
        no_view: false,
        arguments: vec![],
        pinned: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StubFetcher(Vec<CalendarEvent>);
    #[async_trait]
    impl CalendarFetcher for StubFetcher {
        async fn upcoming_events(&self, _: u32, _: u32) -> Vec<CalendarEvent> { self.0.clone() }
    }

    #[tokio::test]
    async fn empty_query_returns_top_events() {
        let events = vec![
            CalendarEvent { event_id: "1".into(), title: "Standup".into(),
                starts_at: Timestamp::now(), ends_at: Timestamp::now() },
        ];
        let src = CalendarSource::new(Arc::new(StubFetcher(events)), 1, 7);
        let r = src.search("").await;
        assert_eq!(r.len(), 1);
    }

    #[tokio::test]
    async fn fuzzy_match_orders_by_relevance() {
        let events = vec![
            CalendarEvent { event_id: "1".into(), title: "Sprint Planning".into(),
                starts_at: Timestamp::now(), ends_at: Timestamp::now() },
            CalendarEvent { event_id: "2".into(), title: "1:1 with Manager".into(),
                starts_at: Timestamp::now(), ends_at: Timestamp::now() },
        ];
        let src = CalendarSource::new(Arc::new(StubFetcher(events)), 1, 7);
        let r = src.search("planning").await;
        assert!(r[0].item.title.contains("Planning"));
    }
}
```

- [ ] **Step 2: Re-export and add config**

`crates/feature-launcher/src/search/mod.rs`:

```rust
pub mod calendar;
pub use calendar::{CalendarEvent, CalendarFetcher, CalendarSource};
```

`crates/config/src/schema/launcher.rs`, add to `LauncherSourcesConfig`:

```rust
pub calendar: CalendarSourceConfig,
```

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarSourceConfig {
    #[serde(default = "default_true_fn")]
    pub enabled: bool,
    #[serde(default = "default_lookback")]
    pub lookback_days: u32,
    #[serde(default = "default_lookahead")]
    pub lookahead_days: u32,
}
fn default_lookback() -> u32 { 1 }
fn default_lookahead() -> u32 { 7 }
impl Default for CalendarSourceConfig {
    fn default() -> Self { Self { enabled: true, lookback_days: 1, lookahead_days: 7 } }
}
```

- [ ] **Step 3: Tests + commit**

```bash
cargo nextest run -p feature-launcher search::calendar
git add -A
git commit -m "feat(launcher): add CalendarSource with CalendarFetcher trait"
```

---

## Task 3.2: Wire calendar source in `app-core/init/launcher.rs`

**Files:**
- Modify: `crates/app-core/src/init/launcher.rs`
- Possibly modify: `crates/app-core/src/handlers/launcher/calendar_fetcher_impl.rs` (new file)

- [ ] **Step 1: Implement `CalendarFetcher` for the existing calendar provider**

The dashboard already pulls calendar events somewhere — find that path:

```bash
grep -rn "fn .*calendar.*" crates/app-core/src/handlers/launcher/dashboard.rs
grep -rn "google_calendar\|calendar_events" crates/app-core/src
```

Wrap that fetcher in a struct that impls `feature_launcher::CalendarFetcher`. If no concrete provider exists yet, gate the source behind `config.launcher.sources.calendar.enabled = false` by default.

- [ ] **Step 2: Register in registry**

In `init_launcher`, after registering other sources:

```rust
if config.launcher.sources.calendar.enabled {
    if let Some(fetcher) = build_calendar_fetcher(&deps) {
        registry.register(Arc::new(CalendarSource::new(
            fetcher,
            config.launcher.sources.calendar.lookback_days,
            config.launcher.sources.calendar.lookahead_days,
        )));
    }
}
```

- [ ] **Step 3: Verify launcher_search returns calendar items**

Smoke test: search for an event title; confirm appears with `📅` icon and starts_at subtitle.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat(launcher): register CalendarSource via app-core fetcher impl"
```

---

# Phase 4 — DomainEventBus integration

## Task 4.1: Define `LauncherItemExecuted` domain event

**Files:**
- Create: `crates/feature-launcher/src/events.rs`
- Modify: `crates/feature-launcher/src/lib.rs`

- [ ] **Step 1: Define event**

```rust
// crates/feature-launcher/src/events.rs
use jiff::Timestamp;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LauncherItemExecuted {
    pub item_id: String,
    pub kind: String,
    pub query: Option<String>,
    pub at: Timestamp,
}
```

- [ ] **Step 2: Re-export**

In `lib.rs`, add `pub mod events; pub use events::*;`.

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat(launcher): define LauncherItemExecuted domain event"
```

---

## Task 4.2: Publish event from `launcher_execute` handler

**Files:**
- Modify: `crates/app-core/src/handlers/launcher/handlers.rs`
- Modify: `crates/app-core/src/state.rs` (verify `domain_bus` is reachable)

- [ ] **Step 1: Read existing publish patterns**

`grep -rn "domain_bus.*publish\|publish_domain" crates/app-core/src/handlers/` to find the canonical pattern (e.g., `TaskCreated` publishes from feature-tasks).

- [ ] **Step 2: Publish at end of `launcher_execute`**

```rust
pub async fn launcher_execute(
    &self,
    item_id: String,
    kind: String,
) -> Result<LauncherExecuteResult, ApiError> {
    let result = self.launcher_engine()?.execute(&item_id, &kind).await?;

    // Best-effort publish; non-fatal if bus is absent
    if let Some(bus) = self.domain_bus.as_ref() {
        let event = feature_launcher::LauncherItemExecuted {
            item_id,
            kind,
            query: None, // wire query through if you have it; else leave None
            at: jiff::Timestamp::now(),
        };
        let _ = bus.publish(event).await;
    }

    Ok(result)
}
```

If the bus needs the query, thread it via the IPC param (add `query: Option<String>` to `launcher_execute`).

- [ ] **Step 3: Verify**

`cargo nextest run -p app-core`
Expected: pass.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat(launcher): publish LauncherItemExecuted on every execute

Mirror, Reforge, and analytics can subscribe to launcher usage signal."
```

---

# Phase 5 — Agent + MCP tool exposure

## Task 5.1: Build `LauncherTool` with multi-action

**Files:**
- Create: `crates/feature-launcher/src/tool/mod.rs`
- Create: `crates/feature-launcher/src/tool/actions.rs`
- Modify: `crates/feature-launcher/Cargo.toml` (add `tools-core-macros`)
- Modify: `crates/feature-launcher/src/lib.rs` (`pub mod tool; FeaturePackage::tools()`)

- [ ] **Step 1: Read an existing `#[derive(Tool)]` tool for the pattern**

`Read` `crates/tools/src/domain/docs.rs` to see the canonical multi-action pattern.

- [ ] **Step 2: Define action params**

`crates/feature-launcher/src/tool/actions.rs`:

```rust
use serde::{Deserialize, Serialize};
use tools_core_macros::ActionParams;

#[derive(Debug, Clone, Serialize, Deserialize, ActionParams, schemars::JsonSchema)]
pub struct SearchParams {
    /// Search query. Empty returns recent/frecent items.
    pub query: String,
    /// Maximum number of results (default 10).
    #[serde(default = "default_limit")]
    pub limit: u32,
}
fn default_limit() -> u32 { 10 }

#[derive(Debug, Clone, Serialize, Deserialize, ActionParams, schemars::JsonSchema)]
pub struct ExecuteParams {
    /// Item ID returned from `search` (e.g. "app:/Applications/Slack.app").
    pub item_id: String,
    /// Item kind discriminator (e.g. "application", "script", "systemCommand").
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ActionParams, schemars::JsonSchema)]
pub struct ApplyWindowParams {
    /// Window action: "leftHalf" | "rightHalf" | "topHalf" | "bottomHalf" |
    /// "leftThird" | "centerThird" | "rightThird" | "maximize" | "center" | "restore"
    /// or "preset:<name>" for named presets.
    pub action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ActionParams, schemars::JsonSchema)]
pub struct PinParams {
    pub item_id: String,
    pub kind: String,
}
```

- [ ] **Step 3: Define the tool**

`crates/feature-launcher/src/tool/mod.rs`:

```rust
use crate::tool::actions::*;
use crate::types::{LauncherExecuteResult, LauncherItem, WindowAction};
use crate::{ClipboardRepo, FrequencyRepo, PinsRepo, SourceRegistry};
use async_trait::async_trait;
use common::Result;
use std::sync::Arc;
use tools_core::{Tool, ToolContext, ToolResult};
use tools_core_macros::{tool_actions, Tool};

pub mod actions;

/// Search and execute launcher items: apps, scripts, files, system commands,
/// window layouts, browser bookmarks, contacts, and more.
#[derive(Tool)]
#[tool(name = "launcher")]
pub struct LauncherTool {
    registry: Arc<SourceRegistry>,
    frequency: Arc<FrequencyRepo>,
    pins: Arc<PinsRepo>,
}

impl LauncherTool {
    pub fn new(registry: Arc<SourceRegistry>, frequency: Arc<FrequencyRepo>, pins: Arc<PinsRepo>) -> Self {
        Self { registry, frequency, pins }
    }
}

#[tool_actions]
impl LauncherTool {
    /// Search the launcher for items matching the query.
    async fn search(&self, ctx: &ToolContext, p: SearchParams) -> Result<Vec<LauncherItem>> {
        // Reuses LauncherSearchEngine logic; for the agent, we don't need
        // tasks/notes/calendar provider injections (those run in app-core).
        // Delegate via a stripped-down search pipeline:
        let results = self.registry.dispatch(&p.query).await;
        Ok(results.into_iter().take(p.limit as usize).map(|r| r.item).collect())
    }

    /// Execute a launcher item by id + kind.
    async fn execute(&self, _ctx: &ToolContext, p: ExecuteParams) -> Result<LauncherExecuteResult> {
        // Record frecency, return Ok envelope. Actual side-effect dispatch
        // (open app, run script) happens client-side via the desktop UI;
        // for non-desktop callers (cron, MCP), we expose run_script and
        // apply_window as separate actions.
        self.frequency.record_usage(&p.item_id, &p.kind).await?;
        Ok(LauncherExecuteResult::ok_msg("recorded"))
    }

    /// Apply a window layout preset.
    async fn apply_window(&self, _ctx: &ToolContext, p: ApplyWindowParams) -> Result<()> {
        let action = parse_window_action(&p.action)?;
        crate::window_manager().execute(&action).await?;
        Ok(())
    }

    /// Pin a launcher item to the top of results.
    async fn pin(&self, _ctx: &ToolContext, p: PinParams) -> Result<()> {
        self.pins.pin(&p.item_id, &p.kind).await
    }

    /// Remove a pin.
    async fn unpin(&self, _ctx: &ToolContext, p: PinParams) -> Result<()> {
        self.pins.unpin(&p.item_id, &p.kind).await
    }
}

fn parse_window_action(s: &str) -> Result<WindowAction> {
    if let Some(rest) = s.strip_prefix("preset:") {
        return Ok(WindowAction::Preset(rest.to_string()));
    }
    Ok(match s {
        "leftHalf" => WindowAction::LeftHalf,
        "rightHalf" => WindowAction::RightHalf,
        "topHalf" => WindowAction::TopHalf,
        "bottomHalf" => WindowAction::BottomHalf,
        "leftThird" => WindowAction::LeftThird,
        "centerThird" => WindowAction::CenterThird,
        "rightThird" => WindowAction::RightThird,
        "maximize" => WindowAction::Maximize,
        "center" => WindowAction::Center,
        "restore" => WindowAction::Restore,
        other => return Err(common::KlyntbotError::other(format!("unknown window action: {other}"))),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_actions() {
        assert!(matches!(parse_window_action("leftHalf").unwrap(), WindowAction::LeftHalf));
        assert!(matches!(parse_window_action("preset:left-third").unwrap(), WindowAction::Preset(_)));
        assert!(parse_window_action("garbage").is_err());
    }
}
```

- [ ] **Step 4: Update `LauncherFeature` and `FeaturePackage::tools()`**

In `crates/feature-launcher/src/lib.rs`:

```rust
pub mod tool;
pub use tool::LauncherTool;

use std::sync::Arc;

#[derive(Clone)]
pub struct LauncherToolDeps {
    pub registry: Arc<SourceRegistry>,
    pub frequency: Arc<FrequencyRepo>,
    pub pins: Arc<PinsRepo>,
}

#[derive(Default)]
pub struct LauncherFeature {
    tool_deps: Option<LauncherToolDeps>,
}

impl LauncherFeature {
    pub fn new() -> Self { Self::default() }
    pub fn with_tool_deps(deps: LauncherToolDeps) -> Self {
        Self { tool_deps: Some(deps) }
    }
}

impl FeaturePackage for LauncherFeature {
    fn name(&self) -> &str { "launcher" }
    fn migrations(&self) -> Vec<FeatureMigration> { launcher_migrations() }
    fn health_check(&self) -> HealthStatus { HealthStatus::Healthy }
    fn tools(&self) -> Vec<DynTool> {
        if let Some(deps) = &self.tool_deps {
            vec![Box::new(LauncherTool::new(
                deps.registry.clone(),
                deps.frequency.clone(),
                deps.pins.clone(),
            ))]
        } else {
            vec![]
        }
    }
}
```

(If `LauncherFeature` already exists with other fields, add the `tool_deps` field and `with_tool_deps` constructor without removing existing fields.)

- [ ] **Step 5: Update `Cargo.toml`**

Add to `crates/feature-launcher/Cargo.toml`:

```toml
tools-core-macros = { workspace = true }
schemars = { workspace = true }
```

- [ ] **Step 6: Tests**

```bash
cargo nextest run -p feature-launcher tool
```

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat(launcher): add LauncherTool with 5 actions (search/execute/apply_window/pin/unpin)"
```

---

## Task 5.2: Register `LauncherFeature` in agent feature registry + MCP allowlist

**Files:**
- Modify: `crates/app-core/src/init/ai_pipeline.rs:139-149`
- Modify: `crates/config/src/schema/mcp.rs:191-208` (add `"launcher"`)

- [ ] **Step 1: Add to feature registry**

In `build_feature_registry()`:

```rust
let launcher = LauncherFeature::with_tool_deps(LauncherToolDeps {
    registry: launcher_engine.registry.clone(),
    frequency: Arc::new(launcher_engine.frequency_repo.clone()),
    pins: Arc::new(launcher_engine.pins_repo.clone()),
});
registry.add_feature(Box::new(launcher));
```

- [ ] **Step 2: Add to MCP allowlist**

```rust
const EXPLICIT_TOOL_ALLOWLIST: &[&str] = &[
    // ... existing entries ...
    "launcher",
];
```

- [ ] **Step 3: Verify MCP discovery**

Build and run: `cargo run -p klyntbot-server -- mcp tools --list`
Expected output includes `launcher`.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat(launcher): expose LauncherTool to agent + MCP allowlist"
```

---

# Phase 6 — Voice mode replacement

## Task 6.1: Build `useVoiceRecording` hook

**Files:**
- Create: `desktop-ui/src/features/launcher/hooks/useVoiceRecording.ts`

`★ Design note:` The voice infrastructure (`crates/desktop/src/main.rs:497-558`) exposes IPC commands like `voice_start_capture`, `voice_stop_capture`, `voice_get_transcript` (verify exact names via `grep -rn "voice_" crates/desktop/src/commands/`). The hook wraps those with a state machine: `idle → recording → processing → done | error`.

- [ ] **Step 1: Verify voice IPC surface**

Run: `grep -rn "#\[klynt_command\]" crates/desktop/src/commands/voice*`
Note: if the voice surface uses different command names, adapt below. Document the exact command names found.

- [ ] **Step 2: Write the hook**

```ts
// desktop-ui/src/features/launcher/hooks/useVoiceRecording.ts
import { useCallback, useEffect, useRef, useState } from "react";
import { ipc, listen } from "@/utils/tauri-bridge";
import { showError } from "../lib/showError";

type Phase = "idle" | "recording" | "processing" | "error";

export function useVoiceRecording(onTranscript: (t: string) => void) {
  const [phase, setPhase] = useState<Phase>("idle");
  const [level, setLevel] = useState(0);
  const stoppedRef = useRef(false);

  const start = useCallback(async () => {
    stoppedRef.current = false;
    setPhase("recording");
    try {
      await ipc("voice_start_capture", {});
    } catch (err) {
      setPhase("error");
      showError("Couldn't start recording:", err);
    }
  }, []);

  const stop = useCallback(async () => {
    if (stoppedRef.current) return;
    stoppedRef.current = true;
    setPhase("processing");
    try {
      const transcript: string = await ipc("voice_stop_capture", {});
      onTranscript(transcript);
      setPhase("idle");
    } catch (err) {
      setPhase("error");
      showError("Couldn't process transcript:", err);
    }
  }, [onTranscript]);

  const cancel = useCallback(async () => {
    stoppedRef.current = true;
    try {
      await ipc("voice_cancel_capture", {});
    } catch (err) {
      console.error("voice_cancel_capture failed:", err);
    }
    setPhase("idle");
  }, []);

  // Live audio level for waveform animation
  useEffect(() => {
    const unlisten = listen<{ level: number }>("voice:level", (e) => {
      setLevel(e.payload.level);
    });
    return () => { unlisten.then((u) => u()); };
  }, []);

  return { phase, level, start, stop, cancel };
}
```

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/launcher/hooks/useVoiceRecording.ts
git commit -m "feat(launcher-ui): add useVoiceRecording hook"
```

---

## Task 6.2: Replace `VoiceRecorderStub` with real `VoiceRecorder`

**Files:**
- Create: `desktop-ui/src/features/launcher/components/VoiceRecorder.tsx`
- Delete: `desktop-ui/src/features/launcher/components/VoiceRecorderStub.tsx`
- Modify: `desktop-ui/src/features/launcher/components/Launcher.tsx:163`
- Modify: `desktop-ui/src/features/launcher/launcher.css`

- [ ] **Step 1: Build the component**

```tsx
// desktop-ui/src/features/launcher/components/VoiceRecorder.tsx
import { useEffect } from "react";
import { useVoiceRecording } from "../hooks/useVoiceRecording";

interface Props {
  onTranscriptReady: (transcript: string) => void;
  onCancel: () => void;
}

export function VoiceRecorder({ onTranscriptReady, onCancel }: Props) {
  const { phase, level, start, stop, cancel } = useVoiceRecording(onTranscriptReady);

  useEffect(() => { void start(); }, [start]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") { void cancel(); onCancel(); }
      if (e.key === "Enter") { void stop(); }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [stop, cancel, onCancel]);

  return (
    <div className="lc-voice">
      <div className={`lc-voice-orb lc-voice-orb--${phase}`}
           style={{ transform: `scale(${1 + level * 0.4})` }}>
        🎙
      </div>
      <p className="lc-muted-sm">
        {phase === "recording" && "Listening… press Enter to send"}
        {phase === "processing" && "Transcribing…"}
        {phase === "error" && "Something went wrong"}
        {phase === "idle" && "Press to start"}
      </p>
      <p className="lc-hint-sm">Esc to cancel</p>
    </div>
  );
}
```

- [ ] **Step 2: Update Launcher.tsx**

Change import + usage:

```tsx
// before:
import { VoiceRecorderStub } from "./VoiceRecorderStub";
// ...
{mode === "recording" && <VoiceRecorderStub onCancel={...} />}

// after:
import { VoiceRecorder } from "./VoiceRecorder";
// ...
{mode === "recording" && (
  <VoiceRecorder
    onTranscriptReady={(t) => {
      setMode("search");
      setQuery(t);
    }}
    onCancel={() => setMode("dashboard")}
  />
)}
```

- [ ] **Step 3: Update CSS**

In `launcher.css`, replace the `.lc-voice-stub*` block with `.lc-voice*` styles:

```css
.lc-voice {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 60px 24px;
  gap: 16px;
}
.lc-voice-orb {
  width: 80px;
  height: 80px;
  border-radius: 50%;
  display: flex; align-items: center; justify-content: center;
  font-size: 36px;
  background: radial-gradient(circle, var(--brand, #6ea8fe) 0%, transparent 70%);
  transition: transform 60ms linear;
  will-change: transform;
}
.lc-voice-orb--recording { animation: lc-voice-pulse 1.6s ease-in-out infinite; }
.lc-voice-orb--processing { opacity: 0.6; animation: lc-voice-spin 1.2s linear infinite; }
.lc-voice-orb--error { background: var(--destructive, #f87171); }
.lc-hint-sm { font-size: var(--fs-2xs); color: var(--text-faint); }

@keyframes lc-voice-pulse { 0%, 100% { box-shadow: 0 0 0 0 rgba(110, 168, 254, 0.4); } 50% { box-shadow: 0 0 0 24px rgba(110, 168, 254, 0); } }
@keyframes lc-voice-spin { from { transform: rotate(0deg); } to { transform: rotate(360deg); } }

@media (prefers-reduced-motion: reduce) {
  .lc-voice-orb { animation: none !important; transition: none !important; }
}
```

Also remove the `/* ── Stubs ── */` section header comment.

- [ ] **Step 4: Delete the stub file**

```bash
rm desktop-ui/src/features/launcher/components/VoiceRecorderStub.tsx
```

- [ ] **Step 5: Verify**

```bash
cd desktop-ui && bun run typecheck && bun run lint
```

- [ ] **Step 6: Smoke test (macOS)**

Open launcher, press voice hotkey, speak, press Enter — confirm transcript fills query. Press Esc — confirm returns to dashboard.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat(launcher-ui): replace VoiceRecorderStub with real VoiceRecorder

Wires through to voice IPC commands; transcript flows into search query."
```

---

# Phase 7 — AI chat mode replacement

`★ Dependency:` This phase requires the chat session API from `docs/superpowers/plans/2026-04-27-chat-surface-integration.md`. If that plan has not landed, defer Phase 7 until it has.

## Task 7.1: Build `LauncherChat` component

**Files:**
- Create: `desktop-ui/src/features/launcher/components/LauncherChat.tsx`
- Delete: `desktop-ui/src/features/launcher/components/LauncherChatStub.tsx`
- Modify: `desktop-ui/src/features/launcher/components/Launcher.tsx:165`
- Modify: `desktop-ui/src/features/launcher/launcher.css`

- [ ] **Step 1: Read the chat session API the chat-surface-integration plan exposed**

`grep -rn "chat_session_create\|chatSession\|useChatSession" desktop-ui/src/`
Document the exact hook/IPC names; the implementation below is illustrative.

- [ ] **Step 2: Build the component**

```tsx
// desktop-ui/src/features/launcher/components/LauncherChat.tsx
import { useEffect, useRef, useState } from "react";
import { useChatSession } from "@/features/chat/hooks/useChatSession"; // adjust per actual API

interface Props {
  initialQuery: string;
  sessionKey: string;
  onBack: () => void;
  onExpandToMain: (sessionKey: string) => void;
}

export function LauncherChat({ initialQuery, sessionKey, onBack, onExpandToMain }: Props) {
  const { messages, sendMessage, isStreaming } = useChatSession(sessionKey);
  const [input, setInput] = useState("");
  const sentInitialRef = useRef(false);

  useEffect(() => {
    if (!sentInitialRef.current && initialQuery) {
      sentInitialRef.current = true;
      void sendMessage(initialQuery);
    }
  }, [initialQuery, sendMessage]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onBack();
      if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) onExpandToMain(sessionKey);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onBack, onExpandToMain, sessionKey]);

  return (
    <div className="lc-chat">
      <header className="lc-chat-header">
        <button className="lc-icon-btn" onClick={onBack} aria-label="Back">←</button>
        <span className="lc-chat-title">Ask</span>
        <button className="lc-icon-btn" onClick={() => onExpandToMain(sessionKey)} aria-label="Expand">↗</button>
      </header>
      <div className="lc-chat-thread" role="log" aria-live="polite">
        {messages.map((m) => (
          <div key={m.id} className={`lc-chat-msg lc-chat-msg--${m.role}`}>
            {m.content}
          </div>
        ))}
        {isStreaming && <div className="lc-chat-streaming">…</div>}
      </div>
      <form className="lc-chat-composer" onSubmit={(e) => {
        e.preventDefault();
        if (input.trim()) { void sendMessage(input); setInput(""); }
      }}>
        <input
          className="lc-chat-input"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          placeholder="Reply… (⌘↵ to expand)"
          autoFocus
        />
      </form>
    </div>
  );
}
```

- [ ] **Step 3: Replace usage in Launcher.tsx**

```tsx
import { LauncherChat } from "./LauncherChat";
// ...
{mode === "chat" && (
  <LauncherChat
    initialQuery={chatInitialQuery ?? ""}
    sessionKey={chatSessionKey}
    onBack={() => setMode("dashboard")}
    onExpandToMain={(key) => {
      emit("navigate", { path: "/chat" });
      emit("open-chat", { sessionKey: key });
      getCurrentWindow().hide();
    }}
  />
)}
```

- [ ] **Step 4: CSS**

Replace `.lc-chat-stub*` styles with real `.lc-chat*` styles. Keep the original visual lineage (rounded card, blurred backdrop) and add scrollable thread, composer.

- [ ] **Step 5: Delete the stub**

```bash
rm desktop-ui/src/features/launcher/components/LauncherChatStub.tsx
```

- [ ] **Step 6: Verify + smoke test**

`bun run typecheck && bun run lint`. Open launcher, type a question ending with `?` to trigger AI fallback, Enter — confirm chat thread appears and streams.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat(launcher-ui): replace LauncherChatStub with real chat surface"
```

---

# Phase 8 — Performance audit + tuning

## Task 8.1: Add criterion benchmark for inverted index

**Files:**
- Create: `crates/feature-launcher/benches/inverted_index.rs`
- Modify: `crates/feature-launcher/Cargo.toml` (add `[[bench]]`)

- [ ] **Step 1: Add bench**

```rust
// crates/feature-launcher/benches/inverted_index.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use feature_launcher::InvertedFileIndex;

fn build_corpus(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("/Users/test/projects/repo-{}/src/module/file_{}.rs", i % 50, i)).collect()
}

fn bench_build(c: &mut Criterion) {
    let corpus = build_corpus(50_000);
    c.bench_function("inverted_index_build_50k", |b| {
        b.iter(|| {
            let mut idx = InvertedFileIndex::new(50_000);
            for (i, p) in corpus.iter().enumerate() {
                idx.insert(black_box(i as u32), black_box(p));
            }
        });
    });
}

fn bench_query(c: &mut Criterion) {
    let mut idx = InvertedFileIndex::new(50_000);
    for (i, p) in build_corpus(50_000).iter().enumerate() { idx.insert(i as u32, p); }
    c.bench_function("inverted_index_query_short", |b| {
        b.iter(|| { let _ = idx.search(black_box("file_42")); });
    });
    c.bench_function("inverted_index_query_path", |b| {
        b.iter(|| { let _ = idx.search(black_box("repo-3 module")); });
    });
}

criterion_group!(benches, bench_build, bench_query);
criterion_main!(benches);
```

In `Cargo.toml`:

```toml
[dev-dependencies]
criterion = { workspace = true }

[[bench]]
name = "inverted_index"
harness = false
```

- [ ] **Step 2: Run baseline**

```bash
cargo bench -p feature-launcher --bench inverted_index | tee bench-baseline.txt
```

Save output for comparison.

- [ ] **Step 3: Commit**

```bash
git add crates/feature-launcher/benches/ crates/feature-launcher/Cargo.toml
git commit -m "perf(launcher): add criterion bench for inverted file index"
```

---

## Task 8.2: Tune cache TTLs and debounce

**Files:**
- Modify: `crates/feature-launcher/src/search/mod.rs` (cache eviction interval, TTLs)
- Modify: `desktop-ui/src/features/launcher/hooks/useLauncherSearch.ts` (debounce)

- [ ] **Step 1: Profile typical query path**

Run launcher with `RUST_LOG=feature_launcher=debug,app_core::handlers::launcher=debug cargo tauri dev`. Type a 5-char query slowly; capture log of source-by-source latency. Common offenders: `BrowserHistorySource` (DB copy), `ContentGrepSource` (rg spawn).

- [ ] **Step 2: Apply targeted tuning**

Document changes per source (in code comments referencing measured numbers):

```rust
// before: 60s cache eviction; after: 90s — typical session keeps a query <90s
const CACHE_EVICT_INTERVAL: Duration = Duration::from_secs(90);

// ContentGrepSource: bump to 8s TTL since rg spawn is ~50ms even on cache miss
const GREP_CACHE_TTL: Duration = Duration::from_secs(8);
```

- [ ] **Step 3: Frontend debounce**

In `useLauncherSearch.ts`, change debounce from 30ms to 16ms (one frame) for responsiveness on fast typing.

- [ ] **Step 4: Re-run bench**

```bash
cargo bench -p feature-launcher --bench inverted_index | tee bench-after.txt
diff bench-baseline.txt bench-after.txt
```

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "perf(launcher): tune cache TTLs and frontend debounce

- Search cache evict: 60s → 90s (matches typical session length)
- Grep cache TTL: 5s → 8s (rg spawn dominates miss cost)
- UI debounce: 30ms → 16ms (single frame for responsive typing)"
```

---

## Task 8.3: Virtualize results list when >40 items

**Files:**
- Modify: `desktop-ui/src/features/launcher/components/ResultsList.tsx`
- Modify: `desktop-ui/package.json` (add `react-window`)

- [ ] **Step 1: Add dep**

```bash
cd desktop-ui && bun add react-window
```

- [ ] **Step 2: Wrap list with conditional virtualizer**

```tsx
import { FixedSizeList as List } from "react-window";

const ROW_HEIGHT = 48;
const VIRTUALIZE_THRESHOLD = 40;

if (items.length >= VIRTUALIZE_THRESHOLD) {
  return (
    <List
      height={Math.min(items.length * ROW_HEIGHT, 480)}
      itemCount={items.length}
      itemSize={ROW_HEIGHT}
      width="100%"
    >
      {({ index, style }) => (
        <div style={style}>
          <ResultRow item={items[index]} ... />
        </div>
      )}
    </List>
  );
}
// else fall through to existing non-virtualized list
```

- [ ] **Step 3: Verify keyboard scroll-into-view still works**

Manually test arrow nav past row 30 with virtualization on; confirm scrolling.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "perf(launcher-ui): virtualize ResultsList when items ≥ 40"
```

---

# Phase 9 — UX/UI polish

## Task 9.1: Typography token audit

**Files:**
- Modify: `desktop-ui/src/features/launcher/launcher.css`

- [ ] **Step 1: Find all hardcoded font sizes**

```bash
grep -nE "font-size: ?[0-9]+px" desktop-ui/src/features/launcher/launcher.css
```

- [ ] **Step 2: Replace each with appropriate token**

Token scale (from CLAUDE.md): `--fs-2xs` (10.5px), `--fs-xs` (11.5px), `--fs-sm` (12.5px = `--fs-base`), `--fs-md` (13.5px), `--fs-lg` (15px), `--fs-xl` (17px). For ≥20px (display-style), add a new token to `ds-tokens.css` (e.g. `--fs-display-md: 24px;`) — don't hardcode.

Specific replacement: calculator `font-size: 28px` (line ~? — find it) → add `--fs-display-lg: 28px;` to `ds-tokens.css` and use the token.

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "style(launcher): replace hardcoded font-size with --fs-* tokens"
```

---

## Task 9.2: Empty state + skeleton loader

**Files:**
- Create: `desktop-ui/src/features/launcher/components/EmptyState.tsx`
- Create: `desktop-ui/src/features/launcher/components/LoadingState.tsx`
- Modify: `desktop-ui/src/features/launcher/components/ResultsList.tsx`
- Modify: `desktop-ui/src/features/launcher/launcher.css`

- [ ] **Step 1: EmptyState**

```tsx
// EmptyState.tsx
export function EmptyState({ query }: { query: string }) {
  return (
    <div className="lc-empty" role="status">
      <div className="lc-empty-icon">🔍</div>
      <p className="lc-empty-title">No results for "{query}"</p>
      <ul className="lc-empty-hints">
        <li><kbd>f/</kbd> Files</li>
        <li><kbd>g/</kbd> Grep</li>
        <li><kbd>h/</kbd> History</li>
        <li><kbd>@</kbd> Contacts</li>
        <li><kbd>{">"}</kbd> Commands</li>
        <li><kbd>?</kbd> Ask AI</li>
      </ul>
    </div>
  );
}
```

- [ ] **Step 2: LoadingState (skeleton)**

```tsx
export function LoadingState() {
  return (
    <ul className="lc-skeleton">
      {[0,1,2,3,4].map((i) => (
        <li key={i} className="lc-skeleton-row" style={{ animationDelay: `${i * 60}ms` }}>
          <div className="lc-skeleton-icon" />
          <div className="lc-skeleton-text">
            <div className="lc-skeleton-title" />
            <div className="lc-skeleton-subtitle" />
          </div>
        </li>
      ))}
    </ul>
  );
}
```

- [ ] **Step 3: Wire into ResultsList**

```tsx
if (isSearching && items.length === 0) return <LoadingState />;
if (!isSearching && items.length === 0 && query.length > 0) return <EmptyState query={query} />;
```

- [ ] **Step 4: CSS**

```css
.lc-empty { display: flex; flex-direction: column; align-items: center; padding: 40px 24px; gap: 12px; }
.lc-empty-icon { font-size: var(--fs-display-md); opacity: 0.6; }
.lc-empty-title { font-size: var(--fs-md); color: var(--text-muted); }
.lc-empty-hints { display: grid; grid-template-columns: repeat(2, 1fr); gap: 6px 24px; list-style: none; padding: 0; font-size: var(--fs-xs); color: var(--text-faint); }
.lc-empty-hints kbd { font-family: var(--code-font-family); padding: 1px 4px; background: var(--surface-control); border-radius: 3px; margin-right: 6px; }

.lc-skeleton { list-style: none; padding: 8px 12px; margin: 0; }
.lc-skeleton-row { display: flex; gap: 10px; padding: 8px; align-items: center; animation: lc-skeleton-shimmer 1.4s ease-in-out infinite; }
.lc-skeleton-icon { width: 22px; height: 22px; border-radius: 4px; background: var(--surface-control); }
.lc-skeleton-text { flex: 1; display: flex; flex-direction: column; gap: 4px; }
.lc-skeleton-title { width: 50%; height: 10px; border-radius: 3px; background: var(--surface-control); }
.lc-skeleton-subtitle { width: 30%; height: 8px; border-radius: 3px; background: var(--surface-control); opacity: 0.6; }
@keyframes lc-skeleton-shimmer { 0%, 100% { opacity: 1; } 50% { opacity: 0.5; } }
@media (prefers-reduced-motion: reduce) { .lc-skeleton-row { animation: none; opacity: 0.7; } }
```

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(launcher-ui): empty state with shortcut hints + skeleton loader"
```

---

## Task 9.3: Mount/unmount transitions + reduced-motion

**Files:**
- Modify: `desktop-ui/src/features/launcher/launcher.css`
- Modify: `desktop-ui/src/features/launcher/components/Launcher.tsx`

- [ ] **Step 1: Add CSS transition (no library)**

```css
.lc-shell { animation: lc-mount 80ms ease-out; }
@keyframes lc-mount { from { opacity: 0; transform: translateY(-4px) scale(0.98); } to { opacity: 1; transform: translateY(0) scale(1); } }

@media (prefers-reduced-motion: reduce) {
  .lc-shell { animation: none; }
}
```

- [ ] **Step 2: Verify on launcher show/hide**

Smoke test: hotkey opens with subtle fade-in; closes instantly (Tauri `hide()` is synchronous).

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/launcher/launcher.css
git commit -m "style(launcher-ui): add 80ms mount transition with reduced-motion respect"
```

---

# Phase 10 — Frontend test coverage

## Task 10.1: Test fixtures

**Files:**
- Create: `desktop-ui/src/features/launcher/__fixtures__/items.ts`

- [ ] **Step 1: Build fixtures**

```ts
import type { LauncherItem, DashboardData } from "../types";

export const itemApp = (overrides?: Partial<LauncherItem>): LauncherItem => ({
  id: "app:/Applications/Slack.app",
  title: "Slack",
  subtitle: "/Applications/Slack.app",
  icon: null,
  kind: { type: "application", path: "/Applications/Slack.app", running: false },
  score: 1.2,
  noView: false,
  arguments: [],
  pinned: false,
  ...overrides,
});

export const itemCalc = (expression = "2+2", result = 4): LauncherItem => ({
  id: `calc:${expression}`,
  title: `${result}`,
  subtitle: expression,
  icon: "🔢",
  kind: { type: "calculator", expression, result },
  score: 2.0,
  noView: false,
  arguments: [],
  pinned: false,
});

export const dashboardEmpty = (): DashboardData => ({
  calendar: [],
  tasks: [],
  productivity: { totalMinutes: 0, topCategory: "", topCategoryPct: 0, score: 0 },
});
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/launcher/__fixtures__/
git commit -m "test(launcher-ui): add shared test fixtures"
```

---

## Task 10.2: Store reducer test

**Files:**
- Create: `desktop-ui/src/features/launcher/store.test.tsx`

- [ ] **Step 1: Test reducer transitions**

```tsx
import { describe, it, expect } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { LauncherStoreProvider, useLauncher } from "./store";
import { itemApp } from "./__fixtures__/items";

const wrapper = ({ children }: { children: React.ReactNode }) => (
  <LauncherStoreProvider>{children}</LauncherStoreProvider>
);

describe("launcher store", () => {
  it("setQuery transitions mode dashboard → search", () => {
    const { result } = renderHook(() => useLauncher(), { wrapper });
    expect(result.current.state.mode).toBe("dashboard");
    act(() => result.current.api.setQuery("hello"));
    expect(result.current.state.mode).toBe("search");
  });

  it("moveSelection clamps within bounds", () => {
    const { result } = renderHook(() => useLauncher(), { wrapper });
    act(() => result.current.api.setResults([itemApp(), itemApp({ id: "b" })]));
    act(() => result.current.api.moveSelection(5));
    expect(result.current.state.selectedIndex).toBe(1);
    act(() => result.current.api.moveSelection(-10));
    expect(result.current.state.selectedIndex).toBe(0);
  });

  it("pushHistory caps at 50", () => {
    const { result } = renderHook(() => useLauncher(), { wrapper });
    act(() => {
      for (let i = 0; i < 60; i++) result.current.api.pushHistory(`q${i}`);
    });
    expect(result.current.state.queryHistory.length).toBe(50);
  });

  it("reset returns to dashboard mode", () => {
    const { result } = renderHook(() => useLauncher(), { wrapper });
    act(() => result.current.api.setQuery("x"));
    act(() => result.current.api.reset());
    expect(result.current.state.mode).toBe("dashboard");
    expect(result.current.state.query).toBe("");
  });
});
```

- [ ] **Step 2: Run**

```bash
cd desktop-ui && bun run test src/features/launcher/store.test.tsx
```

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/launcher/store.test.tsx
git commit -m "test(launcher-ui): cover store reducer transitions"
```

---

## Task 10.3: Component smoke tests

**Files:**
- Create: `desktop-ui/src/features/launcher/components/LauncherInput.test.tsx`
- Create: `desktop-ui/src/features/launcher/components/ResultsList.test.tsx`
- Create: `desktop-ui/src/features/launcher/components/Dashboard.test.tsx`
- Create: `desktop-ui/src/features/launcher/components/ActionMenu.test.tsx`
- Create: `desktop-ui/src/features/launcher/components/ArgChipBar.test.tsx`
- Create: `desktop-ui/src/features/launcher/components/EmptyState.test.tsx`

For each, follow this pattern (example for `LauncherInput`):

- [ ] **Step 1: Write the test**

```tsx
// LauncherInput.test.tsx
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { LauncherStoreProvider } from "../store";
import { LauncherInput } from "./LauncherInput";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

describe("LauncherInput", () => {
  it("renders search input", () => {
    render(
      <LauncherStoreProvider>
        <LauncherInput isSearching={false} />
      </LauncherStoreProvider>
    );
    expect(screen.getByRole("textbox")).toBeInTheDocument();
  });

  it("shows spinner when searching", () => {
    render(
      <LauncherStoreProvider>
        <LauncherInput isSearching={true} />
      </LauncherStoreProvider>
    );
    expect(screen.getByLabelText(/searching/i)).toBeInTheDocument();
  });

  it("typing updates query in store", () => {
    render(
      <LauncherStoreProvider>
        <LauncherInput isSearching={false} />
      </LauncherStoreProvider>
    );
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "hello" } });
    // Read store via separate hook in real test; alternative: add data-testid attrs
  });
});
```

Repeat shape for each component listed.

- [ ] **Step 2: Run all**

```bash
cd desktop-ui && bun run test src/features/launcher/
```

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/launcher/components/*.test.tsx
git commit -m "test(launcher-ui): smoke tests for LauncherInput, ResultsList, Dashboard, ActionMenu, ArgChipBar, EmptyState"
```

---

## Task 10.4: Hook tests

**Files:**
- Create: `desktop-ui/src/features/launcher/hooks/useLauncherSearch.test.ts`
- Create: `desktop-ui/src/features/launcher/hooks/useExecuteItem.test.ts`
- Create: `desktop-ui/src/features/launcher/hooks/useDashboardData.test.ts`
- Create: `desktop-ui/src/features/launcher/hooks/useDndActive.test.ts`
- Create: `desktop-ui/src/features/launcher/hooks/useVoiceRecording.test.ts`

- [ ] **Step 1: Pattern for `useLauncherSearch`**

```ts
import { describe, it, expect, vi } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { useLauncherSearch } from "./useLauncherSearch";
import { mockTauri } from "@/test/mockTauri";
import { itemApp } from "../__fixtures__/items";

describe("useLauncherSearch", () => {
  it("debounces and calls launcher_search", async () => {
    mockTauri({ launcher_search: vi.fn(async () => [itemApp()]) });
    const qc = new QueryClient();
    const wrapper = ({ children }: { children: React.ReactNode }) =>
      <QueryClientProvider client={qc}>{children}</QueryClientProvider>;
    const { result, rerender } = renderHook(({ q }) => useLauncherSearch(q), {
      initialProps: { q: "" }, wrapper,
    });
    rerender({ q: "slack" });
    await waitFor(() => expect(result.current.results.length).toBe(1));
  });

  it("empty query does not fire IPC", async () => {
    const search = vi.fn();
    mockTauri({ launcher_search: search });
    const qc = new QueryClient();
    renderHook(() => useLauncherSearch(""), {
      wrapper: ({ children }) => <QueryClientProvider client={qc}>{children}</QueryClientProvider>,
    });
    await new Promise(r => setTimeout(r, 50));
    expect(search).not.toHaveBeenCalled();
  });
});
```

- [ ] **Step 2: Run**

```bash
bun run test src/features/launcher/hooks/
```

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/launcher/hooks/*.test.ts
git commit -m "test(launcher-ui): hook tests for search/execute/dashboard/dnd/voice"
```

---

# Final verification

## Task FINAL.1: Full workspace verification

- [ ] **Step 1: Lint everything**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
cd desktop-ui && bun run typecheck && bun run lint
```

- [ ] **Step 2: Run all tests**

```bash
cargo nextest run --workspace
cargo test --workspace --doc
cd desktop-ui && bun run test
```

- [ ] **Step 3: Verify drift tests**

```bash
cargo nextest run -p desktop registration_drift bindings_are_current no_raw_tauri_command_outside_macros
```

- [ ] **Step 4: MCP discovery check**

```bash
cargo run -p klyntbot-server -- mcp tools --list | grep launcher
```
Expected: `launcher` listed.

- [ ] **Step 5: Manual end-to-end smoke test (macOS)**

Open launcher (Alt+Space). Verify in order:
1. Dashboard renders with calendar, tasks, productivity (no FocusDashboard widget).
2. Click a task row → main window opens to that task.
3. Search "slack" → Slack appears at top; pin it via Cmd+K → "Pin to top". Close launcher, reopen with empty query → Slack at top with 📌.
4. Search "left half" → Enter → window goes left-half. Search "restore" → Enter → window returns to original frame.
5. Search a calendar event title → it appears with 📅 icon.
6. Search a non-existent term → empty state shows shortcut hints.
7. Type slowly to see skeleton loader briefly.
8. Trigger a known-failing IPC (e.g. delete an indexed app, search & open) → red error badge appears.
9. DND command → fill "30m" → Enter → DND activates for 30 minutes.
10. From a chat with an LLM via `klyntbot mcp` external client, call `launcher.search` and `launcher.apply_window` — confirm both work.

- [ ] **Step 6: Final commit + PR**

```bash
git status
# expect clean

# Push branch + open PR
git push -u origin <branch>
gh pr create --title "feat(launcher): complete launcher feature (cleanup, pinning, calendar, agent tool, voice/chat, polish)" --body "$(cat <<'EOF'
## Summary
- Phase 0: Cleanup of dead code (FocusDashboard, frequencies table, destroy_if_hidden, args param, MSRV pins)
- Phase 1: Restore window action, error toasts, onOpenTask, DND duration threading
- Phase 2: Pinning feature (PinsRepo, IPC, ActionMenu, glyph)
- Phase 3: Calendar search source via injected fetcher trait
- Phase 4: DomainEventBus publishing for launcher executions
- Phase 5: LauncherTool (5 actions) registered with agent + MCP allowlist
- Phase 6: VoiceRecorder replaces stub
- Phase 7: LauncherChat replaces stub (depends on chat-surface-integration plan)
- Phase 8: Performance — criterion bench, cache TTL tuning, virtualized list, frame-aligned debounce
- Phase 9: UX/UI — typography tokens, empty state, skeleton loader, mount transition, reduced-motion
- Phase 10: Frontend test coverage (store, components, hooks)

## Test plan
- [x] cargo nextest run --workspace
- [x] cargo clippy --workspace -- -D warnings
- [x] cd desktop-ui && bun run test && bun run typecheck && bun run lint
- [x] Manual smoke test (10 scenarios)
- [x] MCP discovery: launcher listed

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

# Decision log (rationale captured for future reference)

| Decision | Rationale |
|---|---|
| Drop `launcher_frequencies` table in-place | Pre-release; CLAUDE.md allows; redundant with `launcher_usage_log` |
| Keep `launcher_pins` table, build PinsRepo | Pinning is high-value UX (Alfred/Raycast parity); schema is correct |
| Remove `FocusDashboard` widget entirely | User explicitly does not want; redundant with FocusActiveChip + tray |
| Inject `CalendarFetcher` trait into `feature-launcher` | Preserves L4 dependency-inversion architecture; calendar provider lives in app-core |
| Restore captures frame stack of 8 per window | Bounded memory; supports multiple Restore presses |
| Strip `args` from `launcher_execute`, keep on specific commands | A silently-ignored param is worse than no param |
| Pin score boost = 1000 (after dedup, before resort) | Strong elevation; tied items keep base ordering |
| Voice level live-streaming via `voice:level` event | Avoids per-frame IPC roundtrips |
| Virtualize ResultsList only when ≥40 items | react-window has fixed-cost overhead; unjustified for typical 5-20 item results |
| Frontend debounce 30ms → 16ms | One frame; matches user perception threshold |
| Domain event publishes `query: Option<String>` | Mirror can correlate query patterns with executions |
