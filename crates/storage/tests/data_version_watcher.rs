//! Phase 4 integration test: `start_data_version_watcher` fires
//! `DomainEvent::DataVersionBumped` when *another* connection mutates
//! the database.

use bus::{DomainEvent, DomainEventBus};
use std::sync::Arc;
use std::time::Duration;
use storage::StoragePool;
use tokio::time::timeout;

async fn open_shared_pool(path: &std::path::Path) -> sqlx::SqlitePool {
    sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true)
                .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal),
        )
        .await
        .expect("open second pool")
}

#[tokio::test]
async fn watcher_fires_when_other_pool_writes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("data.db");
    // Pre-create the file so both pools open the same DB.
    std::fs::File::create(&path).unwrap();

    // Pool A: the watcher's pool.
    let pool_a = StoragePool::from_existing(open_shared_pool(&path).await);
    // Create a benign table so subsequent writes have somewhere to go.
    sqlx::query("CREATE TABLE IF NOT EXISTS t (x INTEGER)")
        .execute(pool_a.inner())
        .await
        .unwrap();

    let bus = Arc::new(DomainEventBus::new(8));
    let mut rx = bus.subscribe();
    let _token = pool_a.start_data_version_watcher(bus.clone(), Duration::from_millis(50));

    // Yield long enough for the watcher to read its initial baseline.
    tokio::time::sleep(Duration::from_millis(120)).await;

    // Pool B: simulates the MCP child process writing to the same file.
    let pool_b = open_shared_pool(&path).await;
    sqlx::query("INSERT INTO t (x) VALUES (1)")
        .execute(&pool_b)
        .await
        .unwrap();

    let evt = timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("watcher did not fire within 2 s")
        .expect("bus closed");
    match evt {
        DomainEvent::DataVersionBumped { previous, current } => {
            assert!(
                current > previous,
                "current ({current}) should exceed previous ({previous})"
            );
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[tokio::test]
async fn watcher_does_not_fire_without_writes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("idle.db");
    std::fs::File::create(&path).unwrap();

    let pool = StoragePool::from_existing(open_shared_pool(&path).await);
    sqlx::query("CREATE TABLE t (x INTEGER)")
        .execute(pool.inner())
        .await
        .unwrap();

    let bus = Arc::new(DomainEventBus::new(8));
    let mut rx = bus.subscribe();
    let _token = pool.start_data_version_watcher(bus.clone(), Duration::from_millis(50));

    let res = timeout(Duration::from_millis(300), rx.recv()).await;
    assert!(res.is_err(), "watcher fired despite no writes: {res:?}");
}

#[tokio::test]
async fn cancelling_token_stops_the_watcher() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cancel.db");
    std::fs::File::create(&path).unwrap();

    let pool = StoragePool::from_existing(open_shared_pool(&path).await);
    sqlx::query("CREATE TABLE t (x INTEGER)")
        .execute(pool.inner())
        .await
        .unwrap();

    let bus = Arc::new(DomainEventBus::new(8));
    let mut rx = bus.subscribe();
    let _token = pool.start_data_version_watcher(bus.clone(), Duration::from_millis(50));
    // Dropping the handle cancels the watcher.
    drop(_token);
    tokio::time::sleep(Duration::from_millis(120)).await;

    // Now write — the watcher should have already exited.
    let pool_b = open_shared_pool(&path).await;
    sqlx::query("INSERT INTO t (x) VALUES (1)")
        .execute(&pool_b)
        .await
        .unwrap();

    let res = timeout(Duration::from_millis(300), rx.recv()).await;
    assert!(res.is_err(), "watcher fired after cancel: {res:?}");
}
