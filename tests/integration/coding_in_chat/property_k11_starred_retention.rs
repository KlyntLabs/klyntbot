use proptest::prelude::*;
use storage::StoragePool;
use storage::repos::SessionRepo;

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, .. ProptestConfig::default() })]

    #[test]
    fn k11_starred_session_survives_any_ttl(
        starred_count in 1usize..10,
        unstarred_count in 0usize..10,
        ttl_days in 0u32..365,
    ) {
        tokio::runtime::Runtime::new().unwrap().block_on(async move {
            let pool = StoragePool::connect_in_memory().await.unwrap();
            let repo = SessionRepo::new(pool.inner().clone());
            for i in 0..starred_count {
                let key = format!("starred-{i}");
                repo.upsert_session(&key, &serde_json::json!({})).await.unwrap();
                sqlx::query("UPDATE sessions SET pinned = 1, updated_at = 0 WHERE key = ?")
                    .bind(&key).execute(pool.inner()).await.unwrap();
            }
            for i in 0..unstarred_count {
                let key = format!("ephemeral-{i}");
                repo.upsert_session(&key, &serde_json::json!({})).await.unwrap();
                sqlx::query("UPDATE sessions SET updated_at = 0 WHERE key = ?")
                    .bind(&key).execute(pool.inner()).await.unwrap();
            }
            repo.delete_stale_sessions(ttl_days).await.unwrap();
            let surviving = repo.count_sessions().await.unwrap();
            prop_assert!(surviving as usize >= starred_count,
                "K11: {starred_count} starred sessions must survive (got {surviving})");
            Ok(())
        }).unwrap();
    }
}
