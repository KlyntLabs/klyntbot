use desktop_shared::commands::{
    KnowledgeHealthSummary, TopicDetailParams, TopicDetailResponse, TopicHealthResponse,
};
use desktop_shared::errors::ApiError;

use super::atoms::{atom_row_to_response, map_db};
use crate::state::AppCore;

impl AppCore {
    pub async fn knowledge_health_summary(&self) -> Result<KnowledgeHealthSummary, ApiError> {
        let repo = self.knowledge_atom_repo()?;
        let topics = repo.list_topics_with_atoms().await.map_err(map_db)?;

        let total: usize = topics.iter().map(|t| t.atom_count as usize).sum();
        let avg_ret = if topics.is_empty() {
            1.0
        } else {
            topics
                .iter()
                .map(|t| t.avg_retention * t.atom_count as f64)
                .sum::<f64>()
                / total.max(1) as f64
        };

        Ok(KnowledgeHealthSummary {
            total_atoms: total,
            active_atoms: total, // list_topics_with_atoms only counts active atoms
            avg_retention: avg_ret,
            topics: topics
                .into_iter()
                .map(|t| TopicHealthResponse {
                    id: t.id,
                    name: t.name,
                    domain: t.domain,
                    atom_count: t.atom_count,
                    avg_retention: t.avg_retention,
                })
                .collect(),
        })
    }

    pub async fn knowledge_topic_detail(
        &self,
        params: TopicDetailParams,
    ) -> Result<TopicDetailResponse, ApiError> {
        let repo = self.knowledge_atom_repo()?;

        let topic = repo
            .get_topic(&params.topic_id)
            .await
            .map_err(map_db)?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "Topic not found"))?;

        let atoms = repo
            .list_for_topic(&params.topic_id)
            .await
            .map_err(map_db)?;

        Ok(TopicDetailResponse {
            topic: TopicHealthResponse {
                id: topic.id,
                name: topic.name,
                domain: topic.domain,
                atom_count: topic.atom_count,
                avg_retention: topic.avg_retention,
            },
            atoms: atoms.into_iter().map(atom_row_to_response).collect(),
        })
    }
}
