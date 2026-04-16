//! Transient Chrono ↔ Jiff conversion helpers used during migration.
//! Removed in the final cleanup task once no crate depends on Chrono.

use chrono::{DateTime, Utc};
use jiff::Timestamp;

pub fn chrono_to_jiff(dt: DateTime<Utc>) -> Timestamp {
    Timestamp::from_millisecond(dt.timestamp_millis()).expect("timestamp in range")
}

pub fn jiff_to_chrono(ts: Timestamp) -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp_millis(ts.as_millisecond()).expect("timestamp in range")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_chrono_to_jiff_to_chrono() {
        let original = Utc::now();
        let round = jiff_to_chrono(chrono_to_jiff(original));
        assert_eq!(original.timestamp_millis(), round.timestamp_millis());
    }

    #[test]
    fn round_trip_jiff_to_chrono_to_jiff() {
        let original = Timestamp::now();
        let round = chrono_to_jiff(jiff_to_chrono(original));
        assert_eq!(original.as_millisecond(), round.as_millisecond());
    }
}
