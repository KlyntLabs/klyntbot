//! Project memory handlers — list semantic facts scoped to a project.

use cognitive::repos::SemanticFactRepo;
use desktop_shared::cognitive_commands::SemanticFactResponse;
use desktop_shared::errors::ApiError;

use super::cognitive::fact_to_response;
use crate::errors::map_cognitive_err;
use crate::state::AppCore;

impl AppCore {
    /// List all active memories for a project.
    #[tracing::instrument(skip(self), err)]
    pub async fn project_memories_list(
        &self,
        project_id: String,
    ) -> Result<Vec<SemanticFactResponse>, ApiError> {
        let fact_repo = SemanticFactRepo::new(self.repos.pool().clone());
        let facts = fact_repo
            .list_by_project(&project_id)
            .await
            .map_err(map_cognitive_err)?;
        Ok(facts.iter().map(fact_to_response).collect())
    }

    /// List active memories for a project filtered by memory type.
    #[tracing::instrument(skip(self), err)]
    pub async fn project_memories_by_type(
        &self,
        project_id: String,
        memory_type: String,
    ) -> Result<Vec<SemanticFactResponse>, ApiError> {
        let fact_repo = SemanticFactRepo::new(self.repos.pool().clone());
        let facts = fact_repo
            .list_by_project_and_type(&project_id, &memory_type)
            .await
            .map_err(map_cognitive_err)?;
        Ok(facts.iter().map(fact_to_response).collect())
    }
}
