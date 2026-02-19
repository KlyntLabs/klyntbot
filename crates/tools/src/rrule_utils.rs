//! RRULE utilities for recurring task support.
//!
//! Wraps the `rrule` crate to provide a simplified API for recurrence rules.
//! Supports: FREQ (DAILY/WEEKLY/MONTHLY/YEARLY), INTERVAL, BYDAY, BYHOUR,
//! BYMINUTE, BYMONTHDAY, COUNT, UNTIL, EXDATE.

use chrono::{DateTime, NaiveDateTime, Utc};
use common::Result;
use rrule::RRuleSet;
use std::collections::HashMap;

/// Unsupported RRULE parameters.
const UNSUPPORTED_PARAMS: &[&str] = &["BYSETPOS", "WKST", "EXRULE", "RDATE"];

/// Recurrence frequency.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frequency {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

impl Frequency {
    fn from_str(s: &str) -> Option<Self> {
        match s {
            "DAILY" => Some(Self::Daily),
            "WEEKLY" => Some(Self::Weekly),
            "MONTHLY" => Some(Self::Monthly),
            "YEARLY" => Some(Self::Yearly),
            _ => None,
        }
    }
}

impl std::fmt::Display for Frequency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Daily => write!(f, "DAILY"),
            Self::Weekly => write!(f, "WEEKLY"),
            Self::Monthly => write!(f, "MONTHLY"),
            Self::Yearly => write!(f, "YEARLY"),
        }
    }
}

/// Parsed RRULE with support for COUNT, UNTIL, and EXDATE.
#[derive(Debug, Clone)]
pub struct RRule {
    pub freq: Frequency,
    pub interval: u32,
    pub byday: Vec<String>,
    pub byhour: Vec<u32>,
    pub byminute: Vec<u32>,
    pub bymonthday: Vec<u32>,
    pub count: Option<u32>,
    pub until: Option<DateTime<Utc>>,
    pub exdate: Vec<DateTime<Utc>>,
}

/// Parse a datetime string in `YYYYMMDDTHHMMSSZ` format.
fn parse_ical_datetime(s: &str) -> std::result::Result<DateTime<Utc>, String> {
    let s = s.trim();
    NaiveDateTime::parse_from_str(s, "%Y%m%dT%H%M%SZ")
        .map(|ndt| ndt.and_utc())
        .map_err(|e| format!("Invalid datetime '{}': {}", s, e))
}

/// Parse a comma-separated list of u32 values.
fn parse_u32_list(s: &str) -> Vec<u32> {
    s.split(',')
        .filter_map(|v| v.trim().parse().ok())
        .collect()
}

impl RRule {
    /// Parse an RRULE string into a structured representation.
    ///
    /// Accepts parameters: FREQ, INTERVAL, BYDAY, BYHOUR, BYMINUTE, BYMONTHDAY,
    /// COUNT, UNTIL, EXDATE. Rejects BYSETPOS, WKST, EXRULE, RDATE.
    pub fn parse(rule: &str) -> Result<Self> {
        let upper = rule.to_uppercase();

        // Check for unsupported parameters
        for param in UNSUPPORTED_PARAMS {
            if upper.contains(param) {
                return Err(common::ToolError::InvalidParams(format!(
                    "Unsupported RRULE parameter: {}",
                    param
                ))
                .into());
            }
        }

        let parts: HashMap<&str, &str> = upper
            .split(';')
            .filter_map(|p| {
                let mut kv = p.splitn(2, '=');
                Some((kv.next()?, kv.next()?))
            })
            .collect();

        let freq_str = parts.get("FREQ").ok_or_else(|| {
            common::ToolError::InvalidParams("FREQ is required".to_string())
        })?;

        let freq = Frequency::from_str(freq_str).ok_or_else(|| {
            common::ToolError::InvalidParams(format!("Invalid FREQ: {}", freq_str))
        })?;

        let interval = parts
            .get("INTERVAL")
            .and_then(|v| v.parse().ok())
            .unwrap_or(1);

        let byday = parts
            .get("BYDAY")
            .map(|v| v.split(',').map(|s| s.trim().to_string()).collect())
            .unwrap_or_default();

        let byhour = parts
            .get("BYHOUR")
            .map(|v| parse_u32_list(v))
            .unwrap_or_default();

        let byminute = parts
            .get("BYMINUTE")
            .map(|v| parse_u32_list(v))
            .unwrap_or_default();

        let bymonthday = parts
            .get("BYMONTHDAY")
            .map(|v| parse_u32_list(v))
            .unwrap_or_default();

        let count = parts.get("COUNT").and_then(|v| v.parse().ok());

        let until = parts
            .get("UNTIL")
            .map(|v| {
                parse_ical_datetime(v).map_err(|e| {
                    common::ToolError::InvalidParams(format!("Invalid UNTIL: {}", e))
                })
            })
            .transpose()?;

        let exdate = parts
            .get("EXDATE")
            .map(|v| {
                v.split(',')
                    .map(|d| {
                        parse_ical_datetime(d).map_err(|e| {
                            common::ToolError::InvalidParams(format!("Invalid EXDATE: {}", e))
                        })
                    })
                    .collect::<std::result::Result<Vec<_>, _>>()
            })
            .transpose()?
            .unwrap_or_default();

        Ok(Self {
            freq,
            interval,
            byday,
            byhour,
            byminute,
            bymonthday,
            count,
            until,
            exdate,
        })
    }

    /// Build the core RRULE string (without EXDATE, which is a separate iCal property).
    fn core_rule_string(&self) -> String {
        let mut parts = vec![format!("FREQ={}", self.freq)];

        if self.interval > 1 {
            parts.push(format!("INTERVAL={}", self.interval));
        }
        if !self.byday.is_empty() {
            parts.push(format!("BYDAY={}", self.byday.join(",")));
        }
        if !self.byhour.is_empty() {
            let vals: Vec<String> = self.byhour.iter().map(|v| v.to_string()).collect();
            parts.push(format!("BYHOUR={}", vals.join(",")));
        }
        if !self.byminute.is_empty() {
            let vals: Vec<String> = self.byminute.iter().map(|v| v.to_string()).collect();
            parts.push(format!("BYMINUTE={}", vals.join(",")));
        }
        if !self.bymonthday.is_empty() {
            let vals: Vec<String> = self.bymonthday.iter().map(|v| v.to_string()).collect();
            parts.push(format!("BYMONTHDAY={}", vals.join(",")));
        }
        if let Some(count) = self.count {
            parts.push(format!("COUNT={}", count));
        }
        if let Some(until) = self.until {
            parts.push(format!("UNTIL={}", until.format("%Y%m%dT%H%M%SZ")));
        }

        parts.join(";")
    }

    /// Generate the next occurrences from `from`, up to `max` results.
    ///
    /// Respects COUNT, UNTIL, and EXDATE constraints.
    pub fn next_occurrences(&self, from: DateTime<Utc>, max: usize) -> Result<Vec<DateTime<Utc>>> {
        let dtstart = format!("DTSTART:{}", from.format("%Y%m%dT%H%M%SZ"));
        let core = self.core_rule_string();
        let mut lines = vec![dtstart, format!("RRULE:{}", core)];

        // Add EXDATE as separate iCal lines
        for exd in &self.exdate {
            lines.push(format!("EXDATE:{}", exd.format("%Y%m%dT%H%M%SZ")));
        }

        let full = lines.join("\n");
        let rrule_set: RRuleSet = full
            .parse()
            .map_err(|e| common::ToolError::InvalidParams(format!("Invalid RRULE: {}", e)))?;

        // Generate enough candidates. The rrule crate respects COUNT/UNTIL natively.
        // EXDATE is also handled by the RRuleSet parser.
        // Request more than max to have room for filtering the DTSTART itself.
        let limit = max + 1;
        let result = rrule_set.all(limit as u16);
        let dates: Vec<DateTime<Utc>> = result
            .dates
            .into_iter()
            .map(|dt| dt.with_timezone(&Utc))
            .filter(|dt| *dt > from)
            .take(max)
            .collect();

        Ok(dates)
    }
}

/// Validate an RRULE string, rejecting unsupported parameters.
///
/// Supports: FREQ, INTERVAL, BYDAY, BYHOUR, BYMINUTE, BYMONTHDAY, COUNT, UNTIL, EXDATE.
pub fn validate_rrule(rule: &str) -> Result<()> {
    // Parse validates everything including unsupported param checks
    let parsed = RRule::parse(rule)?;

    // Also validate syntax via the rrule crate (without EXDATE in the RRULE line)
    let dtstart = "DTSTART:20200101T000000Z";
    let core = parsed.core_rule_string();
    let full = format!("{}\nRRULE:{}", dtstart, core);
    full.parse::<RRuleSet>()
        .map_err(|e| common::ToolError::InvalidParams(format!("Invalid RRULE syntax: {}", e)))?;

    Ok(())
}

/// Compute the next occurrence of an RRULE after a given datetime.
///
/// Prepends DTSTART if not present in the rule string, using `after` as the
/// start date. Returns None if no further occurrences.
pub fn next_occurrence(rule: &str, after: DateTime<Utc>) -> Result<Option<DateTime<Utc>>> {
    let parsed = RRule::parse(rule)?;
    let occurrences = parsed.next_occurrences(after, 1)?;
    Ok(occurrences.into_iter().next())
}

/// Check if a recurring task instance should be spawned.
///
/// Returns true if `next_instance_date` is Some and <= `now`.
pub fn should_spawn_instance(
    next_instance_date: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> bool {
    next_instance_date.is_some_and(|next| next <= now)
}

/// Convert an RRULE string to a human-readable description.
pub fn humanize_rrule(rule: &str) -> String {
    let upper = rule.to_uppercase();
    let parts: HashMap<&str, &str> = upper
        .split(';')
        .filter_map(|p| {
            let mut kv = p.splitn(2, '=');
            Some((kv.next()?, kv.next()?))
        })
        .collect();

    let freq = parts.get("FREQ").copied().unwrap_or("DAILY");
    let interval: u32 = parts
        .get("INTERVAL")
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);

    let freq_str = match (freq, interval) {
        ("DAILY", 1) => "Every day".to_string(),
        ("DAILY", n) => format!("Every {} days", n),
        ("WEEKLY", 1) => "Every week".to_string(),
        ("WEEKLY", n) => format!("Every {} weeks", n),
        ("MONTHLY", 1) => "Every month".to_string(),
        ("MONTHLY", n) => format!("Every {} months", n),
        ("YEARLY", 1) => "Every year".to_string(),
        ("YEARLY", n) => format!("Every {} years", n),
        _ => format!("Every {} {}", interval, freq.to_lowercase()),
    };

    let mut details = Vec::new();

    if let Some(byday) = parts.get("BYDAY") {
        let days: Vec<&str> = byday.split(',').collect();
        let day_names: Vec<&str> = days
            .iter()
            .map(|d| match *d {
                "MO" => "Monday",
                "TU" => "Tuesday",
                "WE" => "Wednesday",
                "TH" => "Thursday",
                "FR" => "Friday",
                "SA" => "Saturday",
                "SU" => "Sunday",
                _ => d,
            })
            .collect();
        details.push(format!("on {}", day_names.join(", ")));
    }

    if let Some(bymonthday) = parts.get("BYMONTHDAY") {
        details.push(format!("on day {}", bymonthday));
    }

    let time_parts: Vec<String> = [
        parts.get("BYHOUR").map(|h| format!("{}h", h)),
        parts.get("BYMINUTE").map(|m| format!("{}m", m)),
    ]
    .into_iter()
    .flatten()
    .collect();

    if !time_parts.is_empty() {
        details.push(format!("at {}", time_parts.join(":")));
    }

    if let Some(count) = parts.get("COUNT") {
        details.push(format!("{} times", count));
    }

    if let Some(until) = parts.get("UNTIL") {
        details.push(format!("until {}", until));
    }

    if details.is_empty() {
        freq_str
    } else {
        format!("{} {}", freq_str, details.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_validate_rrule_daily() {
        assert!(validate_rrule("FREQ=DAILY").is_ok());
    }

    #[test]
    fn test_validate_rrule_weekly_byday() {
        assert!(validate_rrule("FREQ=WEEKLY;BYDAY=MO,WE,FR").is_ok());
    }

    #[test]
    fn test_validate_rrule_monthly_bymonthday() {
        assert!(validate_rrule("FREQ=MONTHLY;BYMONTHDAY=1").is_ok());
    }

    #[test]
    fn test_validate_rrule_accepts_count() {
        assert!(validate_rrule("FREQ=DAILY;COUNT=10").is_ok());
    }

    #[test]
    fn test_validate_rrule_accepts_until() {
        assert!(validate_rrule("FREQ=DAILY;UNTIL=20250101T000000Z").is_ok());
    }

    #[test]
    fn test_validate_rrule_accepts_exdate() {
        assert!(validate_rrule("FREQ=DAILY;EXDATE=20250101T000000Z").is_ok());
    }

    #[test]
    fn test_validate_rrule_rejects_bysetpos() {
        let result = validate_rrule("FREQ=MONTHLY;BYDAY=MO;BYSETPOS=1");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_rrule_invalid_syntax() {
        let result = validate_rrule("NOT_A_VALID_RULE");
        assert!(result.is_err());
    }

    // -- RRule::parse tests --

    #[test]
    fn test_parse_basic() {
        let r = RRule::parse("FREQ=DAILY").unwrap();
        assert_eq!(r.freq, Frequency::Daily);
        assert_eq!(r.interval, 1);
        assert!(r.count.is_none());
        assert!(r.until.is_none());
        assert!(r.exdate.is_empty());
    }

    #[test]
    fn test_parse_with_count() {
        let r = RRule::parse("FREQ=DAILY;COUNT=5").unwrap();
        assert_eq!(r.count, Some(5));
    }

    #[test]
    fn test_parse_with_until() {
        let r = RRule::parse("FREQ=WEEKLY;UNTIL=20260301T000000Z").unwrap();
        let expected = Utc.with_ymd_and_hms(2026, 3, 1, 0, 0, 0).unwrap();
        assert_eq!(r.until, Some(expected));
    }

    #[test]
    fn test_parse_with_exdate() {
        let r = RRule::parse("FREQ=DAILY;EXDATE=20260220T000000Z,20260222T000000Z").unwrap();
        assert_eq!(r.exdate.len(), 2);
        assert_eq!(
            r.exdate[0],
            Utc.with_ymd_and_hms(2026, 2, 20, 0, 0, 0).unwrap()
        );
        assert_eq!(
            r.exdate[1],
            Utc.with_ymd_and_hms(2026, 2, 22, 0, 0, 0).unwrap()
        );
    }

    #[test]
    fn test_parse_rejects_bysetpos() {
        assert!(RRule::parse("FREQ=MONTHLY;BYSETPOS=1").is_err());
    }

    #[test]
    fn test_parse_rejects_missing_freq() {
        assert!(RRule::parse("INTERVAL=2").is_err());
    }

    // -- next_occurrences tests --

    #[test]
    fn test_count_produces_exact_occurrences() {
        let r = RRule::parse("FREQ=DAILY;COUNT=5").unwrap();
        let from = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let dates = r.next_occurrences(from, 10).unwrap();
        // COUNT=5 means 5 total occurrences including DTSTART.
        // Since we filter > from, we get 4 (the next 4 after the start).
        assert_eq!(dates.len(), 4);
    }

    #[test]
    fn test_until_stops_at_date() {
        let r = RRule::parse("FREQ=WEEKLY;UNTIL=20260301T000000Z").unwrap();
        let from = Utc.with_ymd_and_hms(2026, 2, 1, 0, 0, 0).unwrap();
        let dates = r.next_occurrences(from, 100).unwrap();
        let boundary = Utc.with_ymd_and_hms(2026, 3, 1, 0, 0, 0).unwrap();
        for dt in &dates {
            assert!(*dt <= boundary, "Date {} should be <= UNTIL", dt);
        }
        // Feb 1 is DTSTART (excluded), so: Feb 8, Feb 15, Feb 22, Mar 1 = 4 occurrences
        assert_eq!(dates.len(), 4);
    }

    #[test]
    fn test_exdate_skips_excluded_dates() {
        let r =
            RRule::parse("FREQ=DAILY;EXDATE=20260220T000000Z,20260222T000000Z").unwrap();
        let from = Utc.with_ymd_and_hms(2026, 2, 18, 0, 0, 0).unwrap();
        let dates = r.next_occurrences(from, 5).unwrap();
        let excluded1 = Utc.with_ymd_and_hms(2026, 2, 20, 0, 0, 0).unwrap();
        let excluded2 = Utc.with_ymd_and_hms(2026, 2, 22, 0, 0, 0).unwrap();
        for dt in &dates {
            assert_ne!(*dt, excluded1, "Should skip Feb 20");
            assert_ne!(*dt, excluded2, "Should skip Feb 22");
        }
        assert_eq!(dates.len(), 5);
    }

    #[test]
    fn test_count_with_exdate() {
        let r = RRule::parse("FREQ=DAILY;COUNT=10;EXDATE=20260220T000000Z").unwrap();
        let from = Utc.with_ymd_and_hms(2026, 2, 18, 0, 0, 0).unwrap();
        let dates = r.next_occurrences(from, 20).unwrap();
        let excluded = Utc.with_ymd_and_hms(2026, 2, 20, 0, 0, 0).unwrap();
        for dt in &dates {
            assert_ne!(*dt, excluded, "Should skip Feb 20");
        }
        // COUNT=10 total occurrences including DTSTART, minus 1 EXDATE = 8 after DTSTART
        assert_eq!(dates.len(), 8);
    }

    // -- Existing tests --

    #[test]
    fn test_next_occurrence_daily() {
        let after = Utc.with_ymd_and_hms(2025, 1, 1, 9, 0, 0).unwrap();
        let next = next_occurrence("FREQ=DAILY", after).unwrap();
        assert!(next.is_some());
        let next_dt = next.unwrap();
        assert!(next_dt > after, "Next occurrence should be after start");
    }

    #[test]
    fn test_next_occurrence_weekly() {
        let after = Utc.with_ymd_and_hms(2025, 1, 1, 9, 0, 0).unwrap();
        let next = next_occurrence("FREQ=WEEKLY;INTERVAL=2", after).unwrap();
        assert!(next.is_some());
        let next_dt = next.unwrap();
        let diff = next_dt - after;
        assert!(
            diff.num_days() >= 14,
            "Bi-weekly should be at least 14 days later"
        );
    }

    #[test]
    fn test_should_spawn_instance_due() {
        let now = Utc::now();
        let past = now - chrono::Duration::hours(1);
        assert!(should_spawn_instance(Some(past), now));
    }

    #[test]
    fn test_should_spawn_instance_not_yet() {
        let now = Utc::now();
        let future = now + chrono::Duration::hours(1);
        assert!(!should_spawn_instance(Some(future), now));
    }

    #[test]
    fn test_should_spawn_instance_none() {
        assert!(!should_spawn_instance(None, Utc::now()));
    }

    #[test]
    fn test_humanize_daily() {
        assert_eq!(humanize_rrule("FREQ=DAILY"), "Every day");
    }

    #[test]
    fn test_humanize_weekly_byday() {
        let result = humanize_rrule("FREQ=WEEKLY;BYDAY=MO,WE,FR");
        assert!(result.contains("Every week"));
        assert!(result.contains("Monday"));
        assert!(result.contains("Wednesday"));
        assert!(result.contains("Friday"));
    }

    #[test]
    fn test_humanize_monthly_bymonthday() {
        let result = humanize_rrule("FREQ=MONTHLY;BYMONTHDAY=1");
        assert!(result.contains("Every month"));
        assert!(result.contains("day 1"));
    }

    #[test]
    fn test_humanize_with_interval() {
        assert_eq!(humanize_rrule("FREQ=DAILY;INTERVAL=3"), "Every 3 days");
    }

    #[test]
    fn test_humanize_yearly() {
        assert_eq!(humanize_rrule("FREQ=YEARLY"), "Every year");
    }
}
