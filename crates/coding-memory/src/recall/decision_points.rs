//! `recall_decision_points` — list decision-laden episodes.

use crate::recall::{DecisionPointRow, DecisionPointsResponse};
use cognitive::EpisodicMemoryRepo;
use std::sync::Arc;

/// Closed kind set considered "decision points" in the coding domain.
pub const DECISION_KINDS: &[&str] = &["fix_attempt", "dead_end_attempt", "refactor_episode"];

/// Service.
#[derive(Debug, Clone)]
pub struct DecisionPointsService {
    ep_repo: Arc<EpisodicMemoryRepo>,
}

impl DecisionPointsService {
    /// Construct.
    #[must_use]
    pub fn new(ep_repo: Arc<EpisodicMemoryRepo>) -> Self {
        Self { ep_repo }
    }

    /// List decision points within the optional repo scope.
    pub async fn list(
        &self,
        domain: Option<&str>,
        repo: Option<&str>,
        limit: i64,
    ) -> common::Result<DecisionPointsResponse> {
        let eps = self
            .ep_repo
            .list_by_kinds(DECISION_KINDS, repo, limit)
            .await
            .map_err(|e| common::KlyntbotError::Storage(format!("list_by_kinds: {e}")))?;
        let eps: Vec<_> = eps
            .into_iter()
            .filter(|e| match domain {
                Some(d) => e.domain == d,
                None => true,
            })
            .collect();
        Ok(DecisionPointsResponse {
            domain: "code".to_string(),
            rows: eps
                .into_iter()
                .map(|e| DecisionPointRow {
                    id: e.id,
                    kind: e.kind.unwrap_or_default(),
                    when: e.occurred_at,
                    summary: e.summary.unwrap_or_default(),
                    scope: e
                        .scope_repo_id
                        .map(|r| format!("repo:{r}"))
                        .unwrap_or_else(|| "global".to_string()),
                })
                .collect(),
        })
    }
}
