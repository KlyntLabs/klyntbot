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
        observation_template = "Focus session started: {session_type} ({target_mins}m)",
    )]
    FocusSessionStarted {
        session_type: String,
        target_mins: i64,
    },

    #[ai(
        importance = 0.5,
        salience = "accumulate",
        observation_template = "Focus session ended: {duration_secs}s, quality {quality:.2}",
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
    )]
    DistractionDetected {
        app: String,
        duration_secs: Option<i64>,
        context: String,
    },

    #[ai(
        importance = 0.4,
        salience = "accumulate",
        observation_template = "Activity completed: {date} ({total_active_secs}s active)",
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
    )]
    ProductivityScoreComputed {
        date: String,
        score: f64,
    },
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
