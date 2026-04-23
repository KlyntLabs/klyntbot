use ai_core::{AiEventMeta, AiFeature, AiSignal, RecallDomain};
use ai_core_macros::AiFeature;

pub enum TaskEvent { Created }
impl AiEventMeta for TaskEvent {
    fn to_signal(&self) -> AiSignal { unimplemented!() }
    fn event_kind(&self) -> &'static str { "Created" }
}

// Stub DomainEvent for the test
#[derive(Debug, Clone)]
pub enum DomainEvent {}

impl From<TaskEvent> for DomainEvent {
    fn from(_: TaskEvent) -> Self { unimplemented!() }
}

#[derive(AiFeature)]
#[ai(
    recall_domain = "Tasks",
    skill = "task-management",
    event = "TaskEvent",
    promotion_threshold = 5,
)]
pub struct TasksFeatureDemo;

fn main() {
    assert_eq!(<TasksFeatureDemo as AiFeature>::DOMAIN, RecallDomain::Tasks);
    assert_eq!(TasksFeatureDemo::PROMOTE_THRESHOLD_OVERRIDE, Some(5));
}
