use storage::{StorageError, StoragePool};

use crate::types::ResourceEdge;

pub struct ResourceEdgeRepo;

impl ResourceEdgeRepo {
    pub async fn upsert(pool: &StoragePool, edge: &ResourceEdge) -> common::Result<()> {
        sqlx::query(
            "INSERT INTO resource_edges (source_id, target_id, edge_type, weight, \
             first_seen_at, last_seen_at) \
             VALUES (?1,?2,?3,?4,?5,?6) \
             ON CONFLICT(source_id, target_id, edge_type) DO UPDATE SET \
             weight = resource_edges.weight + 1.0, \
             last_seen_at = excluded.last_seen_at",
        )
        .bind(&edge.source_id)
        .bind(&edge.target_id)
        .bind(&edge.edge_type)
        .bind(edge.weight)
        .bind(edge.first_seen_at.to_rfc3339())
        .bind(edge.last_seen_at.to_rfc3339())
        .execute(pool.inner())
        .await
        .map_err(StorageError::from)?;
        Ok(())
    }

    pub async fn get_neighbors(
        pool: &StoragePool,
        resource_id: &str,
    ) -> common::Result<Vec<(String, f64)>> {
        let rows: Vec<(String, f64)> = sqlx::query_as(
            "SELECT target_id, weight FROM resource_edges WHERE source_id = ?1 \
             UNION ALL \
             SELECT source_id, weight FROM resource_edges WHERE target_id = ?1",
        )
        .bind(resource_id)
        .fetch_all(pool.inner())
        .await
        .map_err(StorageError::from)?;
        Ok(rows)
    }

    pub async fn get_co_accessed(
        pool: &StoragePool,
        resource_id: &str,
        min_weight: f64,
    ) -> common::Result<Vec<(String, f64)>> {
        let rows: Vec<(String, f64)> = sqlx::query_as(
            "SELECT target_id, weight FROM resource_edges \
             WHERE source_id = ?1 AND edge_type = 'co_access' AND weight >= ?2 \
             UNION ALL \
             SELECT source_id, weight FROM resource_edges \
             WHERE target_id = ?1 AND edge_type = 'co_access' AND weight >= ?2",
        )
        .bind(resource_id)
        .bind(min_weight)
        .fetch_all(pool.inner())
        .await
        .map_err(StorageError::from)?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use crate::normalizers::new_ulid;
    use crate::types::ResourceEdge;
    use crate::work_resource_repo::tests::make_resource;
    use crate::work_resource_repo::WorkResourceRepo;

    fn make_edge(source: &str, target: &str) -> ResourceEdge {
        let now = Utc::now();
        ResourceEdge {
            source_id: source.to_string(),
            target_id: target.to_string(),
            edge_type: "co_access".to_string(),
            weight: 1.0,
            first_seen_at: now,
            last_seen_at: now,
        }
    }

    #[tokio::test]
    async fn test_upsert_increments_weight() {
        let pool = crate::test_pool().await;
        let r1_id = new_ulid();
        let r2_id = new_ulid();
        let edge = make_edge(&r1_id, &r2_id);
        ResourceEdgeRepo::upsert(&pool, &edge).await.unwrap();
        ResourceEdgeRepo::upsert(&pool, &edge).await.unwrap();

        let neighbors = ResourceEdgeRepo::get_neighbors(&pool, &r1_id)
            .await
            .unwrap();
        assert_eq!(neighbors.len(), 1);
        assert!((neighbors[0].1 - 2.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_get_neighbors() {
        let pool = crate::test_pool().await;
        let r1 = make_resource("a.rs", None);
        let r2 = make_resource("b.rs", None);
        WorkResourceRepo::upsert(&pool, &r1).await.unwrap();
        WorkResourceRepo::upsert(&pool, &r2).await.unwrap();

        let edge = make_edge(&r1.id, &r2.id);
        ResourceEdgeRepo::upsert(&pool, &edge).await.unwrap();

        let neighbors = ResourceEdgeRepo::get_neighbors(&pool, &r1.id)
            .await
            .unwrap();
        assert_eq!(neighbors.len(), 1);
        assert_eq!(neighbors[0].0, r2.id);
    }
}
