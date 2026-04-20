//! Domain types for the feature-tasks crate.
//!
//! Canonical definitions of `Task`, `TaskActivity`, and all associated enums
//! and supporting types. Includes conversions from/to storage row types.

mod entity;
mod execution;
mod planning;

pub use entity::*;
pub use execution::*;
pub use planning::*;

#[cfg(test)]
mod tests {
    use super::*;
    use storage::rows::task::*;
    use storage::sqlite_types::SqlTs;

    #[test]
    fn test_task_type_default_is_manual() {
        assert_eq!(TaskType::default(), TaskType::Manual);
    }

    #[test]
    fn test_execution_state_default_is_idle() {
        assert_eq!(ExecutionState::default(), ExecutionState::Idle);
    }

    #[test]
    fn test_energy_level_default_is_medium() {
        assert_eq!(EnergyLevel::default(), EnergyLevel::Medium);
    }

    #[test]
    fn test_task_serde_round_trip() {
        let mut t = Task::default_instance();
        t.area_id = "area-1".to_string();
        t.title = "Test task".to_string();
        t.task_type = TaskType::Agentic;
        t.energy_level = Some(EnergyLevel::High);
        let json = serde_json::to_string(&t).unwrap();
        let parsed: Task = serde_json::from_str(&json).unwrap();
        assert_eq!(t.id, parsed.id);
        assert_eq!(parsed.task_type, TaskType::Agentic);
        assert_eq!(parsed.energy_level, Some(EnergyLevel::High));
        assert!(!parsed.is_template);
        assert!(!parsed.completed);
    }

    #[test]
    fn test_task_from_row_conversion() {
        let now = SqlTs::from(jiff::Timestamp::now());
        let row = TaskRow {
            id: "test1234".to_string(),
            title: "Test".to_string(),
            description: None,
            area_id: "area-1".to_string(),
            project_id: None,
            key_result_id: None,
            objective_id: None,
            parent_id: None,
            priority: Some(2),
            due_date: None,
            tags: vec!["tag1".to_string()],
            status: "doing".to_string(),
            focused_at: None,
            focus_deadline: None,
            focus_expired_count: 0,
            created_at: now,
            updated_at: now,
            completed_at: None,
            total_tracked_secs: 0,
            estimated_minutes: Some(30),
            calendar_event_uid: None,
            last_reminded_at: None,
            recurrence_rule: None,
            recurrence_parent_id: None,
            is_template: false,
            next_instance_date: None,
            status_label_id: None,
            position: 0,
            group_id: None,
            task_type: "agentic".to_string(),
            acceptance_criteria: Some("It works".to_string()),
            agent_config: Some(r#"{"requireApproval":true,"retryPolicy":null}"#.to_string()),
            execution_state: "idle".to_string(),
            spawned_execution_id: None,
            context_snapshot: None,
            energy_level: Some("high".to_string()),
            estimated_focus_blocks: Some(2),
            actual_minutes: None,
            complexity_score: Some(3),
            completed: false,
            scheduled_start: None,
            scheduled_end: None,
        };

        let task = Task::from(row);
        assert_eq!(task.id, "test1234");
        assert_eq!(task.task_type, TaskType::Agentic);
        assert_eq!(task.energy_level, Some(EnergyLevel::High));
        assert_eq!(task.priority, Some(2));
        assert!(task.agent_config.is_some());
        assert!(task.agent_config.unwrap().require_approval);
        assert_eq!(task.estimated_focus_blocks, Some(2));
        assert_eq!(task.complexity_score, Some(3));
    }

    #[test]
    fn test_task_to_row_round_trip() {
        let mut t = Task::default_instance();
        t.area_id = "area-1".to_string();
        t.task_type = TaskType::Hybrid;
        t.energy_level = Some(EnergyLevel::Deep);
        t.agent_config = Some(AgentConfig {
            require_approval: true,
            retry_policy: Some(RetryPolicy::default()),
        });

        let row = TaskRow::from(&t);
        assert_eq!(row.task_type, "hybrid");
        assert_eq!(row.energy_level, Some("deep".to_string()));
        assert!(row.agent_config.is_some());

        let task_back = Task::from(row);
        assert_eq!(task_back.task_type, TaskType::Hybrid);
        assert_eq!(task_back.energy_level, Some(EnergyLevel::Deep));
        assert!(task_back.agent_config.is_some());
    }

    #[test]
    fn test_agent_config_serde() {
        let config = AgentConfig {
            require_approval: true,
            retry_policy: Some(RetryPolicy {
                max_retries: 5,
                base_delay_secs: 120,
                exponential_backoff: true,
            }),
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: AgentConfig = serde_json::from_str(&json).unwrap();
        assert!(parsed.require_approval);
        assert_eq!(parsed.retry_policy.unwrap().max_retries, 5);
    }

    #[test]
    fn test_retry_policy_defaults() {
        let manual = RetryPolicy::default_for_task_type(&TaskType::Manual);
        assert_eq!(manual.max_retries, 0);

        let agentic = RetryPolicy::default_for_task_type(&TaskType::Agentic);
        assert_eq!(agentic.max_retries, 3);
        assert!(agentic.exponential_backoff);

        let hybrid = RetryPolicy::default_for_task_type(&TaskType::Hybrid);
        assert_eq!(hybrid.max_retries, 1);
        assert!(!hybrid.exponential_backoff);
    }

    #[test]
    fn test_task_type_display_and_parse() {
        assert_eq!(TaskType::Manual.to_string(), "manual");
        assert_eq!("agentic".parse::<TaskType>().unwrap(), TaskType::Agentic);
        assert!("unknown".parse::<TaskType>().is_err());
    }

    #[test]
    fn test_execution_state_display_and_parse() {
        assert_eq!(ExecutionState::Idle.to_string(), "idle");
        assert_eq!(
            "running".parse::<ExecutionState>().unwrap(),
            ExecutionState::Running
        );
        assert_eq!(
            "awaiting_approval".parse::<ExecutionState>().unwrap(),
            ExecutionState::AwaitingApproval
        );
    }

    #[test]
    fn test_energy_level_display_and_parse() {
        assert_eq!(EnergyLevel::Deep.to_string(), "deep");
        assert_eq!("low".parse::<EnergyLevel>().unwrap(), EnergyLevel::Low);
        assert!("extreme".parse::<EnergyLevel>().is_err());
    }

    #[test]
    fn test_generate_id_length() {
        let id = Task::generate_id();
        assert_eq!(id.len(), 8);
    }

    #[test]
    fn test_generate_id_uniqueness() {
        use std::collections::HashSet;
        let ids: HashSet<String> = (0..50).map(|_| Task::generate_id()).collect();
        assert_eq!(ids.len(), 50);
    }

    #[test]
    fn test_working_hours_default() {
        let wh = WorkingHours::default();
        assert_eq!(wh.start, jiff::civil::Time::new(9, 0, 0, 0).unwrap());
        assert_eq!(wh.end, jiff::civil::Time::new(17, 0, 0, 0).unwrap());
        assert_eq!(wh.lunch_start, jiff::civil::Time::new(12, 0, 0, 0).unwrap());
    }

    #[test]
    fn test_task_status_from_str_loose() {
        assert_eq!(TaskStatus::from_str_loose("todo"), Some(TaskStatus::Todo));
        assert_eq!(TaskStatus::from_str_loose("DONE"), Some(TaskStatus::Done));
        assert_eq!(TaskStatus::from_str_loose("unknown"), None);
    }

    #[test]
    fn test_context_snapshot_serde() {
        let snap = ContextSnapshot {
            facts: vec!["fact1".to_string()],
            parent_chain: vec!["p1".to_string()],
            sibling_titles: vec!["sibling".to_string()],
            active_blockers: vec![],
            captured_at: jiff::Timestamp::now(),
        };
        let json = serde_json::to_string(&snap).unwrap();
        let parsed: ContextSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.facts.len(), 1);
    }

    #[test]
    fn test_estimation_record_from_row() {
        let row = TaskEstimationRow {
            id: "est1".to_string(),
            task_id: "task1".to_string(),
            estimated_minutes: 30,
            actual_minutes: 45,
            deviation_pct: 50.0,
            complexity_score: Some(3),
            energy_level: Some("high".to_string()),
            tags: vec!["dev".to_string()],
            project_id: Some("proj1".to_string()),
            completed_at: SqlTs::from(jiff::Timestamp::now()),
        };
        let record = EstimationRecord::from(row);
        assert_eq!(record.estimated_minutes, 30);
        assert_eq!(record.actual_minutes, 45);
        assert_eq!(record.energy_level, Some(EnergyLevel::High));
    }
}
