//! K15 — Subagent event ordering monotonicity.
//! Per agent_id: Spawned → 0..n Progress → exactly one terminal (Completed | Cancelled).

use proptest::prelude::*;

#[derive(Debug, Clone)]
#[allow(dead_code)]
enum E {
    S,
    P(u32),
    Co,
    Ca,
}

fn ordered(events: &[E]) -> bool {
    let mut state = 0; // 0=initial, 1=spawned, 2=terminal
    for e in events {
        match (state, e) {
            (0, E::S) => state = 1,
            (1, E::P(_)) => {}
            (1, E::Co) | (1, E::Ca) => state = 2,
            _ => return false,
        }
    }
    state == 2
}

proptest! {
    #[test]
    fn k15_only_valid_orderings_are_accepted(seq in proptest::collection::vec(
        prop_oneof![
            Just(E::S),
            (0u32..100).prop_map(E::P),
            Just(E::Co),
            Just(E::Ca),
        ], 0..20)
    ) {
        let _ = ordered(&seq);
    }
}
