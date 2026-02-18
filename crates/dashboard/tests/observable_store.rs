//! Tests for ObservableTodoStore — the event-emitting wrapper around TodoStore.

mod common;
use common::{create_observable_store, create_test_bus, make_todo};

use std::sync::Arc;
use tempfile::TempDir;
use tokio::time::{timeout, Duration};

use dashboard::events::{ChangeAction, DashboardEvent};
use tools::todo_types::{TodoPatch, TodoStatus};

#[tokio::test]
async fn test_observable_store_add_emits_created_event() {
    let tmp = TempDir::new().unwrap();
    let bus = create_test_bus();
    let mut rx = bus.subscribe();
    let store = create_observable_store(&tmp, bus);

    let todo = make_todo("Buy groceries");
    store.add(todo).await.unwrap();

    let event = timeout(Duration::from_secs(1), rx.recv())
        .await
        .expect("Timed out")
        .expect("Channel closed");

    match event {
        DashboardEvent::TodoChanged { action, .. } => {
            assert_eq!(action, ChangeAction::Created);
        }
        _ => panic!("Expected TodoChanged(Created), got {:?}", event),
    }
}

#[tokio::test]
async fn test_observable_store_update_emits_updated_event() {
    let tmp = TempDir::new().unwrap();
    let bus = create_test_bus();
    let store = create_observable_store(&tmp, bus.clone());

    let todo = make_todo("Buy groceries");
    let created = store.add(todo).await.unwrap();

    // Subscribe AFTER the add so we only see the update event.
    let mut rx = bus.subscribe();

    let patch = TodoPatch {
        title: Some("Buy milk".to_string()),
        ..Default::default()
    };
    store.update(&created.id, patch).await.unwrap();

    let event = timeout(Duration::from_secs(1), rx.recv())
        .await
        .expect("Timed out")
        .expect("Channel closed");

    match event {
        DashboardEvent::TodoChanged { action, id } => {
            assert_eq!(action, ChangeAction::Updated);
            assert_eq!(id, created.id);
        }
        _ => panic!("Expected TodoChanged(Updated), got {:?}", event),
    }
}

#[tokio::test]
async fn test_observable_store_delete_emits_deleted_event() {
    let tmp = TempDir::new().unwrap();
    let bus = create_test_bus();
    let store = create_observable_store(&tmp, bus.clone());

    let todo = make_todo("Temporary task");
    let created = store.add(todo).await.unwrap();

    let mut rx = bus.subscribe();

    store.delete(&created.id).await.unwrap();

    let event = timeout(Duration::from_secs(1), rx.recv())
        .await
        .expect("Timed out")
        .expect("Channel closed");

    match event {
        DashboardEvent::TodoChanged { action, id } => {
            assert_eq!(action, ChangeAction::Deleted);
            assert_eq!(id, created.id);
        }
        _ => panic!("Expected TodoChanged(Deleted), got {:?}", event),
    }
}

#[tokio::test]
async fn test_observable_store_delegates_to_inner_store() {
    let tmp = TempDir::new().unwrap();
    let bus = create_test_bus();
    let store = create_observable_store(&tmp, bus);

    let todo = make_todo("Write tests");
    let created = store.add(todo).await.unwrap();

    // Read back via the observable store (which delegates to inner).
    let fetched = store.get(&created.id).await.unwrap();
    assert!(fetched.is_some());
    assert_eq!(fetched.unwrap().title, "Write tests");
}

#[tokio::test]
async fn test_observable_store_event_failure_does_not_block_write() {
    // With no active receivers, the broadcast send will fail with "no receivers"
    // but the write should still succeed.
    let tmp = TempDir::new().unwrap();
    let bus = create_test_bus();
    // Don't subscribe — no receivers.
    let store = create_observable_store(&tmp, bus);

    let todo = make_todo("No subscribers");
    let result = store.add(todo).await;
    assert!(
        result.is_ok(),
        "Write should succeed even with no subscribers"
    );
}

#[tokio::test]
async fn test_observable_store_complete_emits_updated_event() {
    let tmp = TempDir::new().unwrap();
    let bus = create_test_bus();
    let store = create_observable_store(&tmp, bus.clone());

    // Seed a Doing todo.
    let mut todo = make_todo("Fix bug");
    todo.status = TodoStatus::Doing;
    let created = store.add(todo).await.unwrap();

    let mut rx = bus.subscribe();

    // Complete it.
    let completed = store.complete(&created.id).await.unwrap();
    assert!(completed.is_some());
    assert_eq!(completed.unwrap().status, TodoStatus::Done);

    let event = timeout(Duration::from_secs(1), rx.recv())
        .await
        .expect("Timed out")
        .expect("Channel closed");

    match event {
        DashboardEvent::TodoChanged { action, .. } => {
            assert_eq!(action, ChangeAction::Updated);
        }
        _ => panic!(
            "Expected TodoChanged(Updated) from complete, got {:?}",
            event
        ),
    }
}

#[tokio::test]
async fn test_observable_store_focus_emits_updated_event() {
    let tmp = TempDir::new().unwrap();
    let bus = create_test_bus();
    let store = create_observable_store(&tmp, bus.clone());

    let todo = make_todo("Focus task");
    let created = store.add(todo).await.unwrap();

    let mut rx = bus.subscribe();

    let focused = store.focus(&created.id, 5, 2).await.unwrap();
    assert!(focused);

    let event = timeout(Duration::from_secs(1), rx.recv())
        .await
        .expect("Timed out")
        .expect("Channel closed");

    match event {
        DashboardEvent::TodoChanged { action, .. } => {
            assert_eq!(action, ChangeAction::Updated);
        }
        _ => panic!("Expected TodoChanged(Updated) from focus, got {:?}", event),
    }
}

#[tokio::test]
async fn test_observable_store_multiple_subscribers() {
    let tmp = TempDir::new().unwrap();
    let bus = create_test_bus();
    let store = create_observable_store(&tmp, bus.clone());

    // Create 3 subscribers before writing.
    let mut receivers: Vec<_> = (0..3).map(|_| bus.subscribe()).collect();

    let todo = make_todo("Fan-out test");
    store.add(todo).await.unwrap();

    // All 3 should receive the Created event.
    for rx in &mut receivers {
        let event = timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("Timed out")
            .expect("Channel closed");
        assert!(matches!(
            event,
            DashboardEvent::TodoChanged {
                action: ChangeAction::Created,
                ..
            }
        ));
    }
}
