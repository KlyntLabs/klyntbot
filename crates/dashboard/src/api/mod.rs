//! REST API handlers — one module per resource.
//!
//! All handlers receive `State<AppState>` and return `Result<Json<T>, ApiError>`.

pub mod calendar;
pub mod cron;
pub mod finance;
pub mod health;
pub mod plans;
pub mod projects;
pub mod sessions;
pub mod settings;
pub mod skills;
pub mod status;
pub mod tasks;
