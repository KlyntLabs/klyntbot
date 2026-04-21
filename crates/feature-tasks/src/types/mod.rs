//! Domain types for the feature-tasks crate.
//!
//! Canonical definitions of `Task`, `TaskActivity`, and all associated enums
//! and supporting types. Includes conversions from/to storage row types.

mod entity;
mod planning;

pub use entity::*;
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
    fn test_energy_level_default_is_medium() {
        assert_eq!(EnergyLevel::default(), EnergyLevel::Medium);
    }

    #[test]
    fn test_task_serde_round_trip() {
        let mut t = Task::default_instance();
        t.area_id = "area-1".to_string();
        t.title = "Test task".to_string();
        t.energy_level = Some(EnergyLevel::High);
        let json = serde_json::to_string(&t).unwrap();
        let parsed: Task = serde_json::from_str(&json).unwrap();
        assert_eq!(t.id, parsed.id);
        assert_eq!(parsed.task_type, TaskType::Manual);
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
            task_type: "manual".to_string(),
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
        assert_eq!(task.task_type, TaskType::Manual);
        assert_eq!(task.energy_level, Some(EnergyLevel::High));
        assert_eq!(task.priority, Some(2));
        assert_eq!(task.estimated_focus_blocks, Some(2));
        assert_eq!(task.complexity_score, Some(3));
    }

    #[test]
    fn test_task_to_row_round_trip() {
        let mut t = Task::default_instance();
        t.area_id = "area-1".to_string();
        t.energy_level = Some(EnergyLevel::Deep);

        let row = TaskRow::from(&t);
        assert_eq!(row.task_type, "manual");
        assert_eq!(row.energy_level, Some("deep".to_string()));

        let task_back = Task::from(row);
        assert_eq!(task_back.task_type, TaskType::Manual);
        assert_eq!(task_back.energy_level, Some(EnergyLevel::Deep));
    }

    #[test]
    fn test_task_type_display_and_parse() {
        assert_eq!(TaskType::Manual.to_string(), "manual");
        assert!("unknown".parse::<TaskType>().is_err());
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
}
