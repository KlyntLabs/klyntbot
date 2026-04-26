use coding_memory::recall::budget::HeuristicBudgeter;
use coding_memory::recall::{CodingRecallService, CodingRecallServiceConfig};
use proptest::prelude::*;
use std::sync::Arc;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(8))]

    #[test]
    fn same_query_same_ids(query in "[a-z ]{4,40}") {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = storage::StoragePool::connect_in_memory().await.unwrap();
            storage::StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
            storage::StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations()).await.unwrap();
            let fact_repo = Arc::new(cognitive::SemanticFactRepo::new(pool.inner().clone()));
            let ep_repo = Arc::new(cognitive::EpisodicMemoryRepo::new(pool.inner().clone()));
            let telem = coding_memory::RecallInvocationRepo::new(pool.clone());
            let ums = Arc::new(cognitive::UnifiedMemoryService::new((*fact_repo).clone()));
            let svc = CodingRecallService::new(
                CodingRecallServiceConfig::default(),
                ums, fact_repo, ep_repo, telem,
                Arc::new(HeuristicBudgeter),
            );
            let a = svc.recall_index(&query, None, None, None, 5).await.unwrap();
            let b = svc.recall_index(&query, None, None, None, 5).await.unwrap();
            let a_ids: Vec<_> = a.results.iter().map(|r| r.id).collect();
            let b_ids: Vec<_> = b.results.iter().map(|r| r.id).collect();
            prop_assert_eq!(a_ids, b_ids);
            Ok(())
        }).unwrap();
    }
}
