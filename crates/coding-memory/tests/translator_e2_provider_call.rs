use coding_memory::sink::{translator::RuntimeEvent, MemorySinkSubscriber, RecordingSink};

#[tokio::test]
async fn e2_every_provider_response_produces_provider_call_with_matching_cost() {
    let sink = RecordingSink::new();
    let sub = MemorySinkSubscriber::for_test(sink.clone());
    sub.translate(RuntimeEvent::ProviderResponse {
        model: "gpt-5".into(),
        prompt_tokens: 100,
        completion_tokens: 50,
        cost_usd: 0.05,
        latency_ms: 1200,
        retries: 0,
    })
    .await
    .unwrap();

    let evts = sink.events_for_test();
    let pc = evts
        .iter()
        .find_map(|e| match e {
            coding_ingest::event::AgentEvent::V1(coding_ingest::event::AgentEventV1 {
                kind:
                    coding_ingest::event::EventKind::ProviderCall {
                        cost_usd,
                        latency_ms,
                        ..
                    },
                ..
            }) => Some((*cost_usd, *latency_ms)),
            _ => None,
        })
        .unwrap();
    assert_eq!(pc, (0.05, 1200));
}
