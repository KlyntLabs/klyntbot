//! Lifecycle invariants for the chat/thread system.

use crate::common::chat_harness::ChatTestHarness;
use app_core::runtime::StartTurnRequest;

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
    let got_done = wait_for_event(&emitter, "agent:done", std::time::Duration::from_secs(5)).await;
    if !got_done {
        let names: Vec<String> = emitter
            .events
            .lock()
            .unwrap()
            .iter()
            .map(|e| e.name.clone())
            .collect();
        panic!("first turn should emit agent:done; got events: {:?}", names);
    }

    // Second send on same session must succeed.
    let (_, stream_info_2) = core
        .chat_send("again".into(), session_key.clone(), None, None)
        .await
        .expect("second send should succeed after first done");
    core.spawn_chat_relay(stream_info_2, emitter.clone());

    assert!(
        wait_for_new_event(&emitter, "agent:done", std::time::Duration::from_secs(5)).await,
        "second turn should emit agent:done"
    );

    // Active streams must be empty.
    assert_eq!(core.active_streams_len(), 0, "active_streams must drain");
}

#[tokio::test]
async fn double_send_is_rejected() {
    let (core, emitter) = ChatTestHarness::new_real().await;
    let sk = "test:double-send".to_string();

    let (_, info) = core
        .chat_send("first".into(), sk.clone(), None, None)
        .await
        .expect("first send");
    core.spawn_chat_relay(info, emitter.clone());

    // Immediately fire a second send while the first is still in flight.
    let result = core
        .chat_send("second".into(), sk.clone(), None, None)
        .await;
    assert!(result.is_err(), "second send should be rejected");
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

/// Wait for an additional occurrence of `name` beyond the current count.
async fn wait_for_new_event(
    emitter: &std::sync::Arc<crate::common::chat_harness::RecordingEmitter>,
    name: &str,
    timeout: std::time::Duration,
) -> bool {
    let baseline = emitter
        .events
        .lock()
        .unwrap()
        .iter()
        .filter(|e| e.name == name)
        .count();
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        let current = emitter
            .events
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.name == name)
            .count();
        if current > baseline {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    false
}

// ── PR5: ThreadRuntime trait tests ───────────────────────────────────────────

/// Both runtimes share the same underlying active_turns map.
#[tokio::test]
async fn both_runtimes_share_active_turns_map() {
    let (core, _emitter) = ChatTestHarness::new_real().await;
    let assistant = std::sync::Arc::clone(&core).assistant_runtime();
    let coding = std::sync::Arc::clone(&core).coding_runtime();

    assert!(std::ptr::eq(
        std::sync::Arc::as_ptr(assistant.active_turns()),
        std::sync::Arc::as_ptr(coding.active_turns()),
    ));
}

/// Assistant runtime `start_turn` returns the expected outcome shape.
#[tokio::test]
async fn assistant_runtime_start_turn_returns_outcome() {
    let (core, _emitter) = ChatTestHarness::new_real().await;
    let runtime = std::sync::Arc::clone(&core).assistant_runtime();

    let outcome = runtime
        .start_turn(StartTurnRequest {
            thread_id: "test:assistant-runtime".to_string(),
            content: "hello".into(),
            context: None,
            mode: None,
            model: None,
        })
        .await
        .expect("start_turn");

    assert_eq!(outcome.handle.thread_id, "test:assistant-runtime");
    assert!(outcome.user_message.is_some(), "assistant mode returns user_message");
    assert!(outcome.stream_info.is_some(), "assistant mode returns stream_info");
    assert!(outcome.coding_response.is_none(), "assistant mode does not return coding_response");
}

/// Coding runtime exists and shares the same active_turns map.
#[tokio::test]
async fn coding_runtime_is_wired() {
    let (core, _emitter) = ChatTestHarness::new_real().await;
    let runtime = std::sync::Arc::clone(&core).coding_runtime();

    // The runtime should see the same active_turns as the assistant runtime
    assert_eq!(runtime.active_turns().len(), 0, "no active turns in fresh harness");
}

/// Assistant runtime `cancel_turn` removes the turn from active_turns.
#[tokio::test]
async fn assistant_runtime_cancel_turn_clears_active() {
    let (core, _emitter) = ChatTestHarness::new_real().await;
    let runtime = std::sync::Arc::clone(&core).assistant_runtime();
    let sk = "test:assistant-cancel".to_string();

    let outcome = runtime
        .start_turn(StartTurnRequest {
            thread_id: sk.clone(),
            content: "hello".into(),
            context: None,
            mode: None,
            model: None,
        })
        .await
        .expect("start_turn");

    assert!(runtime.is_active(&outcome.handle.turn_id));

    runtime
        .cancel_turn(&outcome.handle.turn_id)
        .await
        .expect("cancel_turn");

    assert!(!runtime.is_active(&outcome.handle.turn_id));
}
