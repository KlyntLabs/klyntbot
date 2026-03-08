//! Desktop adapter for `app_core` — re-exports `AppCore` and wires
//! `EventChannels` to Tauri events.

pub use ::app_core::AppCore;

use ::app_core::EventChannels;
use desktop_shared::events;
use feature_productivity::dashboard_emitter::{DashboardEmitter, DashboardEvent};
use tauri::Emitter;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::warn;

/// Initialize `AppCore` and wire event channels to Tauri emitters.
pub async fn init(app_handle: tauri::AppHandle) -> Result<AppCore, String> {
    let (core, channels) = AppCore::init(None).await?;
    wire_event_channels(&core, channels, &app_handle);
    Ok(core)
}

/// Wire all `EventChannels` receivers to Tauri event emitters.
fn wire_event_channels(core: &AppCore, channels: EventChannels, app_handle: &tauri::AppHandle) {
    let shutdown = &core.shutdown_token;

    // Auto-focus sessions → Tauri event
    if let Some(auto_focus_rx) = channels.auto_focus_rx {
        spawn_channel_forwarder(auto_focus_rx, app_handle, shutdown, |handle, session| {
            let payload = events::AutoFocusPayload {
                started_at: session.started_at.to_rfc3339(),
                ended_at: session.ended_at.to_rfc3339(),
                duration_mins: session.total_secs / 60,
                dominant_app: session.dominant_app,
                productive_ratio: session.productive_ratio,
            };
            if let Err(e) = handle.emit(events::FOCUS_AUTO_DETECTED, payload) {
                warn!("failed to emit auto-focus event: {e}");
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

    // Coaching interventions → Tauri event
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
                            | bus::DomainEvent::GoalProgress { .. } => "work",
                            bus::DomainEvent::ActivitySessionCompleted { .. }
                            | bus::DomainEvent::FocusSessionEnded { .. }
                            | bus::DomainEvent::DistractionDetected { .. }
                            | bus::DomainEvent::ProductivityScoreComputed { .. } => "energy",
                            bus::DomainEvent::TransactionRecorded { .. }
                            | bus::DomainEvent::BudgetAlert { .. } => "finance",
                            bus::DomainEvent::UserStatedFact { domain, .. } => domain.as_str(),
                            bus::DomainEvent::UserCorrectedAI { .. } => "learning",
                            bus::DomainEvent::CoachingFeedback { .. } => "coaching",
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
            while let Some(pe) = pipeline_rx.recv().await {
                let event_name = match &pe {
                    cognitive::PipelineEvent::Extraction { .. } => "cognitive:extraction",
                    cognitive::PipelineEvent::Consolidation { .. } => "cognitive:consolidation",
                };
                let _ = app_handle_clone.emit(event_name, &pe);
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
