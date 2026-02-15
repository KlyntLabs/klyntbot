//! RRULE utilities for recurring task support.
//!
//! Wraps the `rrule` crate to provide a simplified API for V1 recurrence rules.
//! V1 supports: FREQ (DAILY/WEEKLY/MONTHLY/YEARLY), INTERVAL, BYDAY, BYHOUR,
//! BYMINUTE, BYMONTHDAY only.

use chrono::{DateTime, Utc};
use common::Result;
use rrule::RRuleSet;

/// Unsupported RRULE parameters in V1.
const UNSUPPORTED_PARAMS: &[&str] = &[
    "BYSETPOS", "WKST", "EXDATE", "EXRULE", "RDATE", "COUNT", "UNTIL",
];

/// Validate an RRULE string, rejecting unsupported parameters.
///
/// V1 allows: FREQ, INTERVAL, BYDAY, BYHOUR, BYMINUTE, BYMONTHDAY.
pub fn validate_rrule(rule: &str) -> Result<()> {
    let upper = rule.to_uppercase();

    // Check for unsupported parameters
    for param in UNSUPPORTED_PARAMS {
        if upper.contains(param) {
            return Err(common::ToolError::InvalidParams(format!(
                "Unsupported RRULE parameter: {}. V1 supports: FREQ, INTERVAL, BYDAY, BYHOUR, BYMINUTE, BYMONTHDAY",
                param
            ))
            .into());
        }
    }

    // Try parsing to validate syntax
    let dtstart = "DTSTART:20200101T000000Z";
    let full = format!("{}\nRRULE:{}", dtstart, rule);
    full.parse::<RRuleSet>()
        .map_err(|e| common::ToolError::InvalidParams(format!("Invalid RRULE syntax: {}", e)))?;

    Ok(())
}

/// Compute the next occurrence of an RRULE after a given datetime.
///
/// Prepends DTSTART if not present in the rule string, using `after` as the
/// start date. Returns None if no further occurrences.
pub fn next_occurrence(rule: &str, after: DateTime<Utc>) -> Result<Option<DateTime<Utc>>> {
    let dtstart = format!("DTSTART:{}", after.format("%Y%m%dT%H%M%SZ"));
    let full = format!("{}\nRRULE:{}", dtstart, rule);

    let rrule_set: RRuleSet = full
        .parse()
        .map_err(|e| common::ToolError::InvalidParams(format!("Invalid RRULE: {}", e)))?;

    // Generate a few occurrences and find the first one strictly after `after`
    let result = rrule_set.all(10);
    let next = result
        .dates
        .into_iter()
        .map(|dt| dt.with_timezone(&Utc))
        .find(|dt| *dt > after);

    Ok(next)
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
    let parts: std::collections::HashMap<&str, &str> = upper
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
    fn test_validate_rrule_rejects_count() {
        let result = validate_rrule("FREQ=DAILY;COUNT=10");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("COUNT"), "Error should mention COUNT: {}", err);
    }

    #[test]
    fn test_validate_rrule_rejects_until() {
        let result = validate_rrule("FREQ=DAILY;UNTIL=20250101T000000Z");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_rrule_rejects_exdate() {
        let result = validate_rrule("FREQ=DAILY;EXDATE=20250101T000000Z");
        assert!(result.is_err());
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
