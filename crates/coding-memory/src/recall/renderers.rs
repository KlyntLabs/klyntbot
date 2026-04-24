//! Markdown renderers for passive injection (SessionStart + UserPromptSubmit).
//!
//! Phase 1 stubs. Phase 4 implements full rendering against the budget caps
//! (800 / 1500 tokens — invariant #9).

use crate::error::NotImplementedInPhase;
use common::{KlyntbotError, Result};

/// Token budget for SessionStart injection (design §8).
pub const SESSION_START_BUDGET_TOKENS: u32 = 800;
/// Token budget for UserPromptSubmit injection (design §8).
pub const USER_PROMPT_BUDGET_TOKENS: u32 = 1500;

/// Render the SessionStart injection block for a given repo.
pub async fn render_session_start_block(_repo: Option<&str>) -> Result<String> {
    Err(phase(4))
}

/// Render the UserPromptSubmit injection block.
pub async fn render_user_prompt_block(
    _query: &str,
    _repo: Option<&str>,
) -> Result<String> {
    Err(phase(4))
}

fn phase(p: u8) -> KlyntbotError {
    KlyntbotError::NotImplemented(format!("{:?}", NotImplementedInPhase::new(p)))
}
