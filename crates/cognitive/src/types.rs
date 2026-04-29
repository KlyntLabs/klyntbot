//! Core types for the cognitive memory system.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

/// Memory operation result from consolidation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MemoryOp {
    Add { id: String },
    Update { id: String, old_id: String },
    Delete { id: String, superseded_by: String },
    Noop,
}

/// Default value for `SemanticFact::memory_type`.
pub const DEFAULT_MEMORY_TYPE: &str = "fact";

/// A semantic fact with bi-temporal markers and FSRS decay.
#[derive(Debug, Clone, Default, Serialize, Deserialize, sqlx::FromRow)]
#[sqlx(default)]
pub struct SemanticFact {
    pub id: String,
    pub domain: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence: f64,
    pub source: String,

    pub valid_from: String,
    pub valid_until: Option<String>,
    pub recorded_at: String,
    pub superseded_at: Option<String>,
    pub superseded_by: Option<String>,

    pub stability: f64,
    pub last_accessed: Option<String>,
    pub access_count: i64,
    pub convergence_score: f64,
    pub project_id: Option<String>,
    pub memory_type: String,
    pub scope_type: String,
    pub scope_id: Option<String>,
    pub scope_repo_id: Option<String>,
    pub metadata: Option<String>,
}

/// An episodic memory entry.
#[derive(Debug, Clone, Default, Serialize, Deserialize, sqlx::FromRow)]
#[sqlx(default)]
pub struct EpisodicMemory {
    pub id: String,
    pub domain: String,
    pub content: String,
    pub summary: Option<String>,
    pub importance: f64,
    pub occurred_at: String,
    pub recorded_at: String,
    pub stability: f64,
    pub last_accessed: Option<String>,
    pub access_count: i64,
    pub project_id: Option<String>,
    pub scope_type: String,
    pub scope_id: Option<String>,
    pub scope_repo_id: Option<String>,
    pub metadata: Option<String>,
    pub kind: Option<String>,
    pub tier: String,
    pub parent_id: Option<String>,
    pub child_count: i64,
    pub rolled_up_at: Option<String>,
}

/// A procedural rule learned from reflection.
#[derive(Debug, Clone, Default, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProceduralRule {
    pub id: String,
    pub domain: String,
    pub rule_text: String,
    pub confidence: f64,
    pub source: String,
    pub signal_count: i64,
    pub created_at: String,
    pub updated_at: String,
    pub active: bool,
    pub project_id: Option<String>,
    pub scope_type: String,
    pub scope_id: Option<String>,
    pub effectiveness_score: f64,
    pub stability: f64,
    pub scope_repo_id: Option<String>,
    pub last_applied: Option<String>,
    pub application_count: i64,
    pub metadata: Option<String>,
}

/// Minimum priority level considered "critical" for context injection.
pub const PRIORITY_CRITICAL: i32 = 2;

/// A persistent annotation attached to any entity (tool, fact, rule, skill, project).
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Annotation {
    pub id: String,
    pub target_type: String,
    pub target_id: String,
    pub content: String,
    pub tags: String,
    pub author: String,
    pub priority: i32,
    pub created_at: String,
    pub updated_at: String,
    pub expires_at: Option<String>,
    pub access_count: i64,
    pub mark_id: Option<String>,
    pub quoted_text: Option<String>,
    pub range_start: Option<i64>,
    pub range_end: Option<i64>,
    pub ai_suggestion: Option<String>,
}

/// Salience verdict for event filtering.
#[derive(Debug, Clone, PartialEq)]
pub enum SalienceVerdict {
    Extract,
    Accumulate,
    Discard,
}

impl SalienceVerdict {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Extract => "extract",
            Self::Accumulate => "accumulate",
            Self::Discard => "discard",
        }
    }
}

/// Observation extracted from a DomainEvent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub domain: String,
    pub content: String,
    pub importance: f64,
    pub source_event: String,
    pub timestamp: Timestamp,
}

/// The structured user model — queryable, domain-organized.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserModel {
    pub identity: Vec<SemanticFact>,
    pub energy: Vec<SemanticFact>,
    pub work: Vec<SemanticFact>,
    pub finance: Vec<SemanticFact>,
    pub learning: Vec<SemanticFact>,
    pub preferences: Vec<SemanticFact>,
    /// Facts from domains not covered by the named fields above
    /// (e.g. "general", "tasks", "coaching", "meta").
    #[serde(default)]
    pub other: Vec<SemanticFact>,
}

impl UserModel {
    /// Total number of active facts across all domains.
    pub fn active_fact_count(&self) -> usize {
        self.identity.len()
            + self.energy.len()
            + self.work.len()
            + self.finance.len()
            + self.learning.len()
            + self.preferences.len()
            + self.other.len()
    }

    /// Number of domains that have at least one fact.
    pub fn non_empty_domain_count(&self) -> usize {
        [
            !self.identity.is_empty(),
            !self.energy.is_empty(),
            !self.work.is_empty(),
            !self.finance.is_empty(),
            !self.learning.is_empty(),
            !self.preferences.is_empty(),
            !self.other.is_empty(),
        ]
        .iter()
        .filter(|&&has| has)
        .count()
    }
}
