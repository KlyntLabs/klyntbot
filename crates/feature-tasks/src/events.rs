use ai_core_macros::AiEvent;
use bus::DomainEvent;

#[derive(Debug, Clone, AiEvent)]
#[ai(domain = "Tasks")]
pub enum TaskEvent {
    #[ai(
        importance = 0.7,
        salience = "accumulate",
        observation_template = "Created task: {title} (priority {priority:?})",
        entity_bridge(type = "task", name_from = "title", id_from = "task_id")
    )]
    Created {
        task_id: String,
        title: String,
        area_id: String,
        project_id: Option<String>,
        priority: Option<i16>,
        estimated_minutes: Option<i32>,
    },

    #[ai(
        importance = 0.6,
        salience = "extract_if(deviation_pct.unwrap_or(0.0) > 50.0)",
        observation_template = "Completed {title} (deviation {deviation_pct:?}%)",
        entity_bridge(type = "task", name_from = "title", id_from = "task_id")
    )]
    Completed {
        task_id: String,
        title: String,
        deviation_pct: Option<f64>,
    },

    #[ai(
        importance = 0.5,
        salience = "accumulate",
        observation_template = "Focused on {title}",
        entity_bridge(type = "task", name_from = "title", id_from = "task_id")
    )]
    FocusChanged {
        task_id: String,
        title: String,
        focus_deadline: Option<jiff::Timestamp>,
    },

    #[ai(
        importance = 0.5,
        salience = "accumulate",
        observation_template = "Estimation recorded: est {estimated_minutes:?}m vs actual {actual_minutes:?}m"
    )]
    EstimationRecorded {
        task_id: String,
        estimated_minutes: Option<i32>,
        actual_minutes: Option<i32>,
        deviation_pct: f64,
    },
}

impl From<TaskEvent> for DomainEvent {
    fn from(e: TaskEvent) -> Self {
        match e {
            TaskEvent::Created {
                task_id,
                title: _,
                area_id: _,
                project_id,
                priority: _,
                estimated_minutes,
            } => DomainEvent::TaskCreated {
                task_id,
                project: project_id,
                estimate_mins: estimated_minutes.map(|m| m as i64),
                task_type: "manual".to_string(),
            },
            TaskEvent::Completed {
                task_id,
                title: _,
                deviation_pct,
            } => DomainEvent::TaskCompleted {
                task_id,
                actual_duration_mins: None,
                estimated_duration_mins: None,
                deviation_pct,
            },
            TaskEvent::FocusChanged {
                task_id,
                title: _,
                focus_deadline,
            } => DomainEvent::TaskFocusChanged {
                task_id,
                focus_deadline: focus_deadline.map(|d| d.to_string()),
            },
            TaskEvent::EstimationRecorded {
                task_id,
                estimated_minutes,
                actual_minutes,
                deviation_pct,
            } => DomainEvent::EstimationRecorded {
                task_id,
                estimated_mins: estimated_minutes.unwrap_or(0) as u32,
                actual_mins: actual_minutes.unwrap_or(0) as u32,
                deviation_pct,
            },
        }
    }
}
