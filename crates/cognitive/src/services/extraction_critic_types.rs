//! Types for the extraction critic (KCA Track 5).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionCriticInput {
    pub turn_text: String,
    pub extracted_facts: Vec<FactRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactRef {
    pub fact_id: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExtractionCriticOutput {
    pub verdicts: Vec<Verdict>,
    pub missed_facts: Vec<MissedFact>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Verdict {
    pub fact_id: String,
    /// "grounded" | "hallucinated" | "ambiguous"
    pub verdict: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissedFact {
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence: f64,
}
