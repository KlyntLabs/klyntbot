use coding_memory::sink::{translator::RuntimeEvent, MemorySinkSubscriber, RecordingSink};

#[tokio::test]
async fn e4_tool_start_and_end_pair_to_one_tool_call() {
    let sink = RecordingSink::new();
    let sub = MemorySinkSubscriber::for_test(sink.clone());
    sub.translate(RuntimeEvent::ToolStart {
        call_id: "abc".into(),
        name: "bash".into(),
        args: serde_json::json!({"command": "ls"}),
    })
    .await
    .unwrap();
    sub.translate(RuntimeEvent::ToolEnd {
        call_id: "abc".into(),
        success: true,
        output: "file.txt\n".into(),
        duration_ms: 12,
    })
    .await
    .unwrap();

    let evts = sink.events_for_test();
    let count = evts
        .iter()
        .filter(|e| {
            matches!(
                e,
                coding_ingest::event::AgentEvent::V1(coding_ingest::event::AgentEventV1 {
                    kind: coding_ingest::event::EventKind::ToolCall { .. },
                    ..
                })
            )
        })
        .count();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn e4_orphan_tool_start_emits_nothing() {
    let sink = RecordingSink::new();
    let sub = MemorySinkSubscriber::for_test(sink.clone());
    sub.translate(RuntimeEvent::ToolStart {
        call_id: "orphan".into(),
        name: "bash".into(),
        args: serde_json::json!({}),
    })
    .await
    .unwrap();

    let evts = sink.events_for_test();
    assert!(evts.iter().all(|e| {
        !matches!(
            e,
            coding_ingest::event::AgentEvent::V1(coding_ingest::event::AgentEventV1 {
                kind: coding_ingest::event::EventKind::ToolCall { .. },
                ..
            })
        )
    }));
}
