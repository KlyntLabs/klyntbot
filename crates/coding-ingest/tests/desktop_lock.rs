use coding_ingest::desktop_lock::{is_desktop_alive, write_heartbeat};
use tempfile::TempDir;

#[tokio::test]
async fn missing_lock_is_dead() {
    let dir = TempDir::new().unwrap();
    assert!(!is_desktop_alive(&dir.path().join("desktop.lock")));
}

#[tokio::test]
async fn fresh_heartbeat_is_alive() {
    let dir = TempDir::new().unwrap();
    let lock = dir.path().join("desktop.lock");
    write_heartbeat(&lock).await.unwrap();
    assert!(is_desktop_alive(&lock));
}

#[tokio::test]
async fn stale_heartbeat_is_dead() {
    let dir = TempDir::new().unwrap();
    let lock = dir.path().join("desktop.lock");
    write_heartbeat(&lock).await.unwrap();
    let past = std::time::SystemTime::now() - std::time::Duration::from_secs(120);
    let ft = filetime::FileTime::from_system_time(past);
    filetime::set_file_mtime(&lock, ft).unwrap();
    assert!(!is_desktop_alive(&lock));
}
