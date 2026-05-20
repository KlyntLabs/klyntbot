//! Desktop adapter for `app_core` — re-exports `AppCore` and wires
//! `EventChannels` to Tauri events.

pub use ::app_core::AppCore;

use std::sync::{Arc, Mutex, OnceLock};

use ::app_core::events::{AppEventEmitter, CompoundEmitter};
use ::app_core::EventChannels;
use desktop_shared::events;
use feature_productivity::dashboard_emitter::{DashboardEmitter, DashboardEvent};
use serde_json::Value;
use tauri::{Emitter, Manager};
use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

/// Process-wide handle to the MCP→desktop event bridge. Held forever so
/// the accept loop runs for the desktop's lifetime; on shutdown the
/// `Drop` impl unlinks the socket.
static BRIDGE_SERVER: OnceLock<mcp_bridge::BridgeServer> = OnceLock::new();

/// Process-wide token for the PRAGMA data_version polling fallback.
/// The watcher task exits when this token is cancelled (on graceful
/// shutdown) or when the runtime drops (on process exit).
static DATA_VERSION_WATCHER: OnceLock<storage::DataVersionWatcherHandle> = OnceLock::new();

/// Latest distraction intervention awaiting display. Populated each time a
/// `DistractionAlert` is forwarded; cleared when the overlay's React layer
/// acks via `distraction_clear_pending_intervention`. This survives the
/// emit-before-mount race for the lazily-created overlay window.
static PENDING_INTERVENTION: OnceLock<Mutex<Option<events::InterventionPayload>>> = OnceLock::new();

fn pending_intervention_slot() -> &'static Mutex<Option<events::InterventionPayload>> {
    PENDING_INTERVENTION.get_or_init(|| Mutex::new(None))
}

pub fn take_pending_intervention() -> Option<events::InterventionPayload> {
    pending_intervention_slot().lock().ok()?.clone()
}

pub fn clear_pending_intervention() {
    if let Ok(mut g) = pending_intervention_slot().lock() {
        *g = None;
    }
}

fn store_pending_intervention(payload: events::InterventionPayload) {
    if let Ok(mut g) = pending_intervention_slot().lock() {
        *g = Some(payload);
    }
}

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
) -> Result<
    (
        AppCore,
        broadcast::Sender<(String, Value)>,
        Arc<crate::approval::DesktopApprovalChannel>,
    ),
    String,
> {
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

    // Build the desktop approval channel up-front so AgentLoopBuilder can wire
    // the gate against it. The same Arc is returned to main.rs and registered
    // as Tauri-managed state — the `approval_respond` command resolves the
    // pending request through this exact instance.
    let approval_channel = Arc::new(crate::approval::DesktopApprovalChannel::new(
        app_handle.clone(),
    ));

    let (core, channels) = AppCore::init_with_sender(
        common::AppMode::Desktop,
        None,
        Some(sender),
        Some(emitter),
        None,
    )
    .await?;
    // Cross-process event bridge — receives frames from a child
    // `klyntbot mcp serve --stdio` process and re-emits them via Tauri's
    // global broadcast so every webview's `tauriEventBridge` (Plan 1) picks
    // them up.
    if let Some(socket_path) = mcp_bridge::bridge_socket_path() {
        let app_handle_for_bridge = app_handle.clone();
        let handler: mcp_bridge::server::FrameHandler = Box::new(move |frame| {
            use tauri::Emitter;
            if let Err(e) = app_handle_for_bridge.emit(&frame.event, frame.payload) {
                tracing::warn!("mcp-bridge: failed to re-emit event {}: {e}", frame.event);
            }
        });
        match mcp_bridge::BridgeServer::start(socket_path.clone(), handler).await {
            Ok(server) => {
                if BRIDGE_SERVER.set(server).is_err() {
                    tracing::warn!("mcp-bridge: BRIDGE_SERVER already initialized");
                }
                tracing::info!("mcp-bridge: listening at {}", socket_path.display());
            }
            Err(e) => {
                tracing::warn!(
                    "mcp-bridge: failed to bind {}: {e}; cross-process events disabled",
                    socket_path.display()
                );
            }
        }
    } else {
        tracing::warn!("mcp-bridge: cannot resolve socket path; bridge disabled");
    }

    // Phase 4: PRAGMA data_version polling fallback. Catches writes that
    // bypassed the bridge (e.g. a CLI mutation, or the MCP child running
    // with the bridge socket unreachable). 5s cadence is conservative —
    // this is a safety net, not a primary signal.
    let dv_token = core.storage_pool.start_data_version_watcher(
        channels.domain_event_bus.clone(),
        std::time::Duration::from_secs(5),
    );
    if DATA_VERSION_WATCHER.set(dv_token).is_err() {
        tracing::warn!("data_version_watcher: already initialized");
    }

    wire_event_channels(&core, channels, &app_handle, &global_event_tx);
    Ok((core, global_event_tx, approval_channel))
}

/// Wire all `EventChannels` receivers to Tauri event emitters.
fn wire_event_channels(
    core: &AppCore,
    channels: EventChannels,
    app_handle: &tauri::AppHandle,
    global_event_tx: &broadcast::Sender<(String, Value)>,
) {
    let shutdown = &core.shutdown_token;

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
                    } => {
                        use tauri_specta::Event;
                        events::ActivitySwitchPayload {
                            from_app,
                            to_app,
                            to_site,
                            category_type,
                        }
                        .emit(&emit_handle)
                    }
                    DashboardEvent::ScoreUpdated {
                        score,
                        productive_secs,
                        distracting_secs,
                    } => {
                        use tauri_specta::Event;
                        events::ScorePayload {
                            score,
                            productive_secs,
                            distracting_secs,
                        }
                        .emit(&emit_handle)
                    }
                    DashboardEvent::FocusStateChanged { state, since } => {
                        use tauri_specta::Event;
                        events::FocusStatePayload { state, since }.emit(&emit_handle)
                    }
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
            use tauri_specta::Event;
            let payload = events::NudgePayload {
                nudge_type: nudge.nudge_type.to_string(),
                message: nudge.message,
            };
            if let Err(e) = payload.emit(handle) {
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

            // Cache before emit: covers cold-start where the overlay's React
            // layer hasn't subscribed yet. Frontend pulls this on mount.
            store_pending_intervention(intervention.clone());

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
                use tauri_specta::Event;
                if let Err(e) = intervention.emit(&overlay) {
                    warn!("failed to emit intervention to overlay: {e}");
                }
            } else {
                warn!("distraction-overlay window not found");
            }

            // Also broadcast for other listeners (e.g. main window banner)
            use tauri_specta::Event;
            if let Err(e) = intervention.emit(handle) {
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
            if let Err(e) = detected.emit(handle) {
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
        let token = shutdown.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = token.cancelled() => break,
                    result = event_rx.recv() => {
                        match result {
                            Ok(event) => {
                                let event_type = event.variant_name().to_string();
                                let payload = desktop_shared::cognitive_commands::DomainEventPayload {
                                    salience: "extract".to_string(),
                                    domain: event.domain().to_string(),
                                    timestamp: jiff::Timestamp::now().to_string(),
                                    payload: serde_json::Value::String(event_type.clone()),
                                    event_type,
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

    // Phase 4: forward DataVersionBumped → data:version_bumped, so Plan 1's
    // tauriEventBridge.ts can invalidate the matching TanStack Query keys in
    // every webview.
    {
        let mut event_rx = channels.domain_event_bus.subscribe();
        let app_handle_clone = app_handle.clone();
        let token = shutdown.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = token.cancelled() => break,
                    result = event_rx.recv() => match result {
                        Ok(bus::DomainEvent::DataVersionBumped { previous, current }) => {
                            use tauri_specta::Event;
                            let payload = desktop_shared::events::DataVersionBumpedPayload {
                                previous,
                                current,
                            };
                            if let Err(e) = payload.emit(&app_handle_clone) {
                                tracing::warn!("phase4: failed to emit data:version_bumped: {e}");
                            }
                        }
                        Ok(_) => {}
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!("phase4 forwarder lagged by {n} events");
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            }
        });
    }

    // Pipeline events → extraction + consolidation
    {
        let app_handle_clone = app_handle.clone();
        let mut pipeline_rx = channels.pipeline_rx;
        let token = shutdown.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = token.cancelled() => break,
                    result = pipeline_rx.recv() => {
                        match result {
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
                }
            }
        });
    }

    // ToolEvent::ApprovalRequest rides on `DomainEvent::Generic { kind:
    // "agent_event" }`. The dual-probe (`ApprovalRequest` key OR `type` field)
    // accommodates both externally- and internally-tagged serde shapes.
    {
        let mut rx = channels.domain_event_bus.subscribe();
        let handle = app_handle.clone();
        let token = shutdown.clone();
        let global_tx = global_event_tx.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = token.cancelled() => break,
                    msg = rx.recv() => match msg {
                        Ok(bus::DomainEvent::Generic { kind, payload })
                            if kind == "agent_event" =>
                        {
                            // ToolEvent uses #[serde(tag="type", rename_all="camelCase")]
                            // so ApprovalRequest variants serialize with "type":"approvalRequest"
                            // (internally tagged; no outer "ApprovalRequest" key). The dual-probe
                            // is kept for defensive forward-compat with externally-tagged variants.
                            let is_approval = payload.get("ApprovalRequest").is_some()
                                || matches!(
                                    payload.get("type").and_then(|v| v.as_str()),
                                    Some("approvalRequest" | "ApprovalRequest")
                                );
                            if is_approval {
                                let inner = payload.get("ApprovalRequest")
                                    .and_then(|v| v.get("payload"))
                                    .cloned()
                                    .or_else(|| payload.get("payload").cloned())
                                    .unwrap_or(payload);
                                if let Err(e) = handle.emit("agent:approval_request", &inner) {
                                    warn!("failed to emit agent:approval_request: {e}");
                                }
                                let _ = global_tx
                                    .send(("agent:approval_request".to_string(), inner));
                            }
                        }
                        Ok(_) => {}
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            warn!("approval forwarder lagged by {n} events");
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
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

/// Forward every event from a `TypedBroker` subscriber onto a Tauri event channel.
#[allow(dead_code)]
fn spawn_broker_forwarder<T>(
    mut rx: tokio::sync::broadcast::Receiver<T>,
    app_handle: &tauri::AppHandle,
    shutdown_token: &CancellationToken,
    event_name: &'static str,
    global_event_tx: Option<broadcast::Sender<(String, Value)>>,
) where
    T: Clone + serde::Serialize + Send + 'static,
{
    let handle = app_handle.clone();
    let token = shutdown_token.clone();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = token.cancelled() => break,
                msg = rx.recv() => match msg {
                    Ok(evt) => {
                        if let Err(e) = handle.emit(event_name, &evt) {
                            warn!("failed to emit {event_name}: {e}");
                        }
                        // Mirror to dev_server SSE so browser-only dev mode also receives.
                        if let Some(ref tx) = global_event_tx {
                            if let Ok(payload) = serde_json::to_value(&evt) {
                                let _ = tx.send((event_name.to_string(), payload));
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        warn!("{event_name} forwarder lagged by {n} events");
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    });
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
