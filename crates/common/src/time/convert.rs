//! Storage wire format conversion helpers.
//! All persisted timestamps use Unix epoch milliseconds (i64, UTC).

use jiff::Timestamp;

pub fn ts_to_millis(ts: Timestamp) -> i64 {
    ts.as_millisecond()
}

pub fn millis_to_ts(ms: i64) -> Result<Timestamp, jiff::Error> {
    Timestamp::from_millisecond(ms)
}

pub fn opt_ts_to_millis(ts: Option<Timestamp>) -> Option<i64> {
    ts.map(ts_to_millis)
}

pub fn opt_millis_to_ts(ms: Option<i64>) -> Option<Timestamp> {
    ms.and_then(|v| millis_to_ts(v).ok())
}
