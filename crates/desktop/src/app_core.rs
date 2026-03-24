//! Desktop adapter for `app_core` — re-exports `AppCore` and wires
//! `EventChannels` to Tauri events.

pub use ::app_core::AppCore;

use std::sync::Arc;

use ::app_core::events::AppEventEmitter;
use ::app_core::EventChannels;
use desktop_shared::events;
use feature_productivity::auto_focus::AutoFocusEvent;
use feature_productivity::dashboard_emitter::{DashboardEmitter, DashboardEvent};
use tauri::{Emitter, Manager};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

/// Bridges `AppEventEmitter` to Tauri's native event system.
struct TauriEventEmitter {
    app_handle: tauri::AppHandle,
}

impl AppEventEmitter for TauriEventEmitter {
    fn emit_event(&self, event_name: &str, payload: serde_json::Value) {
        if let Err(e) = self.app_handle.emit(event_name, payload) {
            warn!("TauriEventEmitter: failed to emit {event_name}: {e}");
        }
    }
}

/// Initialize `AppCore` and wire event channels to Tauri emitters.
pub async fn init(app_handle: tauri::AppHandle) -> Result<AppCore, String> {
    let sender = Arc::new(crate::notify::TauriNotificationSender::new(
        app_handle.clone(),
    ));
    let emitter: Arc<dyn AppEventEmitter> = Arc::new(TauriEventEmitter {
        app_handle: app_handle.clone(),
    });
    let (core, channels) =
        AppCore::init_with_sender(common::AppMode::Desktop, None, Some(sender), Some(emitter))
            .await?;
    wire_event_channels(&core, channels, &app_handle);
    Ok(core)
}

/// Wire all `EventChannels` receivers to Tauri event emitters.
fn wire_event_channels(core: &AppCore, channels: EventChannels, app_handle: &tauri::AppHandle) {
    let shutdown = &core.shutdown_token;

    // Auto-focus events → Tauri event
    if let Some(auto_focus_rx) = channels.auto_focus_rx {
        spawn_channel_forwarder(auto_focus_rx, app_handle, shutdown, |handle, event| {
            match event {
                AutoFocusEvent::Started {
                    started_at,
                    dominant_app,
                    dominant_category,
                } => {
                    // Focus session started — emit event and call handler to create DB session
                    let payload = serde_json::json!({
                        "startedAt": started_at.to_rfc3339(),
                        "dominantApp": dominant_app,
                        "dominantCategory": dominant_category,
                    });
                    if let Err(e) = handle.emit(events::FOCUS_AUTO_STARTED, payload) {
                        warn!("failed to emit auto-focus started event: {e}");
                    }
                }
                AutoFocusEvent::Ended {
                    started_at,
                    ended_at,
                    dominant_app,
                    dominant_category: _,
                    productive_ratio,
                    total_secs,
                } => {
                    // Focus session ended — emit event and call handler to end DB session
                    let payload = events::AutoFocusPayload {
                        started_at: started_at.to_rfc3339(),
                        ended_at: ended_at.to_rfc3339(),
                        duration_mins: total_secs / 60,
                        dominant_app,
                        productive_ratio,
                    };
                    if let Err(e) = handle.emit(events::FOCUS_AUTO_DETECTED, payload) {
                        warn!("failed to emit auto-focus ended event: {e}");
                    }
                }
            }
        });
    }

    // Dashboard ticks → Tauri events (activity switch, score, focus state)
    if let Some(tick_rx) = channels.dashboard_tick_rx {
        let emit_handle = app_handle.clone();
        let _dashboard_emitter = DashboardEmitter::start(
            tick_rx,
            Box::new(move |event| {
                let res = match event {
                    DashboardEvent::ActivitySwitch {
                        from_app,
                        to_app,
                        to_site,
                        category_type,
                    } => emit_handle.emit(
                        events::ACTIVITY_SWITCH,
                        events::ActivitySwitchPayload {
                            from_app,
                            to_app,
                            to_site,
                            category_type,
                        },
                    ),
                    DashboardEvent::ScoreUpdated {
                        score,
                        productive_secs,
                        distracting_secs,
                    } => emit_handle.emit(
                        events::SCORE_UPDATED,
                        events::ScorePayload {
                            score,
                            productive_secs,
                            distracting_secs,
                        },
                    ),
                    DashboardEvent::FocusStateChanged { state, since } => emit_handle.emit(
                        events::FOCUS_STATE_CHANGED,
                        events::FocusStatePayload { state, since },
                    ),
                };
                if let Err(e) = res {
                    warn!("DashboardEmitter: failed to emit event: {e}");
                }
            }),
            channels.dashboard_poll_interval_secs,
            core.shutdown_token.clone(),
        );
    }

    // Nudge records → Tauri event
    if let Some(nudge_rx) = channels.nudge_rx {
        spawn_channel_forwarder(nudge_rx, app_handle, shutdown, |handle, nudge| {
            if let Err(e) = handle.emit(
                events::PRODUCTIVITY_NUDGE,
                events::NudgePayload {
                    nudge_type: nudge.nudge_type.to_string(),
                    message: nudge.message,
                },
            ) {
                warn!("failed to emit nudge event: {e}");
            }
        });
    }

    // Distraction alerts → Tauri events (intervention overlay + detected banner)
    if let Some(distraction_rx) = channels.distraction_alert_rx {
        spawn_channel_forwarder(distraction_rx, app_handle, shutdown, |handle, alert| {
            info!(
                app = %alert.app_name,
                title = ?alert.window_title,
                needs_llm = alert.needs_llm,
                "Distraction alert received — showing overlay"
            );

            // Build intervention payload
            let intervention = events::InterventionPayload {
                app_name: alert.app_name.clone(),
                window_title: alert.window_title, // moved — not needed by detected payload
                session_id: alert.session_id.clone(),
                needs_llm: alert.needs_llm,
                heuristic_verdict: if alert.needs_llm {
                    "ambiguous".to_string()
                } else {
                    "confident_distracting".to_string()
                },
            };

            // Show overlay window on the monitor where the cursor is (= where the distracting app is)
            if let Some(overlay) = handle.get_webview_window("distraction-overlay") {
                // Position on the active monitor before showing
                if let Err(e) = center_on_cursor_monitor(&overlay) {
                    debug!("failed to position overlay on cursor monitor: {e}");
                }
                let _ = overlay.show();
                let _ = overlay.set_focus();
                // Emit directly to the overlay window
                if let Err(e) = overlay.emit(events::DISTRACTION_INTERVENTION, &intervention) {
                    warn!("failed to emit intervention to overlay: {e}");
                }
            } else {
                warn!("distraction-overlay window not found");
            }

            // Also broadcast for other listeners (e.g. main window banner)
            if let Err(e) = handle.emit(events::DISTRACTION_INTERVENTION, intervention) {
                warn!("failed to broadcast distraction intervention: {e}");
            }

            // Emit detected event (for DistractionInterventionBanner.tsx)
            let detected = events::DistractionDetectedPayload {
                app_name: alert.app_name,
                session_id: alert.session_id,
                previous_app: alert.previous_app,
                previous_context: alert.previous_context,
                reason: "Distracting app detected during focus session".to_string(),
            };
            if let Err(e) = handle.emit(events::DISTRACTION_DETECTED, detected) {
                warn!("failed to emit distraction detected: {e}");
            }
        });
    }

    // Coaching interventions → Tauri event + tray popup when main window is unfocused
    spawn_channel_forwarder(
        channels.intervention_rx,
        app_handle,
        shutdown,
        |handle, intervention| {
            if let Err(e) =
                handle.emit(desktop_shared::events::COACHING_INTERVENTION, &intervention)
            {
                warn!("failed to emit coaching intervention: {e}");
            }

            // Show tray popup when main window is not focused (Channel 2: tray nudge)
            let main_focused = handle
                .get_webview_window("main")
                .and_then(|w| w.is_focused().ok())
                .unwrap_or(false);
            if !main_focused {
                crate::focus_timer::open_tray_window(handle);
            }
        },
    );

    // Domain events → debug dashboard
    {
        let mut event_rx = channels.domain_event_bus.subscribe();
        let app_handle_clone = app_handle.clone();
        tokio::spawn(async move {
            loop {
                match event_rx.recv().await {
                    Ok(event) => {
                        let salience = cognitive::salience::evaluate_salience(&event);
                        let domain = match &event {
                            bus::DomainEvent::TaskCreated { .. }
                            | bus::DomainEvent::TaskCompleted { .. }
                            | bus::DomainEvent::TaskDeferred { .. }
                            | bus::DomainEvent::GoalProgress { .. }
                            | bus::DomainEvent::TaskDecomposed { .. }
                            | bus::DomainEvent::TaskExecutionStarted { .. }
                            | bus::DomainEvent::TaskExecutionCompleted { .. }
                            | bus::DomainEvent::TaskExecutionFailed { .. }
                            | bus::DomainEvent::TaskBlocked { .. }
                            | bus::DomainEvent::TaskUnblocked { .. }
                            | bus::DomainEvent::DayPlanGenerated { .. }
                            | bus::DomainEvent::ProactiveSuggestionCreated { .. }
                            | bus::DomainEvent::TaskFocusStarted { .. }
                            | bus::DomainEvent::TaskFocusEnded { .. }
                            | bus::DomainEvent::EstimationRecorded { .. }
                            | bus::DomainEvent::TaskExecutionProgress { .. }
                            | bus::DomainEvent::TaskStatusChanged { .. }
                            | bus::DomainEvent::TaskPriorityChanged { .. }
                            | bus::DomainEvent::TaskFieldUpdated { .. } => "work",
                            bus::DomainEvent::ActivitySessionCompleted { .. }
                            | bus::DomainEvent::FocusSessionStarted { .. }
                            | bus::DomainEvent::FocusSessionEnded { .. }
                            | bus::DomainEvent::DistractionDetected { .. }
                            | bus::DomainEvent::ProductivityScoreComputed { .. }
                            | bus::DomainEvent::SessionCreated { .. }
                            | bus::DomainEvent::SessionEnded { .. }
                            | bus::DomainEvent::QualityScored { .. }
                            | bus::DomainEvent::PredictiveAlert { .. }
                            | bus::DomainEvent::NarrativeGenerated { .. }
                            | bus::DomainEvent::RuleEvolved { .. }
                            | bus::DomainEvent::VoiceJournalProcessed { .. } => "energy",
                            bus::DomainEvent::TransactionRecorded { .. }
                            | bus::DomainEvent::BudgetAlert { .. } => "finance",
                            bus::DomainEvent::UserStatedFact { domain, .. } => domain.as_str(),
                            bus::DomainEvent::UserCorrectedAI { .. } => "learning",
                            bus::DomainEvent::CoachingFeedback { .. } => "coaching",
                            bus::DomainEvent::ChatTurnCompleted { .. } => "general",
                            bus::DomainEvent::NoteCreated { .. }
                            | bus::DomainEvent::NoteUpdated { .. }
                            | bus::DomainEvent::NoteContentChanged { .. }
                            | bus::DomainEvent::NoteDeleted { .. } => "notes",
                            bus::DomainEvent::TaskHierarchyChanged { .. } => "work",
                            bus::DomainEvent::ToolCallExecuted { .. } => "general",
                            bus::DomainEvent::BehavioralPatternDetected { .. } => "learning",
                            bus::DomainEvent::ContradictionDetected { .. } => "learning",
                            bus::DomainEvent::AutotunerDecision { .. } => "learning",
                            bus::DomainEvent::KnowledgeAtomCreated { .. }
                            | bus::DomainEvent::KnowledgeAtomAccepted { .. }
                            | bus::DomainEvent::KnowledgeAtomArchived { .. }
                            | bus::DomainEvent::AtomFlashcardReviewed { .. }
                            | bus::DomainEvent::AtomReinforced { .. }
                            | bus::DomainEvent::AtomInteracted { .. }
                            | bus::DomainEvent::RetentionMilestoneReached { .. }
                            | bus::DomainEvent::TranslationCompleted { .. }
                            | bus::DomainEvent::NoteStudied { .. }
                            | bus::DomainEvent::PracticeUnitCompleted { .. }
                            | bus::DomainEvent::PracticeSessionCompleted { .. }
                            | bus::DomainEvent::KnowledgeTransferDetected { .. }
                            | bus::DomainEvent::CoachingLearningDigest { .. }
                            | bus::DomainEvent::FlashcardSessionCompleted { .. } => "learning",
                            bus::DomainEvent::InterventionTriggered { .. } => "productivity",
                            bus::DomainEvent::MemoryPendingConfirmation { .. } => "memory",
                        };
                        let salience_str = match salience {
                            cognitive::types::SalienceVerdict::Extract => "extract",
                            cognitive::types::SalienceVerdict::Accumulate => "accumulate",
                            cognitive::types::SalienceVerdict::Discard => "discard",
                        };
                        let payload = desktop_shared::cognitive_commands::DomainEventPayload {
                            event_type: format!("{:?}", event)
                                .split('{')
                                .next()
                                .unwrap_or("Unknown")
                                .trim()
                                .to_string(),
                            salience: salience_str.to_string(),
                            domain: domain.to_string(),
                            timestamp: chrono::Utc::now().to_rfc3339(),
                            payload: serde_json::to_value(&event).unwrap_or_default(),
                        };
                        let _ = app_handle_clone.emit("cognitive:domain_event", &payload);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        warn!("debug event forwarder lagged by {n} events");
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });
    }

    // Pipeline events → extraction + consolidation
    {
        let app_handle_clone = app_handle.clone();
        let mut pipeline_rx = channels.pipeline_rx;
        tokio::spawn(async move {
            loop {
                match pipeline_rx.recv().await {
                    Ok(pe) => {
                        let event_name = match &pe {
                            cognitive::PipelineEvent::Extraction { .. } => "cognitive:extraction",
                            cognitive::PipelineEvent::Consolidation { .. } => {
                                "cognitive:consolidation"
                            }
                            _ => continue, // BatchStarted, DeadLetterQueued, DeadLetterReprocessed — no desktop handling needed
                        };
                        let _ = app_handle_clone.emit(event_name, &pe);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        warn!("pipeline event forwarder lagged by {n} events");
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });
    }
}

/// Center a window on the monitor where the cursor is currently located.
/// All coordinates are in physical pixels for consistent comparison.
fn center_on_cursor_monitor(
    window: &tauri::WebviewWindow,
) -> Result<(), Box<dyn std::error::Error>> {
    // cursor_position() returns PhysicalPosition<f64>
    let cursor = window.cursor_position()?;
    let monitors = window.available_monitors()?;

    // Find the monitor containing the cursor (all values in physical pixels)
    let target = monitors.iter().find(|m| {
        let pos = m.position(); // PhysicalPosition<i32>
        let size = m.size(); // PhysicalSize<u32>
        let (mx, my) = (pos.x as f64, pos.y as f64);
        let (mw, mh) = (size.width as f64, size.height as f64);
        cursor.x >= mx && cursor.x < mx + mw && cursor.y >= my && cursor.y < my + mh
    });

    if let Some(monitor) = target {
        let mon_pos = monitor.position();
        let mon_size = monitor.size();
        let win_size = window.outer_size()?; // PhysicalSize

        // All math in physical pixels
        let mon_w = mon_size.width as f64;
        let mon_h = mon_size.height as f64;
        let win_w = win_size.width as f64;
        let win_h = win_size.height as f64;

        // Center horizontally, position in upper third vertically
        let x = mon_pos.x as f64 + (mon_w - win_w) / 2.0;
        let y = mon_pos.y as f64 + (mon_h - win_h) / 3.0;

        window.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
            x as i32, y as i32,
        )))?;
    }

    Ok(())
}

/// Spawn a background task that receives from a channel and emits Tauri events.
fn spawn_channel_forwarder<T: Send + 'static>(
    mut rx: mpsc::Receiver<T>,
    app_handle: &tauri::AppHandle,
    shutdown_token: &CancellationToken,
    emit_fn: impl Fn(&tauri::AppHandle, T) + Send + 'static,
) {
    let handle = app_handle.clone();
    let token = shutdown_token.clone();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = token.cancelled() => break,
                msg = rx.recv() => {
                    let Some(msg) = msg else { break };
                    emit_fn(&handle, msg);
                }
            }
        }
    });
}
