//! Assistant-mode thread runtime.
//!
//! Wraps the existing `chat_send` + `relay_chat_stream` pipeline with the
//! `ThreadRuntime` trait interface.

use std::sync::Arc;

use desktop_shared::errors::ApiError;

use crate::runtime::{ActiveTurns, StartTurnOutcome, StartTurnRequest, ThreadRuntime, TurnHandle};
use crate::state::AppCore;

pub struct AssistantThreadRuntime {
    core: Arc<AppCore>,
}

impl AssistantThreadRuntime {
    pub fn new(core: Arc<AppCore>) -> Self {
        Self { core }
    }
}

#[async_trait::async_trait]
impl ThreadRuntime for AssistantThreadRuntime {
    async fn start_turn(&self, req: StartTurnRequest) -> Result<StartTurnOutcome, ApiError> {
        let (user_msg, stream_info) = self
            .core
            .chat_send(req.content, req.thread_id.clone(), req.context, req.mode)
            .await?;

        let handle = TurnHandle {
            thread_id: req.thread_id.clone(),
            turn_id: stream_info.session_key.clone(),
            generation: 0,
        };

        Ok(StartTurnOutcome {
            handle,
            user_message: Some(user_msg),
            stream_info: Some(stream_info),
        })
    }

    async fn cancel_turn(&self, turn_id: &str) -> Result<(), ApiError> {
        self.core.chat_cancel(turn_id.to_string()).await
    }

    fn is_active(&self, turn_id: &str) -> bool {
        self.core.active_streams.contains_key(turn_id)
    }

    fn active_turns(&self) -> &ActiveTurns {
        &self.core.active_streams
    }
}
