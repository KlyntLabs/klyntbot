//! K12: Subscription event ordering monotonicity.
//!
//! For any sequence of `ThreadEvent`s published to a `TypedBroker`, all
//! subscribers MUST observe events in publish order. This is the property
//! the subscription model in Phase 4 relies on — UI reducers assume that
//! `ItemDelta`s for an `item_id` arrive between its `ItemStarted` and
//! `ItemCompleted`. If the broker reordered events the reducer would see
//! deltas before the placeholder existed.

use bus::TypedBroker;
use desktop_shared::coding::{MessageDto, ThreadEvent};
use proptest::prelude::*;

fn arb_thread_event() -> impl Strategy<Value = ThreadEvent> {
    // Generate a small, varied set covering the main shapes the bridge emits.
    prop_oneof![
        (any::<u32>()).prop_map(|n| ThreadEvent::TurnStarted {
            thread_id: format!("t-{n}"),
            turn_id: format!("turn-{n}"),
            model: "test".into(),
            started_at: n as i64,
        }),
        (any::<u32>()).prop_map(|n| ThreadEvent::ItemStarted {
            thread_id: "t".into(),
            turn_id: "turn".into(),
            item: MessageDto {
                id: format!("msg-{n}"),
                session_id: "t".into(),
                role: "assistant".into(),
                parts: vec![],
                model: None,
                turn_id: Some("turn".into()),
                created_at: n as i64,
                finish_reason: None,
            },
        }),
        (any::<u32>()).prop_map(|n| ThreadEvent::ToolCallStarted {
            thread_id: "t".into(),
            turn_id: "turn".into(),
            item_id: "msg".into(),
            call_id: format!("call-{n}"),
            tool: "bash".into(),
        }),
        (any::<u32>()).prop_map(|n| ThreadEvent::TurnCompleted {
            thread_id: "t".into(),
            turn_id: "turn".into(),
            finish_reason: serde_json::json!({"kind": "completed", "n": n}),
            completed_at: n as i64,
            duration_ms: 1,
        }),
    ]
}

fn event_id(e: &ThreadEvent) -> String {
    // Stable identity for ordering comparison — discriminant + per-variant key.
    match e {
        ThreadEvent::TurnStarted { turn_id, started_at, .. } => {
            format!("ts:{turn_id}:{started_at}")
        }
        ThreadEvent::ItemStarted { item, .. } => format!("is:{}", item.id),
        ThreadEvent::ItemDelta { item_id, part_idx, .. } => {
            format!("id:{item_id}:{part_idx}")
        }
        ThreadEvent::ItemCompleted { item, .. } => format!("ic:{}", item.id),
        ThreadEvent::ToolCallStarted { call_id, .. } => format!("tcs:{call_id}"),
        ThreadEvent::ToolCallCompleted { call_id, .. } => format!("tcc:{call_id}"),
        ThreadEvent::FileChanged { path, .. } => format!("fc:{path}"),
        ThreadEvent::CommandExecuted { command, .. } => format!("ce:{}", command.join(" ")),
        ThreadEvent::ContextCompressed { before_tokens, after_tokens, .. } => {
            format!("cc:{before_tokens}:{after_tokens}")
        }
        ThreadEvent::TurnCompleted { turn_id, completed_at, .. } => {
            format!("tc:{turn_id}:{completed_at}")
        }
        ThreadEvent::Heartbeat { server_time, .. } => format!("hb:{server_time}"),
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

    /// Single subscriber observes events in publish order.
    #[test]
    fn k12_single_subscriber_in_order(events in proptest::collection::vec(arb_thread_event(), 1..32)) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let broker: TypedBroker<ThreadEvent> = TypedBroker::new(256);
            let mut rx = broker.subscribe();
            let expected: Vec<String> = events.iter().map(event_id).collect();
            for e in &events {
                broker.publish(e.clone());
            }
            let mut got: Vec<String> = Vec::new();
            for _ in 0..events.len() {
                got.push(event_id(&rx.recv().await.unwrap()));
            }
            prop_assert_eq!(expected, got);
            Ok(())
        }).unwrap();
    }

    /// Multiple subscribers observe the same sequence in the same order.
    #[test]
    fn k12_multi_subscriber_consistent(events in proptest::collection::vec(arb_thread_event(), 1..16)) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let broker: TypedBroker<ThreadEvent> = TypedBroker::new(256);
            let mut rx1 = broker.subscribe();
            let mut rx2 = broker.subscribe();
            for e in &events {
                broker.publish(e.clone());
            }
            for e in &events {
                let want = event_id(e);
                let g1 = event_id(&rx1.recv().await.unwrap());
                let g2 = event_id(&rx2.recv().await.unwrap());
                prop_assert_eq!(&want, &g1);
                prop_assert_eq!(&want, &g2);
            }
            Ok(())
        }).unwrap();
    }
}
