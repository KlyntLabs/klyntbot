//! Shared date/time parsing and formatting utilities.
//!
//! Single source of truth for all date parsing across the codebase.
//! Non-timezone strings are interpreted in the given fallback timezone.

use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveDateTime, TimeZone, Utc, Weekday};
use chrono_tz::Tz;

/// Parse a date/datetime string with timezone awareness.
///
/// Accepts RFC3339, ISO datetime, date-only, "YYYY-MM-DD HH:MM" formats,
/// and natural language relative dates ("today", "tomorrow", "yesterday",
/// "next Monday", "in 3 days", "in 2 weeks").
///
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

    // 6. Natural language relative dates
    if let Some(date) = parse_relative_date(s, tz) {
        return Some(date);
    }

    None
}

/// Parse natural language relative date expressions.
///
/// Supported patterns:
/// - "today", "tomorrow", "yesterday"
/// - "next monday" through "next sunday"
/// - "in N day(s)", "in N week(s)"
fn parse_relative_date(s: &str, tz: Option<Tz>) -> Option<DateTime<Utc>> {
    let lower = s.to_lowercase();

    // Get "today" in the user's local timezone
    let today_local = if let Some(tz) = tz {
        Utc::now().with_timezone(&tz).date_naive()
    } else {
        Utc::now().date_naive()
    };

    let target_date = match lower.as_str() {
        "today" => Some(today_local),
        "tomorrow" => Some(today_local + Duration::days(1)),
        "yesterday" => Some(today_local - Duration::days(1)),
        _ => None,
    };

    if let Some(date) = target_date {
        return naive_date_to_utc(date, tz);
    }

    // "next <weekday>"
    if let Some(day_str) = lower.strip_prefix("next ") {
        if let Some(target_weekday) = parse_weekday(day_str.trim()) {
            let current_weekday = today_local.weekday();
            let days_ahead = days_until_weekday(current_weekday, target_weekday);
            let date = today_local + Duration::days(days_ahead);
            return naive_date_to_utc(date, tz);
        }
    }

    // "in N day(s)" or "in N week(s)"
    if let Some(rest) = lower.strip_prefix("in ") {
        let parts: Vec<&str> = rest.trim().splitn(2, ' ').collect();
        if parts.len() == 2 {
            if let Ok(n) = parts[0].parse::<i64>() {
                let unit = parts[1].trim_end_matches('s');
                let delta = match unit {
                    "day" => Some(Duration::days(n)),
                    "week" => Some(Duration::weeks(n)),
                    _ => None,
                };
                if let Some(delta) = delta {
                    let date = today_local + delta;
                    return naive_date_to_utc(date, tz);
                }
            }
        }
    }

    None
}

/// Convert a NaiveDate (midnight) to UTC using the given timezone.
fn naive_date_to_utc(date: NaiveDate, tz: Option<Tz>) -> Option<DateTime<Utc>> {
    let midnight = date.and_hms_opt(0, 0, 0)?;
    if let Some(tz) = tz {
        tz.from_local_datetime(&midnight)
            .earliest()
            .map(|dt| dt.with_timezone(&Utc))
    } else {
        Some(midnight.and_utc())
    }
}

/// Parse a weekday name to chrono::Weekday.
fn parse_weekday(s: &str) -> Option<Weekday> {
    match s {
        "monday" | "mon" => Some(Weekday::Mon),
        "tuesday" | "tue" | "tues" => Some(Weekday::Tue),
        "wednesday" | "wed" => Some(Weekday::Wed),
        "thursday" | "thu" | "thur" | "thurs" => Some(Weekday::Thu),
        "friday" | "fri" => Some(Weekday::Fri),
        "saturday" | "sat" => Some(Weekday::Sat),
        "sunday" | "sun" => Some(Weekday::Sun),
        _ => None,
    }
}

/// Calculate days from `current` weekday until `target` weekday (always forward, 1-7 days).
fn days_until_weekday(current: Weekday, target: Weekday) -> i64 {
    let current_num = current.num_days_from_monday() as i64;
    let target_num = target.num_days_from_monday() as i64;
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
    use chrono::Timelike;

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

    // --- Natural language relative date tests ---

    #[test]
    fn test_parse_today() {
        let dt = parse_datetime("today", "UTC").unwrap();
        let today = Utc::now().date_naive();
        assert_eq!(dt.date_naive(), today);
        assert_eq!(dt.hour(), 0); // midnight
    }

    #[test]
    fn test_parse_tomorrow() {
        let dt = parse_datetime("tomorrow", "UTC").unwrap();
        let tomorrow = Utc::now().date_naive() + Duration::days(1);
        assert_eq!(dt.date_naive(), tomorrow);
    }

    #[test]
    fn test_parse_yesterday() {
        let dt = parse_datetime("yesterday", "UTC").unwrap();
        let yesterday = Utc::now().date_naive() - Duration::days(1);
        assert_eq!(dt.date_naive(), yesterday);
    }

    #[test]
    fn test_parse_tomorrow_case_insensitive() {
        let dt = parse_datetime("Tomorrow", "UTC").unwrap();
        let tomorrow = Utc::now().date_naive() + Duration::days(1);
        assert_eq!(dt.date_naive(), tomorrow);
    }

    #[test]
    fn test_parse_next_weekday() {
        let dt = parse_datetime("next friday", "UTC").unwrap();
        assert_eq!(dt.weekday(), Weekday::Fri);
        // Must be in the future
        assert!(dt > Utc::now());
    }

    #[test]
    fn test_parse_next_weekday_abbreviated() {
        let dt = parse_datetime("next mon", "UTC").unwrap();
        assert_eq!(dt.weekday(), Weekday::Mon);
        assert!(dt > Utc::now());
    }

    #[test]
    fn test_parse_in_n_days() {
        let dt = parse_datetime("in 3 days", "UTC").unwrap();
        let expected = Utc::now().date_naive() + Duration::days(3);
        assert_eq!(dt.date_naive(), expected);
    }

    #[test]
    fn test_parse_in_1_day() {
        let dt = parse_datetime("in 1 day", "UTC").unwrap();
        let expected = Utc::now().date_naive() + Duration::days(1);
        assert_eq!(dt.date_naive(), expected);
    }

    #[test]
    fn test_parse_in_n_weeks() {
        let dt = parse_datetime("in 2 weeks", "UTC").unwrap();
        let expected = Utc::now().date_naive() + Duration::weeks(2);
        assert_eq!(dt.date_naive(), expected);
    }

    #[test]
    fn test_parse_in_1_week() {
        let dt = parse_datetime("in 1 week", "UTC").unwrap();
        let expected = Utc::now().date_naive() + Duration::weeks(1);
        assert_eq!(dt.date_naive(), expected);
    }

    #[test]
    fn test_parse_natural_date_with_timezone() {
        // "tomorrow" in Bangkok should be midnight Bangkok time converted to UTC
        let dt = parse_datetime("tomorrow", "Asia/Bangkok").unwrap();
        let tz: Tz = "Asia/Bangkok".parse().unwrap();
        let tomorrow_bangkok = (Utc::now().with_timezone(&tz).date_naive()) + Duration::days(1);
        let expected_midnight = tz
            .from_local_datetime(&tomorrow_bangkok.and_hms_opt(0, 0, 0).unwrap())
            .earliest()
            .unwrap()
            .with_timezone(&Utc);
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
        assert_eq!(days_until_weekday(Weekday::Mon, Weekday::Mon), 7);
    }

    #[test]
    fn test_days_until_weekday_forward() {
        // Mon -> Fri = 4 days
        assert_eq!(days_until_weekday(Weekday::Mon, Weekday::Fri), 4);
        // Fri -> Mon = 3 days
        assert_eq!(days_until_weekday(Weekday::Fri, Weekday::Mon), 3);
    }

    #[test]
    fn test_parse_weekday_variants() {
        assert_eq!(parse_weekday("monday"), Some(Weekday::Mon));
        assert_eq!(parse_weekday("tue"), Some(Weekday::Tue));
        assert_eq!(parse_weekday("tues"), Some(Weekday::Tue));
        assert_eq!(parse_weekday("wed"), Some(Weekday::Wed));
        assert_eq!(parse_weekday("thur"), Some(Weekday::Thu));
        assert_eq!(parse_weekday("thurs"), Some(Weekday::Thu));
        assert_eq!(parse_weekday("fri"), Some(Weekday::Fri));
        assert_eq!(parse_weekday("sat"), Some(Weekday::Sat));
        assert_eq!(parse_weekday("sun"), Some(Weekday::Sun));
        assert_eq!(parse_weekday("invalid"), None);
    }
}
