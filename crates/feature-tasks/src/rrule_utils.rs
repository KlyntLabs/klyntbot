//! RRULE utilities for recurring task support.
//!
//! Re-exports and wraps the rrule parsing logic.
//! Supports: FREQ, INTERVAL, BYDAY, BYHOUR, BYMINUTE, BYMONTHDAY, COUNT, UNTIL, EXDATE.
//!
//! Ported from feature-todo's `rrule_utils.rs` without modification.

use common::Result;
use jiff::Timestamp;
use rrule::RRuleSet;
use std::collections::HashMap;

const UNSUPPORTED_PARAMS: &[&str] = &["BYSETPOS", "WKST", "EXRULE", "RDATE"];

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
        let s = match self {
            Self::Daily => "DAILY",
            Self::Weekly => "WEEKLY",
            Self::Monthly => "MONTHLY",
            Self::Yearly => "YEARLY",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone)]
pub struct RRule {
    pub freq: Frequency,
    pub interval: u32,
    pub byday: Vec<String>,
    pub byhour: Vec<u32>,
    pub byminute: Vec<u32>,
    pub bymonthday: Vec<u32>,
    pub count: Option<u32>,
    pub until: Option<Timestamp>,
    pub exdate: Vec<Timestamp>,
}

fn parse_ical_datetime(s: &str) -> std::result::Result<Timestamp, String> {
    let s = s.trim();
    // Format: 20200101T000000Z — parse as a jiff civil datetime and treat as UTC.
    jiff::civil::DateTime::strptime("%Y%m%dT%H%M%SZ", s)
        .and_then(|cdt| cdt.to_zoned(jiff::tz::TimeZone::UTC))
        .map(|z| z.timestamp())
        .map_err(|e| format!("Invalid datetime '{}': {}", s, e))
}

fn parse_u32_list(s: &str) -> Vec<u32> {
    s.split(',').filter_map(|v| v.trim().parse().ok()).collect()
}

impl RRule {
    pub fn parse(rule: &str) -> Result<Self> {
        let upper = rule.to_uppercase();

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

        let freq_str = parts
            .get("FREQ")
            .ok_or_else(|| common::ToolError::InvalidParams("FREQ is required".to_string()))?;

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
                parse_ical_datetime(v)
                    .map_err(|e| common::ToolError::InvalidParams(format!("Invalid UNTIL: {}", e)))
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
            let z = until.to_zoned(jiff::tz::TimeZone::UTC);
            parts.push(format!("UNTIL={}", z.strftime("%Y%m%dT%H%M%SZ")));
        }

        parts.join(";")
    }

    pub fn next_occurrences(&self, from: Timestamp, max: usize) -> Result<Vec<Timestamp>> {
        let from_z = from.to_zoned(jiff::tz::TimeZone::UTC);
        let dtstart = format!("DTSTART:{}", from_z.strftime("%Y%m%dT%H%M%SZ"));
        let core = self.core_rule_string();
        let mut lines = vec![dtstart, format!("RRULE:{}", core)];

        for exd in &self.exdate {
            let exd_z = exd.to_zoned(jiff::tz::TimeZone::UTC);
            lines.push(format!("EXDATE:{}", exd_z.strftime("%Y%m%dT%H%M%SZ")));
        }

        let full = lines.join("\n");
        let rrule_set: RRuleSet = full
            .parse()
            .map_err(|e| common::ToolError::InvalidParams(format!("Invalid RRULE: {}", e)))?;

        let limit = max + 1;
        let result = rrule_set.all(limit as u16);
        // The rrule crate returns chrono::DateTime<chrono_tz::Tz>; convert to jiff::Timestamp
        // via the Unix timestamp so we stay at the boundary.
        let dates: Vec<Timestamp> = result
            .dates
            .into_iter()
            .map(|dt| {
                Timestamp::from_millisecond(dt.timestamp_millis()).expect("timestamp in range")
            })
            .filter(|ts| *ts > from)
            .take(max)
            .collect();

        Ok(dates)
    }
}

/// Validate an RRULE string, rejecting unsupported parameters.
pub fn validate_rrule(rule: &str) -> Result<()> {
    let parsed = RRule::parse(rule)?;
    let dtstart = "DTSTART:20200101T000000Z";
    let core = parsed.core_rule_string();
    let full = format!("{}\nRRULE:{}", dtstart, core);
    full.parse::<RRuleSet>()
        .map_err(|e| common::ToolError::InvalidParams(format!("Invalid RRULE syntax: {}", e)))?;
    Ok(())
}

/// Compute the next occurrence of an RRULE after a given datetime.
pub fn next_occurrence(rule: &str, after: Timestamp) -> Result<Option<Timestamp>> {
    let parsed = RRule::parse(rule)?;
    let occurrences = parsed.next_occurrences(after, 1)?;
    Ok(occurrences.into_iter().next())
}

/// Check if a recurring task instance should be spawned.
pub fn should_spawn_instance(next_instance_date: Option<Timestamp>, now: Timestamp) -> bool {
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

    #[test]
    fn test_validate_daily() {
        assert!(validate_rrule("FREQ=DAILY").is_ok());
    }

    #[test]
    fn test_validate_weekly_byday() {
        assert!(validate_rrule("FREQ=WEEKLY;BYDAY=MO,WE,FR").is_ok());
    }

    #[test]
    fn test_validate_rejects_bysetpos() {
        assert!(validate_rrule("FREQ=MONTHLY;BYDAY=MO;BYSETPOS=1").is_err());
    }

    #[test]
    fn test_humanize_daily() {
        assert_eq!(humanize_rrule("FREQ=DAILY"), "Every day");
    }

    #[test]
    fn test_humanize_weekly_with_byday() {
        let result = humanize_rrule("FREQ=WEEKLY;BYDAY=MO,WE,FR");
        assert!(result.contains("Every week"));
        assert!(result.contains("Monday"));
    }

    #[test]
    fn test_next_occurrence_daily() {
        let after = "2025-01-01T09:00:00Z".parse::<Timestamp>().unwrap();
        let next = next_occurrence("FREQ=DAILY", after).unwrap();
        assert!(next.is_some());
        assert!(next.unwrap() > after);
    }

    #[test]
    fn test_should_spawn_past() {
        let now = Timestamp::now();
        let past = now
            .checked_sub(jiff::SignedDuration::from_secs(3600))
            .unwrap_or(now);
        assert!(should_spawn_instance(Some(past), now));
    }

    #[test]
    fn test_should_spawn_future() {
        let now = Timestamp::now();
        let future = now
            .checked_add(jiff::SignedDuration::from_secs(3600))
            .unwrap_or(now);
        assert!(!should_spawn_instance(Some(future), now));
    }
}
