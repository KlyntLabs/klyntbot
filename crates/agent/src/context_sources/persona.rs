//! Persona context source — injects merged persona instructions into the system prompt.

use std::sync::Arc;

use async_trait::async_trait;
use common::SessionKey;
use context_engine::source::{ContextSource, SourceContext};
use storage::repos::SessionContextRepo;
use tokio::sync::RwLock;

use crate::persona::PersonaManager;

/// Injects the merged persona chain instructions for the current session.
pub struct PersonaContextSource {
    persona_manager: Arc<RwLock<PersonaManager>>,
    session_context_repo: SessionContextRepo,
}

impl PersonaContextSource {
    pub fn new(
        persona_manager: Arc<RwLock<PersonaManager>>,
        session_context_repo: SessionContextRepo,
    ) -> Self {
        Self {
            persona_manager,
            session_context_repo,
        }
    }
}

#[async_trait]
impl ContextSource for PersonaContextSource {
    fn name(&self) -> &str {
        "persona"
    }

    fn priority(&self) -> u8 {
        95
    }

    async fn provide(&self, ctx: &SourceContext) -> Option<String> {
        let session_key = SessionKey::from_parts(&ctx.channel, &ctx.chat_id);
        let context = self
            .session_context_repo
            .get(session_key.as_str())
            .await
            .ok()??;

        let mgr = self.persona_manager.read().await;
        let chain = mgr.for_session(&context);

        if chain.is_empty() || chain.merged_instructions.trim().is_empty() {
            return None;
        }

        let mut output = String::from("# Persona\n\n");
        if let Some(tone) = &chain.tone {
            output.push_str(&format!("**Tone:** {tone}\n\n"));
        }
        output.push_str(&chain.merged_instructions);

        Some(output)
    }
}
