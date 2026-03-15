#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_core;
mod commands;
#[cfg(debug_assertions)]
mod dev_server;
mod focus_timer;
mod notify;
mod oauth;
mod tray_countdown;

use std::sync::Arc;

use clap::{Parser, Subcommand};
use commands::window::{WINDOW_LAUNCHER, WINDOW_TRAY};
use tauri::image::Image;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::Manager;
use tauri_plugin_global_shortcut::{Code, Modifiers, ShortcutState};

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
    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_shortcuts(["alt+space", "alt+shift+space"])
                .expect("failed to parse shortcut")
                .with_handler(|app, shortcut, event| {
                    if event.state != ShortcutState::Pressed {
                        return;
                    }

                    // Option+Space → toggle launcher
                    if shortcut.matches(Modifiers::ALT, Code::Space) {
                        if let Some(window) = app.get_webview_window(WINDOW_LAUNCHER) {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.center();
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }

                    // Option+Shift+Space → toggle tray
                    if shortcut.matches(Modifiers::ALT | Modifiers::SHIFT, Code::Space) {
                        if let Some(window) = app.get_webview_window(WINDOW_TRAY) {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                focus_timer::open_tray_window(app);
                            }
                        }
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
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
                        drop(config);
                        tracing::info!("Starting embedded MCP HTTP server on {host}:{port}");
                        let handler =
                            klyntbot_server::KlyntbotServerHandler::new(mcp_core, whitelist);
                        // TODO: Wire rmcp streamable HTTP server transport
                        tracing::warn!(
                            "HTTP transport not yet implemented — MCP server not started"
                        );
                        let _ = handler;
                    });
                }
            }

            app.manage(core);
            app.manage(Arc::new(focus_timer::FocusTimer::new()));

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
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window(WINDOW_TRAY) {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                focus_timer::open_tray_window(app);
                            }
                        }
                    }
                })
                .build(app)?;

            // Tray window — transparent + dismiss-on-blur (CSS handles the card styling)
            if let Some(tray_window) = app.get_webview_window(WINDOW_TRAY) {
                dismiss_on_blur(&tray_window);
            }

            // Launcher window — no native blur, just transparent + dismiss-on-blur
            if let Some(launcher_window) = app.get_webview_window(WINDOW_LAUNCHER) {
                dismiss_on_blur(&launcher_window);
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
            commands::tasks::task_start_focus,
            commands::tasks::task_end_focus,
            commands::tasks::project_list,
            commands::tasks::objective_list,
            // Notes
            commands::notes::note_list,
            commands::notes::note_get,
            commands::notes::note_create,
            commands::notes::note_update,
            commands::notes::note_delete,
            commands::notes::note_search,
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
            // Finance — reports
            commands::finance::finance_report_spending,
            commands::finance::finance_report_income,
            commands::finance::finance_report_trends,
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
            commands::productivity::distraction_respond,
            commands::productivity::productivity_projects_list,
            commands::productivity::productivity_project_upsert,
            commands::productivity::productivity_project_delete,
            commands::productivity::productivity_weekly_assessment,
            commands::productivity::productivity_calendar_events,
            commands::productivity::calendar_sync_events,
            commands::productivity::productivity_patterns,
            commands::productivity::productivity_hourly_breakdown,
            // Focus Timer
            commands::productivity::focus_timer_start,
            commands::productivity::focus_timer_stop,
            commands::productivity::focus_timer_status,
            commands::productivity::focus_break_start,
            commands::productivity::focus_timer_extend,
            commands::productivity::focus_timer_pause,
            commands::productivity::focus_timer_resume,
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
            commands::cognitive::coaching_submit_feedback,
            commands::cognitive::coaching_report_ignored,
            commands::cognitive::cognitive_inject_event,
            commands::cognitive::cognitive_event_log,
            commands::cognitive::cognitive_pipeline_log,
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
            commands::launcher::launcher_window_action,
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
                api.prevent_exit();
            }
        });
}
