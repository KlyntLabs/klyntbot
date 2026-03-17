use async_trait::async_trait;
use cognitive::repos::EntityRepo;
use context_engine::insight_forge::DomainSearcher;
use context_engine::{MemoryEntry, MemorySource};

pub struct GraphSearcher {
    entity_repo: EntityRepo,
}

impl GraphSearcher {
    pub fn new(entity_repo: EntityRepo) -> Self {
        Self { entity_repo }
    }
}

#[async_trait]
impl DomainSearcher for GraphSearcher {
    fn domain_name(&self) -> &str {
        "graph"
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<MemoryEntry> {
        let entities = match self.entity_repo.find_by_name(query).await {
            Ok(e) => e,
            Err(_) => return Vec::new(),
        };

        entities
            .into_iter()
            .take(limit)
            .enumerate()
            .map(|(i, entity)| {
                let desc = entity.description.as_deref().unwrap_or("no description");
                MemoryEntry {
                    id: entity.id.clone(),
                    content: format!(
                        "[Entity: {} ({})] {} (seen {} times)",
                        entity.name, entity.entity_type, desc, entity.mention_count
                    ),
                    score: 1.0 / (1.0 + i as f64),
                    source: MemorySource::Domain {
                        name: "graph".into(),
                    },
                    raw_score: entity.mention_count as f64,
                }
            })
            .collect()
    }
}
