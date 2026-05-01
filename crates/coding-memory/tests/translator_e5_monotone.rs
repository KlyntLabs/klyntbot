use coding_memory::sink::{translator::RuntimeEvent, MemorySinkSubscriber, RecordingSink};
use proptest::prelude::*;

proptest! {
    #[test]
    fn e5_translator_emit_count_monotone(
        events in proptest::collection::vec(any::<u8>(), 0..40)
    ) {
        let trace: Vec<RuntimeEvent> = events.iter().map(|b| match b % 6 {
            0 => RuntimeEvent::ContentChunk { text: "x".into() },
            1 => RuntimeEvent::ToolStart {
                call_id: format!("t{b}"),
                name: "n".into(),
                args: serde_json::json!({}),
            },
            2 => RuntimeEvent::ToolEnd {
                call_id: format!("t{b}"),
                success: true,
                output: "".into(),
                duration_ms: 1,
            },
            3 => RuntimeEvent::IterationStart,
            4 => RuntimeEvent::IterationEnd,
            _ => RuntimeEvent::ContentChunk { text: "y".into() },
        }).collect();

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let sink = RecordingSink::new();
            let sub = MemorySinkSubscriber::for_test(sink.clone());
            let mut prev_count = 0;
            for e in trace {
                sub.translate(e).await.unwrap();
                let now = sink.events_for_test().len();
                prop_assert!(now >= prev_count, "non-monotone: {} -> {}", prev_count, now);
                prev_count = now;
            }
            Ok(())
        }).unwrap();
    }
}
