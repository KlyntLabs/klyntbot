//! KCA Track 11 — online community membership.

use crate::repos::entity::EntityRepo;
use crate::repos::community::CommunityRepo;

#[derive(Debug, Clone)]
pub struct CommunityMatch {
    pub community_id: String,
    pub community_name: String,
    pub score: f64,
}

pub async fn find_best_community_for_entity(
    entity_repo: &EntityRepo,
    community_repo: &CommunityRepo,
    entity_id: &str,
) -> common::Result<Option<CommunityMatch>> {
    use crate::repos::co_activation::CoActivationRepo;
    let coact = CoActivationRepo::new(entity_repo.pool().clone());

    // Get top-20 co-activated entities for this entity.
    let coact_pairs = coact.top_for_entity(entity_id, 20).await?;
    if coact_pairs.is_empty() {
        return Ok(None);
    }

    // For each co-activated entity, find which community it's in.
    let mut score_by_community: std::collections::HashMap<String, (String, f64)> = Default::default();
    for (other_id, strength) in coact_pairs {
        for community in community_repo.list_for_entity(&other_id).await.unwrap_or_default() {
            let entry = score_by_community.entry(community.id.clone())
                .or_insert((community.name.clone(), 0.0));
            entry.1 += strength;
        }
    }

    Ok(score_by_community
        .into_iter()
        .max_by(|a, b| a.1.1.partial_cmp(&b.1.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(id, (name, score))| CommunityMatch { community_id: id, community_name: name, score }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage::StoragePool;
    use crate::repos::entity::EntityRepo;
    use crate::repos::community::CommunityRepo;

    #[tokio::test]
    async fn finds_best_community_via_co_activation() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let entity_repo = EntityRepo::new(pool.inner().clone());
        let community_repo = CommunityRepo::new(pool.inner().clone());

        // Seed: 3 entities, A & B in community "alpha", C uncategorized.
        let a = entity_repo.upsert_entity(&crate::repos::NewEntity {
            name: "A".into(), entity_type: "concept".into(), description: None, source: "t".into(), source_id: None, metadata: None,
        }).await.unwrap();
        let b = entity_repo.upsert_entity(&crate::repos::NewEntity {
            name: "B".into(), entity_type: "concept".into(), description: None, source: "t".into(), source_id: None, metadata: None,
        }).await.unwrap();
        let c = entity_repo.upsert_entity(&crate::repos::NewEntity {
            name: "C".into(), entity_type: "concept".into(), description: None, source: "t".into(), source_id: None, metadata: None,
        }).await.unwrap();
        let alpha = community_repo.create("alpha", "test community", None).await.unwrap();
        community_repo.add_member(&alpha.id, &a.id).await.unwrap();
        community_repo.add_member(&alpha.id, &b.id).await.unwrap();

        // Co-activate C with A (strength 0.9), C with B (0.7) — strong signal C ∈ alpha.
        crate::repos::CoActivationRepo::new(pool.inner().clone())
            .increment_pair(&c.id, &a.id).await.unwrap();
        crate::repos::CoActivationRepo::new(pool.inner().clone())
            .increment_pair(&c.id, &b.id).await.unwrap();

        let result = find_best_community_for_entity(&entity_repo, &community_repo, &c.id).await.unwrap();
        assert_eq!(result.as_ref().map(|c| c.community_id.clone()), Some(alpha.id));
        assert!(result.as_ref().unwrap().score > 0.5);
    }
}
