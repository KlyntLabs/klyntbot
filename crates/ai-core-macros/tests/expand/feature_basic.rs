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
#[ai(recall_domain = "Tasks", skill = "task-management", event = "TaskEvent")]
pub struct TasksFeature;

fn main() {
    assert_eq!(<TasksFeature as AiFeature>::DOMAIN, RecallDomain::Tasks);
    assert_eq!(<TasksFeature as AiFeature>::SKILL, "task-management");
}
