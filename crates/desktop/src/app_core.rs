//! Desktop adapter for `app_core` — re-exports `AppCore` and wires
//! `EventChannels` to Tauri events.

pub use ::app_core::AppCore;

use std::sync::Arc;

use ::app_core::events::{AppEventEmitter, CompoundEmitter};
use ::app_core::EventChannels;
use desktop_shared::events;
use feature_productivity::auto_focus::AutoFocusEvent;
use feature_productivity::dashboard_emitter::{DashboardEmitter, DashboardEvent};
use serde_json::Value;
use tauri::{Emitter, Manager};
use tokio::sync::{broadcast, mpsc};
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
///
/// Returns `(AppCore, broadcast::Sender)` — the broadcast sender carries a copy
/// of every `emit_event` call so the dev HTTP server can relay them via SSE to
/// browsers at localhost:1420 (brain:ambient, provider:degraded, focus:state, etc.).
pub async fn init(
    app_handle: tauri::AppHandle,
) -> Result<(AppCore, broadcast::Sender<(String, Value)>), String> {
    let sender = Arc::new(crate::notify::TauriNotificationSender::new(
        app_handle.clone(),
    ));

    // Create a global broadcast channel that mirrors all emit_event calls.
    // The dev server subscribes to this for its /api/brain/events SSE endpoint.
    let (global_event_tx, _) = broadcast::channel::<(String, Value)>(256);

    let tauri_emitter = TauriEventEmitter {
        app_handle: app_handle.clone(),
    };
    let emitter: Arc<dyn AppEventEmitter> =
        Arc::new(CompoundEmitter::new(tauri_emitter, global_event_tx.clone()));

    let (core, channels) =
        AppCore::init_with_sender(common::AppMode::Desktop, None, Some(sender), Some(emitter))
            .await?;
    wire_event_channels(&core, channels, &app_handle, &global_event_tx);
    Ok((core, global_event_tx))
}

/// Wire all `EventChannels` receivers to Tauri event emitters.
fn wire_event_channels(
    core: &AppCore,
    channels: EventChannels,
    app_handle: &tauri::AppHandle,
    global_event_tx: &broadcast::Sender<(String, Value)>,
) {
    let shutdown = &core.shutdown_token;

    // Auto-focus events → Tauri event + tray timer sync
    if let Some(mut auto_focus_rx) = channels.auto_focus_rx {
        let handle = app_handle.clone();
        let token = shutdown.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = token.cancelled() => break,
                    msg = auto_focus_rx.recv() => {
                        let Some(event) = msg else { break };
                        match event {
                            AutoFocusEvent::Started {
                                started_at,
                                dominant_app,
                                dominant_category,
                            } => {
                                let payload = serde_json::json!({
                                    "startedAt": started_at.to_rfc3339(),
                                    "dominantApp": dominant_app,
                                    "dominantCategory": dominant_category,
                                });
                                if let Err(e) = handle.emit(events::FOCUS_AUTO_STARTED, payload) {
                                    warn!("failed to emit auto-focus started event: {e}");
                                }

                                // Start the tray focus timer so the UI reflects the active session
                                if let Some(timer) = handle.try_state::<Arc<crate::focus_timer::FocusTimer>>() {
                                    // Read focus duration from config, fall back to 45 min
                                    let work_mins = handle
                                        .try_state::<Arc<AppCore>>()
                                        .and_then(|core| {
                                            core.config.try_read().ok().map(|c| {
                                                c.productivity.focus.default_duration_mins
                                            })
                                        })
                                        .unwrap_or(45);
                                    let config = crate::focus_timer::FocusSessionConfig {
                                        work_secs: work_mins * 60,
                                        short_break_secs: 5 * 60,
                                        long_break_secs: 15 * 60,
                                        long_break_after: 4,
                                    };
                                    let title = if dominant_app.is_empty() {
                                        None
                                    } else {
                                        Some(dominant_app)
                                    };
                                    if let Err(e) = timer
                                        .start(handle.clone(), config, None, title, false, true, true)
                                        .await
                                    {
                                        warn!("failed to start focus timer for auto-focus: {e}");
                                    }
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
                                // Stop the tray focus timer
                                if let Some(timer) = handle.try_state::<Arc<crate::focus_timer::FocusTimer>>() {
                                    timer.stop(&handle).await;
                                }

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
            if let Some(overlay) =
                crate::lazy_window::get_or_create_window(handle, "distraction-overlay")
            {
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
                        let salience_str = match salience {
                            cognitive::types::SalienceVerdict::Extract => "extract",
                            cognitive::types::SalienceVerdict::Accumulate => "accumulate",
                            cognitive::types::SalienceVerdict::Discard => "discard",
                        };
                        // Avoid serializing the full DomainEvent on every tick — large
                        // payloads (note content, chat turns) cause GB-scale heap pressure
                        // under mimalloc because the temporary allocations are not returned
                        // to the OS between mi_collect cycles (sawtooth memory pattern).
                        let event_type = event.variant_name().to_string();
                        let payload_value = serde_json::Value::String(event_type.clone());
                        let payload = desktop_shared::cognitive_commands::DomainEventPayload {
                            event_type,
                            salience: salience_str.to_string(),
                            domain: event.domain().to_string(),
                            timestamp: chrono::Utc::now().to_rfc3339(),
                            payload: payload_value,
                        };
                        let _ = app_handle_clone.emit("cognitive:domain_event", &payload);
                        if let Some(fe) =
                            desktop_shared::commands::fabric::FabricGraphEvent::from_domain_event(
                                &event,
                            )
                        {
                            let _ = app_handle_clone.emit("fabric_graph", &fe);
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        warn!("debug event forwarder lagged by {n} events");
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });
    }

    // Lifecycle events → focus timer suspend/resume on sleep/wake
    {
        let mut lifecycle_rx = channels.domain_event_bus.subscribe();
        let handle = app_handle.clone();
        let token = shutdown.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = token.cancelled() => break,
                    msg = lifecycle_rx.recv() => {
                        match msg {
                            Ok(bus::DomainEvent::SystemWillSleep) => {
                                if let Some(timer) = handle.try_state::<Arc<crate::focus_timer::FocusTimer>>() {
                                    timer.suspend().await;
                                }
                            }
                            Ok(bus::DomainEvent::SystemDidWake { .. }) => {
                                if let Some(timer) = handle.try_state::<Arc<crate::focus_timer::FocusTimer>>() {
                                    timer.resume_suspended().await;
                                }
                            }
                            Ok(_) => {}
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                                warn!("lifecycle event forwarder lagged by {n} events");
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                        }
                    }
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

    // Voice events → Tauri "voice:event" + global SSE broadcast
    if let Some(ref voice_service) = core.voice_service {
        if let Some(mut voice_rx) = voice_service.take_event_rx() {
            let handle = app_handle.clone();
            let global_tx = global_event_tx.clone();
            let token = shutdown.clone();
            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        _ = token.cancelled() => break,
                        msg = voice_rx.recv() => {
                            let Some(event) = msg else { break };
                            if let Err(e) = handle.emit(voice_engine::VOICE_EVENT, &event) {
                                warn!("failed to emit voice event: {e}");
                            }
                            // Also broadcast for dev-server SSE (browser dev mode)
                            if let Ok(payload) = serde_json::to_value(&event) {
                                let _ = global_tx.send((voice_engine::VOICE_EVENT.to_string(), payload));
                            }
                        }
                    }
                }
            });
        }
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
