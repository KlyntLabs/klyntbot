use desktop_macros::klynt_command;
use desktop_shared::commands::{TimelineQuery, TimelineResponse, TimelineSource};

#[klynt_command]
pub async fn timeline_query(
    start_date: String,
    end_date: String,
    sources: Option<Vec<TimelineSource>>,
    include_point_events: Option<bool>,
    tz_offset_mins: Option<i32>,
) -> TimelineResponse {
    state
        .timeline_query(TimelineQuery {
            start_date,
            end_date,
            sources,
            include_point_events,
            tz_offset_mins,
        })
        .await
}
