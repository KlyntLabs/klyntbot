//! Adapter bridging `VectorStore` + `CommunityRepo` → community search traits.
//!
//! Dependency inversion: `NoteTreeNavigator` (L3 context_engine) defines
//! `CommunityEmbeddingSearch` and `CommunityMemberLoader`; this adapter (L5 agent)
//! implements them using concrete storage + cognitive types.

use std::sync::Arc;

use async_trait::async_trait;

use cognitive::repos::CommunityRepo;
use context_engine::insight_forge::note_tree_navigator::{
    CommunityEmbeddingSearch, CommunityHit, CommunityInfo, CommunityMember, CommunityMemberLoader,
};

pub struct CommunitySearchAdapter {
    vector_store: Arc<storage::VectorStore>,
    community_repo: CommunityRepo,
}

impl CommunitySearchAdapter {
    pub fn new(vector_store: Arc<storage::VectorStore>, community_repo: CommunityRepo) -> Self {
        Self {
            vector_store,
            community_repo,
        }
    }
}

#[async_trait]
impl CommunityEmbeddingSearch for CommunitySearchAdapter {
    async fn search_communities(
        &self,
        query_embedding: &[f32],
        limit: usize,
        min_similarity: f64,
    ) -> common::Result<Vec<CommunityHit>> {
        let results = self
            .vector_store
            .search_community_embeddings(query_embedding, limit, min_similarity)
            .await?;

        Ok(results
            .into_iter()
            .map(|r| CommunityHit {
                community_id: r.community_id,
                member_count: r.member_count.parse().unwrap_or(0),
                source_note_count: r.source_note_count.parse().unwrap_or(0),
                score: r.score,
            })
            .collect())
    }
}

#[async_trait]
impl CommunityMemberLoader for CommunitySearchAdapter {
    async fn get_community_members(
        &self,
        community_id: &str,
    ) -> common::Result<Vec<CommunityMember>> {
        let rows = self.community_repo.get_members(community_id).await?;
        Ok(rows
            .into_iter()
            .map(|r| CommunityMember {
                tree_node_id: r.tree_node_id,
                membership_score: r.membership_score,
            })
            .collect())
    }

    async fn get_community_info(
        &self,
        community_id: &str,
    ) -> common::Result<Option<CommunityInfo>> {
        let row = self.community_repo.get_community(community_id).await?;
        Ok(row.map(|r| CommunityInfo {
            id: r.id,
            name: r.name,
            summary: r.summary,
            top_entities: r
                .top_entities
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default(),
            representative_paths: r
                .representative_paths
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default(),
            source_note_count: r.source_note_count.unwrap_or(0) as usize,
            stability: r.stability,
        }))
    }
}
