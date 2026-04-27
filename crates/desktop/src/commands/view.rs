use desktop_macros::klynt_command;
use desktop_shared::commands::view::{ActiveViewResponse, SetActiveViewParams};
use desktop_shared::CommandResult;

use crate::app_core::AppCore;

#[klynt_command]
pub async fn view_set_active(
    params: SetActiveViewParams,
) -> () {
    state.view_set_active(params).await
}

#[klynt_command]
pub async fn view_clear_active() -> () {
    state.view_clear_active().await
}

#[klynt_command]
pub async fn view_get_active() -> ActiveViewResponse {
    state.view_get_active().await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<CommandResult<serde_json::Value>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "view_set_active" => dev::val(
            core.view_set_active(try_field!(dev::parse_params(body)))
                .await,
        ),
        "view_clear_active" => dev::val(core.view_clear_active().await),
        "view_get_active" => dev::val(core.view_get_active().await),
        _ => return None,
    })
}
