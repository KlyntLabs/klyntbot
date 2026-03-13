//! Session context source — injects the active entity for the current chat session.
//!
//! Reads the `session_context` table entry for the current session key (`ctx.chat_id`)
//! and tells the LLM which task/project/area the user is actively working on. This lets
//! the LLM resolve implicit references like "the task" or "it" without asking for
//! clarification.

use async_trait::async_trait;
use context_engine::source::{ContextSource, SourceContext};
use storage::Repos;

/// Injects active-entity context for the current chat session into the system prompt.
///
/// Priority 85 — higher than project/area/todo sources so it appears near the top.
pub struct SessionContextSource {
    repos: Repos,
}

impl SessionContextSource {
    pub fn new(repos: Repos) -> Self {
        Self { repos }
    }
}

#[async_trait]
impl ContextSource for SessionContextSource {
    fn name(&self) -> &str {
        "session_context"
    }

    fn priority(&self) -> u8 {
        85
    }

    async fn provide(&self, ctx: &SourceContext) -> Option<String> {
        let session_key = &ctx.chat_id;

        let sc = self
            .repos
            .session_context
            .get(session_key)
            .await
            .ok()
            .flatten()?;

        let entity_kind = sc.entity_kind.as_deref()?;
        let entity_id = sc.entity_id.as_deref()?;

        let entity_line = match entity_kind {
            "task" => {
                // Try the newer tasks repo first, then fall back to legacy actions.
                if let Ok(Some(task)) = self.repos.tasks.get(entity_id).await {
                    let short_id = &task.id[..task.id.len().min(8)];
                    let mut line =
                        format!("- **Task**: \"{}\" (ID: {}…)", task.title, short_id);
                    line.push_str(&format!(" — Status: {}", task.status));
                    if let Some(ref pid) = task.project_id {
                        if let Ok(Some(proj)) = self.repos.projects.get(pid).await {
                            line.push_str(&format!(", Project: {}", proj.name));
                            if let Ok(Some(area)) =
                                self.repos.areas.get(&proj.area_id).await
                            {
                                line.push_str(&format!(", Area: {}", area.name));
                            }
                        }
                    } else if let Ok(Some(area)) =
                        self.repos.areas.get(&task.area_id).await
                    {
                        line.push_str(&format!(", Area: {}", area.name));
                    }
                    line
                } else if let Ok(Some(action)) = self.repos.actions.get(entity_id).await {
                    let short_id = &action.id[..action.id.len().min(8)];
                    let mut line =
                        format!("- **Task**: \"{}\" (ID: {}…)", action.title, short_id);
                    line.push_str(&format!(" — Status: {}", action.status));
                    if let Some(ref pid) = action.project_id {
                        if let Ok(Some(proj)) = self.repos.projects.get(pid).await {
                            line.push_str(&format!(", Project: {}", proj.name));
                            if let Ok(Some(area)) =
                                self.repos.areas.get(&proj.area_id).await
                            {
                                line.push_str(&format!(", Area: {}", area.name));
                            }
                        }
                    } else if let Ok(Some(area)) =
                        self.repos.areas.get(&action.area_id).await
                    {
                        line.push_str(&format!(", Area: {}", area.name));
                    }
                    line
                } else {
                    return None;
                }
            }
            "project" => {
                if let Ok(Some(proj)) = self.repos.projects.get(entity_id).await {
                    let short_id = &proj.id[..proj.id.len().min(8)];
                    let mut line =
                        format!("- **Project**: \"{}\" (ID: {}…)", proj.name, short_id);
                    if let Ok(Some(area)) = self.repos.areas.get(&proj.area_id).await {
                        line.push_str(&format!(", Area: {}", area.name));
                    }
                    line
                } else {
                    return None;
                }
            }
            "area" => {
                if let Ok(Some(area)) = self.repos.areas.get(entity_id).await {
                    let short_id = &area.id[..area.id.len().min(8)];
                    format!("- **Area**: \"{}\" (ID: {}…)", area.name, short_id)
                } else {
                    return None;
                }
            }
            _ => return None,
        };

        Some(format!(
            "## Active Conversation Context\n\
             The user is currently working on:\n\
             {entity_line}\n\n\
             When the user refers to \"the task\", \"the project\", \"it\", or a similar \
             implicit reference without specifying an ID, default to the entity above."
        ))
    }

    fn estimated_tokens(&self) -> usize {
        150
    }
}
