use coding_memory::sink::{translator::RuntimeEvent, MemorySinkSubscriber, RecordingSink};
use std::path::PathBuf;

#[tokio::test]
async fn e1_every_file_edit_with_symbols_produces_one_file_edit_enriched() {
    let sink = RecordingSink::new();
    let sub = MemorySinkSubscriber::for_test(sink.clone());
    sub.translate(RuntimeEvent::FileEditWithSymbols {
        path: PathBuf::from("src/main.rs"),
        op: "edit".into(),
        bytes: 1240,
        anchored_symbols: vec![],
        lsp_diagnostics_delta: None,
    })
    .await
    .unwrap();

    let recorded = sink.events_for_test();
    let count = recorded
        .iter()
        .filter(|e| {
            matches!(
                e,
                coding_ingest::event::AgentEvent::V1(coding_ingest::event::AgentEventV1 {
                    kind: coding_ingest::event::EventKind::FileEditEnriched { .. },
                    ..
                })
            )
        })
        .count();
    assert_eq!(count, 1);
}
