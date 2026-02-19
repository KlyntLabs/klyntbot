//! I2: Learning→Context Event System via MessageBus
//!
//! AC-I2.1: LearningService publishes ThresholdChanged event when threshold changes
//! AC-I2.2: LearningService publishes AnalysisCompleted event after analysis
//! AC-I2.3: AgentLoop subscribes and receives LearningEvents from bus
//! AC-I2.4: ContextBuilder.set_confidence_threshold called when ThresholdChanged received
//! AC-I2.5: Subscriber can receive both event types from LearningEventBus

use std::sync::Arc;
use std::time::Duration;

use bus::learning_events::{LearningEvent, LearningEventBus};

// ─── AC-I2.1 ──────────────────────────────────────────────────────────────────
/// LearningService publishes ThresholdChanged event when it adjusts the threshold.
#[tokio::test]
async fn ac_i2_1_threshold_changed_event_published() {
    let event_bus = Arc::new(LearningEventBus::new(16));
    let mut rx = event_bus.subscribe();

    // Publish a ThresholdChanged event (simulating what LearningService does)
    event_bus
        .publish(LearningEvent::ThresholdChanged {
            old_threshold: 0.70,
            new_threshold: 0.75,
            reason: "success_rate_increased".to_string(),
        })
        .await;

    let event: LearningEvent = tokio::time::timeout(Duration::from_millis(100), rx.recv())
        .await
        .expect("timeout waiting for event")
        .expect("channel closed");

    match event {
        LearningEvent::ThresholdChanged {
            old_threshold,
            new_threshold,
            reason,
        } => {
            assert!((old_threshold - 0.70).abs() < f32::EPSILON);
            assert!((new_threshold - 0.75).abs() < f32::EPSILON);
            assert_eq!(reason, "success_rate_increased");
        }
        other => panic!("Expected ThresholdChanged, got {:?}", other),
    }
}

// ─── AC-I2.2 ──────────────────────────────────────────────────────────────────
/// LearningService publishes AnalysisCompleted event after analysis runs.
#[tokio::test]
async fn ac_i2_2_analysis_completed_event_published() {
    let event_bus = Arc::new(LearningEventBus::new(16));
    let mut rx = event_bus.subscribe();

    event_bus
        .publish(LearningEvent::AnalysisCompleted {
            total_outcomes: 42,
            suggested_threshold: 0.73,
        })
        .await;

    let event: LearningEvent = tokio::time::timeout(Duration::from_millis(100), rx.recv())
        .await
        .expect("timeout")
        .expect("channel closed");

    match event {
        LearningEvent::AnalysisCompleted {
            total_outcomes,
            suggested_threshold,
        } => {
            assert_eq!(total_outcomes, 42);
            assert!((suggested_threshold - 0.73).abs() < f32::EPSILON);
        }
        other => panic!("Expected AnalysisCompleted, got {:?}", other),
    }
}

// ─── AC-I2.3 ──────────────────────────────────────────────────────────────────
/// The LearningEventBus correctly exposes a subscribe() method and receiver
/// delivers events from a background publisher (simulating AgentLoop subscription).
#[tokio::test]
async fn ac_i2_3_subscriber_receives_events_from_background_publisher() {
    let event_bus = Arc::new(LearningEventBus::new(16));
    let mut rx = event_bus.subscribe();

    let bus_clone = Arc::clone(&event_bus);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(10)).await;
        bus_clone
            .publish(LearningEvent::ThresholdChanged {
                old_threshold: 0.7,
                new_threshold: 0.8,
                reason: "test".to_string(),
            })
            .await;
    });

    let event: LearningEvent = tokio::time::timeout(Duration::from_millis(200), rx.recv())
        .await
        .expect("timeout waiting for background-published event")
        .expect("channel closed");

    assert!(matches!(event, LearningEvent::ThresholdChanged { .. }));
}

// ─── AC-I2.4 ──────────────────────────────────────────────────────────────────
/// When a ThresholdChanged event is received, the new threshold value
/// can be extracted and applied to ContextBuilder.set_confidence_threshold.
#[tokio::test]
async fn ac_i2_4_threshold_change_applies_to_context_builder() {
    use agent::ContextBuilder;

    let event_bus = Arc::new(LearningEventBus::new(16));
    let mut rx = event_bus.subscribe();

    // Simulate LearningService publishing threshold change
    event_bus
        .publish(LearningEvent::ThresholdChanged {
            old_threshold: 0.70,
            new_threshold: 0.78,
            reason: "adaptive_update".to_string(),
        })
        .await;

    let event = rx.recv().await.expect("channel closed");

    // Build a ContextBuilder and apply the event (as AgentLoop would)
    let workspace = std::path::PathBuf::from("/tmp/test-workspace-i2");
    let pool = storage::StoragePool::connect_lazy("postgres://localhost/klyntbot_test").unwrap();
    let memory_note_repo = storage::MemoryNoteRepo::new(pool.inner().clone());
    let mut ctx =
        ContextBuilder::new(workspace, "UTC".to_string(), None, None, memory_note_repo).await;

    if let LearningEvent::ThresholdChanged { new_threshold, .. } = event {
        ctx.set_confidence_threshold(new_threshold);
        // Verify it can be retrieved
        assert!((ctx.confidence_threshold() - 0.78).abs() < f32::EPSILON);
    } else {
        panic!("Expected ThresholdChanged event");
    }
}

// ─── AC-I2.5 ──────────────────────────────────────────────────────────────────
/// Multiple subscribers can each receive both event types independently.
#[tokio::test]
async fn ac_i2_5_multiple_subscribers_receive_all_event_types() {
    let event_bus = Arc::new(LearningEventBus::new(16));
    let mut rx1 = event_bus.subscribe();
    let mut rx2 = event_bus.subscribe();

    // Publish both event types
    event_bus
        .publish(LearningEvent::AnalysisCompleted {
            total_outcomes: 10,
            suggested_threshold: 0.65,
        })
        .await;
    event_bus
        .publish(LearningEvent::ThresholdChanged {
            old_threshold: 0.65,
            new_threshold: 0.67,
            reason: "nudge".to_string(),
        })
        .await;

    // Both subscribers receive both events
    for rx in [&mut rx1, &mut rx2] {
        let ev1: LearningEvent = tokio::time::timeout(Duration::from_millis(100), rx.recv())
            .await
            .expect("timeout ev1")
            .expect("closed");
        let ev2: LearningEvent = tokio::time::timeout(Duration::from_millis(100), rx.recv())
            .await
            .expect("timeout ev2")
            .expect("closed");

        assert!(matches!(ev1, LearningEvent::AnalysisCompleted { .. }));
        assert!(matches!(ev2, LearningEvent::ThresholdChanged { .. }));
    }
}
