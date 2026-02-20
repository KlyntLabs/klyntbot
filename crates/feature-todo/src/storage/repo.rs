use chrono::{DateTime, Utc};
use sqlx::PgPool;
use storage::StorageError;

use crate::storage::rows::{TodoAttachmentRow, TodoDependencyRow, TodoRow, TodoTimeEntryRow};

#[derive(Debug, Default, Clone)]
pub struct TodoFilter {
    pub status: Option<String>,
    pub tags: Option<Vec<String>>,
    pub project_id: Option<String>,
    pub priority_min: Option<i16>,
    pub limit: Option<i64>,
    pub templates_only: bool,
}

#[derive(Debug, Clone, Default)]
pub struct TodoSummary {
    pub todo: i64,
    pub doing: i64,
    pub done: i64,
    pub total: i64,
}

#[derive(Debug, Default, Clone)]
pub struct TodoPatch {
    pub id: String,
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub priority: Option<Option<i16>>,
    pub due_date: Option<Option<DateTime<Utc>>>,
    pub tags: Option<Vec<String>>,
    pub status: Option<String>,
    pub calendar_event_uid: Option<Option<String>>,
    pub next_instance_date: Option<Option<DateTime<Utc>>>,
    pub last_reminded_at: Option<Option<DateTime<Utc>>>,
    pub estimated_minutes: Option<Option<i32>>,
}

#[derive(Debug, Clone)]
pub struct TodoRepo {
    pool: PgPool,
}

impl TodoRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn add(&self, row: &TodoRow) -> Result<TodoRow, StorageError> {
        let inserted = sqlx::query_as::<_, TodoRow>(
            r#"
            INSERT INTO todos (
                id, title, description, priority, due_date, tags, status,
                focused_at, focus_deadline, focus_expired_count,
                created_at, updated_at, completed_at,
                parent_id, project_id, total_tracked_secs, estimated_minutes,
                calendar_event_uid, last_reminded_at,
                recurrence_rule, recurrence_parent_id, is_template, next_instance_date
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7,
                $8, $9, $10,
                $11, $12, $13,
                $14, $15, $16, $17,
                $18, $19,
                $20, $21, $22, $23
            )
            RETURNING *
            "#,
        )
        .bind(&row.id)
        .bind(&row.title)
        .bind(&row.description)
        .bind(row.priority)
        .bind(row.due_date)
        .bind(&row.tags)
        .bind(&row.status)
        .bind(row.focused_at)
        .bind(row.focus_deadline)
        .bind(row.focus_expired_count)
        .bind(row.created_at)
        .bind(row.updated_at)
        .bind(row.completed_at)
        .bind(&row.parent_id)
        .bind(&row.project_id)
        .bind(row.total_tracked_secs)
        .bind(row.estimated_minutes)
        .bind(&row.calendar_event_uid)
        .bind(row.last_reminded_at)
        .bind(&row.recurrence_rule)
        .bind(&row.recurrence_parent_id)
        .bind(row.is_template)
        .bind(row.next_instance_date)
        .fetch_one(&self.pool)
        .await?;

        Ok(inserted)
    }

    pub async fn get(&self, id: &str) -> Result<Option<TodoRow>, StorageError> {
        let row = sqlx::query_as::<_, TodoRow>("SELECT * FROM todos WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn get_or_err(&self, id: &str) -> Result<TodoRow, StorageError> {
        self.get(id)
            .await?
            .ok_or_else(|| StorageError::NotFound(format!("todo {id}")))
    }

    pub async fn update(&self, patch: &TodoPatch) -> Result<TodoRow, StorageError> {
        let row = sqlx::query_as::<_, TodoRow>(
            r#"
            UPDATE todos SET
                title              = COALESCE($2, title),
                description        = CASE WHEN $3 THEN $4 ELSE description END,
                priority           = CASE WHEN $5 THEN $6 ELSE priority END,
                due_date           = CASE WHEN $7 THEN $8 ELSE due_date END,
                tags               = COALESCE($9, tags),
                status             = COALESCE($10, status),
                completed_at       = CASE
                    WHEN $10 = 'done' AND completed_at IS NULL THEN now()
                    WHEN $10 IS NOT NULL AND $10 != 'done' THEN NULL
                    ELSE completed_at
                END,
                calendar_event_uid = CASE WHEN $11 THEN $12 ELSE calendar_event_uid END,
                next_instance_date = CASE WHEN $13 THEN $14 ELSE next_instance_date END,
                last_reminded_at   = CASE WHEN $15 THEN $16 ELSE last_reminded_at END,
                estimated_minutes  = CASE WHEN $17 THEN $18 ELSE estimated_minutes END,
                updated_at         = now()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(&patch.id)
        .bind(&patch.title)
        .bind(patch.description.is_some())
        .bind(patch.description.as_ref().and_then(|d| d.as_deref()))
        .bind(patch.priority.is_some())
        .bind(patch.priority.unwrap_or_default())
        .bind(patch.due_date.is_some())
        .bind(patch.due_date.unwrap_or_default())
        .bind(&patch.tags)
        .bind(&patch.status)
        .bind(patch.calendar_event_uid.is_some())
        .bind(patch.calendar_event_uid.as_ref().and_then(|v| v.as_deref()))
        .bind(patch.next_instance_date.is_some())
        .bind(patch.next_instance_date.unwrap_or_default())
        .bind(patch.last_reminded_at.is_some())
        .bind(patch.last_reminded_at.unwrap_or_default())
        .bind(patch.estimated_minutes.is_some())
        .bind(patch.estimated_minutes.unwrap_or_default())
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| StorageError::NotFound(format!("todo {}", patch.id)))?;

        Ok(row)
    }

    pub async fn delete(&self, id: &str) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM todos WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn list(&self, filter: &TodoFilter) -> Result<Vec<TodoRow>, StorageError> {
        let mut qb = sqlx::QueryBuilder::<sqlx::Postgres>::new("SELECT * FROM todos WHERE ");

        if filter.templates_only {
            qb.push("is_template = TRUE");
        } else {
            qb.push("is_template = FALSE");
        }

        if let Some(ref status) = filter.status {
            qb.push(" AND status = ");
            qb.push_bind(status);
        }

        if let Some(ref tags) = filter.tags {
            qb.push(" AND tags @> ");
            qb.push_bind(tags);
        }

        if let Some(ref project_id) = filter.project_id {
            qb.push(" AND project_id = ");
            qb.push_bind(project_id);
        }

        if let Some(pmin) = filter.priority_min {
            qb.push(" AND priority >= ");
            qb.push_bind(pmin);
        }

        qb.push(" ORDER BY created_at DESC");

        if let Some(limit) = filter.limit {
            qb.push(" LIMIT ");
            qb.push_bind(limit);
        }

        let rows = qb.build_query_as::<TodoRow>().fetch_all(&self.pool).await?;
        Ok(rows)
    }

    pub async fn list_templates(&self) -> Result<Vec<TodoRow>, StorageError> {
        self.list(&TodoFilter {
            templates_only: true,
            ..Default::default()
        })
        .await
    }

    pub async fn search_by_keyword(
        &self,
        query: &str,
        limit: Option<i64>,
    ) -> Result<Vec<TodoRow>, StorageError> {
        let escaped = query
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let pattern = format!("%{escaped}%");

        let rows = if let Some(lim) = limit {
            sqlx::query_as::<_, TodoRow>(
                r#"
                SELECT * FROM todos
                WHERE is_template = FALSE
                  AND (title ILIKE $1 OR description ILIKE $1)
                ORDER BY created_at DESC
                LIMIT $2
                "#,
            )
            .bind(&pattern)
            .bind(lim)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, TodoRow>(
                r#"
                SELECT * FROM todos
                WHERE is_template = FALSE
                  AND (title ILIKE $1 OR description ILIKE $1)
                ORDER BY created_at DESC
                "#,
            )
            .bind(&pattern)
            .fetch_all(&self.pool)
            .await?
        };
        Ok(rows)
    }

    pub async fn focus(
        &self,
        id: &str,
        max_slots: i64,
        deadline: Option<DateTime<Utc>>,
    ) -> Result<bool, StorageError> {
        let result = sqlx::query(
            r#"
            UPDATE todos
            SET focused_at = now(), focus_deadline = $3, updated_at = now()
            WHERE id = $1
              AND focused_at IS NULL
              AND (SELECT COUNT(*) FROM todos WHERE focused_at IS NOT NULL) < $2
            "#,
        )
        .bind(id)
        .bind(max_slots)
        .bind(deadline)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn unfocus(&self, id: &str) -> Result<bool, StorageError> {
        let result = sqlx::query(
            "UPDATE todos SET focused_at = NULL, focus_deadline = NULL, updated_at = now() WHERE id = $1",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn list_focused(&self) -> Result<Vec<TodoRow>, StorageError> {
        let rows = sqlx::query_as::<_, TodoRow>(
            "SELECT * FROM todos WHERE focused_at IS NOT NULL ORDER BY focused_at",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn add_dependency(
        &self,
        task_id: &str,
        blocker_id: &str,
    ) -> Result<(), StorageError> {
        let would_cycle = self.would_create_cycle(task_id, blocker_id).await?;
        if would_cycle {
            return Err(StorageError::Conflict(format!(
                "Adding dependency {task_id} -> {blocker_id} would create a cycle"
            )));
        }

        sqlx::query("INSERT INTO todo_dependencies (task_id, blocker_id) VALUES ($1, $2)")
            .bind(task_id)
            .bind(blocker_id)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                if let sqlx::Error::Database(ref db_err) = e {
                    if db_err.constraint().is_some() {
                        return StorageError::Conflict(format!(
                            "Cannot add dependency: {task_id} -> {blocker_id}"
                        ));
                    }
                }
                StorageError::Sqlx(e)
            })?;

        Ok(())
    }

    pub async fn remove_dependency(
        &self,
        task_id: &str,
        blocker_id: &str,
    ) -> Result<bool, StorageError> {
        let result =
            sqlx::query("DELETE FROM todo_dependencies WHERE task_id = $1 AND blocker_id = $2")
                .bind(task_id)
                .bind(blocker_id)
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn get_blockers(&self, task_id: &str) -> Result<Vec<TodoRow>, StorageError> {
        let rows = sqlx::query_as::<_, TodoRow>(
            r#"
            SELECT t.* FROM todos t
            INNER JOIN todo_dependencies d ON d.blocker_id = t.id
            WHERE d.task_id = $1
            "#,
        )
        .bind(task_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn incomplete_blockers(&self, task_id: &str) -> Result<Vec<TodoRow>, StorageError> {
        let rows = sqlx::query_as::<_, TodoRow>(
            r#"
            SELECT t.* FROM todos t
            INNER JOIN todo_dependencies d ON d.blocker_id = t.id
            WHERE d.task_id = $1 AND t.status != 'done'
            "#,
        )
        .bind(task_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_blocking(&self, blocker_id: &str) -> Result<Vec<TodoRow>, StorageError> {
        let rows = sqlx::query_as::<_, TodoRow>(
            r#"
            SELECT t.* FROM todos t
            INNER JOIN todo_dependencies d ON d.task_id = t.id
            WHERE d.blocker_id = $1
            "#,
        )
        .bind(blocker_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    async fn would_create_cycle(
        &self,
        task_id: &str,
        blocker_id: &str,
    ) -> Result<bool, StorageError> {
        let row: Option<(bool,)> = sqlx::query_as(
            r#"
            WITH RECURSIVE reachable AS (
                SELECT blocker_id AS node FROM todo_dependencies WHERE task_id = $2
                UNION
                SELECT d.blocker_id FROM todo_dependencies d
                INNER JOIN reachable r ON d.task_id = r.node
            )
            SELECT EXISTS(SELECT 1 FROM reachable WHERE node = $1)
            "#,
        )
        .bind(task_id)
        .bind(blocker_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.0).unwrap_or(false))
    }

    pub async fn get_dependencies(
        &self,
        task_id: &str,
    ) -> Result<Vec<TodoDependencyRow>, StorageError> {
        let rows = sqlx::query_as::<_, TodoDependencyRow>(
            "SELECT * FROM todo_dependencies WHERE task_id = $1",
        )
        .bind(task_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn add_attachment(
        &self,
        todo_id: &str,
        attachment_type: &str,
        value: &str,
        title: Option<&str>,
        tags: &[String],
    ) -> Result<TodoAttachmentRow, StorageError> {
        let row = sqlx::query_as::<_, TodoAttachmentRow>(
            r#"
            INSERT INTO todo_attachments (todo_id, attachment_type, value, title, tags)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *
            "#,
        )
        .bind(todo_id)
        .bind(attachment_type)
        .bind(value)
        .bind(title)
        .bind(tags)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn remove_attachment(
        &self,
        todo_id: &str,
        attachment_id: uuid::Uuid,
    ) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM todo_attachments WHERE id = $1 AND todo_id = $2")
            .bind(attachment_id)
            .bind(todo_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn list_attachments(
        &self,
        todo_id: &str,
    ) -> Result<Vec<TodoAttachmentRow>, StorageError> {
        let rows = sqlx::query_as::<_, TodoAttachmentRow>(
            "SELECT * FROM todo_attachments WHERE todo_id = $1 ORDER BY created_at",
        )
        .bind(todo_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn add_time_entry(
        &self,
        todo_id: &str,
        source: &str,
        started_at: DateTime<Utc>,
        duration_secs: Option<i64>,
        note: Option<&str>,
    ) -> Result<TodoTimeEntryRow, StorageError> {
        let ended_at = duration_secs.map(|d| started_at + chrono::Duration::seconds(d));
        let row = sqlx::query_as::<_, TodoTimeEntryRow>(
            r#"
            INSERT INTO todo_time_entries (todo_id, source, started_at, ended_at, duration_secs, note)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING *
            "#,
        )
        .bind(todo_id)
        .bind(source)
        .bind(started_at)
        .bind(ended_at)
        .bind(duration_secs)
        .bind(note)
        .fetch_one(&self.pool)
        .await?;

        if let Some(secs) = duration_secs {
            sqlx::query(
                "UPDATE todos SET total_tracked_secs = total_tracked_secs + $2, updated_at = now() WHERE id = $1",
            )
            .bind(todo_id)
            .bind(secs)
            .execute(&self.pool)
            .await?;
        }

        Ok(row)
    }

    pub async fn close_time_entry(
        &self,
        todo_id: &str,
        entry_id: uuid::Uuid,
    ) -> Result<TodoTimeEntryRow, StorageError> {
        let row = sqlx::query_as::<_, TodoTimeEntryRow>(
            r#"
            UPDATE todo_time_entries
            SET ended_at = now(),
                duration_secs = EXTRACT(EPOCH FROM (now() - started_at))::BIGINT
            WHERE id = $1 AND todo_id = $2 AND ended_at IS NULL
            RETURNING *
            "#,
        )
        .bind(entry_id)
        .bind(todo_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| {
            StorageError::NotFound(format!("open time entry {entry_id} for todo {todo_id}"))
        })?;

        if let Some(secs) = row.duration_secs {
            sqlx::query(
                "UPDATE todos SET total_tracked_secs = total_tracked_secs + $2, updated_at = now() WHERE id = $1",
            )
            .bind(todo_id)
            .bind(secs)
            .execute(&self.pool)
            .await?;
        }

        Ok(row)
    }

    pub async fn list_time_entries(
        &self,
        todo_id: &str,
    ) -> Result<Vec<TodoTimeEntryRow>, StorageError> {
        let rows = sqlx::query_as::<_, TodoTimeEntryRow>(
            "SELECT * FROM todo_time_entries WHERE todo_id = $1 ORDER BY started_at",
        )
        .bind(todo_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_children(&self, parent_id: &str) -> Result<Vec<TodoRow>, StorageError> {
        let rows = sqlx::query_as::<_, TodoRow>(
            "SELECT * FROM todos WHERE parent_id = $1 ORDER BY created_at",
        )
        .bind(parent_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_subtree(&self, root_id: &str) -> Result<Vec<TodoRow>, StorageError> {
        let rows = sqlx::query_as::<_, TodoRow>(
            r#"
            WITH RECURSIVE subtree AS (
                SELECT * FROM todos WHERE id = $1
                UNION ALL
                SELECT t.* FROM todos t
                INNER JOIN subtree s ON t.parent_id = s.id
            )
            SELECT * FROM subtree ORDER BY created_at
            "#,
        )
        .bind(root_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn move_todo(
        &self,
        id: &str,
        new_parent_id: Option<&str>,
        new_project_id: Option<&str>,
    ) -> Result<TodoRow, StorageError> {
        if let Some(parent_id) = new_parent_id {
            if self.would_create_parent_cycle(id, parent_id).await? {
                return Err(StorageError::Conflict(format!(
                    "Setting parent {parent_id} for todo {id} would create a cycle"
                )));
            }
        }

        let row = sqlx::query_as::<_, TodoRow>(
            r#"
            UPDATE todos
            SET parent_id = $2, project_id = $3, updated_at = now()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(new_parent_id)
        .bind(new_project_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| StorageError::NotFound(format!("todo {id}")))?;

        Ok(row)
    }

    async fn would_create_parent_cycle(
        &self,
        child_id: &str,
        new_parent_id: &str,
    ) -> Result<bool, StorageError> {
        if child_id == new_parent_id {
            return Ok(true);
        }

        let row: Option<(bool,)> = sqlx::query_as(
            r#"
            WITH RECURSIVE ancestors AS (
                SELECT parent_id FROM todos WHERE id = $2
                UNION ALL
                SELECT t.parent_id FROM todos t
                INNER JOIN ancestors a ON t.id = a.parent_id
                WHERE a.parent_id IS NOT NULL
            )
            SELECT EXISTS(SELECT 1 FROM ancestors WHERE parent_id = $1)
            "#,
        )
        .bind(child_id)
        .bind(new_parent_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.0).unwrap_or(false))
    }

    pub async fn cascade_complete(&self, root_id: &str) -> Result<u64, StorageError> {
        let result = sqlx::query(
            r#"
            WITH RECURSIVE subtree AS (
                SELECT id FROM todos WHERE id = $1
                UNION ALL
                SELECT t.id FROM todos t
                INNER JOIN subtree s ON t.parent_id = s.id
            )
            UPDATE todos SET status = 'done', completed_at = now(), updated_at = now()
            WHERE id IN (SELECT id FROM subtree) AND status != 'done'
            "#,
        )
        .bind(root_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn summary(&self) -> Result<TodoSummary, StorageError> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            r#"
            SELECT status, COUNT(*) as count
            FROM todos
            WHERE is_template = FALSE
            GROUP BY status
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let mut summary = TodoSummary::default();
        for (status, count) in &rows {
            match status.as_str() {
                "todo" => summary.todo = *count,
                "doing" => summary.doing = *count,
                "done" => summary.done = *count,
                _ => {}
            }
            summary.total += count;
        }
        Ok(summary)
    }

    pub async fn overdue(&self) -> Result<Vec<TodoRow>, StorageError> {
        let rows = sqlx::query_as::<_, TodoRow>(
            r#"
            SELECT * FROM todos
            WHERE due_date < now()
              AND status != 'done'
              AND is_template = FALSE
            ORDER BY due_date
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn to_context_string(&self) -> Result<String, StorageError> {
        let rows = sqlx::query_as::<_, TodoRow>(
            r#"
            SELECT * FROM todos
            WHERE status IN ('todo', 'doing')
              AND is_template = FALSE
            ORDER BY
                CASE WHEN focused_at IS NOT NULL THEN 0 ELSE 1 END,
                priority ASC NULLS LAST,
                created_at
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        if rows.is_empty() {
            return Ok("No active tasks.".to_string());
        }

        let mut out = String::from("Active tasks:\n");
        for row in &rows {
            let focus_marker = if row.focused_at.is_some() {
                " [FOCUSED]"
            } else {
                ""
            };
            let priority_str = row.priority.map(|p| format!(" P{p}")).unwrap_or_default();
            out.push_str(&format!(
                "- [{}]{}{} {}\n",
                row.status, focus_marker, priority_str, row.title
            ));
        }
        Ok(out)
    }

    pub async fn add_template(&self, row: &TodoRow) -> Result<TodoRow, StorageError> {
        self.add(row).await
    }

    pub async fn delete_template(&self, id: &str) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM todos WHERE id = $1 AND is_template = TRUE")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
