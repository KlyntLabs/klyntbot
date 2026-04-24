use ai_core_macros::AiEvent;
use bus::DomainEvent;

#[derive(Debug, Clone, AiEvent)]
#[ai(domain = "Tasks")]
pub enum TaskEvent {
    #[ai(
        importance = 0.7,
        salience = "accumulate",
        observation_template = "Created task: {title} (priority {priority:?})",
        entity_bridge(type = "task", name_from = "title", id_from = "task_id"),
        coaching_signal
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
        entity_bridge(type = "task", name_from = "title", id_from = "task_id"),
        coaching_signal
    )]
    Completed {
        task_id: String,
        title: String,
        deviation_pct: Option<f64>,
    },

    #[ai(
        importance = 0.6,
        salience = "accumulate",
        observation_template = "Focus expired on task: {title}",
        entity_bridge(type = "task", name_from = "title", id_from = "task_id"),
        metric(
            name = "focus_expiration_rate",
            value_from = 1.0_f64,
            window = "7d",
            min_samples = 3,
            aggregation = "sum",
        )
    )]
    FocusExpired { task_id: String, title: String },

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
        observation_template = "Deferred '{title}' from {previous_due:?} to {new_due:?}",
        entity_bridge(type = "task", name_from = "title", id_from = "task_id"),
        metric(
            name = "task_deferral_rate",
            value_from = 1.0_f64,
            window = "7d",
            min_samples = 3,
            aggregation = "sum",
        )
    )]
    Deferred {
        task_id: String,
        title: String,
        previous_due: Option<String>,
        new_due: Option<String>,
    },

    #[ai(
        importance = 0.5,
        salience = "accumulate",
        observation_template = "Estimation recorded: est {estimated_minutes:?}m vs actual {actual_minutes:?}m",
        metric(
            name = "task_estimation_bias",
            value_from = *deviation_pct,
            window = "7d",
            min_samples = 3,
            aggregation = "avg",
        ),
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
            TaskEvent::FocusExpired { task_id, title: _ } => DomainEvent::TaskFocusExpired {
                task_id,
                title: String::new(),
            },
            TaskEvent::FocusChanged {
                task_id,
                title: _,
                focus_deadline,
            } => DomainEvent::TaskFocusChanged {
                task_id,
                focus_deadline: focus_deadline.map(|d| d.to_string()),
            },
            TaskEvent::Deferred {
                task_id,
                title: _,
                previous_due: _,
                new_due: _,
            } => DomainEvent::TaskDeferred {
                task_id,
                times_deferred: 1,
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

/// Translate a bus::DomainEvent into the typed TaskEvent form, if it's a task event.
pub fn try_from_domain_event(e: &DomainEvent) -> Option<TaskEvent> {
    match e {
        DomainEvent::TaskCreated {
            task_id,
            project,
            estimate_mins,
            ..
        } => Some(TaskEvent::Created {
            task_id: task_id.clone(),
            title: String::new(),
            area_id: String::new(),
            project_id: project.clone(),
            priority: None,
            estimated_minutes: estimate_mins.map(|m| m as i32),
        }),
        DomainEvent::TaskCompleted {
            task_id,
            deviation_pct,
            ..
        } => Some(TaskEvent::Completed {
            task_id: task_id.clone(),
            title: String::new(),
            deviation_pct: *deviation_pct,
        }),
        DomainEvent::TaskFocusChanged {
            task_id,
            focus_deadline,
            ..
        } => Some(TaskEvent::FocusChanged {
            task_id: task_id.clone(),
            title: String::new(),
            focus_deadline: focus_deadline.as_ref().and_then(|d| d.parse().ok()),
        }),
        DomainEvent::TaskFocusExpired { task_id, title } => Some(TaskEvent::FocusExpired {
            task_id: task_id.clone(),
            title: title.clone(),
        }),
        DomainEvent::TaskDeferred { task_id, .. } => Some(TaskEvent::Deferred {
            task_id: task_id.clone(),
            title: String::new(),
            previous_due: None,
            new_due: None,
        }),
        DomainEvent::EstimationRecorded {
            task_id,
            estimated_mins,
            actual_mins,
            ..
        } => Some(TaskEvent::EstimationRecorded {
            task_id: task_id.clone(),
            estimated_minutes: Some(*estimated_mins as i32),
            actual_minutes: Some(*actual_mins as i32),
            deviation_pct: 0.0,
        }),
        _ => None,
    }
}
