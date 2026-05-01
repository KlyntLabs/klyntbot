use coding_memory::sink::{translator::RuntimeEvent, MemorySinkSubscriber, RecordingSink};

#[tokio::test]
async fn e3_n_content_chunks_aggregate_to_one_assistant_msg() {
    let sink = RecordingSink::new();
    let sub = MemorySinkSubscriber::for_test(sink.clone());
    sub.translate(RuntimeEvent::IterationStart).await.unwrap();
    for chunk in &["Hello ", "world", "!"] {
        sub.translate(RuntimeEvent::ContentChunk {
            text: chunk.to_string(),
        })
        .await
        .unwrap();
    }
    sub.translate(RuntimeEvent::IterationEnd).await.unwrap();

    let evts = sink.events_for_test();
    let msgs: Vec<_> = evts
        .iter()
        .filter_map(|e| match e {
            coding_ingest::event::AgentEvent::V1(coding_ingest::event::AgentEventV1 {
                kind: coding_ingest::event::EventKind::AssistantMsg { text, .. },
                ..
            }) => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(msgs, vec!["Hello world!".to_string()]);
}
