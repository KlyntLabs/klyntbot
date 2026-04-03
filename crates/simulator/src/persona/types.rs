use std::collections::HashMap;
use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ── Fact triple ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactTriple {
    pub subject: String,
    pub predicate: String,
    pub object: String,
}

// ── Persona profile ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaProfile {
    pub known_facts: Vec<FactTriple>,
}

// ── Phase configuration ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseConfig {
    pub duration_days: u32,
    pub correction_rate: f64,
    pub topic_weights: HashMap<String, f64>,
    pub new_fact_introduction_rate: f64,
    pub tool_action_rate: f64,
    #[serde(default)]
    pub shift_description: Option<String>,
    #[serde(default)]
    pub new_facts: Vec<FactTriple>,
    /// Probability of generating an adversarial message. Default: 0.0 (opt-in).
    #[serde(default)]
    pub adversarial_rate: f64,
    /// Probability of injecting a tool execution failure. Default: 0.0.
    #[serde(default)]
    pub error_injection_rate: f64,
    /// Probability of provider returning malformed responses. Default: 0.0.
    #[serde(default)]
    pub provider_error_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaPhases {
    pub onboarding: PhaseConfig,
    pub routine: PhaseConfig,
    pub power_user: PhaseConfig,
    pub behavior_shift: PhaseConfig,
}

// ── Messages per day ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagesPerDay {
    pub onboarding: u32,
    pub routine: u32,
    pub power_user: u32,
    pub shift: u32,
}

// ── Persona ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Persona {
    pub name: String,
    pub timezone: String,
    pub language: String,
    #[serde(default = "default_seed")]
    pub seed: u64,
    pub messages_per_day: MessagesPerDay,
    pub profile: PersonaProfile,
    pub phases: PersonaPhases,
}

fn default_seed() -> u64 {
    42
}

// ── Lifecycle phase ────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecyclePhase {
    Onboarding,
    Routine,
    PowerUser,
    BehaviorShift,
}

impl fmt::Display for LifecyclePhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Onboarding => write!(f, "onboarding"),
            Self::Routine => write!(f, "routine"),
            Self::PowerUser => write!(f, "power_user"),
            Self::BehaviorShift => write!(f, "behavior_shift"),
        }
    }
}

// ── Ground truth annotation ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroundTruthAnnotation {
    pub introduces_fact: Option<FactTriple>,
    pub relevant_facts: Vec<String>,
    pub expected_skill: Option<String>,
    #[serde(default)]
    pub expected_response: Option<String>,
}

// ── Simulated tool actions ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SimulatedToolAction {
    CreateTask {
        title: String,
        due_offset_days: Option<i32>,
        project: Option<String>,
    },
    CompleteTask {
        task_ref: String,
    },
    CreateNote {
        title: String,
        content: String,
    },
    UpdateNote {
        note_ref: String,
        new_content: String,
    },
    RecordTransaction {
        amount: f64,
        category: String,
        description: String,
    },
    StartFocus {
        task_ref: Option<String>,
        duration_mins: u32,
    },
    CreateObjective {
        title: String,
        project: Option<String>,
        due_offset_days: Option<i32>,
    },
    RecordProductivityEvent {
        event_type: String,
        duration_mins: Option<u32>,
    },
    CreateFlashcard {
        front: String,
        back: String,
        topic: String,
    },
    ReviewFlashcard {
        topic: String,
        rating: u8,
    },
}

// ── Annotated message ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AnnotatedMessage {
    pub content: String,
    pub phase: LifecyclePhase,
    pub simulated_at: DateTime<Utc>,
    pub ground_truth: Option<GroundTruthAnnotation>,
    pub tool_actions: Vec<SimulatedToolAction>,
    pub is_correction: bool,
    pub topic: String,
    pub is_followup: bool,
    pub workflow: Option<crate::agent_types::WorkflowPattern>,
    pub is_adversarial: bool,
}
