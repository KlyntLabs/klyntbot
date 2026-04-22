use ai_core::{AiEventMeta, AiFeature, AiSignal, MirrorSnapshotSpec, RecallDomain};
use ai_core_macros::AiFeature;
use bus::DomainEvent;

#[derive(Debug, Clone)]
pub enum TinyEvent {
    Ping,
}

impl AiEventMeta for TinyEvent {
    fn to_signal(&self) -> AiSignal {
        AiSignal {
            domain: RecallDomain::General,
            event_kind: "Ping",
            importance: 0.5,
            salience: ai_core::SalienceVerdict::Accumulate,
            content: String::new(),
            entity: None,
            timestamp: jiff::Timestamp::now(),
            raw_event: None,
            metrics: ai_core::AiMetrics::default(),
            coaching_signal: false,
            coaching_rule: None,
        }
    }
    fn event_kind(&self) -> &'static str {
        "Ping"
    }
}

impl From<TinyEvent> for DomainEvent {
    fn from(_: TinyEvent) -> Self {
        DomainEvent::ChatTurnCompleted {
            session_key: String::new(),
            user_message: None,
        }
    }
}

#[derive(AiFeature)]
#[ai(
    recall_domain = "Tasks",
    skill = "task-management",
    event = "crate::TinyEvent",
    mirror_snapshot(
        name = "task_focus",
        flush_interval_secs = 3600,
        event_kinds = ["TaskFocusChanged", "TaskCompleted"],
    ),
    mirror_snapshot(
        name = "task_velocity",
        event_kinds = ["TaskCompleted"],
    ),
)]
pub struct TinyFeature;

#[test]
fn mirror_snapshot_attr_emits_constant() {
    assert_eq!(<TinyFeature as AiFeature>::DOMAIN, RecallDomain::Tasks);
    let specs: &'static [MirrorSnapshotSpec] = TinyFeature::MIRROR_SNAPSHOTS;
    assert_eq!(specs.len(), 2);
    assert_eq!(specs[0].name, "task_focus");
    assert_eq!(specs[0].flush_interval_secs, Some(3600));
    assert_eq!(
        specs[0].subscribed_kinds,
        &["TaskFocusChanged", "TaskCompleted"]
    );
    assert_eq!(specs[1].name, "task_velocity");
    assert_eq!(specs[1].flush_interval_secs, None);
    assert_eq!(specs[1].subscribed_kinds, &["TaskCompleted"]);
}
