//! Launcher handler methods on AppCore.

use desktop_shared::errors::ApiError;
use feature_launcher::{DashboardData, LauncherExecuteResult, LauncherItem};

use crate::errors::map_storage_err;
use crate::state::AppCore;

impl AppCore {
    /// Search across all providers.
    #[tracing::instrument(skip(self), err)]
    pub async fn launcher_search(&self, query: String) -> Result<Vec<LauncherItem>, ApiError> {
        let engine = self.launcher_engine()?;
        engine.search(&query, &self.repos, &self.note_repo).await
    }

    /// Record execution of a launcher item for frequency boosting.
    #[tracing::instrument(skip(self), err)]
    pub async fn launcher_execute(
        &self,
        item_id: String,
        kind: String,
    ) -> Result<LauncherExecuteResult, ApiError> {
        let engine = self.launcher_engine()?;
        let result = engine.execute(&item_id, &kind).await?;

        // Best-effort publish; non-fatal if bus is absent
        if let Some(bus) = self.domain_event_bus.as_ref() {
            let event = bus::DomainEvent::LauncherItemExecuted {
                item_id,
                kind,
                query: None,
            };
            bus.publish(event);
        }

        Ok(result)
    }

    /// Build dashboard data for the launcher.
    #[tracing::instrument(skip(self), err)]
    pub async fn launcher_dashboard(&self) -> Result<DashboardData, ApiError> {
        super::dashboard::build_dashboard_data(&self.repos, self.productivity_repos.as_ref()).await
    }

    /// Paste clipboard entry (retrieve content).
    #[tracing::instrument(skip(self), err)]
    pub async fn launcher_clipboard_paste(
        &self,
        id: i64,
    ) -> Result<Option<feature_launcher::ClipboardEntry>, ApiError> {
        let repo = self.launcher_clipboard_repo()?;
        repo.get(id).await.map_err(map_storage_err)
    }

    /// Delete clipboard entry.
    #[tracing::instrument(skip(self), err)]
    pub async fn launcher_clipboard_delete(&self, id: i64) -> Result<(), ApiError> {
        let repo = self.launcher_clipboard_repo()?;
        repo.delete(id).await.map_err(map_storage_err)
    }

    /// Pin/unpin clipboard entry.
    #[tracing::instrument(skip(self), err)]
    pub async fn launcher_clipboard_pin(&self, id: i64, pinned: bool) -> Result<(), ApiError> {
        let repo = self.launcher_clipboard_repo()?;
        repo.pin(id, pinned).await.map_err(map_storage_err)
    }

    /// Pin a launcher item.
    #[tracing::instrument(skip(self), err)]
    pub async fn launcher_pin(&self, item_id: String, kind: String) -> Result<(), ApiError> {
        self.launcher_engine()?.pins_repo.pin(&item_id, &kind).await.map_err(Into::into)
    }

    /// Unpin a launcher item.
    #[tracing::instrument(skip(self), err)]
    pub async fn launcher_unpin(&self, item_id: String, kind: String) -> Result<(), ApiError> {
        self.launcher_engine()?.pins_repo.unpin(&item_id, &kind).await.map_err(Into::into)
    }

    /// List pinned launcher items.
    #[tracing::instrument(skip(self), err)]
    pub async fn launcher_list_pinned(&self) -> Result<Vec<feature_launcher::Pin>, ApiError> {
        self.launcher_engine()?.pins_repo.list_pinned().await.map_err(Into::into)
    }
}
