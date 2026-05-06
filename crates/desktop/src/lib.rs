pub mod app_core;
pub mod approval;
pub mod claude_code_integration;
pub mod commands;
#[cfg(debug_assertions)]
pub mod dev_server;
pub mod focus_timer;
pub mod lazy_window;
pub mod notify;
pub mod oauth;
pub mod shortcuts;
pub mod specta_builder;
pub mod tray_countdown;

/// Snapshot of every command currently registered via `tauri::generate_handler!`.
/// Hand-maintained alongside the `generate_handler!` list during the Plan 6
/// migration. **Deleted in Phase E.** The `no_double_registration` test
/// asserts no command name appears in both this list and `KLYNT_COMMANDS`.
pub const LEGACY_COMMAND_NAMES: &[&str] = &[];
