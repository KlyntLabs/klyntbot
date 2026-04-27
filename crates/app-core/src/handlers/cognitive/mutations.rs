//! Cognitive memory CRUD mutations.

use cognitive::repos::SemanticFactRepo;
use cognitive::types::{SemanticFact, DEFAULT_MEMORY_TYPE};
use desktop_shared::cognitive_commands::*;
use desktop_shared::errors::ApiError;

use crate::errors::map_cognitive_err;
use crate::state::AppCore;

use super::memory::{fact_to_response, rule_to_response};

impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn cognitive_fact_create(
        &self,
        params: FactCreateParams,
    ) -> Result<SemanticFactResponse, ApiError> {
        let pool = self.repos.pool();
        let fact_repo = SemanticFactRepo::new(pool.clone());

        let now = jiff::Timestamp::now().to_string();
        let fact = SemanticFact {
            id: uuid::Uuid::new_v4().to_string(),
            domain: params.domain,
            subject: params.subject,
            predicate: params.predicate,
            object: params.object,
            confidence: params.confidence,
            source: "debug_dashboard".to_string(),
            valid_from: now.clone(),
            valid_until: None,
            recorded_at: now,
            superseded_at: None,
            superseded_by: None,
            stability: 1.0,
            last_accessed: None,
            access_count: 0,
            convergence_score: 0.0,
            project_id: None,
            memory_type: DEFAULT_MEMORY_TYPE.to_string(),
            scope_type: "system".to_string(),
            scope_id: None,
            metadata: None,
            scope_repo_id: None,
        };

        fact_repo.upsert(&fact).await.map_err(map_cognitive_err)?;

        Ok(fact_to_response(&fact))
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn cognitive_fact_update(
        &self,
        id: String,
        params: FactUpdateParams,
    ) -> Result<SemanticFactResponse, ApiError> {
        let pool = self.repos.pool();
        let fact_repo = SemanticFactRepo::new(pool.clone());

        let mut fact = fact_repo
            .get(&id)
            .await
            .map_err(map_cognitive_err)?
            .ok_or_else(|| ApiError::new("NOT_FOUND", format!("fact {id} not found")))?;

        if let Some(obj) = params.object {
            fact.object = obj;
        }
        if let Some(conf) = params.confidence {
            fact.confidence = conf;
        }

        fact_repo.upsert(&fact).await.map_err(map_cognitive_err)?;

        Ok(fact_to_response(&fact))
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn cognitive_fact_delete(&self, id: String) -> Result<bool, ApiError> {
        let pool = self.repos.pool();
        let fact_repo = SemanticFactRepo::new(pool.clone());

        fact_repo
            .supersede(&id, "deleted_by_debug_dashboard")
            .await
            .map_err(map_cognitive_err)?;

        Ok(true)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn cognitive_rule_create(
        &self,
        params: RuleCreateParams,
    ) -> Result<ProceduralRuleResponse, ApiError> {
        let pool = self.repos.pool();
        let repo = cognitive::repos::ProceduralRuleRepo::new(pool.clone());
        let now = jiff::Timestamp::now().to_string();

        let rule = cognitive::types::ProceduralRule {
            id: uuid::Uuid::new_v4().to_string(),
            domain: params.domain,
            rule_text: params.rule_text,
            confidence: params.confidence,
            source: "debug_dashboard".to_string(),
            signal_count: 0,
            created_at: now.clone(),
            updated_at: now,
            active: true,
            project_id: None,
            scope_type: "system".to_string(),
            scope_id: None,
            effectiveness_score: 0.5,
            stability: 1.0,
            scope_repo_id: None,
            last_applied: None,
            application_count: 0,
            metadata: None,
        };

        repo.upsert(&rule).await.map_err(map_cognitive_err)?;

        Ok(rule_to_response(&rule))
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn cognitive_rule_deactivate(&self, id: String) -> Result<bool, ApiError> {
        let pool = self.repos.pool();
        let repo = cognitive::repos::ProceduralRuleRepo::new(pool.clone());

        repo.deactivate(&id).await.map_err(map_cognitive_err)?;

        Ok(true)
    }
}
