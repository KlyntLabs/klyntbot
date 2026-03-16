//! Launcher handler methods on AppCore.

use desktop_shared::errors::ApiError;
use feature_launcher::{DashboardData, LauncherItem};

use crate::errors::map_storage_err;
use crate::state::AppCore;

impl AppCore {
    /// Search across all providers.
    pub async fn launcher_search(&self, query: String) -> Result<Vec<LauncherItem>, ApiError> {
        let engine = self.launcher_engine()?;
        engine.search(&query, &self.repos, &self.note_repo).await
    }

    /// Record execution of a launcher item for frequency boosting.
    pub async fn launcher_execute(&self, item_id: String, kind: String) -> Result<(), ApiError> {
        let engine = self.launcher_engine()?;
        engine.record_execution(&item_id, &kind).await
    }

    /// Build dashboard data for the launcher.
    pub async fn launcher_dashboard(&self) -> Result<DashboardData, ApiError> {
        super::dashboard::build_dashboard_data(&self.repos, self.productivity_repos.as_ref()).await
    }

    /// Paste clipboard entry (retrieve content).
    pub async fn launcher_clipboard_paste(
        &self,
        id: i64,
    ) -> Result<Option<feature_launcher::ClipboardEntry>, ApiError> {
        let repo = self.launcher_clipboard_repo()?;
        repo.get(id).await.map_err(map_storage_err)
    }

    /// Delete clipboard entry.
    pub async fn launcher_clipboard_delete(&self, id: i64) -> Result<(), ApiError> {
        let repo = self.launcher_clipboard_repo()?;
        repo.delete(id).await.map_err(map_storage_err)
    }

    /// Pin/unpin clipboard entry.
    pub async fn launcher_clipboard_pin(&self, id: i64, pinned: bool) -> Result<(), ApiError> {
        let repo = self.launcher_clipboard_repo()?;
        repo.pin(id, pinned).await.map_err(map_storage_err)
    }
}
