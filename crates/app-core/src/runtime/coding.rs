//! Coding-mode thread runtime.
//!
//! Wraps the existing `coding_message_send` pipeline with the `ThreadRuntime`
//! trait interface.

use std::sync::Arc;

use desktop_shared::errors::ApiError;

use crate::runtime::{ActiveTurns, StartTurnOutcome, StartTurnRequest, ThreadRuntime, TurnHandle};
use crate::state::AppCore;

pub struct CodingThreadRuntime {
    core: Arc<AppCore>,
}

impl CodingThreadRuntime {
    pub fn new(core: Arc<AppCore>) -> Self {
        Self { core }
    }
}

#[async_trait::async_trait]
impl ThreadRuntime for CodingThreadRuntime {
    async fn start_turn(&self, req: StartTurnRequest) -> Result<StartTurnOutcome, ApiError> {
        let resp = self
            .core
            .coding_message_send(&req.thread_id, &req.content, req.model)
            .await
            .map_err(|e| ApiError::new("CODING_SEND_FAILED", e.to_string()))?;

        let handle = TurnHandle {
            thread_id: req.thread_id.clone(),
            turn_id: resp.turn_id.clone(),
            generation: 0,
        };

        Ok(StartTurnOutcome {
            handle,
            user_message: None,
            stream_info: None,
            coding_response: Some(resp),
        })
    }

    async fn cancel_turn(&self, turn_id: &str) -> Result<(), ApiError> {
        self.core
            .coding_turn_interrupt("", turn_id)
            .await
            .map_err(|e| ApiError::new("CANCEL_FAILED", e.to_string()))?;
        Ok(())
    }

    fn is_active(&self, turn_id: &str) -> bool {
        self.core.active_streams.contains_key(turn_id)
    }

    fn active_turns(&self) -> &ActiveTurns {
        &self.core.active_streams
    }
}
