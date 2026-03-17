use async_trait::async_trait;
use context_engine::insight_forge::DomainSearcher;
use context_engine::{MemoryEntry, MemorySource};
use storage::Repos;

pub struct TaskSearcher {
    repos: Repos,
}

impl TaskSearcher {
    pub fn new(repos: Repos) -> Self {
        Self { repos }
    }
}

#[async_trait]
impl DomainSearcher for TaskSearcher {
    fn domain_name(&self) -> &str {
        "tasks"
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<MemoryEntry> {
        let rows = match self
            .repos
            .tasks
            .search_by_keyword(query, Some(limit as i64))
            .await
        {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };

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
