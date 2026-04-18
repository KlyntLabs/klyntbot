//! Misfire policy evaluation.
//!
//! A fire is "misfired" when `fire_at <= now` by more than an epsilon.
//! The policy determines whether to dispatch, skip, or coalesce.
//! Pure logic — no I/O.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MisfirePolicy {
    /// Always fire, no matter how stale.
    Strict,
    /// Fire if within grace window; otherwise mark as missed.
    SkipIfStale,
    /// Fire the most recent pending row per (task_id, kind) group; suppress older.
    Coalesce,
}

#[allow(clippy::derivable_impls)]
impl Default for MisfirePolicy {
    fn default() -> Self {
        Self::SkipIfStale
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Fire,
    SkipStale,
    CoalesceLater,
}

impl Decision {
    pub fn classify(
        policy: MisfirePolicy,
        grace: std::time::Duration,
        fire_at: Timestamp,
        now: Timestamp,
    ) -> Self {
        let age_ms = (now.as_millisecond() - fire_at.as_millisecond()).max(0);
        match policy {
            MisfirePolicy::Strict => Self::Fire,
            MisfirePolicy::SkipIfStale =>
            {
                #[allow(clippy::cast_sign_loss)]
                if (age_ms as u128) <= grace.as_millis() {
                    Self::Fire
                } else {
                    Self::SkipStale
                }
            }
            MisfirePolicy::Coalesce => Self::CoalesceLater,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::Timestamp;
    use std::time::Duration;

    fn t(ms: i64) -> Timestamp {
        Timestamp::from_millisecond(ms).unwrap()
    }

    fn grace_60min() -> Duration {
        Duration::from_secs(3600)
    }

    #[test]
    fn strict_fires_no_matter_how_stale() {
        let d = Decision::classify(MisfirePolicy::Strict, grace_60min(), t(0), t(100_000_000));
        assert_eq!(d, Decision::Fire);
    }

    #[test]
    fn skip_if_stale_fires_within_grace() {
        let d = Decision::classify(
            MisfirePolicy::SkipIfStale,
            grace_60min(),
            t(0),
            t(30 * 60 * 1000),
        );
        assert_eq!(d, Decision::Fire);
    }

    #[test]
    fn skip_if_stale_skips_past_grace() {
        let d = Decision::classify(
            MisfirePolicy::SkipIfStale,
            grace_60min(),
            t(0),
            t(61 * 60 * 1000),
        );
        assert_eq!(d, Decision::SkipStale);
    }
}
