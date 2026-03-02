#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_core;
mod commands;

use app_core::AppCore;
use commands::window::{WINDOW_LAUNCHER, WINDOW_TRAY};
use tauri::image::Image;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Manager, PhysicalPosition};
use tauri_plugin_global_shortcut::{Code, Modifiers, ShortcutState};

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
    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_shortcuts(["alt+space"])
                .expect("failed to parse shortcut")
                .with_handler(|app, shortcut, event| {
                    if event.state == ShortcutState::Pressed
                        && shortcut.matches(Modifiers::ALT, Code::Space)
                    {
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
                })
                .build(),
        )
        .setup(|app| {
            let core = tauri::async_runtime::block_on(AppCore::init())
                .expect("failed to initialize app core");

            // Store AppHandle in core for event emission (chat commands use it)
            app.manage(core);

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
                        rect,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window(WINDOW_TRAY) {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                // Position window directly below the tray icon
                                if let Ok(win_size) = window.outer_size() {
                                    let scale = window.scale_factor().unwrap_or(1.0);
                                    let tray_pos = rect.position.to_physical::<f64>(scale);
                                    let tray_size = rect.size.to_physical::<f64>(scale);
                                    let x = tray_pos.x + (tray_size.width / 2.0)
                                        - (win_size.width as f64 / 2.0);
                                    let y = tray_pos.y + tray_size.height;
                                    let _ = window
                                        .set_position(PhysicalPosition::new(x as i32, y as i32));
                                }
                                let _ = window.show();
                                let _ = window.set_focus();
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

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Tasks
            commands::tasks::today_tasks,
            commands::tasks::task_list,
            commands::tasks::task_create,
            commands::tasks::task_update,
            commands::tasks::task_delete,
            commands::tasks::task_toggle_complete,
            commands::tasks::project_list,
            commands::tasks::objective_list,
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
            commands::chat::chat_cancel,
            // Calendar
            commands::calendar::calendar_events,
            // Status
            commands::status::agent_status,
            // Window
            commands::window::resize_window,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Klynt desktop");
}
