//! Tests for DashboardEventBus — the broadcast event distribution system.

mod common;

use std::sync::Arc;
use tokio::time::{timeout, Duration};

use dashboard::events::{ChangeAction, DashboardEvent, DashboardEventBus};
use agent::AgentEvent;

#[tokio::test]
async fn test_event_bus_publish_and_receive() {
    let bus = Arc::new(DashboardEventBus::new(64));
    let mut rx = bus.subscribe();

    bus.publish(DashboardEvent::TodoChanged {
        action: ChangeAction::Created,
        id: "todo-1".to_string(),
    });

    let received = timeout(Duration::from_secs(1), rx.recv())
        .await
        .expect("Timed out waiting for event")
        .expect("Channel closed");

    match received {
        DashboardEvent::TodoChanged { action, id } => {
            assert_eq!(action, ChangeAction::Created);
            assert_eq!(id, "todo-1");
        }
        _ => panic!("Unexpected event type"),
    }
}

#[tokio::test]
async fn test_event_bus_multiple_event_types() {
    let bus = Arc::new(DashboardEventBus::new(64));
    let mut rx = bus.subscribe();

    bus.publish(DashboardEvent::TodoChanged {
        action: ChangeAction::Created,
        id: "todo-1".to_string(),
    });
    bus.publish(DashboardEvent::ProjectChanged {
        action: ChangeAction::Updated,
        id: "proj-1".to_string(),
    });
    bus.publish(DashboardEvent::Notification {
        title: "Test".to_string(),
        body: "Hello".to_string(),
    });

    // Should receive all 3 events in order.
    let ev1 = timeout(Duration::from_secs(1), rx.recv()).await.unwrap().unwrap();
    assert!(matches!(ev1, DashboardEvent::TodoChanged { .. }));

    let ev2 = timeout(Duration::from_secs(1), rx.recv()).await.unwrap().unwrap();
    assert!(matches!(ev2, DashboardEvent::ProjectChanged { .. }));

    let ev3 = timeout(Duration::from_secs(1), rx.recv()).await.unwrap().unwrap();
    assert!(matches!(ev3, DashboardEvent::Notification { .. }));
}

#[tokio::test]
async fn test_event_bus_multiple_subscribers() {
    let bus = Arc::new(DashboardEventBus::new(64));

    let mut receivers: Vec<_> = (0..5).map(|_| bus.subscribe()).collect();

    bus.publish(DashboardEvent::ConfigReloaded);

    for rx in &mut receivers {
        let ev = timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("Timed out")
            .expect("Channel closed");
        assert!(matches!(ev, DashboardEvent::ConfigReloaded));
    }
}

#[tokio::test]
async fn test_event_bus_subscriber_drop_does_not_affect_others() {
    let bus = Arc::new(DashboardEventBus::new(64));

    let mut rx1 = bus.subscribe();
    let mut rx2 = bus.subscribe();
    let rx3 = bus.subscribe(); // will be dropped

    drop(rx3); // drop one subscriber

    bus.publish(DashboardEvent::ConfigReloaded);

    // Both remaining subscribers still receive the event.
    let ev1 = timeout(Duration::from_secs(1), rx1.recv()).await.unwrap().unwrap();
    assert!(matches!(ev1, DashboardEvent::ConfigReloaded));

    let ev2 = timeout(Duration::from_secs(1), rx2.recv()).await.unwrap().unwrap();
    assert!(matches!(ev2, DashboardEvent::ConfigReloaded));
}

#[tokio::test]
async fn test_event_bus_agent_event_forwarding() {
    let bus = Arc::new(DashboardEventBus::new(64));
    let mut rx = bus.subscribe();

    // Simulate forwarding an AgentEvent to the dashboard bus.
    let agent_ev = AgentEvent::ContentChunk("Hello!".to_string());
    bus.publish(DashboardEvent::AgentEvent(agent_ev));

    let received = timeout(Duration::from_secs(1), rx.recv())
        .await
        .expect("Timed out")
        .expect("Channel closed");

    match received {
        DashboardEvent::AgentEvent(AgentEvent::ContentChunk(content)) => {
            assert_eq!(content, "Hello!");
        }
        _ => panic!("Expected AgentEvent(ContentChunk), got {:?}", received),
    }
}

#[tokio::test]
async fn test_event_bus_config_reloaded_event() {
    let bus = Arc::new(DashboardEventBus::new(64));
    let mut rx = bus.subscribe();

    bus.publish(DashboardEvent::ConfigReloaded);

    let received = timeout(Duration::from_secs(1), rx.recv())
        .await
        .expect("Timed out")
        .expect("Channel closed");

    assert!(
        matches!(received, DashboardEvent::ConfigReloaded),
        "Expected ConfigReloaded, got {:?}",
        received
    );
}
