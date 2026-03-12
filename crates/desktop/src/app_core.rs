//! Desktop adapter for `app_core` — re-exports `AppCore` and wires
//! `EventChannels` to Tauri events.

pub use ::app_core::AppCore;

use std::sync::Arc;

use ::app_core::EventChannels;
use desktop_shared::events;
use feature_productivity::auto_focus::AutoFocusEvent;
use feature_productivity::dashboard_emitter::{DashboardEmitter, DashboardEvent};
use tauri::{Emitter, Manager};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::warn;

/// Initialize `AppCore` and wire event channels to Tauri emitters.
pub async fn init(app_handle: tauri::AppHandle) -> Result<AppCore, String> {
    let sender = Arc::new(crate::notify::TauriNotificationSender::new(
        app_handle.clone(),
    ));
    let (core, channels) =
        AppCore::init_with_sender(common::AppMode::Desktop, None, Some(sender)).await?;
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
                    dominant_category,
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
                if let Some(tray_window) = handle.get_webview_window("tray") {
                    let _ = tray_window.show();
                    let _ = tray_window.set_focus();
                }
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
                            | bus::DomainEvent::TaskExecutionProgress { .. } => "work",
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
                            | bus::DomainEvent::NoteUpdated { .. } => "notes",
                            bus::DomainEvent::ToolCallExecuted { .. } => "general",
                            bus::DomainEvent::BehavioralPatternDetected { .. } => "learning",
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
