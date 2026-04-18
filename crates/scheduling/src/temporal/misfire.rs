//! Misfire policy evaluation.
//!
//! A fire is "misfired" when `fire_at <= now` by more than an epsilon.
//! The policy determines whether to dispatch, skip, or coalesce.
//! Pure logic — no I/O.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MisfirePolicy {
    /// Always fire, no matter how stale.
    Strict,
    /// Fire if within grace window; otherwise mark as missed.
    #[default]
    SkipIfStale,
    /// Fire the most recent pending row per (task_id, kind) group; suppress older.
    Coalesce,
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
            MisfirePolicy::SkipIfStale => {
                #[allow(clippy::cast_sign_loss)]
                // grace is inclusive: age exactly equal to grace still fires.
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

    #[test]
    fn skip_if_stale_fires_at_exact_grace_boundary() {
        let grace = grace_60min();
        let age_ms = i64::try_from(grace.as_millis()).unwrap();
        let d = Decision::classify(MisfirePolicy::SkipIfStale, grace, t(0), t(age_ms));
        assert_eq!(d, Decision::Fire);
    }

    #[test]
    fn coalesce_always_returns_coalesce_later() {
        let d = Decision::classify(MisfirePolicy::Coalesce, grace_60min(), t(0), t(100_000_000));
        assert_eq!(d, Decision::CoalesceLater);
    }

    #[test]
    fn misfire_policy_serde_round_trips_snake_case() {
        let encoded = serde_json::to_string(&MisfirePolicy::SkipIfStale).unwrap();
        assert_eq!(encoded, "\"skip_if_stale\"");
        let decoded: MisfirePolicy = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, MisfirePolicy::SkipIfStale);
    }
}
