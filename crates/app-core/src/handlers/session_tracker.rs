use desktop_shared::errors::ApiError;
use feature_session_tracker::types::{
    BrainstormConversation, BrainstormMessage, BrainstormMode, PinnedMessage, SessionContext,
    SessionMessage, SessionStatus, TrackedSession,
};
use futures_util::StreamExt;
use providers::{ChatParams, Message, UserContent};
use std::path::Path;
use tokio::sync::mpsc;

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

    // --- Brainstorm Message Editing ---

    pub async fn edit_brainstorm_message(
        &self,
        message_id: &str,
        edited_content: &str,
    ) -> Result<(), ApiError> {
        self.session_tracker_repos
            .update_brainstorm_message_edit(message_id, edited_content)
            .await
            .map_err(map_storage_err)
    }

    // --- Brainstorm LLM Integration ---

    /// Send a brainstorm message and stream the LLM response.
    ///
    /// Returns an `mpsc::Receiver<String>` of token chunks. The caller drains it
    /// and forwards tokens to the frontend (e.g., via Tauri events or SSE).
    /// The final saved `BrainstormMessage` is returned after the stream completes.
    pub async fn send_brainstorm_message(
        &self,
        conversation_id: &str,
        content: &str,
    ) -> Result<(mpsc::Receiver<String>, tokio::task::JoinHandle<Result<BrainstormMessage, ApiError>>), ApiError> {
        // 1. Load conversation
        let conv = self
            .session_tracker_repos
            .get_conversation(conversation_id)
            .await
            .map_err(map_storage_err)?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "brainstorm conversation not found"))?;

        // 2. Save user message to DB
        let user_msg = BrainstormMessage {
            id: uuid::Uuid::new_v4().to_string(),
            conversation_id: conversation_id.to_string(),
            role: "user".to_string(),
            content: content.to_string(),
            is_result_block: false,
            edited_content: None,
            sent_to_cc: false,
            created_at: chrono::Utc::now(),
        };
        self.session_tracker_repos
            .add_brainstorm_message(&user_msg)
            .await
            .map_err(map_storage_err)?;

        // 3. Build LLM messages: system context + conversation history + new user message
        let context = self.get_session_context(&conv.session_id).await?;
        let history = self
            .session_tracker_repos
            .list_brainstorm_messages(conversation_id)
            .await
            .map_err(map_storage_err)?;

        let system_prompt = format!(
            "You are a brainstorming assistant helping analyze a Claude Code session.\n\n\
             ## Session Context\n\
             Rolling summary: {}\n\n\
             ## Pinned Messages\n{}\n\n\
             Be concise and insightful. Help the user think through their code and ideas.",
            context.rolling_summary,
            context
                .pinned_messages
                .iter()
                .map(|p| format!("- [{}] {}", p.message_role, p.message_content))
                .collect::<Vec<_>>()
                .join("\n"),
        );

        let mut messages = vec![Message::System {
            content: system_prompt,
        }];
        for msg in &history {
            let display = msg.edited_content.as_deref().unwrap_or(&msg.content);
            match msg.role.as_str() {
                "user" => messages.push(Message::User {
                    content: UserContent::Text(display.to_string()),
                }),
                "assistant" => messages.push(Message::Assistant {
                    content: Some(display.to_string()),
                    tool_calls: None,
                    reasoning_content: None,
                }),
                _ => {}
            }
        }

        // 4. Create provider and stream
        let config = self.config.read().await;
        let model = conv
            .model_key
            .as_deref()
            .unwrap_or(&config.agents.defaults.model);
        let params = ChatParams {
            model: model.to_string(),
            temperature: Some(0.7),
            max_tokens: Some(4096),
            response_format: None,
        };
        let provider = providers::create_provider(&config)
            .map_err(|e| ApiError::new("PROVIDER_ERROR", format!("no LLM provider: {e}")))?
            .0;
        drop(config);

        let (token_tx, token_rx) = mpsc::channel::<String>(64);

        // 5. Spawn streaming task
        let repos = self.session_tracker_repos.clone();
        let conv_id = conversation_id.to_string();
        let handle = tokio::spawn(async move {
            let mut full_response = String::new();

            let stream_result = provider.chat_stream(&messages, None, &params).await;
            match stream_result {
                Ok(mut stream) => {
                    while let Some(chunk_result) = stream.next().await {
                        match chunk_result {
                            Ok(chunk) => {
                                if let Some(ref text) = chunk.content {
                                    full_response.push_str(text);
                                    let _ = token_tx.send(text.clone()).await;
                                }
                            }
                            Err(e) => {
                                tracing::warn!("brainstorm stream error: {e}");
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    // Fallback to non-streaming
                    tracing::warn!("brainstorm streaming not available, using non-streaming: {e}");
                    match provider.chat(&messages, None, &params).await {
                        Ok(resp) => {
                            if let Some(text) = resp.content {
                                full_response = text.clone();
                                let _ = token_tx.send(text).await;
                            }
                        }
                        Err(e) => {
                            return Err(ApiError::new(
                                "LLM_ERROR",
                                format!("brainstorm LLM call failed: {e}"),
                            ));
                        }
                    }
                }
            }

            // 6. Save assistant message
            let assistant_msg = BrainstormMessage {
                id: uuid::Uuid::new_v4().to_string(),
                conversation_id: conv_id,
                role: "assistant".to_string(),
                content: full_response,
                is_result_block: true,
                edited_content: None,
                sent_to_cc: false,
                created_at: chrono::Utc::now(),
            };
            repos
                .add_brainstorm_message(&assistant_msg)
                .await
                .map_err(map_storage_err)?;

            Ok(assistant_msg)
        });

        Ok((token_rx, handle))
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
