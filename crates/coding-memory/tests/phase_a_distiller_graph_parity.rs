//! KCA AIT.2 — coding distiller produces typed entity edges + graph linker fires.

use coding_memory::distiller::test_helpers::{distill_test_turn, FixtureBuilder};
use cognitive::repos::entity::{EdgeType, EntityRepo};
use cognitive::repos::semantic_fact::SemanticFactRepo;
use cognitive::services::graph_linker::NoopGraphLinkHandler;
use storage::StoragePool;

async fn fresh_pool() -> StoragePool {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    pool
}

#[tokio::test]
async fn distiller_writes_typed_correlational_edges_by_default() {
    let pool = fresh_pool().await;
    let fact_repo = SemanticFactRepo::new(pool.inner().clone());
    let entity_repo = EntityRepo::new(pool.inner().clone());

    let fixture = FixtureBuilder::new()
        .with_user_prompt("what testing tool do we use?")
        .with_assistant_msg("We use cargo-nextest for Rust tests in klyntbot.")
        .build();

    distill_test_turn(&fixture, &fact_repo, &entity_repo).await;

    // Entity graph should contain klyntbot --uses--> cargo-nextest with edge_type = correlational
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
    let nextest = nbrs
        .iter()
        .find(|n| n.neighbor.name == "cargo-nextest")
        .expect("cargo-nextest neighbor");
    assert_eq!(
        nextest.edge_type,
        EdgeType::Correlational,
        "default distiller edges should be correlational, got {:?}",
        nextest.edge_type
    );
}

#[tokio::test]
async fn distiller_linker_handler_can_be_wired() {
    let pool = fresh_pool().await;
    let fact_repo = SemanticFactRepo::new(pool.inner().clone());
    let entity_repo = EntityRepo::new(pool.inner().clone());

    let _fixture = FixtureBuilder::new()
        .with_user_prompt("what language is klyntbot written in?")
        .with_assistant_msg("klyntbot is written in Rust.")
        .build();

    // Distiller with a NoopGraphLinkHandler — just verifies the wiring compiles & runs.
    let ingest = std::sync::Arc::new(coding_ingest::store::IngestEventLogRepo::new(
        pool.inner().clone(),
    ));
    let writer = coding_memory::distiller::DistillerWriter::new(
        fact_repo.clone(),
        cognitive::EpisodicMemoryRepo::new(pool.inner().clone()),
    )
    .with_entity_repo(entity_repo.clone());

    let provider = std::sync::Arc::new(providers::ProviderManager::new(
        std::sync::Arc::new(providers::NoopProvider),
        None,
        None,
    ));
    let retriever = std::sync::Arc::new(cognitive::UnifiedMemoryService::new(fact_repo));

    let distiller = coding_memory::distiller::Distiller::new(
        coding_memory::distiller::DistillerConfig::default(),
        ingest,
        writer,
        provider,
        retriever,
    )
    .with_graph_link_handler(std::sync::Arc::new(NoopGraphLinkHandler));

    // Just ensure distill_turn runs without panic when linker is wired.
    // (No actual LLM call because provider is NoopProvider.)
    let _ = distiller.distill_turn("s1", None).await;
}
