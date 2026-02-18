//! Tests for ConfigWatcher — file system watcher for hot-reload.

mod common;
use common::create_test_bus;

use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::{sleep, timeout};

use dashboard::config_watcher::ConfigWatcher;
use dashboard::events::DashboardEvent;

/// Helper: write valid JSON to the config file.
async fn write_valid_config(path: &std::path::Path) {
    let json = r#"{"provider": "anthropic", "model": "claude-3"}"#;
    tokio::fs::write(path, json).await.unwrap();
}

/// Helper: write invalid JSON to the config file.
async fn write_invalid_config(path: &std::path::Path) {
    tokio::fs::write(path, "not { valid json").await.unwrap();
}

#[tokio::test]
async fn test_config_watcher_detects_file_change() {
    let tmp = TempDir::new().unwrap();
    let config_path = tmp.path().join("config.json");

    // Write initial valid config.
    write_valid_config(&config_path).await;

    let bus = create_test_bus();
    let mut rx = bus.subscribe();

    let watcher = ConfigWatcher::new(
        config_path.clone(),
        bus.clone(),
        Duration::from_millis(50), // fast debounce for tests
    );
    let _handle = watcher.start();

    // Give the watcher time to initialize.
    sleep(Duration::from_millis(100)).await;

    // Modify the config file.
    write_valid_config(&config_path).await;

    // Should receive ConfigReloaded within 2 seconds.
    let event = timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("Timed out waiting for ConfigReloaded event")
        .expect("Channel closed");

    assert!(
        matches!(event, DashboardEvent::ConfigReloaded),
        "Expected ConfigReloaded, got {:?}",
        event
    );
}

#[tokio::test]
async fn test_config_watcher_validates_before_emitting() {
    let tmp = TempDir::new().unwrap();
    let config_path = tmp.path().join("config.json");

    write_valid_config(&config_path).await;

    let bus = create_test_bus();
    let mut rx = bus.subscribe();

    let watcher = ConfigWatcher::new(
        config_path.clone(),
        bus.clone(),
        Duration::from_millis(50),
    );
    let _handle = watcher.start();

    sleep(Duration::from_millis(100)).await;

    // Write invalid JSON — should NOT emit ConfigReloaded.
    write_invalid_config(&config_path).await;

    // Wait long enough to be sure no event arrives.
    let result = timeout(Duration::from_millis(500), rx.recv()).await;

    assert!(
        result.is_err(),
        "Should NOT receive ConfigReloaded for invalid JSON"
    );
}

#[tokio::test]
async fn test_config_watcher_debounces_rapid_changes() {
    let tmp = TempDir::new().unwrap();
    let config_path = tmp.path().join("config.json");

    write_valid_config(&config_path).await;

    let bus = create_test_bus();
    let mut rx = bus.subscribe();

    // Large debounce window so rapid writes are coalesced.
    let watcher = ConfigWatcher::new(
        config_path.clone(),
        bus.clone(),
        Duration::from_millis(300),
    );
    let _handle = watcher.start();

    sleep(Duration::from_millis(100)).await;

    // Write 5 times rapidly.
    for _ in 0..5 {
        write_valid_config(&config_path).await;
        sleep(Duration::from_millis(20)).await;
    }

    // Collect events for 1 second.
    let mut event_count = 0;
    let collect_deadline = tokio::time::Instant::now() + Duration::from_secs(1);
    loop {
        let remaining = collect_deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        if timeout(remaining, rx.recv()).await.is_ok() {
            event_count += 1;
        } else {
            break;
        }
    }

    // Due to debouncing, we expect far fewer than 5 events (1 or 2).
    assert!(
        event_count <= 2,
        "Expected ≤2 events after debounce, got {}",
        event_count
    );
}

#[tokio::test]
async fn test_config_watcher_ignores_non_config_files() {
    let tmp = TempDir::new().unwrap();
    let config_path = tmp.path().join("config.json");
    let other_path = tmp.path().join("other.txt");

    write_valid_config(&config_path).await;

    let bus = create_test_bus();
    let mut rx = bus.subscribe();

    let watcher = ConfigWatcher::new(
        config_path.clone(),
        bus.clone(),
        Duration::from_millis(50),
    );
    let _handle = watcher.start();

    sleep(Duration::from_millis(100)).await;

    // Write to a different file in the same directory.
    tokio::fs::write(&other_path, "hello").await.unwrap();

    // No event should be emitted.
    let result = timeout(Duration::from_millis(500), rx.recv()).await;
    assert!(
        result.is_err(),
        "Should NOT receive event when non-config file changes"
    );
}
