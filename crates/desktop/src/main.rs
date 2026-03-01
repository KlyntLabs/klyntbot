#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_core;
mod commands;

use app_core::AppCore;

fn main() {
    tauri::Builder::default()
        .manage(AppCore::new())
        .invoke_handler(tauri::generate_handler![
            commands::tasks::task_list,
            commands::tasks::project_list,
            commands::tasks::objective_list,
            commands::chat::chat_send,
            commands::chat::chat_cancel,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Klynt desktop");
}
