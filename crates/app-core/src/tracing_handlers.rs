use crate::state::AppCore;
use crate::tracing::types::*;
use desktop_shared::errors::ApiError;
use std::path::PathBuf;
use std::sync::Arc;

impl AppCore {
    fn tracing_provider(
        &self,
        id: &str,
    ) -> Result<Arc<dyn crate::tracing::provider::TracingProvider>, ApiError> {
        self.tracing_registry.get(id).map_err(ApiError::from)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn tracing_list_providers(&self) -> Result<Vec<ProviderInfo>, ApiError> {
        self.tracing_registry
            .list_providers()
            .await
            .map_err(ApiError::from)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn tracing_list_sessions(
        &self,
        provider_id: String,
    ) -> Result<Vec<SessionSummary>, ApiError> {
        self.tracing_provider(&provider_id)?
            .list_sessions()
            .await
            .map_err(ApiError::from)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn tracing_load_session(
        &self,
        provider_id: String,
        session_id: String,
        scope: Scope,
    ) -> Result<SessionDetail, ApiError> {
        self.tracing_provider(&provider_id)?
            .load_session(&session_id, scope)
            .await
            .map_err(ApiError::from)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn tracing_load_context(
        &self,
        provider_id: String,
        session_id: String,
        scope: Scope,
    ) -> Result<Vec<ContextMessage>, ApiError> {
        self.tracing_provider(&provider_id)?
            .load_context(&session_id, scope)
            .await
            .map_err(ApiError::from)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn tracing_load_state(
        &self,
        provider_id: String,
        session_id: String,
    ) -> Result<SessionState, ApiError> {
        self.tracing_provider(&provider_id)?
            .load_state(&session_id)
            .await
            .map_err(ApiError::from)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn tracing_list_subagents(
        &self,
        provider_id: String,
        session_id: String,
    ) -> Result<Vec<SubagentSummary>, ApiError> {
        self.tracing_provider(&provider_id)?
            .list_subagents(&session_id)
            .await
            .map_err(ApiError::from)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn tracing_import(
        &self,
        provider_id: String,
        file_path: String,
    ) -> Result<String, ApiError> {
        self.tracing_provider(&provider_id)?
            .import_from_file(std::path::Path::new(&file_path))
            .await
            .map_err(ApiError::from)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn tracing_get_dir(
        &self,
        provider_id: String,
        session_id: String,
    ) -> Result<PathBuf, ApiError> {
        self.tracing_provider(&provider_id)?
            .open_dir(&session_id)
            .await
            .map_err(ApiError::from)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn tracing_open_dir(
        &self,
        provider_id: String,
        session_id: String,
    ) -> Result<PathBuf, ApiError> {
        let path = self.tracing_get_dir(provider_id, session_id).await?;
        // Open in Finder via standard `open` command.
        let _ = tokio::process::Command::new("open").arg(&path).spawn();
        Ok(path)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn tracing_stats(&self, provider_id: String) -> Result<StatsBundle, ApiError> {
        self.tracing_provider(&provider_id)?
            .stats()
            .await
            .map_err(ApiError::from)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn tracing_session_summary(
        &self,
        provider_id: String,
        session_id: String,
    ) -> Result<SessionSummary, ApiError> {
        self.tracing_provider(&provider_id)?
            .session_summary(&session_id)
            .await
            .map_err(ApiError::from)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn tracing_load_subagent_session(
        &self,
        provider_id: String,
        session_id: String,
        agent_id: String,
    ) -> Result<SessionDetail, ApiError> {
        self.tracing_provider(&provider_id)?
            .load_subagent_session(&session_id, &agent_id)
            .await
            .map_err(ApiError::from)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn tracing_load_subagent_context(
        &self,
        provider_id: String,
        session_id: String,
        agent_id: String,
    ) -> Result<Vec<ContextMessage>, ApiError> {
        self.tracing_provider(&provider_id)?
            .load_subagent_context(&session_id, &agent_id)
            .await
            .map_err(ApiError::from)
    }
}
