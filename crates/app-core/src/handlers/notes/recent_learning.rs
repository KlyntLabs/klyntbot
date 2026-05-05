use desktop_shared::commands::RecentLearningSession;
use desktop_shared::errors::ApiError;

use crate::errors::map_storage_err;
use crate::handlers::chat::extract_title;
use crate::state::AppCore;

impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn flashcard_recent_learning_sessions(
        &self,
        limit: usize,
    ) -> Result<Vec<RecentLearningSession>, ApiError> {
        let sessions = self
            .repos
            .sessions
            .list_recent(limit as i64)
            .await
            .map_err(map_storage_err)?;

        // Fetch messages concurrently (limit is typically 3)
        let message_futures: Vec<_> = sessions
            .iter()
            .map(|s| self.repos.sessions.get_recent_messages(&s.key, 10))
            .collect();
        let all_messages = futures_util::future::join_all(message_futures).await;

        let result = sessions
            .iter()
            .zip(all_messages)
            .map(|(s, messages)| {
                let messages = messages.unwrap_or_default();
                let preview: String = messages
                    .iter()
                    .filter(|m| m.role == "user" || m.role == "assistant")
                    .map(|m| m.content.as_str())
                    .collect::<Vec<_>>()
                    .join(" ");

                RecentLearningSession {
                    session_key: s.key.clone(),
                    title: extract_title(&s.metadata),
                    updated_at: s.updated_at.to_string(),
                    preview: preview.chars().take(200).collect(),
                }
            })
            .collect();

        Ok(result)
    }
}
