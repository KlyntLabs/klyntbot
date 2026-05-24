#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

// FFI bindings for mimalloc tuning. The symbols are always present when
// mimalloc is the global allocator — no extra crate needed.
unsafe extern "C" {
    /// Force mimalloc to return freed pages to the OS.
    /// `force = true` aggressively collects across all thread-local heaps.
    fn mi_collect(force: bool);
    /// Set a mimalloc runtime option. Returns true if successful.
    fn mi_option_set(option: i32, value: i64) -> bool;
}

/// Configure mimalloc for minimal memory retention.
fn configure_mimalloc() {
    const MI_OPTION_PURGE_DELAY: i32 = 10;
    const MI_OPTION_ARENA_PURGE_MULT: i32 = 24;
    const MI_OPTION_ABANDONED_PAGE_PURGE: i32 = 25;
    const MI_OPTION_ALLOW_LARGE_OS_PAGES: i32 = 6;
    const MI_OPTION_EAGER_COMMIT: i32 = 1;
    const MI_OPTION_ARENA_EAGER_COMMIT: i32 = 14;
    unsafe {
        // Purge freed pages immediately instead of retaining them (default: 10ms).
        mi_option_set(MI_OPTION_PURGE_DELAY, 0);
        // Purge arena segments more aggressively (default: 10).
        mi_option_set(MI_OPTION_ARENA_PURGE_MULT, 1);
        // Purge abandoned pages from terminated threads immediately.
        mi_option_set(MI_OPTION_ABANDONED_PAGE_PURGE, 1);
        // Disable large OS pages — they can't be partially returned to the OS.
        mi_option_set(MI_OPTION_ALLOW_LARGE_OS_PAGES, 0);
        // Don't eagerly commit memory — only commit pages when first accessed.
        mi_option_set(MI_OPTION_EAGER_COMMIT, 0);
        mi_option_set(MI_OPTION_ARENA_EAGER_COMMIT, 0);
    }
}

#[cfg(debug_assertions)]
use desktop::dev_server;
use desktop::{
    app_core, claude_code_integration, commands, focus_timer, lazy_window, shortcuts,
    specta_builder, tray_countdown,
};

use std::sync::atomic::Ordering;
use std::sync::Arc;

use clap::{Parser, Subcommand};
use commands::window::{QUIT_REQUESTED, WINDOW_TRAY};
use tauri::image::Image;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::Manager;
use tauri_plugin_global_shortcut::GlobalShortcutExt;

#[derive(Parser)]
#[command(name = "Klynt", about = "Klynt personal AI agent")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the MCP server
    Mcp {
        #[command(subcommand)]
        command: McpCommands,
    },
}

#[derive(Subcommand)]
enum McpCommands {
    /// Serve MCP over stdin/stdout
    Serve {
        /// Use stdio transport
        #[arg(long)]
        stdio: bool,
    },
    /// Inspect MCP tool surface
    Tools {
        /// List all exposed tool names
        #[arg(long)]
        list: bool,
    },
}

/// Position the voice orb at the bottom-right of the window's current monitor.
fn position_orb_bottom_right(window: &tauri::WebviewWindow) {
    if let Ok(Some(monitor)) = window.current_monitor() {
        let scale = monitor.scale_factor();
        let pos: tauri::LogicalPosition<f64> = monitor.position().to_logical(scale);
        let size: tauri::LogicalSize<f64> = monitor.size().to_logical(scale);
        let x = pos.x + size.width - 200.0 - 24.0;
        let y = pos.y + size.height - 200.0 - 24.0;
        let _ = window.set_position(tauri::LogicalPosition::new(x, y));
    }
}

fn main() {
    // `--hook` short-circuit — runs as a tiny CLI client to the ingest socket.
    // Lives BEFORE `configure_mimalloc()` and clap parsing so hook spawns stay
    // sub-10ms (no Tauri runtime, no SQLite pool, no config watcher).
    let raw_args: Vec<String> = std::env::args().collect();
    if raw_args.get(1).map(String::as_str) == Some("--hook") {
        let _hook_args: Vec<String> = raw_args.into_iter().skip(2).collect();
        eprintln!("--hook is not supported");
        std::process::exit(1);
    }

    // Pre-main hardening: ptrace deny, core-dump disable, env-var scrub.
    // Must run before any allocator setup that may inspect env vars.
    klynt_process_hardening::pre_main_hardening();

    configure_mimalloc();
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Mcp { command }) => match command {
            McpCommands::Serve { stdio } => {
                if stdio {
                    run_mcp_stdio();
                } else {
                    eprintln!("Only --stdio transport is currently supported");
                    std::process::exit(1);
                }
            }
            McpCommands::Tools { list } => {
                if list {
                    run_mcp_tools_list();
                } else {
                    eprintln!("Use --list to print exposed tools");
                    std::process::exit(1);
                }
            }
        },
        None => {
            run_desktop_app();
        }
    }
}

fn run_mcp_stdio() {
    common::memory::set_purge_hook(purge_mimalloc);
    use tracing_subscriber::EnvFilter;

    // Init tracing to stderr (stdout is reserved for MCP transport)
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_stack_size(2 * 1024 * 1024)
        .enable_all()
        .build()
        .expect("Failed to create tokio runtime");
    rt.block_on(async {
        // Read whitelist from AppCore's finalised config (auto-filled from
        // AiFeatureRegistry when the user leaves exposed_tools empty).
        let config = config::load_with_env_overrides()
            .await
            .expect("config load failed");
        // Wire a SocketBridgeEmitter so global events (entity:updated,
        // provider:degraded, …) emitted by tools and handlers in this MCP
        // child process flow back to a running desktop app via the
        // bridge socket. When no desktop is running, frames are dropped
        // silently — the MCP child still functions standalone.
        let socket_path = mcp_bridge::bridge_socket_path()
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp/klynt-mcp-events.sock"));
        let bridge_client = mcp_bridge::BridgeClient::new(socket_path);
        let event_emitter: std::sync::Arc<dyn ::app_core::AppEventEmitter> =
            std::sync::Arc::new(mcp_bridge::SocketBridgeEmitter::new(bridge_client));

        let (app, events) = app_core::AppCore::init_with_sender(
            common::AppMode::Server,
            Some(config),
            None, // notification_sender — not needed for stdio MCP
            Some(event_emitter),
            None,
        )
        .await
        .expect("init failed");
        let app = Arc::new(app);

        // Drain unused EventChannels — both receivers must close before task exits.
        tokio::spawn(async move {
            let mut intervention_rx = events.intervention_rx;
            let mut pipeline_rx = events.pipeline_rx;
            let mut intervention_closed = false;
            let mut pipeline_closed = false;
            while !intervention_closed || !pipeline_closed {
                tokio::select! {
                    msg = intervention_rx.recv(), if !intervention_closed => {
                        if msg.is_none() { intervention_closed = true; }
                    }
                    result = pipeline_rx.recv(), if !pipeline_closed => {
                        if result.is_err() { pipeline_closed = true; }
                    }
                }
            }
        });

        let whitelist = app.config.read().await.mcp.server.exposed_tools.clone();
        if let Err(e) = klyntbot_server::serve_stdio(app, whitelist).await {
            tracing::error!("MCP serve failed: {e}");
        }
    });
}

fn run_mcp_tools_list() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to create tokio runtime");
    rt.block_on(async {
        if let Err(e) = klyntbot_server::print_exposed_tools().await {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    });
}

/// Wrapper for mi_collect that matches the `fn()` signature.
/// Called twice: first pass collects thread-local heaps into the global pool,
/// second pass returns freed segments to the OS.
fn purge_mimalloc() {
    unsafe {
        mi_collect(true);
        mi_collect(true);
    };
}

fn run_desktop_app() {
    // Register mimalloc purge as the global memory hook so lower-layer crates
    // (storage, agent) can trigger OS page return after large transient allocations.
    common::memory::set_purge_hook(purge_mimalloc);
    // Cap Tauri's tokio runtime to 4 workers with 2MB stacks (default: 1 per core, 8MB).
    // The runtime must not be dropped, so we leak it — it lives for the process lifetime.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .thread_stack_size(2 * 1024 * 1024)
        .enable_all()
        .build()
        .expect("tokio runtime");
    let handle = rt.handle().clone();
    Box::leak(Box::new(rt));
    tauri::async_runtime::set(handle);

    // Initialize tracing so info!/warn!/debug! output to stderr
    {
        use tracing_subscriber::EnvFilter;
        let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(std::io::stderr)
            .init();
    }

    let specta = specta_builder::build_specta();

    #[cfg(debug_assertions)]
    {
        if let Err(err) = specta.export(
            specta_typescript::Typescript::default()
                .header("// @ts-nocheck\n// Auto-generated by tauri-specta — DO NOT EDIT.\n")
                .bigint(specta_typescript::BigIntExportBehavior::Number),
            "../../desktop-ui/src/bindings.ts",
        ) {
            eprintln!("warn: tauri-specta export failed: {err}");
        }
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .setup(move |app| {
            specta.mount_events(app);
            let handle = app.handle().clone();
            let (core_inner, global_event_tx, approval_channel) =
                tauri::async_runtime::block_on(app_core::init(handle))
                    .expect("failed to initialize app core");
            let core = Arc::new(core_inner);

            // First-launch: register klyntbot's MCP server with Claude Code if present.
            // Idempotent — uses a marker file in ~/.klyntbot/ to run only once.
            std::thread::spawn(claude_code_integration::run_first_launch_check);

            // Start dev HTTP server (debug builds only) so localhost:1420 works in Chrome
            #[cfg(debug_assertions)]
            {
                let dev_core = Arc::clone(&core);
                tauri::async_runtime::spawn(async move {
                    dev_server::start(dev_core, global_event_tx).await;
                });
            }

            // Start embedded MCP HTTP server if enabled in config.
            // Must clone before app.manage(core) moves the Arc.
            {
                let mcp_core = Arc::clone(&core);
                let enabled = tauri::async_runtime::block_on(async {
                    mcp_core.config.read().await.mcp.server.enabled
                });
                if enabled {
                    tauri::async_runtime::spawn(async move {
                        let config = mcp_core.config.read().await;
                        let host = config.mcp.server.host.clone();
                        let port = config.mcp.server.port;
                        let whitelist = config.mcp.server.exposed_tools.clone();
                        let auth_config = config.mcp.server.auth.clone();
                        drop(config);

                        tracing::info!("Starting embedded MCP HTTP server on {host}:{port}");

                        use rmcp::transport::streamable_http_server::{
                            session::local::LocalSessionManager, StreamableHttpServerConfig,
                            StreamableHttpService,
                        };
                        use tokio_util::sync::CancellationToken;

                        let ct = CancellationToken::new();
                        let mcp_config = StreamableHttpServerConfig {
                            cancellation_token: ct.clone(),
                            ..Default::default()
                        };

                        let factory_app = mcp_core;
                        let mcp_service: StreamableHttpService<
                            klyntbot_server::KlyntbotServerHandler,
                            LocalSessionManager,
                        > = StreamableHttpService::new(
                            move || {
                                Ok(klyntbot_server::KlyntbotServerHandler::new(
                                    factory_app.clone(),
                                    whitelist.clone(),
                                ))
                            },
                            std::sync::Arc::new(LocalSessionManager::default()),
                            mcp_config,
                        );

                        let mut router = axum::Router::new().nest_service("/mcp", mcp_service);

                        // Wire bearer-token auth middleware if configured.
                        if auth_config.enabled {
                            if let Some(ref token) = auth_config.token {
                                let expected = token.expose().clone();
                                router = router.layer(axum::middleware::from_fn(
                                    move |req: axum::extract::Request,
                                          next: axum::middleware::Next| {
                                        use axum::response::IntoResponse;
                                        let expected = expected.clone();
                                        async move {
                                            let auth_header = req
                                                .headers()
                                                .get(axum::http::header::AUTHORIZATION)
                                                .and_then(|v| v.to_str().ok());
                                            match auth_header {
                                                Some(value)
                                                    if value.strip_prefix("Bearer ")
                                                        == Some(expected.as_str()) =>
                                                {
                                                    Ok(next.run(req).await)
                                                }
                                                _ => Err((
                                                    axum::http::StatusCode::UNAUTHORIZED,
                                                    "Unauthorized: invalid or missing Bearer token",
                                                )
                                                    .into_response()),
                                            }
                                        }
                                    },
                                ));
                            }
                        }

                        let bind_addr = format!("{host}:{port}");
                        match tokio::net::TcpListener::bind(&bind_addr).await {
                            Ok(listener) => {
                                tracing::info!("MCP HTTP server listening on {bind_addr}");
                                if let Err(e) = axum::serve(listener, router)
                                    .with_graceful_shutdown(
                                        async move { ct.cancelled_owned().await },
                                    )
                                    .await
                                {
                                    tracing::error!("MCP HTTP server error: {e}");
                                }
                            }
                            Err(e) => {
                                tracing::error!(
                                    "Failed to bind MCP HTTP server to {bind_addr}: {e}"
                                );
                            }
                        }
                    });
                }
            }

            app.manage(core);
            app.manage(approval_channel);
            app.manage(Arc::new(focus_timer::FocusTimer::new()));

            // No periodic mi_collect timer needed — common::memory::purge_freed_memory()
            // is called at the source (after LanceDB compaction, prune, index creation)
            // via the global purge hook registered above.

            // Register global shortcuts from config (or defaults if config invalid).
            {
                let core_ref = app.state::<Arc<app_core::AppCore>>();
                let shortcuts_config = tauri::async_runtime::block_on(async {
                    core_ref.config.read().await.shortcuts.clone()
                });
                if let Err(e) = shortcuts::register_shortcuts(app.handle(), &shortcuts_config) {
                    tracing::warn!(
                        "Failed to register shortcuts from config, falling back to defaults: {e}"
                    );
                    let defaults = config::ShortcutsConfig::default();
                    if let Err(e2) = shortcuts::register_shortcuts(app.handle(), &defaults) {
                        tracing::error!("Failed to register default shortcuts: {e2}");
                    }
                }
            }

            // Register voice hotkey (separate from the 3-shortcut system because
            // it toggles capture, not a window).
            {
                let core_ref = app.state::<Arc<app_core::AppCore>>();
                let voice_hotkey = tauri::async_runtime::block_on(async {
                    core_ref.config.read().await.voice.input.hotkey.clone()
                });
                tracing::info!(
                    "Voice hotkey setup: service_available={}, hotkey={voice_hotkey}",
                    core_ref.voice_service().is_ok()
                );
                // Always register the hotkey — the handler gracefully errors if voice isn't ready.
                {
                    let manager = app.global_shortcut();
                    match voice_hotkey.parse::<tauri_plugin_global_shortcut::Shortcut>() {
                        Ok(shortcut) => {
                            let app_clone = app.handle().clone();
                            if let Err(e) =
                                manager.on_shortcut(shortcut, move |_app, _shortcut, event| {
                                    if event.state
                                        != tauri_plugin_global_shortcut::ShortcutState::Pressed
                                    {
                                        return;
                                    }
                                    tracing::info!("Voice hotkey pressed");
                                    let handle = app_clone.clone();
                                    tauri::async_runtime::spawn(async move {
                                        use tauri::{Emitter, Manager};

                                        // Context-aware: focus session → quick voice journal
                                        if crate::tray_countdown::FOCUS_ACTIVE
                                            .load(std::sync::atomic::Ordering::Relaxed)
                                        {
                                            let core =
                                                handle.state::<std::sync::Arc<app_core::AppCore>>();
                                            // Quick capture without orb — start conversation manager directly
                                            if let Ok(manager) = core.voice_conversation_manager() {
                                                match manager.start(None).await {
                                                    Ok(_) => {
                                                        crate::tray_countdown::VOICE_ACTIVE.store(
                                                            true,
                                                            std::sync::atomic::Ordering::Relaxed,
                                                        );
                                                    }
                                                    Err(e) => {
                                                        tracing::warn!(
                                                            "Quick voice journal failed: {e}"
                                                        );
                                                    }
                                                }
                                            }
                                            return;
                                        }

                                        // Context-aware: launcher open → hands-free search
                                        if let Some(launcher) =
                                            handle.get_webview_window("launcher")
                                        {
                                            if launcher.is_visible().unwrap_or(false) {
                                                // Emit voice-recording-start to launcher for hands-free mode
                                                let _ = handle.emit("voice-recording-start", ());
                                                return;
                                            }
                                        }

                                        // Voice conversation manager toggle
                                        let core =
                                            handle.state::<std::sync::Arc<app_core::AppCore>>();
                                        if let Ok(manager) = core.voice_conversation_manager() {
                                            use ::app_core::handlers::voice_conversation::ConversationPhase;
                                            let phase = manager.phase().await;
                                            if phase == ConversationPhase::Idle {
                                                // First press → open orb and start conversation
                                                if let Some(orb_window) =
                                                    crate::lazy_window::get_or_create_window(&handle, "voice-orb")
                                                {
                                                    position_orb_bottom_right(&orb_window);
                                                    let _ = orb_window.show();
                                                    let _ = orb_window.set_focus();
                                                }

                                                // Brief yield so the voice-orb webview's event
                                                // listener is ready before events fire.
                                                tokio::time::sleep(
                                                    std::time::Duration::from_millis(100),
                                                )
                                                .await;

                                                // Emit a Tauri event that the frontend uses to
                                                // unlock AudioContext with a synthetic gesture.
                                                // Global hotkeys bypass WebKit's DOM gesture
                                                // requirement, so we signal the WebView to
                                                // dispatch a click + resume the AudioContext.
                                                {
                                                    use tauri::Emitter;
                                                    let _ = handle.emit("voice:unlock-audio", ());
                                                }

                                                match manager.start(None).await {
                                                    Ok(_) => {
                                                        crate::tray_countdown::VOICE_ACTIVE.store(
                                                            true,
                                                            std::sync::atomic::Ordering::Relaxed,
                                                        );
                                                    }
                                                    Err(e) => {
                                                        tracing::warn!(
                                                            "Failed to start voice conversation: {e}"
                                                        );
                                                        let _ = handle.emit(
                                                            "voice:event",
                                                            serde_json::json!({
                                                                "type": "error",
                                                                "message": e.to_string(),
                                                                "recoverable": true
                                                            }),
                                                        );
                                                    }
                                                }
                                            } else {
                                                // Active conversation → end it
                                                use ::app_core::handlers::voice_conversation::VoiceCommand;
                                                manager
                                                    .send_command(VoiceCommand::End)
                                                    .await;
                                                // Hide the orb
                                                if let Some(orb_window) =
                                                    handle.get_webview_window("voice-orb")
                                                {
                                                    let _ = orb_window.hide();
                                                }
                                                crate::tray_countdown::VOICE_ACTIVE.store(
                                                    false,
                                                    std::sync::atomic::Ordering::Relaxed,
                                                );
                                            }
                                        }
                                    });
                                })
                            {
                                tracing::warn!(
                                    "Failed to register voice hotkey '{voice_hotkey}': {e}"
                                );
                            } else {
                                tracing::info!("Voice hotkey registered: {voice_hotkey}");
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Invalid voice hotkey '{voice_hotkey}': {e}");
                        }
                    }
                }
            }

            // Custom macOS menu: Cmd+Q hides the dashboard (tray keeps running),
            // Cmd+W is NOT bound so the frontend can use it for in-app navigation.
            #[cfg(target_os = "macos")]
            {
                use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};

                let close_window = MenuItem::with_id(
                    app,
                    "close_window",
                    "Close Window",
                    true,
                    Some("CmdOrCtrl+Q"),
                )?;

                let app_menu = Submenu::with_items(
                    app,
                    "Klynt",
                    true,
                    &[
                        &PredefinedMenuItem::about(app, Some("About Klynt"), None)?,
                        &PredefinedMenuItem::separator(app)?,
                        &PredefinedMenuItem::hide(app, None)?,
                        &PredefinedMenuItem::hide_others(app, None)?,
                        &PredefinedMenuItem::show_all(app, None)?,
                        &PredefinedMenuItem::separator(app)?,
                        &close_window,
                    ],
                )?;

                let edit_menu = Submenu::with_items(
                    app,
                    "Edit",
                    true,
                    &[
                        &PredefinedMenuItem::undo(app, None)?,
                        &PredefinedMenuItem::redo(app, None)?,
                        &PredefinedMenuItem::separator(app)?,
                        &PredefinedMenuItem::cut(app, None)?,
                        &PredefinedMenuItem::copy(app, None)?,
                        &PredefinedMenuItem::paste(app, None)?,
                        &PredefinedMenuItem::select_all(app, None)?,
                    ],
                )?;

                let menu = Menu::with_items(app, &[&app_menu, &edit_menu])?;
                app.set_menu(menu)?;

                let handle = app.handle().clone();
                app.on_menu_event(move |_app, event| {
                    if event.id().as_ref() == "close_window" {
                        if let Some(window) = handle.get_webview_window("main") {
                            let _ = window.hide();
                        }
                        let _ =
                            handle.set_activation_policy(tauri::ActivationPolicy::Accessory);
                    }
                });
            }

            // Show the main window now that init is complete (starts hidden
            // via tauri.conf.json to avoid a blank window during boot).
            if let Some(main_window) = app.get_webview_window("main") {
                let _ = main_window.show();
                let _ = main_window.set_focus();

                // Hide on close instead of quitting — keeps the tray alive
                let mw = main_window.clone();
                let app_handle = app.handle().clone();
                main_window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = mw.hide();
                        // Remove from Dock — pure tray app when main window is hidden
                        #[cfg(target_os = "macos")]
                        let _ =
                            app_handle.set_activation_policy(tauri::ActivationPolicy::Accessory);
                    }
                });
            }

            // Build system tray icon — click toggles the tray popup window
            let tray_icon = Image::from_bytes(include_bytes!("../icons/tray.png"))
                .expect("failed to load tray icon");

            TrayIconBuilder::with_id("klynt-tray")
                .icon(tray_icon)
                .icon_as_template(true)
                .tooltip("Klynt")
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        // When voice is active, tray click toggles pause/resume
                        if tray_countdown::VOICE_ACTIVE.load(Ordering::Relaxed) {
                            let handle = tray.app_handle().clone();
                            tauri::async_runtime::spawn(async move {
                                let core =
                                    handle.state::<std::sync::Arc<app_core::AppCore>>();
                                if let Ok(manager) = core.voice_conversation_manager() {
                                    use ::app_core::handlers::voice_conversation::{
                                        ConversationPhase, VoiceCommand,
                                    };
                                    let phase = manager.phase().await;
                                    if phase == ConversationPhase::Idle {
                                        manager.send_command(VoiceCommand::Resume).await;
                                    } else {
                                        manager.send_command(VoiceCommand::Pause).await;
                                    }
                                }
                            });
                            return;
                        }
                        shortcuts::toggle_window(tray.app_handle(), WINDOW_TRAY);
                    }
                })
                .build(app)?;

            // Auxiliary windows (tray, launcher, quick-capture, distraction-overlay,
            // voice-orb) are created lazily on first show via lazy_window.rs.
            // Dismiss-on-blur handlers are registered at creation time.

            // Periodic mimalloc memory compaction — force the allocator to
            // return freed pages to the OS. Without this, mimalloc retains all
            // freed thread-local pages indefinitely, causing RSS to grow
            // monotonically in long-running sessions (macOS Activity Monitor
            // shows RSS, not actual live allocations).
            {
                let shutdown = app.state::<Arc<app_core::AppCore>>().shutdown_token.clone();
                tauri::async_runtime::spawn(async move {
                    let mut interval =
                        tokio::time::interval(std::time::Duration::from_secs(10));
                    loop {
                        tokio::select! {
                            _ = shutdown.cancelled() => break,
                            _ = interval.tick() => {
                                // SAFETY: mi_collect is thread-safe and idempotent.
                                // `true` = force aggressive collection across all threads.
                                unsafe { mi_collect(true); }
                            }
                        }
                    }
                });
            }

            // Start the tray countdown (next upcoming event in menu bar)
            {
                let core = app.state::<Arc<app_core::AppCore>>();
                let shutdown = core.shutdown_token.clone();
                let bus = core
                    .domain_event_bus()
                    .expect("domain event bus must be initialised before tray countdown")
                    .clone();
                tray_countdown::spawn(app.handle(), shutdown, bus);
            }

            Ok(())
        })
        .invoke_handler(crate::specta_builder::klynt_invoke_handler())
        .build(tauri::generate_context!())
        .expect("error while building Klynt desktop")
        .run(|_app, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                if !QUIT_REQUESTED.load(Ordering::SeqCst) {
                    api.prevent_exit();
                }
            }
        });
}
