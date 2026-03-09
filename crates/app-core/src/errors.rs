use chrono::{DateTime, NaiveDate, Utc};
use desktop_shared::errors::ApiError;

/// Convert a `KlyntbotError` into a productivity-flavored `ApiError`.
pub fn map_prod_err(e: common::KlyntbotError) -> ApiError {
    ApiError::new("PRODUCTIVITY_ERROR", e.to_string())
}

/// Convert a cognitive/sqlx error into an `ApiError`.
pub fn map_cognitive_err(e: impl std::fmt::Display) -> ApiError {
    ApiError::new("STORAGE_ERROR", e.to_string())
}

/// Convert a `StorageError` into an `ApiError`, preserving specific error codes
/// for NotFound and Conflict variants.
pub fn map_storage_err(e: storage::StorageError) -> ApiError {
    match e {
        storage::StorageError::NotFound(msg) => ApiError::new("NOT_FOUND", msg),
        storage::StorageError::Conflict(msg) => ApiError::new("CONFLICT", msg),
        other => ApiError::new("STORAGE_ERROR", other.to_string()),
    }
}

/// Parse a "YYYY-MM-DD" string into a midnight UTC DateTime.
pub fn parse_date(s: &str) -> Option<DateTime<Utc>> {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .ok()
        .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc())
}

/// Parse a "YYYY-MM-DD" string or return a validation `ApiError`.
pub fn parse_date_or_err(s: &str) -> Result<DateTime<Utc>, ApiError> {
    parse_date(s).ok_or_else(|| ApiError::new("VALIDATION", format!("invalid date: {s}")))
}

/// Parse a local date into UTC day boundaries, accounting for timezone offset.
///
/// `tz_offset_mins` is the JS-style offset (e.g. -420 for UTC+7, meaning local = UTC + 7h).
/// When `None`, treats the date as UTC (offset = 0).
///
/// Returns `(start_utc, end_utc)` representing the 24-hour local day in UTC.
pub fn parse_local_day_range(
    s: &str,
    tz_offset_mins: Option<i32>,
) -> Result<(DateTime<Utc>, DateTime<Utc>), ApiError> {
    let naive_date = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|_| ApiError::new("VALIDATION", format!("invalid date: {s}")))?;
    let midnight = naive_date.and_hms_opt(0, 0, 0).unwrap().and_utc();
    // JS `getTimezoneOffset()` returns minutes *behind* UTC (e.g. UTC+7 → -420).
    // To get UTC from local midnight: UTC = local - offset_from_utc = local + js_offset.
    let offset_secs = i64::from(tz_offset_mins.unwrap_or(0)) * 60;
    let start = midnight + chrono::Duration::seconds(offset_secs);
    let end = start + chrono::Duration::days(1);
    Ok((start, end))
}

/// Parse a "YYYY-MM-DD" string into a `NaiveDate`.
pub fn parse_naive_date(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
}

/// Convert a config save error into an `ApiError`.
pub fn map_config_save_err(e: impl std::fmt::Display) -> ApiError {
    ApiError::new("CONFIG_SAVE", e.to_string())
}

/// Convert a serialization error into an `ApiError`.
pub fn map_serialization_err(e: impl std::fmt::Display) -> ApiError {
    ApiError::new("SERIALIZATION", e.to_string())
}
