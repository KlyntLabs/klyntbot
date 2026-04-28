//! Track 3 — coding facts must produce entity_relationships rows like chat facts.

use cognitive::repos::entity::EntityRepo;
use cognitive::repos::semantic_fact::SemanticFactRepo;
use storage::StoragePool;

use coding_memory::distiller::test_helpers::{distill_test_turn, FixtureBuilder};

#[tokio::test]
async fn distiller_writes_entity_edges_for_repo_context_fact() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    storage::StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    let fact_repo = SemanticFactRepo::new(pool.inner().clone());
    let entity_repo = EntityRepo::new(pool.inner().clone());

    let fixture = FixtureBuilder::new()
        .with_user_prompt("which test framework does this repo use?")
        .with_assistant_msg("This repo uses cargo-nextest for testing.")
        .build();

    distill_test_turn(&fixture, &fact_repo, &entity_repo).await;

    let kb = entity_repo
        .find_by_name("klyntbot")
        .await
        .unwrap()
        .into_iter()
        .next()
        .expect("klyntbot entity");
    let nbrs = entity_repo
        .get_neighborhood_with_edges(&kb.id, 1)
        .await
        .unwrap();
    let names: Vec<&str> = nbrs.iter().map(|n| n.neighbor.name.as_str()).collect();
    assert!(
        names.contains(&"cargo-nextest"),
        "expected cargo-nextest neighbor, got {:?}",
        names
    );
}
