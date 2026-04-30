use serde::{Deserialize, Serialize};
use storage::{Repos, SessionRow};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum ChatMode {
    Chat,
    Coding,
}

#[derive(Debug, Error)]
pub enum ModeError {
    #[error(transparent)]
    Storage(#[from] storage::StorageError),
    #[error("session not found: {0}")]
    NotFound(String),
}

#[tracing::instrument(skip(repos), err)]
pub async fn set_mode(
    repos: &Repos,
    session_key: &str,
    mode: ChatMode,
) -> Result<SessionRow, ModeError> {
    let s = match mode {
        ChatMode::Chat => "chat",
        ChatMode::Coding => "coding",
    };
    repos
        .sessions
        .update_conversation_type(session_key, s)
        .await?;
    let row = match repos.sessions.get_session(session_key).await {
        Ok(row) => row,
        Err(storage::StorageError::NotFound(_)) => {
            return Err(ModeError::NotFound(session_key.into()));
        }
        Err(e) => return Err(e.into()),
    };
    Ok(row)
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage::StoragePool;

    #[tokio::test]
    async fn set_mode_persists() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repos = Repos::from_pool(&pool);
        let key = "u1";
        repos
            .sessions
            .upsert_session(key, &serde_json::json!({}))
            .await
            .unwrap();
        let row = set_mode(&repos, key, ChatMode::Coding).await.unwrap();
        assert_eq!(row.conversation_type.as_deref(), Some("coding"));
    }
}
