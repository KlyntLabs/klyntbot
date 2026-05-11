//! Lifecycle invariants for the chat/thread system.

use crate::common::chat_harness::ChatTestHarness;

/// REGRESSION: after a turn reaches Done, a subsequent chat_send must succeed.
#[tokio::test]
async fn back_to_back_send_succeeds() {
    let (core, emitter) = ChatTestHarness::new_real().await;
    let session_key = "test:back-to-back".to_string();

    let (_, stream_info_1) = core
        .chat_send("hello".into(), session_key.clone(), None, None)
        .await
        .expect("first send");
    core.spawn_chat_relay(stream_info_1, emitter.clone());

    // Wait for first turn to complete.
    let _ = wait_for_event(&emitter, "agent:done", std::time::Duration::from_secs(5)).await;

    // Second send on same session must succeed.
    let (_, stream_info_2) = core
        .chat_send("again".into(), session_key.clone(), None, None)
        .await
        .expect("second send should succeed after first done");
    core.spawn_chat_relay(stream_info_2, emitter.clone());

    let _ = wait_for_event(&emitter, "agent:done", std::time::Duration::from_secs(5)).await;

    // Active streams must be empty.
    assert_eq!(core.active_streams_len(), 0, "active_streams must drain");
}

async fn wait_for_event(
    emitter: &std::sync::Arc<crate::common::chat_harness::RecordingEmitter>,
    name: &str,
    timeout: std::time::Duration,
) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if emitter
            .events
            .lock()
            .unwrap()
            .iter()
            .any(|e| e.name == name)
        {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    false
}
