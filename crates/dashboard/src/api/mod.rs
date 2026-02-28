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

use axum::http::StatusCode;

use crate::error::ApiError;

/// Parse a comma-separated query param into a `Vec<String>`, trimming whitespace
/// and discarding empty segments. Used by list endpoints for tag filtering.
pub fn parse_comma_tags(s: &str) -> Vec<String> {
    s.split(',')
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect()
}

/// Return 204 No Content if the operation succeeded, or 404 if nothing was affected.
/// Used by delete handlers that return a `bool` indicating whether the row existed.
pub fn deleted_or_not_found(deleted: bool, entity: &str, id: &str) -> Result<StatusCode, ApiError> {
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found(format!("{entity} '{id}' not found")))
    }
}
