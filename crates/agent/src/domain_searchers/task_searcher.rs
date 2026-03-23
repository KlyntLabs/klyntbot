use async_trait::async_trait;
use context_engine::insight_forge::DomainSearcher;
use context_engine::{MemoryEntry, MemorySource};
use storage::Repos;
use tracing::debug;

pub struct TaskSearcher {
    repos: Repos,
}

impl TaskSearcher {
    pub fn new(repos: Repos) -> Self {
        Self { repos }
    }

    /// Extract the single best keyword from the sub-query for LIKE search.
    /// TaskRepo::search_by_keyword uses LIKE %term% — a full phrase won't
    /// match partial title text. We pick the most distinctive single term.
    fn extract_search_term(query: &str) -> String {
        let stop = [
            "background", "context", "current", "status", "related", "people",
            "teams", "risks", "blockers", "overview", "details", "skill",
            "the", "and", "for", "with", "about", "what", "how", "show",
            "tell", "give", "remind", "any", "updates", "left", "going",
            "doing", "work", "tasks", "todo", "done",
        ];
        let terms: Vec<&str> = query
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.len() > 2 && !stop.contains(&w.to_lowercase().as_str()))
            .collect();
        // Use the first meaningful term — respects user's intent priority.
        // "what auth tasks are blocked" → "auth" (not "blocked")
        terms.into_iter()
            .next()
            .unwrap_or("")
            .to_string()
    }
}

#[async_trait]
impl DomainSearcher for TaskSearcher {
    fn domain_name(&self) -> &str {
        "tasks"
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<MemoryEntry> {
        let search_term = Self::extract_search_term(query);
        if search_term.is_empty() {
            return Vec::new();
        }

        debug!(
            original_query = query,
            search_term = search_term.as_str(),
            "📋 TaskSearcher: searching"
        );

        let rows = match self
            .repos
            .tasks
            .search_by_keyword(&search_term, Some(limit as i64))
            .await
        {
            Ok(r) => r,
            Err(e) => {
                debug!(error = %e, "📋 TaskSearcher: search error");
                return Vec::new();
            }
        };

        debug!(
            result_count = rows.len(),
            titles = ?rows.iter().take(3).map(|t| t.title.as_str()).collect::<Vec<_>>(),
            "📋 TaskSearcher: found"
        );

        rows.into_iter()
            .enumerate()
            .map(|(i, task)| MemoryEntry {
                id: task.id.clone(),
                content: format!(
                    "[Task: {} ({})] {}",
                    task.title,
                    task.status,
                    task.description.as_deref().unwrap_or("")
                ),
                score: 1.0 / (1.0 + i as f64),
                source: MemorySource::Domain {
                    name: "tasks".into(),
                },
                raw_score: 0.0,
            })
            .collect()
    }
}
