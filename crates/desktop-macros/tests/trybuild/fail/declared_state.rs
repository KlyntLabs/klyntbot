//! Declaring a `state` parameter should fail.

#[desktop_macros::klynt_command]
pub async fn ping(state: tauri::State<'_, std::sync::Arc<app_core::AppCore>>) -> i32 {
    42
}

fn main() {}
