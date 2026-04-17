use storage::{StorageError, StoragePool};

use crate::normalizers::parse_rfc3339;
use crate::types::WorkResource;

const WR_COLS: &str = "id, resource_type, resource_name, resource_path, resource_uri, \
     first_seen_at, last_seen_at, access_count, embedding_id";

const WR_COLS_PREFIXED: &str = "r.id, r.resource_type, r.resource_name, r.resource_path, \
     r.resource_uri, r.first_seen_at, r.last_seen_at, r.access_count, r.embedding_id";

pub struct WorkResourceRepo;

impl WorkResourceRepo {
    pub async fn upsert(pool: &StoragePool, res: &WorkResource) -> common::Result<()> {
        sqlx::query(
            "INSERT INTO work_resources (id, resource_type, resource_name, resource_path, \
             resource_uri, first_seen_at, last_seen_at, access_count, embedding_id) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9) \
             ON CONFLICT(id) DO UPDATE SET \
             last_seen_at = excluded.last_seen_at, \
             access_count = work_resources.access_count + 1",
        )
        .bind(&res.id)
        .bind(&res.resource_type)
        .bind(&res.resource_name)
        .bind(&res.resource_path)
        .bind(&res.resource_uri)
        .bind(res.first_seen_at.to_string())
        .bind(res.last_seen_at.to_string())
        .bind(res.access_count)
        .bind(&res.embedding_id)
        .execute(pool.inner())
        .await
        .map_err(StorageError::from)?;
        Ok(())
    }

    pub async fn get(pool: &StoragePool, id: &str) -> common::Result<Option<WorkResource>> {
        let row = sqlx::query_as::<_, WrRawRow>(&format!(
            "SELECT {WR_COLS} FROM work_resources WHERE id = ?1"
        ))
        .bind(id)
        .fetch_optional(pool.inner())
        .await
        .map_err(StorageError::from)?;
        Ok(row.map(Into::into))
    }

    pub async fn find_by_name(pool: &StoragePool, name: &str) -> common::Result<Vec<WorkResource>> {
        let rows = sqlx::query_as::<_, WrRawRow>(&format!(
            "SELECT {WR_COLS} FROM work_resources WHERE resource_name = ?1"
        ))
        .bind(name)
        .fetch_all(pool.inner())
        .await
        .map_err(StorageError::from)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn find_by_path(pool: &StoragePool, path: &str) -> common::Result<Vec<WorkResource>> {
        let rows = sqlx::query_as::<_, WrRawRow>(&format!(
            "SELECT {WR_COLS} FROM work_resources WHERE resource_path = ?1"
        ))
        .bind(path)
        .fetch_all(pool.inner())
        .await
        .map_err(StorageError::from)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn find_by_uri(pool: &StoragePool, uri: &str) -> common::Result<Vec<WorkResource>> {
        let rows = sqlx::query_as::<_, WrRawRow>(&format!(
            "SELECT {WR_COLS} FROM work_resources WHERE resource_uri = ?1"
        ))
        .bind(uri)
        .fetch_all(pool.inner())
        .await
        .map_err(StorageError::from)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn list_recent(pool: &StoragePool, limit: i64) -> common::Result<Vec<WorkResource>> {
        let rows = sqlx::query_as::<_, WrRawRow>(&format!(
            "SELECT {WR_COLS} FROM work_resources ORDER BY last_seen_at DESC LIMIT ?1"
        ))
        .bind(limit)
        .fetch_all(pool.inner())
        .await
        .map_err(StorageError::from)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn get_by_ids(
        pool: &StoragePool,
        ids: &[String],
    ) -> common::Result<Vec<WorkResource>> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        // Use individual queries to avoid dynamic SQL placeholder issues
        let mut results = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(res) = Self::get(pool, id).await? {
                results.push(res);
            }
        }
        Ok(results)
    }

    pub async fn list_by_context(
        pool: &StoragePool,
        context_id: &str,
    ) -> common::Result<Vec<WorkResource>> {
        let rows = sqlx::query_as::<_, WrRawRow>(&format!(
            "SELECT {WR_COLS_PREFIXED} FROM work_resources r \
             INNER JOIN work_context_resources cr ON cr.resource_id = r.id \
             WHERE cr.context_id = ?1 \
             ORDER BY cr.relevance_score DESC"
        ))
        .bind(context_id)
        .fetch_all(pool.inner())
        .await
        .map_err(StorageError::from)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }
}

#[derive(sqlx::FromRow)]
pub(crate) struct WrRawRow {
    pub id: String,
    pub resource_type: String,
    pub resource_name: String,
    pub resource_path: Option<String>,
    pub resource_uri: Option<String>,
    pub first_seen_at: String,
    pub last_seen_at: String,
    pub access_count: i64,
    pub embedding_id: Option<String>,
}

impl From<WrRawRow> for WorkResource {
    fn from(row: WrRawRow) -> Self {
        Self {
            id: row.id,
            resource_type: row.resource_type,
            resource_name: row.resource_name,
            resource_path: row.resource_path,
            resource_uri: row.resource_uri,
            first_seen_at: parse_rfc3339(&row.first_seen_at),
            last_seen_at: parse_rfc3339(&row.last_seen_at),
            access_count: row.access_count,
            embedding_id: row.embedding_id,
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::normalizers::new_ulid;
    use jiff::Timestamp;

    pub(crate) fn make_resource(name: &str, path: Option<&str>) -> WorkResource {
        let now = Timestamp::now();
        WorkResource {
            id: new_ulid(),
            resource_type: "file".to_string(),
            resource_name: name.to_string(),
            resource_path: path.map(String::from),
            resource_uri: None,
            first_seen_at: now,
            last_seen_at: now,
            access_count: 1,
            embedding_id: None,
        }
    }

    #[tokio::test]
    async fn test_upsert_and_get() {
        let pool = crate::test_pool().await;
        let res = make_resource("main.rs", Some("/src/main.rs"));
        WorkResourceRepo::upsert(&pool, &res).await.unwrap();
        let loaded = WorkResourceRepo::get(&pool, &res.id).await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.as_ref().unwrap().resource_name, "main.rs");
        assert_eq!(loaded.as_ref().unwrap().access_count, 1);

        // Upsert again — access_count should increment
        WorkResourceRepo::upsert(&pool, &res).await.unwrap();
        let loaded = WorkResourceRepo::get(&pool, &res.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(loaded.access_count, 2);
    }

    #[tokio::test]
    async fn test_find_by_path() {
        let pool = crate::test_pool().await;
        let res = make_resource("lib.rs", Some("/src/lib.rs"));
        WorkResourceRepo::upsert(&pool, &res).await.unwrap();
        let results = WorkResourceRepo::find_by_path(&pool, "/src/lib.rs")
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_list_recent() {
        let pool = crate::test_pool().await;
        for name in ["a.rs", "b.rs", "c.rs"] {
            let res = make_resource(name, None);
            WorkResourceRepo::upsert(&pool, &res).await.unwrap();
        }
        let recent = WorkResourceRepo::list_recent(&pool, 10).await.unwrap();
        assert_eq!(recent.len(), 3);
    }
}
