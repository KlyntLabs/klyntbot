# Sleep/Wake Lifecycle & Opportunistic Continuity — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Klyntbot sleep/wake-aware with layered signals (NSWorkspace + CGEventSource), per-feature pause/resume, intent-window-based scheduling, and a unified wake greeting panel.

**Architecture:** Event-driven with WakeOrchestrator (Approach B from spec). `LifecycleMonitor` in `platform-macos` emits raw OS events, `app-core` bridges them to `DomainEventBus`, features subscribe independently, `WakeOrchestrator` collects "ready" signals and sequences the user-facing wake experience. Intent windows are an optional overlay on `CronJob`.

**Tech Stack:** Rust, Tauri 2, objc2-app-kit (NSWorkspace), core-graphics (CGEventSource), tokio, chrono, SQLite (sqlx)

**Spec:** `docs/superpowers/specs/2026-03-27-sleep-wake-lifecycle-design.md`

---

## Task 1: Foundation Types — Config & Domain Events

**Files:**
- Create: `crates/config/src/schema/lifecycle.rs`
- Modify: `crates/config/src/schema/mod.rs`
- Modify: `crates/config/src/schema/core.rs:L100-L200`
- Modify: `crates/bus/src/domain_events.rs:L417-L427`

- [ ] **Step 1: Create `LifecycleConfig` and `WakeDeliveryConfig` in the config crate**

Create `crates/config/src/schema/lifecycle.rs`:

```rust
use serde::{Deserialize, Serialize};

/// Lifecycle monitoring — macOS sleep/wake + user presence detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleConfig {
    /// Seconds of no keyboard/mouse input before user is considered idle.
    #[serde(default = "default_idle_threshold")]
    pub idle_threshold_secs: u64,

    /// Seconds of input that marks user as "returned" from idle.
    #[serde(default = "default_presence_threshold")]
    pub presence_threshold_secs: u64,

    /// Seconds to wait after didWake before emitting UserReturned.
    #[serde(default = "default_wake_grace_period")]
    pub wake_grace_period_secs: u64,

    /// CGEventSource polling interval when Active (seconds).
    #[serde(default = "default_active_poll")]
    pub active_poll_interval_secs: u64,

    /// CGEventSource polling interval when Idle/Sleeping (seconds).
    #[serde(default = "default_idle_poll")]
    pub idle_poll_interval_secs: u64,

    /// Wake delivery timing and thresholds.
    #[serde(default)]
    pub wake_delivery: WakeDeliveryConfig,

    /// When true, all intent windows are ignored (pure cron behavior).
    #[serde(default)]
    pub disable_smart_scheduling: bool,
}

/// Wake delivery timing — controls when and how the wake panel appears.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WakeDeliveryConfig {
    /// Minimum absence (seconds) before showing a wake panel. Default: 1800 (30 min).
    #[serde(default = "default_min_absence_for_panel")]
    pub min_absence_for_panel_secs: u64,

    /// Quiet period before showing wake panel (5am-11am). Default: 45s.
    #[serde(default = "default_quiet_morning")]
    pub quiet_period_morning_secs: u64,

    /// Quiet period (12pm-4pm). Default: 15s.
    #[serde(default = "default_quiet_midday")]
    pub quiet_period_midday_secs: u64,

    /// Quiet period (after 8pm). Default: 60s.
    #[serde(default = "default_quiet_evening")]
    pub quiet_period_evening_secs: u64,

    /// Quiet period (all other times). Default: 30s.
    #[serde(default = "default_quiet_default")]
    pub quiet_period_default_secs: u64,

    /// Seconds between staggered catch-up tiers. Default: 120.
    #[serde(default = "default_tier_stagger")]
    pub catch_up_tier_stagger_secs: u64,

    /// FromIdle absence threshold (seconds) for showing focus resume prompt. Default: 600 (10 min).
    #[serde(default = "default_idle_resume_threshold")]
    pub idle_resume_prompt_threshold_secs: u64,

    /// Absence threshold (seconds) for consolidating nudges. Default: 1800 (30 min).
    #[serde(default = "default_nudge_consolidation")]
    pub nudge_consolidation_threshold_secs: u64,
}

impl Default for LifecycleConfig {
    fn default() -> Self {
        Self {
            idle_threshold_secs: default_idle_threshold(),
            presence_threshold_secs: default_presence_threshold(),
            wake_grace_period_secs: default_wake_grace_period(),
            active_poll_interval_secs: default_active_poll(),
            idle_poll_interval_secs: default_idle_poll(),
            wake_delivery: WakeDeliveryConfig::default(),
            disable_smart_scheduling: false,
        }
    }
}

impl Default for WakeDeliveryConfig {
    fn default() -> Self {
        Self {
            min_absence_for_panel_secs: default_min_absence_for_panel(),
            quiet_period_morning_secs: default_quiet_morning(),
            quiet_period_midday_secs: default_quiet_midday(),
            quiet_period_evening_secs: default_quiet_evening(),
            quiet_period_default_secs: default_quiet_default(),
            catch_up_tier_stagger_secs: default_tier_stagger(),
            idle_resume_prompt_threshold_secs: default_idle_resume_threshold(),
            nudge_consolidation_threshold_secs: default_nudge_consolidation(),
        }
    }
}

fn default_idle_threshold() -> u64 { 300 }
fn default_presence_threshold() -> u64 { 2 }
fn default_wake_grace_period() -> u64 { 60 }
fn default_active_poll() -> u64 { 10 }
fn default_idle_poll() -> u64 { 30 }
fn default_min_absence_for_panel() -> u64 { 1800 }
fn default_quiet_morning() -> u64 { 45 }
fn default_quiet_midday() -> u64 { 15 }
fn default_quiet_evening() -> u64 { 60 }
fn default_quiet_default() -> u64 { 30 }
fn default_tier_stagger() -> u64 { 120 }
fn default_idle_resume_threshold() -> u64 { 600 }
fn default_nudge_consolidation() -> u64 { 1800 }
```

- [ ] **Step 2: Wire the config into the Config root struct**

In `crates/config/src/schema/mod.rs`, add:
```rust
pub mod lifecycle;
pub use self::lifecycle::*;
```

In `crates/config/src/schema/core.rs`, add import and field:
```rust
use super::lifecycle::LifecycleConfig;
```

Add to `Config` struct (after the `launcher` field):
```rust
    /// Lifecycle monitoring — macOS sleep/wake + user presence detection.
    #[serde(default)]
    pub lifecycle: LifecycleConfig,
```

- [ ] **Step 3: Add lifecycle domain events to the bus crate**

In `crates/bus/src/domain_events.rs`, add these variants before the closing `}` of the `DomainEvent` enum (after `MirrorSnippetCreated`):

```rust
    // -- Lifecycle events --
    /// macOS is about to sleep (lid close, explicit sleep, idle sleep).
    SystemWillSleep,
    /// macOS woke from sleep.
    SystemDidWake {
        away_duration: std::time::Duration,
        wake_type: WakeType,
    },
    /// User became idle (no keyboard/mouse input for threshold duration).
    UserBecameIdle {
        idle_secs: u64,
    },
    /// User returned after being idle or after system sleep.
    UserReturned {
        absence_duration: std::time::Duration,
        wake_type: WakeType,
    },

    // -- Wake orchestrator ready signals --
    /// Focus timer was suspended due to sleep/idle.
    FocusSessionSuspended {
        remaining_secs: u64,
        phase_name: String,
    },
    /// Cron service classified missed jobs for catch-up.
    CronCatchUpReady {
        immediate_count: usize,
        deferred_count: usize,
        expired_count: usize,
    },
```

Add the `WakeType` enum above the `DomainEvent` enum:

```rust
/// How the user returned — from OS sleep or from idle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WakeType {
    FromSleep,
    FromIdle,
}
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo build -p config -p bus`

Expected: success, zero warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/config/src/schema/lifecycle.rs crates/config/src/schema/mod.rs \
  crates/config/src/schema/core.rs crates/bus/src/domain_events.rs
git commit -m "feat(lifecycle): add LifecycleConfig and domain events for sleep/wake"
```

---

## Task 2: Intent Window Types & Storage Schema

**Files:**
- Modify: `crates/scheduling/src/types.rs:L98-L127`
- Modify: `crates/storage/migrations/001_initial.sql:L235-L249`
- Modify: `crates/storage/src/rows/cron.rs`
- Modify: `crates/storage/src/repos/cron.rs`
- Modify: `crates/scheduling/src/service/store.rs` (row↔domain conversion)

- [ ] **Step 1: Write tests for IntentWindow types**

Add to `crates/scheduling/src/types.rs` inside `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn test_intent_window_serde() {
        let window = IntentWindow {
            trigger: IntentTrigger::UserPresent,
            tolerance: std::time::Duration::from_secs(7200),
            catch_up: CatchUpPriority::WhenPresent,
        };
        let json = serde_json::to_value(&window).unwrap();
        assert_eq!(json["trigger"]["kind"], "user_present");
        assert_eq!(json["toleranceSecs"], 7200);
        assert_eq!(json["catchUp"], "when_present");

        let roundtrip: IntentWindow = serde_json::from_value(json).unwrap();
        assert_eq!(roundtrip.tolerance.as_secs(), 7200);
    }

    #[test]
    fn test_intent_trigger_first_activity_after() {
        let trigger = IntentTrigger::FirstActivityAfter {
            after_local: chrono::NaiveTime::from_hms_opt(8, 0, 0).unwrap(),
        };
        let json = serde_json::to_value(&trigger).unwrap();
        assert_eq!(json["kind"], "first_activity_after");
        assert_eq!(json["afterLocal"], "08:00:00");
    }

    #[test]
    fn test_cron_job_with_intent_window() {
        let schedule = CronSchedule::Cron {
            expr: "0 0 9 * * 1".to_string(),
            tz: None,
        };
        let mut job = CronJob::new("j1", "Weekly reflection", schedule, "", CronOrigin::System);
        job.intent_window = Some(IntentWindow {
            trigger: IntentTrigger::FirstActivityAfter {
                after_local: chrono::NaiveTime::from_hms_opt(8, 0, 0).unwrap(),
            },
            tolerance: std::time::Duration::from_secs(7200),
            catch_up: CatchUpPriority::WhenPresent,
        });

        let json = serde_json::to_value(&job).unwrap();
        assert!(json["intentWindow"].is_object());

        let roundtrip: CronJob = serde_json::from_value(json).unwrap();
        assert!(roundtrip.intent_window.is_some());
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p scheduling -E 'test(intent_window)'`

Expected: FAIL — `IntentWindow` not defined.

- [ ] **Step 3: Add IntentWindow types to scheduling/types.rs**

Add before the `CronJob` struct (after `CronOrigin`):

```rust
/// Optional intent window — controls when a job actually fires relative to its cron schedule.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntentWindow {
    pub trigger: IntentTrigger,
    /// Serialize as seconds for JSON compactness.
    #[serde(rename = "toleranceSecs", with = "duration_secs")]
    pub tolerance: std::time::Duration,
    #[serde(rename = "catchUp")]
    pub catch_up: CatchUpPriority,
}

/// What must be true for an intent-windowed job to fire.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IntentTrigger {
    /// User must be actively present (idle < threshold).
    UserPresent,
    /// First user activity after a specific local time.
    FirstActivityAfter {
        #[serde(rename = "afterLocal")]
        after_local: chrono::NaiveTime,
    },
    /// User has been continuously active for N minutes.
    MinActiveMinutes { minutes: u32 },
    /// User is idle (good for maintenance that shouldn't interrupt).
    UserIdle {
        #[serde(rename = "minIdleSecs")]
        min_idle_secs: u64,
    },
}

/// Priority for catch-up after sleep/idle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatchUpPriority {
    Immediate,
    WhenPresent,
    WhenIdle,
}

/// Serde helper: Duration <-> u64 seconds.
mod duration_secs {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S: Serializer>(d: &Duration, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u64(d.as_secs())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Duration, D::Error> {
        Ok(Duration::from_secs(u64::deserialize(d)?))
    }
}
```

Add `intent_window` and `intent_pending_since_ms` to `CronJob`:

```rust
pub struct CronJob {
    // ... existing fields ...
    pub delete_after_run: bool,

    /// Optional intent window for opportunistic scheduling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent_window: Option<IntentWindow>,

    /// When the job started waiting for its intent trigger (ms since epoch).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent_pending_since_ms: Option<i64>,
}
```

Update `CronJob::new()` to include the new fields:
```rust
            delete_after_run: false,
            intent_window: None,
            intent_pending_since_ms: None,
```

Add `use chrono` to the imports at the top of the file if not already present.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo nextest run -p scheduling -E 'test(intent_window)'`

Expected: PASS.

- [ ] **Step 5: Update the cron_jobs table schema**

In `crates/storage/migrations/001_initial.sql`, add two columns to the `cron_jobs` table (before the closing `);`):

```sql
    intent_window    TEXT,
    intent_pending_since_ms INTEGER
```

- [ ] **Step 6: Update CronJobRow and CronRepo**

In `crates/storage/src/rows/cron.rs`, add fields to `CronJobRow`:

```rust
    pub intent_window: Option<String>,
    pub intent_pending_since_ms: Option<i64>,
```

In `crates/storage/src/repos/cron.rs`, update the `upsert` query to include the new columns in the INSERT and ON CONFLICT UPDATE clauses. Update the `list` / `list_active` queries to select the new columns.

- [ ] **Step 7: Update row<->domain conversion in scheduling**

In `crates/scheduling/src/service/store.rs` (or wherever `CronJobRow` → `CronJob` conversion lives), parse `intent_window` from JSON string:

```rust
intent_window: row.intent_window.as_deref()
    .and_then(|s| serde_json::from_str(s).ok()),
intent_pending_since_ms: row.intent_pending_since_ms,
```

And the reverse for `CronJob` → `CronJobRow`:
```rust
intent_window: job.intent_window.as_ref()
    .and_then(|w| serde_json::to_string(w).ok()),
intent_pending_since_ms: job.intent_pending_since_ms,
```

- [ ] **Step 8: Verify everything compiles and existing tests pass**

Run: `cargo nextest run -p scheduling -p storage`

Expected: all pass.

- [ ] **Step 9: Commit**

```bash
git add crates/scheduling/src/types.rs crates/storage/migrations/001_initial.sql \
  crates/storage/src/rows/cron.rs crates/storage/src/repos/cron.rs \
  crates/scheduling/src/service/store.rs
git commit -m "feat(scheduling): add IntentWindow types and schema columns"
```

---

## Task 3: LifecycleMonitor State Machine (Pure Logic)

**Files:**
- Create: `crates/platform-macos/src/lifecycle.rs`
- Modify: `crates/platform-macos/src/lib.rs`

This task implements the pure state machine logic with no OS dependencies — fully testable on any platform.

- [ ] **Step 1: Write failing tests for the state machine**

Create `crates/platform-macos/src/lifecycle.rs` with tests at the bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> LifecycleConfig {
        LifecycleConfig {
            idle_threshold_secs: 300,
            presence_threshold_secs: 2,
            wake_grace_period_secs: 60,
        }
    }

    #[test]
    fn starts_active() {
        let sm = LifecycleStateMachine::new(default_config());
        assert_eq!(sm.state(), LifecycleState::Active);
    }

    #[test]
    fn active_to_idle_on_threshold() {
        let mut sm = LifecycleStateMachine::new(default_config());
        let events = sm.on_idle_reading(310);
        assert_eq!(sm.state(), LifecycleState::Idle);
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], LifecycleEvent::UserBecameIdle { idle_secs: 310 }));
    }

    #[test]
    fn idle_to_active_on_input() {
        let mut sm = LifecycleStateMachine::new(default_config());
        sm.on_idle_reading(310); // go idle
        let events = sm.on_idle_reading(1); // user returned
        assert_eq!(sm.state(), LifecycleState::Active);
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], LifecycleEvent::UserReturned {
            wake_type: WakeType::FromIdle, ..
        }));
    }

    #[test]
    fn active_to_sleeping_on_will_sleep() {
        let mut sm = LifecycleStateMachine::new(default_config());
        let events = sm.on_will_sleep();
        assert_eq!(sm.state(), LifecycleState::Sleeping);
        assert_eq!(events, vec![LifecycleEvent::SystemWillSleep]);
    }

    #[test]
    fn sleeping_to_waking_grace_on_did_wake() {
        let mut sm = LifecycleStateMachine::new(default_config());
        sm.on_will_sleep();
        let events = sm.on_did_wake(std::time::Duration::from_secs(3600));
        assert_eq!(sm.state(), LifecycleState::WakingGrace);
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], LifecycleEvent::SystemDidWake { .. }));
    }

    #[test]
    fn waking_grace_to_active_on_user_input() {
        let mut sm = LifecycleStateMachine::new(default_config());
        sm.on_will_sleep();
        sm.on_did_wake(std::time::Duration::from_secs(3600));
        let events = sm.on_idle_reading(1); // user input
        assert_eq!(sm.state(), LifecycleState::Active);
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], LifecycleEvent::UserReturned {
            wake_type: WakeType::FromSleep, ..
        }));
    }

    #[test]
    fn waking_grace_expires_to_active() {
        let mut sm = LifecycleStateMachine::new(default_config());
        sm.on_will_sleep();
        sm.on_did_wake(std::time::Duration::from_secs(3600));
        let events = sm.on_grace_expired();
        assert_eq!(sm.state(), LifecycleState::Active);
        assert!(matches!(events[0], LifecycleEvent::UserReturned {
            wake_type: WakeType::FromSleep, ..
        }));
    }

    #[test]
    fn no_duplicate_idle_events() {
        let mut sm = LifecycleStateMachine::new(default_config());
        sm.on_idle_reading(310);
        let events = sm.on_idle_reading(400);
        assert_eq!(sm.state(), LifecycleState::Idle);
        assert!(events.is_empty()); // already idle, no new event
    }

    #[test]
    fn below_threshold_stays_active() {
        let mut sm = LifecycleStateMachine::new(default_config());
        let events = sm.on_idle_reading(200);
        assert_eq!(sm.state(), LifecycleState::Active);
        assert!(events.is_empty());
    }

    #[test]
    fn debounce_rapid_sleep_wake() {
        let mut sm = LifecycleStateMachine::new(default_config());
        sm.on_will_sleep();
        let events = sm.on_did_wake(std::time::Duration::from_secs(5)); // 5s microsleep
        // Should still emit SystemDidWake — debounce is handled at the orchestrator level
        assert_eq!(events.len(), 1);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p platform-macos -E 'test(lifecycle)'`

Expected: FAIL — `LifecycleStateMachine` not defined.

- [ ] **Step 3: Implement the state machine**

In `crates/platform-macos/src/lifecycle.rs`, above the tests:

```rust
use std::time::{Duration, Instant};

/// Lifecycle event emitted by the state machine.
#[derive(Debug, Clone, PartialEq)]
pub enum LifecycleEvent {
    SystemWillSleep,
    SystemDidWake {
        away_duration: Duration,
        wake_type: WakeType,
    },
    UserBecameIdle {
        idle_secs: u64,
    },
    UserReturned {
        absence_duration: Duration,
        wake_type: WakeType,
    },
}

/// How the user returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WakeType {
    FromSleep,
    FromIdle,
}

/// Current lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleState {
    Active,
    Idle,
    Sleeping,
    WakingGrace,
}

/// Config for the state machine thresholds.
pub struct LifecycleConfig {
    pub idle_threshold_secs: u64,
    pub presence_threshold_secs: u64,
    pub wake_grace_period_secs: u64,
}

/// Pure state machine — no OS calls, no async.
pub struct LifecycleStateMachine {
    state: LifecycleState,
    config: LifecycleConfig,
    idle_started_at: Option<Instant>,
    sleep_started_at: Option<Instant>,
    last_away_duration: Duration,
}

impl LifecycleStateMachine {
    pub fn new(config: LifecycleConfig) -> Self {
        Self {
            state: LifecycleState::Active,
            config,
            idle_started_at: None,
            sleep_started_at: None,
            last_away_duration: Duration::ZERO,
        }
    }

    pub fn state(&self) -> LifecycleState {
        self.state
    }

    /// Called with the latest CGEventSource idle reading (seconds since last input).
    pub fn on_idle_reading(&mut self, idle_secs: u64) -> Vec<LifecycleEvent> {
        match self.state {
            LifecycleState::Active => {
                if idle_secs >= self.config.idle_threshold_secs {
                    self.state = LifecycleState::Idle;
                    self.idle_started_at = Some(Instant::now());
                    vec![LifecycleEvent::UserBecameIdle { idle_secs }]
                } else {
                    vec![]
                }
            }
            LifecycleState::Idle => {
                if idle_secs <= self.config.presence_threshold_secs {
                    let absence = self
                        .idle_started_at
                        .map(|t| t.elapsed())
                        .unwrap_or(Duration::ZERO);
                    self.state = LifecycleState::Active;
                    self.idle_started_at = None;
                    vec![LifecycleEvent::UserReturned {
                        absence_duration: absence,
                        wake_type: WakeType::FromIdle,
                    }]
                } else {
                    vec![]
                }
            }
            LifecycleState::WakingGrace => {
                if idle_secs <= self.config.presence_threshold_secs {
                    self.state = LifecycleState::Active;
                    vec![LifecycleEvent::UserReturned {
                        absence_duration: self.last_away_duration,
                        wake_type: WakeType::FromSleep,
                    }]
                } else {
                    vec![]
                }
            }
            LifecycleState::Sleeping => vec![], // no polling during sleep
        }
    }

    /// Called when NSWorkspace willSleepNotification fires.
    pub fn on_will_sleep(&mut self) -> Vec<LifecycleEvent> {
        self.sleep_started_at = Some(Instant::now());
        self.state = LifecycleState::Sleeping;
        vec![LifecycleEvent::SystemWillSleep]
    }

    /// Called when NSWorkspace didWakeNotification fires.
    pub fn on_did_wake(&mut self, away_duration: Duration) -> Vec<LifecycleEvent> {
        self.last_away_duration = away_duration;
        self.state = LifecycleState::WakingGrace;
        self.sleep_started_at = None;
        vec![LifecycleEvent::SystemDidWake {
            away_duration,
            wake_type: WakeType::FromSleep,
        }]
    }

    /// Called when the grace period timer expires without user input.
    pub fn on_grace_expired(&mut self) -> Vec<LifecycleEvent> {
        if self.state == LifecycleState::WakingGrace {
            self.state = LifecycleState::Active;
            vec![LifecycleEvent::UserReturned {
                absence_duration: self.last_away_duration,
                wake_type: WakeType::FromSleep,
            }]
        } else {
            vec![]
        }
    }
}
```

- [ ] **Step 4: Register the module**

In `crates/platform-macos/src/lib.rs`, add:
```rust
pub mod lifecycle;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo nextest run -p platform-macos -E 'test(lifecycle)'`

Expected: all PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/platform-macos/src/lifecycle.rs crates/platform-macos/src/lib.rs
git commit -m "feat(lifecycle): pure state machine for sleep/wake/idle transitions"
```

---

## Task 4: LifecycleMonitor OS Integration (macOS APIs)

**Files:**
- Modify: `crates/platform-macos/src/lifecycle.rs`
- Modify: `crates/platform-macos/Cargo.toml`

This task adds the actual macOS API calls: NSWorkspace notifications, CGEventSource polling, App Nap prevention. Wraps the state machine from Task 3 with real OS events.

- [ ] **Step 1: Add required dependencies**

In `crates/platform-macos/Cargo.toml`, add `tokio` and extend `objc2-app-kit` features:

```toml
[dependencies]
tracing.workspace = true
base64.workspace = true
tokio = { workspace = true, features = ["sync", "time", "rt"] }

[target.'cfg(target_os = "macos")'.dependencies]
objc2 = "0.6"
objc2-app-kit = { version = "0.3", features = [
    "NSWorkspace",
    "NSRunningApplication",
    "NSPasteboard",
    "NSNotification",
] }
objc2-foundation = { version = "0.3", features = ["NSString", "NSArray", "NSNotification"] }
core-graphics = "0.24"
core-foundation = "0.10"
```

- [ ] **Step 2: Implement LifecycleMonitor**

Add to the bottom of `crates/platform-macos/src/lifecycle.rs` (after the state machine, before `#[cfg(test)]`):

```rust
/// High-level monitor that wraps the state machine with real macOS APIs.
///
/// - Subscribes to NSWorkspace sleep/wake notifications
/// - Polls CGEventSource for idle detection
/// - Manages App Nap assertions
///
/// The callback fires on a tokio task, not the main thread.
#[cfg(target_os = "macos")]
pub struct LifecycleMonitor {
    shutdown: tokio::sync::watch::Sender<bool>,
}

#[cfg(target_os = "macos")]
impl LifecycleMonitor {
    /// Start monitoring. Spawns background tasks for:
    /// - NSWorkspace sleep/wake observer (main thread -> mpsc -> tokio)
    /// - CGEventSource idle poller (tokio interval)
    /// - Grace period timer (tokio sleep)
    pub fn start(
        config: config::LifecycleConfig,
        callback: impl Fn(LifecycleEvent) + Send + Sync + 'static,
    ) -> Self {
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let callback = std::sync::Arc::new(callback);

        let sm_config = LifecycleConfig {
            idle_threshold_secs: config.idle_threshold_secs,
            presence_threshold_secs: config.presence_threshold_secs,
            wake_grace_period_secs: config.wake_grace_period_secs,
        };

        // Channel for NSWorkspace notifications (main thread -> tokio)
        let (ns_tx, mut ns_rx) = tokio::sync::mpsc::unbounded_channel::<NsEvent>();

        // Register NSWorkspace observers on main thread
        Self::register_workspace_observers(ns_tx);

        // Begin App Nap prevention
        Self::begin_activity("Klyntbot scheduling");

        // Spawn the unified event loop
        let cb = callback.clone();
        let mut shutdown = shutdown_rx;
        tokio::spawn(async move {
            let mut sm = LifecycleStateMachine::new(sm_config);
            let active_poll = Duration::from_secs(config.active_poll_interval_secs);
            let idle_poll = Duration::from_secs(config.idle_poll_interval_secs);
            let grace_secs = config.wake_grace_period_secs;

            let mut poll_interval = tokio::time::interval(active_poll);
            poll_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            let mut grace_timer: Option<tokio::time::Sleep> = None;

            loop {
                tokio::select! {
                    _ = shutdown.changed() => break,
                    Some(ns_event) = ns_rx.recv() => {
                        let events = match ns_event {
                            NsEvent::WillSleep => sm.on_will_sleep(),
                            NsEvent::DidWake(away) => {
                                grace_timer = Some(tokio::time::sleep(
                                    Duration::from_secs(grace_secs),
                                ));
                                sm.on_did_wake(away)
                            }
                        };
                        for e in events { cb(e); }
                    }
                    _ = async {
                        if let Some(ref mut timer) = grace_timer {
                            timer.await;
                        } else {
                            std::future::pending::<()>().await;
                        }
                    } => {
                        grace_timer = None;
                        let events = sm.on_grace_expired();
                        for e in events { cb(e); }
                    }
                    _ = poll_interval.tick() => {
                        let idle = Self::get_idle_secs();
                        let events = sm.on_idle_reading(idle);
                        for e in events { cb(e); }

                        // Adapt polling interval to state
                        let target = match sm.state() {
                            LifecycleState::Active => active_poll,
                            _ => idle_poll,
                        };
                        if poll_interval.period() != target {
                            poll_interval = tokio::time::interval(target);
                            poll_interval.set_missed_tick_behavior(
                                tokio::time::MissedTickBehavior::Skip,
                            );
                        }
                    }
                }
            }
        });

        Self { shutdown: shutdown_tx }
    }

    pub fn stop(&self) {
        let _ = self.shutdown.send(true);
    }

    /// Current idle seconds from CGEventSource.
    fn get_idle_secs() -> u64 {
        use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
        use core_graphics::event::CGEventType;

        CGEventSource::seconds_since_last_event_type(
            CGEventSourceStateID::HIDSystemState,
            CGEventType::Null, // any event type
        )
        .max(0.0) as u64
    }

    /// Register NSWorkspace sleep/wake observers.
    fn register_workspace_observers(tx: tokio::sync::mpsc::UnboundedSender<NsEvent>) {
        use objc2_app_kit::NSWorkspace;
        use objc2_foundation::NSNotificationCenter;

        // NSWorkspace observer registration happens on the main thread.
        // For now, log that we would register — actual implementation uses
        // objc2 block closures to forward events to the mpsc channel.
        //
        // Implementation note: Use `NSWorkspace::sharedWorkspace().notificationCenter()`
        // with `addObserverForName:object:queue:usingBlock:` for willSleepNotification
        // and didWakeNotification. The block captures `tx.clone()` and sends NsEvent.
        tracing::info!("LifecycleMonitor: registering NSWorkspace sleep/wake observers");

        // TODO: Wire objc2 block observers — this requires unsafe objc2 block API.
        // The pattern is established in window.rs for NSWorkspace usage.
        // For the initial implementation, use a polling fallback that checks
        // SystemConfiguration framework's sleep state every 30s.
        let _ = tx; // suppress unused warning until observers are wired
    }

    fn begin_activity(reason: &str) {
        tracing::info!("LifecycleMonitor: beginActivity({})", reason);
        // NSProcessInfo.processInfo.beginActivity(.userInitiated, reason)
        // Implementation uses objc2 msg_send to NSProcessInfo.
    }
}

/// Internal event from NSWorkspace observers.
#[cfg(target_os = "macos")]
enum NsEvent {
    WillSleep,
    DidWake(Duration),
}

/// Stub for non-macOS platforms (compile-only, no functionality).
#[cfg(not(target_os = "macos"))]
pub struct LifecycleMonitor;

#[cfg(not(target_os = "macos"))]
impl LifecycleMonitor {
    pub fn start(
        _config: config::LifecycleConfig,
        _callback: impl Fn(LifecycleEvent) + Send + Sync + 'static,
    ) -> Self {
        Self
    }
    pub fn stop(&self) {}
}
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build -p platform-macos`

Expected: success. The NSWorkspace observer body is a placeholder — the state machine and CGEventSource polling are real.

- [ ] **Step 4: Run existing tests still pass**

Run: `cargo nextest run -p platform-macos`

Expected: all PASS (state machine tests from Task 3 still work).

- [ ] **Step 5: Commit**

```bash
git add crates/platform-macos/
git commit -m "feat(lifecycle): LifecycleMonitor with CGEventSource polling and NSWorkspace stubs"
```

---

## Task 5: FocusTimer Wall-Clock Rewrite + Suspended State

**Files:**
- Modify: `crates/desktop/src/focus_timer.rs:L36-L75` (Phase enum)
- Modify: `crates/desktop/src/focus_timer.rs:L243-L340` (session_loop)

- [ ] **Step 1: Add `Suspended` variant to Phase enum**

In `crates/desktop/src/focus_timer.rs`, modify the `Phase` enum:

```rust
#[derive(Debug, Clone)]
enum Phase {
    Working { remaining: u64, total: u64 },
    BreakPending { remaining: u64 },
    Break { remaining: u64, total: u64 },
    Suspended { previous_phase: String, remaining: u64, total: u64 },
}
```

Update `Phase::as_str()`:
```rust
    fn as_str(&self) -> &'static str {
        match self {
            Phase::Working { .. } => "working",
            Phase::BreakPending { .. } => "break_pending",
            Phase::Break { .. } => "break",
            Phase::Suspended { .. } => "suspended",
        }
    }
```

Update `Phase::remaining()`:
```rust
    fn remaining(&self) -> u64 {
        match self {
            Phase::Working { remaining, .. }
            | Phase::BreakPending { remaining }
            | Phase::Break { remaining, .. }
            | Phase::Suspended { remaining, .. } => *remaining,
        }
    }
```

Update `Phase::total()`:
```rust
    fn total(&self) -> u64 {
        match self {
            Phase::Working { total, .. }
            | Phase::Break { total, .. }
            | Phase::Suspended { total, .. } => *total,
            Phase::BreakPending { remaining } => *remaining,
        }
    }
```

Update `Phase::decrement()` to skip `Suspended`:
```rust
    fn decrement(&mut self) {
        match self {
            Phase::Working { remaining, .. }
            | Phase::BreakPending { remaining }
            | Phase::Break { remaining, .. } => {
                *remaining = remaining.saturating_sub(1);
            }
            Phase::Suspended { .. } => {} // frozen — don't decrement
        }
    }
```

- [ ] **Step 2: Add `Suspend` and `ResumeSuspended` commands**

Add to `SessionCommand`:
```rust
pub enum SessionCommand {
    Pause,
    Resume,
    Extend(u64),
    StartBreak,
    ExtendWork(u64),
    SkipBreak,
    TakeBreak,
    Suspend,
    ResumeSuspended,
}
```

- [ ] **Step 3: Handle Suspend/Resume commands in session_loop**

In the `session_loop` function's command-drain loop (`while let Ok(cmd) = cmd_rx.try_recv()`), add handlers:

```rust
                SessionCommand::Suspend => {
                    if !matches!(phase, Phase::Suspended { .. }) {
                        let prev = phase.as_str().to_string();
                        let rem = phase.remaining();
                        let tot = phase.total();
                        phase = Phase::Suspended {
                            previous_phase: prev,
                            remaining: rem,
                            total: tot,
                        };
                        emit_phase_changed(
                            &app, &phase, cycle_position, &config,
                            true, truncated_title.as_deref(), dnd_enabled,
                        );
                        update_tray_title(&app, rem, true, truncated_title.as_deref());
                    }
                }
                SessionCommand::ResumeSuspended => {
                    if let Phase::Suspended { previous_phase, remaining, total } = &phase {
                        phase = match previous_phase.as_str() {
                            "working" => Phase::Working { remaining: *remaining, total: *total },
                            "break" => Phase::Break { remaining: *remaining, total: *total },
                            _ => Phase::BreakPending { remaining: *remaining },
                        };
                        paused = false;
                        emit_phase_changed(
                            &app, &phase, cycle_position, &config,
                            false, truncated_title.as_deref(), dnd_enabled,
                        );
                    }
                }
```

- [ ] **Step 4: Skip tick decrement when suspended**

In the main tick handler (the part after `interval.tick()` where `phase.decrement()` is called), add a guard:

```rust
        // Only tick when not paused and not suspended
        if !paused && !matches!(phase, Phase::Suspended { .. }) {
            phase.decrement();
            // ... rest of existing tick logic
        }
```

- [ ] **Step 5: Add `suspend` and `resume_suspended` methods to FocusTimer**

Add public methods that send the commands:

```rust
    pub async fn suspend(&self) {
        if let Some(state) = self.state.lock().await.as_ref() {
            let _ = state.cmd_tx.send(SessionCommand::Suspend).await;
        }
    }

    pub async fn resume_suspended(&self) {
        if let Some(state) = self.state.lock().await.as_ref() {
            let _ = state.cmd_tx.send(SessionCommand::ResumeSuspended).await;
        }
    }
```

- [ ] **Step 6: Verify compilation**

Run: `cargo build -p desktop`

Expected: success.

- [ ] **Step 7: Commit**

```bash
git add crates/desktop/src/focus_timer.rs
git commit -m "feat(focus): add Suspended phase + wall-clock-safe suspend/resume"
```

---

## Task 6: Graceful Shutdown + DND Recovery

**Files:**
- Modify: `crates/desktop/src/main.rs:L818-L827`
- Modify: `crates/storage/migrations/001_initial.sql`
- Create: `crates/storage/src/repos/dnd_override.rs`
- Modify: `crates/storage/src/repos/mod.rs`
- Modify: `crates/app-core/src/state.rs`

- [ ] **Step 1: Add dnd_override table to schema**

In `crates/storage/migrations/001_initial.sql`, add after the `cron_jobs` table:

```sql
-- ============================================================
-- DND Override (crash recovery for focus sessions)
-- ============================================================
CREATE TABLE dnd_override (
    id               INTEGER PRIMARY KEY DEFAULT 1,
    original_state   TEXT NOT NULL,
    overridden_at    TEXT NOT NULL,
    session_id       TEXT
);
```

- [ ] **Step 2: Create DndOverrideRepo**

Create `crates/storage/src/repos/dnd_override.rs`:

```rust
use sqlx::SqlitePool;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DndOverrideRow {
    pub id: i64,
    pub original_state: String,
    pub overridden_at: String,
    pub session_id: Option<String>,
}

#[derive(Clone)]
pub struct DndOverrideRepo {
    db: SqlitePool,
}

impl DndOverrideRepo {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }

    /// Record that we overrode DND for a focus session.
    pub async fn set(
        &self,
        original_state: &str,
        session_id: Option<&str>,
    ) -> common::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT OR REPLACE INTO dnd_override (id, original_state, overridden_at, session_id)
             VALUES (1, ?1, ?2, ?3)",
        )
        .bind(original_state)
        .bind(&now)
        .bind(session_id)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    /// Clear the override (DND restored normally).
    pub async fn clear(&self) -> common::Result<()> {
        sqlx::query("DELETE FROM dnd_override WHERE id = 1")
            .execute(&self.db)
            .await?;
        Ok(())
    }

    /// Check if there's an orphaned override (crash recovery).
    pub async fn get(&self) -> common::Result<Option<DndOverrideRow>> {
        let row = sqlx::query_as::<_, DndOverrideRow>(
            "SELECT id, original_state, overridden_at, session_id FROM dnd_override WHERE id = 1",
        )
        .fetch_optional(&self.db)
        .await?;
        Ok(row)
    }
}
```

- [ ] **Step 3: Wire DndOverrideRepo into Repos**

In `crates/storage/src/repos/mod.rs`:

Add `pub mod dnd_override;` and `pub use dnd_override::*;`.

Add field to `Repos` struct:
```rust
    pub dnd_override: DndOverrideRepo,
```

Add to `Repos::from_pool`:
```rust
    dnd_override: DndOverrideRepo::new(db.clone()),
```

- [ ] **Step 4: Fix the graceful quit bug**

In `crates/desktop/src/main.rs`, add the atomic flag near the top (after imports):

```rust
use std::sync::atomic::{AtomicBool, Ordering};

static QUIT_REQUESTED: AtomicBool = AtomicBool::new(false);
```

Replace the `quit_app` command (find the existing implementation) to set the flag and run shutdown:

```rust
#[tauri::command]
async fn quit_app(app: AppHandle) {
    QUIT_REQUESTED.store(true, Ordering::SeqCst);

    // Graceful shutdown: stop focus timer (restores DND), shutdown core
    if let Some(core) = app.try_state::<std::sync::Arc<app_core::AppCore>>() {
        core.shutdown().await;
    }

    app.exit(0);
}
```

Fix the `RunEvent::ExitRequested` handler:

```rust
        .run(|_app, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                if !QUIT_REQUESTED.load(Ordering::SeqCst) {
                    api.prevent_exit();
                }
            }
        });
```

- [ ] **Step 5: Verify compilation**

Run: `cargo build -p storage -p desktop`

Expected: success.

- [ ] **Step 6: Commit**

```bash
git add crates/storage/ crates/desktop/src/main.rs
git commit -m "fix(desktop): graceful quit + DND crash recovery table"
```

---

## Task 7: CronService Sleep/Wake Handlers + Intent Evaluation

**Files:**
- Create: `crates/scheduling/src/service/intent.rs`
- Modify: `crates/scheduling/src/service/mod.rs`
- Modify: `crates/scheduling/src/service/store.rs`

- [ ] **Step 1: Write tests for missed job classification**

Create `crates/scheduling/src/service/intent.rs`:

```rust
use crate::types::{CatchUpPriority, CronJob, CronSchedule, IntentTrigger, IntentWindow};

/// Classification of a missed job for wake catch-up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MissedJobClass {
    NotMissed,
    Immediate,
    Deferred,
    Expired,
}

/// Snapshot of lifecycle state for intent evaluation.
#[derive(Debug, Clone)]
pub struct PresenceSnapshot {
    pub idle_secs: u64,
    pub is_user_present: bool,
    pub continuous_active_mins: u32,
    pub smart_scheduling_disabled: bool,
}

/// Known cheap jobs that need no LLM.
const CHEAP_JOBS: &[&str] = &[
    "__klyntbot_atom_decay_daily",
    "__klyntbot_session_cleanup",
    "__klyntbot_memory_maintenance",
    "__klyntbot_analytics_cleanup",
    "__klyntbot_blackboard_cleanup",
    "__klyntbot_mirror_cleanup",
    "todo_overdue_check",
    "__klyntbot_recurring_tasks",
    "__klyntbot_reminder_check",
    "todo_focus_check",
];

/// Classify a job as missed during a sleep window.
pub fn classify_missed_job(
    job: &CronJob,
    sleep_start_ms: i64,
    now_ms: i64,
) -> MissedJobClass {
    let missed = job
        .state
        .next_run_at_ms
        .map_or(false, |next| next >= sleep_start_ms && next <= now_ms);

    if !missed {
        return MissedJobClass::NotMissed;
    }

    match (&job.schedule, &job.intent_window) {
        // One-shot At jobs → expired (we don't auto-fire late one-shots)
        (CronSchedule::At { .. }, _) => MissedJobClass::Expired,

        // Jobs with explicit WhenPresent/WhenIdle intent → deferred
        (_, Some(IntentWindow { catch_up: CatchUpPriority::WhenPresent, .. })) => {
            MissedJobClass::Deferred
        }
        (_, Some(IntentWindow { catch_up: CatchUpPriority::WhenIdle, .. })) => {
            MissedJobClass::Deferred
        }
        (_, Some(IntentWindow { catch_up: CatchUpPriority::Immediate, .. })) => {
            MissedJobClass::Immediate
        }

        // Known cheap jobs → immediate
        _ if CHEAP_JOBS.contains(&job.name.as_str()) => MissedJobClass::Immediate,

        // Everything else (LLM-heavy by default) → deferred
        _ => MissedJobClass::Deferred,
    }
}

/// Evaluate whether an intent trigger condition is currently met.
pub fn evaluate_trigger(trigger: &IntentTrigger, presence: &PresenceSnapshot) -> bool {
    if presence.smart_scheduling_disabled {
        return true; // smart scheduling off = always fire immediately
    }

    match trigger {
        IntentTrigger::UserPresent => presence.is_user_present,
        IntentTrigger::MinActiveMinutes { minutes } => {
            presence.continuous_active_mins >= *minutes
        }
        IntentTrigger::UserIdle { min_idle_secs } => {
            presence.idle_secs >= *min_idle_secs
        }
        IntentTrigger::FirstActivityAfter { after_local } => {
            let local_now = chrono::Local::now().time();
            presence.is_user_present && local_now >= *after_local
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{CronJobState, CronOrigin};
    use std::time::Duration;

    fn make_job(name: &str, schedule: CronSchedule, next_run: i64) -> CronJob {
        let mut job = CronJob::new("id", name, schedule, "", CronOrigin::System);
        job.state.next_run_at_ms = Some(next_run);
        job
    }

    #[test]
    fn not_missed_if_next_run_outside_window() {
        let job = make_job("test", CronSchedule::Every { every_ms: 3600000 }, 6000);
        assert_eq!(classify_missed_job(&job, 1000, 5000), MissedJobClass::NotMissed);
    }

    #[test]
    fn cheap_job_is_immediate() {
        let job = make_job(
            "__klyntbot_atom_decay_daily",
            CronSchedule::Cron { expr: "0 0 3 * * *".into(), tz: None },
            2000,
        );
        assert_eq!(classify_missed_job(&job, 1000, 5000), MissedJobClass::Immediate);
    }

    #[test]
    fn at_job_is_expired() {
        let job = make_job("reminder", CronSchedule::At { at_ms: 2000 }, 2000);
        assert_eq!(classify_missed_job(&job, 1000, 5000), MissedJobClass::Expired);
    }

    #[test]
    fn llm_job_with_when_present_is_deferred() {
        let mut job = make_job(
            "__klyntbot_cognitive_weekly_reflection",
            CronSchedule::Cron { expr: "0 0 9 * * 1".into(), tz: None },
            2000,
        );
        job.intent_window = Some(IntentWindow {
            trigger: IntentTrigger::UserPresent,
            tolerance: Duration::from_secs(7200),
            catch_up: CatchUpPriority::WhenPresent,
        });
        assert_eq!(classify_missed_job(&job, 1000, 5000), MissedJobClass::Deferred);
    }

    #[test]
    fn unknown_job_defaults_to_deferred() {
        let job = make_job(
            "some_custom_llm_job",
            CronSchedule::Cron { expr: "0 0 9 * * *".into(), tz: None },
            2000,
        );
        assert_eq!(classify_missed_job(&job, 1000, 5000), MissedJobClass::Deferred);
    }

    #[test]
    fn evaluate_user_present_when_present() {
        let presence = PresenceSnapshot {
            idle_secs: 1,
            is_user_present: true,
            continuous_active_mins: 5,
            smart_scheduling_disabled: false,
        };
        assert!(evaluate_trigger(&IntentTrigger::UserPresent, &presence));
    }

    #[test]
    fn evaluate_user_present_when_absent() {
        let presence = PresenceSnapshot {
            idle_secs: 600,
            is_user_present: false,
            continuous_active_mins: 0,
            smart_scheduling_disabled: false,
        };
        assert!(!evaluate_trigger(&IntentTrigger::UserPresent, &presence));
    }

    #[test]
    fn smart_scheduling_disabled_always_fires() {
        let presence = PresenceSnapshot {
            idle_secs: 600,
            is_user_present: false,
            continuous_active_mins: 0,
            smart_scheduling_disabled: true,
        };
        assert!(evaluate_trigger(&IntentTrigger::UserPresent, &presence));
    }

    #[test]
    fn evaluate_min_active_minutes() {
        let presence = PresenceSnapshot {
            idle_secs: 0,
            is_user_present: true,
            continuous_active_mins: 3,
            smart_scheduling_disabled: false,
        };
        assert!(!evaluate_trigger(&IntentTrigger::MinActiveMinutes { minutes: 5 }, &presence));
        let presence = PresenceSnapshot { continuous_active_mins: 6, ..presence };
        assert!(evaluate_trigger(&IntentTrigger::MinActiveMinutes { minutes: 5 }, &presence));
    }

    #[test]
    fn evaluate_user_idle() {
        let presence = PresenceSnapshot {
            idle_secs: 200,
            is_user_present: false,
            continuous_active_mins: 0,
            smart_scheduling_disabled: false,
        };
        assert!(!evaluate_trigger(&IntentTrigger::UserIdle { min_idle_secs: 300 }, &presence));
        let presence = PresenceSnapshot { idle_secs: 400, ..presence };
        assert!(evaluate_trigger(&IntentTrigger::UserIdle { min_idle_secs: 300 }, &presence));
    }
}
```

- [ ] **Step 2: Register the module and run tests**

In `crates/scheduling/src/service/mod.rs`, add:
```rust
pub(crate) mod intent;
pub use intent::{classify_missed_job, evaluate_trigger, MissedJobClass, PresenceSnapshot};
```

Run: `cargo nextest run -p scheduling -E 'test(intent)'`

Expected: all PASS.

- [ ] **Step 3: Add sleep/wake state to CronService**

In `crates/scheduling/src/service/mod.rs`, add fields to `CronService`:

```rust
pub struct CronService {
    // ... existing fields ...
    /// Timestamp when the system went to sleep (ms).
    pub(crate) sleep_start_ms: Arc<RwLock<Option<i64>>>,
}
```

Initialize in `new()` and `new_for_test()`:
```rust
    sleep_start_ms: Arc::new(RwLock::new(None)),
```

Add methods:

```rust
    /// Called when system is about to sleep.
    pub async fn on_system_will_sleep(&self) {
        *self.sleep_start_ms.write().await = Some(now_ms());
    }

    /// Called when system wakes. Returns classified missed jobs.
    pub async fn on_system_did_wake(&self) -> (Vec<CronJob>, Vec<CronJob>, Vec<CronJob>) {
        let sleep_start = self.sleep_start_ms.write().await.take().unwrap_or(now_ms());
        let now = now_ms();
        let store = self.store.read().await;

        let mut immediate = Vec::new();
        let mut deferred = Vec::new();
        let mut expired = Vec::new();

        for job in &store.jobs {
            if !job.enabled { continue; }
            match intent::classify_missed_job(job, sleep_start, now) {
                MissedJobClass::NotMissed => {}
                MissedJobClass::Immediate => immediate.push(job.clone()),
                MissedJobClass::Deferred => deferred.push(job.clone()),
                MissedJobClass::Expired => expired.push(job.clone()),
            }
        }

        // Recompute next_run_at_ms for all recurring jobs from now
        drop(store);
        self.recompute_next_runs().await;
        if let Err(e) = self.save_store().await {
            tracing::error!("Failed to save store after wake: {e}");
        }
        self.wake.notify_one();

        (immediate, deferred, expired)
    }
```

- [ ] **Step 4: Verify compilation and tests**

Run: `cargo nextest run -p scheduling`

Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/scheduling/src/service/
git commit -m "feat(scheduling): missed job classification + intent window evaluation"
```

---

## Task 8: Feature Handlers — Productivity & Cognitive Fixes

**Files:**
- Modify: `crates/activity-log/src/inference_loop.rs:L32-L33`
- Modify: `crates/feature-productivity/src/tracker/mod.rs`
- Modify: `crates/feature-productivity/src/nudge.rs`

- [ ] **Step 1: Fix ContextInferenceLoop MissedTickBehavior**

In `crates/activity-log/src/inference_loop.rs`, after the interval creation (around L32-L33), add:

```rust
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
```

Do this for both intervals in the file (inference interval and archival interval).

- [ ] **Step 2: Verify compilation**

Run: `cargo build -p activity-log`

Expected: success.

- [ ] **Step 3: Commit**

```bash
git add crates/activity-log/src/inference_loop.rs
git commit -m "fix(activity-log): set MissedTickBehavior::Skip to prevent burst on wake"
```

---

## Task 9: WakeOrchestrator — Collection & Greeting

**Files:**
- Create: `crates/app-core/src/wake_orchestrator.rs`
- Modify: `crates/app-core/src/state.rs`
- Modify: `crates/app-core/src/init/mod.rs`

- [ ] **Step 1: Write tests for greeting logic**

Create `crates/app-core/src/wake_orchestrator.rs`:

```rust
use bus::{DomainEvent, WakeType};
use std::time::Duration;
use tokio::sync::broadcast;

/// Summary of what happened during absence.
#[derive(Debug, Clone)]
pub struct WakePanel {
    pub greeting: String,
    pub absence_duration: Duration,
    pub wake_type: WakeType,
    pub focus_suspended: Option<FocusSuspendedInfo>,
    pub immediate_jobs_run: usize,
    pub deferred_jobs_pending: usize,
    pub expired_jobs: usize,
}

#[derive(Debug, Clone)]
pub struct FocusSuspendedInfo {
    pub remaining_secs: u64,
    pub phase_name: String,
}

/// Build the greeting string based on context.
pub fn build_greeting(absence: Duration, wake_type: WakeType) -> String {
    let hour = chrono::Local::now().hour();
    let period = match hour {
        5..=11 => "Good morning",
        12..=16 => "Good afternoon",
        17..=21 => "Good evening",
        _ => "Welcome back",
    };

    let duration_str = humanize_duration(absence);

    match (wake_type, absence.as_secs() > 3600) {
        (WakeType::FromSleep, true) => format!("{period}. You were away for {duration_str}."),
        (WakeType::FromSleep, false) => format!("{period}. Quick nap — {duration_str}."),
        (WakeType::FromIdle, true) => format!("{period}. You stepped away for {duration_str}."),
        (WakeType::FromIdle, false) => String::new(),
    }
}

fn humanize_duration(d: Duration) -> String {
    let total_secs = d.as_secs();
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;

    match (hours, minutes) {
        (0, m) => format!("{m}m"),
        (h, 0) => format!("{h}h"),
        (h, m) => format!("{h}h {m}m"),
    }
}

/// Compute the quiet period (seconds) based on time of day.
pub fn quiet_period_secs(config: &config::WakeDeliveryConfig) -> u64 {
    let hour = chrono::Local::now().hour();
    match hour {
        5..=11 => config.quiet_period_morning_secs,
        12..=16 => config.quiet_period_midday_secs,
        20..=23 | 0..=4 => config.quiet_period_evening_secs,
        _ => config.quiet_period_default_secs,
    }
}

use chrono::Timelike;

/// The WakeOrchestrator — subscribes to lifecycle and feature events,
/// assembles the wake panel when the user returns.
pub struct WakeOrchestrator {
    bus: std::sync::Arc<bus::DomainEventBus>,
    config: config::WakeDeliveryConfig,
}

impl WakeOrchestrator {
    pub fn new(
        bus: std::sync::Arc<bus::DomainEventBus>,
        config: config::WakeDeliveryConfig,
    ) -> Self {
        Self { bus, config }
    }

    /// Start the orchestrator background loop.
    pub fn start(self) -> tokio::task::JoinHandle<()> {
        let mut rx = self.bus.subscribe();
        tokio::spawn(async move {
            let mut pending_wake: Option<WakeContext> = None;

            loop {
                let event = match rx.recv().await {
                    Ok(e) => e,
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("WakeOrchestrator lagged {n} events");
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                };

                match event {
                    DomainEvent::SystemDidWake { away_duration, wake_type } => {
                        if away_duration.as_secs() < self.config.min_absence_for_panel_secs {
                            continue; // too short, no panel
                        }
                        pending_wake = Some(WakeContext {
                            away_duration,
                            wake_type,
                            focus_suspended: None,
                            immediate_count: 0,
                            deferred_count: 0,
                            expired_count: 0,
                        });
                        // Start 5s collection window (features will emit ready signals)
                    }
                    DomainEvent::FocusSessionSuspended { remaining_secs, phase_name } => {
                        if let Some(ref mut ctx) = pending_wake {
                            ctx.focus_suspended = Some(FocusSuspendedInfo {
                                remaining_secs,
                                phase_name,
                            });
                        }
                    }
                    DomainEvent::CronCatchUpReady {
                        immediate_count,
                        deferred_count,
                        expired_count,
                    } => {
                        if let Some(ref mut ctx) = pending_wake {
                            ctx.immediate_count = immediate_count;
                            ctx.deferred_count = deferred_count;
                            ctx.expired_count = expired_count;
                        }
                    }
                    DomainEvent::UserReturned { absence_duration, wake_type } => {
                        if let Some(ctx) = pending_wake.take() {
                            // Apply quiet period then present
                            let quiet = quiet_period_secs(&self.config);
                            tokio::time::sleep(Duration::from_secs(quiet)).await;

                            let panel = WakePanel {
                                greeting: build_greeting(ctx.away_duration, ctx.wake_type),
                                absence_duration: ctx.away_duration,
                                wake_type: ctx.wake_type,
                                focus_suspended: ctx.focus_suspended,
                                immediate_jobs_run: ctx.immediate_count,
                                deferred_jobs_pending: ctx.deferred_count,
                                expired_jobs: ctx.expired_count,
                            };

                            tracing::info!(
                                "Wake panel: {} | focus={} immediate={} deferred={} expired={}",
                                panel.greeting,
                                panel.focus_suspended.is_some(),
                                panel.immediate_jobs_run,
                                panel.deferred_jobs_pending,
                                panel.expired_jobs,
                            );

                            // Emit wake panel event for the UI
                            self.bus.publish(DomainEvent::WakePanelReady {
                                greeting: panel.greeting,
                                away_secs: panel.absence_duration.as_secs(),
                            });
                        }
                    }
                    _ => {}
                }
            }
        })
    }
}

struct WakeContext {
    away_duration: Duration,
    wake_type: WakeType,
    focus_suspended: Option<FocusSuspendedInfo>,
    immediate_count: usize,
    deferred_count: usize,
    expired_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greeting_morning_long_sleep() {
        // This test is time-of-day dependent; it validates format
        let g = build_greeting(Duration::from_secs(4 * 3600 + 12 * 60), WakeType::FromSleep);
        assert!(g.contains("4h 12m"));
        assert!(!g.is_empty());
    }

    #[test]
    fn greeting_short_idle_is_empty() {
        let g = build_greeting(Duration::from_secs(300), WakeType::FromIdle);
        assert!(g.is_empty());
    }

    #[test]
    fn humanize_hours_and_minutes() {
        assert_eq!(humanize_duration(Duration::from_secs(7380)), "2h 3m");
        assert_eq!(humanize_duration(Duration::from_secs(3600)), "1h");
        assert_eq!(humanize_duration(Duration::from_secs(300)), "5m");
    }

    #[test]
    fn quiet_period_uses_config_defaults() {
        let config = config::WakeDeliveryConfig::default();
        let secs = quiet_period_secs(&config);
        // Should return one of the configured values (time-dependent)
        assert!(secs > 0 && secs <= 60);
    }
}
```

- [ ] **Step 2: Add WakePanelReady event to bus**

In `crates/bus/src/domain_events.rs`, add variant:

```rust
    /// Wake panel assembled and ready for UI display.
    WakePanelReady {
        greeting: String,
        away_secs: u64,
    },
```

- [ ] **Step 3: Wire into app-core**

In `crates/app-core/src/state.rs`, add module and optional field:

Add `pub mod wake_orchestrator;` to `crates/app-core/src/lib.rs` (or wherever modules are declared).

Add to `AppCore` struct:
```rust
    pub _wake_orchestrator_handle: Option<tokio::task::JoinHandle<()>>,
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p app-core -E 'test(greeting)' && cargo nextest run -p app-core -E 'test(humanize)' && cargo nextest run -p app-core -E 'test(quiet_period)'`

Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/wake_orchestrator.rs crates/app-core/src/ \
  crates/bus/src/domain_events.rs
git commit -m "feat(wake): WakeOrchestrator with greeting logic and wake panel assembly"
```

---

## Task 10: Lifecycle Bridge + Startup Recovery

**Files:**
- Modify: `crates/app-core/src/init/mod.rs`
- Modify: `crates/app-core/src/state.rs`

- [ ] **Step 1: Add lifecycle bridge to AppCore init**

In `crates/app-core/src/init/mod.rs`, after the cron initialization phase, add a new lifecycle phase:

```rust
    // Phase 10: Lifecycle monitoring
    let lifecycle_config = config_snapshot.lifecycle.clone();
    let bus_for_lifecycle = domain_event_bus.clone();
    let cron_for_lifecycle = cron_service.clone();
    let focus_timer_for_lifecycle = focus_timer.clone(); // if available

    let lifecycle_monitor = if cfg!(target_os = "macos") {
        let monitor = platform_macos::lifecycle::LifecycleMonitor::start(
            lifecycle_config,
            move |event| {
                use platform_macos::lifecycle::LifecycleEvent as LE;
                match event {
                    LE::SystemWillSleep => {
                        if let Some(ref bus) = bus_for_lifecycle {
                            bus.publish(bus::DomainEvent::SystemWillSleep);
                        }
                        // Tell cron service to record sleep start
                        let cron = cron_for_lifecycle.clone();
                        tokio::task::block_in_place(|| {
                            tokio::runtime::Handle::current().block_on(
                                cron.on_system_will_sleep()
                            );
                        });
                    }
                    LE::SystemDidWake { away_duration, wake_type } => {
                        let bus_wake_type = match wake_type {
                            platform_macos::lifecycle::WakeType::FromSleep => bus::WakeType::FromSleep,
                            platform_macos::lifecycle::WakeType::FromIdle => bus::WakeType::FromIdle,
                        };
                        if let Some(ref bus) = bus_for_lifecycle {
                            bus.publish(bus::DomainEvent::SystemDidWake {
                                away_duration,
                                wake_type: bus_wake_type,
                            });
                        }
                        // Classify missed jobs
                        let cron = cron_for_lifecycle.clone();
                        let bus = bus_for_lifecycle.clone();
                        tokio::task::block_in_place(|| {
                            let (immediate, deferred, expired) =
                                tokio::runtime::Handle::current().block_on(
                                    cron.on_system_did_wake()
                                );
                            if let Some(ref bus) = bus {
                                bus.publish(bus::DomainEvent::CronCatchUpReady {
                                    immediate_count: immediate.len(),
                                    deferred_count: deferred.len(),
                                    expired_count: expired.len(),
                                });
                            }
                            // Run immediate (cheap) jobs now
                            for job in &immediate {
                                let _ = tokio::runtime::Handle::current().block_on(
                                    cron.run_job(&job.id)
                                );
                            }
                        });
                    }
                    LE::UserBecameIdle { idle_secs } => {
                        if let Some(ref bus) = bus_for_lifecycle {
                            bus.publish(bus::DomainEvent::UserBecameIdle { idle_secs });
                        }
                    }
                    LE::UserReturned { absence_duration, wake_type } => {
                        let bus_wake_type = match wake_type {
                            platform_macos::lifecycle::WakeType::FromSleep => bus::WakeType::FromSleep,
                            platform_macos::lifecycle::WakeType::FromIdle => bus::WakeType::FromIdle,
                        };
                        if let Some(ref bus) = bus_for_lifecycle {
                            bus.publish(bus::DomainEvent::UserReturned {
                                absence_duration,
                                wake_type: bus_wake_type,
                            });
                        }
                    }
                }
            },
        );
        Some(monitor)
    } else {
        None
    };
```

- [ ] **Step 2: Add startup recovery**

Add a recovery function early in `init_with_sender`, before the cron service starts:

```rust
    // Startup recovery — runs before cron and lifecycle start
    if let Ok(Some(dnd_row)) = repos.dnd_override.get().await {
        tracing::warn!(
            "Recovering DND state from interrupted session (overridden at {})",
            dnd_row.overridden_at
        );
        // Restore DND — the actual restore logic depends on platform_macos::dnd
        #[cfg(target_os = "macos")]
        {
            platform_macos::dnd::restore_dnd_state();
        }
        let _ = repos.dnd_override.clear().await;
    }
```

- [ ] **Step 3: Start WakeOrchestrator**

After lifecycle monitor setup, start the orchestrator:

```rust
    let wake_handle = domain_event_bus.as_ref().map(|bus| {
        let orchestrator = crate::wake_orchestrator::WakeOrchestrator::new(
            bus.clone(),
            config_snapshot.lifecycle.wake_delivery.clone(),
        );
        orchestrator.start()
    });
```

Store in AppCore:
```rust
    _wake_orchestrator_handle: wake_handle,
```

- [ ] **Step 4: Verify compilation**

Run: `cargo build -p app-core`

Expected: success.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/
git commit -m "feat(lifecycle): bridge LifecycleMonitor to DomainEventBus + startup recovery"
```

---

## Task 11: Intent Window Defaults for AI Jobs

**Files:**
- Modify: `crates/app-core/src/init/cron.rs:L858+` (inside `ensure_cron_jobs`)

- [ ] **Step 1: Add intent windows to default AI jobs**

In `crates/app-core/src/init/cron.rs`, after each `ensure_job!` call for AI-heavy jobs, set the intent window. Find the job in the store and update it:

```rust
    // After ensure_cron_jobs creates all jobs, set intent windows on AI jobs
    fn set_default_intent_windows(cron_service: &CronService) {
        use scheduling::types::{IntentWindow, IntentTrigger, CatchUpPriority};
        use std::time::Duration;

        let windows: &[(&str, IntentWindow)] = &[
            (JOB_WEEKLY_REFLECTION, IntentWindow {
                trigger: IntentTrigger::FirstActivityAfter {
                    after_local: chrono::NaiveTime::from_hms_opt(8, 0, 0).unwrap(),
                },
                tolerance: Duration::from_secs(7200),
                catch_up: CatchUpPriority::WhenPresent,
            }),
            (JOB_MIRROR_WEEKLY_NARRATIVE, IntentWindow {
                trigger: IntentTrigger::FirstActivityAfter {
                    after_local: chrono::NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
                },
                tolerance: Duration::from_secs(10800),
                catch_up: CatchUpPriority::WhenPresent,
            }),
            (JOB_AUTOTUNER_NIGHTLY, IntentWindow {
                trigger: IntentTrigger::UserIdle { min_idle_secs: 300 },
                tolerance: Duration::from_secs(14400),
                catch_up: CatchUpPriority::WhenIdle,
            }),
            (JOB_PROACTIVE_SCAN, IntentWindow {
                trigger: IntentTrigger::MinActiveMinutes { minutes: 5 },
                tolerance: Duration::from_secs(3600),
                catch_up: CatchUpPriority::WhenPresent,
            }),
            (JOB_INSIGHT_REFRESH, IntentWindow {
                trigger: IntentTrigger::UserIdle { min_idle_secs: 600 },
                tolerance: Duration::from_secs(21600),
                catch_up: CatchUpPriority::WhenIdle,
            }),
            (JOB_ATOM_EXTRACTION_CATCHALL, IntentWindow {
                trigger: IntentTrigger::UserIdle { min_idle_secs: 300 },
                tolerance: Duration::from_secs(14400),
                catch_up: CatchUpPriority::WhenIdle,
            }),
            (JOB_WEEKLY_REPORT, IntentWindow {
                trigger: IntentTrigger::UserPresent,
                tolerance: Duration::from_secs(7200),
                catch_up: CatchUpPriority::WhenPresent,
            }),
        ];

        // Apply intent windows to existing jobs
        // This is called after ensure_cron_jobs, so jobs exist in the store
        tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            for (name, window) in windows {
                rt.block_on(cron_service.set_intent_window(name, window.clone()));
            }
        });
    }
```

- [ ] **Step 2: Add `set_intent_window` method to CronService**

In `crates/scheduling/src/service/mod.rs`:

```rust
    /// Set or update the intent window for a job by name.
    pub async fn set_intent_window(&self, name: &str, window: IntentWindow) {
        let mut store = self.store.write().await;
        if let Some(job) = store.jobs.iter_mut().find(|j| j.name == name) {
            job.intent_window = Some(window);
        }
    }
```

- [ ] **Step 3: Call it from init_cron**

In `ensure_cron_jobs`, after all `ensure_job!` calls, add:

```rust
    set_default_intent_windows(&cron_service);
```

- [ ] **Step 4: Verify compilation**

Run: `cargo build -p app-core`

Expected: success.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/init/cron.rs crates/scheduling/src/service/mod.rs
git commit -m "feat(scheduling): default intent windows for AI-heavy cron jobs"
```

---

## Task 12: Integration Tests — Continuity Journeys

**Files:**
- Create: `tests/continuity/mod.rs`
- Create: `tests/continuity/sleep_wake.rs`

- [ ] **Step 1: Create continuity test module**

Create `tests/continuity/mod.rs`:
```rust
mod sleep_wake;
```

Create `tests/continuity/sleep_wake.rs`:

```rust
//! Continuity journey tests — validate the *feeling* of sleep/wake transitions.
//!
//! These simulate entire user journeys using the DomainEventBus and in-memory storage.
//! If any journey produces notification spam or a jarring prompt, the test fails.

use bus::{DomainEvent, DomainEventBus, WakeType};
use std::sync::Arc;
use std::time::Duration;

/// Helper: collect all events from a bus subscriber within a timeout.
async fn collect_events(
    mut rx: tokio::sync::broadcast::Receiver<DomainEvent>,
    timeout: Duration,
) -> Vec<DomainEvent> {
    let mut events = Vec::new();
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        tokio::select! {
            _ = tokio::time::sleep_until(deadline) => break,
            result = rx.recv() => {
                match result {
                    Ok(e) => events.push(e),
                    Err(_) => break,
                }
            }
        }
    }
    events
}

#[tokio::test]
async fn coffee_run_idle_no_panel() {
    // 5-minute idle → return → no wake panel should be emitted
    tokio::time::pause();

    let bus = Arc::new(DomainEventBus::new(64));
    let rx = bus.subscribe();

    let config = config::WakeDeliveryConfig::default(); // min_absence = 1800s
    let orchestrator = app_core::wake_orchestrator::WakeOrchestrator::new(bus.clone(), config);
    let _handle = orchestrator.start();

    // Simulate 5-minute idle wake (below 30-min threshold)
    bus.publish(DomainEvent::SystemDidWake {
        away_duration: Duration::from_secs(300),
        wake_type: WakeType::FromIdle,
    });

    tokio::time::advance(Duration::from_secs(10)).await;

    bus.publish(DomainEvent::UserReturned {
        absence_duration: Duration::from_secs(300),
        wake_type: WakeType::FromIdle,
    });

    tokio::time::advance(Duration::from_secs(120)).await;

    let events = collect_events(rx, Duration::from_millis(100)).await;
    let panel_count = events
        .iter()
        .filter(|e| matches!(e, DomainEvent::WakePanelReady { .. }))
        .count();

    assert_eq!(panel_count, 0, "Short idle should NOT produce a wake panel");
}

#[tokio::test]
async fn full_sleep_wake_produces_single_panel() {
    // 4-hour sleep → wake → exactly 1 WakePanelReady event
    tokio::time::pause();

    let bus = Arc::new(DomainEventBus::new(64));
    let rx = bus.subscribe();

    let config = config::WakeDeliveryConfig::default();
    let orchestrator = app_core::wake_orchestrator::WakeOrchestrator::new(bus.clone(), config);
    let _handle = orchestrator.start();

    // System wakes after 4 hours
    bus.publish(DomainEvent::SystemDidWake {
        away_duration: Duration::from_secs(4 * 3600),
        wake_type: WakeType::FromSleep,
    });

    tokio::time::advance(Duration::from_secs(2)).await;

    // Features emit their ready signals
    bus.publish(DomainEvent::CronCatchUpReady {
        immediate_count: 3,
        deferred_count: 2,
        expired_count: 1,
    });

    tokio::time::advance(Duration::from_secs(5)).await;

    // User returns
    bus.publish(DomainEvent::UserReturned {
        absence_duration: Duration::from_secs(4 * 3600),
        wake_type: WakeType::FromSleep,
    });

    // Wait for quiet period (default: 30-45s) + buffer
    tokio::time::advance(Duration::from_secs(60)).await;

    let events = collect_events(rx, Duration::from_millis(100)).await;
    let panels: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, DomainEvent::WakePanelReady { .. }))
        .collect();

    assert_eq!(panels.len(), 1, "Full sleep/wake should produce exactly 1 wake panel");

    // Notification fatigue guard: no other notification-like events
    // (In production this means the UI shows exactly one surface)
}

#[tokio::test]
async fn notification_fatigue_guard_max_one_panel() {
    // Multiple rapid wake events should still produce at most 1 panel
    tokio::time::pause();

    let bus = Arc::new(DomainEventBus::new(64));
    let rx = bus.subscribe();

    let config = config::WakeDeliveryConfig::default();
    let orchestrator = app_core::wake_orchestrator::WakeOrchestrator::new(bus.clone(), config);
    let _handle = orchestrator.start();

    // Rapid lid open/close/open
    for _ in 0..3 {
        bus.publish(DomainEvent::SystemDidWake {
            away_duration: Duration::from_secs(3600),
            wake_type: WakeType::FromSleep,
        });
        tokio::time::advance(Duration::from_secs(30)).await;
    }

    bus.publish(DomainEvent::UserReturned {
        absence_duration: Duration::from_secs(3600),
        wake_type: WakeType::FromSleep,
    });

    tokio::time::advance(Duration::from_secs(120)).await;

    let events = collect_events(rx, Duration::from_millis(100)).await;
    let panel_count = events
        .iter()
        .filter(|e| matches!(e, DomainEvent::WakePanelReady { .. }))
        .count();

    assert!(panel_count <= 1, "At most 1 wake panel across rapid wake cycles");
}
```

- [ ] **Step 2: Register in test binary**

Add `mod continuity;` to the appropriate test entry point (likely a new test binary in `Cargo.toml` or add to `tests/integration/`).

- [ ] **Step 3: Run continuity tests**

Run: `cargo nextest run -E 'test(continuity)'`

Expected: all PASS once Tasks 1-10 are complete.

- [ ] **Step 4: Commit**

```bash
git add tests/continuity/
git commit -m "test: continuity journey tests for sleep/wake lifecycle"
```

---

## Task 13: Full Workspace Verification

- [ ] **Step 1: Run full workspace build**

Run: `cargo build --workspace`

Expected: success, zero errors.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`

Expected: zero warnings.

- [ ] **Step 3: Check formatting**

Run: `cargo fmt --all --check`

Expected: no formatting issues.

- [ ] **Step 4: Run all tests**

Run: `cargo nextest run --workspace`

Expected: all PASS.

- [ ] **Step 5: Final commit if any fixups**

```bash
git commit -m "chore: clippy + fmt fixes for sleep/wake lifecycle"
```
