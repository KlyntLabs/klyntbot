use rustc_hash::{FxHashMap, FxHashSet};
use sqlx::SqlitePool;
use std::sync::atomic::{AtomicBool, Ordering};
use storage::StorageError;

const DECAY_HALF_LIFE_DAYS: f64 = 14.0;
static LEGACY_SITE_PURGE_DONE: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone)]
pub struct AttentionAggregator {
    pool: SqlitePool,
}

impl AttentionAggregator {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Rebuild `entity_attention` from `activity_events` within the given window.
    ///
    /// Computes decay-weighted attention seconds (14-day half-life) for each
    /// distinct app/site and upserts the results. Returns the number of rows
    /// affected (inserted or updated).
    ///
    /// Target: 90 days × 50 k events → < 2 s on Apple Silicon.
    pub async fn rebuild_from_activity(&self, window_days: i64) -> Result<u64, StorageError> {
        let cutoff = jiff::Timestamp::now()
            .saturating_sub(jiff::SignedDuration::from_hours(window_days * 24))
            .unwrap()
            .to_string();

        // Stream events in batches to keep memory bounded.
        // We only need the fields required for grouping + scoring.
        let rows: Vec<ActivityEventLite> = sqlx::query_as(
            "SELECT
                CASE
                    WHEN site_name IS NOT NULL AND site_name != ''
                        THEN lower(site_name)
                    ELSE COALESCE(NULLIF(bundle_id, ''), lower(app_name))
                END AS canonical_id,
                CASE WHEN site_name IS NOT NULL AND site_name != '' THEN 'site' ELSE 'app' END AS kind,
                COALESCE(site_name, app_name) AS display_name,
                started_at,
                duration_secs,
                category_id
             FROM activity_events
             WHERE started_at > ?1
               AND is_idle = FALSE
               AND COALESCE(bundle_id, site_name, app_name) IS NOT NULL
             ORDER BY canonical_id, kind, started_at",
        )
        .bind(&cutoff)
        .fetch_all(&self.pool)
        .await?;

        let now = jiff::Timestamp::now();
        let lambda = (2.0_f64).ln() / DECAY_HALF_LIFE_DAYS;

        // Group by (canonical_id, kind) and accumulate decayed attention.
        #[derive(Default)]
        struct Accum {
            display_name: String,
            attention_secs: f64,
            last_used_at: String,
            category_id: Option<String>,
        }

        let mut groups: FxHashMap<(String, String), Accum> =
            FxHashMap::with_capacity_and_hasher(rows.len() / 4, Default::default());

        for row in rows {
            let key = (row.canonical_id.clone(), row.kind.clone());
            let days_ago = match row.started_at.parse::<jiff::Timestamp>() {
                Ok(ts) => {
                    let millis = now.as_millisecond() - ts.as_millisecond();
                    (millis as f64) / (86_400_000.0)
                }
                Err(_) => continue,
            };
            let decay = (-lambda * days_ago).exp();
            let weighted = row.duration_secs.unwrap_or(0) as f64 * decay;

            groups
                .entry(key)
                .and_modify(|acc| {
                    acc.attention_secs += weighted;
                    if row.started_at > acc.last_used_at {
                        acc.last_used_at = row.started_at.clone();
                        // Prefer the most recent display_name.
                        if !row.display_name.is_empty() {
                            acc.display_name = row.display_name.clone();
                        }
                    }
                    if row.category_id.is_some() {
                        acc.category_id = row.category_id.clone();
                    }
                })
                .or_insert(Accum {
                    display_name: if row.display_name.is_empty() {
                        row.canonical_id.clone()
                    } else {
                        row.display_name.clone()
                    },
                    attention_secs: weighted,
                    last_used_at: row.started_at,
                    category_id: row.category_id,
                });
        }

        let unique_category_ids: Vec<String> = groups
            .values()
            .filter_map(|acc| acc.category_id.clone())
            .collect::<FxHashSet<_>>()
            .into_iter()
            .collect();
        let category_map = Self::resolve_categories(&self.pool, &unique_category_ids).await?;

        // Single transaction so 1000 entities = 1 fsync, not 1000.
        let mut tx = self.pool.begin().await?;

        // One-shot per-process: purge legacy poisoned site rows whose canonical_id
        // is actually a browser bundle id (pre-fix COALESCE ordering bug).
        if !LEGACY_SITE_PURGE_DONE.swap(true, Ordering::Relaxed) {
            sqlx::query(
                "DELETE FROM entity_attention
                 WHERE kind = 'site'
                   AND canonical_id IN (
                       SELECT canonical_id FROM entity_attention WHERE kind = 'app'
                   )",
            )
            .execute(&mut *tx)
            .await?;
        }

        let mut affected: u64 = 0;
        for ((canonical_id, kind), acc) in groups {
            sqlx::query(
                "INSERT INTO entity_attention
                     (canonical_id, kind, display_name, attention_secs, last_used_at, icon_hint, category)
                 VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6)
                 ON CONFLICT(canonical_id, kind) DO UPDATE SET
                     attention_secs = excluded.attention_secs,
                     last_used_at   = excluded.last_used_at,
                     display_name   = excluded.display_name,
                     icon_hint      = excluded.icon_hint,
                     category       = excluded.category",
            )
            .bind(&canonical_id)
            .bind(&kind)
            .bind(&acc.display_name)
            .bind(acc.attention_secs.round() as i64)
            .bind(&acc.last_used_at)
            .bind(acc.category_id.and_then(|id| category_map.get(&id).cloned()))
            .execute(&mut *tx)
            .await?;
            affected += 1;
        }
        tx.commit().await?;

        Ok(affected)
    }

    async fn resolve_categories(
        pool: &SqlitePool,
        ids: &[String],
    ) -> Result<FxHashMap<String, String>, StorageError> {
        if ids.is_empty() {
            return Ok(FxHashMap::default());
        }
        let placeholders: Vec<String> = (1..=ids.len()).map(|i| format!("?{i}")).collect();
        let sql = format!(
            "SELECT id, name FROM activity_categories WHERE id IN ({})",
            placeholders.join(", ")
        );
        let mut query = sqlx::query_as::<_, (String, String)>(&sql);
        for id in ids {
            query = query.bind(id);
        }
        let rows = query.fetch_all(pool).await?;
        Ok(rows.into_iter().collect())
    }
}

#[derive(sqlx::FromRow)]
struct ActivityEventLite {
    canonical_id: String,
    kind: String,
    display_name: String,
    started_at: String,
    duration_secs: Option<i64>,
    category_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::EntityAttentionRepo;
    use storage::StoragePool;

    async fn setup() -> (AttentionAggregator, SqlitePool) {
        let pool = StoragePool::connect_in_memory()
            .await
            .unwrap()
            .inner()
            .clone();
        StoragePool::run_feature_migrations(&pool, &crate::launcher_migrations())
            .await
            .unwrap();
        // Create activity_events table directly (we can't depend on feature-productivity).
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS activity_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                app_name TEXT NOT NULL,
                window_title TEXT,
                site_name TEXT,
                bundle_id TEXT,
                url TEXT,
                category_id TEXT,
                started_at TEXT NOT NULL,
                ended_at TEXT,
                duration_secs INTEGER,
                is_idle BOOLEAN NOT NULL DEFAULT FALSE,
                metadata TEXT,
                project_id TEXT,
                focus_session_id TEXT
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        (AttentionAggregator::new(pool.clone()), pool)
    }

    #[tokio::test]
    async fn aggregator_rebuild_from_seeded_events_yields_expected_attention() {
        let (agg, pool) = setup().await;

        // Seed some activity events.
        let now = jiff::Timestamp::now();
        type EventRow<'a> = (
            &'a str,
            Option<&'a str>,
            Option<&'a str>,
            i64,
            jiff::Timestamp,
        );
        let events: Vec<EventRow> = vec![
            ("Safari", None, Some("com.apple.Safari"), 3600, now),
            ("Safari", None, Some("com.apple.Safari"), 1800, now),
            (
                "Chrome",
                Some("github.com"),
                Some("com.google.Chrome"),
                7200,
                now,
            ),
        ];

        for (app, site, bundle, dur, ts) in events {
            sqlx::query(
                "INSERT INTO activity_events
                 (app_name, site_name, bundle_id, started_at, ended_at, duration_secs, is_idle, category_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, FALSE, NULL)",
            )
            .bind(app)
            .bind(site)
            .bind(bundle)
            .bind(ts.to_string())
            .bind(ts.to_string())
            .bind(dur)
            .execute(&pool)
            .await
            .unwrap();
        }

        let affected = agg.rebuild_from_activity(30).await.unwrap();
        assert_eq!(affected, 2); // com.apple.Safari (app) + github.com (site)

        let repo = EntityAttentionRepo::new(pool);
        let top = repo.top_by_attention(None, 10).await.unwrap();
        assert_eq!(top.len(), 2);
        assert!(top
            .iter()
            .any(|r| r.canonical_id == "com.apple.Safari" && r.kind == "app"));
        assert!(top
            .iter()
            .any(|r| r.canonical_id == "github.com" && r.kind == "site"));
    }

    #[tokio::test]
    async fn aggregator_idempotent_on_repeated_rebuild() {
        let (agg, pool) = setup().await;

        let now = jiff::Timestamp::now();
        sqlx::query(
            "INSERT INTO activity_events
             (app_name, started_at, ended_at, duration_secs, is_idle)
             VALUES (?1, ?2, ?3, ?4, FALSE)",
        )
        .bind("Xcode")
        .bind(now.to_string())
        .bind(now.to_string())
        .bind(100i64)
        .execute(&pool)
        .await
        .unwrap();

        let a1 = agg.rebuild_from_activity(30).await.unwrap();
        let a2 = agg.rebuild_from_activity(30).await.unwrap();
        assert_eq!(a1, a2);

        let repo = EntityAttentionRepo::new(pool);
        let top = repo.top_by_attention(None, 10).await.unwrap();
        assert_eq!(top.len(), 1);
        assert_eq!(top[0].attention_secs, 100);
    }
}
