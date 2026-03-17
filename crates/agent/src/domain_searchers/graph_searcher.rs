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

        let mut entries = Vec::new();

        for (i, entity) in entities.iter().take(limit / 2).enumerate() {
            let desc = entity.description.as_deref().unwrap_or("no description");
            entries.push(MemoryEntry {
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
            });

            // Expand 1-hop neighborhood for the top match only
            if i == 0 {
                if let Ok(Some(hood)) = self.entity_repo.get_neighborhood(&entity.id, 1).await {
                    for (j, neighbor) in hood.neighbors.iter().enumerate() {
                        if entries.len() >= limit {
                            break;
                        }
                        let rel_type = hood
                            .relationships
                            .iter()
                            .find(|r| {
                                r.target_entity_id == neighbor.id
                                    || r.source_entity_id == neighbor.id
                            })
                            .map(|r| r.relationship_type.as_str())
                            .unwrap_or("related");

                        let n_desc = neighbor.description.as_deref().unwrap_or("");
                        entries.push(MemoryEntry {
                            id: neighbor.id.clone(),
                            content: format!(
                                "[Connected: {} ({}) — {} → {}] {}",
                                neighbor.name, neighbor.entity_type, rel_type, entity.name, n_desc
                            ),
                            score: 0.5 / (1.0 + j as f64),
                            source: MemorySource::Domain {
                                name: "graph".into(),
                            },
                            raw_score: neighbor.mention_count as f64,
                        });
                    }
                }
            }
        }

        entries.truncate(limit);
        entries
    }
}
