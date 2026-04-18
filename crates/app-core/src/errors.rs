use chrono::{DateTime, Utc};
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
/// Bridge function: returns `DateTime<Utc>` for unmigrated `feature-productivity` call sites.
pub fn parse_date(s: &str) -> Option<DateTime<Utc>> {
    jiff::civil::Date::strptime("%Y-%m-%d", s)
        .ok()
        .and_then(|d| {
            let ts = d
                .at(0, 0, 0, 0)
                .to_zoned(jiff::tz::TimeZone::UTC)
                .ok()?
                .timestamp();
            Some(common::time::bridge::jiff_to_chrono(ts))
        })
}

/// Parse a "YYYY-MM-DD" string or return a validation `ApiError`.
/// Bridge function: returns `DateTime<Utc>` for unmigrated `feature-productivity` call sites.
pub fn parse_date_or_err(s: &str) -> Result<DateTime<Utc>, ApiError> {
    parse_date(s).ok_or_else(|| ApiError::new("VALIDATION", format!("invalid date: {s}")))
}

/// Parse an RFC3339 timestamp or return an `INVALID_DATE` `ApiError`.
/// Bridge function: returns `DateTime<Utc>` for unmigrated `feature-productivity` call sites.
pub fn parse_rfc3339_or_err(field: &str, s: &str) -> Result<DateTime<Utc>, ApiError> {
    common::parse_datetime(s, "UTC")
        .ok_or_else(|| ApiError::new("INVALID_DATE", format!("Bad {field}: {s}")))
}

/// Parse a local date into UTC day boundaries, accounting for timezone offset.
///
/// `tz_offset_mins` is the JS-style offset (e.g. -420 for UTC+7, meaning local = UTC + 7h).
/// When `None`, treats the date as UTC (offset = 0).
///
/// Returns `(start_utc, end_utc)` representing the 24-hour local day in UTC.
/// Bridge function: returns `DateTime<Utc>` for unmigrated `feature-productivity` call sites.
pub fn parse_local_day_range(
    s: &str,
    tz_offset_mins: Option<i32>,
) -> Result<(DateTime<Utc>, DateTime<Utc>), ApiError> {
    let date = jiff::civil::Date::strptime("%Y-%m-%d", s)
        .map_err(|_| ApiError::new("VALIDATION", format!("invalid date: {s}")))?;
    let midnight = date
        .at(0, 0, 0, 0)
        .to_zoned(jiff::tz::TimeZone::UTC)
        .map_err(|_| ApiError::new("VALIDATION", format!("invalid date: {s}")))?
        .timestamp();
    // JS `getTimezoneOffset()` returns minutes *behind* UTC (e.g. UTC+7 → -420).
    // To get UTC from local midnight: UTC = local - offset_from_utc = local + js_offset.
    let offset_secs = i64::from(tz_offset_mins.unwrap_or(0)) * 60;
    let start = midnight
        .checked_add(jiff::SignedDuration::from_secs(offset_secs))
        .unwrap_or(midnight);
    let end = start
        .checked_add(jiff::SignedDuration::from_secs(86400))
        .unwrap_or(start);
    Ok((
        common::time::bridge::jiff_to_chrono(start),
        common::time::bridge::jiff_to_chrono(end),
    ))
}

/// Parse a "YYYY-MM-DD" string into a `jiff::civil::Date`.
pub fn parse_naive_date(s: &str) -> Option<jiff::civil::Date> {
    jiff::civil::Date::strptime("%Y-%m-%d", s).ok()
}

/// Convert a config save error into an `ApiError`.
pub fn map_config_save_err(e: impl std::fmt::Display) -> ApiError {
    ApiError::new("CONFIG_SAVE", e.to_string())
}

/// Convert a serialization error into an `ApiError`.
pub fn map_serialization_err(e: impl std::fmt::Display) -> ApiError {
    ApiError::new("SERIALIZATION", e.to_string())
}

/// Convert a `KlyntbotError` into a work-context–flavored `ApiError`.
pub fn map_activity_log_err(e: common::KlyntbotError) -> ApiError {
    ApiError::new("WORK_CONTEXT_ERROR", e.to_string())
}
