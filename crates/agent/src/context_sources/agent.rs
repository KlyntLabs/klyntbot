//! Agent context source — injects active agent's instructions and skills into the system prompt.

use std::sync::Arc;

use async_trait::async_trait;
use context_engine::source::{ContextSource, SourceContext};
use tokio::sync::RwLock;

use crate::agent_profile::AgentProfile;

/// Injects the active agent's instructions and always-loaded skills into the system prompt.
/// Replaces SkillSummarySource + SkillContentSource.
pub struct AgentContextSource {
    active_profile: Arc<RwLock<Option<Arc<AgentProfile>>>>,
}

impl AgentContextSource {
    pub fn new(active_profile: Arc<RwLock<Option<Arc<AgentProfile>>>>) -> Self {
        Self { active_profile }
    }
}

#[async_trait]
impl ContextSource for AgentContextSource {
    fn name(&self) -> &str {
        "agent_profile"
    }

    fn priority(&self) -> u8 {
        35
    }

    async fn provide(&self, ctx: &SourceContext) -> Option<String> {
        let guard = self.active_profile.read().await;
        let profile = guard.as_ref()?;

        let mut sections = Vec::new();

        // Agent instructions
        if !profile.instructions.is_empty() {
            sections.push(format!(
                "# Agent: {}\n\n{}",
                profile.name, profile.instructions
            ));
        }

        // Always-loaded skill content
        for skill_content in profile.always_loaded_skill_content() {
            sections.push(skill_content);
        }

        // On-demand skills activated by message content
        if let Some(ref message) = ctx.message {
            for skill in profile.message_activated_skills(message) {
                sections.push(format!(
                    "# Skill: {} (activated)\n\n{}",
                    skill.name, skill.content
                ));
            }
        }

        if sections.is_empty() {
            None
        } else {
            Some(sections.join("\n\n---\n\n"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_profile::AgentSkill;

    #[tokio::test]
    async fn test_agent_context_provides_instructions_and_skills() {
        let profile = AgentProfile {
            name: "task".into(),
            instructions: "You are the task agent.".into(),
            always_skills: vec!["todo".into()],
            skills: vec![AgentSkill {
                name: "todo".into(),
                description: "Task workflow".into(),
                always: true,
                content: "Create tasks using the task tool.".into(),
                ..Default::default()
            }],
            ..Default::default()
        };

        let source = AgentContextSource::new(Arc::new(RwLock::new(Some(Arc::new(profile)))));
        let ctx = SourceContext {
            channel: "test".into(),
            chat_id: "1".into(),
            message: None,
            intent_summary: None,
        };
        let result = source.provide(&ctx).await;

        assert!(result.is_some());
        let text = result.unwrap();
        assert!(
            text.contains("task agent"),
            "Should contain agent instructions"
        );
        assert!(
            text.contains("Create tasks"),
            "Should contain always-loaded skill content"
        );
    }

    #[tokio::test]
    async fn test_agent_context_returns_none_when_no_profile() {
        let source = AgentContextSource::new(Arc::new(RwLock::new(None)));
        let ctx = SourceContext {
            channel: "test".into(),
            chat_id: "1".into(),
            message: None,
            intent_summary: None,
        };
        assert!(source.provide(&ctx).await.is_none());
    }

    #[tokio::test]
    async fn test_agent_context_returns_none_when_empty_instructions() {
        let profile = AgentProfile::default();
        let source = AgentContextSource::new(Arc::new(RwLock::new(Some(Arc::new(profile)))));
        let ctx = SourceContext {
            channel: "test".into(),
            chat_id: "1".into(),
            message: None,
            intent_summary: None,
        };
        assert!(source.provide(&ctx).await.is_none());
    }
}
