//! Bi-temporal point-in-time lookup.

use crate::recall::{FactAsOfRow, FactsAsOfResponse};
use cognitive::SemanticFactRepo;
use jiff::Timestamp;
use std::sync::Arc;

/// Bi-temporal service.
#[derive(Debug, Clone)]
pub struct FactsAsOfService {
    fact_repo: Arc<SemanticFactRepo>,
}

impl FactsAsOfService {
    /// Construct.
    #[must_use]
    pub fn new(fact_repo: Arc<SemanticFactRepo>) -> Self {
        Self { fact_repo }
    }

    /// Query — returns all rows with `(subject, predicate)` where
    /// `valid_from <= as_of < COALESCE(valid_until, +inf)`.
    pub async fn query(
        &self,
        subject: &str,
        predicate: &str,
        as_of: Timestamp,
    ) -> common::Result<FactsAsOfResponse> {
        let rows = self
            .fact_repo
            .list_valid_at(subject, predicate, &as_of.to_string())
            .await
            .map_err(|e| common::KlyntbotError::Storage(format!("list_valid_at: {e}")))?;
        Ok(FactsAsOfResponse {
            subject: subject.to_string(),
            predicate: predicate.to_string(),
            as_of,
            rows: rows
                .into_iter()
                .map(|f| FactAsOfRow {
                    id: f.id,
                    subject: f.subject,
                    predicate: f.predicate,
                    object: f.object,
                    valid_from: f.valid_from,
                    valid_until: f.valid_until,
                    confidence: f.confidence as f32,
                })
                .collect(),
        })
    }
}
