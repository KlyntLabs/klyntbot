//! Shared date/time parsing and formatting utilities.
//!
//! Single source of truth for all date parsing across the codebase.
//! Non-timezone strings are interpreted in the given fallback timezone.

use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;

/// Parse a date/datetime string with timezone awareness.
///
/// Accepts RFC3339, ISO datetime, date-only, and "YYYY-MM-DD HH:MM" formats.
/// Non-timezone strings are interpreted in the given `fallback_tz` (e.g. "Asia/Bangkok").
///
/// Returns `None` if the string is empty or cannot be parsed.
pub fn parse_datetime(s: &str, fallback_tz: &str) -> Option<DateTime<Utc>> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    // 1. RFC3339 with timezone (most specific)
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }

    // Parse the fallback timezone once for the remaining branches
    let tz: Option<Tz> = fallback_tz.parse().ok();

    // Helper: interpret a NaiveDateTime in the fallback timezone (or UTC)
    let to_utc = |naive: NaiveDateTime| -> Option<DateTime<Utc>> {
        if let Some(tz) = tz {
            tz.from_local_datetime(&naive)
                .earliest()
                .map(|dt| dt.with_timezone(&Utc))
        } else {
            Some(naive.and_utc())
        }
    };

    // 2. ISO datetime without timezone: "2026-02-17T21:00:00"
    if let Ok(naive) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
        return to_utc(naive);
    }

    // 3. "YYYY-MM-DD HH:MM:SS"
    if let Ok(naive) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return to_utc(naive);
    }

    // 4. "YYYY-MM-DD HH:MM"
    if let Ok(naive) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M") {
        return to_utc(naive);
    }

    // 5. Date only: "2026-02-17" → midnight in fallback timezone
    if let Ok(date) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        let midnight = date.and_hms_opt(0, 0, 0)?;
        return to_utc(midnight);
    }

    None
}

/// Format a UTC datetime for display in the given timezone.
pub fn format_datetime_local(dt: &DateTime<Utc>, timezone: &str, fmt: &str) -> String {
    if let Ok(tz) = timezone.parse::<Tz>() {
        dt.with_timezone(&tz).format(fmt).to_string()
    } else {
        dt.format(fmt).to_string()
    }
}

/// Get the UTC offset string for a timezone (e.g., "+07:00", "-05:00").
///
/// Uses the current time to determine the offset (accounts for DST).
pub fn timezone_utc_offset(timezone: &str) -> String {
    if let Ok(tz) = timezone.parse::<Tz>() {
        let now = Utc::now().with_timezone(&tz);
        now.format("%:z").to_string()
    } else {
        "+00:00".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, Timelike};

    #[test]
    fn test_parse_rfc3339_with_offset() {
        let dt = parse_datetime("2026-02-17T21:00:00+07:00", "UTC").unwrap();
        // 21:00 +07:00 = 14:00 UTC
        assert_eq!(dt.hour(), 14);
        assert_eq!(dt.day(), 17);
    }

    #[test]
    fn test_parse_rfc3339_utc() {
        let dt = parse_datetime("2026-02-17T21:00:00Z", "Asia/Bangkok").unwrap();
        assert_eq!(dt.hour(), 21);
        assert_eq!(dt.day(), 17);
    }

    #[test]
    fn test_parse_iso_datetime_no_tz_with_fallback() {
        // "2026-02-17T21:00:00" interpreted in Asia/Bangkok (UTC+7) = 14:00 UTC
        let dt = parse_datetime("2026-02-17T21:00:00", "Asia/Bangkok").unwrap();
        assert_eq!(dt.hour(), 14);
        assert_eq!(dt.day(), 17);
    }

    #[test]
    fn test_parse_date_only_in_timezone() {
        // "2026-02-17" midnight Bangkok (UTC+7) = 2026-02-16T17:00:00Z
        let dt = parse_datetime("2026-02-17", "Asia/Bangkok").unwrap();
        assert_eq!(dt.hour(), 17);
        assert_eq!(dt.day(), 16);
    }

    #[test]
    fn test_parse_date_with_time_space_format() {
        // "2026-02-17 21:00" in Bangkok = 14:00 UTC
        let dt = parse_datetime("2026-02-17 21:00", "Asia/Bangkok").unwrap();
        assert_eq!(dt.hour(), 14);
        assert_eq!(dt.day(), 17);
    }

    #[test]
    fn test_parse_empty_string() {
        assert!(parse_datetime("", "UTC").is_none());
    }

    #[test]
    fn test_parse_whitespace_only() {
        assert!(parse_datetime("   ", "UTC").is_none());
    }

    #[test]
    fn test_parse_invalid_string() {
        assert!(parse_datetime("not a date", "UTC").is_none());
    }

    #[test]
    fn test_parse_invalid_timezone_falls_back_to_utc() {
        // With invalid timezone, naive datetimes should be treated as UTC
        let dt = parse_datetime("2026-02-17T21:00:00", "Invalid/TZ").unwrap();
        assert_eq!(dt.hour(), 21);
    }

    #[test]
    fn test_format_datetime_local() {
        let dt = "2026-02-17T14:00:00Z".parse::<DateTime<Utc>>().unwrap();
        let formatted = format_datetime_local(&dt, "Asia/Bangkok", "%H:%M");
        assert_eq!(formatted, "21:00"); // 14:00 UTC = 21:00 Bangkok
    }

    #[test]
    fn test_timezone_utc_offset_bangkok() {
        let offset = timezone_utc_offset("Asia/Bangkok");
        assert_eq!(offset, "+07:00");
    }

    #[test]
    fn test_timezone_utc_offset_utc() {
        let offset = timezone_utc_offset("UTC");
        assert_eq!(offset, "+00:00");
    }

    #[test]
    fn test_timezone_utc_offset_invalid() {
        let offset = timezone_utc_offset("Invalid/TZ");
        assert_eq!(offset, "+00:00");
    }
}
