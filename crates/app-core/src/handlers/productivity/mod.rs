//! Productivity handlers — daily summaries, focus sessions, activity timeline,
//! goals, time entries, insights, auto-focus, and project tracking.

pub mod converters;
mod calendar;
mod focus;
mod summaries;
mod tracking;

pub use converters::{
    event_to_timeline, insight_to_response, project_to_response, session_to_response,
    summary_to_response,
};
