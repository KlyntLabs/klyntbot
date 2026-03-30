# Second Brain Comes Alive — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Klyntbot feel like a living second brain by adding ambient intelligence signals, a unified BrainVoice router, a persistent memory pulse orb, focus-bubble channel deferral, embedding-based cross-domain dots, and a guided onboarding journey — all in a 3-week sprint.

**Architecture:** A thin `BrainVoice` subscriber on the existing `DomainEventBus` collects intelligence signals (memory promotion, cross-domain dots, coaching, mirror insights, deferred messages), applies timing/priority/dedup rules, and emits a single `brain:ambient` Tauri event. The frontend renders a persistent `BrainOrb` in the global top bar. Focus sessions defer all signals for a unified post-session debrief.

**Tech Stack:** Rust (backend — bus, app-core, agent, feature-insights, cognitive, channels crates), React + TypeScript (frontend — Tailwind v4, Tauri IPC via `useEvent` hook), SQLite (new `brain_signal_feedback` table), LanceDB (existing vector search for cross-domain dots).

**Spec:** `docs/superpowers/specs/2026-03-30-second-brain-comes-alive-design.md`

---

## Week 1: Ambient Magic (Tasks 1–13)

### Task 1: Add DomainEvent Variants

**Files:**
- Modify: `crates/bus/src/domain_events.rs`
- Test: `cargo nextest run -p bus`

- [ ] **Step 1: Add three new event variants to DomainEvent enum**

Open `crates/bus/src/domain_events.rs`. Find the enum definition (starts around line 17). Add these three variants near the end, before the closing brace. Follow the existing naming pattern (e.g., `MirrorSnippetCreated`, `MirrorTrialKilled`):

```rust
    // ── Brain ambient signals ──────────────────────────────────
    MemoryPromoted {
        fact_id: String,
        summary: String,
        from_scope: String,
        to_scope: String,
    },
    CrossDomainDotReady {
        source_kind: String,
        source_id: String,
        source_title: String,
        target_kind: String,
        target_id: String,
        target_title: String,
        confidence: f64,
        tooltip: String,
        detail_route: Option<String>,
    },
    MessageDeferred {
        channel: String,
        sender: String,
        preview: String,
    },
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build -p bus`
Expected: compiles with 0 errors. All variants derive `Debug, Clone, Serialize, Deserialize` from the existing derive on the enum.

- [ ] **Step 3: Commit**

```bash
git add crates/bus/src/domain_events.rs
git commit -m "feat(bus): add MemoryPromoted, CrossDomainDotReady, MessageDeferred events"
```

---

### Task 2: Brain Signal Feedback Storage

**Files:**
- Create: `crates/storage/src/repos/brain_signal.rs`
- Modify: `crates/storage/src/repos/mod.rs`
- Modify: `crates/storage/src/rows/mod.rs` (or add to existing rows file)
- Modify: `crates/storage/migrations/001_initial.sql` (pre-release, modify in-place per CLAUDE.md)
- Test: inline `#[cfg(test)] mod tests`

We need a purpose-built table for brain signal dedup and dismissal tracking. The existing `enrichment_feedback` table is task-enrichment specific — different schema.

- [ ] **Step 1: Write the test for feedback creation and query**

Create `crates/storage/src/repos/brain_signal.rs`:

```rust
use crate::{StoragePool, StorageError};
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrainSignalFeedbackRow {
    pub id: i64,
    pub signal_type: String,
    pub entity_pair: String,
    pub action: String,
    pub timestamp: DateTime<Utc>,
}

pub struct BrainSignalFeedbackRepo {
    pool: StoragePool,
}

impl BrainSignalFeedbackRepo {
    pub fn new(pool: StoragePool) -> Self {
        Self { pool }
    }

    pub async fn record(
        &self,
        signal_type: &str,
        entity_pair: &str,
        action: &str,
    ) -> Result<(), StorageError> {
        todo!()
    }

    /// Check if this entity pair was surfaced in the last `hours` hours.
    pub async fn was_surfaced_recently(
        &self,
        entity_pair: &str,
        hours: i64,
    ) -> Result<bool, StorageError> {
        todo!()
    }

    /// Check if this entity pair was dismissed in the last `days` days.
    pub async fn was_dismissed_recently(
        &self,
        entity_pair: &str,
        days: i64,
    ) -> Result<bool, StorageError> {
        todo!()
    }

    /// Count dismissals in the last `hours` hours (for adaptive dampening).
    pub async fn dismissal_count_since(
        &self,
        hours: i64,
    ) -> Result<i64, StorageError> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_record_and_query_feedback() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = BrainSignalFeedbackRepo::new(pool);

        repo.record("cross_domain_dot", "task:1|finance:2", "accepted")
            .await
            .unwrap();

        assert!(repo.was_surfaced_recently("task:1|finance:2", 24).await.unwrap());
        assert!(!repo.was_surfaced_recently("task:1|finance:99", 24).await.unwrap());
    }

    #[tokio::test]
    async fn test_dismissal_count() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = BrainSignalFeedbackRepo::new(pool);

        repo.record("cross_domain_dot", "task:1|finance:2", "dismissed").await.unwrap();
        repo.record("memory_promoted", "fact:3", "dismissed").await.unwrap();
        repo.record("cross_domain_dot", "task:4|note:5", "accepted").await.unwrap();

        let count = repo.dismissal_count_since(48).await.unwrap();
        assert_eq!(count, 2); // only dismissed, not accepted
    }

    #[tokio::test]
    async fn test_dismissed_recently() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = BrainSignalFeedbackRepo::new(pool);

        repo.record("cross_domain_dot", "task:1|finance:2", "dismissed").await.unwrap();

        assert!(repo.was_dismissed_recently("task:1|finance:2", 30).await.unwrap());
        assert!(!repo.was_dismissed_recently("task:1|finance:99", 30).await.unwrap());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p storage -E 'test(brain_signal)'`
Expected: FAIL — `todo!()` panics.

- [ ] **Step 3: Add the migration SQL**

Open `crates/storage/migrations/001_initial.sql`. Add at the end (pre-release, no versioned migrations needed):

```sql
-- Brain signal feedback (dedup, dismissal tracking, adaptive dampening)
CREATE TABLE IF NOT EXISTS brain_signal_feedback (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    signal_type TEXT NOT NULL,
    entity_pair TEXT NOT NULL,
    action      TEXT NOT NULL,
    timestamp   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_brain_signal_entity_pair ON brain_signal_feedback(entity_pair, timestamp);
CREATE INDEX IF NOT EXISTS idx_brain_signal_action ON brain_signal_feedback(action, timestamp);
```

- [ ] **Step 4: Implement the repo methods**

Replace the `todo!()` calls in `BrainSignalFeedbackRepo`:

```rust
    pub async fn record(
        &self,
        signal_type: &str,
        entity_pair: &str,
        action: &str,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO brain_signal_feedback (signal_type, entity_pair, action) VALUES (?, ?, ?)",
        )
        .bind(signal_type)
        .bind(entity_pair)
        .bind(action)
        .execute(self.pool.inner())
        .await?;
        Ok(())
    }

    pub async fn was_surfaced_recently(
        &self,
        entity_pair: &str,
        hours: i64,
    ) -> Result<bool, StorageError> {
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM brain_signal_feedback WHERE entity_pair = ? AND timestamp > datetime('now', ?)",
        )
        .bind(entity_pair)
        .bind(format!("-{hours} hours"))
        .fetch_one(self.pool.inner())
        .await?;
        Ok(count.0 > 0)
    }

    pub async fn was_dismissed_recently(
        &self,
        entity_pair: &str,
        days: i64,
    ) -> Result<bool, StorageError> {
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM brain_signal_feedback WHERE entity_pair = ? AND action = 'dismissed' AND timestamp > datetime('now', ?)",
        )
        .bind(entity_pair)
        .bind(format!("-{days} days"))
        .fetch_one(self.pool.inner())
        .await?;
        Ok(count.0 > 0)
    }

    pub async fn dismissal_count_since(
        &self,
        hours: i64,
    ) -> Result<i64, StorageError> {
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM brain_signal_feedback WHERE action = 'dismissed' AND timestamp > datetime('now', ?)",
        )
        .bind(format!("-{hours} hours"))
        .fetch_one(self.pool.inner())
        .await?;
        Ok(count.0)
    }
```

- [ ] **Step 5: Register the module in repos/mod.rs**

Add `pub mod brain_signal;` to `crates/storage/src/repos/mod.rs`. Re-export if needed.

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo nextest run -p storage -E 'test(brain_signal)'`
Expected: 3 tests PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/storage/
git commit -m "feat(storage): add brain_signal_feedback table and repo"
```

---

### Task 3: BrainVoice Signal Router (Backend Core)

**Files:**
- Create: `crates/app-core/src/brain_voice.rs`
- Modify: `crates/app-core/src/lib.rs` (add `pub mod brain_voice;`)
- Test: inline `#[cfg(test)] mod tests`

This is the core intelligence — the single router that decides Pulse/Badge/Deferred/Merged for all brain signals. It runs as a `tokio::spawn` task, subscribes to `DomainEventBus`, and emits `brain:ambient` Tauri events via `AppEventEmitter`.

- [ ] **Step 1: Write the test for signal routing modes**

Create `crates/app-core/src/brain_voice.rs`:

```rust
use bus::DomainEvent;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use storage::repos::brain_signal::BrainSignalFeedbackRepo;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

use crate::events::AppEventEmitter;

/// How BrainVoice decides to surface a signal.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SignalMode {
    Pulse,
    Badge,
    Deferred,
    Merged,
}

/// A single intelligence signal summary.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalSummary {
    pub signal_type: String,
    pub entity_pair: String,
    pub headline: String,
}

/// The event emitted to the frontend via Tauri.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrainAmbientEvent {
    pub mode: SignalMode,
    pub signals: Vec<SignalSummary>,
    pub tooltip: String,
    pub detail_route: Option<String>,
}

pub const BRAIN_AMBIENT_EVENT: &str = "brain:ambient";

/// Configuration for BrainVoice routing rules.
#[derive(Debug, Clone)]
pub struct BrainVoiceConfig {
    pub max_pulses_per_hour: u32,
    pub merge_window: Duration,
    pub hold_duration: Duration,
    pub dampened_max_pulses: u32,
    pub dampened_merge_window: Duration,
    pub dampening_threshold: i64,
    pub dampening_window_hours: i64,
}

impl Default for BrainVoiceConfig {
    fn default() -> Self {
        Self {
            max_pulses_per_hour: 2,
            merge_window: Duration::from_secs(30),
            hold_duration: Duration::from_secs(5),
            dampened_max_pulses: 1,
            dampened_merge_window: Duration::from_secs(60),
            dampening_threshold: 2,
            dampening_window_hours: 48,
        }
    }
}

pub struct BrainVoice {
    cancel: CancellationToken,
    _handle: tokio::task::JoinHandle<()>,
}

impl BrainVoice {
    pub fn start(
        event_rx: broadcast::Receiver<DomainEvent>,
        emitter: Arc<dyn AppEventEmitter>,
        focus_active: Arc<AtomicBool>,
        feedback_repo: BrainSignalFeedbackRepo,
        config: BrainVoiceConfig,
    ) -> Self {
        let cancel = CancellationToken::new();
        let cancel_clone = cancel.clone();

        let handle = tokio::spawn(async move {
            run_brain_voice(event_rx, emitter, focus_active, feedback_repo, config, cancel_clone)
                .await;
        });

        Self {
            cancel,
            _handle: handle,
        }
    }

    pub fn shutdown(&self) {
        self.cancel.cancel();
    }
}

/// Internal state for the run loop.
struct RouterState {
    pulses_this_hour: u32,
    hour_start: Instant,
    pending_signals: Vec<(SignalSummary, String, Option<String>)>, // (summary, tooltip, detail_route)
    deferred_signals: Vec<(SignalSummary, String, Option<String>)>,
    last_signal_time: Option<Instant>,
}

impl RouterState {
    fn new() -> Self {
        Self {
            pulses_this_hour: 0,
            hour_start: Instant::now(),
            pending_signals: Vec::new(),
            deferred_signals: Vec::new(),
            last_signal_time: None,
        }
    }

    fn reset_hour_if_needed(&mut self) {
        if self.hour_start.elapsed() >= Duration::from_secs(3600) {
            self.pulses_this_hour = 0;
            self.hour_start = Instant::now();
        }
    }
}

async fn run_brain_voice(
    mut event_rx: broadcast::Receiver<DomainEvent>,
    emitter: Arc<dyn AppEventEmitter>,
    focus_active: Arc<AtomicBool>,
    feedback_repo: BrainSignalFeedbackRepo,
    config: BrainVoiceConfig,
    shutdown: CancellationToken,
) {
    let mut state = RouterState::new();

    loop {
        // If we have pending signals and the hold duration has passed, flush them.
        let flush_timeout = if !state.pending_signals.is_empty() {
            let elapsed = state.last_signal_time.map(|t| t.elapsed()).unwrap_or_default();
            if elapsed >= config.hold_duration {
                Duration::ZERO
            } else {
                config.hold_duration - elapsed
            }
        } else {
            Duration::from_secs(3600) // idle — just wait for events
        };

        tokio::select! {
            _ = shutdown.cancelled() => break,
            _ = tokio::time::sleep(flush_timeout), if !state.pending_signals.is_empty() => {
                flush_pending(&mut state, &emitter, &feedback_repo, &config).await;
            }
            result = event_rx.recv() => {
                match result {
                    Ok(event) => {
                        if let Some(signal) = extract_signal(&event) {
                            handle_signal(
                                signal,
                                &mut state,
                                &focus_active,
                                &emitter,
                                &feedback_repo,
                                &config,
                            ).await;
                        }
                        // Handle focus session end — flush deferred
                        if matches!(event, DomainEvent::FocusSessionEnded { .. }) {
                            flush_deferred(&mut state, &emitter).await;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("BrainVoice lagged, skipped {n} events");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }
}

fn extract_signal(event: &DomainEvent) -> Option<(SignalSummary, String, Option<String>)> {
    match event {
        DomainEvent::MemoryPromoted {
            fact_id,
            summary,
            to_scope,
            ..
        } => Some((
            SignalSummary {
                signal_type: "memory_promoted".into(),
                entity_pair: format!("fact:{fact_id}"),
                headline: summary.clone(),
            },
            summary.clone(),
            Some("/brain".into()),
        )),
        DomainEvent::CrossDomainDotReady {
            source_kind,
            source_id,
            target_kind,
            target_id,
            tooltip,
            detail_route,
            ..
        } => Some((
            SignalSummary {
                signal_type: "cross_domain_dot".into(),
                entity_pair: format!("{source_kind}:{source_id}|{target_kind}:{target_id}"),
                headline: tooltip.clone(),
            },
            tooltip.clone(),
            detail_route.clone(),
        )),
        DomainEvent::MessageDeferred {
            channel,
            sender,
            preview,
        } => Some((
            SignalSummary {
                signal_type: "message_deferred".into(),
                entity_pair: format!("msg:{channel}:{sender}"),
                headline: format!("{sender}: {preview}"),
            },
            format!("{sender} sent a message on {channel}"),
            None,
        )),
        _ => None,
    }
}

async fn handle_signal(
    signal: (SignalSummary, String, Option<String>),
    state: &mut RouterState,
    focus_active: &Arc<AtomicBool>,
    emitter: &Arc<dyn AppEventEmitter>,
    feedback_repo: &BrainSignalFeedbackRepo,
    config: &BrainVoiceConfig,
) {
    let (summary, tooltip, detail_route) = signal;

    // Dedup: skip if surfaced in last 24h
    if feedback_repo
        .was_surfaced_recently(&summary.entity_pair, 24)
        .await
        .unwrap_or(false)
    {
        return;
    }

    // Focus deferral
    if focus_active.load(Ordering::Relaxed) {
        state.deferred_signals.push((summary, tooltip, detail_route));
        return;
    }

    // Add to pending (merge window)
    state.last_signal_time = Some(Instant::now());
    state.pending_signals.push((summary, tooltip, detail_route));

    // If merge window exceeded, flush immediately
    if state.pending_signals.len() > 1 {
        // Check if first signal is past merge window
        // (simplified — the flush_timeout in the main loop handles timing)
    }
}

async fn flush_pending(
    state: &mut RouterState,
    emitter: &Arc<dyn AppEventEmitter>,
    feedback_repo: &BrainSignalFeedbackRepo,
    config: &BrainVoiceConfig,
) {
    if state.pending_signals.is_empty() {
        return;
    }

    state.reset_hour_if_needed();

    // Check adaptive dampening
    let dampened = feedback_repo
        .dismissal_count_since(config.dampening_window_hours)
        .await
        .unwrap_or(0)
        > config.dampening_threshold;

    let max_pulses = if dampened {
        config.dampened_max_pulses
    } else {
        config.max_pulses_per_hour
    };

    let signals: Vec<(SignalSummary, String, Option<String>)> =
        state.pending_signals.drain(..).collect();

    let mode = if state.pulses_this_hour >= max_pulses {
        SignalMode::Badge
    } else if signals.len() > 1 {
        state.pulses_this_hour += 1;
        SignalMode::Merged
    } else {
        state.pulses_this_hour += 1;
        SignalMode::Pulse
    };

    let tooltip = if signals.len() == 1 {
        signals[0].1.clone()
    } else {
        format!(
            "I noticed {} things — {}",
            signals.len(),
            signals
                .iter()
                .map(|(s, _, _)| s.headline.as_str())
                .collect::<Vec<_>>()
                .join("; ")
        )
    };

    let detail_route = if signals.len() == 1 {
        signals[0].2.clone()
    } else {
        Some("/brain".into())
    };

    let summaries: Vec<SignalSummary> = signals.iter().map(|(s, _, _)| s.clone()).collect();

    // Record as surfaced
    for summary in &summaries {
        let _ = feedback_repo
            .record(&summary.signal_type, &summary.entity_pair, "surfaced")
            .await;
    }

    let event = BrainAmbientEvent {
        mode,
        signals: summaries,
        tooltip,
        detail_route,
    };

    if let Ok(payload) = serde_json::to_value(&event) {
        emitter.emit_event(BRAIN_AMBIENT_EVENT, payload);
    }

    state.last_signal_time = None;
}

async fn flush_deferred(state: &mut RouterState, emitter: &Arc<dyn AppEventEmitter>) {
    if state.deferred_signals.is_empty() {
        return;
    }

    let signals: Vec<(SignalSummary, String, Option<String>)> =
        state.deferred_signals.drain(..).collect();

    let msg_count = signals
        .iter()
        .filter(|(s, _, _)| s.signal_type == "message_deferred")
        .count();
    let brain_count = signals.len() - msg_count;

    let tooltip = match (msg_count, brain_count) {
        (0, n) => format!("While you were focused, I noticed {n} thing{} — want to catch up?", if n == 1 { "" } else { "s" }),
        (m, 0) => format!("While you were focused, I held {m} message{} — want to catch up?", if m == 1 { "" } else { "s" }),
        (m, n) => format!("While you were focused, I held {m} message{} and noticed {n} connection{} — want to catch up?",
            if m == 1 { "" } else { "s" },
            if n == 1 { "" } else { "s" }),
    };

    let summaries: Vec<SignalSummary> = signals.into_iter().map(|(s, _, _)| s).collect();

    let event = BrainAmbientEvent {
        mode: SignalMode::Deferred,
        signals: summaries,
        tooltip,
        detail_route: None, // debrief opens as inline panel
    };

    if let Ok(payload) = serde_json::to_value(&event) {
        emitter.emit_event(BRAIN_AMBIENT_EVENT, payload);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::NoopEmitter;
    use bus::DomainEventBus;

    fn test_emitter() -> Arc<dyn AppEventEmitter> {
        Arc::new(NoopEmitter)
    }

    #[tokio::test]
    async fn test_extract_memory_promoted() {
        let event = DomainEvent::MemoryPromoted {
            fact_id: "f1".into(),
            summary: "You prefer dark mode".into(),
            from_scope: "persona".into(),
            to_scope: "squad".into(),
        };
        let signal = extract_signal(&event);
        assert!(signal.is_some());
        let (summary, tooltip, route) = signal.unwrap();
        assert_eq!(summary.signal_type, "memory_promoted");
        assert_eq!(summary.entity_pair, "fact:f1");
        assert_eq!(tooltip, "You prefer dark mode");
        assert_eq!(route, Some("/brain".into()));
    }

    #[tokio::test]
    async fn test_extract_cross_domain_dot() {
        let event = DomainEvent::CrossDomainDotReady {
            source_kind: "task".into(),
            source_id: "t1".into(),
            source_title: "Q2 deck".into(),
            target_kind: "finance".into(),
            target_id: "f1".into(),
            target_title: "Consulting spend".into(),
            confidence: 0.78,
            tooltip: "Your Q2 deck connects to consulting spend".into(),
            detail_route: Some("/brain?filter=cross-domain".into()),
        };
        let signal = extract_signal(&event);
        assert!(signal.is_some());
        let (summary, _, _) = signal.unwrap();
        assert_eq!(summary.entity_pair, "task:t1|finance:f1");
    }

    #[tokio::test]
    async fn test_extract_irrelevant_event_returns_none() {
        let event = DomainEvent::TaskCreated {
            task_id: "t1".into(),
            project: None,
            estimate_mins: None,
            task_type: "task".into(),
        };
        assert!(extract_signal(&event).is_none());
    }

    #[tokio::test]
    async fn test_dedup_skips_recently_surfaced() {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let repo = BrainSignalFeedbackRepo::new(pool);
        let focus = Arc::new(AtomicBool::new(false));
        let emitter = test_emitter();
        let config = BrainVoiceConfig::default();

        // Record as surfaced
        repo.record("cross_domain_dot", "task:1|finance:2", "surfaced")
            .await
            .unwrap();

        let mut state = RouterState::new();
        let signal = (
            SignalSummary {
                signal_type: "cross_domain_dot".into(),
                entity_pair: "task:1|finance:2".into(),
                headline: "test".into(),
            },
            "test tooltip".into(),
            None,
        );

        handle_signal(signal, &mut state, &focus, &emitter, &repo, &config).await;
        // Should be skipped — nothing in pending
        assert!(state.pending_signals.is_empty());
    }

    #[tokio::test]
    async fn test_focus_defers_signals() {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let repo = BrainSignalFeedbackRepo::new(pool);
        let focus = Arc::new(AtomicBool::new(true)); // Focus ON
        let emitter = test_emitter();
        let config = BrainVoiceConfig::default();

        let mut state = RouterState::new();
        let signal = (
            SignalSummary {
                signal_type: "memory_promoted".into(),
                entity_pair: "fact:1".into(),
                headline: "test".into(),
            },
            "test tooltip".into(),
            None,
        );

        handle_signal(signal, &mut state, &focus, &emitter, &repo, &config).await;
        assert!(state.pending_signals.is_empty());
        assert_eq!(state.deferred_signals.len(), 1);
    }

    #[tokio::test]
    async fn test_flush_deferred_builds_correct_tooltip() {
        let emitter = test_emitter();
        let mut state = RouterState::new();

        state.deferred_signals.push((
            SignalSummary {
                signal_type: "message_deferred".into(),
                entity_pair: "msg:telegram:alice".into(),
                headline: "Alice: Hey".into(),
            },
            "Alice sent a message".into(),
            None,
        ));
        state.deferred_signals.push((
            SignalSummary {
                signal_type: "cross_domain_dot".into(),
                entity_pair: "task:1|finance:2".into(),
                headline: "Q2 + spending".into(),
            },
            "connection found".into(),
            Some("/brain".into()),
        ));

        flush_deferred(&mut state, &emitter).await;
        assert!(state.deferred_signals.is_empty());
        // Emitter is noop — we just verify state was drained
    }
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo nextest run -p app-core -E 'test(brain_voice)'`
Expected: 5 tests PASS.

- [ ] **Step 3: Register the module**

Add `pub mod brain_voice;` to `crates/app-core/src/lib.rs`.

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/brain_voice.rs crates/app-core/src/lib.rs
git commit -m "feat(app-core): BrainVoice signal router with dedup, focus deferral, adaptive dampening"
```

---

### Task 4: Wire BrainVoice into AppCore Initialization

**Files:**
- Modify: `crates/app-core/src/state.rs` (add BrainVoice field)
- Modify: `crates/app-core/src/init/mod.rs` (initialize BrainVoice)
- Test: `cargo build -p app-core`

- [ ] **Step 1: Add BrainVoice field to AppCore state**

Open `crates/app-core/src/state.rs`. Add to the AppCore struct (follow the MirrorFacade pattern at lines ~129-134):

```rust
    /// BrainVoice ambient intelligence router (None in tests without event bus).
    pub brain_voice: Option<crate::brain_voice::BrainVoice>,
```

- [ ] **Step 2: Initialize BrainVoice in init/mod.rs**

Open `crates/app-core/src/init/mod.rs`. After the MirrorEngine initialization block (~line 314), add:

```rust
    // ── BrainVoice ambient intelligence router ──────────────────
    let brain_voice = if let Some(ref bus) = domain_event_bus_opt {
        let feedback_repo = storage::repos::brain_signal::BrainSignalFeedbackRepo::new(
            storage_pool.clone(),
        );
        // BrainVoice needs access to focus state. Create a shared AtomicBool
        // that the desktop adapter will wire to the existing tray_countdown::FOCUS_ACTIVE.
        let focus_active = Arc::new(std::sync::atomic::AtomicBool::new(false));

        Some(crate::brain_voice::BrainVoice::start(
            bus.subscribe(),
            Arc::clone(&event_emitter_ref),
            focus_active,
            feedback_repo,
            crate::brain_voice::BrainVoiceConfig::default(),
        ))
    } else {
        None
    };
```

Note: `domain_event_bus_opt` and `event_emitter_ref` — find the exact variable names used in the init function for the domain event bus and event emitter. They may differ slightly (e.g., `domain_event_bus` vs `event_channels.domain_bus`).

Add `brain_voice` to the AppCore struct construction at the bottom of `init_with_sender`.

- [ ] **Step 3: Verify it compiles**

Run: `cargo build -p app-core`
Expected: compiles with 0 errors.

- [ ] **Step 4: Wire focus_active to desktop's FOCUS_ACTIVE**

Open `crates/desktop/src/app_core.rs`. After `AppCore::init_with_sender()` returns, sync the focus_active flag. The simplest approach: spawn a tiny task that polls `tray_countdown::FOCUS_ACTIVE` and mirrors it to `core.brain_voice.focus_active`. Alternatively, expose the `Arc<AtomicBool>` from BrainVoice and set it in the desktop setup.

The exact wiring depends on the desktop init flow — the implementation agent should find where `tray_countdown::FOCUS_ACTIVE` is set (in `focus_timer.rs`) and ensure BrainVoice's `focus_active` points to the same `Arc<AtomicBool>`, or subscribe BrainVoice to `FocusSessionStarted`/`FocusSessionEnded` events on the bus (which it already does — see `handle_signal` checking `focus_active.load()`).

**Recommended approach:** Instead of sharing the static, BrainVoice already receives `FocusSessionStarted`/`FocusSessionEnded` events via the bus. Update the run loop in `brain_voice.rs` to track focus state from these events instead of relying on the AtomicBool:

In `run_brain_voice`, add to the event match:
```rust
DomainEvent::FocusSessionStarted { .. } => {
    focus_active.store(true, Ordering::Relaxed);
}
DomainEvent::FocusSessionEnded { .. } => {
    focus_active.store(false, Ordering::Relaxed);
    // flush_deferred handled below
}
```

This makes BrainVoice self-contained — no external wiring needed.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/state.rs crates/app-core/src/init/mod.rs
git commit -m "feat(app-core): wire BrainVoice into AppCore initialization"
```

---

### Task 5: Memory Promotion Event Emission

**Files:**
- Modify: `crates/cognitive/src/services/memory_promotion.rs`
- Test: `cargo nextest run -p cognitive -E 'test(promote)'`

- [ ] **Step 1: Write test for event emission on promote**

Open `crates/cognitive/src/services/memory_promotion.rs`. Add a test:

```rust
    #[tokio::test]
    async fn test_promote_fact_emits_event() {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let repo = SemanticFactRepo::new(pool.inner().clone());
        let bus = Arc::new(bus::DomainEventBus::new(16));
        let mut rx = bus.subscribe();

        // Insert a test fact
        let fact = SemanticFact {
            id: "f1".into(),
            scope_type: "persona".into(),
            scope_id: Some("p1".into()),
            fact_text: "User prefers dark mode".into(),
            source: "extraction".into(),
            recorded_at: chrono::Utc::now().to_rfc3339(),
            // ... fill required fields based on SemanticFact struct
        };
        repo.upsert(&fact).await.unwrap();

        let promoted = promote_fact(&repo, "f1", "squad", None, Some(&bus)).await.unwrap();
        assert!(promoted.is_some());

        // Check event was emitted
        let event = rx.try_recv().unwrap();
        match event {
            bus::DomainEvent::MemoryPromoted { fact_id, summary, from_scope, to_scope } => {
                assert_eq!(from_scope, "persona");
                assert_eq!(to_scope, "squad");
                assert!(summary.contains("dark mode"));
            }
            _ => panic!("Expected MemoryPromoted event"),
        }
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p cognitive -E 'test(promote_fact_emits)'`
Expected: FAIL — `promote_fact` doesn't accept a bus parameter.

- [ ] **Step 3: Add optional bus parameter to promote_fact**

Update the function signature:

```rust
pub async fn promote_fact(
    repo: &SemanticFactRepo,
    fact_id: &str,
    target_scope_type: &str,
    target_scope_id: Option<&str>,
    bus: Option<&bus::DomainEventBus>,
) -> Result<Option<SemanticFact>, sqlx::Error> {
    let original = repo.get(fact_id).await?;
    let Some(original) = original else {
        return Ok(None);
    };

    let promoted = SemanticFact {
        id: Uuid::new_v4().to_string(),
        scope_type: target_scope_type.to_string(),
        scope_id: target_scope_id.map(|s| s.to_string()),
        source: format!("promoted:{}", original.source),
        recorded_at: chrono::Utc::now().to_rfc3339(),
        ..original.clone()
    };

    repo.upsert(&promoted).await?;

    // Emit event if bus is provided
    if let Some(bus) = bus {
        bus.publish(bus::DomainEvent::MemoryPromoted {
            fact_id: promoted.id.clone(),
            summary: promoted.fact_text.clone(),
            from_scope: original.scope_type.clone(),
            to_scope: promoted.scope_type.clone(),
        });
    }

    Ok(Some(promoted))
}
```

Update existing callers and tests to pass `None` for the bus parameter (backward compatible).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo nextest run -p cognitive -E 'test(promote)'`
Expected: all tests PASS.

- [ ] **Step 5: Find and update all callsites**

Search for all callers of `promote_fact` outside of tests:

```bash
rg 'promote_fact\(' crates/ --type rust -l
```

Update each to pass the `DomainEventBus` reference if available, or `None` if in a context without bus access.

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/
git commit -m "feat(cognitive): emit MemoryPromoted event on fact promotion"
```

---

### Task 6: Cross-Domain Heuristic (Pure Function)

**Files:**
- Create: `crates/feature-insights/src/cross_domain.rs`
- Modify: `crates/feature-insights/src/lib.rs` (add `pub mod cross_domain;`)
- Test: inline `#[cfg(test)] mod tests`

This is the pure function that takes an entity + vector search results + feedback history and returns `Option<CrossDomainDot>`. No state, no background thread, no bus interaction.

- [ ] **Step 1: Define types**

Create `crates/feature-insights/src/cross_domain.rs`:

```rust
use serde::Serialize;

/// Which domain an entity belongs to.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityDomain {
    Task,
    Note,
    Finance,
    Productivity,
}

/// Reference to a cross-domain entity.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityRef {
    pub domain: EntityDomain,
    pub id: String,
    pub title: String,
}

/// Which signal layers matched.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Layer {
    SemanticOverlap { cosine: f64 },
    TemporalProximity { days_apart: i64 },
    FrequencySignal { mentions: u32, features: u32 },
}

/// A detected cross-domain connection.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CrossDomainDot {
    pub source: EntityRef,
    pub target: EntityRef,
    pub layers_matched: Vec<Layer>,
    pub confidence: f64,
    pub tooltip: String,
    pub detail_route: String,
}

/// Pre-fetched context for the heuristic.
pub struct HeuristicInput {
    pub source: EntityRef,
    pub source_created: chrono::DateTime<chrono::Utc>,
    /// Vector search results from target domains: (entity_ref, cosine_score, created_at)
    pub vector_hits: Vec<(EntityRef, f64, chrono::DateTime<chrono::Utc>)>,
    /// Frequency data: (entity_pair, mention_count, feature_count)
    pub frequency_data: Vec<(String, u32, u32)>,
    /// Whether the source entity has enough content (>= 10 chars)
    pub source_has_content: bool,
}

/// Configuration thresholds.
pub struct HeuristicConfig {
    pub min_cosine: f64,
    pub max_temporal_days: i64,
    pub min_frequency_mentions: u32,
    pub min_frequency_features: u32,
    pub min_layers: usize,
}

impl Default for HeuristicConfig {
    fn default() -> Self {
        Self {
            min_cosine: 0.72,
            max_temporal_days: 7,
            min_frequency_mentions: 2,
            min_frequency_features: 2,
            min_layers: 2,
        }
    }
}
```

- [ ] **Step 2: Write tests for the heuristic**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn task_ref(id: &str, title: &str) -> EntityRef {
        EntityRef { domain: EntityDomain::Task, id: id.into(), title: title.into() }
    }

    fn finance_ref(id: &str, title: &str) -> EntityRef {
        EntityRef { domain: EntityDomain::Finance, id: id.into(), title: title.into() }
    }

    #[test]
    fn test_two_layers_match_returns_dot() {
        let now = Utc::now();
        let input = HeuristicInput {
            source: task_ref("t1", "Q2 investor deck"),
            source_created: now,
            vector_hits: vec![(
                finance_ref("f1", "March consulting spend"),
                0.78,
                now - chrono::Duration::days(3),
            )],
            frequency_data: vec![],
            source_has_content: true,
        };
        let config = HeuristicConfig::default();
        let result = evaluate_cross_domain(&input, &config);
        assert!(result.is_some());
        let dot = result.unwrap();
        assert_eq!(dot.layers_matched.len(), 2); // semantic + temporal
        assert!(dot.confidence >= 0.72);
    }

    #[test]
    fn test_one_layer_returns_none() {
        let now = Utc::now();
        let input = HeuristicInput {
            source: task_ref("t1", "Q2 investor deck"),
            source_created: now,
            vector_hits: vec![(
                finance_ref("f1", "March consulting spend"),
                0.78,
                // 30 days ago — temporal doesn't match
                now - chrono::Duration::days(30),
            )],
            frequency_data: vec![],
            source_has_content: true,
        };
        let config = HeuristicConfig::default();
        let result = evaluate_cross_domain(&input, &config);
        assert!(result.is_none()); // only semantic, no temporal or frequency
    }

    #[test]
    fn test_low_cosine_returns_none() {
        let now = Utc::now();
        let input = HeuristicInput {
            source: task_ref("t1", "Q2 deck"),
            source_created: now,
            vector_hits: vec![(
                finance_ref("f1", "Grocery shopping"),
                0.45, // below 0.72
                now - chrono::Duration::days(1),
            )],
            frequency_data: vec![],
            source_has_content: true,
        };
        let config = HeuristicConfig::default();
        assert!(evaluate_cross_domain(&input, &config).is_none());
    }

    #[test]
    fn test_short_content_returns_none() {
        let now = Utc::now();
        let input = HeuristicInput {
            source: task_ref("t1", "TODO"),
            source_created: now,
            vector_hits: vec![(finance_ref("f1", "misc"), 0.85, now)],
            frequency_data: vec![],
            source_has_content: false, // <10 chars
        };
        let config = HeuristicConfig::default();
        assert!(evaluate_cross_domain(&input, &config).is_none());
    }

    #[test]
    fn test_all_three_layers_match() {
        let now = Utc::now();
        let input = HeuristicInput {
            source: task_ref("t1", "Q2 investor deck"),
            source_created: now,
            vector_hits: vec![(
                finance_ref("f1", "March consulting spend"),
                0.82,
                now - chrono::Duration::days(2),
            )],
            frequency_data: vec![("task:t1|finance:f1".into(), 3, 2)],
            source_has_content: true,
        };
        let config = HeuristicConfig::default();
        let dot = evaluate_cross_domain(&input, &config).unwrap();
        assert_eq!(dot.layers_matched.len(), 3);
    }

    #[test]
    fn test_tooltip_template_task_finance() {
        let source = task_ref("t1", "Q2 budget prep");
        let target = finance_ref("f1", "Consulting spend");
        let tooltip = build_tooltip(&source, &target);
        assert!(tooltip.contains("Q2 budget prep"));
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo nextest run -p feature-insights -E 'test(cross_domain)'`
Expected: FAIL — `evaluate_cross_domain` and `build_tooltip` not defined.

- [ ] **Step 4: Implement evaluate_cross_domain**

```rust
/// Evaluate whether a source entity has meaningful cross-domain connections.
/// Returns the strongest connection if >= min_layers match.
pub fn evaluate_cross_domain(
    input: &HeuristicInput,
    config: &HeuristicConfig,
) -> Option<CrossDomainDot> {
    if !input.source_has_content {
        return None;
    }

    let mut best: Option<(EntityRef, Vec<Layer>, f64)> = None;

    for (target, cosine, target_created) in &input.vector_hits {
        if *cosine < config.min_cosine {
            continue;
        }

        let mut layers = Vec::new();

        // Layer 1: Semantic overlap
        layers.push(Layer::SemanticOverlap { cosine: *cosine });

        // Layer 2: Temporal proximity
        let days_apart = (input.source_created - *target_created).num_days().abs();
        if days_apart <= config.max_temporal_days {
            layers.push(Layer::TemporalProximity { days_apart });
        }

        // Layer 3: Frequency signal
        let entity_pair = format!(
            "{}:{}|{}:{}",
            domain_str(&input.source.domain),
            input.source.id,
            domain_str(&target.domain),
            target.id,
        );
        if let Some((_, mentions, features)) = input
            .frequency_data
            .iter()
            .find(|(pair, _, _)| *pair == entity_pair)
        {
            if *mentions >= config.min_frequency_mentions
                && *features >= config.min_frequency_features
            {
                layers.push(Layer::FrequencySignal {
                    mentions: *mentions,
                    features: *features,
                });
            }
        }

        if layers.len() >= config.min_layers {
            let score = *cosine;
            if best.as_ref().map(|(_, _, s)| score > *s).unwrap_or(true) {
                best = Some((target.clone(), layers, score));
            }
        }
    }

    let (target, layers, confidence) = best?;

    let tooltip = build_tooltip(&input.source, &target);
    let detail_route = format!(
        "/brain?filter=cross-domain&source={}:{}&target={}:{}",
        domain_str(&input.source.domain),
        input.source.id,
        domain_str(&target.domain),
        target.id,
    );

    Some(CrossDomainDot {
        source: input.source.clone(),
        target,
        layers_matched: layers,
        confidence,
        tooltip,
        detail_route,
    })
}

fn domain_str(domain: &EntityDomain) -> &'static str {
    match domain {
        EntityDomain::Task => "task",
        EntityDomain::Note => "note",
        EntityDomain::Finance => "finance",
        EntityDomain::Productivity => "productivity",
    }
}
```

- [ ] **Step 5: Implement build_tooltip with 7 templates**

```rust
/// Build a first-person, one-clause tooltip based on connection type.
pub fn build_tooltip(source: &EntityRef, target: &EntityRef) -> String {
    match (&source.domain, &target.domain) {
        (EntityDomain::Task, EntityDomain::Finance) => {
            format!(
                "Your \"{}\" connects to {} — want me to pull the numbers?",
                source.title, target.title,
            )
        }
        (EntityDomain::Task, EntityDomain::Note) => {
            format!(
                "You wrote about this topic in a note — want me to pull \"{}\"?",
                target.title,
            )
        }
        (EntityDomain::Task, EntityDomain::Productivity) => {
            format!(
                "Last time you had a similar task you slipped — want me to block focus time?",
            )
        }
        (EntityDomain::Finance, EntityDomain::Note) => {
            format!(
                "Your notes reference this expense category — want the full history?",
            )
        }
        (EntityDomain::Note, EntityDomain::Finance) => {
            format!(
                "Your note predicted this spending pattern — want me to log it as an insight?",
            )
        }
        (EntityDomain::Finance, EntityDomain::Task) => {
            format!(
                "Your \"{}\" has fresh numbers ready — want me to prep a summary?",
                target.title,
            )
        }
        _ => {
            format!(
                "I noticed a connection between \"{}\" and \"{}\"",
                source.title, target.title,
            )
        }
    }
}
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo nextest run -p feature-insights -E 'test(cross_domain)'`
Expected: all tests PASS.

- [ ] **Step 7: Register the module**

Add `pub mod cross_domain;` to `crates/feature-insights/src/lib.rs`.

- [ ] **Step 8: Commit**

```bash
git add crates/feature-insights/src/cross_domain.rs crates/feature-insights/src/lib.rs
git commit -m "feat(insights): embedding-based cross-domain heuristic with 7 tooltip templates"
```

---

### Task 7: Wire Cross-Domain into InsightService

**Files:**
- Modify: `crates/feature-insights/src/service.rs`
- Modify: `crates/app-core/src/init/mod.rs` (pass bus to InsightService)
- Test: `cargo nextest run -p feature-insights`

- [ ] **Step 1: Add check_cross_domain method to InsightService**

Open `crates/feature-insights/src/service.rs`. Add a new field for the domain event bus and vector store:

```rust
use crate::cross_domain::{
    CrossDomainDot, EntityDomain, EntityRef, HeuristicConfig, HeuristicInput,
    evaluate_cross_domain,
};
```

Add to `InsightService` struct:

```rust
    pub(crate) domain_bus: Option<Arc<bus::DomainEventBus>>,
```

Add the method:

```rust
    /// Check for cross-domain connections when a user views an entity.
    /// Runs synchronously (< 80ms via LanceDB). Emits CrossDomainDotReady if found.
    pub async fn check_cross_domain(
        &self,
        source_domain: EntityDomain,
        source_id: &str,
        source_title: &str,
        source_created: chrono::DateTime<chrono::Utc>,
    ) -> Result<Option<CrossDomainDot>, common::KlyntbotError> {
        // Skip if content too short
        if source_title.len() < 10 {
            return Ok(None);
        }

        // Embed the source (should already be computed — fetch embedding)
        let embedding = self.embedder.get_embedding(source_id).await?;
        let Some(embedding) = embedding else {
            return Ok(None);
        };

        // Search target domains
        let target_domains: Vec<&str> = match source_domain {
            EntityDomain::Task => vec!["note", "finance"],
            EntityDomain::Note => vec!["task", "finance"],
            EntityDomain::Finance => vec!["task", "note"],
            EntityDomain::Productivity => vec!["task", "note", "finance"],
        };

        let mut vector_hits = Vec::new();
        for domain in &target_domains {
            // Use existing VectorStore::search_similar or search_cognitive_facts
            // The exact table names depend on the codebase — discover via:
            //   rg 'open_table\("' crates/storage/src/vector_store/ --type rust
            let table_name = match *domain {
                "task" => "task_embeddings",
                "note" => "note_embeddings",
                "finance" => "finance_embeddings",
                _ => continue,
            };

            // TODO: Implementation agent must verify exact table names and adapt
            // the vector store API call. The pattern from crud.rs is:
            //   vector_store.search_similar(table, &embedding, 3, 0.72)
            //   Returns: Vec<(id: String, score: f64)>
            // Then look up the entity metadata (title, created_at) from SQLite.
        }

        let config = HeuristicConfig::default();
        let input = HeuristicInput {
            source: EntityRef {
                domain: source_domain,
                id: source_id.into(),
                title: source_title.into(),
            },
            source_created,
            vector_hits,
            frequency_data: vec![], // Phase 1: skip frequency layer
            source_has_content: true,
        };

        let dot = evaluate_cross_domain(&input, &config);

        // Emit event if connection found
        if let (Some(dot), Some(bus)) = (&dot, &self.domain_bus) {
            bus.publish(bus::DomainEvent::CrossDomainDotReady {
                source_kind: crate::cross_domain::domain_str(&dot.source.domain).into(),
                source_id: dot.source.id.clone(),
                source_title: dot.source.title.clone(),
                target_kind: crate::cross_domain::domain_str(&dot.target.domain).into(),
                target_id: dot.target.id.clone(),
                target_title: dot.target.title.clone(),
                confidence: dot.confidence,
                tooltip: dot.tooltip.clone(),
                detail_route: Some(dot.detail_route.clone()),
            });
        }

        Ok(dot)
    }
```

- [ ] **Step 2: Update InsightService constructor to accept bus**

Add `domain_bus: Option<Arc<bus::DomainEventBus>>` parameter to `InsightService::new()`. Update the initialization in `crates/app-core/src/init/mod.rs` to pass the bus.

- [ ] **Step 3: Call check_cross_domain from Tauri task/note/finance detail commands**

In `crates/desktop/src/commands/tasks.rs` (and similar for notes, finance), when the user views a detail view, call:

```rust
if let Some(insight_svc) = &state.insight_service {
    // Fire and forget — don't block the UI
    let svc = Arc::clone(insight_svc);
    tokio::spawn(async move {
        let _ = svc.check_cross_domain(
            EntityDomain::Task,
            &task_id,
            &task.title,
            task.created_at,
        ).await;
    });
}
```

- [ ] **Step 4: Verify it compiles and tests pass**

Run: `cargo build --workspace && cargo nextest run -p feature-insights`
Expected: compiles, existing tests still pass.

- [ ] **Step 5: Commit**

```bash
git add crates/feature-insights/ crates/app-core/ crates/desktop/
git commit -m "feat(insights): wire cross-domain heuristic into InsightService with event emission"
```

---

### Task 8: Focus-Aware Message Deferral in AgentLoop

**Files:**
- Modify: `crates/agent/src/agent_loop/mod.rs`
- Test: `cargo nextest run -p agent`

The AgentLoop's `run_with_rx` currently uses a `timeout`-based recv. We convert it to `tokio::select!` to also listen for `FocusSessionStarted`/`FocusSessionEnded` events on the DomainEventBus.

- [ ] **Step 1: Add DomainEventBus to AgentLoop**

Open `crates/agent/src/agent_loop/mod.rs`. Add a field to the AgentLoop struct:

```rust
    pub domain_event_bus: Option<Arc<bus::DomainEventBus>>,
```

Update the constructor to accept it. Find the builder/new function and add the parameter.

- [ ] **Step 2: Add auto-reply config field**

Open `crates/config/src/schema/` — find the productivity config section. Add:

```rust
    /// Focus bubble auto-reply settings.
    #[serde(default)]
    pub focus_bubble: FocusBubbleConfig,
```

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusBubbleConfig {
    /// Whether to auto-reply to senders during focus. Off by default.
    #[serde(default)]
    pub auto_reply_enabled: bool,
    /// Custom auto-reply text.
    #[serde(default = "default_auto_reply")]
    pub auto_reply_text: String,
}

fn default_auto_reply() -> String {
    "I'm in a deep focus session right now. I'll get back to you when I'm done.".into()
}

impl Default for FocusBubbleConfig {
    fn default() -> Self {
        Self {
            auto_reply_enabled: false,
            auto_reply_text: default_auto_reply(),
        }
    }
}
```

- [ ] **Step 3: Modify run_with_rx to use tokio::select!**

Replace the current `run_with_rx` implementation (~lines 263-288):

```rust
    pub async fn run_with_rx(&self, mut inbound_rx: mpsc::Receiver<InboundMessage>) -> Result<()> {
        self.running.store(true, Ordering::SeqCst);

        // Subscribe to domain events for focus state tracking
        let mut event_rx = self.domain_event_bus.as_ref().map(|bus| bus.subscribe());
        let mut focus_active = false;
        let mut deferred_messages: Vec<InboundMessage> = Vec::new();

        while self.running.load(Ordering::SeqCst) {
            tokio::select! {
                msg = inbound_rx.recv() => {
                    let Some(msg) = msg else { break };

                    if focus_active {
                        // Defer during focus
                        if let Some(ref bus) = self.domain_event_bus {
                            bus.publish(DomainEvent::MessageDeferred {
                                channel: msg.channel.to_string(),
                                sender: msg.sender_name.clone().unwrap_or_default(),
                                preview: msg.text.chars().take(100).collect(),
                            });
                        }
                        deferred_messages.push(msg);
                    } else {
                        if let Err(e) = self.process_message(msg).await {
                            error!("Error processing message: {}", e);
                        }
                    }
                }
                result = async {
                    match event_rx.as_mut() {
                        Some(rx) => rx.recv().await,
                        None => std::future::pending().await,
                    }
                } => {
                    match result {
                        Ok(DomainEvent::FocusSessionStarted { .. }) => {
                            focus_active = true;
                            tracing::info!("AgentLoop: focus started, deferring inbound messages");
                        }
                        Ok(DomainEvent::FocusSessionEnded { .. }) => {
                            focus_active = false;
                            tracing::info!(
                                "AgentLoop: focus ended, processing {} deferred messages",
                                deferred_messages.len()
                            );
                            for msg in deferred_messages.drain(..) {
                                if let Err(e) = self.process_message(msg).await {
                                    error!("Error processing deferred message: {}", e);
                                }
                            }
                        }
                        Ok(_) => {} // ignore other events
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!("AgentLoop event rx lagged {n}");
                        }
                        Err(broadcast::error::RecvError::Closed) => {}
                    }
                }
            }
        }

        Ok(())
    }
```

- [ ] **Step 3: Wire the bus in AgentLoop initialization**

Find where AgentLoop is constructed (likely in `crates/app-core/src/init/mod.rs`). Pass the `domain_event_bus` when building AgentLoop.

- [ ] **Step 4: Verify it compiles**

Run: `cargo build -p agent`
Expected: compiles with 0 errors.

- [ ] **Step 5: Write integration test for focus deferral**

Add a test in the agent crate's test module:

```rust
#[tokio::test]
async fn test_focus_defers_inbound_messages() {
    // Setup: create AgentLoop with a test bus
    let bus = Arc::new(DomainEventBus::new(16));
    // ... create minimal AgentLoop with the bus ...

    let (tx, rx) = mpsc::channel(16);
    let mut event_rx = bus.subscribe();

    // Start focus
    bus.publish(DomainEvent::FocusSessionStarted {
        session_type: "deep".into(),
        target_mins: 25,
    });

    // Send a message during focus
    tx.send(InboundMessage { /* test message */ }).await.unwrap();

    // Verify MessageDeferred was emitted
    // ... check event_rx for MessageDeferred ...

    // End focus
    bus.publish(DomainEvent::FocusSessionEnded {
        duration_secs: 1500,
        quality: 0.8,
        interruptions: 0,
    });

    // Verify deferred message was processed
    // ... check agent processed it ...
}
```

- [ ] **Step 6: Run tests**

Run: `cargo nextest run -p agent -E 'test(focus_defer)'`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/agent/ crates/app-core/
git commit -m "feat(agent): defer inbound messages during focus sessions, emit MessageDeferred"
```

---

### Task 9: Frontend useAmbientSignals Hook

**Files:**
- Create: `desktop-ui/src/shared/hooks/useAmbientSignals.ts`
- Test: `cd desktop-ui && bun run test`

- [ ] **Step 1: Create the hook**

Create `desktop-ui/src/shared/hooks/useAmbientSignals.ts`:

```typescript
import { useState, useCallback } from "react";
import { useEvent } from "@shared/hooks/useEvent";

export type SignalMode = "pulse" | "badge" | "deferred" | "merged";

export interface SignalSummary {
  signalType: string;
  entityPair: string;
  headline: string;
}

export interface BrainAmbientEvent {
  mode: SignalMode;
  signals: SignalSummary[];
  tooltip: string;
  detailRoute: string | null;
}

export interface AmbientSignalState {
  /** The most recent event (null if no signals yet). */
  current: BrainAmbientEvent | null;
  /** Badge count (incremented on badge events, reset on click). */
  badgeCount: number;
  /** Whether the orb should be in pulse animation state. */
  isPulsing: boolean;
  /** Whether focus is active (shield overlay). */
  isFocusDeferred: boolean;
  /** All accumulated badge signals (for the summary list). */
  badgeSignals: SignalSummary[];
  /** Acknowledge the current signal (resets pulse state). */
  acknowledge: () => void;
  /** Clear badge count (on click). */
  clearBadge: () => void;
}

export function useAmbientSignals(): AmbientSignalState {
  const [current, setCurrent] = useState<BrainAmbientEvent | null>(null);
  const [badgeCount, setBadgeCount] = useState(0);
  const [badgeSignals, setBadgeSignals] = useState<SignalSummary[]>([]);
  const [isPulsing, setIsPulsing] = useState(false);
  const [isFocusDeferred, setIsFocusDeferred] = useState(false);

  useEvent<BrainAmbientEvent>("brain:ambient", (event) => {
    setCurrent(event);

    switch (event.mode) {
      case "pulse":
      case "merged":
        setIsPulsing(true);
        // Auto-reset pulse after animation duration (2s fade)
        setTimeout(() => setIsPulsing(false), 2000);
        break;
      case "badge":
        setBadgeCount((c) => c + event.signals.length);
        setBadgeSignals((prev) => [...prev, ...event.signals]);
        break;
      case "deferred":
        setIsFocusDeferred(false); // Focus just ended — show debrief
        setIsPulsing(true);
        setTimeout(() => setIsPulsing(false), 2000);
        break;
    }
  });

  // Listen for focus state changes
  useEvent<{ active: boolean }>("focus:state", (payload) => {
    setIsFocusDeferred(payload.active);
  });

  const acknowledge = useCallback(() => {
    setIsPulsing(false);
    setCurrent(null);
  }, []);

  const clearBadge = useCallback(() => {
    setBadgeCount(0);
    setBadgeSignals([]);
  }, []);

  return {
    current,
    badgeCount,
    isPulsing,
    isFocusDeferred,
    badgeSignals,
    acknowledge,
    clearBadge,
  };
}
```

- [ ] **Step 2: Write test**

Create `desktop-ui/src/shared/hooks/useAmbientSignals.test.ts`:

```typescript
import { describe, expect, it } from "vitest";
// Test the types compile correctly
import type { BrainAmbientEvent, SignalMode } from "./useAmbientSignals";

describe("useAmbientSignals types", () => {
  it("should have correct signal modes", () => {
    const modes: SignalMode[] = ["pulse", "badge", "deferred", "merged"];
    expect(modes).toHaveLength(4);
  });

  it("should match backend event shape", () => {
    const event: BrainAmbientEvent = {
      mode: "pulse",
      signals: [
        {
          signalType: "memory_promoted",
          entityPair: "fact:1",
          headline: "test",
        },
      ],
      tooltip: "Test tooltip",
      detailRoute: "/brain",
    };
    expect(event.signals).toHaveLength(1);
  });
});
```

- [ ] **Step 3: Run tests**

Run: `cd desktop-ui && bun run test`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/shared/hooks/useAmbientSignals.ts desktop-ui/src/shared/hooks/useAmbientSignals.test.ts
git commit -m "feat(ui): useAmbientSignals hook for BrainVoice Tauri events"
```

---

### Task 10: BrainOrb Component

**Files:**
- Create: `desktop-ui/src/shared/components/BrainOrb.tsx`
- Modify: `desktop-ui/src/styles/theme.css` (add orb keyframes)
- Test: `cd desktop-ui && bun run test`

- [ ] **Step 1: Add orb animations to theme.css**

Open `desktop-ui/src/styles/theme.css`. Find the existing `@keyframes` section (~lines 328-448). Add:

```css
@keyframes orb-pulse {
  0% {
    transform: scale(1);
    box-shadow: 0 0 0 0 rgba(245, 158, 11, 0.4);
  }
  50% {
    transform: scale(1.3);
    box-shadow: 0 0 12px 4px rgba(245, 158, 11, 0.2);
  }
  100% {
    transform: scale(1);
    box-shadow: 0 0 0 0 rgba(245, 158, 11, 0);
  }
}

@keyframes orb-breathe {
  0%, 100% {
    opacity: 0.3;
  }
  50% {
    opacity: 0.45;
  }
}
```

- [ ] **Step 2: Create BrainOrb component**

Create `desktop-ui/src/shared/components/BrainOrb.tsx`:

```tsx
import { useState, useRef, useEffect } from "react";
import { Shield, Brain } from "lucide-react";
import { useAmbientSignals } from "@shared/hooks/useAmbientSignals";
import { useNavigate } from "react-router-dom";

export function BrainOrb() {
  const {
    current,
    badgeCount,
    isPulsing,
    isFocusDeferred,
    badgeSignals,
    acknowledge,
    clearBadge,
  } = useAmbientSignals();
  const navigate = useNavigate();
  const [showTooltip, setShowTooltip] = useState(false);
  const tooltipTimeout = useRef<ReturnType<typeof setTimeout>>();

  // Auto-dismiss tooltip after 8s
  useEffect(() => {
    if (showTooltip) {
      tooltipTimeout.current = setTimeout(() => setShowTooltip(false), 8000);
      return () => clearTimeout(tooltipTimeout.current);
    }
  }, [showTooltip]);

  const handleMouseEnter = () => {
    if (isPulsing || badgeCount > 0) {
      setShowTooltip(true);
    }
  };

  const handleMouseLeave = () => {
    // Only auto-dismiss if not hovering
    tooltipTimeout.current = setTimeout(() => setShowTooltip(false), 300);
  };

  const handleClick = () => {
    setShowTooltip(false);

    if (current?.detailRoute) {
      navigate(current.detailRoute);
      acknowledge();
    } else if (badgeCount > 0) {
      setShowTooltip(true); // Toggle badge summary
    } else {
      navigate("/brain");
    }

    clearBadge();
  };

  return (
    <div className="relative">
      {/* The orb dot */}
      <button
        type="button"
        onClick={handleClick}
        onMouseEnter={handleMouseEnter}
        onMouseLeave={handleMouseLeave}
        className="relative flex items-center justify-center size-7 rounded-full transition-all duration-200"
        aria-label="Second brain status"
      >
        {/* Base dot */}
        <div
          className={`size-2.5 rounded-full transition-all duration-500 ${
            isPulsing
              ? "bg-amber-400 animate-[orb-pulse_600ms_ease-out]"
              : isFocusDeferred
                ? "bg-muted opacity-50"
                : badgeCount > 0
                  ? "bg-amber-400/60 animate-[orb-breathe_8s_ease-in-out_infinite]"
                  : "bg-muted opacity-30"
          }`}
        />

        {/* Badge counter */}
        {badgeCount > 0 && !isPulsing && (
          <span className="absolute -top-0.5 -right-0.5 size-3.5 flex items-center justify-center rounded-full bg-amber-500 text-[9px] font-medium text-white">
            {badgeCount > 9 ? "9+" : badgeCount}
          </span>
        )}

        {/* Focus shield overlay */}
        {isFocusDeferred && (
          <Shield
            className="absolute size-2 text-muted-foreground opacity-50"
            strokeWidth={2}
          />
        )}
      </button>

      {/* Tooltip panel */}
      {showTooltip && (
        <div
          className="absolute top-full right-0 mt-2 w-80 glass-panel p-3 animate-[glass-appear_0.2s_ease-out] z-50"
          onMouseEnter={() => clearTimeout(tooltipTimeout.current)}
          onMouseLeave={handleMouseLeave}
        >
          {isPulsing && current ? (
            // Pulse tooltip — first person voice
            <div className="space-y-2">
              <p className="text-sm text-foreground">{current.tooltip}</p>
              {current.detailRoute && (
                <button
                  type="button"
                  onClick={() => {
                    navigate(current.detailRoute!);
                    setShowTooltip(false);
                    acknowledge();
                  }}
                  className="text-xs text-amber-400 hover:text-amber-300 transition-colors"
                >
                  See more →
                </button>
              )}
            </div>
          ) : badgeCount > 0 ? (
            // Badge summary — neutral voice
            <div className="space-y-2">
              <p className="text-xs text-muted-foreground font-medium">
                {badgeCount} new connection{badgeCount !== 1 ? "s" : ""}
              </p>
              <div className="border-t border-border pt-2 space-y-1">
                {badgeSignals.map((signal, i) => (
                  <p key={`${signal.entityPair}-${i}`} className="text-xs text-foreground">
                    {signal.headline}
                  </p>
                ))}
              </div>
              <button
                type="button"
                onClick={() => {
                  navigate("/brain");
                  setShowTooltip(false);
                  clearBadge();
                }}
                className="text-xs text-amber-400 hover:text-amber-300 transition-colors"
              >
                See all →
              </button>
            </div>
          ) : null}
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 3: Run lint and tests**

Run: `cd desktop-ui && bun run lint:fix && bun run test`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/shared/components/BrainOrb.tsx desktop-ui/src/styles/theme.css
git commit -m "feat(ui): BrainOrb component with pulse, badge, focus-shield, and glass tooltips"
```

---

### Task 11: Mount BrainOrb in AppShell Global Top Bar

**Files:**
- Modify: `desktop-ui/src/app/layouts/AppShell.tsx`

- [ ] **Step 1: Add a header area and mount BrainOrb**

Open `desktop-ui/src/app/layouts/AppShell.tsx`. The current layout is: `Sidebar | Outlet | SidebarChat`. We need to add the BrainOrb to the top-right of the main content area.

Find the `<Outlet />` rendering. Wrap it with a header:

```tsx
import { BrainOrb } from "@shared/components/BrainOrb";

// Inside the return, replace the bare <Outlet /> with:
<div className="flex-1 flex flex-col overflow-hidden">
  {/* Global top bar */}
  <div className="flex items-center justify-end px-3 py-1.5 shrink-0">
    <BrainOrb />
  </div>
  {/* Route content */}
  <div className="flex-1 overflow-hidden">
    <Outlet />
  </div>
</div>
```

This adds a thin header strip above the route content with the BrainOrb positioned top-right. The header is minimal — no background, no border. Just the orb floating in the corner.

- [ ] **Step 2: Verify it renders**

Run: `cd desktop-ui && bun run dev`
Open `localhost:1420`. The orb should appear as a dim dot in the top-right corner of the main content area, visible across all routes.

- [ ] **Step 3: Run lint**

Run: `cd desktop-ui && bun run lint:fix`
Expected: no errors.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/app/layouts/AppShell.tsx
git commit -m "feat(ui): mount BrainOrb in global top bar across all routes"
```

---

### Task 12: Mirror → Brain Rename

**Files:**
- Modify: `desktop-ui/src/app/router.tsx`
- Modify: `desktop-ui/src/app/layouts/Sidebar.tsx`
- Modify: any files referencing `/mirror` route

- [ ] **Step 1: Update router**

Open `desktop-ui/src/app/router.tsx`. Change:

```typescript
// Before:
{ path: "/mirror", element: <MirrorPage /> },

// After:
{ path: "/brain", element: <MirrorPage /> },
```

- [ ] **Step 2: Update sidebar nav item**

Open `desktop-ui/src/app/layouts/Sidebar.tsx`. Find the Mirror item in the items array (~line 36). Change:

```typescript
// Before:
{ key: "Mirror", icon: Eye, path: "/mirror" },

// After:
{ key: "Brain", icon: Brain, path: "/brain" },
```

Update the import to include `Brain` from `lucide-react` (add to the existing import). Remove `Eye` if unused elsewhere.

- [ ] **Step 3: Find and update all references to /mirror**

Search: `rg '"/mirror"' desktop-ui/src/ --type ts --type tsx`

Update each reference to `"/brain"`. This includes:
- Any `navigate("/mirror")` calls
- Any `Link to="/mirror"` components
- Any `detail_route` references in BrainOrb (already uses `/brain`)

- [ ] **Step 4: Run lint and dev server**

Run: `cd desktop-ui && bun run lint:fix && bun run dev`
Navigate to `/brain` in the browser. Verify the Mirror page loads.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/
git commit -m "refactor(ui): rename Mirror to Brain in nav and routes"
```

---

### Task 13: Focus Debrief Panel + Brain Nav Pulse Indicator

**Files:**
- Create: `desktop-ui/src/features/productivity/components/FocusDebrief.tsx`
- Modify: `desktop-ui/src/app/layouts/Sidebar.tsx` (pulse indicator)

- [ ] **Step 1: Create FocusDebrief component**

Create `desktop-ui/src/features/productivity/components/FocusDebrief.tsx`:

```tsx
import { useState } from "react";
import { MessageSquare, Brain, Sparkles, ChevronDown, X } from "lucide-react";
import type { SignalSummary } from "@shared/hooks/useAmbientSignals";

interface FocusDebriefProps {
  signals: SignalSummary[];
  tooltip: string;
  onClose: () => void;
}

function CollapsibleSection({
  title,
  icon: Icon,
  children,
  count,
}: {
  title: string;
  icon: React.ElementType;
  children: React.ReactNode;
  count: number;
}) {
  const [open, setOpen] = useState(true);
  if (count === 0) return null;

  return (
    <div>
      <button
        type="button"
        onClick={() => setOpen(!open)}
        className="w-full flex items-center gap-2 px-3 py-2 rounded-t-[var(--radius-2xl)] bg-accent"
      >
        <Icon className="size-3 text-muted-foreground" strokeWidth={1.5} />
        <span className="flex-1 text-left text-[11px] font-medium text-muted-foreground">
          {title} ({count})
        </span>
        <ChevronDown
          className={`size-3 text-muted-foreground transition-transform ${open ? "rotate-0" : "-rotate-90"}`}
          strokeWidth={1.5}
        />
      </button>
      {open && <div className="px-3 py-2 space-y-1 text-2xs font-light">{children}</div>}
    </div>
  );
}

export function FocusDebrief({ signals, tooltip, onClose }: FocusDebriefProps) {
  const messages = signals.filter((s) => s.signalType === "message_deferred");
  const brainActivity = signals.filter(
    (s) => s.signalType === "cross_domain_dot" || s.signalType === "memory_promoted",
  );
  const coaching = signals.filter((s) => s.signalType === "coaching");

  return (
    <div className="glass-panel w-96 animate-[glass-appear_0.2s_ease-out] p-4 space-y-3">
      {/* Header */}
      <div className="flex items-center justify-between">
        <p className="text-sm font-medium text-foreground">{tooltip}</p>
        <button type="button" onClick={onClose} className="text-muted-foreground hover:text-foreground">
          <X className="size-4" />
        </button>
      </div>

      {/* Messages held */}
      <CollapsibleSection title="Messages held" icon={MessageSquare} count={messages.length}>
        {messages.map((m) => (
          <p key={m.entityPair} className="text-foreground">
            {m.headline}
          </p>
        ))}
      </CollapsibleSection>

      {/* Brain activity */}
      <CollapsibleSection title="Brain activity" icon={Brain} count={brainActivity.length}>
        {brainActivity.map((s) => (
          <p key={s.entityPair} className="text-foreground">
            {s.headline}
          </p>
        ))}
      </CollapsibleSection>

      {/* Coaching */}
      <CollapsibleSection title="Coaching" icon={Sparkles} count={coaching.length}>
        {coaching.map((s) => (
          <p key={s.entityPair} className="text-foreground">
            {s.headline}
          </p>
        ))}
      </CollapsibleSection>
    </div>
  );
}
```

- [ ] **Step 2: Wire FocusDebrief into BrainOrb**

Open `desktop-ui/src/shared/components/BrainOrb.tsx`. When `current?.mode === "deferred"`, render the FocusDebrief instead of the standard tooltip:

```tsx
import { FocusDebrief } from "@features/productivity/components/FocusDebrief";

// Inside the tooltip section, add before the existing tooltip logic:
{showTooltip && current?.mode === "deferred" ? (
  <div className="absolute top-full right-0 mt-2 z-50">
    <FocusDebrief
      signals={current.signals}
      tooltip={current.tooltip}
      onClose={() => {
        setShowTooltip(false);
        acknowledge();
      }}
    />
  </div>
) : showTooltip && (isPulsing || badgeCount > 0) ? (
  // ... existing tooltip ...
) : null}
```

- [ ] **Step 3: Add pulse indicator to Brain nav item in Sidebar**

Open `desktop-ui/src/app/layouts/Sidebar.tsx`. Import `useAmbientSignals`:

```tsx
import { useAmbientSignals } from "@shared/hooks/useAmbientSignals";
```

Inside the Sidebar component, get signal state:

```tsx
const { badgeCount, isPulsing } = useAmbientSignals();
const hasBrainSignal = badgeCount > 0 || isPulsing;
```

In the nav item rendering loop, when `item.key === "Brain"`, add a tiny indicator:

```tsx
{item.key === "Brain" && hasBrainSignal && (
  <div className="absolute top-1 right-1 size-1 rounded-full bg-amber-400" />
)}
```

- [ ] **Step 4: Run lint and test**

Run: `cd desktop-ui && bun run lint:fix && bun run test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/
git commit -m "feat(ui): Focus Debrief panel and Brain nav pulse indicator"
```

---

## Week 2: Polish (Tasks 14–19)

### Task 14: JourneyTracker Backend

**Files:**
- Create: `crates/app-core/src/journey.rs`
- Modify: `crates/app-core/src/lib.rs`
- Test: inline

- [ ] **Step 1: Define milestones as a bitfield**

Create `crates/app-core/src/journey.rs`:

```rust
use storage::StoragePool;

/// Milestones in the 7-day guided journey. Stored as a u32 bitfield.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Milestone {
    SetupComplete       = 1 << 0,
    FirstImport         = 1 << 1,
    FirstChatResponse   = 1 << 2,
    OrbAwakening        = 1 << 3,
    FirstFocusDebrief   = 1 << 4,
    FirstDotAccepted    = 1 << 5,
    QuietDay            = 1 << 6,
    FirstBrainReport    = 1 << 7,
    HelloPulse          = 1 << 8,
}

pub struct JourneyTracker {
    pool: StoragePool,
}

impl JourneyTracker {
    pub fn new(pool: StoragePool) -> Self {
        Self { pool }
    }

    pub async fn is_complete(&self, milestone: Milestone) -> bool {
        let bits = self.load_bits().await;
        bits & (milestone as u32) != 0
    }

    pub async fn mark_complete(&self, milestone: Milestone) {
        let bits = self.load_bits().await;
        let new_bits = bits | (milestone as u32);
        self.save_bits(new_bits).await;
    }

    /// Count items across all features (for hello pulse guard: >= 3).
    pub async fn total_item_count(&self) -> i64 {
        let count: (i64,) = sqlx::query_as(
            "SELECT (SELECT COUNT(*) FROM tasks) + (SELECT COUNT(*) FROM notes) + (SELECT COUNT(*) FROM finance_transactions)",
        )
        .fetch_one(self.pool.inner())
        .await
        .unwrap_or((0,));
        count.0
    }

    async fn load_bits(&self) -> u32 {
        let result: Option<(String,)> = sqlx::query_as(
            "SELECT value FROM user_preferences WHERE key = 'journey_milestones'",
        )
        .fetch_optional(self.pool.inner())
        .await
        .unwrap_or(None);

        result
            .and_then(|(v,)| v.parse::<u32>().ok())
            .unwrap_or(0)
    }

    async fn save_bits(&self, bits: u32) {
        let _ = sqlx::query(
            "INSERT OR REPLACE INTO user_preferences (key, value) VALUES ('journey_milestones', ?)",
        )
        .bind(bits.to_string())
        .execute(self.pool.inner())
        .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_milestone_roundtrip() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let tracker = JourneyTracker::new(pool);

        assert!(!tracker.is_complete(Milestone::OrbAwakening).await);
        tracker.mark_complete(Milestone::OrbAwakening).await;
        assert!(tracker.is_complete(Milestone::OrbAwakening).await);
        assert!(!tracker.is_complete(Milestone::FirstFocusDebrief).await);
    }

    #[tokio::test]
    async fn test_multiple_milestones() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let tracker = JourneyTracker::new(pool);

        tracker.mark_complete(Milestone::SetupComplete).await;
        tracker.mark_complete(Milestone::FirstChatResponse).await;

        assert!(tracker.is_complete(Milestone::SetupComplete).await);
        assert!(tracker.is_complete(Milestone::FirstChatResponse).await);
        assert!(!tracker.is_complete(Milestone::OrbAwakening).await);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo nextest run -p app-core -E 'test(milestone)'`
Expected: PASS.

- [ ] **Step 3: Add to AppCore state and register module**

Add `pub mod journey;` to `crates/app-core/src/lib.rs`. Add `pub journey_tracker: Option<journey::JourneyTracker>` to AppCore state. Initialize in `init/mod.rs`.

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/journey.rs crates/app-core/src/lib.rs crates/app-core/src/state.rs crates/app-core/src/init/mod.rs
git commit -m "feat(app-core): JourneyTracker milestone bitfield for 7-day guided journey"
```

---

### Task 15: useJourney Hook and Guided Tooltips

**Files:**
- Create: `desktop-ui/src/shared/hooks/useJourney.ts`
- Modify: `desktop-ui/src/shared/components/BrainOrb.tsx` (add guided tooltip variants)
- Test: `cd desktop-ui && bun run test`

- [ ] **Step 1: Create useJourney hook**

Create `desktop-ui/src/shared/hooks/useJourney.ts`:

```typescript
import { useQuery, useMutation } from "@shared/hooks/useQuery";
import { ipc } from "@shared/lib/ipc";

export type Milestone =
  | "setup_complete"
  | "first_import"
  | "first_chat_response"
  | "orb_awakening"
  | "first_focus_debrief"
  | "first_dot_accepted"
  | "quiet_day"
  | "first_brain_report"
  | "hello_pulse";

export function useJourney() {
  const { data: milestones } = useQuery<string[]>("journey_milestones", {});
  const { mutate: markComplete } = useMutation("journey_mark_complete");

  const isComplete = (milestone: Milestone): boolean => {
    return milestones?.includes(milestone) ?? false;
  };

  return {
    isComplete,
    markComplete: (milestone: Milestone) => markComplete({ milestone }),
    milestones: milestones ?? [],
  };
}
```

- [ ] **Step 2: Add Tauri commands for journey**

Create corresponding Tauri commands in `crates/desktop/src/commands/journey.rs` that delegate to `AppCore.journey_tracker`. Add `journey_milestones` (list completed) and `journey_mark_complete` (mark one done).

- [ ] **Step 3: Add guided tooltip variant to BrainOrb**

In `BrainOrb.tsx`, check `useJourney().isComplete("orb_awakening")`. If false and a pulse arrives, show the Day 3 Awakening tooltip instead of the normal one:

```tsx
const { isComplete, markComplete } = useJourney();

// In tooltip rendering:
{!isComplete("orb_awakening") && isPulsing && current ? (
  <div className="glass-panel w-80 p-4 space-y-3 animate-[glass-appear_0.2s_ease-out]">
    <p className="text-sm text-foreground">
      Hey — I'm your second brain's heartbeat. I only light up when I've connected
      something worth your attention.
    </p>
    <p className="text-xs text-muted-foreground">{current.tooltip}</p>
    <div className="flex gap-2">
      <button
        type="button"
        onClick={() => {
          markComplete("orb_awakening");
          if (current.detailRoute) navigate(current.detailRoute);
        }}
        className="text-xs px-3 py-1.5 rounded-md bg-amber-500 text-white hover:bg-amber-400"
      >
        Show me →
      </button>
      <button
        type="button"
        onClick={() => {
          markComplete("orb_awakening");
          setShowTooltip(false);
        }}
        className="text-xs px-3 py-1.5 rounded-md bg-accent text-foreground hover:bg-accent/80"
      >
        Got it — keep whispering
      </button>
    </div>
  </div>
) : // ... normal tooltip ...
```

- [ ] **Step 4: Run lint and test**

Run: `cd desktop-ui && bun run lint:fix && bun run test`

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/ crates/desktop/src/commands/journey.rs
git commit -m "feat(ui): useJourney hook with Day 3 Orb Awakening guided tooltip"
```

---

### Task 16: LLM Fallback Warm Messages

**Files:**
- Modify: `desktop-ui/src/features/chat/pages/ChatPage.tsx`
- Modify: `crates/app-core/src/events.rs` (add provider:degraded event)

- [ ] **Step 1: Add provider:degraded Tauri event**

In the provider manager or app-core, emit a `provider:degraded` event when the circuit breaker opens:

```rust
// In crates/app-core/src/events.rs or wherever provider state is tracked:
pub const PROVIDER_DEGRADED: &str = "provider:degraded";

#[derive(Serialize)]
pub struct ProviderDegradedPayload {
    pub level: String, // "fallback" or "offline"
}
```

- [ ] **Step 2: Listen in ChatPage and show warm messages**

Open `desktop-ui/src/features/chat/pages/ChatPage.tsx`. Add:

```tsx
import { useEvent } from "@shared/hooks/useEvent";

const [providerStatus, setProviderStatus] = useState<"ok" | "fallback" | "offline">("ok");

useEvent<{ level: string }>("provider:degraded", (payload) => {
  setProviderStatus(payload.level as "fallback" | "offline");
});

// In the chat message area, when providerStatus changes:
{providerStatus === "fallback" && (
  <div className="px-4 py-2 text-xs text-amber-400 bg-amber-400/5 rounded-lg mx-4">
    Claude is taking a moment. I'm working from what I already know about you — give me a sec.
  </div>
)}
{providerStatus === "offline" && (
  <div className="px-4 py-2 text-xs text-muted-foreground bg-accent rounded-lg mx-4">
    All my cloud connections are down right now. I can still search your tasks, notes, and memory locally — just ask.
  </div>
)}
```

- [ ] **Step 3: Wire circuit breaker state to Tauri event**

In the provider manager (where circuit breaker state changes), emit `provider:degraded` via AppEventEmitter when state transitions to open or when all providers fail.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/chat/ crates/app-core/ crates/providers/
git commit -m "feat(ui): warm LLM fallback messages when providers degrade"
```

---

### Task 17: Nightly Cross-Domain LLM Batch

**Files:**
- Create: `crates/feature-insights/src/nightly_batch.rs`
- Modify: `crates/storage/migrations/001_initial.sql` (add `cross_domain_insights` table)
- Modify: cron job registration

- [ ] **Step 1: Add cross_domain_insights table**

In `crates/storage/migrations/001_initial.sql`:

```sql
CREATE TABLE IF NOT EXISTS cross_domain_insights (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    date        TEXT NOT NULL,
    insight_text TEXT NOT NULL,
    dot_refs    TEXT NOT NULL,
    surfaced    INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
```

- [ ] **Step 2: Implement nightly batch function**

Create `crates/feature-insights/src/nightly_batch.rs`:

```rust
/// Nightly cron job: take today's cross-domain dots, generate polished insight
/// sentences via one LLM call, store for tomorrow's morning briefing.
pub async fn run_nightly_cross_domain_batch(
    feedback_repo: &BrainSignalFeedbackRepo,
    insight_repo: &CrossDomainInsightRepo,
    llm_provider: &dyn LlmProvider,
) -> Result<(), common::KlyntbotError> {
    // 1. Fetch today's surfaced cross-domain dots from feedback
    // 2. If empty, return early
    // 3. Build prompt with dot summaries
    // 4. Single LLM call (~500 tokens)
    // 5. Parse response into 1-3 insight sentences
    // 6. Store in cross_domain_insights with surfaced=false
    // 7. If LLM fails, skip silently (template fallback is automatic)
    todo!("Implementation agent fills in using existing cron job patterns")
}
```

- [ ] **Step 3: Register as cron job**

Follow the pattern of `JOB_MIRROR_WEEKLY_NARRATIVE`. Register with schedule "0 2 * * *" (2 AM daily local).

- [ ] **Step 4: Wire BrainVoice to check for unsurfaced insights on app launch**

In BrainVoice initialization or a startup hook, check `cross_domain_insights` for `surfaced=false`. If found, emit as the first pulse of the day.

- [ ] **Step 5: Commit**

```bash
git add crates/feature-insights/ crates/storage/
git commit -m "feat(insights): nightly LLM batch for polished cross-domain insight sentences"
```

---

### Task 18: Weekly Brain Report Pulse

**Files:**
- Modify: `crates/app-core/src/brain_voice.rs`

- [ ] **Step 1: Handle MirrorInsight signal in BrainVoice**

The existing `JOB_MIRROR_WEEKLY_NARRATIVE` cron (Sunday 10 AM UTC) generates the weekly narrative. When it completes, it should publish a `MirrorInsight` domain event (or use an existing event like `MirrorSnippetCreated`).

Add to `extract_signal` in brain_voice.rs:

```rust
DomainEvent::MirrorSnippetCreated { snippet_id, headline } => Some((
    SignalSummary {
        signal_type: "mirror_insight".into(),
        entity_pair: format!("snippet:{snippet_id}"),
        headline: headline.clone(),
    },
    format!("Your weekly brain report is ready — {headline}"),
    Some("/brain".into()),
)),
```

If a dedicated `MirrorNarrativeReady` event doesn't exist, add one to the DomainEvent enum and emit it from the narrative cron job.

- [ ] **Step 2: Test and commit**

```bash
cargo nextest run -p app-core -E 'test(brain_voice)'
git add crates/
git commit -m "feat(brain-voice): weekly Brain Report pulse on Sunday narrative"
```

---

### Task 19: Idle Orb Breathing Animation

**Files:**
- Modify: `desktop-ui/src/shared/components/BrainOrb.tsx`

The `orb-breathe` keyframe was already added in Task 10. This task wires it to the "recent signals exist but currently idle" state.

- [ ] **Step 1: Add breathing state logic**

In `BrainOrb.tsx`, track whether signals have been received recently:

```tsx
const [hasRecentSignals, setHasRecentSignals] = useState(false);

// When any signal arrives, mark as recent
useEffect(() => {
  if (current) {
    setHasRecentSignals(true);
    // Reset after 5 minutes of no new signals
    const timer = setTimeout(() => setHasRecentSignals(false), 5 * 60 * 1000);
    return () => clearTimeout(timer);
  }
}, [current]);
```

Update the dot className to include breathing when idle but has recent signals:

```tsx
// In the base dot className, add the breathing case:
hasRecentSignals && !isPulsing && !isFocusDeferred && badgeCount === 0
  ? "bg-muted animate-[orb-breathe_8s_ease-in-out_infinite]"
  : "bg-muted opacity-30"
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/shared/components/BrainOrb.tsx
git commit -m "feat(ui): idle orb breathing animation when recent signals exist"
```

---

## Week 3: Code Diet (Out of Scope for This Plan)

Code Diet is a separate spec (`docs/superpowers/specs/2026-03-30-code-diet-design.md`) with its own implementation plan. The hard constraint from this plan: **nothing in Code Diet may degrade first-message latency or BrainVoice signal delivery.**

---

## Appendix: File Map

### New files (7)
| File | Crate/Layer | Purpose |
|------|-------------|---------|
| `crates/app-core/src/brain_voice.rs` | app-core (L7) | BrainVoice signal router |
| `crates/app-core/src/journey.rs` | app-core (L7) | JourneyTracker milestone bitfield |
| `crates/feature-insights/src/cross_domain.rs` | feature-insights (L4) | Cross-domain heuristic |
| `crates/feature-insights/src/nightly_batch.rs` | feature-insights (L4) | Nightly LLM batch job |
| `desktop-ui/src/shared/components/BrainOrb.tsx` | Frontend | Orb + animation + tooltips |
| `desktop-ui/src/shared/hooks/useAmbientSignals.ts` | Frontend | Tauri event listener |
| `desktop-ui/src/shared/hooks/useJourney.ts` | Frontend | Milestone state |

### Key modified files
| File | Change |
|------|--------|
| `crates/bus/src/domain_events.rs` | 3 new event variants |
| `crates/cognitive/src/services/memory_promotion.rs` | Event emission on promote |
| `crates/agent/src/agent_loop/mod.rs` | Focus-aware message deferral |
| `crates/feature-insights/src/service.rs` | Wire cross-domain checks |
| `crates/app-core/src/state.rs` | BrainVoice + JourneyTracker fields |
| `crates/app-core/src/init/mod.rs` | Initialize new components |
| `crates/storage/migrations/001_initial.sql` | 2 new tables |
| `desktop-ui/src/app/layouts/AppShell.tsx` | Mount BrainOrb in top bar |
| `desktop-ui/src/app/layouts/Sidebar.tsx` | Brain nav + pulse indicator |
| `desktop-ui/src/app/router.tsx` | `/mirror` → `/brain` |
| `desktop-ui/src/styles/theme.css` | Orb animations |
