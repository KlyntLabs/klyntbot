use desktop_shared::commands::view::{ActiveViewResponse, SetActiveViewParams};
use desktop_shared::errors::ApiError;

use crate::AppCore;

impl AppCore {
    /// Update the shared active view from a desktop navigation event.
    #[tracing::instrument(skip(self), err)]
    pub async fn view_set_active(&self, params: SetActiveViewParams) -> Result<(), ApiError> {
        let Some(ref view_lock) = self.active_view else {
            return Ok(());
        };
        *view_lock.write().await = Some(context_engine::ActiveView {
            dashboard: params.dashboard,
            focused_entity: params.focused_entity,
            description: params.description,
        });
        Ok(())
    }

    /// Clear the active view (e.g., when navigating to chat-only page).
    #[tracing::instrument(skip(self), err)]
    pub async fn view_clear_active(&self) -> Result<(), ApiError> {
        if let Some(ref view_lock) = self.active_view {
            *view_lock.write().await = None;
        }
        Ok(())
    }

    /// Get the current active view.
    #[tracing::instrument(skip(self), err)]
    pub async fn view_get_active(&self) -> Result<ActiveViewResponse, ApiError> {
        let view = match self.active_view {
            Some(ref view_lock) => view_lock.read().await.clone(),
            None => None,
        };
        Ok(view
            .map(|v| ActiveViewResponse {
                dashboard: Some(v.dashboard),
                focused_entity: v.focused_entity,
                description: v.description,
            })
            .unwrap_or_default())
    }
}
