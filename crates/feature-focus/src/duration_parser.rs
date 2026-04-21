//! Parse user-entered duration strings into end timestamps.
//!
//! # Grammar
//!
//! ```text
//! <n>(s|m|h|d)        e.g. "30m", "2h", "1d", "90s"
//! until <clock>       e.g. "until 5pm", "until 17:30", "until noon", "until midnight"
//! until tomorrow      → tomorrow TOMORROW_HOUR:00 local
//! until next meeting  → end of next calendar event (requires calendar fn)
//! ```

use jiff::{civil, Timestamp, Zoned};
use thiserror::Error;

const TOMORROW_HOUR: i8 = 9;

#[derive(Debug, Error, PartialEq)]
pub enum ParseError {
    #[error("no upcoming meeting found")]
    NoMeeting,
    #[error("unrecognised duration syntax: {0:?}")]
    Grammar(String),
}

/// Parse a user-entered duration string into an end timestamp.
///
/// - `input`            — the raw string from the user/chip.
/// - `now`              — reference point (use `Timestamp::now()` in production).
/// - `tz`               — local timezone for clock-based forms.
/// - `next_meeting_end` — called only for "until next meeting"; returns `None` if no meeting.
pub fn parse_until(
    input: &str,
    now: Timestamp,
    tz: &jiff::tz::TimeZone,
    next_meeting_end: impl FnOnce() -> Option<Timestamp>,
) -> Result<Timestamp, ParseError> {
    let trimmed = input.trim();

    // ── Simple duration: \d+[smhd] ──────────────────────────────────────────
    if let Some(ts) = try_parse_simple_duration(trimmed, now) {
        return Ok(ts);
    }

    // ── "until …" forms ─────────────────────────────────────────────────────
    let after_until = trimmed
        .strip_prefix("until ")
        .or_else(|| trimmed.strip_prefix("Until "));

    if let Some(rest) = after_until {
        let rest = rest.trim();

        if rest.eq_ignore_ascii_case("tomorrow") {
            return Ok(tomorrow_at(now, tz, TOMORROW_HOUR));
        }

        if rest.eq_ignore_ascii_case("next meeting") {
            return next_meeting_end().ok_or(ParseError::NoMeeting);
        }

        if let Some(ts) = try_parse_clock(rest, now, tz) {
            return Ok(ts);
        }
    }

    Err(ParseError::Grammar(input.to_owned()))
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn try_parse_simple_duration(s: &str, now: Timestamp) -> Option<Timestamp> {
    let (num_str, unit) = s.split_at(s.len().checked_sub(1)?);
    let n: i64 = num_str.parse().ok()?;
    if n <= 0 {
        return None;
    }
    let secs = match unit {
        "s" => n,
        "m" => n * 60,
        "h" => n * 3600,
        "d" => n * 86_400,
        _ => return None,
    };
    now.checked_add(jiff::Span::new().seconds(secs)).ok()
}

/// Parse clock strings: "5pm", "17:30", "noon", "midnight".
/// If the resulting time is ≤ now local → next day.
fn try_parse_clock(s: &str, now: Timestamp, tz: &jiff::tz::TimeZone) -> Option<Timestamp> {
    let civil_time = parse_civil_time(s)?;
    let now_zoned: Zoned = now.to_zoned(tz.clone());
    let today = now_zoned.date();

    let candidate = today
        .at(civil_time.hour(), civil_time.minute(), 0, 0)
        .to_zoned(tz.clone())
        .ok()?;

    if candidate.timestamp() > now {
        Some(candidate.timestamp())
    } else {
        // Roll over to next day.
        let tomorrow = today
            .checked_add(jiff::Span::new().days(1))
            .ok()?
            .at(civil_time.hour(), civil_time.minute(), 0, 0)
            .to_zoned(tz.clone())
            .ok()?;
        Some(tomorrow.timestamp())
    }
}

fn parse_civil_time(s: &str) -> Option<civil::Time> {
    if s.eq_ignore_ascii_case("noon") {
        return Some(civil::time(12, 0, 0, 0));
    }
    if s.eq_ignore_ascii_case("midnight") {
        return Some(civil::time(0, 0, 0, 0));
    }

    // "5pm" / "11am"
    let lower = s.to_ascii_lowercase();
    if let Some(h_str) = lower.strip_suffix("pm") {
        let h: i8 = h_str.trim().parse().ok()?;
        let h24 = if h == 12 { 12i8 } else { h + 12 };
        return Some(civil::time(h24, 0, 0, 0));
    }
    if let Some(h_str) = lower.strip_suffix("am") {
        let h: i8 = h_str.trim().parse().ok()?;
        let h24 = if h == 12 { 0i8 } else { h };
        return Some(civil::time(h24, 0, 0, 0));
    }

    // "17:30" / "9:05"
    let (h_str, m_str) = s.split_once(':')?;
    let h: i8 = h_str.trim().parse().ok()?;
    let m: i8 = m_str.trim().parse().ok()?;
    Some(civil::time(h, m, 0, 0))
}

fn tomorrow_at(now: Timestamp, tz: &jiff::tz::TimeZone, hour: i8) -> Timestamp {
    let now_zoned: Zoned = now.to_zoned(tz.clone());
    let tomorrow = now_zoned
        .date()
        .checked_add(jiff::Span::new().days(1))
        .expect("date overflow")
        .at(hour, 0, 0, 0)
        .to_zoned(tz.clone())
        .expect("invalid zoned datetime");
    tomorrow.timestamp()
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::tz::TimeZone;

    /// Fixed UTC+0 timezone for deterministic tests.
    fn utc() -> TimeZone {
        TimeZone::UTC
    }

    /// Timestamp at a specific UTC second offset from epoch.
    fn ts(secs: i64) -> Timestamp {
        Timestamp::from_second(secs).unwrap()
    }

    /// 2024-01-15 13:00:00 UTC (1pm)
    const BASE_SECS: i64 = 1705323600;

    fn no_meeting() -> Option<Timestamp> {
        None
    }

    // ── Simple durations ─────────────────────────────────────────────────────

    #[test]
    fn parse_30m() {
        let now = ts(BASE_SECS);
        let result = parse_until("30m", now, &utc(), no_meeting).unwrap();
        assert_eq!(result, ts(BASE_SECS + 30 * 60));
    }

    #[test]
    fn parse_2h() {
        let now = ts(BASE_SECS);
        let result = parse_until("2h", now, &utc(), no_meeting).unwrap();
        assert_eq!(result, ts(BASE_SECS + 2 * 3600));
    }

    #[test]
    fn parse_1d() {
        let now = ts(BASE_SECS);
        let result = parse_until("1d", now, &utc(), no_meeting).unwrap();
        assert_eq!(result, ts(BASE_SECS + 86_400));
    }

    #[test]
    fn parse_90s() {
        let now = ts(BASE_SECS);
        let result = parse_until("90s", now, &utc(), no_meeting).unwrap();
        assert_eq!(result, ts(BASE_SECS + 90));
    }

    // ── until <clock> ─────────────────────────────────────────────────────────

    /// 1pm UTC, "until 5pm" → today 17:00 UTC (+4h)
    #[test]
    fn until_5pm_before_5pm() {
        let now = ts(BASE_SECS); // 13:00 UTC
        let result = parse_until("until 5pm", now, &utc(), no_meeting).unwrap();
        let expected = ts(BASE_SECS + 4 * 3600); // 17:00 UTC
        assert_eq!(result, expected);
    }

    /// 6pm UTC, "until 5pm" → tomorrow 17:00 UTC
    #[test]
    fn until_5pm_after_5pm_rolls_to_next_day() {
        let now = ts(BASE_SECS + 5 * 3600); // 18:00 UTC
        let result = parse_until("until 5pm", now, &utc(), no_meeting).unwrap();
        // 2024-01-16 17:00 UTC
        let expected = ts(BASE_SECS + 5 * 3600 + 23 * 3600);
        assert_eq!(result, expected);
    }

    /// 1pm UTC, "until 17:30" → today 17:30 UTC (+4.5h)
    #[test]
    fn until_17_30() {
        let now = ts(BASE_SECS); // 13:00 UTC
        let result = parse_until("until 17:30", now, &utc(), no_meeting).unwrap();
        let expected = ts(BASE_SECS + 4 * 3600 + 30 * 60); // 17:30 UTC
        assert_eq!(result, expected);
    }

    #[test]
    fn until_noon_before_noon() {
        // 10:00 UTC, "until noon" → today 12:00 UTC
        let now = ts(BASE_SECS - 3 * 3600); // 10:00 UTC
        let result = parse_until("until noon", now, &utc(), no_meeting).unwrap();
        let expected = ts(BASE_SECS - 3 * 3600 + 2 * 3600); // +2h to noon
        assert_eq!(result, expected);
    }

    #[test]
    fn until_midnight() {
        // 13:00 UTC, "until midnight" → next midnight (2024-01-16 00:00 UTC)
        let now = ts(BASE_SECS);
        let result = parse_until("until midnight", now, &utc(), no_meeting).unwrap();
        // midnight = 00:00 < now (13:00), so rolls to next day
        let expected = ts(BASE_SECS + 11 * 3600); // +11h to midnight
        assert_eq!(result, expected);
    }

    // ── until tomorrow ────────────────────────────────────────────────────────

    #[test]
    fn until_tomorrow() {
        let now = ts(BASE_SECS); // 2024-01-15 13:00 UTC
        let result = parse_until("until tomorrow", now, &utc(), no_meeting).unwrap();
        // 2024-01-16 09:00 UTC = BASE_SECS + 20h
        let expected = ts(BASE_SECS + 20 * 3600);
        assert_eq!(result, expected);
    }

    // ── until next meeting ────────────────────────────────────────────────────

    #[test]
    fn until_next_meeting_found() {
        let now = ts(BASE_SECS);
        let meeting_end = ts(BASE_SECS + 5400);
        let result = parse_until("until next meeting", now, &utc(), || Some(meeting_end)).unwrap();
        assert_eq!(result, meeting_end);
    }

    #[test]
    fn until_next_meeting_none() {
        let now = ts(BASE_SECS);
        let result = parse_until("until next meeting", now, &utc(), no_meeting);
        assert_eq!(result, Err(ParseError::NoMeeting));
    }

    // ── Grammar errors ────────────────────────────────────────────────────────

    #[test]
    fn invalid_input_returns_grammar_error() {
        let now = ts(BASE_SECS);
        let result = parse_until("next week", now, &utc(), no_meeting);
        assert!(matches!(result, Err(ParseError::Grammar(_))));
    }

    #[test]
    fn empty_input_returns_grammar_error() {
        let now = ts(BASE_SECS);
        let result = parse_until("", now, &utc(), no_meeting);
        assert!(matches!(result, Err(ParseError::Grammar(_))));
    }

    #[test]
    fn zero_duration_returns_grammar_error() {
        let now = ts(BASE_SECS);
        let result = parse_until("0m", now, &utc(), no_meeting);
        assert!(matches!(result, Err(ParseError::Grammar(_))));
    }
}
