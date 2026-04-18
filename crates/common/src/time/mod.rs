//! Canonical time types and helpers for Klyntbot.
//! New code should use `jiff::Timestamp` / `jiff::Zoned` / `jiff::civil::*`
//! instead of `chrono` types.

pub mod convert;
pub mod helpers;

pub use helpers::{now_in_tz, now_utc, system_tz};
pub use jiff;
