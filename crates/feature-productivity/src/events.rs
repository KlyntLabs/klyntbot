use ai_core_macros::AiEvent;
use bus::DomainEvent;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, AiEvent, Serialize, Deserialize)]
#[ai(domain = "Productivity")]
pub enum ProductivityEvent {
    #[ai(
        importance = 0.5,
        salience = "accumulate",
        observation_template = "Session ended: {duration_mins}m, quality {quality:.2}",
        metric(
            name = "focus_quality_trend",
            value_from = *quality,
            window = "7d",
            min_samples = 3,
            aggregation = "avg",
        ),
    )]
    SessionEnded {
        session_id: String,
        quality: f64,
        duration_mins: u32,
    },

    #[ai(
        importance = 0.4,
        salience = "accumulate",
        observation_template = "Focus session started: {session_type} ({target_mins}m)"
    )]
    FocusSessionStarted {
        session_type: String,
        target_mins: i64,
    },

    #[ai(
        importance = 0.5,
        salience = "accumulate",
        observation_template = "Focus session ended: {duration_secs}s, quality {quality:.2}",
        metric(
            name = "focus_session_duration_trend",
            value_from = *duration_secs as f64,
            window = "7d",
            min_samples = 3,
            aggregation = "avg",
        )
    )]
    FocusSessionEnded {
        duration_secs: i64,
        quality: f64,
        interruptions: i32,
    },

    #[ai(
        importance = 0.6,
        salience = "accumulate",
        observation_template = "Distraction: {app} — {context}",
        coaching_signal,
        metric(
            name = "distraction_frequency",
            value_from = 1.0_f64,
            window = "7d",
            min_samples = 3,
            aggregation = "sum",
        )
    )]
    DistractionDetected {
        app: String,
        duration_secs: Option<i64>,
        context: String,
    },

    #[ai(
        importance = 0.4,
        salience = "accumulate",
        observation_template = "Activity completed: {date} ({total_active_secs}s active)"
    )]
    ActivitySessionCompleted {
        date: String,
        total_active_secs: i64,
        productive_secs: i64,
        distracting_secs: i64,
    },

    #[ai(
        importance = 0.4,
        salience = "extract_if(*score > 0.8 || *score < 0.3)",
        observation_template = "Productivity score: {score:.2}",
        metric(
            name = "productivity_score_trend",
            value_from = *score,
            window = "14d",
            min_samples = 3,
            aggregation = "avg",
        )
    )]
    ProductivityScoreComputed { date: String, score: f64 },
}

impl From<ProductivityEvent> for DomainEvent {
    fn from(e: ProductivityEvent) -> Self {
        match e {
            ProductivityEvent::SessionEnded {
                session_id,
                quality,
                duration_mins,
            } => DomainEvent::ProductivitySessionEnded {
                session_id,
                quality,
                duration_mins,
            },
            ProductivityEvent::FocusSessionStarted {
                session_type,
                target_mins,
            } => DomainEvent::FocusSessionStarted {
                session_type,
                target_mins,
            },
            ProductivityEvent::FocusSessionEnded {
                duration_secs,
                quality,
                interruptions,
            } => DomainEvent::FocusSessionEnded {
                duration_secs,
                quality,
                interruptions,
            },
            ProductivityEvent::DistractionDetected {
                app,
                duration_secs,
                context,
            } => DomainEvent::DistractionDetected {
                app,
                duration_secs,
                context,
            },
            ProductivityEvent::ActivitySessionCompleted {
                date,
                total_active_secs,
                productive_secs,
                distracting_secs,
            } => DomainEvent::ActivitySessionCompleted {
                date,
                total_active_secs,
                productive_secs,
                distracting_secs,
            },
            ProductivityEvent::ProductivityScoreComputed { date, score } => {
                DomainEvent::ProductivityScoreComputed { date, score }
            }
        }
    }
}

/// Translate a bus::DomainEvent into the typed ProductivityEvent form, if applicable.
pub fn try_from_domain_event(e: &bus::DomainEvent) -> Option<ProductivityEvent> {
    use bus::DomainEvent;
    match e {
        DomainEvent::ProductivitySessionEnded {
            session_id,
            quality,
            duration_mins,
        } => Some(ProductivityEvent::SessionEnded {
            session_id: session_id.clone(),
            quality: *quality,
            duration_mins: *duration_mins,
        }),
        DomainEvent::FocusSessionStarted {
            session_type,
            target_mins,
        } => Some(ProductivityEvent::FocusSessionStarted {
            session_type: session_type.clone(),
            target_mins: *target_mins,
        }),
        DomainEvent::FocusSessionEnded {
            duration_secs,
            quality,
            interruptions,
        } => Some(ProductivityEvent::FocusSessionEnded {
            duration_secs: *duration_secs,
            quality: *quality,
            interruptions: *interruptions,
        }),
        DomainEvent::DistractionDetected {
            app,
            duration_secs,
            context,
        } => Some(ProductivityEvent::DistractionDetected {
            app: app.clone(),
            duration_secs: *duration_secs,
            context: context.clone(),
        }),
        DomainEvent::ActivitySessionCompleted {
            date,
            total_active_secs,
            productive_secs,
            distracting_secs,
        } => Some(ProductivityEvent::ActivitySessionCompleted {
            date: date.clone(),
            total_active_secs: *total_active_secs,
            productive_secs: *productive_secs,
            distracting_secs: *distracting_secs,
        }),
        DomainEvent::ProductivityScoreComputed { date, score } => {
            Some(ProductivityEvent::ProductivityScoreComputed {
                date: date.clone(),
                score: *score,
            })
        }
        _ => None,
    }
}
