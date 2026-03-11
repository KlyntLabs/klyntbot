//! Project context source — injects project instructions, role, personality, and memories.

use async_trait::async_trait;
use cognitive::SemanticFactRepo;
use context_engine::source::{ContextSource, SourceContext};
use storage::repos::Repos;

/// Provides project-scoped context when `SourceContext::project_id` is set.
pub struct ProjectContextSource {
    repos: Repos,
    semantic_repo: SemanticFactRepo,
}

impl ProjectContextSource {
    pub fn new(repos: Repos, semantic_repo: SemanticFactRepo) -> Self {
        Self {
            repos,
            semantic_repo,
        }
    }
}

#[async_trait]
impl ContextSource for ProjectContextSource {
    fn name(&self) -> &str {
        "project"
    }

    fn priority(&self) -> u8 {
        80
    }

    async fn provide(&self, ctx: &SourceContext) -> Option<String> {
        let project_id = ctx.project_id.as_deref()?;
        let project = self.repos.projects.get(project_id).await.ok()??;

        let mut sections = Vec::new();

        // 1. Instructions
        if let Some(instructions_json) = &project.instructions {
            if let Ok(instructions) = serde_json::from_str::<serde_json::Value>(instructions_json) {
                if let Some(context) = instructions.get("context").and_then(|v| v.as_str()) {
                    if !context.is_empty() {
                        sections.push(format!("## Project Context\n{context}"));
                    }
                }
                if let Some(guidelines) = instructions.get("guidelines").and_then(|v| v.as_str()) {
                    if !guidelines.is_empty() {
                        sections.push(format!("## Guidelines\n{guidelines}"));
                    }
                }
                if let Some(constraints) = instructions.get("constraints").and_then(|v| v.as_str())
                {
                    if !constraints.is_empty() {
                        sections.push(format!("## Constraints\n{constraints}"));
                    }
                }
            }
        }

        // 2. User role
        if let Some(role) = &project.user_role {
            if !role.is_empty() {
                sections.push(format!(
                    "## User's Role\n{role}\n\n\
                     ## Role-Aware Guidance\n\
                     Based on the user's role as \"{role}\", adjust your suggestions and coaching \
                     to focus on their responsibilities. Communicate in a way that's relevant \
                     to their position. Prioritize information and tasks that align with this role."
                ));
            }
        }

        // 3. AI personality
        if let Some(personality) = &project.ai_personality {
            if !personality.is_empty() {
                sections.push(format!("## Communication Style\n{personality}"));
            }
        }

        // 4. Project-scoped memories (decisions, milestones, insights, patterns)
        if let Ok(facts) = self.semantic_repo.list_by_project(project_id).await {
            if !facts.is_empty() {
                let memory_text: Vec<String> = facts
                    .iter()
                    .take(10)
                    .map(|f| {
                        format!(
                            "- [{}] {} {} {}",
                            f.memory_type, f.subject, f.predicate, f.object
                        )
                    })
                    .collect();
                sections.push(format!("## Project Memories\n{}", memory_text.join("\n")));
            }
        }

        if sections.is_empty() {
            None
        } else {
            Some(format!(
                "# Project: {}\n\n{}",
                project.name,
                sections.join("\n\n")
            ))
        }
    }

    fn estimated_tokens(&self) -> usize {
        1500
    }
}
