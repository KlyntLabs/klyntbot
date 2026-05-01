use app_core::coding::help_handler::HelpEntry;
use desktop_macros::klynt_command;
use desktop_shared::errors::ApiError;

#[klynt_command]
pub async fn coding_help(command: Option<String>) -> Vec<HelpEntry> {
    state
        .coding_help(command)
        .await
        .map_err(|e| ApiError::new("HELP_ERROR", e.to_string()))
}
