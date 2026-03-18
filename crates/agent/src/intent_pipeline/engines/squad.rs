//! SquadExecutor — orchestrates parallel persona LLM calls for squad chat mode.

use cognitive::PersonaRow;
use providers::{ChatParams, DynProvider, Message, UserContent};

/// Build a persona-specific system prompt layered on orchestrator context.
pub fn build_persona_prompt(
    orchestrator_context: &str,
    persona_name: &str,
    persona_role: &str,
    persona_perspective: &str,
    persona_tone: &str,
) -> String {
    format!(
        r#"{orchestrator_context}

---

You are now responding as **{persona_name}**, a {persona_role}.
Your perspective: {persona_perspective}
Your tone should be: {persona_tone}

Respond to the user's message from your unique perspective. Be direct and specific.
Do NOT repeat other personas' points. Offer your distinct viewpoint."#
    )
}

/// Format multi-voice responses into combined markdown.
pub fn format_multi_voice(responses: &[(String, String)]) -> String {
    responses
        .iter()
        .map(|(name, content)| format!("## {name}\n\n{content}"))
        .collect::<Vec<_>>()
        .join("\n\n---\n\n")
}

/// Build synthesis prompt from multiple persona responses.
pub fn build_squad_synthesis_prompt(
    user_message: &str,
    persona_responses: &[(String, String)],
) -> String {
    let voices: String = persona_responses
        .iter()
        .map(|(name, content)| format!("**{name}:** {content}"))
        .collect::<Vec<_>>()
        .join("\n\n");

    format!(
        r#"You received the following perspectives from multiple expert personas analyzing a user's message.

User's message: {user_message}

Persona perspectives:
{voices}

Synthesize these perspectives into a single, coherent response that:
1. Integrates the strongest points from each persona
2. Resolves contradictions by explaining trade-offs
3. Provides a clear, actionable recommendation
4. Credits specific personas when their insight is particularly valuable

Respond directly to the user — do not describe the synthesis process."#
    )
}

/// Run parallel persona LLM calls. Returns (persona_name, response) pairs.
pub async fn fan_out_personas(
    provider: &DynProvider,
    orchestrator_context: &str,
    user_message: &str,
    personas: &[PersonaRow],
    params: &ChatParams,
    event_tx: Option<&tokio::sync::mpsc::Sender<crate::AgentEvent>>,
) -> Vec<(String, String)> {
    let futures: Vec<_> = personas
        .iter()
        .map(|persona| {
            let provider = provider.clone();
            let params = params.clone();
            let system = build_persona_prompt(
                orchestrator_context,
                &persona.name,
                &persona.role,
                &persona.perspective,
                &persona.tone,
            );
            let user_msg = user_message.to_string();
            let persona_name = persona.name.clone();
            let persona_id = persona.id.clone();
            let persona_icon = persona.icon.clone();
            let persona_role = persona.role.clone();
            let tx = event_tx.cloned();

            async move {
                let messages = vec![
                    Message::System { content: system },
                    Message::User {
                        content: UserContent::Text(user_msg),
                    },
                ];
                let result = provider.chat(&messages, None, &params).await;
                let text = match result {
                    Ok(r) => r.content.unwrap_or_default(),
                    Err(e) => {
                        tracing::warn!(persona = %persona_name, "Persona LLM call failed: {e}");
                        String::new()
                    }
                };

                if let Some(tx) = &tx {
                    let _ = tx
                        .send(crate::AgentEvent::PersonaPerspective {
                            persona_id: persona_id.clone(),
                            persona_name: persona_name.clone(),
                            persona_icon: persona_icon.clone(),
                            persona_role: persona_role.clone(),
                            content: text.clone(),
                        })
                        .await;
                }

                (persona_name, text)
            }
        })
        .collect();

    futures_util::future::join_all(futures).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_persona_prompt() {
        let prompt = build_persona_prompt(
            "You are an AI assistant.",
            "Skeptic",
            "Critical analyst",
            "Questions claims and looks for weak spots",
            "direct",
        );
        assert!(prompt.contains("Skeptic"));
        assert!(prompt.contains("Critical analyst"));
        assert!(prompt.contains("direct"));
    }

    #[test]
    fn test_format_multi_voice() {
        let responses = vec![
            (
                "Skeptic".to_string(),
                "I disagree with this approach.".to_string(),
            ),
            (
                "Connector".to_string(),
                "I see a link to game theory.".to_string(),
            ),
        ];
        let combined = format_multi_voice(&responses);
        assert!(combined.contains("## Skeptic"));
        assert!(combined.contains("## Connector"));
        assert!(combined.contains("I disagree"));
        assert!(combined.contains("game theory"));
    }

    #[test]
    fn test_build_synthesis_prompt() {
        let prompt = build_squad_synthesis_prompt(
            "Should I invest?",
            &[("Analyst".to_string(), "The numbers look good.".to_string())],
        );
        assert!(prompt.contains("Should I invest?"));
        assert!(prompt.contains("Analyst"));
    }
}
