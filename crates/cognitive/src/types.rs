//! Core types for the cognitive memory system.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Memory operation result from consolidation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MemoryOp {
    Add { id: String },
    Update { id: String, old_id: String },
    Delete { id: String, superseded_by: String },
    Noop,
}

/// A semantic fact with bi-temporal markers and FSRS decay.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
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
}

/// An episodic memory entry.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
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
}

/// A procedural rule learned from reflection.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
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
}

/// Salience verdict for event filtering.
#[derive(Debug, Clone, PartialEq)]
pub enum SalienceVerdict {
    Extract,
    Accumulate,
    Discard,
}

/// Observation extracted from a DomainEvent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub domain: String,
    pub content: String,
    pub importance: f64,
    pub source_event: String,
    pub timestamp: DateTime<Utc>,
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
        ]
        .iter()
        .filter(|&&has| has)
        .count()
    }
}
