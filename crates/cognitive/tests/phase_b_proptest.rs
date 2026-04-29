//! Property tests for Phase B deduplication and critic stability.

use cognitive::repos::procedural_rule::word_overlap_ratio;
use cognitive::repos::semantic_fact::SemanticFactRepo;
use cognitive::types::SemanticFact;
use proptest::prelude::*;
use storage::StoragePool;

fn fact_pool() -> tokio::runtime::Runtime {
    tokio::runtime::Runtime::new().unwrap()
}

proptest! {
    #[test]
    fn word_overlap_commutative(a in "[a-zA-Z ]{0,30}", b in "[a-zA-Z ]{0,30}") {
        let x = word_overlap_ratio(&a, &b);
        let y = word_overlap_ratio(&b, &a);
        prop_assert!((x - y).abs() < f64::EPSILON, "not commutative: {} vs {}", x, y);
    }

    #[test]
    fn word_overlap_range(a in "[a-zA-Z ]{0,30}", b in "[a-zA-Z ]{0,30}") {
        let s = word_overlap_ratio(&a, &b);
        prop_assert!((0.0..=1.0).contains(&s), "score out of range: {}", s);
    }

    #[test]
    fn word_overlap_identical(a in "[a-zA-Z]{3,10}( [a-zA-Z]{3,10}){0,5}") {
        prop_assert_eq!(word_overlap_ratio(&a, &a), 1.0);
    }

    #[test]
    fn word_overlap_empty(a in "[a-zA-Z ]{0,30}") {
        prop_assert_eq!(word_overlap_ratio("", &a), 0.0);
        prop_assert_eq!(word_overlap_ratio(&a, ""), 0.0);
    }

    #[test]
    fn scale_stability_non_increasing(factor in 0.0f64..=1.0) {
        let rt = fact_pool();
        let _ = rt.block_on(async {
            let pool = StoragePool::connect_in_memory().await.unwrap();
            let migrations = cognitive::repos::cognitive_migrations();
            StoragePool::run_feature_migrations(pool.inner(), &migrations).await.unwrap();
            let repo = SemanticFactRepo::new(pool.inner().clone());
            let fact = SemanticFact {
                id: "f1".into(),
                domain: "test".into(),
                subject: "s".into(),
                predicate: "p".into(),
                object: "o".into(),
                confidence: 0.8,
                source: "t".into(),
                valid_from: "2026-04-29".into(),
                recorded_at: "2026-04-29".into(),
                stability: 1.0,
                ..Default::default()
            };
            repo.upsert(&fact).await.unwrap();
            repo.scale_stability("f1", factor).await.unwrap();
            let after = repo.get("f1").await.unwrap().unwrap();
            prop_assert!(after.stability <= fact.stability, "stability increased: {} -> {}", fact.stability, after.stability);
            if factor == 0.0 {
                prop_assert_eq!(after.stability, 0.0);
            }
            Ok(())
        });
    }
}
