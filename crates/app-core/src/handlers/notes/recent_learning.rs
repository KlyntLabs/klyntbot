use desktop_shared::commands::RecentLearningSession;
use desktop_shared::errors::ApiError;

use crate::errors::map_storage_err;
use crate::state::AppCore;

impl AppCore {
    pub async fn flashcard_recent_learning_sessions(
        &self,
        limit: usize,
    ) -> Result<Vec<RecentLearningSession>, ApiError> {
        let mut sessions = self
            .repos
            .sessions
            .list_sessions()
            .await
            .map_err(map_storage_err)?;

        // Sort by updated_at descending, take first `limit`
        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        sessions.truncate(limit);

        let mut result = Vec::with_capacity(sessions.len());
        for s in &sessions {
            let messages = self
                .repos
                .sessions
                .get_recent_messages(&s.key, 10)
                .await
                .unwrap_or_default();

            let preview: String = messages
                .iter()
                .filter(|m| m.role == "user" || m.role == "assistant")
                .map(|m| m.content.as_str())
                .collect::<Vec<_>>()
                .join(" ");

            let title = s
                .metadata
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("Untitled")
                .to_string();

            result.push(RecentLearningSession {
                session_key: s.key.clone(),
                title,
                updated_at: s.updated_at.to_rfc3339(),
                preview: preview.chars().take(200).collect(),
            });
        }

        Ok(result)
    }
}
