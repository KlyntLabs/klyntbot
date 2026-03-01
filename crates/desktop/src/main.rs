#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_core;
mod commands;

use app_core::AppCore;
use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let core = tauri::async_runtime::block_on(AppCore::init())
                .expect("failed to initialize app core");
            app.manage(core);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::tasks::task_list,
            commands::tasks::project_list,
            commands::tasks::objective_list,
            commands::tasks::task_toggle_complete,
            commands::tasks::task_create,
            commands::areas::area_list,
            commands::calendar::calendar_events,
            commands::chat::chat_threads,
            commands::chat::chat_messages,
            commands::chat::chat_send,
            commands::chat::chat_cancel,
            commands::status::agent_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Klynt desktop");
}
