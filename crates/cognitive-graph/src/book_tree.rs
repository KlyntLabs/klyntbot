use async_trait::async_trait;
use common::Result;
use context_engine::book_index::types::{SourceType, TreeNode, TreeNodeType};
use context_engine::book_index::BookTreeRepo;

pub struct SqliteBookTreeRepo {
    pool: sqlx::SqlitePool,
}

impl SqliteBookTreeRepo {
    pub fn new(pool: sqlx::SqlitePool) -> Self {
        Self { pool }
    }
}

#[allow(clippy::too_many_arguments)]
fn row_to_tree_node(
    id: String,
    parent_id: Option<String>,
    node_type: String,
    content: String,
    title: Option<String>,
    level: i32,
    source_type: String,
    source_id: String,
    position: i32,
    metadata: Option<String>,
) -> TreeNode {
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
}

#[async_trait]
impl BookTreeRepo for SqliteBookTreeRepo {
    async fn insert_node(&self, node: &TreeNode) -> Result<()> {
        sqlx::query(
            "INSERT INTO book_tree_nodes (id, parent_id, node_type, content, title, level, source_type, source_id, position, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        )
        .bind(&node.id)
        .bind(&node.parent_id)
        .bind(node.node_type.as_str())
        .bind(&node.content)
        .bind(&node.title)
        .bind(node.level as i32)
        .bind(node.source_type.as_str())
        .bind(&node.source_id)
        .bind(node.position as i32)
        .bind(&node.metadata)
        .execute(&self.pool)
        .await
        .map_err(cognitive_schema::map_sqlx)?;
        Ok(())
    }

    async fn insert_nodes(&self, nodes: &[TreeNode]) -> Result<()> {
        // Insert in order to satisfy parent FK constraints (parents before children).
        for node in nodes {
            self.insert_node(node).await?;
        }
        Ok(())
    }

    async fn get_node(&self, id: &str) -> Result<Option<TreeNode>> {
        let row: Option<(String, Option<String>, String, String, Option<String>, i32, String, String, i32, Option<String>)> =
            sqlx::query_as(
                "SELECT id, parent_id, node_type, content, title, level, source_type, source_id, position, metadata
                 FROM book_tree_nodes WHERE id = ?1",
            )
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(cognitive_schema::map_sqlx)?;

        Ok(row.map(
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
                row_to_tree_node(
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
                )
            },
        ))
    }

    async fn get_children(&self, parent_id: &str) -> Result<Vec<TreeNode>> {
        let rows: Vec<(String, Option<String>, String, String, Option<String>, i32, String, String, i32, Option<String>)> =
            sqlx::query_as(
                "SELECT id, parent_id, node_type, content, title, level, source_type, source_id, position, metadata
                 FROM book_tree_nodes WHERE parent_id = ?1 ORDER BY position",
            )
            .bind(parent_id)
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
                    row_to_tree_node(
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
                    )
                },
            )
            .collect())
    }

    async fn get_subtree(&self, node_id: &str) -> Result<Vec<TreeNode>> {
        let rows: Vec<(String, Option<String>, String, String, Option<String>, i32, String, String, i32, Option<String>)> =
            sqlx::query_as(
                "WITH RECURSIVE subtree AS (
                    SELECT id, parent_id, node_type, content, title, level, source_type, source_id, position, metadata
                    FROM book_tree_nodes WHERE id = ?1
                    UNION ALL
                    SELECT n.id, n.parent_id, n.node_type, n.content, n.title, n.level, n.source_type, n.source_id, n.position, n.metadata
                    FROM book_tree_nodes n JOIN subtree s ON n.parent_id = s.id
                ) SELECT * FROM subtree",
            )
            .bind(node_id)
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
                    row_to_tree_node(
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
                    )
                },
            )
            .collect())
    }

    async fn get_root_sections(&self, source_type: &SourceType) -> Result<Vec<TreeNode>> {
        let rows: Vec<(String, Option<String>, String, String, Option<String>, i32, String, String, i32, Option<String>)> =
            sqlx::query_as(
                "SELECT id, parent_id, node_type, content, title, level, source_type, source_id, position, metadata
                 FROM book_tree_nodes WHERE parent_id IS NULL AND source_type = ?1 ORDER BY position",
            )
            .bind(source_type.as_str())
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
                    row_to_tree_node(
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
                    )
                },
            )
            .collect())
    }

    async fn get_path_to_root(&self, node_id: &str) -> Result<Vec<TreeNode>> {
        let rows: Vec<(String, Option<String>, String, String, Option<String>, i32, String, String, i32, Option<String>)> =
            sqlx::query_as(
                "WITH RECURSIVE ancestors AS (
                    SELECT id, parent_id, node_type, content, title, level, source_type, source_id, position, metadata
                    FROM book_tree_nodes WHERE id = ?1
                    UNION ALL
                    SELECT n.id, n.parent_id, n.node_type, n.content, n.title, n.level, n.source_type, n.source_id, n.position, n.metadata
                    FROM book_tree_nodes n JOIN ancestors a ON n.id = a.parent_id
                ) SELECT * FROM ancestors",
            )
            .bind(node_id)
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
                    row_to_tree_node(
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
                    )
                },
            )
            .collect())
    }

    async fn delete_by_source(&self, source_type: &SourceType, source_id: &str) -> Result<u64> {
        let result =
            sqlx::query("DELETE FROM book_tree_nodes WHERE source_type = ?1 AND source_id = ?2")
                .bind(source_type.as_str())
                .bind(source_id)
                .execute(&self.pool)
                .await
                .map_err(cognitive_schema::map_sqlx)?;
        Ok(result.rows_affected())
    }

    async fn search_fts(&self, query: &str, limit: usize) -> Result<Vec<TreeNode>> {
        let rows: Vec<(String, Option<String>, String, String, Option<String>, i32, String, String, i32, Option<String>)> =
            sqlx::query_as(
                "SELECT n.id, n.parent_id, n.node_type, n.content, n.title, n.level, n.source_type, n.source_id, n.position, n.metadata
                 FROM book_tree_nodes_fts f
                 JOIN book_tree_nodes n ON n.rowid = f.rowid
                 WHERE book_tree_nodes_fts MATCH ?1
                 LIMIT ?2",
            )
            .bind(query)
            .bind(limit as i32)
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
                    row_to_tree_node(
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
                    )
                },
            )
            .collect())
    }

    async fn has_any_nodes(&self) -> Result<bool> {
        let row: (i64,) = sqlx::query_as("SELECT EXISTS(SELECT 1 FROM book_tree_nodes LIMIT 1)")
            .fetch_one(&self.pool)
            .await
            .map_err(cognitive_schema::map_sqlx)?;
        Ok(row.0 == 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cognitive_schema::cognitive_test_pool;

    #[tokio::test]
    async fn insert_and_get_subtree() {
        let pool = cognitive_test_pool().await;
        let repo = SqliteBookTreeRepo::new(pool);
        let root = TreeNode {
            id: "root".into(),
            parent_id: None,
            node_type: TreeNodeType::Section,
            content: "Chapter 1".into(),
            title: Some("Chapter 1".into()),
            level: 0,
            source_type: SourceType::Note,
            source_id: "note-1".into(),
            position: 0,
            metadata: None,
        };
        let child = TreeNode {
            id: "child".into(),
            parent_id: Some("root".into()),
            node_type: TreeNodeType::Text,
            content: "Some paragraph".into(),
            title: None,
            level: 1,
            source_type: SourceType::Note,
            source_id: "note-1".into(),
            position: 0,
            metadata: None,
        };
        repo.insert_nodes(&[root, child]).await.unwrap();
        let subtree = repo.get_subtree("root").await.unwrap();
        assert_eq!(subtree.len(), 2);
    }

    #[tokio::test]
    async fn delete_by_source() {
        let pool = cognitive_test_pool().await;
        let repo = SqliteBookTreeRepo::new(pool);
        let node = TreeNode {
            id: "n1".into(),
            parent_id: None,
            node_type: TreeNodeType::Text,
            content: "test".into(),
            title: None,
            level: 0,
            source_type: SourceType::Note,
            source_id: "note-1".into(),
            position: 0,
            metadata: None,
        };
        repo.insert_node(&node).await.unwrap();
        let deleted = repo
            .delete_by_source(&SourceType::Note, "note-1")
            .await
            .unwrap();
        assert_eq!(deleted, 1);
        assert!(repo.get_node("n1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn fts_search() {
        let pool = cognitive_test_pool().await;
        let repo = SqliteBookTreeRepo::new(pool);
        let node = TreeNode {
            id: "n1".into(),
            parent_id: None,
            node_type: TreeNodeType::Text,
            content: "Rust programming language".into(),
            title: Some("Rust".into()),
            level: 0,
            source_type: SourceType::Note,
            source_id: "note-1".into(),
            position: 0,
            metadata: None,
        };
        repo.insert_node(&node).await.unwrap();
        let results = repo.search_fts("programming", 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "n1");
    }
}
