//! Page context source — injects entity details for scoped sessions.

use async_trait::async_trait;
use common::SessionKey;
use context_engine::source::{ContextSource, SourceContext};
use storage::repos::Repos;

/// Injects entity details (project, task, objective, etc.) when a session
/// is scoped to a specific entity via `session_context`.
pub struct PageContextSource {
    repos: Repos,
}

impl PageContextSource {
    pub fn new(repos: Repos) -> Self {
        Self { repos }
    }
}

#[async_trait]
impl ContextSource for PageContextSource {
    fn name(&self) -> &str {
        "page_context"
    }

    fn priority(&self) -> u8 {
        90
    }

    async fn provide(&self, ctx: &SourceContext) -> Option<String> {
        let session_key = SessionKey::from_parts(&ctx.channel, &ctx.chat_id);
        let context = self
            .repos
            .session_context
            .get(session_key.as_str())
            .await
            .ok()??;

        let entity_kind = context.entity_kind.as_deref()?;
        let entity_id = context.entity_id.as_deref();

        let details = match entity_kind {
            "project" => self.project_context(entity_id?).await,
            "task" => self.task_context(entity_id?).await,
            "objective" => self.objective_context(entity_id?).await,
            "area" => self.area_context(entity_id?).await,
            _ => None,
        };

        details.map(|d| format!("# Page Context\n\n{d}"))
    }

    fn estimated_tokens(&self) -> usize {
        300
    }
}

impl PageContextSource {
    async fn project_context(&self, id: &str) -> Option<String> {
        let filter = storage::TaskFilter {
            project_id: Some(id.to_string()),
            ..Default::default()
        };
        let (project_res, tasks_res, objectives_res) = tokio::join!(
            self.repos.projects.get(id),
            self.repos.tasks.list(&filter),
            self.repos.objectives.list(Some(id), None),
        );
        let project = project_res.ok()??;
        let tasks = tasks_res.unwrap_or_default();
        let objectives = objectives_res.unwrap_or_default();

        let mut out = format!(
            "**Project:** {} (status: {})\n",
            project.name, project.status
        );
        if let Some(desc) = &project.description {
            out.push_str(&format!("**Description:** {desc}\n"));
        }

        if !tasks.is_empty() {
            out.push_str(&format!("\n**Tasks ({}):**\n", tasks.len()));
            for t in tasks.iter().take(20) {
                out.push_str(&format!("- [{}] {} ({})\n", t.id, t.title, t.status));
            }
            if tasks.len() > 20 {
                out.push_str(&format!("  ... and {} more\n", tasks.len() - 20));
            }
        }

        if !objectives.is_empty() {
            out.push_str(&format!("\n**Objectives ({}):**\n", objectives.len()));
            for o in &objectives {
                out.push_str(&format!("- [{}] {} ({})\n", o.id, o.title, o.status));
            }
        }

        Some(out)
    }

    async fn task_context(&self, id: &str) -> Option<String> {
        let task = self.repos.tasks.get(id).await.ok()??;
        let subtasks = self.repos.tasks.get_children(id).await.unwrap_or_default();

        let mut out = format!(
            "**Task:** {} (status: {}, priority: {})\n",
            task.title,
            task.status,
            task.priority.map_or("none".to_string(), |p| p.to_string())
        );
        if let Some(desc) = &task.description {
            out.push_str(&format!("**Description:** {desc}\n"));
        }
        if let Some(due) = &task.due_date {
            out.push_str(&format!("**Due:** {due}\n"));
        }

        if !subtasks.is_empty() {
            out.push_str(&format!("\n**Subtasks ({}):**\n", subtasks.len()));
            for st in &subtasks {
                out.push_str(&format!("- [{}] {} ({})\n", st.id, st.title, st.status));
            }
        }

        Some(out)
    }

    async fn objective_context(&self, id: &str) -> Option<String> {
        let objective = self.repos.objectives.get(id).await.ok()??;
        let krs = self
            .repos
            .key_results
            .list(Some(id))
            .await
            .unwrap_or_default();

        let mut out = format!(
            "**Objective:** {} (status: {})\n",
            objective.title, objective.status
        );

        if !krs.is_empty() {
            out.push_str(&format!("\n**Key Results ({}):**\n", krs.len()));
            for kr in &krs {
                let progress = match kr.target_value {
                    Some(target) if target > 0.0 => {
                        format!("{:.0}%", (kr.current_value / target) * 100.0)
                    }
                    _ => "N/A".to_string(),
                };
                out.push_str(&format!(
                    "- [{}] {} ({:.0}/{} = {})\n",
                    kr.id,
                    kr.title,
                    kr.current_value,
                    kr.target_value
                        .map_or("?".to_string(), |v| format!("{v:.0}")),
                    progress
                ));
            }
        }

        Some(out)
    }

    async fn area_context(&self, id: &str) -> Option<String> {
        let area = self.repos.areas.get(id).await.ok()??;
        let projects = self
            .repos
            .projects
            .list(&storage::repos::ProjectFilter {
                area_id: Some(id.to_string()),
                ..Default::default()
            })
            .await
            .unwrap_or_default();

        let mut out = format!("**Area:** {} (status: {})\n", area.name, area.status);

        if !projects.is_empty() {
            out.push_str(&format!("\n**Projects ({}):**\n", projects.len()));
            for p in &projects {
                out.push_str(&format!("- [{}] {} ({})\n", p.id, p.name, p.status));
            }
        }

        Some(out)
    }

}
