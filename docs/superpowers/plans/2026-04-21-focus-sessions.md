# Focus Sessions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a durable, extendable "timed system action" primitive — debuted as DND-with-duration (the PR-3 chip finally honoring `2h` / `until 5pm` / `until next meeting`), but architected as a general Focus subsystem that can be reused for Pomodoro, timed silent mode, scheduled screen-off, etc.

**Problem today:** The `ToggleDoNotDisturb` launcher command accepts a `duration` argument (PR-3 wired the chip), but the Rust executor just runs the Apple Shortcut `Toggle Do Not Disturb` and ignores the string. The chip is a lie. A fire-and-forget `tokio::spawn(async { sleep; untoggle })` would fix that in 30 LOC but loses state on restart, can't be cancelled or extended, and leaves no UX for the user to see remaining time.

**Architecture:**
- **Persistence:** a `focus_sessions` table (at most one active). Survives restarts.
- **Scheduling:** end-time is an `AlarmRule::Absolute { fire_at: ends_at }` on the existing `TemporalScheduler`. A new `FocusEndSubscriber` listens for `DomainEvent::AlarmFired { kind: "focus_end", .. }` and runs the "turn off" side effect.
- **macOS bridge:** klyntbot ships two bundled shortcuts — `Klyntbot Focus On.shortcut` and `Klyntbot Focus Off.shortcut` — plus a one-click installer Tauri command that opens them in the Shortcuts app. No reading of macOS state; we own activation, so we own the state.
- **Launcher UX:** `ArgChipBar` already collects the duration. The chip parser handles `30m`, `2h`, `1d`, `until 5pm`, `until tomorrow`, `until next meeting` (calendar-aware). Invoking DND while active offers **Extend +30m** or **Turn off now**.
- **Tray countdown:** reuses the `tray_countdown.rs` bus-subscriber pattern — menu bar shows "🌙 1h 23m" while a session is active.

**Non-goals:**
- Full Focus Modes picker (Personal / Work / Sleep). v1 ships DND only, but the table and API are keyed by `mode: String` so adding modes is data-only.
- Honoring user-created Focus schedules from System Settings.
- iPhone / cross-device Focus sync.

**Tech Stack:** Rust 1.93, Tokio, jiff (duration parsing), `TemporalScheduler` (existing), `DomainEventBus` (existing), SQLite/sqlx (existing), Tauri 2, React 18 + Vite + Bun.

**Reference spec:** (none — spec derived from this plan + the Focus subsystem subsection of `docs/superpowers/specs/2026-04-17-unified-temporal-scheduler-and-notifications-design.md`)

---

## File Map

### PR-1 — `focus-core` crate + `focus_sessions` table
- **Create:** `crates/feature-focus/Cargo.toml` — new crate, L4.
- **Create:** `crates/feature-focus/src/lib.rs` — `FocusManager`, `FocusSession`, `FocusMode`, public API.
- **Create:** `crates/feature-focus/src/repo.rs` — `FocusSessionRepo` (active session CRUD).
- **Create:** `crates/feature-focus/src/migrations/001_focus_sessions.sql`.
- **Create:** `crates/feature-focus/src/duration_parser.rs` — `parse_until(s, now, calendar) -> Timestamp` (handles `30m`, `2h`, `until 5pm`, `until tomorrow`, `until next meeting`).
- **Modify:** `Cargo.toml` (workspace) — add member.
- **Modify:** `src/lib.rs` (facade) — re-export `FocusManager`.

### PR-2 — Integrate with TemporalScheduler + DomainEventBus
- **Create:** `crates/feature-focus/src/alarm_bridge.rs` — schedules `AlarmRule::Absolute` on activate, cancels on deactivate/extend.
- **Create:** `crates/app-core/src/focus/end_subscriber.rs` — subscribes to `AlarmFired { kind: "focus_end" }`, calls `FocusManager::deactivate`.
- **Modify:** `crates/app-core/src/app_core.rs` — hold `Arc<FocusManager>`, start subscriber during init.

### PR-3 — macOS bridge + shortcut install
- **Create:** `crates/feature-focus/src/bridge/macos.rs` — `run_shortcut("Klyntbot Focus On" | "Off")`; stub for non-macOS.
- **Create:** `crates/feature-focus/assets/Klyntbot Focus On.shortcut` — binary-bundled macOS Shortcut.
- **Create:** `crates/feature-focus/assets/Klyntbot Focus Off.shortcut`.
- **Create:** `crates/desktop/src/commands/focus.rs` — `focus_install_shortcuts` (extracts + opens in Shortcuts.app), `focus_shortcuts_installed` (idempotency check via `shortcuts list`), `focus_activate`, `focus_deactivate`, `focus_extend`.
- **Modify:** `crates/desktop/src/main.rs` / `lib.rs` — register commands + `DEV_COMMANDS`.

### PR-4 — Launcher + tray UX
- **Modify:** `crates/feature-launcher/src/search/system_commands.rs` — `ToggleDoNotDisturb` command renamed `Focus` (v1 keeps DND label); when active, command title becomes "DND on — 1h 23m left" and chip arg becomes `Extend/Off`.
- **Modify:** `desktop-ui/src/features/launcher/components/ArgChipBar.tsx` — accept a `placeholderHint` prop; no structural change.
- **Create:** `desktop-ui/src/features/launcher/components/FocusActiveChip.tsx` — dropdown with `Extend +30m`, `Extend +2h`, `Turn off now`.
- **Modify:** `crates/desktop/src/tray_countdown.rs` — subscribe to `FocusSessionChanged` bus event; show 🌙 Xh Ym while active.
- **Create:** `desktop-ui/src/features/settings/components/FocusOnboarding.tsx` — first-use install panel triggered by failed activation.

---

# PR-1 — `focus-core` crate + storage

### Task 1.1: Scaffold `feature-focus` crate

**Files:**
- Create: `crates/feature-focus/Cargo.toml`
- Create: `crates/feature-focus/src/lib.rs`
- Modify: root `Cargo.toml` (workspace members)

- [ ] **Step 1: Cargo.toml**

```toml
[package]
name = "feature-focus"
version = "0.1.0"
edition = "2021"

[dependencies]
common = { path = "../common" }
storage = { path = "../storage" }
bus = { path = "../bus" }
scheduling = { path = "../scheduling" }
tools-core = { path = "../tools-core" }
jiff = { workspace = true }
serde = { workspace = true, features = ["derive"] }
sqlx = { workspace = true }
tokio = { workspace = true, features = ["sync"] }
async-trait = { workspace = true }
tracing = { workspace = true }
thiserror = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
```

- [ ] **Step 2: Workspace registration**

Add `"crates/feature-focus"` to the `members` array in root `Cargo.toml`.

- [ ] **Step 3: Initial lib.rs**

```rust
//! Focus Sessions — timed system-level actions (DND, silent mode, etc.).

pub mod duration_parser;
pub mod repo;

pub use repo::{FocusSession, FocusSessionRepo};

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FocusMode {
    Dnd,
}

impl FocusMode {
    pub fn as_str(&self) -> &'static str { match self { Self::Dnd => "dnd" } }
}
```

- [ ] **Step 4: Verify**

```bash
cargo check -p feature-focus
```

- [ ] **Step 5: Commit**

```bash
git add crates/feature-focus/ Cargo.toml Cargo.lock
git commit -m "feat(feature-focus): scaffold focus-sessions crate"
```

### Task 1.2: `focus_sessions` migration + repo

**Files:**
- Create: `crates/feature-focus/src/migrations/001_focus_sessions.sql`
- Create: `crates/feature-focus/src/repo.rs`

- [ ] **Step 1: Migration**

```sql
CREATE TABLE IF NOT EXISTS focus_sessions (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    mode        TEXT NOT NULL,
    started_at  TEXT NOT NULL,          -- RFC 3339
    ends_at     TEXT NOT NULL,
    ended_at    TEXT,                   -- NULL while active
    alarm_id    INTEGER,                -- FK to scheduled_fires.id
    source      TEXT NOT NULL DEFAULT 'launcher'
);

CREATE UNIQUE INDEX IF NOT EXISTS ix_focus_sessions_active
    ON focus_sessions(mode) WHERE ended_at IS NULL;
```

- [ ] **Step 2: Repo API**

`FocusSessionRepo`:
- `async fn active(&self, mode: FocusMode) -> Result<Option<FocusSession>>`
- `async fn insert_active(&self, mode, started_at, ends_at, alarm_id) -> Result<FocusSession>`
- `async fn end(&self, id: i64, ended_at: Timestamp) -> Result<()>`
- `async fn extend(&self, id: i64, new_ends_at: Timestamp, new_alarm_id: i64) -> Result<()>`

Match style of existing repos in `crates/feature-tasks/src/repo.rs`.

- [ ] **Step 3: Unit tests**

Tests per method using `StoragePool::connect_in_memory()` + migration loader. Cover: insert → active returns Some, end → active returns None, extend updates ends_at and alarm_id, unique-active constraint.

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(feature-focus): focus_sessions migration + repo"
```

### Task 1.3: Duration parser

**Files:**
- Create: `crates/feature-focus/src/duration_parser.rs`

- [ ] **Step 1: API**

```rust
/// Parse a user-entered duration string into an end timestamp.
///
/// Grammar:
///   <n>(s|m|h|d)        e.g. "30m", "2h", "1d"
///   until <clock>       e.g. "until 5pm", "until 17:30", "until noon"
///   until tomorrow      → tomorrow 09:00 local (configurable constant)
///   until next meeting  → end of next calendar event (requires calendar fn)
pub fn parse_until(
    input: &str,
    now: Timestamp,
    tz: &jiff::tz::TimeZone,
    next_meeting_end: impl FnOnce() -> Option<Timestamp>,
) -> Result<Timestamp, ParseError>;
```

- [ ] **Step 2: Tests**

One test per grammar branch, pinning the boundary cases:
- `"30m"` at a fixed `now` → `now + 30min`
- `"until 5pm"` at 2pm local → today 17:00; at 6pm local → tomorrow 17:00
- `"until tomorrow"` → tomorrow 09:00 local
- `"until next meeting"` with stubbed meeting → meeting end; with no meeting → `Err(NoMeeting)`
- invalid inputs → `Err(Grammar)`

- [ ] **Step 3: Commit**

```bash
git commit -m "feat(feature-focus): duration parser with calendar-aware 'until' forms"
```

### Task 1.4: PR-1 gates

```bash
cargo nextest run -p feature-focus
cargo clippy -p feature-focus --all-targets -- -D warnings
cargo fmt --all --check
```

Expected: all green. Commit any fmt/clippy fixes and open PR.

---

# PR-2 — Scheduler integration

### Task 2.1: `FocusManager` activate/deactivate/extend

**Files:**
- Create: `crates/feature-focus/src/manager.rs`
- Modify: `crates/feature-focus/src/lib.rs` (add `pub mod manager; pub use manager::FocusManager;`)

- [ ] **Step 1: API**

```rust
pub struct FocusManager {
    repo: FocusSessionRepo,
    scheduler: Arc<dyn FocusScheduler>,   // trait, impl'd in alarm_bridge.rs
    bridge:    Arc<dyn FocusBridge>,      // trait, impl'd in bridge/macos.rs
    bus:       Arc<DomainEventBus>,
}

impl FocusManager {
    pub async fn activate(&self, mode: FocusMode, ends_at: Timestamp) -> Result<FocusSession>;
    pub async fn deactivate(&self, mode: FocusMode) -> Result<()>;
    pub async fn extend(&self, mode: FocusMode, new_ends_at: Timestamp) -> Result<FocusSession>;
    pub async fn active(&self, mode: FocusMode) -> Result<Option<FocusSession>>;
}
```

Rules:
- `activate` when already-active for that mode → `Err(AlreadyActive)` (caller should call `extend`).
- `activate` = bridge.turn_on, scheduler.schedule → insert row, emit `DomainEvent::FocusSessionChanged`.
- `deactivate` = bridge.turn_off, scheduler.cancel(alarm_id), repo.end, emit event.
- `extend` = scheduler.cancel(old), scheduler.schedule(new), repo.extend, emit event.
- All bridge calls idempotent; failure → Err but state stays consistent (bridge contract).

- [ ] **Step 2: `FocusBridge` + `FocusScheduler` traits**

Trait stubs in `lib.rs`. Mock impls in `tests/` using in-memory state.

- [ ] **Step 3: Unit tests**

- `activate → active=Some, scheduler.schedule called once, bridge.turn_on called once`
- `activate then activate same mode → AlreadyActive`
- `activate then extend → alarm cancelled, new scheduled, ends_at updated`
- `activate then deactivate → active=None, bridge.turn_off called`
- All event emissions verified on the bus.

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(feature-focus): FocusManager activate/deactivate/extend"
```

### Task 2.2: Alarm bridge (real `FocusScheduler` impl)

**Files:**
- Create: `crates/feature-focus/src/alarm_bridge.rs`

- [ ] **Step 1: Impl**

```rust
pub struct TemporalAlarmBridge {
    scheduler: Arc<TemporalScheduler>,
}

#[async_trait]
impl FocusScheduler for TemporalAlarmBridge {
    async fn schedule(&self, fire_at: Timestamp) -> Result<i64 /* alarm_id */> {
        self.scheduler
            .schedule(AlarmRule::Absolute { fire_at }, AlarmMeta { kind: "focus_end".into(), .. })
            .await
    }
    async fn cancel(&self, alarm_id: i64) -> Result<()> { ... }
}
```

- [ ] **Step 2: Integration test**

Spins up `TemporalScheduler` + in-memory pool, activates a 1-second focus, asserts `AlarmFired { kind: "focus_end" }` is emitted after ~1s.

### Task 2.3: End subscriber in app-core

**Files:**
- Create: `crates/app-core/src/focus/end_subscriber.rs`
- Modify: `crates/app-core/src/app_core.rs`

- [ ] **Step 1: Subscriber**

```rust
pub fn spawn_focus_end_subscriber(
    manager: Arc<FocusManager>,
    bus: Arc<DomainEventBus>,
    shutdown: CancellationToken,
) -> JoinHandle<()>;
```

Listens on `bus.subscribe()`, filters `AlarmFired { kind == "focus_end" }`, calls `manager.deactivate(FocusMode::Dnd)`. Swallows + logs errors (deactivate should be idempotent).

- [ ] **Step 2: Wire into AppCore init**

`AppCore` gains `pub focus: Arc<FocusManager>`. `init_focus()` called after scheduler + bus are up.

- [ ] **Step 3: Integration test**

End-to-end: activate 500ms focus → wait → assert bridge.turn_off called, repo.active returns None.

### Task 2.4: PR-2 gates — same as 1.4

---

# PR-3 — macOS bridge + shortcut install

### Task 3.1: Ship the `.shortcut` files

**Files:**
- Create: `crates/feature-focus/assets/Klyntbot Focus On.shortcut`
- Create: `crates/feature-focus/assets/Klyntbot Focus Off.shortcut`

Build both in the macOS Shortcuts app:
- "Klyntbot Focus On" → **Set Focus** action, mode = Do Not Disturb, turn on, **until turned off** (klyntbot manages the timer).
- "Klyntbot Focus Off" → **Set Focus** action, mode = Do Not Disturb, turn off.

Export each via **File → Export** to `.shortcut` binary. Commit alongside a `README.md` in `assets/` documenting how to regenerate.

- [ ] **Step 1: Create + export both shortcuts**
- [ ] **Step 2: Embed via `include_bytes!` in `bridge/macos.rs`**
- [ ] **Step 3: Commit** — `feat(feature-focus): bundle macOS Focus On/Off shortcuts`

### Task 3.2: `FocusBridge` macOS impl

**Files:**
- Create: `crates/feature-focus/src/bridge/macos.rs`
- Create: `crates/feature-focus/src/bridge/mod.rs`

- [ ] **Step 1: API**

```rust
pub async fn run_shortcut(name: &str) -> Result<()>;
pub async fn is_shortcut_installed(name: &str) -> Result<bool>;
pub async fn install_bundled_shortcut(name: &str) -> Result<()>;  // extracts asset, `open`s in Shortcuts.app
```

Uses `tokio::process::Command` on `shortcuts` CLI (`shortcuts run`, `shortcuts list`). `open -a Shortcuts <path>` for install.

- [ ] **Step 2: `MacosFocusBridge`**

```rust
#[async_trait]
impl FocusBridge for MacosFocusBridge {
    async fn turn_on(&self, _: FocusMode) -> Result<()> {
        run_shortcut("Klyntbot Focus On").await
    }
    async fn turn_off(&self, _: FocusMode) -> Result<()> {
        run_shortcut("Klyntbot Focus Off").await
    }
    async fn is_ready(&self) -> Result<bool> {
        Ok(is_shortcut_installed("Klyntbot Focus On").await?
            && is_shortcut_installed("Klyntbot Focus Off").await?)
    }
}
```

- [ ] **Step 3: Non-macOS stub** returns `Unsupported`.

### Task 3.3: Tauri commands

**Files:**
- Create: `crates/desktop/src/commands/focus.rs`
- Modify: `crates/desktop/src/commands/mod.rs`
- Modify: `crates/desktop/src/main.rs` / `lib.rs` (register in `generate_handler!`)

Commands:
- `focus_install_shortcuts()` — extract both assets, open in Shortcuts.app
- `focus_shortcuts_installed() -> bool` — calls bridge.is_ready
- `focus_activate(mode, ends_at)` — delegates to `AppCore.focus.activate`
- `focus_deactivate(mode)` — delegates
- `focus_extend(mode, new_ends_at)` — delegates
- `focus_active(mode) -> Option<FocusSession>` — delegates

Don't forget `DEV_COMMANDS` + `dispatch_dev` + the `dev_server_covers_all_tauri_commands` test (CLAUDE.md gotcha).

### Task 3.4: PR-3 gates + manual smoke

Manual: launch Klyntbot → from launcher invoke DND → if shortcuts not installed, the activation fails with `ShortcutsNotReady` → frontend opens install panel → after two clicks in Shortcuts.app, DND arms with "2h" chip → check Control Center crescent-moon appears.

---

# PR-4 — Launcher + tray UX

### Task 4.1: Wire chip → `focus_activate`

**Files:**
- Modify: `desktop-ui/src/features/launcher/hooks/useExecuteItem.ts`
- Modify: `crates/feature-launcher/src/search/system_commands.rs`

The `ToggleDoNotDisturb` arm in `useExecuteItem.ts` currently just calls the toggle shortcut via `launcher_system_command`. Replace with:

```ts
case "systemCommand":
  if (item.kind.action === "toggleDoNotDisturb") {
    const end = parseUntil(args.duration);
    ipc("focus_activate", { mode: "dnd", endsAt: end }).then(...)
  } else { /* existing path */ }
```

(Or cleaner: add a dedicated `FocusCommand` item kind emitted by `SystemCommands::search` only when shortcuts are installed. See Task 4.3.)

### Task 4.2: Active-state surfacing

**Files:**
- Create: `desktop-ui/src/features/launcher/components/FocusActiveChip.tsx`

When user invokes DND while already active:
- Launcher item title becomes **"DND on — 1h 23m left"**
- Chip bar is replaced by `FocusActiveChip` with three buttons: **Extend +30m**, **Extend +2h**, **Turn off now**.
- Feed via `useQuery("focus_active", { mode: "dnd" })` polling every 30s.

### Task 4.3: First-use onboarding

**Files:**
- Create: `desktop-ui/src/features/settings/components/FocusOnboarding.tsx`
- Modify: `desktop-ui/src/features/launcher/hooks/useExecuteItem.ts`

If `focus_activate` fails with `ShortcutsNotReady`:
- Open a settings panel explaining the one-time install
- Button: "Install Focus Shortcuts" → calls `focus_install_shortcuts` → shows "Once you've tapped **Add Shortcut** in both tabs, click Done"
- Done button: calls `focus_shortcuts_installed`; if true, retries the activation automatically.

### Task 4.4: Tray countdown

**Files:**
- Modify: `crates/desktop/src/tray_countdown.rs`

Add a bus subscriber for `DomainEvent::FocusSessionChanged`. When an active session exists, the tray title precedence becomes: FocusSession > ActiveTask > NextCalendarEvent (current priority). Format: `🌙 1h 23m`.

Reuse the existing `dirty` + `notify` pattern — no new subscriber infrastructure.

### Task 4.5: PR-4 gates

```bash
cargo nextest run --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
cd desktop-ui && bun run lint && bun run test && bun run build
```

**Manual smoke** (the acceptance test):
1. Cold launch Klyntbot.
2. Open launcher → type `dnd` → type `2h` in chip → Enter.
3. First-use onboarding appears → install shortcuts → return.
4. Control Center crescent moon appears.
5. Tray title shows `🌙 1h 59m` (decrementing).
6. Open launcher → type `dnd` → sees "DND on — 1h 59m left" → chooses "Turn off now".
7. Crescent moon disappears within 1s; tray reverts to default title.
8. Quit + relaunch klyntbot during a 10-minute DND → tray resumes countdown from persisted state.

### Task 4.6: PR

```bash
gh pr create --title "feat(focus): DND-with-duration via Focus Sessions subsystem" \
             --body "Implements the four Focus Session PRs."
```

---

## Self-Review Checklist (engineer to confirm before each PR ships)

- [ ] Migration is idempotent (`CREATE TABLE IF NOT EXISTS`, `INSERT OR IGNORE`).
- [ ] `DEV_COMMANDS` updated for every new `#[tauri::command]`.
- [ ] `FocusManager` activate is atomic under the bridge contract (no orphaned alarms if the DB insert fails).
- [ ] The `focus_sessions.active` unique index prevents double-activation races.
- [ ] Tray countdown doesn't regress: polling is still event-driven (no wall-clock loops).
- [ ] Shortcut assets are committed as LFS or checked they're <100KB (`.shortcut` files are typically ~5KB).
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` = 0.
- [ ] Zero raw `tokio::spawn` for end-of-session — must go through the scheduler (durability invariant).

---

## Acceptance Criteria

- Launcher DND command with `2h` chip actually turns DND on and back off 2h later, surviving restart — manual check.
- `until next meeting` with a meeting starting in 10 min / ending in 30 min activates DND for 30 min — manual check.
- Invoking DND while active shows remaining time and offers extend/off — manual check.
- Tray shows a visible countdown while active — manual check.
- First-use onboarding completes in ≤2 clicks in Shortcuts.app — manual check.
- All four PRs ship independently green; main branch never broken.
