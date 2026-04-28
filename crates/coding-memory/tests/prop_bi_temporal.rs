//! Inv 4 — bi-temporal monotone: `valid_until >= valid_from` always.

use jiff::{Timestamp, ToSpan};
use proptest::prelude::*;
use storage::StoragePool;

mod common;

fn arb_offset_seconds() -> impl Strategy<Value = i64> {
    -86_400i64..=86_400i64
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn valid_until_never_precedes_valid_from(
        valid_from_offset in arb_offset_seconds(),
        valid_until_offset in arb_offset_seconds(),
    ) {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let pool = common::pool_with_migrations().await;
            let repos = cognitive::SemanticFactRepo::new(pool.inner().clone());

            let now = Timestamp::now();
            let valid_from = now.checked_add(valid_from_offset.seconds()).unwrap();
            let candidate_until = now.checked_add(valid_until_offset.seconds()).unwrap();

            let result = repos.insert_with_validity(
                "s", "p", "o",
                None,
                valid_from,
                Some(candidate_until),
            ).await;

            if candidate_until < valid_from {
                prop_assert!(result.is_err(), "repo accepted invalid until < from");
            } else {
                let fact = result.unwrap();
                let stored_until = fact.valid_until.unwrap();
                prop_assert!(stored_until >= fact.valid_from);
            }
            Ok(())
        }).unwrap();
    }
}
