//! KCA Track 9-typing — renderer groups causal vs related context.

use coding_memory::recall::renderers::render_causal_context;
use cognitive::repos::entity::{EntityRepo, NewEntity};
use storage::StoragePool;

async fn fresh_pool() -> StoragePool {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    pool
}

#[tokio::test]
async fn renderer_groups_causal_edges_separately() {
    let pool = fresh_pool().await;
    let entity_repo = EntityRepo::new(pool.inner().clone());

    let cause = entity_repo
        .upsert_entity(&NewEntity {
            name: "deadline_pressure".into(),
            entity_type: "concept".into(),
            description: None,
            source: "t".into(),
            source_id: None,
            metadata: None,
        })
        .await
        .unwrap();
    let effect = entity_repo
        .upsert_entity(&NewEntity {
            name: "late_night_coding".into(),
            entity_type: "concept".into(),
            description: None,
            source: "t".into(),
            source_id: None,
            metadata: None,
        })
        .await
        .unwrap();
    let other = entity_repo
        .upsert_entity(&NewEntity {
            name: "rust".into(),
            entity_type: "tool".into(),
            description: None,
            source: "t".into(),
            source_id: None,
            metadata: None,
        })
        .await
        .unwrap();

    entity_repo
        .upsert_relationship_typed(&cause.id, &effect.id, "leads_to", "causal", 0.9, None, "t")
        .await
        .unwrap();
    entity_repo
        .upsert_relationship_typed(
            &effect.id,
            &other.id,
            "uses",
            "correlational",
            0.5,
            None,
            "t",
        )
        .await
        .unwrap();

    let block = render_causal_context(&entity_repo, &["late_night_coding"])
        .await
        .unwrap();
    assert!(
        block.contains("Causal Context"),
        "must label causal edges, got:\n{block}"
    );
    assert!(block.contains("deadline_pressure"));
    assert!(block.contains("leads_to"));
    // Correlational edges should be in Related Context
    assert!(
        block.contains("Related Context"),
        "must label related edges, got:\n{block}"
    );
    assert!(block.contains("rust"));
    assert!(block.contains("uses"));
}

#[tokio::test]
async fn renderer_omits_empty_sections() {
    let pool = fresh_pool().await;
    let entity_repo = EntityRepo::new(pool.inner().clone());

    let a = entity_repo
        .upsert_entity(&NewEntity {
            name: "alice".into(),
            entity_type: "person".into(),
            description: None,
            source: "t".into(),
            source_id: None,
            metadata: None,
        })
        .await
        .unwrap();
    let b = entity_repo
        .upsert_entity(&NewEntity {
            name: "bob".into(),
            entity_type: "person".into(),
            description: None,
            source: "t".into(),
            source_id: None,
            metadata: None,
        })
        .await
        .unwrap();

    // Only correlational edge — no causal
    entity_repo
        .upsert_relationship_typed(&a.id, &b.id, "knows", "correlational", 0.8, None, "t")
        .await
        .unwrap();

    let block = render_causal_context(&entity_repo, &["alice"])
        .await
        .unwrap();
    assert!(
        !block.contains("Causal Context"),
        "should omit empty causal section, got:\n{block}"
    );
    assert!(block.contains("Related Context"));
    assert!(block.contains("bob"));
}
