//! Integration test for Phase 4: every successful Distiller write must
//! publish a `DomainEvent::CodingMemoryUpdated` on the attached bus.

use bus::{CodingMemoryKind, DomainEvent, DomainEventBus};
use std::sync::Arc;
use storage::StoragePool;
use tokio::time::{timeout, Duration};

mod harness {
    use coding_ingest::store::IngestEventLogRepo;
    use coding_memory::distiller::{Distiller, DistillerConfig, DistillerWriter};
    use cognitive::{EpisodicMemoryRepo, SemanticFactRepo};
    use std::sync::Arc;
    use storage::StoragePool;

    pub async fn build(bus: Arc<bus::DomainEventBus>) -> Distiller {
        let pool = StoragePool::connect_in_memory().await.expect("pool");
        StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
            .await
            .unwrap();
        StoragePool::run_feature_migrations(
            pool.inner(),
            &coding_memory::coding_memory_migrations(),
        )
        .await
        .unwrap();
        let inner = pool.inner().clone();
        let ingest_repo = Arc::new(IngestEventLogRepo::new(inner.clone()));
        let fact_repo = SemanticFactRepo::new(inner.clone());
        let episode_repo = EpisodicMemoryRepo::new(inner.clone());
        let writer = DistillerWriter::new(fact_repo.clone(), episode_repo);
        let retriever: Arc<dyn context_engine::MemoryRetriever> =
            Arc::new(cognitive::UnifiedMemoryService::new(fact_repo));
        let provider = Arc::new(providers::ProviderManager::new(
            Arc::new(providers::NoopProvider),
            None,
            None,
        ));
        Distiller::new(
            DistillerConfig::default(),
            ingest_repo,
            writer,
            provider,
            retriever,
        )
        .with_event_bus(bus)
    }
}

#[tokio::test]
async fn fact_write_publishes_coding_memory_updated() {
    let bus = Arc::new(DomainEventBus::new(16));
    let mut rx = bus.subscribe();
    let d = harness::build(bus.clone()).await;

    let id = d
        .write_fact_for_test("session-x", "fact text")
        .await
        .unwrap();

    let evt = timeout(Duration::from_millis(200), rx.recv())
        .await
        .expect("no event in 200 ms")
        .expect("bus closed");

    match evt {
        DomainEvent::CodingMemoryUpdated {
            kind,
            id: emitted_id,
        } => {
            assert_eq!(kind, CodingMemoryKind::Fact);
            assert_eq!(emitted_id, id);
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[tokio::test]
async fn episode_write_publishes_coding_memory_updated() {
    let bus = Arc::new(DomainEventBus::new(16));
    let mut rx = bus.subscribe();
    let d = harness::build(bus.clone()).await;

    let id = d.write_episode_for_test("session-y").await.unwrap();

    let evt = timeout(Duration::from_millis(200), rx.recv())
        .await
        .expect("no event")
        .expect("closed");
    match evt {
        DomainEvent::CodingMemoryUpdated {
            kind,
            id: emitted_id,
        } => {
            assert_eq!(kind, CodingMemoryKind::Episode);
            assert_eq!(emitted_id, id);
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[tokio::test]
async fn no_bus_attached_is_silent() {
    // Regression: constructing without `with_event_bus` must not panic
    // and must not deadlock.
    let bus = Arc::new(DomainEventBus::new(16));
    let mut rx = bus.subscribe();
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();
    let inner = pool.inner().clone();
    let ingest = Arc::new(coding_ingest::store::IngestEventLogRepo::new(inner.clone()));
    let fact = cognitive::SemanticFactRepo::new(inner.clone());
    let ep = cognitive::EpisodicMemoryRepo::new(inner.clone());
    let writer = coding_memory::distiller::DistillerWriter::new(fact.clone(), ep);
    let retriever: Arc<dyn context_engine::MemoryRetriever> =
        Arc::new(cognitive::UnifiedMemoryService::new(fact));
    let d = coding_memory::distiller::Distiller::new(
        coding_memory::distiller::DistillerConfig::default(),
        ingest,
        writer,
        Arc::new(providers::ProviderManager::new(
            Arc::new(providers::NoopProvider),
            None,
            None,
        )),
        retriever,
    );
    let _ = d.write_fact_for_test("session-z", "fact").await.unwrap();

    // A short window to confirm nothing arrived on the foreign bus.
    let res = timeout(Duration::from_millis(80), rx.recv()).await;
    assert!(res.is_err(), "no event should arrive when bus not attached");
}
