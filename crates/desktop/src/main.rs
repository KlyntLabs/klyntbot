#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_core;
mod commands;
#[cfg(debug_assertions)]
mod dev_server;
mod focus_timer;
mod notify;
mod oauth;
mod shortcuts;
mod tray_countdown;

use std::sync::atomic::Ordering;
use std::sync::Arc;

use clap::{Parser, Subcommand};
use commands::window::{QUIT_REQUESTED, WINDOW_LAUNCHER, WINDOW_QUICK_CAPTURE, WINDOW_TRAY};
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
}

/// Register a dismiss-on-blur handler that hides the window when it loses focus.
fn dismiss_on_blur(window: &tauri::WebviewWindow) {
    let w = window.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::Focused(false) = event {
            let _ = w.hide();
        }
    });
}

fn main() {
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
        },
        None => {
            run_desktop_app();
        }
    }
}

fn run_mcp_stdio() {
    use klyntbot_server::handler::KlyntbotServerHandler;
    use rmcp::service::ServiceExt;
    use tracing_subscriber::EnvFilter;

    // Init tracing to stderr (stdout is reserved for MCP transport)
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    rt.block_on(async {
        // Load config
        let config = config::load_with_env_overrides()
            .await
            .expect("config load failed");

        // Init AppCore in Server mode
        let (app, events) = app_core::AppCore::init(common::AppMode::Server, Some(config.clone()))
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

        // Build MCP handler
        let whitelist = config.mcp.server.exposed_tools.clone();
        let handler = KlyntbotServerHandler::new(app.clone(), whitelist);

        // Serve over stdio
        tracing::info!("Starting MCP server (stdio)");
        let transport = rmcp::transport::io::stdio();
        let service = match handler.serve(transport).await {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("Failed to serve MCP: {e}");
                app.shutdown().await;
                return;
            }
        };

        tokio::select! {
            result = service.waiting() => {
                if let Err(e) = result { eprintln!("Server error: {e}"); }
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("Shutting down...");
            }
        }

        app.shutdown().await;
    });
}

fn run_desktop_app() {
    // Initialize tracing so info!/warn!/debug! output to stderr
    {
        use tracing_subscriber::EnvFilter;
        let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(std::io::stderr)
            .init();
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let core = Arc::new(
                tauri::async_runtime::block_on(app_core::init(handle))
                    .expect("failed to initialize app core"),
            );

            // Start dev HTTP server (debug builds only) so localhost:1420 works in Chrome
            #[cfg(debug_assertions)]
            {
                let dev_core = Arc::clone(&core);
                tauri::async_runtime::spawn(async move {
                    dev_server::start(dev_core).await;
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
            app.manage(Arc::new(focus_timer::FocusTimer::new()));

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
                    core_ref.voice_service.is_some()
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
                                            // Quick capture without orb — just start, record, stop, spoken confirmation
                                            match core.voice_start_capture().await {
                                                Ok(_) => {
                                                    crate::tray_countdown::VOICE_ACTIVE.store(
                                                        true,
                                                        std::sync::atomic::Ordering::Relaxed,
                                                    );
                                                    // No orb — voice events still flow for the background pipeline
                                                }
                                                Err(e) => {
                                                    tracing::warn!(
                                                        "Quick voice journal failed: {e}"
                                                    );
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

                                        // Check if voice-orb is already visible (toggle behavior)
                                        if let Some(orb_window) =
                                            handle.get_webview_window("voice-orb")
                                        {
                                            let is_visible =
                                                orb_window.is_visible().unwrap_or(false);
                                            if is_visible {
                                                // Second press while capturing → stop capture
                                                let core = handle
                                                    .state::<std::sync::Arc<app_core::AppCore>>();
                                                let _ = core.voice_stop_capture().await;
                                                crate::tray_countdown::VOICE_ACTIVE.store(
                                                    false,
                                                    std::sync::atomic::Ordering::Relaxed,
                                                );
                                                let _ = orb_window.hide();
                                                return;
                                            }
                                        }

                                        // First press → open orb and start capture
                                        if let Some(orb_window) =
                                            handle.get_webview_window("voice-orb")
                                        {
                                            // Position: top-center of active monitor, 80px from top
                                            if let Ok(Some(monitor)) = orb_window.current_monitor()
                                            {
                                                let monitor_pos = monitor.position();
                                                let monitor_size = monitor.size();
                                                let x = monitor_pos.x
                                                    + (monitor_size.width as i32 / 2)
                                                    - 160; // half of 320px width
                                                let y = monitor_pos.y + 80;
                                                let _ = orb_window.set_position(
                                                    tauri::PhysicalPosition::new(x, y),
                                                );
                                            }
                                            let _ = orb_window.show();
                                            let _ = orb_window.set_focus();
                                        }

                                        // Brief yield so the voice-orb webview's event listener
                                        // is ready before CaptureStarted fires.
                                        tokio::time::sleep(std::time::Duration::from_millis(100))
                                            .await;

                                        // Start voice capture
                                        let core =
                                            handle.state::<std::sync::Arc<app_core::AppCore>>();
                                        match core.voice_start_capture().await {
                                            Ok(_info) => {
                                                crate::tray_countdown::VOICE_ACTIVE.store(
                                                    true,
                                                    std::sync::atomic::Ordering::Relaxed,
                                                );
                                            }
                                            Err(e) => {
                                                tracing::warn!(
                                                    "Failed to start voice capture: {e}"
                                                );
                                                // Emit error to orb
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
                        shortcuts::toggle_window(tray.app_handle(), WINDOW_TRAY);
                    }
                })
                .build(app)?;

            // Tray window — transparent + dismiss-on-blur (CSS handles the card styling)
            if let Some(tray_window) = app.get_webview_window(WINDOW_TRAY) {
                dismiss_on_blur(&tray_window);
            }

            // Launcher window — dismiss-on-blur + reset state so next open is fresh
            if let Some(launcher_window) = app.get_webview_window(WINDOW_LAUNCHER) {
                let w = launcher_window.clone();
                launcher_window.on_window_event(move |event| {
                    if let tauri::WindowEvent::Focused(false) = event {
                        let _ = w.hide();
                        // Reset launcher state so next open (via hotkey) starts fresh
                        use tauri::Emitter;
                        let _ = w.emit("voice-recording-reset", ());
                    }
                });
            }

            // Quick capture window — dismiss-on-blur
            if let Some(capture_window) = app.get_webview_window(WINDOW_QUICK_CAPTURE) {
                dismiss_on_blur(&capture_window);
            }

            // Start the tray countdown (next upcoming event in menu bar)
            tray_countdown::spawn(app.handle());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Timeline / Dashboard
            commands::timeline::timeline_query,
            // Tasks
            commands::tasks::today_tasks,
            commands::tasks::task_get,
            commands::tasks::task_list,
            commands::tasks::task_create,
            commands::tasks::task_update,
            commands::tasks::task_delete,
            commands::tasks::task_toggle_complete,
            commands::tasks::task_list_children,
            commands::tasks::task_get_suggestions,
            commands::tasks::task_apply_suggestion,
            commands::tasks::task_dismiss_suggestion,
            commands::tasks::task_decompose,
            commands::tasks::task_apply_decomposition,
            commands::tasks::task_reject_decomposition,
            commands::tasks::task_forecast,
            commands::tasks::task_add_dependency,
            commands::tasks::task_list_dependencies,
            commands::tasks::task_add_attachment,
            commands::tasks::task_list_attachments,
            commands::tasks::task_add_time_entry,
            commands::tasks::task_list_time_entries,
            commands::tasks::project_list,
            commands::tasks::objective_list,
            // Notes
            commands::notes::note_list,
            commands::notes::note_get,
            commands::notes::note_create,
            commands::notes::note_update,
            commands::notes::note_delete,
            commands::notes::note_search,
            commands::notes::note_search_semantic,
            commands::notes::note_search_hybrid,
            commands::notes::note_links_all,
            commands::notes::note_list_by_entity,
            commands::notes::note_version_list,
            commands::notes::note_version_create,
            commands::notes::note_version_restore,
            commands::notes::note_save_attachment,
            commands::notes::notebook_list,
            commands::notes::notebook_create,
            commands::notes::notebook_update,
            commands::notes::notebook_delete,
            commands::notes::note_archive,
            commands::notes::note_unarchive,
            commands::notes::note_list_archived,
            commands::notes::note_backlinks,
            commands::notes::note_suggestions,
            commands::notes::note_tags_all,
            commands::notes::note_unlinked_mentions,
            commands::notes::inbox_create,
            commands::notes::inbox_list,
            commands::notes::inbox_delete,
            commands::notes::note_insight_review,
            commands::notes::note_insight_cache_get,
            commands::notes::note_insight_save_flashcards,
            commands::notes::note_insight_submit_quiz,
            commands::notes::note_insight_regenerate_tab,
            commands::notes::note_insight_debate,
            commands::notes::note_insight_list_versions,
            commands::notes::note_insight_get_evolution,
            commands::notes::note_insight_get_version,
            commands::notes::note_insight_generate_scenario,
            commands::notes::note_insight_changes_summary,
            commands::notes::note_insight_knowledge_growth,
            commands::notes::note_insight_list_personas,
            commands::notes::note_insight_create_persona,
            commands::notes::note_insight_update_persona,
            commands::notes::note_insight_delete_persona,
            commands::notes::note_insight_toggle_persona,
            commands::notes::note_insight_set_pins,
            commands::notes::note_insight_rate_persona,
            commands::notes::note_insight_auto_generate_persona,
            commands::notes::note_insight_persona_chat,
            commands::notes::note_insight_preview_scope,
            commands::notes::flashcard_list_decks,
            commands::notes::flashcard_get_due,
            commands::notes::flashcard_record_review,
            commands::notes::flashcard_get,
            commands::notes::flashcard_create,
            commands::notes::flashcard_update,
            commands::notes::flashcard_list_cards,
            commands::notes::flashcard_delete,
            commands::notes::flashcard_get_all_due,
            commands::notes::flashcard_total_due,
            commands::notes::flashcard_list_struggling,
            commands::notes::flashcard_generate,
            commands::notes::flashcard_save_generated,
            commands::notes::flashcard_submit_answer,
            commands::notes::flashcard_explain_answer,
            commands::notes::flashcard_generate_distractors,
            commands::notes::flashcard_save_mode_preference,
            commands::notes::flashcard_get_mode_preference,
            commands::notes::flashcard_get_prerequisites,
            commands::notes::flashcard_save_session,
            commands::notes::flashcard_recent_learning_sessions,
            commands::notes::note_retention_health,
            commands::notes::note_editing_finished,
            commands::notes::note_import_files,
            commands::notes::note_export,
            commands::notes::note_insight_tab_chat,
            commands::notes::note_insight_clear_tab_chats,
            // Fabric Explorer
            commands::fabric::fabric_graph_base,
            commands::fabric::fabric_graph_expand,
            commands::fabric::fabric_graph_action,
            // Voice
            commands::voice::voice_start_capture,
            commands::voice::voice_stop_capture,
            commands::voice::voice_dismiss,
            commands::voice::voice_get_status,
            commands::voice::voice_get_models,
            commands::voice::voice_download_model,
            commands::voice::voice_simulate_event,
            // Annotations
            commands::annotations::annotation_create,
            commands::annotations::annotation_update,
            commands::annotations::annotation_delete,
            commands::annotations::annotation_list_for_note,
            commands::annotations::annotation_get_ai_suggestion,
            commands::annotations::note_get_linked_context,
            // Language Learning
            commands::language::language_translate_breakdown,
            commands::language::language_evaluate_translation,
            commands::language::language_save_vocabulary,
            commands::language::language_detect_confusables,
            commands::language::language_enrich_annotation,
            commands::language::language_quick_translate,
            // Practice Mode
            commands::practice::practice_segment_note,
            commands::practice::practice_start_session,
            commands::practice::practice_submit_unit,
            commands::practice::practice_confirm_unit,
            commands::practice::practice_get_session,
            commands::practice::practice_complete_session,
            commands::practice::practice_list_sessions,
            // Knowledge Health
            commands::knowledge_health::knowledge_health_summary,
            commands::knowledge_health::knowledge_topic_detail,
            // Morning Briefing
            commands::morning_briefing::morning_briefing_summary,
            // Retention History
            commands::retention_history::retention_history,
            // Review Stats
            commands::review_stats::review_stats_summary,
            // Areas
            commands::areas::area_list,
            commands::areas::area_create,
            commands::areas::area_update,
            commands::areas::area_delete,
            commands::areas::area_reorder,
            // Projects
            commands::projects::project_create,
            commands::projects::project_get,
            commands::projects::project_update,
            commands::projects::project_delete,
            commands::projects::project_archive,
            commands::projects::project_update_instructions,
            commands::projects::project_update_role,
            commands::projects::project_health_metrics,
            // Entities (knowledge graph)
            commands::entities::entity_search,
            commands::entities::entity_merge,
            commands::entities::entity_get_neighborhood,
            // Entity Links
            commands::entity_links::entity_link_create,
            commands::entity_links::entity_link_delete,
            commands::entity_links::entity_links_for_entity,
            // Project Memories
            commands::project_memories::project_memories_list,
            commands::project_memories::project_memories_by_type,
            // Project Conversations
            commands::project_conversations::project_conversations_list,
            // Project Sources
            commands::project_sources::project_source_create,
            commands::project_sources::project_source_delete,
            commands::project_sources::project_source_list,
            // Objectives
            commands::objectives::objective_create,
            commands::objectives::objective_get,
            commands::objectives::objective_update,
            commands::objectives::objective_delete,
            // Key Results
            commands::key_results::key_result_create,
            commands::key_results::key_result_update,
            commands::key_results::key_result_update_metric,
            commands::key_results::key_result_delete,
            // Chat
            commands::chat::chat_threads,
            commands::chat::chat_messages,
            commands::chat::chat_get_session,
            commands::chat::chat_list_sessions_by_project,
            commands::chat::chat_delete_stale_sessions,
            commands::chat::chat_send,
            commands::chat::chat_pin_thread,
            commands::chat::chat_rename_thread,
            commands::chat::chat_delete_thread,
            commands::chat::chat_respond_interaction,
            commands::chat::chat_cancel,
            // Finance — queries
            commands::finance::finance_accounts,
            commands::finance::finance_transactions,
            commands::finance::finance_transactions_filtered,
            commands::finance::finance_budget_usage,
            commands::finance::finance_portfolios,
            commands::finance::finance_investments,
            commands::finance::finance_investments_filtered,
            commands::finance::finance_goals,
            commands::finance::finance_liabilities,
            commands::finance::finance_net_worth,
            commands::finance::finance_exchange_rates,
            // Finance — mutations
            commands::finance::finance_account_create,
            commands::finance::finance_account_update,
            commands::finance::finance_account_delete,
            commands::finance::finance_transaction_create,
            commands::finance::finance_transaction_delete,
            commands::finance::finance_budget_create,
            commands::finance::finance_budget_update,
            commands::finance::finance_budget_delete,
            commands::finance::finance_goal_create,
            commands::finance::finance_goal_update,
            commands::finance::finance_goal_delete,
            commands::finance::finance_liability_create,
            commands::finance::finance_liability_update,
            commands::finance::finance_liability_delete,
            commands::finance::finance_portfolio_create,
            commands::finance::finance_investment_create,
            commands::finance::finance_investment_update,
            commands::finance::finance_allocation_target_upsert,
            commands::finance::finance_allocation_targets,
            commands::finance::finance_investment_tx_create,
            commands::finance::finance_investment_txs,
            // Finance — reports
            commands::finance::finance_report_spending,
            commands::finance::finance_report_income,
            commands::finance::finance_report_trends,
            commands::finance::finance_monthly_summary,
            commands::finance::finance_daily_spending,
            commands::finance::finance_period_summary,
            // Productivity
            commands::productivity::productivity_today,
            commands::productivity::productivity_timeline,
            commands::productivity::productivity_focus_start,
            commands::productivity::productivity_focus_end,
            commands::productivity::productivity_focus_status,
            commands::productivity::productivity_sessions,
            commands::productivity::productivity_intelligence_sessions,
            commands::productivity::productivity_weekly,
            commands::productivity::productivity_categories,
            commands::productivity::productivity_summary_range,
            commands::productivity::productivity_activity_feed,
            commands::productivity::productivity_goals,
            commands::productivity::productivity_pomodoro_start,
            commands::productivity::productivity_time_entries,
            commands::productivity::productivity_goal_create,
            commands::productivity::productivity_goal_delete,
            commands::productivity::productivity_goal_toggle,
            commands::productivity::productivity_time_entry_create,
            commands::productivity::productivity_time_entry_delete,
            commands::productivity::productivity_category_upsert,
            commands::productivity::productivity_tracked_apps,
            commands::productivity::productivity_category_delete,
            commands::productivity::productivity_recategorize_app,
            commands::productivity::productivity_insights,
            commands::productivity::productivity_insight_dismiss,
            commands::productivity::productivity_auto_focus_start,
            commands::productivity::productivity_auto_focus_end,
            commands::productivity::productivity_auto_focus_confirm,
            commands::productivity::distraction_respond,
            commands::productivity::productivity_projects_list,
            commands::productivity::productivity_project_upsert,
            commands::productivity::productivity_project_delete,
            commands::productivity::productivity_weekly_assessment,
            commands::productivity::productivity_calendar_events,
            commands::productivity::calendar_sync_events,
            commands::productivity::productivity_patterns,
            commands::productivity::productivity_hourly_breakdown,
            // Focus Session
            commands::productivity::focus_session_start,
            commands::productivity::focus_session_stop,
            commands::productivity::focus_session_status,
            commands::productivity::focus_session_pause,
            commands::productivity::focus_session_resume,
            commands::productivity::focus_session_extend,
            commands::productivity::focus_session_start_break,
            commands::productivity::focus_session_extend_work,
            commands::productivity::focus_session_skip_break,
            commands::productivity::focus_session_take_break,
            // Distraction
            commands::distraction::distraction_dismiss,
            commands::distraction::distraction_allow_temp,
            commands::distraction::distraction_allow_session,
            commands::distraction::distraction_learned_rules,
            commands::distraction::distraction_delete_rule,
            // Permissions
            commands::permissions::permissions_check_accessibility,
            commands::permissions::permissions_open_accessibility,
            // Settings (MCP)
            commands::settings::mcp_get_config,
            commands::settings::mcp_add_server,
            commands::settings::mcp_remove_server,
            commands::settings::mcp_toggle_server,
            commands::settings::mcp_update_server,
            // Settings (generic config)
            commands::settings::app_info,
            commands::settings::config_get_section,
            commands::settings::config_update_section,
            commands::settings::config_mark_setup_completed,
            // Shortcuts
            commands::shortcuts::shortcuts_get,
            commands::shortcuts::shortcuts_update,
            // OAuth
            oauth::commands::mcp_oauth_start,
            oauth::commands::mcp_oauth_disconnect,
            // Workflows
            commands::workflows::workflow_list,
            commands::workflows::workflow_get,
            commands::workflows::workflow_get_effective,
            commands::workflows::workflow_create,
            commands::workflows::workflow_delete,
            commands::workflows::label_create,
            commands::workflows::label_update,
            commands::workflows::label_delete,
            commands::workflows::label_reorder,
            // Groups
            commands::groups::group_list,
            commands::groups::group_create,
            commands::groups::group_update,
            commands::groups::group_delete,
            commands::groups::group_reorder,
            // Custom Columns
            commands::columns::custom_column_list,
            commands::columns::custom_column_create,
            commands::columns::custom_column_update,
            commands::columns::custom_column_delete,
            commands::columns::custom_column_reorder,
            commands::columns::custom_column_values,
            commands::columns::custom_column_value_set,
            commands::columns::custom_column_value_delete,
            // Cognitive Debug
            commands::cognitive::cognitive_user_model,
            commands::cognitive::cognitive_facts_list,
            commands::cognitive::cognitive_episodic_list,
            commands::cognitive::cognitive_rules_list,
            commands::cognitive::cognitive_memory_stats,
            commands::cognitive::memory_health,
            commands::cognitive::coaching_situation,
            commands::cognitive::coaching_signals,
            commands::cognitive::coaching_patterns,
            commands::cognitive::coaching_feedback_stats,
            commands::cognitive::coaching_router_status,
            commands::cognitive::coaching_pending_interventions,
            commands::cognitive::cognitive_system_status,
            commands::cognitive::cognitive_fact_create,
            commands::cognitive::cognitive_fact_update,
            commands::cognitive::cognitive_fact_delete,
            commands::cognitive::cognitive_rule_create,
            commands::cognitive::cognitive_rule_deactivate,
            commands::cognitive::cognitive_run_compaction,
            commands::cognitive::cognitive_run_reflection,
            commands::cognitive::coaching_reset_dismissals,
            commands::cognitive::coaching_clear_signals,
            commands::cognitive::coaching_seed_patterns,
            commands::cognitive::coaching_submit_feedback,
            commands::cognitive::coaching_report_ignored,
            commands::cognitive::coaching_intervention_log,
            commands::cognitive::cognitive_inject_event,
            commands::cognitive::cognitive_event_log,
            commands::cognitive::cognitive_pipeline_log,
            // Squads
            commands::squads::list_squads,
            commands::squads::get_squad,
            commands::squads::create_squad,
            commands::squads::update_squad,
            commands::squads::delete_squad,
            commands::squads::add_squad_member,
            commands::squads::remove_squad_member,
            // Cron / Automations
            commands::cron::cron_list,
            commands::cron::cron_status,
            commands::cron::cron_enable,
            commands::cron::cron_run,
            commands::cron::cron_delete,
            commands::cron::cron_create,
            commands::cron::cron_update,
            // Status
            commands::status::agent_status,
            // AutoTuner
            commands::autotuner::autotuner_status,
            commands::autotuner::autotuner_history,
            commands::autotuner::autotuner_revert,
            commands::autotuner::autotuner_pause,
            commands::autotuner::autotuner_resume,
            commands::autotuner::autotuner_set_pace,
            commands::autotuner::autotuner_get_toast_count,
            commands::autotuner::autotuner_increment_toast_count,
            // Mirror
            commands::mirror::get_mirror_state,
            commands::mirror::get_routing_history,
            commands::mirror::get_mirror_narratives,
            commands::mirror::get_pending_snippets,
            commands::mirror::submit_mirror_feedback,
            commands::mirror::generate_mirror_response,
            commands::mirror::approve_meta_rule,
            commands::mirror::dismiss_meta_rule,
            commands::mirror::get_brain_versions,
            commands::mirror::revert_brain_version,
            commands::mirror::kill_trial,
            commands::mirror::continue_trial,
            // Active View
            commands::view::view_set_active,
            commands::view::view_clear_active,
            commands::view::view_get_active,
            // Window
            // Work Contexts
            commands::work_context::list_work_contexts,
            commands::work_context::get_work_context,
            commands::work_context::get_work_context_detail,
            commands::work_context::update_work_context,
            commands::work_context::archive_work_context,
            commands::work_context::merge_work_contexts,
            commands::work_context::search_work_contexts,
            commands::work_context::get_context_timeline,
            commands::work_context::get_context_resume_data,
            commands::work_context::get_inference_stats,
            commands::work_context::get_dashboard_intelligence,
            commands::work_context::update_inference_config,
            // Capture
            commands::capture::capture_status,
            commands::capture::capture_shell_hook_status,
            commands::capture::capture_install_shell_hook,
            commands::capture::capture_uninstall_shell_hook,
            commands::capture::capture_get_ingestion_token,
            commands::capture::capture_regenerate_ingestion_token,
            // Workspace Config
            commands::workspace::workspace_list_files,
            commands::workspace::workspace_read_file,
            commands::workspace::workspace_write_file,
            // AI Tool Integrations
            commands::integrations::ai_tools_detect,
            commands::integrations::ai_tools_install,
            // Agent Profiles
            commands::agents::agent_list_profiles,
            commands::agents::agent_read_file,
            commands::agents::agent_write_file,
            commands::agents::agent_create_profile,
            commands::agents::agent_create_skill,
            commands::agents::agent_delete_file,
            // Launcher
            commands::launcher::launcher_search,
            commands::launcher::launcher_execute,
            commands::launcher::launcher_dashboard,
            commands::launcher::launcher_clipboard_paste,
            commands::launcher::launcher_clipboard_delete,
            commands::launcher::launcher_clipboard_pin,
            commands::launcher::launcher_run_script,
            commands::launcher::launcher_system_command,
            commands::launcher::launcher_open_app,
            commands::window::resize_window,
            commands::window::open_url,
            commands::window::show_dashboard,
            commands::window::quit_app,
        ])
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
