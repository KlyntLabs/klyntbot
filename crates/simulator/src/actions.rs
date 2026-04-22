//! Executor that translates [`SimulatedToolAction`] values into [`DomainEvent`]
//! publications on the [`DomainEventBus`].

use std::sync::Arc;

use bus::{DomainEvent, DomainEventBus};
use jiff::Timestamp;
use tracing::debug;
use uuid::Uuid;

use crate::persona::types::SimulatedToolAction;

/// Converts simulated tool actions into domain events and publishes them.
pub struct ActionExecutor {
    bus: Arc<DomainEventBus>,
    pool: sqlx::SqlitePool,
}

impl ActionExecutor {
    pub fn new(bus: Arc<DomainEventBus>, pool: sqlx::SqlitePool) -> Self {
        Self { bus, pool }
    }

    /// Insert or replace a `book_tree_nodes` row for a note.
    async fn upsert_tree_node(&self, id: &str, content: &str, title: &str) {
        let _ = sqlx::query(
            "INSERT OR REPLACE INTO book_tree_nodes \
             (id, parent_id, node_type, content, title, level, source_type, source_id, position) \
             VALUES (?, NULL, 'Section', ?, ?, 0, 'Note', ?, 0)",
        )
        .bind(id)
        .bind(content)
        .bind(title)
        .bind(id)
        .execute(&self.pool)
        .await;
    }

    /// Execute a single simulated tool action by publishing the corresponding
    /// [`DomainEvent`] on the bus.
    ///
    /// `simulated_now` is the virtual clock timestamp for the action — used when
    /// domain events need a date string.
    pub async fn execute(
        &self,
        action: &SimulatedToolAction,
        simulated_now: Timestamp,
    ) -> common::Result<()> {
        match action {
            SimulatedToolAction::CreateTask {
                title,
                due_offset_days: _,
                project,
            } => {
                let task_id = Uuid::new_v4().to_string();
                debug!(task_id = %task_id, "action: CreateTask");
                let ev: DomainEvent = feature_tasks::events::TaskEvent::Created {
                    task_id: task_id.clone(),
                    title: title.clone(),
                    area_id: "sim-area".to_string(),
                    project_id: project.clone(),
                    priority: Some(3),
                    estimated_minutes: None,
                }.into();
                self.bus.publish(ev);

                // INSERT task row into the tasks table.
                let _ = sqlx::query(
                    "INSERT OR IGNORE INTO tasks (id, title, area_id, status, priority, created_at, updated_at, completed) \
                     VALUES (?, ?, 'sim-area', 'todo', 3, ?, ?, 0)",
                )
                .bind(&task_id)
                .bind(title)
                .bind(simulated_now.to_string())
                .bind(simulated_now.to_string())
                .execute(&self.pool)
                .await;
            }

            SimulatedToolAction::CompleteTask {
                task_ref,
                estimated_duration_mins,
                actual_duration_mins,
            } => {
                debug!(task_ref = %task_ref, "action: CompleteTask");

                let deviation_pct = match (estimated_duration_mins, actual_duration_mins) {
                    (Some(est), Some(act)) if *est > 0 => {
                        Some((*act as f64 - *est as f64) / *est as f64 * 100.0)
                    }
                    _ => None,
                };

                let ev: DomainEvent = feature_tasks::events::TaskEvent::Completed {
                    task_id: task_ref.clone(),
                    title: task_ref.clone(),
                    deviation_pct,
                }.into();
                self.bus.publish(ev);

                if let (Some(est), Some(act)) = (estimated_duration_mins, actual_duration_mins) {
                    self.bus.publish(DomainEvent::EstimationRecorded {
                        task_id: task_ref.clone(),
                        estimated_mins: *est,
                        actual_mins: *act,
                        deviation_pct: deviation_pct.unwrap_or(0.0),
                    });
                }

                // UPDATE the task row to completed.
                let _ = sqlx::query(
                    "UPDATE tasks SET status = 'completed', completed = 1, completed_at = ?, updated_at = ? WHERE title = ?",
                )
                .bind(simulated_now.to_string())
                .bind(simulated_now.to_string())
                .bind(task_ref)
                .execute(&self.pool)
                .await;
            }

            SimulatedToolAction::CreateNote { title, content } => {
                let note_id = Uuid::new_v4().to_string();
                debug!(note_id = %note_id, title = %title, "action: CreateNote");
                self.bus.publish(DomainEvent::NoteCreated {
                    note_id: note_id.clone(),
                    title: title.clone(),
                });
                self.bus.publish(DomainEvent::NoteContentChanged {
                    note_id: note_id.clone(),
                });
                self.upsert_tree_node(&note_id, content, title).await;
            }

            SimulatedToolAction::UpdateNote {
                note_ref,
                new_content,
            } => {
                debug!(note_ref = %note_ref, "action: UpdateNote");
                self.bus.publish(DomainEvent::NoteContentChanged {
                    note_id: note_ref.clone(),
                });
                self.upsert_tree_node(note_ref, new_content, note_ref).await;
            }

            SimulatedToolAction::RecordTransaction {
                amount,
                category,
                description,
            } => {
                debug!(category = %category, amount = %amount, "action: RecordTransaction");
                // INSERT a finance_transactions row.
                let tx_id = Uuid::new_v4().to_string();
                let ev: DomainEvent = feature_finance::events::FinanceEvent::TransactionRecorded {
                    tx_id: tx_id.clone(),
                    category: category.clone(),
                    amount: (*amount * 100.0) as i64,
                    currency: "VND".to_string(),
                    is_over_budget: false,
                }.into();
                self.bus.publish(ev);
                let _ = sqlx::query(
                    "INSERT INTO finance_transactions \
                     (id, account_id, tx_type, amount, currency, category, notes, tx_date, created_at, updated_at) \
                     VALUES (?, 'sim-account', 'expense', ?, 'VND', ?, ?, ?, ?, ?)",
                )
                .bind(&tx_id)
                .bind((*amount * 100.0) as i64)
                .bind(category)
                .bind(description)
                .bind(simulated_now.strftime("%Y-%m-%d").to_string())
                .bind(simulated_now.to_string())
                .bind(simulated_now.to_string())
                .execute(&self.pool)
                .await;
            }

            SimulatedToolAction::StartFocus {
                task_ref: _,
                duration_mins,
            } => {
                debug!(duration_mins = %duration_mins, "action: StartFocus");
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
                debug!(objective_id = %objective_id, "action: CreateObjective");
                // GoalProgress variant deleted; no-op in simulator.
            }

            SimulatedToolAction::RecordProductivityEvent {
                event_type,
                duration_mins,
            } => {
                // No exact DomainEvent variant for a generic productivity event.
                // Emit a ProductivityScoreComputed as a reasonable approximation.
                let date = simulated_now.strftime("%Y-%m-%d").to_string();
                debug!(
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

            SimulatedToolAction::CreateFlashcard {
                front,
                back: _,
                topic,
            } => {
                let atom_id = Uuid::new_v4().to_string();
                debug!(atom_id = %atom_id, topic = %topic, "action: CreateFlashcard");
                self.bus.publish(DomainEvent::KnowledgeAtomCreated {
                    atom_id,
                    atom_type: "flashcard".to_string(),
                    domain: topic.clone(),
                    source_note_id: None,
                    personal_importance: 0.7,
                });
                let truncated = &front[..front.floor_char_boundary(30)];
                debug!(front = %truncated, "flashcard front (truncated)");
            }

            SimulatedToolAction::ReviewFlashcard { topic, rating } => {
                debug!(topic = %topic, rating = %rating, "action: ReviewFlashcard");
                // Realistic recall speed: higher ratings = faster recall.
                // Rating 1 (forgot): 3000-5000ms, Rating 4 (easy): 600-1200ms.
                let base_speed: u64 = match *rating {
                    1 => 4000,
                    2 => 2500,
                    3 => 1500,
                    _ => 900,
                };
                let noise = topic.bytes().fold(0u64, |a, b| a.wrapping_add(b as u64)) % 500;
                let recall_speed_ms = base_speed + noise;

                // Retention follows FSRS-5 mapping: rating → expected retention.
                let base_retention: f64 = match *rating {
                    1 => 30.0,
                    2 => 50.0,
                    3 => 70.0,
                    _ => 90.0,
                };
                let retention_noise = (noise as f64 % 10.0) - 5.0;
                let new_retention_pct = (base_retention + retention_noise).clamp(10.0, 99.0);

                self.bus.publish(DomainEvent::AtomFlashcardReviewed {
                    atom_id: Uuid::new_v4().to_string(),
                    card_id: Uuid::new_v4().to_string(),
                    quality: *rating,
                    recall_speed_ms,
                    new_retention_pct,
                    source_note_id: None,
                });
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_pool() -> sqlx::SqlitePool {
        let pool = storage::StoragePool::connect_in_memory()
            .await
            .expect("in-memory pool");
        let inner = pool.inner().clone();
        storage::StoragePool::run_feature_migrations(&inner, &cognitive::cognitive_migrations())
            .await
            .expect("cognitive migrations");
        // Run feature-tasks migration so task INSERTs succeed.
        sqlx::query(feature_tasks::TasksFeature::migration_sql())
            .execute(&inner)
            .await
            .expect("tasks migration");
        // Seed parent rows for FK constraints.
        sqlx::query(
            "INSERT OR IGNORE INTO areas (id, name, color, status) \
             VALUES ('sim-area', 'Simulation', '#6366f1', 'active')",
        )
        .execute(&inner)
        .await
        .expect("seed area");
        sqlx::query(
            "INSERT OR IGNORE INTO finance_accounts \
             (id, name, account_type, currency, balance) \
             VALUES ('sim-account', 'Simulation Account', 'checking', 'VND', 0)",
        )
        .execute(&inner)
        .await
        .expect("seed finance account");
        // Keep the StoragePool alive by leaking it (tests are short-lived).
        std::mem::forget(pool);
        inner
    }

    #[tokio::test]
    async fn create_task_publishes_event() {
        let bus = Arc::new(DomainEventBus::new(32));
        let mut rx = bus.subscribe();
        let pool = test_pool().await;
        let executor = ActionExecutor::new(Arc::clone(&bus), pool);

        let action = SimulatedToolAction::CreateTask {
            title: "Buy groceries".into(),
            due_offset_days: Some(1),
            project: Some("personal".into()),
        };

        executor
            .execute(&action, Timestamp::now())
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
        let pool = test_pool().await;
        let executor = ActionExecutor::new(Arc::clone(&bus), pool);

        let action = SimulatedToolAction::CreateNote {
            title: "Meeting notes".into(),
            content: "Discussed roadmap".into(),
        };

        executor
            .execute(&action, Timestamp::now())
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
        let pool = test_pool().await;
        let executor = ActionExecutor::new(Arc::clone(&bus), pool);

        let action = SimulatedToolAction::CompleteTask {
            task_ref: "task-abc".into(),
            estimated_duration_mins: None,
            actual_duration_mins: None,
        };

        executor
            .execute(&action, Timestamp::now())
            .await
            .expect("execute should succeed");

        let event = rx.try_recv().expect("should receive TaskCompleted");
        assert!(
            matches!(event, DomainEvent::TaskCompleted { task_id, .. } if task_id == "task-abc")
        );
    }

    #[tokio::test]
    async fn complete_task_emits_estimation_recorded() {
        let bus = Arc::new(DomainEventBus::new(32));
        let mut rx = bus.subscribe();
        let pool = test_pool().await;
        let executor = ActionExecutor::new(Arc::clone(&bus), pool);

        let action = SimulatedToolAction::CompleteTask {
            task_ref: "task-xyz".into(),
            estimated_duration_mins: Some(30),
            actual_duration_mins: Some(45),
        };

        executor
            .execute(&action, Timestamp::now())
            .await
            .expect("execute should succeed");

        let _completed = rx.try_recv().expect("should receive TaskCompleted");
        let estimation = rx.try_recv().expect("should receive EstimationRecorded");
        assert!(
            matches!(
                estimation,
                DomainEvent::EstimationRecorded {
                    estimated_mins: 30,
                    actual_mins: 45,
                    ..
                }
            ),
            "got {estimation:?}"
        );
    }

    #[tokio::test]
    async fn record_transaction_publishes_event() {
        let bus = Arc::new(DomainEventBus::new(32));
        let mut rx = bus.subscribe();
        let pool = test_pool().await;
        let executor = ActionExecutor::new(Arc::clone(&bus), pool);

        let action = SimulatedToolAction::RecordTransaction {
            amount: 42.50,
            category: "food".into(),
            description: "Lunch".into(),
        };

        executor
            .execute(&action, Timestamp::now())
            .await
            .expect("execute should succeed");

        let event = rx.try_recv().expect("should receive TransactionRecorded");
        assert!(
            matches!(event, DomainEvent::TransactionRecorded { category, amount, .. }
                     if category == "food" && (amount - 4250.0).abs() < f64::EPSILON)
        );
    }

    #[tokio::test]
    async fn start_focus_publishes_event() {
        let bus = Arc::new(DomainEventBus::new(32));
        let mut rx = bus.subscribe();
        let pool = test_pool().await;
        let executor = ActionExecutor::new(Arc::clone(&bus), pool);

        let action = SimulatedToolAction::StartFocus {
            task_ref: None,
            duration_mins: 25,
        };

        executor
            .execute(&action, Timestamp::now())
            .await
            .expect("execute should succeed");

        let event = rx.try_recv().expect("should receive FocusSessionStarted");
        assert!(matches!(
            event,
            DomainEvent::FocusSessionStarted {
                target_mins: 25,
                ..
            }
        ));
    }
}
