use desktop_shared::errors::ApiError;
use feature_session_tracker::types::{
    BrainstormConversation, BrainstormMessage, BrainstormMode, PinnedMessage, SessionContext,
    SessionMessage, SessionStatus, TrackedSession,
};
use std::path::Path;

use crate::errors::map_storage_err;
use crate::state::AppCore;

impl AppCore {
    async fn require_session(&self, session_id: &str) -> Result<TrackedSession, ApiError> {
        self.session_tracker_repos
            .get_session(session_id)
            .await
            .map_err(map_storage_err)?
            .ok_or_else(|| ApiError::new("NOT_FOUND", format!("session '{session_id}' not found")))
    }

    // --- Session Tracking ---

    pub async fn get_tracked_sessions(&self) -> Result<Vec<TrackedSession>, ApiError> {
        self.session_tracker_repos
            .list_sessions()
            .await
            .map_err(map_storage_err)
    }

    pub async fn get_session_messages(
        &self,
        session_id: &str,
        limit: Option<usize>,
    ) -> Result<Vec<SessionMessage>, ApiError> {
        let session = self.require_session(session_id).await?;

        let content = tokio::fs::read_to_string(&session.jsonl_path)
            .await
            .map_err(|e| ApiError::new("IO_ERROR", format!("failed to read session file: {e}")))?;

        let messages = feature_session_tracker::parser::parse_lines(&content);

        match limit {
            Some(n) if n < messages.len() => Ok(messages[messages.len() - n..].to_vec()),
            _ => Ok(messages),
        }
    }

    pub async fn sync_sessions(&self) -> Result<Vec<TrackedSession>, ApiError> {
        let claude_dir = feature_session_tracker::discovery::default_claude_dir()
            .ok_or_else(|| ApiError::new("CONFIG_ERROR", "cannot determine home directory"))?;

        let discovered =
            feature_session_tracker::discovery::discover_sessions(&claude_dir).await;

        for session in &discovered {
            self.session_tracker_repos
                .upsert_session(session)
                .await
                .map_err(map_storage_err)?;
        }

        self.session_tracker_repos
            .list_sessions()
            .await
            .map_err(map_storage_err)
    }

    // --- Pinning ---

    pub async fn pin_session_message(
        &self,
        session_id: String,
        message_uuid: String,
        message_content: String,
        message_role: String,
    ) -> Result<(), ApiError> {
        let existing_pins = self
            .session_tracker_repos
            .list_pins(&session_id)
            .await
            .map_err(map_storage_err)?;

        let max_order = existing_pins.iter().map(|p| p.pin_order).max().unwrap_or(0);

        let pin = PinnedMessage {
            id: 0,
            session_id,
            message_uuid,
            message_content,
            message_role,
            pin_order: max_order + 1,
            created_at: chrono::Utc::now(),
        };

        self.session_tracker_repos
            .pin_message(&pin)
            .await
            .map_err(map_storage_err)
    }

    pub async fn unpin_session_message(&self, pin_id: i64) -> Result<(), ApiError> {
        self.session_tracker_repos
            .unpin_message(pin_id)
            .await
            .map_err(map_storage_err)
    }

    pub async fn get_pinned_messages(
        &self,
        session_id: &str,
    ) -> Result<Vec<PinnedMessage>, ApiError> {
        self.session_tracker_repos
            .list_pins(session_id)
            .await
            .map_err(map_storage_err)
    }

    // --- Send to Claude Code ---

    pub async fn send_to_claude_code(
        &self,
        session_id: &str,
        content: &str,
    ) -> Result<(), ApiError> {
        let session = self.require_session(session_id).await?;

        if session.status != SessionStatus::Active {
            return Err(ApiError::new(
                "INVALID_STATE",
                "can only send to active sessions",
            ));
        }

        feature_session_tracker::injector::send_to_session(
            Path::new(&session.jsonl_path),
            session_id,
            content,
        )
        .await
        .map_err(|e| ApiError::new("IO_ERROR", format!("failed to inject message: {e}")))?;

        Ok(())
    }

    // --- Brainstorming ---

    pub async fn create_brainstorm(
        &self,
        session_id: String,
        mode: BrainstormMode,
        model_key: Option<String>,
        agent_profile: Option<String>,
        title: Option<String>,
    ) -> Result<BrainstormConversation, ApiError> {
        self.require_session(&session_id).await?;

        let conv = BrainstormConversation {
            id: uuid::Uuid::new_v4().to_string(),
            session_id,
            title,
            mode,
            model_key,
            agent_profile,
            created_at: chrono::Utc::now(),
            updated_at: None,
        };

        self.session_tracker_repos
            .create_conversation(&conv)
            .await
            .map_err(map_storage_err)?;

        Ok(conv)
    }

    pub async fn list_brainstorms(
        &self,
        session_id: &str,
    ) -> Result<Vec<BrainstormConversation>, ApiError> {
        self.session_tracker_repos
            .list_conversations(session_id)
            .await
            .map_err(map_storage_err)
    }

    pub async fn get_brainstorm_messages(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<BrainstormMessage>, ApiError> {
        self.session_tracker_repos
            .list_brainstorm_messages(conversation_id)
            .await
            .map_err(map_storage_err)
    }

    // --- Context ---

    pub async fn get_session_context(
        &self,
        session_id: &str,
    ) -> Result<SessionContext, ApiError> {
        let session = self.require_session(session_id).await?;

        let (rolling_summary, pinned_messages) = tokio::try_join!(
            async {
                self.session_tracker_repos
                    .get_latest_summary(session_id)
                    .await
                    .map_err(map_storage_err)
                    .map(|s| s.unwrap_or_default())
            },
            async {
                self.session_tracker_repos
                    .list_pins(session_id)
                    .await
                    .map_err(map_storage_err)
            },
        )?;

        // Read recent messages from JSONL (last 30)
        let content = tokio::fs::read_to_string(&session.jsonl_path)
            .await
            .map_err(|e| ApiError::new("IO_ERROR", format!("failed to read session file: {e}")))?;
        let mut all_messages = feature_session_tracker::parser::parse_lines(&content);
        let window_size = 30;
        let recent_messages = if all_messages.len() > window_size {
            all_messages.split_off(all_messages.len() - window_size)
        } else {
            all_messages
        };

        // Rough token estimate: ~4 chars per token
        let estimated_tokens = rolling_summary.len() / 4
            + pinned_messages
                .iter()
                .map(|p| p.message_content.len() / 4)
                .sum::<usize>()
            + recent_messages.len() * 100;

        Ok(SessionContext {
            rolling_summary,
            pinned_messages,
            recent_messages,
            total_messages: session.message_count,
            estimated_tokens,
        })
    }
}
