//! Channel-agnostic interaction rendering trait.
//!
//! Lives in Layer 0 so that channel implementations (Layer 4)
//! can implement it in the future. Only CLI renderer is built now.

use crate::{FormResponse, InteractionRequest, Result};
use async_trait::async_trait;

/// Renders an InteractionRequest and collects a FormResponse.
///
/// Implementors:
/// - CLI: Tabbed terminal UI (crates/cli/src/wizard/ask_user_prompt.rs)
/// - Future: Telegram inline keyboards, Discord components, etc.
#[async_trait]
pub trait InteractionRenderer: Send + Sync {
    async fn render(&self, request: &InteractionRequest) -> Result<FormResponse>;
}
