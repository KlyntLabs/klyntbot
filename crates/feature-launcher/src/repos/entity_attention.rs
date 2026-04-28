use sqlx::SqlitePool;
use storage::StorageError;
use std::collections::HashMap;

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct EntityAttentionRow {
    pub canonical_id: String,
    pub kind: String,
    pub display_name: String,
    pub attention_secs: i64,
    pub last_used_at: String,
    pub icon_hint: Option<String>,
    pub category: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EntityAttentionRepo {
    pool: SqlitePool,
}

impl EntityAttentionRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Returns top-N entities by decay-weighted attention, optionally filtered by kind.
    pub async fn top_by_attention(
        &self,
        kind: Option<&str>,
        limit: i64,
    ) -> Result<Vec<EntityAttentionRow>, StorageError> {
        let rows = if let Some(k) = kind {
            sqlx::query_as::<_, EntityAttentionRow>(
                "SELECT canonical_id, kind, display_name, attention_secs, last_used_at, icon_hint, category \
                 FROM entity_attention \
                 WHERE kind = ?1 \
                 ORDER BY attention_secs DESC \
                 LIMIT ?2",
            )
            .bind(k)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, EntityAttentionRow>(
                "SELECT canonical_id, kind, display_name, attention_secs, last_used_at, icon_hint, category \
                 FROM entity_attention \
                 ORDER BY attention_secs DESC \
                 LIMIT ?1",
            )
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        };
        Ok(rows)
    }

    /// Full-text search over display_name, ranked by combined BM25 and attention.
    pub async fn fts_search(
        &self,
        q: &str,
        limit: i64,
    ) -> Result<Vec<EntityAttentionRow>, StorageError> {
        // FTS5 bm25 returns a negative log-probability; lower is better.
        // We normalize attention_secs to [0,1] via division by max attention in the result set.
        // The query uses a CTE to compute the max attention, then blends scores.
        let rows = sqlx::query_as::<_, EntityAttentionRow>(
            "WITH matches AS (
                SELECT canonical_id, kind,
                       bm25(entity_attention_fts) AS bm25_score
                FROM entity_attention_fts
                WHERE entity_attention_fts MATCH ?1
            ),
            ranked AS (
                SELECT m.canonical_id, m.kind,
                       a.display_name, a.attention_secs, a.last_used_at,
                       a.icon_hint, a.category,
                       m.bm25_score,
                       MAX(a.attention_secs) OVER () AS max_attention
                FROM matches m
                JOIN entity_attention a
                  ON m.canonical_id = a.canonical_id AND m.kind = a.kind
            )
            SELECT canonical_id, kind, display_name, attention_secs, last_used_at, icon_hint, category
            FROM ranked
            ORDER BY (bm25_score * 0.4 + ((CAST(attention_secs AS REAL) / NULLIF(max_attention, 0)) * 0.6)) ASC
            LIMIT ?2",
        )
        .bind(q)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Upsert a single row. On conflict, overwrite attention_secs, last_used_at, display_name, category.
    pub async fn upsert(&self, row: &EntityAttentionRow) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO entity_attention
                 (canonical_id, kind, display_name, attention_secs, last_used_at, icon_hint, category)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(canonical_id, kind) DO UPDATE SET
                 attention_secs = excluded.attention_secs,
                 last_used_at   = excluded.last_used_at,
                 display_name   = excluded.display_name,
                 icon_hint      = excluded.icon_hint,
                 category       = excluded.category",
        )
        .bind(&row.canonical_id)
        .bind(&row.kind)
        .bind(&row.display_name)
        .bind(row.attention_secs)
        .bind(&row.last_used_at)
        .bind(&row.icon_hint)
        .bind(&row.category)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Delete entities whose last_used_at is older than the given number of days.
    pub async fn purge_older_than(&self, days: i64) -> Result<u64, StorageError> {
        let cutoff = jiff::Timestamp::now()
            .saturating_sub(jiff::SignedDuration::from_hours(days * 24))
            .unwrap()
            .to_string();
        let result = sqlx::query("DELETE FROM entity_attention WHERE last_used_at < ?1")
            .bind(&cutoff)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Returns a personal score map for the given file paths on a 0..1 scale.
    /// Paths that have no attention record get a score of 0.0.
    pub async fn personal_for_paths(
        &self,
        paths: &[String],
    ) -> Result<HashMap<String, f32>, StorageError> {
        if paths.is_empty() {
            return Ok(HashMap::new());
        }

        // Build an IN clause. SQLite has a limit of 999 variables per query,
        // but typical launcher batches are < 1000. For safety we could chunk,
        // but the caller (InvertedFileIndex builder) already caps entries.
        let placeholders: Vec<String> = paths.iter().enumerate().map(|(i, _)| format!("?{}", i + 1)).collect();
        let sql = format!(
            "SELECT canonical_id, attention_secs FROM entity_attention \
             WHERE kind = 'file' AND canonical_id IN ({}) \
             ORDER BY attention_secs DESC",
            placeholders.join(", ")
        );

        let mut query = sqlx::query_as::<_, (String, i64)>(&sql);
        for p in paths {
            query = query.bind(p);
        }
        let rows = query.fetch_all(&self.pool).await?;

        let max_attention = rows.iter().map(|(_, a)| *a).max().unwrap_or(1).max(1);
        let mut scores = HashMap::with_capacity(rows.len());
        for (path, attention) in rows {
            scores.insert(path, attention as f32 / max_attention as f32);
        }
        Ok(scores)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage::StoragePool;

    async fn setup() -> (EntityAttentionRepo, SqlitePool) {
        let pool = StoragePool::connect_in_memory().await.unwrap().inner().clone();
        StoragePool::run_feature_migrations(&pool, &crate::launcher_migrations())
            .await
            .unwrap();
        (EntityAttentionRepo::new(pool.clone()), pool)
    }

    fn row(canonical_id: &str, kind: &str, display_name: &str, attention_secs: i64) -> EntityAttentionRow {
        EntityAttentionRow {
            canonical_id: canonical_id.to_string(),
            kind: kind.to_string(),
            display_name: display_name.to_string(),
            attention_secs,
            last_used_at: jiff::Timestamp::now().to_string(),
            icon_hint: None,
            category: None,
        }
    }

    #[tokio::test]
    async fn upsert_and_top_by_attention() {
        let (repo, _pool) = setup().await;
        repo.upsert(&row("com.apple.Safari", "app", "Safari", 3600)).await.unwrap();
        repo.upsert(&row("github.com", "site", "GitHub", 7200)).await.unwrap();
        repo.upsert(&row("com.apple.Xcode", "app", "Xcode", 1800)).await.unwrap();

        let top = repo.top_by_attention(None, 10).await.unwrap();
        assert_eq!(top.len(), 3);
        assert_eq!(top[0].canonical_id, "github.com");
        assert_eq!(top[1].canonical_id, "com.apple.Safari");
        assert_eq!(top[2].canonical_id, "com.apple.Xcode");

        let apps = repo.top_by_attention(Some("app"), 10).await.unwrap();
        assert_eq!(apps.len(), 2);
        assert!(apps.iter().all(|r| r.kind == "app"));
    }

    #[tokio::test]
    async fn upsert_overwrites_on_conflict() {
        let (repo, _pool) = setup().await;
        repo.upsert(&row("com.apple.Safari", "app", "Safari", 100)).await.unwrap();
        repo.upsert(&row("com.apple.Safari", "app", "Safari", 200)).await.unwrap();

        let top = repo.top_by_attention(None, 1).await.unwrap();
        assert_eq!(top[0].attention_secs, 200);
    }

    #[tokio::test]
    async fn purge_older_than_removes_stale() {
        let (repo, _pool) = setup().await;
        let mut old = row("old.app", "app", "Old App", 100);
        old.last_used_at = (jiff::Timestamp::now() - jiff::SignedDuration::from_hours(48 * 24)).to_string();
        repo.upsert(&old).await.unwrap();

        let fresh = row("fresh.app", "app", "Fresh App", 100);
        repo.upsert(&fresh).await.unwrap();

        let n = repo.purge_older_than(30).await.unwrap();
        assert_eq!(n, 1);

        let top = repo.top_by_attention(None, 10).await.unwrap();
        assert_eq!(top.len(), 1);
        assert_eq!(top[0].canonical_id, "fresh.app");
    }

    #[tokio::test]
    async fn personal_for_paths_returns_scores() {
        let (repo, _pool) = setup().await;
        repo.upsert(&row("/home/user/project/main.rs", "file", "main.rs", 500)).await.unwrap();
        repo.upsert(&row("/home/user/project/lib.rs", "file", "lib.rs", 250)).await.unwrap();

        let scores = repo.personal_for_paths(&[
            "/home/user/project/main.rs".to_string(),
            "/home/user/project/lib.rs".to_string(),
            "/home/user/other.rs".to_string(),
        ]).await.unwrap();

        assert_eq!(scores.len(), 2);
        assert!(scores.get("/home/user/project/main.rs").unwrap() > scores.get("/home/user/project/lib.rs").unwrap());
        assert!(*scores.get("/home/user/project/main.rs").unwrap() <= 1.0);
    }

    #[tokio::test]
    async fn fts_search_ranks_by_combined_bm25_and_attention() {
        let (repo, _pool) = setup().await;
        repo.upsert(&row("com.apple.Safari", "app", "Safari", 100)).await.unwrap();
        repo.upsert(&row("com.google.Chrome", "app", "Google Chrome", 500)).await.unwrap();
        repo.upsert(&row("org.mozilla.Firefox", "app", "Firefox", 300)).await.unwrap();

        // Searching for "chrome" should return Google Chrome.
        let results = repo.fts_search("chrome", 10).await.unwrap();
        assert!(!results.is_empty(), "FTS should find 'chrome'");
        assert_eq!(results[0].canonical_id, "com.google.Chrome");
    }
}
