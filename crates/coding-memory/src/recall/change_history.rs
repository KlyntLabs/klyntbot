//! Walk the SUPERSEDE chain for `(subject, predicate)`.

use crate::recall::{ChangeHistoryResponse, ChangeHistoryStep};
use cognitive::SemanticFactRepo;
use std::sync::Arc;

/// Service.
#[derive(Debug, Clone)]
pub struct ChangeHistoryService {
    fact_repo: Arc<SemanticFactRepo>,
}

impl ChangeHistoryService {
    /// Construct.
    #[must_use]
    pub fn new(fact_repo: Arc<SemanticFactRepo>) -> Self {
        Self { fact_repo }
    }

    /// Query the full chain — caller passes `(subject, predicate)`.
    /// Optional `repo` filter narrows scope.
    pub async fn query(
        &self,
        subject: &str,
        predicate: &str,
        repo: Option<&str>,
    ) -> common::Result<ChangeHistoryResponse> {
        let rows = self
            .fact_repo
            .list_chain_for(subject, predicate, repo)
            .await
            .map_err(|e| common::KlyntbotError::Storage(format!("list_chain_for: {e}")))?;
        let mut steps: Vec<ChangeHistoryStep> = rows
            .into_iter()
            .map(|f| ChangeHistoryStep {
                id: f.id,
                object: f.object,
                valid_from: f.valid_from,
                valid_until: f.valid_until,
            })
            .collect();
        steps.sort_by(|a, b| a.valid_from.cmp(&b.valid_from));
        Ok(ChangeHistoryResponse {
            subject: subject.to_string(),
            predicate: predicate.to_string(),
            steps,
        })
    }
}
