use common::Result;
use sqlx::SqlitePool;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CommunityRow {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub member_count: i64,
    pub modularity_score: Option<f64>,
    pub stability: f64,
    pub top_entities: Option<String>,
    pub representative_paths: Option<String>,
    pub source_note_count: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CommunityMemberRow {
    pub community_id: String,
    pub tree_node_id: String,
    pub membership_score: f64,
    pub joined_at: String,
}

fn map_sqlx(e: sqlx::Error) -> common::KlyntbotError {
    common::KlyntbotError::Storage(e.to_string())
}

pub struct CommunityRepo {
    pool: SqlitePool,
}

impl CommunityRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn upsert_community(&self, community: &CommunityRow) -> Result<()> {
        sqlx::query(
            "INSERT INTO communities (id, name, summary, member_count, modularity_score, stability, top_entities, representative_paths, source_note_count, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(id) DO UPDATE SET
               name = ?2, summary = ?3, member_count = ?4, modularity_score = ?5,
               stability = ?6, top_entities = ?7, representative_paths = ?8,
               source_note_count = ?9, updated_at = ?11",
        )
        .bind(&community.id)
        .bind(&community.name)
        .bind(&community.summary)
        .bind(community.member_count)
        .bind(community.modularity_score)
        .bind(community.stability)
        .bind(&community.top_entities)
        .bind(&community.representative_paths)
        .bind(community.source_note_count)
        .bind(&community.created_at)
        .bind(&community.updated_at)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    pub async fn set_members(&self, community_id: &str, members: &[(String, f64)]) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        sqlx::query("DELETE FROM community_members WHERE community_id = ?1")
            .bind(community_id)
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx)?;
        for (node_id, score) in members {
            sqlx::query(
                "INSERT INTO community_members (community_id, tree_node_id, membership_score)
                 VALUES (?1, ?2, ?3)",
            )
            .bind(community_id)
            .bind(node_id)
            .bind(score)
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx)?;
        }
        tx.commit().await.map_err(map_sqlx)?;
        Ok(())
    }

    pub async fn get_community(&self, id: &str) -> Result<Option<CommunityRow>> {
        sqlx::query_as::<_, CommunityRow>("SELECT * FROM communities WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_sqlx)
    }

    pub async fn list_active_communities(&self) -> Result<Vec<CommunityRow>> {
        sqlx::query_as::<_, CommunityRow>(
            "SELECT * FROM communities WHERE stability >= 0.3 ORDER BY member_count DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx)
    }

    pub async fn get_members(&self, community_id: &str) -> Result<Vec<CommunityMemberRow>> {
        sqlx::query_as::<_, CommunityMemberRow>(
            "SELECT * FROM community_members WHERE community_id = ?1 ORDER BY membership_score DESC",
        )
        .bind(community_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx)
    }

    pub async fn get_communities_for_node(&self, tree_node_id: &str) -> Result<Vec<CommunityRow>> {
        sqlx::query_as::<_, CommunityRow>(
            "SELECT c.* FROM communities c
             JOIN community_members cm ON cm.community_id = c.id
             WHERE cm.tree_node_id = ?1 AND c.stability >= 0.3
             ORDER BY cm.membership_score DESC",
        )
        .bind(tree_node_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx)
    }

    pub async fn prune_weak_communities(&self, min_stability: f64) -> Result<Vec<String>> {
        sqlx::query_scalar::<_, String>("DELETE FROM communities WHERE stability < ?1 RETURNING id")
            .bind(min_stability)
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx)
    }

    /// Load all entity-shared edges between tree nodes for graph construction.
    /// Returns (node_a, node_b, weight) where weight = count(shared_entities) * avg(strength).
    pub async fn load_shared_entity_edges(&self) -> Result<Vec<(String, String, f64)>> {
        sqlx::query_as::<_, (String, String, f64)>(
            "SELECT a.tree_node_id AS node_a, b.tree_node_id AS node_b,
                    COUNT(DISTINCT a.entity_id) * COALESCE(AVG(er.strength), 0.5) AS weight
             FROM entity_tree_links a
             JOIN entity_tree_links b ON a.entity_id = b.entity_id AND a.tree_node_id < b.tree_node_id
             LEFT JOIN entity_relationships er
               ON (er.source_entity_id = a.entity_id OR er.target_entity_id = a.entity_id)
             GROUP BY a.tree_node_id, b.tree_node_id
             HAVING COUNT(DISTINCT a.entity_id) >= 1",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx)
    }

    pub async fn delete_community(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM communities WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_sqlx)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::cognitive_test_pool;

    #[tokio::test]
    async fn test_upsert_and_get_community() {
        let pool = cognitive_test_pool().await;
        let repo = CommunityRepo::new(pool);

        let community = CommunityRow {
            id: "c1".into(),
            name: "Test Community".into(),
            summary: "A test community".into(),
            member_count: 5,
            modularity_score: Some(0.45),
            stability: 0.8,
            top_entities: Some(r#"["entity1","entity2"]"#.into()),
            representative_paths: Some(r#"["Note > Section"]"#.into()),
            source_note_count: Some(3),
            created_at: "2026-03-28T00:00:00".into(),
            updated_at: "2026-03-28T00:00:00".into(),
        };

        repo.upsert_community(&community).await.unwrap();

        let fetched = repo.get_community("c1").await.unwrap().unwrap();
        assert_eq!(fetched.name, "Test Community");
        assert_eq!(fetched.member_count, 5);
        assert!((fetched.stability - 0.8).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_set_and_get_members() {
        let pool = cognitive_test_pool().await;
        let repo = CommunityRepo::new(pool.clone());

        // Insert tree nodes first
        sqlx::query("INSERT INTO book_tree_nodes (id, parent_id, node_type, content, title, level, source_type, source_id, position) VALUES ('n1', NULL, 'heading', 'c', 'h', 0, 'note', 'note1', 0), ('n2', NULL, 'heading', 'c', 'h', 0, 'note', 'note1', 1)")
            .execute(&pool)
            .await
            .unwrap();

        let community = CommunityRow {
            id: "c1".into(),
            name: "Test".into(),
            summary: "Summary".into(),
            member_count: 2,
            modularity_score: None,
            stability: 1.0,
            top_entities: None,
            representative_paths: None,
            source_note_count: None,
            created_at: "2026-03-28T00:00:00".into(),
            updated_at: "2026-03-28T00:00:00".into(),
        };
        repo.upsert_community(&community).await.unwrap();

        let members = vec![("n1".to_string(), 0.9), ("n2".to_string(), 0.7)];
        repo.set_members("c1", &members).await.unwrap();

        let fetched = repo.get_members("c1").await.unwrap();
        assert_eq!(fetched.len(), 2);
        assert_eq!(fetched[0].tree_node_id, "n1");
    }

    #[tokio::test]
    async fn test_list_active_communities() {
        let pool = cognitive_test_pool().await;
        let repo = CommunityRepo::new(pool);

        for (id, stability) in [("c1", 0.8), ("c2", 0.1), ("c3", 0.5)] {
            let c = CommunityRow {
                id: id.into(),
                name: format!("Community {id}"),
                summary: "s".into(),
                member_count: 3,
                modularity_score: None,
                stability,
                top_entities: None,
                representative_paths: None,
                source_note_count: None,
                created_at: "2026-03-28T00:00:00".into(),
                updated_at: "2026-03-28T00:00:00".into(),
            };
            repo.upsert_community(&c).await.unwrap();
        }

        let active = repo.list_active_communities().await.unwrap();
        assert_eq!(active.len(), 2);
    }

    #[tokio::test]
    async fn test_prune_weak_communities() {
        let pool = cognitive_test_pool().await;
        let repo = CommunityRepo::new(pool);

        for (id, stability) in [("c1", 0.8), ("c2", 0.1), ("c3", 0.2)] {
            let c = CommunityRow {
                id: id.into(),
                name: "n".into(),
                summary: "s".into(),
                member_count: 1,
                modularity_score: None,
                stability,
                top_entities: None,
                representative_paths: None,
                source_note_count: None,
                created_at: "2026-03-28T00:00:00".into(),
                updated_at: "2026-03-28T00:00:00".into(),
            };
            repo.upsert_community(&c).await.unwrap();
        }

        let pruned = repo.prune_weak_communities(0.3).await.unwrap();
        assert_eq!(pruned.len(), 2);
        assert!(pruned.contains(&"c2".to_string()));
        assert!(pruned.contains(&"c3".to_string()));

        assert!(repo.get_community("c1").await.unwrap().is_some());
        assert!(repo.get_community("c2").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_get_communities_for_node() {
        let pool = cognitive_test_pool().await;
        let repo = CommunityRepo::new(pool.clone());

        sqlx::query("INSERT INTO book_tree_nodes (id, parent_id, node_type, content, title, level, source_type, source_id, position) VALUES ('n1', NULL, 'heading', 'c', 'h', 0, 'note', 'note1', 0)")
            .execute(&pool)
            .await
            .unwrap();

        for (id, stability) in [("c1", 0.8), ("c2", 0.1)] {
            let c = CommunityRow {
                id: id.into(),
                name: "n".into(),
                summary: "s".into(),
                member_count: 1,
                modularity_score: None,
                stability,
                top_entities: None,
                representative_paths: None,
                source_note_count: None,
                created_at: "2026-03-28T00:00:00".into(),
                updated_at: "2026-03-28T00:00:00".into(),
            };
            repo.upsert_community(&c).await.unwrap();
            repo.set_members(id, &[("n1".to_string(), 0.5)])
                .await
                .unwrap();
        }

        let communities = repo.get_communities_for_node("n1").await.unwrap();
        assert_eq!(communities.len(), 1);
        assert_eq!(communities[0].id, "c1");
    }
}
