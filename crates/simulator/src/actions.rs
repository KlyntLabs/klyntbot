//! Executor that translates [`SimulatedToolAction`] values into [`DomainEvent`]
//! publications on the [`DomainEventBus`].

use std::sync::Arc;

use bus::{DomainEvent, DomainEventBus};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::persona::types::SimulatedToolAction;

/// Converts simulated tool actions into domain events and publishes them.
pub struct ActionExecutor {
    bus: Arc<DomainEventBus>,
}

impl ActionExecutor {
    pub fn new(bus: Arc<DomainEventBus>) -> Self {
        Self { bus }
    }

    /// Execute a single simulated tool action by publishing the corresponding
    /// [`DomainEvent`] on the bus.
    ///
    /// `simulated_now` is the virtual clock timestamp for the action — used when
    /// domain events need a date string.
    pub async fn execute(
        &self,
        action: &SimulatedToolAction,
        simulated_now: DateTime<Utc>,
    ) -> common::Result<()> {
        match action {
            SimulatedToolAction::CreateTask {
                title: _,
                due_offset_days: _,
                project,
            } => {
                let task_id = Uuid::new_v4().to_string();
                tracing::debug!(task_id = %task_id, "action: CreateTask");
                self.bus.publish(DomainEvent::TaskCreated {
                    task_id,
                    project: project.clone(),
                    estimate_mins: None,
                    task_type: "todo".to_string(),
                });
            }

            SimulatedToolAction::CompleteTask { task_ref } => {
                tracing::debug!(task_ref = %task_ref, "action: CompleteTask");
                self.bus.publish(DomainEvent::TaskCompleted {
                    task_id: task_ref.clone(),
                    actual_duration_mins: None,
                    estimated_duration_mins: None,
                    deviation_pct: None,
                });
            }

            SimulatedToolAction::CreateNote { title, content } => {
                let note_id = Uuid::new_v4().to_string();
                tracing::debug!(note_id = %note_id, title = %title, "action: CreateNote");
                self.bus.publish(DomainEvent::NoteCreated {
                    note_id: note_id.clone(),
                    title: title.clone(),
                });
                self.bus.publish(DomainEvent::NoteContentChanged {
                    note_id,
                    content: content.clone(),
                });
            }

            SimulatedToolAction::UpdateNote {
                note_ref,
                new_content,
            } => {
                tracing::debug!(note_ref = %note_ref, "action: UpdateNote");
                self.bus.publish(DomainEvent::NoteContentChanged {
                    note_id: note_ref.clone(),
                    content: new_content.clone(),
                });
            }

            SimulatedToolAction::RecordTransaction {
                amount,
                category,
                description: _,
            } => {
                tracing::debug!(category = %category, amount = %amount, "action: RecordTransaction");
                self.bus.publish(DomainEvent::TransactionRecorded {
                    category: category.clone(),
                    amount: *amount,
                    is_over_budget: false,
                });
            }

            SimulatedToolAction::StartFocus {
                task_ref: _,
                duration_mins,
            } => {
                tracing::debug!(duration_mins = %duration_mins, "action: StartFocus");
                self.bus.publish(DomainEvent::FocusSessionStarted {
                    session_type: "simulated".to_string(),
                    target_mins: i64::from(*duration_mins),
                });
            }

            SimulatedToolAction::CreateObjective {
                title: _,
                project: _,
                due_offset_days: _,
            } => {
                let objective_id = Uuid::new_v4().to_string();
                tracing::debug!(objective_id = %objective_id, "action: CreateObjective");
                self.bus.publish(DomainEvent::GoalProgress {
                    objective_id,
                    progress: 0.0,
                    target: 100.0,
                });
            }

            SimulatedToolAction::RecordProductivityEvent {
                event_type,
                duration_mins,
            } => {
                // No exact DomainEvent variant for a generic productivity event.
                // Emit a ProductivityScoreComputed as a reasonable approximation.
                let date = simulated_now.format("%Y-%m-%d").to_string();
                tracing::debug!(
                    event_type = %event_type,
                    duration_mins = ?duration_mins,
                    date = %date,
                    "action: RecordProductivityEvent — no exact DomainEvent variant, \
                     emitting ProductivityScoreComputed as proxy"
                );
                self.bus.publish(DomainEvent::ProductivityScoreComputed {
                    date,
                    score: duration_mins.map(f64::from).unwrap_or(50.0),
                });
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn create_task_publishes_event() {
        let bus = Arc::new(DomainEventBus::new(32));
        let mut rx = bus.subscribe();
        let executor = ActionExecutor::new(Arc::clone(&bus));

        let action = SimulatedToolAction::CreateTask {
            title: "Buy groceries".into(),
            due_offset_days: Some(1),
            project: Some("personal".into()),
        };

        executor
            .execute(&action, Utc::now())
            .await
            .expect("execute should succeed");

        let event = rx.try_recv().expect("should receive TaskCreated");
        assert!(
            matches!(event, DomainEvent::TaskCreated { project: Some(p), .. } if p == "personal")
        );
    }

    #[tokio::test]
    async fn create_note_publishes_two_events() {
        let bus = Arc::new(DomainEventBus::new(32));
        let mut rx = bus.subscribe();
        let executor = ActionExecutor::new(Arc::clone(&bus));

        let action = SimulatedToolAction::CreateNote {
            title: "Meeting notes".into(),
            content: "Discussed roadmap".into(),
        };

        executor
            .execute(&action, Utc::now())
            .await
            .expect("execute should succeed");

        let e1 = rx.try_recv().expect("should receive NoteCreated");
        assert!(matches!(e1, DomainEvent::NoteCreated { .. }));

        let e2 = rx.try_recv().expect("should receive NoteContentChanged");
        assert!(matches!(e2, DomainEvent::NoteContentChanged { .. }));
    }

    #[tokio::test]
    async fn complete_task_publishes_event() {
        let bus = Arc::new(DomainEventBus::new(32));
        let mut rx = bus.subscribe();
        let executor = ActionExecutor::new(Arc::clone(&bus));

        let action = SimulatedToolAction::CompleteTask {
            task_ref: "task-abc".into(),
        };

        executor
            .execute(&action, Utc::now())
            .await
            .expect("execute should succeed");

        let event = rx.try_recv().expect("should receive TaskCompleted");
        assert!(
            matches!(event, DomainEvent::TaskCompleted { task_id, .. } if task_id == "task-abc")
        );
    }

    #[tokio::test]
    async fn record_transaction_publishes_event() {
        let bus = Arc::new(DomainEventBus::new(32));
        let mut rx = bus.subscribe();
        let executor = ActionExecutor::new(Arc::clone(&bus));

        let action = SimulatedToolAction::RecordTransaction {
            amount: 42.50,
            category: "food".into(),
            description: "Lunch".into(),
        };

        executor
            .execute(&action, Utc::now())
            .await
            .expect("execute should succeed");

        let event = rx.try_recv().expect("should receive TransactionRecorded");
        assert!(
            matches!(event, DomainEvent::TransactionRecorded { category, amount, .. }
                     if category == "food" && (amount - 42.50).abs() < f64::EPSILON)
        );
    }

    #[tokio::test]
    async fn start_focus_publishes_event() {
        let bus = Arc::new(DomainEventBus::new(32));
        let mut rx = bus.subscribe();
        let executor = ActionExecutor::new(Arc::clone(&bus));

        let action = SimulatedToolAction::StartFocus {
            task_ref: None,
            duration_mins: 25,
        };

        executor
            .execute(&action, Utc::now())
            .await
            .expect("execute should succeed");

        let event = rx.try_recv().expect("should receive FocusSessionStarted");
        assert!(
            matches!(event, DomainEvent::FocusSessionStarted { target_mins: 25, .. })
        );
    }
}
