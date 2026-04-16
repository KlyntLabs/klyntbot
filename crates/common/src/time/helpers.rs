//! User-facing time helpers that consume the application timezone.

use jiff::{tz::TimeZone, Timestamp, Zoned};

pub fn now_utc() -> Timestamp {
    Timestamp::now()
}

pub fn now_in_tz(iana: &str) -> Result<Zoned, jiff::Error> {
    let tz = TimeZone::get(iana)?;
    Ok(Timestamp::now().to_zoned(tz))
}

/// Returns the system timezone, or UTC if it cannot be determined.
pub fn system_tz() -> TimeZone {
    TimeZone::system()
}
