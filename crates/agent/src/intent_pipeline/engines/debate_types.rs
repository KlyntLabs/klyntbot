use providers::Message;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// --- Config ---

#[derive(Debug, Clone)]
pub struct DebateConfig {
    pub output_mode: OutputMode,
    pub max_rounds: u8,
    pub timeout_seconds: u64,
    pub consensus_threshold: f64,
    pub temperature_override: Option<f32>,
    pub confidence_floor: Option<f32>,
    pub token_budget: Option<TokenBudget>,
    pub accuracy_blend: f32,
    pub respect_user_prefs: bool,
}

impl Default for DebateConfig {
    fn default() -> Self {
        Self {
            output_mode: OutputMode::Synthesized,
            max_rounds: 6,
            timeout_seconds: 120,
            consensus_threshold: 85.0,
            temperature_override: None,
            confidence_floor: None,
            token_budget: None,
            accuracy_blend: 0.3,
            respect_user_prefs: true,
        }
    }
}

impl DebateConfig {
    pub fn for_chat() -> Self {
        Self::default()
    }

    pub fn for_notes() -> Self {
        Self {
            output_mode: OutputMode::StructuredPerPersona,
            max_rounds: 3,
            timeout_seconds: 240,
            temperature_override: Some(0.3),
            confidence_floor: Some(0.6),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputMode {
    Synthesized,
    StructuredPerPersona,
}

#[derive(Debug, Clone)]
pub struct TokenBudget {
    pub remaining_monthly: u64,
    pub daily_squad_cap: Option<u64>,
    pub estimated_tokens_per_round: u64,
}

// --- Context ---

#[derive(Debug, Clone)]
pub struct DebateContext {
    pub skill_prompt: String,
    pub conversation_history: Vec<Message>,
    pub user_message: String,
    pub cognitive_context: Option<String>,
    pub domains: Vec<String>,
}

// --- Result ---

#[derive(Debug, Clone, Serialize)]
pub struct DebateResult {
    pub persona_responses: Vec<PersonaResponse>,
    pub synthesis: String,
    pub rounds_completed: u8,
    pub total_rounds_planned: u8,
    pub consensus_reached: bool,
    pub final_consensus_score: f64,
    pub partial_reason: Option<PartialReason>,
    pub token_usage: TokenUsage,
    pub learned_weights_applied: Vec<LearnedWeight>,
    pub accuracy_outcomes: Vec<AccuracyOutcome>,
    pub rounds: Vec<DebateRound>,
}

impl DebateResult {
    /// Create an empty result for when the debate is cancelled (e.g. budget approval rejected).
    pub fn empty_cancelled() -> Self {
        Self {
            persona_responses: Vec::new(),
            synthesis: String::new(),
            rounds_completed: 0,
            total_rounds_planned: 0,
            consensus_reached: false,
            final_consensus_score: 0.0,
            partial_reason: Some(PartialReason::Cancelled),
            token_usage: TokenUsage::default(),
            learned_weights_applied: Vec::new(),
            accuracy_outcomes: Vec::new(),
            rounds: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PersonaResponse {
    pub persona_id: String,
    pub persona_name: String,
    pub persona_icon: String,
    pub persona_role: String,
    pub content: String,
    pub round: u8,
    pub phase: DebatePhase,
    pub confidence: f64,
    pub effective_confidence: f64,
    pub consensus_alignment: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DebateRound {
    pub round: u8,
    pub phase: DebatePhase,
    pub responses: Vec<PersonaResponse>,
    pub judge_decision: Option<JudgeDecision>,
    pub persona_order: Vec<String>,
    pub order_reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DebatePhase {
    Opening,
    Discussion,
    Targeted,
    Final,
}

impl DebatePhase {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Opening => "opening",
            Self::Discussion => "discussion",
            Self::Targeted => "targeted",
            Self::Final => "final",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JudgeDecision {
    pub consensus_score: f64,
    pub decision: String,
    pub speaking_order: Vec<String>,
    pub challenges: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PartialReason {
    Timeout { elapsed_seconds: u64 },
    BudgetCap { estimated_cost: u64, remaining: u64 },
    ConsensusEarly { at_round: u8 },
    Cancelled,
}

#[derive(Debug, Clone, Serialize)]
pub struct LearnedWeight {
    pub persona_id: String,
    pub base_weight: f64,
    pub accuracy_weight: f64,
    pub blended_weight: f64,
    pub domain: String,
    pub debates_used: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct AccuracyOutcome {
    pub persona_id: String,
    pub squad_id: String,
    pub domain: String,
    pub consensus_alignment: f64,
    pub fsrs_rating: u8,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct TokenUsage {
    pub persona_tokens: u64,
    pub judge_tokens: u64,
    pub synthesis_tokens: u64,
    pub total: u64,
}

impl TokenUsage {
    pub fn add_persona(&mut self, tokens: u64) {
        self.persona_tokens += tokens;
        self.total += tokens;
    }

    pub fn add_judge(&mut self, tokens: u64) {
        self.judge_tokens += tokens;
        self.total += tokens;
    }

    pub fn add_synthesis(&mut self, tokens: u64) {
        self.synthesis_tokens += tokens;
        self.total += tokens;
    }
}

// --- Events ---

#[derive(Debug, Clone)]
pub enum DebateEvent {
    BudgetCheck {
        estimated_cost: u64,
        remaining: u64,
        action: BudgetAction,
    },
    RoundStarted {
        round: u8,
        phase: DebatePhase,
        persona_order: Vec<String>,
        order_reason: String,
    },
    PersonaPerspective {
        persona_id: String,
        name: String,
        icon: String,
        role: String,
        content: String,
        round: u8,
        challenge: Option<String>,
    },
    JudgeDecision {
        round: u8,
        consensus_score: f64,
        speaking_order: Vec<String>,
        decision: String,
    },
    RoundCompleted {
        round: u8,
        phase: DebatePhase,
    },
    ConsensusReached {
        round: u8,
        score: f64,
    },
    DebatePartial {
        reason: PartialReason,
        completed_rounds: u8,
    },
    SynthesisStarted,
    SynthesisChunk {
        content: String,
    },
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetAction {
    Proceed,
    ReducedRounds { from: u8, to: u8 },
    RequiresApproval,
}

// --- Interaction Modes ---

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SquadInteractionMode {
    Debate,
    DirectAddress { persona_id: String },
    LeadResponse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DefaultInteractionMode {
    Lead,
    Debate,
    Smart,
}

impl Default for DefaultInteractionMode {
    fn default() -> Self {
        Self::Lead
    }
}

impl DefaultInteractionMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Lead => "lead",
            Self::Debate => "debate",
            Self::Smart => "smart",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "debate" => Self::Debate,
            "smart" => Self::Smart,
            _ => Self::Lead,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DirectAddressResult {
    pub response: PersonaResponse,
    pub token_usage: TokenUsage,
    pub context_used: Vec<String>,
}
