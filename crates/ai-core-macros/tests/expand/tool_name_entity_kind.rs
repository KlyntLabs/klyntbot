use ai_core::{AiEventMeta, AiSignal};
use ai_core_macros::AiFeature;

pub enum TaskEvent { _Placeholder }
impl AiEventMeta for TaskEvent {
    fn to_signal(&self) -> AiSignal { unimplemented!() }
    fn event_kind(&self) -> &'static str { "_Placeholder" }
}

impl From<TaskEvent> for bus::DomainEvent {
    fn from(_: TaskEvent) -> Self { unimplemented!() }
}

#[derive(AiFeature)]
#[ai(
    recall_domain = "Tasks",
    skill = "task-management",
    tool_name = "tasks",
    entity_kind = "task",
    event = "TaskEvent",
)]
pub struct TasksFeatureSnap;

fn main() {
    assert_eq!(TasksFeatureSnap::TOOL_NAME, Some("tasks"));
    assert_eq!(TasksFeatureSnap::ENTITY_KIND, Some("task"));
}
