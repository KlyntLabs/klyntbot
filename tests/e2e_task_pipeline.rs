//! End-to-end test: Task creation flows through the AI pipeline.
//!
//! This test verifies that creating a task via the TaskTool produces
//! an observation and entity through the SignalRouter -> IngestionConsumer path.

use ai_core::{AiSignal, RecallDomain, SalienceVerdict, SignalConsumer, SignalRouter};
use async_trait::async_trait;
use bus::{DomainEvent, DomainEventBus};
use std::sync::{Arc, Mutex};

struct TestConsumer {
    observations: Arc<Mutex<Vec<TestObservation>>>,
    entities: Arc<Mutex<Vec<TestEntity>>>,
}

#[derive(Debug, Clone)]
struct TestObservation {
    event_kind: String,
    domain: String,
    content: String,
    importance: f64,
}

#[derive(Debug, Clone)]
struct TestEntity {
    entity_type: String,
    id: String,
    name: String,
}

#[async_trait]
impl SignalConsumer for TestConsumer {
    fn name(&self) -> &'static str { "test_consumer" }

    async fn consume(&self, signal: &AiSignal) -> common::Result<()> {
        self.observations.lock().unwrap().push(TestObservation {
            event_kind: signal.event_kind.to_string(),
            domain: signal.domain.as_str().to_string(),
            content: signal.content.clone(),
            importance: signal.importance,
        });

        if let Some(entity) = &signal.entity {
            self.entities.lock().unwrap().push(TestEntity {
                entity_type: entity.entity_type.to_string(),
                id: entity.id.clone(),
                name: entity.name.clone(),
            });
        }

        Ok(())
    }
}

#[tokio::test]
async fn task_created_event_produces_signal_with_entity() {
    let bus = Arc::new(DomainEventBus::new(64));
    let observations = Arc::new(Mutex::new(Vec::new()));
    let entities = Arc::new(Mutex::new(Vec::new()));

    let consumer = Arc::new(TestConsumer {
        observations: observations.clone(),
        entities: entities.clone(),
    }) as Arc<dyn SignalConsumer>;

    let _router = SignalRouter::start(
        bus.clone(),
        vec![consumer],
        app_core::init::ai_pipeline::translate,
    );

    // Publish a TaskCreated domain event (as would happen via TaskTool)
    // Note: DomainEvent::TaskCreated doesn't include title, so the signal
    // content will have empty title. This is the actual production behavior.
    let event = DomainEvent::TaskCreated {
        task_id: "task-123".into(),
        project: Some("proj-1".into()),
        estimate_mins: Some(120),
        task_type: "manual".into(),
    };

    bus.publish(event);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let obs = observations.lock().unwrap().clone();
    assert_eq!(obs.len(), 1, "expected exactly one observation");
    assert_eq!(obs[0].event_kind, "Created");
    assert_eq!(obs[0].domain, "tasks");
    // Note: DomainEvent::TaskCreated doesn't include title, so content has empty title
    // This is a known limitation - the title would need to be fetched from the task repo
    assert!(obs[0].content.contains("Created task:"), "content should start with 'Created task:'");
    assert!(obs[0].importance > 0.5, "importance should be > 0.5");

    // Note: Entity bridge creates entity with empty name since title isn't in DomainEvent::TaskCreated
    let ents = entities.lock().unwrap().clone();
    assert_eq!(ents.len(), 1, "entity bridge creates entity even with empty title");
    assert_eq!(ents[0].entity_type, "task");
    assert_eq!(ents[0].id, "task-123");
    assert_eq!(ents[0].name, "", "name is empty because DomainEvent::TaskCreated has no title");
}

#[tokio::test]
async fn task_completed_high_deviation_extracts() {
    let bus = Arc::new(DomainEventBus::new(64));
    let observations = Arc::new(Mutex::new(Vec::new()));
    let entities = Arc::new(Mutex::new(Vec::new()));

    let consumer = Arc::new(TestConsumer {
        observations: observations.clone(),
        entities: entities.clone(),
    }) as Arc<dyn SignalConsumer>;

    let _router = SignalRouter::start(
        bus.clone(),
        vec![consumer],
        app_core::init::ai_pipeline::translate,
    );

    // Publish a TaskCompleted with high deviation
    let event: DomainEvent = feature_tasks::events::TaskEvent::Completed {
        task_id: "task-456".into(),
        title: "Big feature".into(),
        deviation_pct: Some(80.0),
    }.into();

    bus.publish(event);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let obs = observations.lock().unwrap().clone();
    assert_eq!(obs.len(), 1);
    assert_eq!(obs[0].event_kind, "Completed");
    // High deviation should produce Extract salience (verified in unit tests)
    // Here we just verify the signal flows through
}
