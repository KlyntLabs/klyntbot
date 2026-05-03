//! K11 proptest: Sessions retention monotonicity — starred sessions never pruned.
//!
//! Invariant: When `preserve_starred = true`, sessions with `pinned = 1`
//! are never deleted by the retention pass, regardless of age.

use proptest::prelude::*;

/// Simulate the retention SQL logic: DELETE FROM sessions WHERE created_at < cutoff
/// AND COALESCE(pinned, 0) = 0.
///
/// Returns the set of session indices that would survive.
fn simulate_retention(
    sessions: &[(bool, i64)], // (starred, created_at_ms)
    cutoff_ms: i64,
    preserve_starred: bool,
) -> Vec<usize> {
    sessions
        .iter()
        .enumerate()
        .filter(|(_, (starred, created_at))| {
            // Session survives if:
            // 1. It's newer than the cutoff, OR
            // 2. preserve_starred is true AND it's starred
            *created_at >= cutoff_ms || (preserve_starred && *starred)
        })
        .map(|(i, _)| i)
        .collect()
}

proptest! {
    #[test]
    fn k11_starred_sessions_never_pruned(
        sessions in proptest::collection::vec(
            (any::<bool>(), 0i64..1_000_000_000),
            1..50,
        ),
        retention_days in 1i64..365,
    ) {
        let now_ms = 2_000_000_000_000i64; // fixed "now"
        let cutoff_ms = now_ms - retention_days * 86_400_000;

        let survivors = simulate_retention(&sessions, cutoff_ms, true);

        // Every starred session must survive
        for (i, (starred, _)) in sessions.iter().enumerate() {
            if *starred {
                prop_assert!(
                    survivors.contains(&i),
                    "starred session {i} was pruned (now={now_ms}, cutoff={cutoff_ms})"
                );
            }
        }
    }

    #[test]
    fn k11_old_unstarred_sessions_always_pruned(
        sessions in proptest::collection::vec(
            (any::<bool>(), 0i64..100_000_000), // old timestamps
            1..50,
        ),
    ) {
        let now_ms = 2_000_000_000_000i64;
        let cutoff_ms = now_ms - 86_400_000; // 1 day retention
        // All sessions are old (before cutoff)

        let survivors = simulate_retention(&sessions, cutoff_ms, true);

        // Unstarred old sessions must be pruned
        for (i, (starred, created_at)) in sessions.iter().enumerate() {
            if !starred && *created_at < cutoff_ms {
                prop_assert!(
                    !survivors.contains(&i),
                    "old unstarred session {i} should have been pruned"
                );
            }
        }
    }

    #[test]
    fn k11_preserve_starred_false_prunes_everything_old(
        sessions in proptest::collection::vec(
            (any::<bool>(), 0i64..100_000_000),
            1..50,
        ),
    ) {
        let now_ms = 2_000_000_000_000i64;
        let cutoff_ms = now_ms - 86_400_000;

        let survivors = simulate_retention(&sessions, cutoff_ms, false);

        // With preserve_starred=false, ALL old sessions are pruned
        for (i, (_, created_at)) in sessions.iter().enumerate() {
            if *created_at < cutoff_ms {
                prop_assert!(
                    !survivors.contains(&i),
                    "session {i} should have been pruned when preserve_starred=false"
                );
            }
        }
    }
}
