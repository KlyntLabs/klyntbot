//! RRULE DSL → RFC 5545 compiler + evaluator.
//!
//! ⚠️ CHRONO BOUNDARY: the upstream `rrule` crate uses `chrono::DateTime<Tz>`
//! at its API boundary. Chrono is a *private* dep of this module — downstream
//! code sees only `jiff::Timestamp`.
//!
//! Conversion strategy (lossless via epoch-ms):
//!   jiff::Timestamp  <--ms-->  chrono::DateTime<Utc>  <--with_timezone-->  chrono::DateTime<Tz>

use chrono::{DateTime, TimeZone, Utc};
use jiff::civil::Time as CivilTime;
use jiff::Timestamp;
use rrule::{RRuleSet, Tz as RruleTz};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use crate::error::SchedulerError;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Frequency {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RRuleSpec {
    pub frequency: Frequency,
    pub interval: Option<u32>,
    /// Weekday codes: "MO","TU","WE","TH","FR","SA","SU".
    pub by_day: Option<Vec<String>>,
    /// Day of month 1..=31 or negative for "from end" (e.g., -1 = last day).
    pub by_month_day: Option<Vec<i32>>,
    pub at: Option<CivilTime>,
    /// IANA timezone (e.g., "America/New_York").
    pub timezone: String,
    pub until: Option<Timestamp>,
    pub count: Option<u32>,
}

impl RRuleSpec {
    /// Compile to RFC 5545 RRULE text. Does NOT include DTSTART; callers supply.
    pub fn compile(&self) -> Result<String, SchedulerError> {
        let freq = match self.frequency {
            Frequency::Daily => "DAILY",
            Frequency::Weekly => "WEEKLY",
            Frequency::Monthly => "MONTHLY",
            Frequency::Yearly => "YEARLY",
        };
        let mut parts = vec![format!("FREQ={freq}")];
        if let Some(i) = self.interval {
            parts.push(format!("INTERVAL={i}"));
        }
        if let Some(bd) = &self.by_day {
            parts.push(format!("BYDAY={}", bd.join(",")));
        }
        if let Some(bmd) = &self.by_month_day {
            parts.push(format!(
                "BYMONTHDAY={}",
                bmd.iter().map(i32::to_string).collect::<Vec<_>>().join(",")
            ));
        }
        if let Some(t) = self.at {
            parts.push(format!("BYHOUR={}", t.hour()));
            parts.push(format!("BYMINUTE={}", t.minute()));
        }
        if let Some(c) = self.count {
            parts.push(format!("COUNT={c}"));
        }
        if let Some(u) = self.until {
            let dt = timestamp_to_chrono_utc(u);
            parts.push(format!("UNTIL={}", dt.format("%Y%m%dT%H%M%SZ")));
        }
        Ok(parts.join(";"))
    }
}

/// Parse a bare RRULE string (e.g. `"FREQ=DAILY;BYDAY=MO,WE"`) together with an
/// IANA timezone and an `after` cursor, and return the next `n` occurrences
/// strictly after `after`.
///
/// Internally this prepends a `DTSTART;TZID=…` line and delegates to the
/// upstream `rrule` crate's `RRuleSet` parser, so the **full RFC 5545 property
/// set** (`BYSETPOS`, `BYYEARDAY`, `WKST`, negative `BYMONTHDAY`, etc.) is
/// supported without any custom parsing.
pub fn next_n_from_rrule_string(
    rrule: &str,
    iana_tz: &str,
    after: Timestamp,
    n: usize,
) -> Result<Vec<Timestamp>, SchedulerError> {
    let tz = chrono_tz::Tz::from_str(iana_tz)
        .map_err(|e| SchedulerError::Rrule(format!("bad timezone {iana_tz}: {e}")))?;
    let rrule_tz: RruleTz = tz.into();

    let after_utc = timestamp_to_chrono_utc(after);
    let dtstart: DateTime<RruleTz> = after_utc.with_timezone(&rrule_tz);
    let full = format!(
        "DTSTART;TZID={iana_tz}:{}\nRRULE:{rrule}",
        dtstart.format("%Y%m%dT%H%M%S"),
    );

    let set: RRuleSet = full
        .parse()
        .map_err(|e| SchedulerError::Rrule(format!("rrule parse: {e}")))?;

    let out: Vec<Timestamp> = set
        .into_iter()
        .take(n)
        .filter_map(|dt| Timestamp::from_millisecond(dt.timestamp_millis()).ok())
        .collect();
    Ok(out)
}

pub fn evaluate_next_n(
    spec: &RRuleSpec,
    after: Timestamp,
    n: usize,
) -> Result<Vec<Timestamp>, SchedulerError> {
    let rrule_text = spec.compile()?;
    let tz = chrono_tz::Tz::from_str(&spec.timezone)
        .map_err(|e| SchedulerError::Rrule(format!("bad timezone {}: {e}", spec.timezone)))?;
    let rrule_tz: RruleTz = tz.into();

    let dtstart_utc = timestamp_to_chrono_utc(after);
    let dtstart: DateTime<RruleTz> = dtstart_utc.with_timezone(&rrule_tz);
    let full = format!(
        "DTSTART;TZID={}:{}\nRRULE:{}",
        spec.timezone,
        dtstart.format("%Y%m%dT%H%M%S"),
        rrule_text,
    );

    let set: RRuleSet = full
        .parse()
        .map_err(|e| SchedulerError::Rrule(format!("rrule parse: {e}")))?;
    let out: Vec<Timestamp> = set
        .into_iter()
        .take(n)
        .filter_map(|dt| Timestamp::from_millisecond(dt.timestamp_millis()).ok())
        .collect();
    Ok(out)
}

fn timestamp_to_chrono_utc(t: Timestamp) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(t.as_millisecond())
        .single()
        .expect("valid ms")
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::time;

    #[test]
    fn compiles_weekly_mwf_at_9am() {
        let dsl = RRuleSpec {
            frequency: Frequency::Weekly,
            interval: Some(1),
            by_day: Some(vec!["MO".into(), "WE".into(), "FR".into()]),
            at: Some(time(9, 0, 0, 0)),
            timezone: "America/New_York".into(),
            by_month_day: None,
            until: None,
            count: None,
        };
        let rule = dsl.compile().unwrap();
        assert!(rule.contains("FREQ=WEEKLY"));
        assert!(rule.contains("BYDAY=MO,WE,FR"));
        assert!(rule.contains("BYHOUR=9"));
    }

    #[test]
    fn evaluator_returns_next_three_instances_daily() {
        let dsl = RRuleSpec {
            frequency: Frequency::Daily,
            interval: Some(1),
            at: Some(time(9, 0, 0, 0)),
            timezone: "America/New_York".into(),
            by_day: None,
            by_month_day: None,
            until: None,
            count: None,
        };
        let start = jiff::Timestamp::from_millisecond(1_800_000_000_000).unwrap();
        let next = evaluate_next_n(&dsl, start, 3).unwrap();
        assert_eq!(next.len(), 3);
        let delta = next[1].as_millisecond() - next[0].as_millisecond();
        assert!((23 * 3600 * 1000..=25 * 3600 * 1000).contains(&delta));
    }

    #[test]
    fn daily_at_9am_ny_skips_evenly_across_dst_spring_forward() {
        let dsl = RRuleSpec {
            frequency: Frequency::Daily,
            interval: Some(1),
            at: Some(time(9, 0, 0, 0)),
            timezone: "America/New_York".into(),
            by_day: None,
            by_month_day: None,
            until: None,
            count: None,
        };
        // Anchor: 2026-03-07 08:00 NY (day before spring-forward at 02:00 on 2026-03-08).
        let anchor = jiff::civil::date(2026, 3, 7)
            .at(8, 0, 0, 0)
            .to_zoned(jiff::tz::TimeZone::get("America/New_York").unwrap())
            .unwrap()
            .timestamp();
        let next = evaluate_next_n(&dsl, anchor, 3).unwrap();
        let diff_0_1 = next[1].as_millisecond() - next[0].as_millisecond();
        // Crossing EDT transition: wall-clock 9am on both days, but UTC delta = 23h.
        assert_eq!(diff_0_1, 23 * 3600 * 1000);
    }
}
