//! Quiet-hours evaluation against a user's IANA timezone using Jiff.
use config::schema::QuietHoursConfig;
use jiff::{civil::Time, tz::TimeZone, Timestamp, Zoned};

use crate::error::{NotificationError, Result};

pub struct QuietHoursPolicy {
    cfg: QuietHoursConfig,
    tz: TimeZone,
}

impl QuietHoursPolicy {
    pub fn new(cfg: QuietHoursConfig, iana_tz: &str) -> Result<Self> {
        let tz = TimeZone::get(iana_tz)
            .map_err(|e| NotificationError::InvalidConfig(format!("tz {iana_tz}: {e}")))?;
        Ok(Self { cfg, tz })
    }

    pub fn is_in_quiet_hours(&self, at: Timestamp) -> Result<bool> {
        if !self.cfg.enabled {
            return Ok(false);
        }
        let start = parse_hhmm(&self.cfg.start)?;
        let end = parse_hhmm(&self.cfg.end)?;
        let zoned: Zoned = at.to_zoned(self.tz.clone());
        let now = zoned.time();
        if start <= end {
            Ok(now >= start && now < end)
        } else {
            Ok(now >= start || now < end)
        }
    }

    pub fn next_window_end(&self, at: Timestamp) -> Result<Timestamp> {
        let end = parse_hhmm(&self.cfg.end)?;
        let zoned: Zoned = at.to_zoned(self.tz.clone());
        let today_end = zoned
            .date()
            .at(end.hour(), end.minute(), 0, 0)
            .to_zoned(self.tz.clone())?;
        let candidate = if today_end.timestamp() > at {
            today_end
        } else {
            zoned
                .date()
                .tomorrow()?
                .at(end.hour(), end.minute(), 0, 0)
                .to_zoned(self.tz.clone())?
        };
        Ok(candidate.timestamp())
    }

    pub fn override_for_urgent(&self) -> bool {
        self.cfg.override_for_urgent_tasks
    }

    pub fn enabled(&self) -> bool {
        self.cfg.enabled
    }
}

fn parse_hhmm(s: &str) -> Result<Time> {
    let (h, m) = s
        .split_once(':')
        .ok_or_else(|| NotificationError::InvalidConfig(format!("bad HH:MM {s}")))?;
    let h: i8 = h
        .parse()
        .map_err(|_| NotificationError::InvalidConfig(format!("hour {s}")))?;
    let m: i8 = m
        .parse()
        .map_err(|_| NotificationError::InvalidConfig(format!("min {s}")))?;
    Time::new(h, m, 0, 0).map_err(|e| NotificationError::InvalidConfig(format!("{s}: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(enabled: bool, start: &str, end: &str) -> QuietHoursConfig {
        QuietHoursConfig {
            enabled,
            start: start.into(),
            end: end.into(),
            override_for_urgent_tasks: true,
        }
    }

    fn ts(iso: &str) -> Timestamp {
        iso.parse().unwrap()
    }

    #[test]
    fn disabled_always_false() {
        let p = QuietHoursPolicy::new(cfg(false, "22:00", "07:00"), "UTC").unwrap();
        assert!(!p.is_in_quiet_hours(ts("2026-01-01T23:00:00Z")).unwrap());
    }

    #[test]
    fn overnight_window_midnight_inside() {
        let p = QuietHoursPolicy::new(cfg(true, "22:00", "07:00"), "UTC").unwrap();
        assert!(p.is_in_quiet_hours(ts("2026-01-01T23:30:00Z")).unwrap());
        assert!(p.is_in_quiet_hours(ts("2026-01-01T03:00:00Z")).unwrap());
    }

    #[test]
    fn overnight_window_midday_outside() {
        let p = QuietHoursPolicy::new(cfg(true, "22:00", "07:00"), "UTC").unwrap();
        assert!(!p.is_in_quiet_hours(ts("2026-01-01T12:00:00Z")).unwrap());
    }

    #[test]
    fn daytime_window_inside_outside() {
        let p = QuietHoursPolicy::new(cfg(true, "09:00", "17:00"), "UTC").unwrap();
        assert!(p.is_in_quiet_hours(ts("2026-01-01T10:00:00Z")).unwrap());
        assert!(!p.is_in_quiet_hours(ts("2026-01-01T20:00:00Z")).unwrap());
    }

    #[test]
    fn tz_shifts_boundary() {
        let p = QuietHoursPolicy::new(cfg(true, "09:00", "17:00"), "America/New_York").unwrap();
        // 2026-01-01T15:00Z = 10:00 EST — inside 09:00–17:00
        assert!(p.is_in_quiet_hours(ts("2026-01-01T15:00:00Z")).unwrap());
        // 2026-01-01T23:00Z = 18:00 EST — outside 09:00–17:00
        assert!(!p.is_in_quiet_hours(ts("2026-01-01T23:00:00Z")).unwrap());
    }

    #[test]
    fn next_window_end_overnight() {
        let p = QuietHoursPolicy::new(cfg(true, "22:00", "07:00"), "UTC").unwrap();
        let end = p.next_window_end(ts("2026-01-01T23:30:00Z")).unwrap();
        assert_eq!(end.to_string(), "2026-01-02T07:00:00Z");
    }
}
