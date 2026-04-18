//! Alarm rule variants and fire-time computation.
//!
//! Three orthogonal variants cover every user utterance:
//! - `RelativeBefore` — "24h before deadline", "5min before"
//! - `CivilTimeOnDayOffset` — "9am the day before", "8am on the deadline day"
//! - `Absolute` — "at 2026-04-20T09:00:00-04:00"
//!
//! All computation is DST-correct via `jiff::Zoned` arithmetic.

use jiff::civil::Time as CivilTime;
use jiff::tz::TimeZone;
use jiff::{Span, Timestamp};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AlarmRule {
    RelativeBefore {
        offset: Span,
    },
    CivilTimeOnDayOffset {
        day_offset: i32,
        time_of_day: CivilTime,
        iana_tz: String,
    },
    Absolute {
        fire_at: Timestamp,
    },
}

#[derive(Debug, Error)]
pub enum RuleError {
    #[error("relative_before rule requires a due_date")]
    MissingDueDate,
    #[error("civil_time rule requires a due_date to compute day offset")]
    MissingDueDateForCivil,
    #[error("unknown timezone: {0}")]
    UnknownTimezone(String),
    #[error("jiff arithmetic failed: {0}")]
    Jiff(#[from] jiff::Error),
}

impl AlarmRule {
    /// Compute the UTC Timestamp at which this rule should fire.
    ///
    /// `due_date` is the task's deadline as a UTC instant (required for relative/civil variants).
    /// `default_tz` is reserved for future variants that don't store their own tz.
    pub fn compute_fire_at(
        &self,
        due_date: Option<Timestamp>,
        _default_tz: &str,
    ) -> Result<Timestamp, RuleError> {
        match self {
            Self::Absolute { fire_at } => Ok(*fire_at),
            Self::RelativeBefore { offset } => {
                let due = due_date.ok_or(RuleError::MissingDueDate)?;
                Ok(due.checked_sub(*offset)?)
            }
            Self::CivilTimeOnDayOffset {
                day_offset,
                time_of_day,
                iana_tz,
            } => {
                let tz = TimeZone::get(iana_tz)
                    .map_err(|_| RuleError::UnknownTimezone(iana_tz.clone()))?;
                let due = due_date.ok_or(RuleError::MissingDueDateForCivil)?;
                let due_zoned = due.to_zoned(tz.clone());
                let civil_date = due_zoned
                    .date()
                    .checked_add(Span::new().days(*day_offset as i64))?;
                let civil_dt = civil_date.at(
                    time_of_day.hour(),
                    time_of_day.minute(),
                    time_of_day.second(),
                    0,
                );
                let zoned = civil_dt.to_zoned(tz)?;
                Ok(zoned.timestamp())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::{date, time};
    use jiff::tz::TimeZone;
    use jiff::Timestamp;

    fn ts(y: i16, m: i8, d: i8, hh: i8, mm: i8, tz: &str) -> Timestamp {
        date(y, m, d)
            .at(hh, mm, 0, 0)
            .to_zoned(TimeZone::get(tz).unwrap())
            .unwrap()
            .timestamp()
    }

    #[test]
    fn relative_before_subtracts_offset() {
        let due = ts(2026, 4, 22, 14, 0, "America/New_York");
        let rule = AlarmRule::RelativeBefore {
            offset: jiff::Span::new().hours(1),
        };
        let fire = rule.compute_fire_at(Some(due), "UTC").unwrap();
        assert_eq!(fire.as_millisecond(), due.as_millisecond() - 3_600_000);
    }

    #[test]
    fn civil_time_day_minus_one_9am_is_dst_correct_in_ny() {
        let due = ts(2026, 3, 9, 14, 0, "America/New_York");
        let rule = AlarmRule::CivilTimeOnDayOffset {
            day_offset: -1,
            time_of_day: time(9, 0, 0, 0),
            iana_tz: "America/New_York".into(),
        };
        let fire = rule.compute_fire_at(Some(due), "UTC").unwrap();
        let expected = ts(2026, 3, 8, 9, 0, "America/New_York");
        assert_eq!(fire.as_millisecond(), expected.as_millisecond());
    }

    #[test]
    fn civil_time_skipped_hour_resolves_to_post_transition() {
        let due = ts(2026, 3, 8, 14, 0, "America/New_York");
        let rule = AlarmRule::CivilTimeOnDayOffset {
            day_offset: 0,
            time_of_day: time(2, 30, 0, 0),
            iana_tz: "America/New_York".into(),
        };
        let fire = rule.compute_fire_at(Some(due), "UTC").unwrap();
        let expected = ts(2026, 3, 8, 3, 30, "America/New_York");
        assert_eq!(fire.as_millisecond(), expected.as_millisecond());
    }

    #[test]
    fn absolute_returns_input_unchanged() {
        let t = Timestamp::from_millisecond(1_800_000_000_000).unwrap();
        let rule = AlarmRule::Absolute { fire_at: t };
        let fire = rule.compute_fire_at(None, "UTC").unwrap();
        assert_eq!(fire, t);
    }

    #[test]
    fn relative_before_without_due_errors() {
        let rule = AlarmRule::RelativeBefore {
            offset: jiff::Span::new().hours(1),
        };
        assert!(rule.compute_fire_at(None, "UTC").is_err());
    }
}
