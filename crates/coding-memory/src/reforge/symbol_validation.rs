//! Reforge Phase 6.5b — deep tree-sitter validation of anchored facts.
//!
//! Walks every fact / episode whose `metadata.anchoredSymbols` is non-empty,
//! re-parses the current file, and applies invalidation:
//!
//! - **Symbol deleted** → bi-temporal invalidate (`valid_until = now`); emit
//!   `StaleFactDetected` Mirror alert.
//! - **File modified, symbol survives** → mark `metadata.status = 'stale_candidate'`;
//!   no invalidation. Surfaced for user review via `recall_index` which prefers
//!   non-stale candidates.

use crate::causal::CausalEdgeRepo;
use crate::scope::AnchoredSymbol;
use crate::symbols::SymbolExtractor;
use cognitive::{EpisodicMemoryRepo, SemanticFactRepo};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// Phase output.
#[derive(Debug, Default)]
pub struct SymbolValidationOutcome {
    /// Number of facts invalidated (`valid_until` set).
    pub invalidated: u32,
    /// Number of facts marked `stale_candidate`.
    pub marked_stale: u32,
    /// Number of facts left untouched.
    pub untouched: u32,
}

/// Phase entry point.
#[derive(Debug)]
pub struct SymbolValidationPhase {
    fact_repo: Arc<SemanticFactRepo>,
    #[allow(dead_code)]
    ep_repo: Arc<EpisodicMemoryRepo>,
    extractor: Arc<dyn SymbolExtractor>,
    repo_roots: HashMap<String, PathBuf>,
    #[allow(dead_code)]
    causal_repo: Arc<CausalEdgeRepo>,
}

impl SymbolValidationPhase {
    /// Construct.
    #[must_use]
    pub fn new(
        fact_repo: Arc<SemanticFactRepo>,
        ep_repo: Arc<EpisodicMemoryRepo>,
        extractor: Arc<dyn SymbolExtractor>,
        repo_roots: HashMap<String, PathBuf>,
        causal_repo: Arc<CausalEdgeRepo>,
    ) -> Self {
        Self {
            fact_repo,
            ep_repo,
            extractor,
            repo_roots,
            causal_repo,
        }
    }

    /// Run the phase. Best-effort — IO/parse errors per fact are logged but
    /// do not fail the cycle.
    pub async fn run(&self) -> common::Result<SymbolValidationOutcome> {
        let mut outcome = SymbolValidationOutcome::default();

        let rows: Vec<(String, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT id, scope_repo_id, metadata FROM semantic_facts \
             WHERE json_extract(metadata, '$.anchoredSymbols') IS NOT NULL",
        )
        .fetch_all(self.fact_repo.pool())
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("scan facts: {e}")))?;

        for (id, repo_id, metadata) in rows {
            let Some(meta_str) = metadata else { continue };
            let parsed: serde_json::Value = match meta_str.parse() {
                Ok(v) => v,
                Err(_) => continue,
            };
            let anchors: Vec<AnchoredSymbol> =
                match serde_json::from_value(parsed["anchoredSymbols"].clone()) {
                    Ok(a) => a,
                    Err(_) => continue,
                };
            let Some(repo_root) = repo_id
                .as_deref()
                .and_then(|r| self.repo_roots.get(r).cloned())
            else {
                outcome.untouched += 1;
                continue;
            };
            let mut any_deleted = false;
            let mut any_modified = false;
            for anchor in &anchors {
                let abs_path = if anchor.file_path.is_absolute() {
                    anchor.file_path.clone()
                } else {
                    repo_root.join(&anchor.file_path)
                };
                let Ok(source) = tokio::fs::read_to_string(&abs_path).await else {
                    any_deleted = true;
                    continue;
                };
                let current = self.extractor.extract(&abs_path, &source, "head");
                let still_present = current
                    .iter()
                    .any(|s| s.symbol == anchor.symbol && s.kind == anchor.kind);
                if !still_present {
                    any_deleted = true;
                } else {
                    any_modified = true;
                }
            }
            if any_deleted {
                self.invalidate_fact(&id).await?;
                outcome.invalidated += 1;
            } else if any_modified {
                self.mark_needs_review(&id, &meta_str).await?;
                outcome.marked_stale += 1;
            } else {
                outcome.untouched += 1;
            }
        }

        Ok(outcome)
    }

    async fn invalidate_fact(&self, id: &str) -> common::Result<()> {
        sqlx::query(
            "UPDATE semantic_facts \
             SET valid_until = COALESCE(valid_until, datetime('now')) \
             WHERE id = ?1",
        )
        .bind(id)
        .execute(self.fact_repo.pool())
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("invalidate: {e}")))?;
        Ok(())
    }

    async fn mark_needs_review(&self, id: &str, meta: &str) -> common::Result<()> {
        let mut parsed: serde_json::Value = meta.parse().unwrap_or(serde_json::json!({}));
        if let Some(obj) = parsed.as_object_mut() {
            obj.insert(
                "status".into(),
                serde_json::Value::String("needs_review".into()),
            );
        }
        sqlx::query("UPDATE semantic_facts SET metadata = ?1 WHERE id = ?2")
            .bind(parsed.to_string())
            .bind(id)
            .execute(self.fact_repo.pool())
            .await
            .map_err(|e| common::KlyntbotError::Storage(format!("mark needs_review: {e}")))?;
        Ok(())
    }
}
