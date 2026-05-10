//! Loads chat-history view for the Context tab.
//!
//! Returns one ContextMessage per session_messages row, with `content`
//! holding the full structured `parts` JSON. The Context tab renders this
//! verbatim — it does not interpret part variants.

use common::{KlyntbotError, Result};
use storage::repos::Repos;

use crate::tracing::types::ContextMessage;

pub async fn load_context(repos: &Repos, session_id: &str) -> Result<Vec<ContextMessage>> {
    let messages = repos
        .sessions
        .get_messages_parts(session_id, 5_000)
        .await
        .map_err(|e| KlyntbotError::Storage(format!("klynt load_context: {e}")))?;

    let mut out = Vec::with_capacity(messages.len());
    for (idx, msg) in messages.into_iter().enumerate() {
        let content = serde_json::to_value(&msg.parts).unwrap_or(serde_json::Value::Null);
        out.push(ContextMessage {
            index: idx as u32,
            role: msg.role.to_string(),
            content,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::SessionMode;
    use storage::messages::parts::MessagePart;
    use storage::StoragePool;

    async fn fresh_repos() -> Repos {
        let pool = StoragePool::connect_in_memory().await.expect("memory pool");
        Repos::from_pool(&pool)
    }

    #[tokio::test]
    async fn empty_session_returns_empty() {
        let repos = fresh_repos().await;
        repos
            .sessions
            .upsert_session_with_mode("coding:1", SessionMode::Coding, &serde_json::json!({}))
            .await
            .unwrap();
        let out = load_context(&repos, "coding:1").await.unwrap();
        assert!(out.is_empty());
    }

    #[tokio::test]
    async fn returns_role_and_content_per_row() {
        let repos = fresh_repos().await;
        repos
            .sessions
            .upsert_session_with_mode("coding:1", SessionMode::Coding, &serde_json::json!({}))
            .await
            .unwrap();
        repos
            .sessions
            .add_message_with_parts(
                "coding:1",
                uuid::Uuid::new_v4(),
                "user",
                &[MessagePart::Text { text: "hi".into() }],
                Some("t1"),
                None,
            )
            .await
            .unwrap();
        let out = load_context(&repos, "coding:1").await.unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].role, "user");
        assert_eq!(out[0].index, 0);
        let arr = out[0].content.as_array().expect("array");
        assert_eq!(arr.len(), 1);
    }
}
