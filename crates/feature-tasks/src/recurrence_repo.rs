//! SQLite-backed implementations of `scheduling::TemplateRepo` and
//! `scheduling::InstanceRepo` for the recurrence engine.

use async_trait::async_trait;
use jiff::Timestamp;
use scheduling::temporal::recurrence::{InstanceRepo, RecurrenceTemplate, TemplateRepo};
use storage::StoragePool;

// ---------------------------------------------------------------------------
// SqliteTemplateRepo
// ---------------------------------------------------------------------------

/// SQLite implementation of `TemplateRepo` backed by `task_recurrence_templates`.
pub struct SqliteTemplateRepo {
    pool: StoragePool,
}

impl SqliteTemplateRepo {
    pub fn new(pool: StoragePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TemplateRepo for SqliteTemplateRepo {
    async fn get(&self, id: &str) -> anyhow::Result<Option<RecurrenceTemplate>> {
        let row: Option<(
            String,      // id
            String,      // source_task_id
            String,      // rrule
            String,      // iana_tz
            i64,         // materialize_ahead
            Option<i64>, // next_instance_at_ms
            Option<i64>, // until_at_ms
            Option<i64>, // count_remaining
            i64,         // enabled
        )> = sqlx::query_as(
            r#"
            SELECT id, source_task_id, rrule, iana_tz, materialize_ahead,
                   next_instance_at_ms, until_at_ms, count_remaining, enabled
              FROM task_recurrence_templates
             WHERE id = ?1
            "#,
        )
        .bind(id)
        .fetch_optional(self.pool.inner())
        .await?;

        let Some((
            id,
            source_task_id,
            rrule,
            iana_tz,
            materialize_ahead,
            next_instance_at_ms,
            until_at_ms,
            count_remaining,
            enabled,
        )) = row
        else {
            return Ok(None);
        };

        let next_instance_at = next_instance_at_ms
            .map(Timestamp::from_millisecond)
            .transpose()
            .map_err(|e| anyhow::anyhow!("invalid next_instance_at_ms: {e}"))?;

        let until_at = until_at_ms
            .map(Timestamp::from_millisecond)
            .transpose()
            .map_err(|e| anyhow::anyhow!("invalid until_at_ms: {e}"))?;

        Ok(Some(RecurrenceTemplate {
            id,
            source_task_id,
            rrule,
            iana_tz,
            materialize_ahead: materialize_ahead as u32,
            next_instance_at,
            until_at,
            count_remaining: count_remaining.map(|c| c as u32),
            enabled: enabled != 0,
        }))
    }

    async fn update_next_instance(
        &self,
        id: &str,
        next_at: Option<Timestamp>,
    ) -> anyhow::Result<()> {
        let next_ms: Option<i64> = next_at.map(|t| t.as_millisecond());
        sqlx::query("UPDATE task_recurrence_templates SET next_instance_at_ms = ?1 WHERE id = ?2")
            .bind(next_ms)
            .bind(id)
            .execute(self.pool.inner())
            .await?;
        Ok(())
    }

    async fn decrement_count(&self, id: &str) -> anyhow::Result<Option<u32>> {
        // Atomic decrement + read in a transaction.
        let mut tx = self.pool.inner().begin().await?;

        sqlx::query(
            r#"
            UPDATE task_recurrence_templates
               SET count_remaining = count_remaining - 1
             WHERE id = ?1
               AND count_remaining IS NOT NULL
               AND count_remaining > 0
            "#,
        )
        .bind(id)
        .execute(&mut *tx)
        .await?;

        let val: Option<Option<i64>> = sqlx::query_scalar(
            "SELECT count_remaining FROM task_recurrence_templates WHERE id = ?1",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(val.flatten().map(|c| c as u32))
    }

    async fn disable(&self, id: &str) -> anyhow::Result<()> {
        sqlx::query("UPDATE task_recurrence_templates SET enabled = 0 WHERE id = ?1")
            .bind(id)
            .execute(self.pool.inner())
            .await?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// SqliteInstanceRepo
// ---------------------------------------------------------------------------

/// SQLite implementation of `InstanceRepo` backed by the `tasks` table.
pub struct SqliteInstanceRepo {
    pool: StoragePool,
}

impl SqliteInstanceRepo {
    pub fn new(pool: StoragePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl InstanceRepo for SqliteInstanceRepo {
    async fn create_instance(
        &self,
        template_id: &str,
        due_at: Timestamp,
    ) -> anyhow::Result<String> {
        // Copy metadata from the source task.
        let source: Option<(
            String,         // title
            String,         // area_id
            Option<String>, // project_id
            Option<String>, // key_result_id
            Option<String>, // objective_id
            Option<i16>,    // priority
            Option<String>, // energy_level
        )> = sqlx::query_as(
            r#"
            SELECT t.title, t.area_id, t.project_id, t.key_result_id, t.objective_id,
                   t.priority, t.energy_level
              FROM tasks t
              JOIN task_recurrence_templates trt ON trt.source_task_id = t.id
             WHERE trt.id = ?1
            "#,
        )
        .bind(template_id)
        .fetch_optional(self.pool.inner())
        .await?;

        let (title, area_id, project_id, key_result_id, objective_id, priority, energy_level) =
            source.ok_or_else(|| {
                anyhow::anyhow!("no source task found for template {template_id}")
            })?;

        let new_id = uuid::Uuid::new_v4().to_string();
        let now_ms = Timestamp::now().as_millisecond();
        let due_ms = due_at.as_millisecond();

        sqlx::query(
            r#"
            INSERT INTO tasks (
                id, title, area_id, project_id, key_result_id, objective_id,
                priority, due_date, status, task_type, execution_state,
                energy_level, template_id, tags,
                focus_expired_count, total_tracked_secs, position, completed,
                is_template, created_at, updated_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6,
                ?7, ?8, 'todo', 'manual', 'idle',
                ?9, ?10, '[]',
                0, 0, 0, 0,
                0, ?11, ?11
            )
            "#,
        )
        .bind(&new_id)
        .bind(&title)
        .bind(&area_id)
        .bind(&project_id)
        .bind(&key_result_id)
        .bind(&objective_id)
        .bind(priority)
        .bind(due_ms)
        .bind(&energy_level)
        .bind(template_id)
        .bind(now_ms)
        .execute(self.pool.inner())
        .await?;

        Ok(new_id)
    }

    async fn cancel_unfired_instances(&self, template_id: &str) -> anyhow::Result<()> {
        // Hard-delete future unfired instances. The tasks table has no soft-delete
        // concept beyond `completed` / `completed_at`; incomplete future instances
        // generated by a disabled template should simply be removed.
        let now_ms = Timestamp::now().as_millisecond();
        sqlx::query(
            r#"
            DELETE FROM tasks
             WHERE template_id = ?1
               AND completed_at IS NULL
               AND due_date > ?2
            "#,
        )
        .bind(template_id)
        .bind(now_ms)
        .execute(self.pool.inner())
        .await?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use storage::pool::StoragePool;
    use tools_core::FeatureMigration;

    async fn setup_pool() -> StoragePool {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        StoragePool::run_feature_migrations(
            pool.inner(),
            &[FeatureMigration {
                feature_name: "tasks".into(),
                version: 2,
                description: "tasks tables".into(),
                sql: include_str!("../migrations/001_create_tasks.sql").into(),
            }],
        )
        .await
        .unwrap();
        pool
    }

    /// Insert a bare-minimum template row (no source task needed for template-only tests).
    async fn insert_template(pool: &StoragePool, id: &str, count_remaining: Option<i64>) {
        let now_ms = Timestamp::now().as_millisecond();
        sqlx::query(
            r#"
            INSERT INTO task_recurrence_templates
                (id, source_task_id, rrule, iana_tz, materialize_ahead,
                 count_remaining, enabled, created_at_ms)
            VALUES (?1, ?2, 'FREQ=DAILY', 'UTC', 3, ?3, 1, ?4)
            "#,
        )
        .bind(id)
        .bind(format!("src_{id}"))
        .bind(count_remaining)
        .bind(now_ms)
        .execute(pool.inner())
        .await
        .unwrap();
    }

    // ------------------------------------------------------------------
    // Test 1: decrement_count — 3 decrements → 2, 1, 0; 4th stays at 0
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn decrement_count_basic() {
        let pool = setup_pool().await;
        insert_template(&pool, "t1", Some(3)).await;
        let repo = SqliteTemplateRepo::new(pool);

        assert_eq!(repo.decrement_count("t1").await.unwrap(), Some(2));
        assert_eq!(repo.decrement_count("t1").await.unwrap(), Some(1));
        assert_eq!(repo.decrement_count("t1").await.unwrap(), Some(0));
        // 4th call: already zero, WHERE count_remaining > 0 is false → no decrement
        assert_eq!(
            repo.decrement_count("t1").await.unwrap(),
            Some(0),
            "should return Some(0) when already at zero"
        );
    }

    // ------------------------------------------------------------------
    // Test 2: get — round-trip all fields including UNTIL / count_remaining
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn get_round_trip() {
        let pool = setup_pool().await;
        let now_ms = Timestamp::now().as_millisecond();
        // Truncate to ms precision for comparison.
        let next_ms: i64 = 1_800_000_000_000;
        let until_ms: i64 = 1_900_000_000_000;

        sqlx::query(
            r#"
            INSERT INTO task_recurrence_templates
                (id, source_task_id, rrule, iana_tz, materialize_ahead,
                 next_instance_at_ms, until_at_ms, count_remaining, enabled, created_at_ms)
            VALUES ('t2', 'src2', 'FREQ=WEEKLY', 'America/New_York', 5,
                    ?1, ?2, 7, 1, ?3)
            "#,
        )
        .bind(next_ms)
        .bind(until_ms)
        .bind(now_ms)
        .execute(pool.inner())
        .await
        .unwrap();

        let repo = SqliteTemplateRepo::new(pool);
        let tmpl = repo.get("t2").await.unwrap().expect("row should exist");

        assert_eq!(tmpl.id, "t2");
        assert_eq!(tmpl.source_task_id, "src2");
        assert_eq!(tmpl.rrule, "FREQ=WEEKLY");
        assert_eq!(tmpl.iana_tz, "America/New_York");
        assert_eq!(tmpl.materialize_ahead, 5);
        assert_eq!(tmpl.next_instance_at.unwrap().as_millisecond(), next_ms);
        assert_eq!(tmpl.until_at.unwrap().as_millisecond(), until_ms);
        assert_eq!(tmpl.count_remaining, Some(7));
        assert!(tmpl.enabled);

        // get on missing id returns None
        assert!(repo.get("nope").await.unwrap().is_none());
    }

    // ------------------------------------------------------------------
    // Test 3: create_instance — verifies new task row with correct fields
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn create_instance_inserts_task_row() {
        let pool = setup_pool().await;
        let now_ms = Timestamp::now().as_millisecond();

        // Insert the source area (required FK).
        sqlx::query(
            "INSERT INTO areas (id, name, created_at, updated_at) VALUES ('area1', 'Test', ?1, ?1)",
        )
        .bind(now_ms)
        .execute(pool.inner())
        .await
        .unwrap();

        // Insert source task.
        sqlx::query(
            r#"
            INSERT INTO tasks (id, title, area_id, status, task_type, execution_state,
                               tags, focus_expired_count, total_tracked_secs, position,
                               completed, is_template, created_at, updated_at)
            VALUES ('src3', 'Source Task', 'area1', 'todo', 'manual', 'idle',
                    '[]', 0, 0, 0, 0, 0, ?1, ?1)
            "#,
        )
        .bind(now_ms)
        .execute(pool.inner())
        .await
        .unwrap();

        // Insert the template referencing the source task.
        sqlx::query(
            r#"
            INSERT INTO task_recurrence_templates
                (id, source_task_id, rrule, iana_tz, materialize_ahead, enabled, created_at_ms)
            VALUES ('tmpl3', 'src3', 'FREQ=DAILY', 'UTC', 3, 1, ?1)
            "#,
        )
        .bind(now_ms)
        .execute(pool.inner())
        .await
        .unwrap();

        let due_at = Timestamp::from_millisecond(2_000_000_000_000).unwrap();
        let repo = SqliteInstanceRepo::new(pool.clone());
        let new_id = repo.create_instance("tmpl3", due_at).await.unwrap();

        // Verify the new task row.
        let row: (String, Option<i64>, Option<String>) =
            sqlx::query_as("SELECT id, due_date, template_id FROM tasks WHERE id = ?1")
                .bind(&new_id)
                .fetch_one(pool.inner())
                .await
                .unwrap();

        assert_eq!(row.0, new_id);
        assert_eq!(row.1, Some(due_at.as_millisecond()));
        assert_eq!(row.2.as_deref(), Some("tmpl3"));
    }
}
