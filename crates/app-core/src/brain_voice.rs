//! BrainVoice signal router — decides Pulse/Badge/Deferred/Merged for all brain signals.
//!
//! Subscribes to [`DomainEventBus`], deduplicates via [`BrainSignalFeedbackRepo`],
//! defers signals during focus sessions, applies adaptive dampening based on
//! dismissal history, and emits `brain:ambient` Tauri events via [`AppEventEmitter`].

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use bus::DomainEvent;
use storage::repos::BrainSignalFeedbackRepo;

use crate::events::AppEventEmitter;

// ── Public types ────────────────────────────────────────────────────────

pub const BRAIN_AMBIENT_EVENT: &str = "brain:ambient";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SignalMode {
    Pulse,
    Badge,
    Deferred,
    Merged,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalSummary {
    pub signal_type: String,
    pub entity_pair: String,
    pub headline: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrainAmbientEvent {
    pub mode: SignalMode,
    pub signals: Vec<SignalSummary>,
    pub tooltip: String,
    pub detail_route: Option<String>,
}

// ── Configuration ───────────────────────────────────────────────────────

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
            hold_duration: Duration::from_secs(1),
            dampened_max_pulses: 1,
            dampened_merge_window: Duration::from_secs(60),
            dampening_threshold: 2,
            dampening_window_hours: 48,
        }
    }
}

// ── BrainVoice handle ───────────────────────────────────────────────────

/// Handle to the BrainVoice background task. Drop or call [`shutdown`] to stop.
pub struct BrainVoice {
    cancel: CancellationToken,
    _handle: tokio::task::JoinHandle<()>,
}

impl BrainVoice {
    /// Spawn the brain voice run loop.
    pub fn start(
        rx: broadcast::Receiver<DomainEvent>,
        feedback_repo: BrainSignalFeedbackRepo,
        emitter: Arc<dyn AppEventEmitter>,
        config: BrainVoiceConfig,
        journey_tracker: Option<crate::journey::JourneyTracker>,
    ) -> Self {
        let cancel = CancellationToken::new();
        let token = cancel.clone();
        let handle = tokio::spawn(async move {
            run_brain_voice(rx, feedback_repo, emitter, config, token, journey_tracker).await;
        });
        Self {
            cancel,
            _handle: handle,
        }
    }

    /// Cancel the background task.
    pub fn shutdown(&self) {
        self.cancel.cancel();
    }
}

// ── Signal extraction ───────────────────────────────────────────────────

/// Extract a `(SignalSummary, tooltip, detail_route)` from a relevant domain event.
/// Returns `None` for events the brain voice does not care about.
pub fn extract_signal(event: &DomainEvent) -> Option<(SignalSummary, String, Option<String>)> {
    match event {
        DomainEvent::CrossDomainDotReady {
            source_kind,
            source_id,
            source_title,
            target_kind,
            target_id,
            target_title,
            tooltip,
            detail_route,
            ..
        } => {
            let entity_pair = format!("{source_kind}:{source_id}→{target_kind}:{target_id}");
            let headline = format!("{source_title} ↔ {target_title}");
            Some((
                SignalSummary {
                    signal_type: "cross_domain_dot".into(),
                    entity_pair,
                    headline,
                },
                tooltip.clone(),
                detail_route.clone(),
            ))
        }

        _ => None,
    }
}

// ── Pending signal wrapper ──────────────────────────────────────────────

struct PendingSignal {
    summary: SignalSummary,
    tooltip: String,
    detail_route: Option<String>,
    _received_at: Instant,
}

/// Mutable state passed to `handle_signal` to avoid too-many-arguments.
struct SignalState<'a> {
    pending_signals: &'a mut Vec<PendingSignal>,
    deferred_signals: &'a mut Vec<PendingSignal>,
    deferred_msg_count: &'a mut u32,
    hold_deadline: &'a mut Option<Instant>,
}

// ── Run loop ────────────────────────────────────────────────────────────

async fn run_brain_voice(
    mut rx: broadcast::Receiver<DomainEvent>,
    feedback_repo: BrainSignalFeedbackRepo,
    emitter: Arc<dyn AppEventEmitter>,
    config: BrainVoiceConfig,
    shutdown: CancellationToken,
    journey_tracker: Option<crate::journey::JourneyTracker>,
) {
    let focus_active = AtomicBool::new(false);
    let mut pending_signals: Vec<PendingSignal> = Vec::new();
    let mut deferred_signals: Vec<PendingSignal> = Vec::new();
    let mut deferred_msg_count: u32 = 0;
    let mut focus_start: Option<Instant> = None;
    let mut pulses_this_hour: u32 = 0;
    let mut hour_start = Instant::now();
    let mut hold_deadline: Option<Instant> = None;

    info!("BrainVoice: run loop started");

    loop {
        // Determine timeout: either hold_duration if we have pending signals, or a long wait.
        let timeout_dur = if let Some(deadline) = hold_deadline {
            deadline.saturating_duration_since(Instant::now())
        } else {
            Duration::from_secs(3600)
        };

        tokio::select! {
            _ = shutdown.cancelled() => {
                info!("BrainVoice: shutdown received");
                if !pending_signals.is_empty() {
                    flush_pending(
                        &mut pending_signals,
                        &feedback_repo,
                        &emitter,
                        &config,
                        &mut pulses_this_hour,
                        &mut hour_start,
                    ).await;
                }
                break;
            }

            _ = tokio::time::sleep(timeout_dur) => {
                // Hold timer expired — flush pending signals.
                if !pending_signals.is_empty() {
                    flush_pending(
                        &mut pending_signals,
                        &feedback_repo,
                        &emitter,
                        &config,
                        &mut pulses_this_hour,
                        &mut hour_start,
                    ).await;
                    hold_deadline = None;
                }
            }

            result = rx.recv() => {
                match result {
                    Ok(DomainEvent::FocusSessionStarted { .. }) => {
                        focus_active.store(true, Ordering::Relaxed);
                        focus_start = Some(Instant::now());
                        deferred_msg_count = 0;
                        debug!("BrainVoice: focus session started, deferring signals");
                    }
                    Ok(DomainEvent::FocusSessionEnded { .. }) => {
                        focus_active.store(false, Ordering::Relaxed);
                        let focus_duration = focus_start
                            .map(|s| s.elapsed())
                            .unwrap_or(Duration::ZERO);
                        if !deferred_signals.is_empty() || deferred_msg_count > 0 {
                            flush_deferred(
                                &mut deferred_signals,
                                deferred_msg_count,
                                focus_duration,
                                &feedback_repo,
                                &emitter,
                            ).await;
                        }
                        deferred_msg_count = 0;
                        focus_start = None;

                        // Wire: FirstFocusDebrief journey milestone
                        if let Some(ref tracker) = journey_tracker {
                            if !tracker
                                .is_complete(crate::journey::Milestone::FirstFocusDebrief)
                                .await
                            {
                                tracker
                                    .mark_complete(
                                        crate::journey::Milestone::FirstFocusDebrief,
                                    )
                                    .await;
                            }
                        }

                        debug!("BrainVoice: focus session ended, flushed deferred signals");
                    }
                    Ok(ref event) => {
                        if let Some((summary, tooltip, detail_route)) = extract_signal(event) {
                            let mut state = SignalState {
                                pending_signals: &mut pending_signals,
                                deferred_signals: &mut deferred_signals,
                                deferred_msg_count: &mut deferred_msg_count,
                                hold_deadline: &mut hold_deadline,
                            };
                            handle_signal(
                                summary,
                                tooltip,
                                detail_route,
                                focus_active.load(Ordering::Relaxed),
                                &feedback_repo,
                                &config,
                                &mut state,
                            ).await;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!("BrainVoice: lagged, skipped {n} events");
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        info!("BrainVoice: event channel closed");
                        break;
                    }
                }
            }
        }
    }

    info!("BrainVoice: run loop exited");
}

// ── Signal handling ─────────────────────────────────────────────────────

async fn handle_signal(
    summary: SignalSummary,
    tooltip: String,
    detail_route: Option<String>,
    is_focus_active: bool,
    feedback_repo: &BrainSignalFeedbackRepo,
    config: &BrainVoiceConfig,
    state: &mut SignalState<'_>,
) {
    // Dedup: skip if this entity pair was surfaced within the last 24 hours.
    match feedback_repo
        .was_surfaced_recently(&summary.entity_pair, 24)
        .await
    {
        Ok(true) => {
            debug!(
                "BrainVoice: skipping recently surfaced signal: {}",
                summary.entity_pair
            );
            return;
        }
        Ok(false) => {}
        Err(e) => {
            warn!("BrainVoice: dedup check failed: {e}");
            // Proceed anyway — better to show a duplicate than miss a signal.
        }
    }

    let pending = PendingSignal {
        summary,
        tooltip,
        detail_route,
        _received_at: Instant::now(),
    };

    if is_focus_active {
        // Track message deferrals separately for the debrief.
        if pending.summary.signal_type == "message_deferred" {
            *state.deferred_msg_count += 1;
        }
        state.deferred_signals.push(pending);
    } else {
        state.pending_signals.push(pending);
        // Set hold deadline if not already set — wait for fast followers.
        if state.hold_deadline.is_none() {
            *state.hold_deadline = Some(Instant::now() + config.hold_duration);
        }
    }
}

// ── Flush pending signals ───────────────────────────────────────────────

async fn flush_pending(
    pending: &mut Vec<PendingSignal>,
    feedback_repo: &BrainSignalFeedbackRepo,
    emitter: &Arc<dyn AppEventEmitter>,
    config: &BrainVoiceConfig,
    pulses_this_hour: &mut u32,
    hour_start: &mut Instant,
) {
    if pending.is_empty() {
        return;
    }

    // Reset pulse counter if the hour has elapsed.
    if hour_start.elapsed() > Duration::from_secs(3600) {
        *pulses_this_hour = 0;
        *hour_start = Instant::now();
    }

    // Adaptive dampening: check if user has been dismissing signals.
    let is_dampened = match feedback_repo
        .dismissal_count_since(config.dampening_window_hours)
        .await
    {
        Ok(count) => count >= config.dampening_threshold,
        Err(e) => {
            warn!("BrainVoice: dampening check failed: {e}");
            false
        }
    };

    let max_pulses = if is_dampened {
        config.dampened_max_pulses
    } else {
        config.max_pulses_per_hour
    };

    let signals: Vec<PendingSignal> = std::mem::take(pending);

    // Determine mode.
    let mode = if *pulses_this_hour >= max_pulses {
        SignalMode::Badge
    } else if signals.len() > 1 {
        SignalMode::Merged
    } else {
        SignalMode::Pulse
    };

    // Only count Pulse and Merged towards the per-hour cap.
    if mode == SignalMode::Pulse || mode == SignalMode::Merged {
        *pulses_this_hour += 1;
    }

    // Build the tooltip.
    let tooltip = if signals.len() == 1 {
        signals[0].tooltip.clone()
    } else {
        format!(
            "{} signals: {}",
            signals.len(),
            signals
                .iter()
                .map(|s| s.summary.headline.as_str())
                .collect::<Vec<_>>()
                .join("; ")
        )
    };

    let detail_route = if signals.len() == 1 {
        signals[0].detail_route.clone()
    } else {
        None
    };

    // Record all as surfaced.
    for sig in &signals {
        if let Err(e) = feedback_repo
            .record(
                &sig.summary.signal_type,
                &sig.summary.entity_pair,
                "surfaced",
            )
            .await
        {
            warn!("BrainVoice: failed to record surfaced signal: {e}");
        }
    }

    let event = BrainAmbientEvent {
        mode,
        signals: signals.into_iter().map(|s| s.summary).collect(),
        tooltip,
        detail_route,
    };

    if let Ok(payload) = serde_json::to_value(&event) {
        emitter.emit_event(BRAIN_AMBIENT_EVENT, payload);
        debug!("BrainVoice: emitted {:?} event", event.mode);
    }
}

// ── Flush deferred signals ──────────────────────────────────────────────

async fn flush_deferred(
    deferred: &mut Vec<PendingSignal>,
    msg_count: u32,
    focus_duration: Duration,
    feedback_repo: &BrainSignalFeedbackRepo,
    emitter: &Arc<dyn AppEventEmitter>,
) {
    let brain_count = deferred
        .iter()
        .filter(|s| s.summary.signal_type != "message_deferred")
        .count();

    let duration_mins = focus_duration.as_secs() / 60;
    let duration_display = if duration_mins >= 60 {
        format!("{}h {}m", duration_mins / 60, duration_mins % 60)
    } else {
        format!("{duration_mins}m")
    };

    let tooltip = build_debrief_tooltip(msg_count, brain_count, &duration_display);

    let signals: Vec<PendingSignal> = std::mem::take(deferred);

    // Record all as surfaced.
    for sig in &signals {
        if let Err(e) = feedback_repo
            .record(
                &sig.summary.signal_type,
                &sig.summary.entity_pair,
                "surfaced",
            )
            .await
        {
            warn!("BrainVoice: failed to record deferred signal: {e}");
        }
    }

    let event = BrainAmbientEvent {
        mode: SignalMode::Deferred,
        signals: signals.into_iter().map(|s| s.summary).collect(),
        tooltip,
        detail_route: None,
    };

    if let Ok(payload) = serde_json::to_value(&event) {
        emitter.emit_event(BRAIN_AMBIENT_EVENT, payload);
        debug!("BrainVoice: emitted Deferred debrief event");
    }
}

/// Build the debrief tooltip shown after a focus session ends.
pub fn build_debrief_tooltip(msg_count: u32, brain_count: usize, duration: &str) -> String {
    format!(
        "While you were focused for {duration}, I held {msg_count} message(s) and noticed {brain_count} connection(s) \u{2014} want to catch up?"
    )
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::NoopEmitter;
    use storage::StoragePool;

    #[test]
    fn test_extract_cross_domain_dot() {
        let event = DomainEvent::CrossDomainDotReady {
            source_kind: "note".into(),
            source_id: "n1".into(),
            source_title: "Morning Ritual".into(),
            target_kind: "task".into(),
            target_id: "t1".into(),
            target_title: "Plan day".into(),
            confidence: 0.85,
            tooltip: "These are related".into(),
            detail_route: Some("/fabric/n1-t1".into()),
        };
        let result = extract_signal(&event);
        assert!(result.is_some());
        let (summary, tooltip, route) = result.unwrap();
        assert_eq!(summary.signal_type, "cross_domain_dot");
        assert_eq!(summary.entity_pair, "note:n1\u{2192}task:t1");
        assert!(summary.headline.contains("Morning Ritual"));
        assert!(summary.headline.contains("Plan day"));
        assert_eq!(tooltip, "These are related");
        assert_eq!(route, Some("/fabric/n1-t1".into()));
    }

    #[test]
    fn test_extract_irrelevant_event_returns_none() {
        let event = DomainEvent::TaskCreated {
            task_id: "t1".into(),
            project: None,
            estimate_mins: None,
            task_type: "standard".into(),
        };
        assert!(extract_signal(&event).is_none());

        let event2 = DomainEvent::NoteCreated {
            note_id: "n1".into(),
            title: "Hello".into(),
        };
        assert!(extract_signal(&event2).is_none());

        let event3 = DomainEvent::FocusSessionStarted {
            session_type: "deep".into(),
            target_mins: 25,
        };
        assert!(extract_signal(&event3).is_none());
    }

    #[tokio::test]
    async fn test_dedup_skips_recently_surfaced() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = BrainSignalFeedbackRepo::new(pool.inner().clone());

        // Record that this entity pair was recently surfaced.
        repo.record("memory_promoted", "memory:fact-1", "surfaced")
            .await
            .unwrap();

        let summary = SignalSummary {
            signal_type: "memory_promoted".into(),
            entity_pair: "memory:fact-1".into(),
            headline: "Test".into(),
        };

        let config = BrainVoiceConfig::default();

        let mut pending = Vec::new();
        let mut deferred = Vec::new();
        let mut deferred_msg_count = 0u32;
        let mut hold_deadline: Option<Instant> = None;

        let mut state = SignalState {
            pending_signals: &mut pending,
            deferred_signals: &mut deferred,
            deferred_msg_count: &mut deferred_msg_count,
            hold_deadline: &mut hold_deadline,
        };

        handle_signal(
            summary,
            "tooltip".into(),
            None,
            false,
            &repo,
            &config,
            &mut state,
        )
        .await;

        // Signal should NOT appear in pending because it was recently surfaced.
        assert!(
            state.pending_signals.is_empty(),
            "recently surfaced signal should be skipped"
        );

        // Verify a different entity pair DOES get through.
        let summary2 = SignalSummary {
            signal_type: "memory_promoted".into(),
            entity_pair: "memory:fact-2".into(),
            headline: "New fact".into(),
        };

        handle_signal(
            summary2,
            "tooltip2".into(),
            None,
            false,
            &repo,
            &config,
            &mut state,
        )
        .await;

        assert_eq!(
            state.pending_signals.len(),
            1,
            "new entity pair should be added"
        );
    }

    #[tokio::test]
    async fn test_focus_defers_signals() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = BrainSignalFeedbackRepo::new(pool.inner().clone());
        let config = BrainVoiceConfig::default();

        let mut pending = Vec::new();
        let mut deferred = Vec::new();
        let mut deferred_msg_count = 0u32;
        let mut hold_deadline: Option<Instant> = None;

        let summary = SignalSummary {
            signal_type: "memory_promoted".into(),
            entity_pair: "memory:fact-3".into(),
            headline: "Focus test".into(),
        };

        let mut state = SignalState {
            pending_signals: &mut pending,
            deferred_signals: &mut deferred,
            deferred_msg_count: &mut deferred_msg_count,
            hold_deadline: &mut hold_deadline,
        };

        // Simulate focus active.
        handle_signal(
            summary,
            "tooltip".into(),
            None,
            true, // focus_active
            &repo,
            &config,
            &mut state,
        )
        .await;

        assert!(
            state.pending_signals.is_empty(),
            "during focus, signals should not go to pending"
        );
        assert_eq!(
            state.deferred_signals.len(),
            1,
            "during focus, signals should go to deferred"
        );

        // Message deferred signals should also increment the msg counter.
        let msg_summary = SignalSummary {
            signal_type: "message_deferred".into(),
            entity_pair: "msg:telegram:alice".into(),
            headline: "alice: Hey!".into(),
        };

        handle_signal(
            msg_summary,
            "tooltip".into(),
            None,
            true,
            &repo,
            &config,
            &mut state,
        )
        .await;

        assert_eq!(state.deferred_signals.len(), 2);
        assert_eq!(
            *state.deferred_msg_count, 1,
            "message deferrals should be counted"
        );
    }

    #[test]
    fn test_flush_deferred_builds_correct_tooltip() {
        let tooltip = build_debrief_tooltip(3, 2, "25m");
        assert_eq!(
            tooltip,
            "While you were focused for 25m, I held 3 message(s) and noticed 2 connection(s) \u{2014} want to catch up?"
        );

        let tooltip2 = build_debrief_tooltip(0, 1, "1h 15m");
        assert_eq!(
            tooltip2,
            "While you were focused for 1h 15m, I held 0 message(s) and noticed 1 connection(s) \u{2014} want to catch up?"
        );
    }

    #[tokio::test]
    async fn test_flush_pending_emits_pulse_for_single_signal() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = BrainSignalFeedbackRepo::new(pool.inner().clone());
        let emitter: Arc<dyn AppEventEmitter> = Arc::new(NoopEmitter);
        let config = BrainVoiceConfig::default();

        let mut pending = vec![PendingSignal {
            summary: SignalSummary {
                signal_type: "memory_promoted".into(),
                entity_pair: "memory:fact-10".into(),
                headline: "Single pulse".into(),
            },
            tooltip: "A single signal".into(),
            detail_route: None,
            _received_at: Instant::now(),
        }];

        let mut pulses = 0u32;
        let mut hour_start = Instant::now();

        flush_pending(
            &mut pending,
            &repo,
            &emitter,
            &config,
            &mut pulses,
            &mut hour_start,
        )
        .await;

        assert!(pending.is_empty(), "pending should be drained after flush");
        assert_eq!(pulses, 1, "pulse counter should increment");

        // Verify the signal was recorded as surfaced.
        let surfaced = repo
            .was_surfaced_recently("memory:fact-10", 24)
            .await
            .unwrap();
        assert!(surfaced, "signal should be marked as surfaced after flush");
    }

    #[tokio::test]
    async fn test_flush_pending_badge_when_cap_reached() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = BrainSignalFeedbackRepo::new(pool.inner().clone());
        let recording = Arc::new(RecordingEmitter::new());
        let emitter: Arc<dyn AppEventEmitter> = recording.clone();
        let config = BrainVoiceConfig::default();

        // Simulate having already used all pulses this hour.
        let mut pulses = config.max_pulses_per_hour;
        let mut hour_start = Instant::now();

        let mut pending = vec![PendingSignal {
            summary: SignalSummary {
                signal_type: "memory_promoted".into(),
                entity_pair: "memory:fact-badge".into(),
                headline: "Badge test".into(),
            },
            tooltip: "Should be badge".into(),
            detail_route: None,
            _received_at: Instant::now(),
        }];

        flush_pending(
            &mut pending,
            &repo,
            &emitter,
            &config,
            &mut pulses,
            &mut hour_start,
        )
        .await;

        // Pulse counter should NOT increment for Badge mode.
        assert_eq!(
            pulses, config.max_pulses_per_hour,
            "badge should not increment pulse counter"
        );

        // Check the emitted event was Badge mode.
        let events = recording.events.lock().unwrap();
        assert_eq!(events.len(), 1);
        let (name, payload) = &events[0];
        assert_eq!(name, BRAIN_AMBIENT_EVENT);
        let ambient: BrainAmbientEvent = serde_json::from_value(payload.clone()).unwrap();
        assert_eq!(ambient.mode, SignalMode::Badge);
    }

    #[tokio::test]
    async fn test_flush_pending_merged_for_multiple_signals() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = BrainSignalFeedbackRepo::new(pool.inner().clone());
        let recording = Arc::new(RecordingEmitter::new());
        let emitter: Arc<dyn AppEventEmitter> = recording.clone();
        let config = BrainVoiceConfig::default();

        let mut pulses = 0u32;
        let mut hour_start = Instant::now();

        let mut pending = vec![
            PendingSignal {
                summary: SignalSummary {
                    signal_type: "memory_promoted".into(),
                    entity_pair: "memory:m1".into(),
                    headline: "First".into(),
                },
                tooltip: "first".into(),
                detail_route: None,
                _received_at: Instant::now(),
            },
            PendingSignal {
                summary: SignalSummary {
                    signal_type: "cross_domain_dot".into(),
                    entity_pair: "note:n1\u{2192}task:t1".into(),
                    headline: "Second".into(),
                },
                tooltip: "second".into(),
                detail_route: Some("/fabric/link".into()),
                _received_at: Instant::now(),
            },
        ];

        flush_pending(
            &mut pending,
            &repo,
            &emitter,
            &config,
            &mut pulses,
            &mut hour_start,
        )
        .await;

        let events = recording.events.lock().unwrap();
        assert_eq!(events.len(), 1);
        let ambient: BrainAmbientEvent = serde_json::from_value(events[0].1.clone()).unwrap();
        assert_eq!(ambient.mode, SignalMode::Merged);
        assert_eq!(ambient.signals.len(), 2);
        assert!(ambient.tooltip.contains("2 signals"));
    }

    // ── Test helpers ────────────────────────────────────────────────────

    /// A recording emitter that captures events for test assertions.
    struct RecordingEmitter {
        events: std::sync::Mutex<Vec<(String, serde_json::Value)>>,
    }

    impl RecordingEmitter {
        fn new() -> Self {
            Self {
                events: std::sync::Mutex::new(Vec::new()),
            }
        }
    }

    impl AppEventEmitter for RecordingEmitter {
        fn emit_event(&self, event_name: &str, payload: serde_json::Value) {
            self.events
                .lock()
                .unwrap()
                .push((event_name.to_string(), payload));
        }
    }
}
