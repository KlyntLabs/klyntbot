use async_trait::async_trait;
use common::Result;
use context_engine::book_index::types::{SourceType, TreeNode, TreeNodeType};
use context_engine::book_index::GTLinkRepo;

pub struct SqliteGTLinkRepo {
    pool: sqlx::SqlitePool,
}

impl SqliteGTLinkRepo {
    pub fn new(pool: sqlx::SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl GTLinkRepo for SqliteGTLinkRepo {
    async fn link(&self, entity_id: &str, tree_node_id: &str) -> Result<()> {
        sqlx::query(
            "INSERT OR IGNORE INTO entity_tree_links (entity_id, tree_node_id) VALUES (?1, ?2)",
        )
        .bind(entity_id)
        .bind(tree_node_id)
        .execute(&self.pool)
        .await
        .map_err(cognitive_schema::map_sqlx)?;
        Ok(())
    }

    async fn link_batch(&self, links: &[(String, String)]) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(cognitive_schema::map_sqlx)?;
        for (entity_id, tree_node_id) in links {
            sqlx::query(
                "INSERT OR IGNORE INTO entity_tree_links (entity_id, tree_node_id) VALUES (?1, ?2)",
            )
            .bind(entity_id)
            .bind(tree_node_id)
            .execute(&mut *tx)
            .await
            .map_err(cognitive_schema::map_sqlx)?;
        }
        tx.commit().await.map_err(cognitive_schema::map_sqlx)?;
        Ok(())
    }

    async fn get_linked_nodes(&self, entity_id: &str) -> Result<Vec<TreeNode>> {
        let rows: Vec<(String, Option<String>, String, String, Option<String>, i32, String, String, i32, Option<String>)> =
            sqlx::query_as(
                "SELECT n.id, n.parent_id, n.node_type, n.content, n.title, n.level, n.source_type, n.source_id, n.position, n.metadata
                 FROM entity_tree_links l
                 JOIN book_tree_nodes n ON n.id = l.tree_node_id
                 WHERE l.entity_id = ?1",
            )
            .bind(entity_id)
            .fetch_all(&self.pool)
            .await
            .map_err(cognitive_schema::map_sqlx)?;

        Ok(rows
            .into_iter()
            .map(
                |(
                    id,
                    parent_id,
                    node_type,
                    content,
                    title,
                    level,
                    source_type,
                    source_id,
                    position,
                    metadata,
                )| {
                    TreeNode {
                        id,
                        parent_id,
                        node_type: TreeNodeType::parse(&node_type),
                        content,
                        title,
                        level: level as u32,
                        source_type: SourceType::parse(&source_type),
                        source_id,
                        position: position as u32,
                        metadata,
                    }
                },
            )
            .collect())
    }

    async fn get_entities_in_subtree(&self, node_id: &str) -> Result<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT l.entity_id
             FROM entity_tree_links l
             JOIN (
                WITH RECURSIVE subtree AS (
                    SELECT id FROM book_tree_nodes WHERE id = ?1
                    UNION ALL
                    SELECT n.id FROM book_tree_nodes n JOIN subtree s ON n.parent_id = s.id
                ) SELECT id FROM subtree
             ) s ON l.tree_node_id = s.id",
        )
        .bind(node_id)
        .fetch_all(&self.pool)
        .await
        .map_err(cognitive_schema::map_sqlx)?;

        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    async fn delete_by_tree_node(&self, tree_node_id: &str) -> Result<u64> {
        let result = sqlx::query("DELETE FROM entity_tree_links WHERE tree_node_id = ?1")
            .bind(tree_node_id)
            .execute(&self.pool)
            .await
            .map_err(cognitive_schema::map_sqlx)?;
        Ok(result.rows_affected())
    }

    async fn migrate_entity_links(
        &self,
        source_entity_id: &str,
        target_entity_id: &str,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(cognitive_schema::map_sqlx)?;

        sqlx::query(
            "INSERT OR IGNORE INTO entity_tree_links (entity_id, tree_node_id, created_at)
             SELECT ?1, tree_node_id, created_at FROM entity_tree_links WHERE entity_id = ?2",
        )
        .bind(target_entity_id)
        .bind(source_entity_id)
        .execute(&mut *tx)
        .await
        .map_err(cognitive_schema::map_sqlx)?;

        sqlx::query("DELETE FROM entity_tree_links WHERE entity_id = ?1")
            .bind(source_entity_id)
            .execute(&mut *tx)
            .await
            .map_err(cognitive_schema::map_sqlx)?;

        tx.commit().await.map_err(cognitive_schema::map_sqlx)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cognitive_schema::cognitive_test_pool;

    #[tokio::test]
    async fn link_and_query() {
        let pool = cognitive_test_pool().await;

        // Insert an entity
        sqlx::query(
            "INSERT INTO entities (id, name, entity_type, mention_count, first_seen_at, last_seen_at, created_at, updated_at)
             VALUES ('e1', 'TestEntity', 'Concept', 1, datetime('now'), datetime('now'), datetime('now'), datetime('now'))",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Insert a tree node
        sqlx::query(
            "INSERT INTO book_tree_nodes (id, node_type, content, level, source_type, source_id, position)
             VALUES ('n1', 'Text', 'content', 0, 'Note', 'note-1', 0)",
        )
        .execute(&pool)
        .await
        .unwrap();

        let repo = SqliteGTLinkRepo::new(pool);
        repo.link("e1", "n1").await.unwrap();

        let nodes = repo.get_linked_nodes("e1").await.unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].id, "n1");

        let entities = repo.get_entities_in_subtree("n1").await.unwrap();
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0], "e1");
    }

    #[tokio::test]
    async fn migrate_entity_links() {
        let pool = cognitive_test_pool().await;

        // Insert two entities
        sqlx::query(
            "INSERT INTO entities (id, name, entity_type, mention_count, first_seen_at, last_seen_at, created_at, updated_at) VALUES
             ('eA', 'EntityA', 'Concept', 1, datetime('now'), datetime('now'), datetime('now'), datetime('now')),
             ('eB', 'EntityB', 'Concept', 1, datetime('now'), datetime('now'), datetime('now'), datetime('now'))",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Insert a tree node
        sqlx::query(
            "INSERT INTO book_tree_nodes (id, node_type, content, level, source_type, source_id, position)
             VALUES ('n1', 'Text', 'content', 0, 'Note', 'note-1', 0)",
        )
        .execute(&pool)
        .await
        .unwrap();

        let repo = SqliteGTLinkRepo::new(pool);
        repo.link("eA", "n1").await.unwrap();

        repo.migrate_entity_links("eA", "eB").await.unwrap();

        let nodes_b = repo.get_linked_nodes("eB").await.unwrap();
        assert_eq!(nodes_b.len(), 1);
        assert_eq!(nodes_b[0].id, "n1");

        let nodes_a = repo.get_linked_nodes("eA").await.unwrap();
        assert!(nodes_a.is_empty());
    }
}
