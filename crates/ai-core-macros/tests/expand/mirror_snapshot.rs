use ai_core::{AiEventMeta, AiFeature, AiSignal, MirrorSnapshotSpec, RecallDomain};
use ai_core_macros::AiFeature;

// Stub DomainEvent for the test (trybuild context doesn't have bus crate)
#[derive(Debug, Clone)]
pub enum DomainEvent {}

#[derive(Debug, Clone)]
pub struct TinyEvent;

impl AiEventMeta for TinyEvent {
    fn to_signal(&self) -> AiSignal { unimplemented!() }
    fn event_kind(&self) -> &'static str { "TinyEvent" }
}

impl From<TinyEvent> for DomainEvent {
    fn from(_: TinyEvent) -> Self { unimplemented!() }
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

fn main() {
    assert_eq!(<TinyFeature as AiFeature>::DOMAIN, RecallDomain::Tasks);
    let specs: &'static [MirrorSnapshotSpec] = TinyFeature::MIRROR_SNAPSHOTS;
    assert_eq!(specs.len(), 2);
    assert_eq!(specs[0].name, "task_focus");
    assert_eq!(specs[0].flush_interval_secs, Some(3600));
    assert_eq!(specs[0].subscribed_kinds, &["TaskFocusChanged", "TaskCompleted"]);
    assert_eq!(specs[1].name, "task_velocity");
    assert_eq!(specs[1].flush_interval_secs, None);
    assert_eq!(specs[1].subscribed_kinds, &["TaskCompleted"]);
}
