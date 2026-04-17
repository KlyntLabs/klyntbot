//! Shared date/time parsing and formatting utilities.
//!
//! Single source of truth for all date parsing across the codebase.
//! Non-timezone strings are interpreted in the given fallback timezone.

use chrono::{DateTime, Utc};
use jiff::{
    civil::{self, Weekday},
    tz::TimeZone,
    Span, Timestamp, Zoned,
};

use crate::time::bridge::jiff_to_chrono;

/// Parse a date/datetime string with timezone awareness (Jiff-native).
///
/// Prefer this over [`parse_datetime`] in new code. The legacy
/// [`parse_datetime`] function returns a Chrono type and is retained
/// for callers that haven't migrated yet; it will be removed in the
/// final cleanup of the chrono→jiff migration.
pub fn parse_datetime_jiff(s: &str, fallback_tz: &str) -> Option<Timestamp> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    // 1. RFC 3339 / 9557 with timezone (most specific)
    if let Ok(ts) = s.parse::<Timestamp>() {
        return Some(ts);
    }
    if let Ok(z) = s.parse::<Zoned>() {
        return Some(z.timestamp());
    }

    // Resolve fallback timezone once (falls back to UTC if invalid)
    let tz = TimeZone::get(fallback_tz).unwrap_or(TimeZone::UTC);

    // Helper: interpret a civil::DateTime in the fallback timezone
    let to_utc = |naive: civil::DateTime| -> Option<Timestamp> {
        naive
            .to_zoned(tz.clone())
            .ok()
            .map(|z: Zoned| z.timestamp())
    };

    // 2. ISO datetime without timezone: "2026-02-17T21:00:00"
    if let Ok(naive) = civil::DateTime::strptime("%Y-%m-%dT%H:%M:%S", s) {
        return to_utc(naive);
    }

    // 3. "YYYY-MM-DD HH:MM:SS"
    if let Ok(naive) = civil::DateTime::strptime("%Y-%m-%d %H:%M:%S", s) {
        return to_utc(naive);
    }

    // 4. "YYYY-MM-DD HH:MM"
    if let Ok(naive) = civil::DateTime::strptime("%Y-%m-%d %H:%M", s) {
        return to_utc(naive);
    }

    // 5. Date only: "2026-02-17" → midnight in fallback timezone
    if let Ok(date) = civil::Date::strptime("%Y-%m-%d", s) {
        return to_utc(date.at(0, 0, 0, 0));
    }

    // 6. Natural language relative dates
    parse_relative_date(s, &tz)
}

/// Legacy Chrono-returning wrapper around [`parse_datetime_jiff`].
///
/// Retained for callers that haven't migrated. New code should use
/// [`parse_datetime_jiff`] which returns the canonical `jiff::Timestamp`.
pub fn parse_datetime(s: &str, fallback_tz: &str) -> Option<DateTime<Utc>> {
    parse_datetime_jiff(s, fallback_tz).map(jiff_to_chrono)
}

/// Parse natural language relative date expressions.
///
/// Supported patterns:
/// - "today", "tomorrow", "yesterday"
/// - "next monday" through "next sunday"
/// - "in N day(s)", "in N week(s)"
fn parse_relative_date(s: &str, tz: &TimeZone) -> Option<Timestamp> {
    let lower = s.to_lowercase();

    // Get "today" in the user's local timezone
    let today_local: civil::Date = Timestamp::now().to_zoned(tz.clone()).date();

    let target_date = match lower.as_str() {
        "today" => Some(today_local),
        "tomorrow" => today_local.checked_add(Span::new().days(1)).ok(),
        "yesterday" => today_local.checked_sub(Span::new().days(1)).ok(),
        _ => None,
    };

    if let Some(date) = target_date {
        return civil_date_to_utc(date, tz);
    }

    // "next <weekday>"
    if let Some(day_str) = lower.strip_prefix("next ") {
        if let Some(target_weekday) = parse_weekday(day_str.trim()) {
            let current_weekday = today_local.weekday();
            let days_ahead = days_until_weekday(current_weekday, target_weekday);
            let date = today_local.checked_add(Span::new().days(days_ahead)).ok()?;
            return civil_date_to_utc(date, tz);
        }
    }

    // "in N day(s)" or "in N week(s)"
    if let Some(rest) = lower.strip_prefix("in ") {
        let parts: Vec<&str> = rest.trim().splitn(2, ' ').collect();
        if parts.len() == 2 {
            if let Ok(n) = parts[0].parse::<i64>() {
                let unit = parts[1].trim_end_matches('s');
                let delta = match unit {
                    "day" => Some(Span::new().days(n)),
                    "week" => Some(Span::new().weeks(n)),
                    _ => None,
                };
                if let Some(delta) = delta {
                    let date = today_local.checked_add(delta).ok()?;
                    return civil_date_to_utc(date, tz);
                }
            }
        }
    }

    None
}

/// Convert a civil::Date (midnight) to UTC Timestamp using the given timezone.
fn civil_date_to_utc(date: civil::Date, tz: &TimeZone) -> Option<Timestamp> {
    date.at(0, 0, 0, 0)
        .to_zoned(tz.clone())
        .ok()
        .map(|z| z.timestamp())
}

/// Parse a weekday name to jiff::civil::Weekday.
fn parse_weekday(s: &str) -> Option<Weekday> {
    match s {
        "monday" | "mon" => Some(Weekday::Monday),
        "tuesday" | "tue" | "tues" => Some(Weekday::Tuesday),
        "wednesday" | "wed" => Some(Weekday::Wednesday),
        "thursday" | "thu" | "thur" | "thurs" => Some(Weekday::Thursday),
        "friday" | "fri" => Some(Weekday::Friday),
        "saturday" | "sat" => Some(Weekday::Saturday),
        "sunday" | "sun" => Some(Weekday::Sunday),
        _ => None,
    }
}

/// Calculate days from `current` weekday until `target` weekday (always forward, 1-7 days).
fn days_until_weekday(current: Weekday, target: Weekday) -> i64 {
    // Jiff's Weekday::to_monday_zero_offset() returns 0-6 with Monday = 0
    let current_num = current.to_monday_zero_offset() as i64;
    let target_num = target.to_monday_zero_offset() as i64;
    let diff = target_num - current_num;
    if diff <= 0 {
        diff + 7
    } else {
        diff
    }
}

/// Get the UTC offset string for a timezone (e.g., "+07:00", "-05:00").
///
/// Uses the current time to determine the offset (accounts for DST).
pub fn timezone_utc_offset(timezone: &str) -> String {
    match TimeZone::get(timezone) {
        Ok(tz) => {
            let zoned = Timestamp::now().to_zoned(tz);
            let offset_secs = zoned.offset().seconds();
            let sign = if offset_secs >= 0 { '+' } else { '-' };
            let abs_secs = offset_secs.unsigned_abs();
            let hours = abs_secs / 3600;
            let minutes = (abs_secs % 3600) / 60;
            format!("{sign}{hours:02}:{minutes:02}")
        }
        Err(_) => "+00:00".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hour_of(ts: Timestamp) -> i8 {
        ts.to_zoned(TimeZone::UTC).hour()
    }
    fn day_of(ts: Timestamp) -> i8 {
        ts.to_zoned(TimeZone::UTC).day()
    }
    fn date_of(ts: Timestamp) -> civil::Date {
        ts.to_zoned(TimeZone::UTC).date()
    }
    fn weekday_of(ts: Timestamp) -> Weekday {
        ts.to_zoned(TimeZone::UTC).weekday()
    }

    #[test]
    fn test_parse_rfc3339_with_offset() {
        let dt = parse_datetime_jiff("2026-02-17T21:00:00+07:00", "UTC").unwrap();
        // 21:00 +07:00 = 14:00 UTC
        assert_eq!(hour_of(dt), 14);
        assert_eq!(day_of(dt), 17);
    }

    #[test]
    fn test_parse_rfc3339_utc() {
        let dt = parse_datetime_jiff("2026-02-17T21:00:00Z", "Asia/Bangkok").unwrap();
        assert_eq!(hour_of(dt), 21);
        assert_eq!(day_of(dt), 17);
    }

    #[test]
    fn test_parse_iso_datetime_no_tz_with_fallback() {
        // "2026-02-17T21:00:00" interpreted in Asia/Bangkok (UTC+7) = 14:00 UTC
        let dt = parse_datetime_jiff("2026-02-17T21:00:00", "Asia/Bangkok").unwrap();
        assert_eq!(hour_of(dt), 14);
        assert_eq!(day_of(dt), 17);
    }

    #[test]
    fn test_parse_date_only_in_timezone() {
        // "2026-02-17" midnight Bangkok (UTC+7) = 2026-02-16T17:00:00Z
        let dt = parse_datetime_jiff("2026-02-17", "Asia/Bangkok").unwrap();
        assert_eq!(hour_of(dt), 17);
        assert_eq!(day_of(dt), 16);
    }

    #[test]
    fn test_parse_date_with_time_space_format() {
        // "2026-02-17 21:00" in Bangkok = 14:00 UTC
        let dt = parse_datetime_jiff("2026-02-17 21:00", "Asia/Bangkok").unwrap();
        assert_eq!(hour_of(dt), 14);
        assert_eq!(day_of(dt), 17);
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
        let dt = parse_datetime_jiff("2026-02-17T21:00:00", "Invalid/TZ").unwrap();
        assert_eq!(hour_of(dt), 21);
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

    #[test]
    fn test_parse_rfc9557_with_iana_bracket() {
        // RFC 9557 extension — IANA bracket after RFC 3339
        let ts = parse_datetime_jiff("2024-07-21T17:11:00-04:00[America/New_York]", "UTC").unwrap();
        // 17:11 EDT = 21:11 UTC
        assert_eq!(hour_of(ts), 21);
        assert_eq!(day_of(ts), 21);
    }

    // --- Natural language relative date tests ---

    #[test]
    fn test_parse_today() {
        let dt = parse_datetime_jiff("today", "UTC").unwrap();
        let today = Timestamp::now().to_zoned(TimeZone::UTC).date();
        assert_eq!(date_of(dt), today);
        assert_eq!(hour_of(dt), 0); // midnight
    }

    #[test]
    fn test_parse_tomorrow() {
        let dt = parse_datetime_jiff("tomorrow", "UTC").unwrap();
        let tomorrow = Timestamp::now()
            .to_zoned(TimeZone::UTC)
            .date()
            .checked_add(Span::new().days(1))
            .unwrap();
        assert_eq!(date_of(dt), tomorrow);
    }

    #[test]
    fn test_parse_yesterday() {
        let dt = parse_datetime_jiff("yesterday", "UTC").unwrap();
        let yesterday = Timestamp::now()
            .to_zoned(TimeZone::UTC)
            .date()
            .checked_sub(Span::new().days(1))
            .unwrap();
        assert_eq!(date_of(dt), yesterday);
    }

    #[test]
    fn test_parse_tomorrow_case_insensitive() {
        let dt = parse_datetime_jiff("Tomorrow", "UTC").unwrap();
        let tomorrow = Timestamp::now()
            .to_zoned(TimeZone::UTC)
            .date()
            .checked_add(Span::new().days(1))
            .unwrap();
        assert_eq!(date_of(dt), tomorrow);
    }

    #[test]
    fn test_parse_next_weekday() {
        let dt = parse_datetime_jiff("next friday", "UTC").unwrap();
        assert_eq!(weekday_of(dt), Weekday::Friday);
        assert!(dt > Timestamp::now());
    }

    #[test]
    fn test_parse_next_weekday_abbreviated() {
        let dt = parse_datetime_jiff("next mon", "UTC").unwrap();
        assert_eq!(weekday_of(dt), Weekday::Monday);
        assert!(dt > Timestamp::now());
    }

    #[test]
    fn test_parse_in_n_days() {
        let dt = parse_datetime_jiff("in 3 days", "UTC").unwrap();
        let expected = Timestamp::now()
            .to_zoned(TimeZone::UTC)
            .date()
            .checked_add(Span::new().days(3))
            .unwrap();
        assert_eq!(date_of(dt), expected);
    }

    #[test]
    fn test_parse_in_1_day() {
        let dt = parse_datetime_jiff("in 1 day", "UTC").unwrap();
        let expected = Timestamp::now()
            .to_zoned(TimeZone::UTC)
            .date()
            .checked_add(Span::new().days(1))
            .unwrap();
        assert_eq!(date_of(dt), expected);
    }

    #[test]
    fn test_parse_in_n_weeks() {
        let dt = parse_datetime_jiff("in 2 weeks", "UTC").unwrap();
        let expected = Timestamp::now()
            .to_zoned(TimeZone::UTC)
            .date()
            .checked_add(Span::new().weeks(2))
            .unwrap();
        assert_eq!(date_of(dt), expected);
    }

    #[test]
    fn test_parse_in_1_week() {
        let dt = parse_datetime_jiff("in 1 week", "UTC").unwrap();
        let expected = Timestamp::now()
            .to_zoned(TimeZone::UTC)
            .date()
            .checked_add(Span::new().weeks(1))
            .unwrap();
        assert_eq!(date_of(dt), expected);
    }

    #[test]
    fn test_parse_natural_date_with_timezone() {
        let dt = parse_datetime_jiff("tomorrow", "Asia/Bangkok").unwrap();
        let tz = TimeZone::get("Asia/Bangkok").unwrap();
        let tomorrow_bangkok = Timestamp::now()
            .to_zoned(tz.clone())
            .date()
            .checked_add(Span::new().days(1))
            .unwrap();
        let expected_midnight = tomorrow_bangkok
            .at(0, 0, 0, 0)
            .to_zoned(tz)
            .unwrap()
            .timestamp();
        assert_eq!(dt, expected_midnight);
    }

    #[test]
    fn test_parse_natural_date_invalid() {
        assert!(parse_datetime("next someday", "UTC").is_none());
        assert!(parse_datetime("in zero days", "UTC").is_none());
        assert!(parse_datetime("in 3 months", "UTC").is_none());
    }

    #[test]
    fn test_days_until_weekday_same_day() {
        // Same day should be 7 (next week)
        assert_eq!(days_until_weekday(Weekday::Monday, Weekday::Monday), 7);
    }

    #[test]
    fn test_days_until_weekday_forward() {
        // Mon -> Fri = 4 days
        assert_eq!(days_until_weekday(Weekday::Monday, Weekday::Friday), 4);
        // Fri -> Mon = 3 days
        assert_eq!(days_until_weekday(Weekday::Friday, Weekday::Monday), 3);
    }

    #[test]
    fn test_parse_weekday_variants() {
        assert_eq!(parse_weekday("monday"), Some(Weekday::Monday));
        assert_eq!(parse_weekday("tue"), Some(Weekday::Tuesday));
        assert_eq!(parse_weekday("tues"), Some(Weekday::Tuesday));
        assert_eq!(parse_weekday("wed"), Some(Weekday::Wednesday));
        assert_eq!(parse_weekday("thur"), Some(Weekday::Thursday));
        assert_eq!(parse_weekday("thurs"), Some(Weekday::Thursday));
        assert_eq!(parse_weekday("fri"), Some(Weekday::Friday));
        assert_eq!(parse_weekday("sat"), Some(Weekday::Saturday));
        assert_eq!(parse_weekday("sun"), Some(Weekday::Sunday));
        assert_eq!(parse_weekday("invalid"), None);
    }
}
