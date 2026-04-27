use desktop_shared::commands::InboxItemResponse;
use desktop_shared::errors::ApiError;

use crate::errors::map_storage_err;
use crate::state::AppCore;

impl AppCore {
    #[tracing::instrument(skip(self, content), err)]
    pub async fn inbox_create(&self, content: &str) -> Result<InboxItemResponse, ApiError> {
        let item = self
            .note_repo
            .create_inbox_item(content)
            .await
            .map_err(map_storage_err)?;

        Ok(InboxItemResponse {
            id: item.id,
            content: item.content,
            status: item.status,
            created_at: item.created_at,
        })
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn inbox_list(&self) -> Result<Vec<InboxItemResponse>, ApiError> {
        let items = self
            .note_repo
            .list_inbox_items()
            .await
            .map_err(map_storage_err)?;

        Ok(items
            .into_iter()
            .map(|i| InboxItemResponse {
                id: i.id,
                content: i.content,
                status: i.status,
                created_at: i.created_at,
            })
            .collect())
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn inbox_delete(&self, id: &str) -> Result<(), ApiError> {
        self.note_repo
            .delete_inbox_item(id)
            .await
            .map_err(map_storage_err)
    }
}
