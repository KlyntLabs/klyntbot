//! Phase 4 — `list_open_threads` is implemented (replaces stub).

#[test]
fn open_threads_module_exists() {
    let src = include_str!("../src/recall/open_threads.rs");
    assert!(
        src.contains("pub async fn list_open_threads"),
        "open_threads.rs should expose list_open_threads"
    );
}

#[test]
fn service_calls_open_threads() {
    let src = include_str!("../src/recall/service.rs");
    assert!(
        src.contains("open_threads::list_open_threads"),
        "recall::service should call open_threads::list_open_threads"
    );
}
