use desktop_macros::klynt_command;
use desktop_shared::commands::MorningBriefingResponse;

#[klynt_command]
pub async fn morning_briefing_summary() -> MorningBriefingResponse {
    state.morning_briefing().await
}
