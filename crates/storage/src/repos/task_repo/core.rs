//! CRUD operations and listing/filtering for tasks.

use super::{TaskFilter, TaskPatch, TaskRepo};
use crate::error::{OptionExt, StorageError};
use crate::rows::task::TaskRow;

impl TaskRepo {
    /// Insert a new task. Returns the inserted row.
    pub async fn add(&self, row: &TaskRow) -> Result<TaskRow, StorageError> {
        let inserted = sqlx::query_as::<_, TaskRow>(
            r#"
            INSERT INTO tasks (
                id, title, description, area_id, project_id, key_result_id,
                parent_id, priority, due_date, tags, status,
                focused_at, focus_deadline, focus_expired_count,
                created_at, updated_at, completed_at,
                total_tracked_secs, estimated_minutes,
                calendar_event_uid, last_reminded_at,
                recurrence_rule, recurrence_parent_id, is_template, next_instance_date,
                status_label_id, position, group_id,
                task_type,
                energy_level, estimated_focus_blocks, actual_minutes,
                complexity_score, completed, objective_id,
                scheduled_start, scheduled_end
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6,
                ?7, ?8, ?9, ?10, ?11,
                ?12, ?13, ?14,
                ?15, ?16, ?17,
                ?18, ?19,
                ?20, ?21,
                ?22, ?23, ?24, ?25,
                ?26, ?27, ?28,
                ?29,
                ?30, ?31, ?32,
                ?33, ?34, ?35,
                ?36, ?37
            )
            RETURNING *
            "#,
        )
        .bind(&row.id)
        .bind(&row.title)
        .bind(&row.description)
        .bind(&row.area_id)
        .bind(&row.project_id)
        .bind(&row.key_result_id)
        .bind(&row.parent_id)
        .bind(row.priority)
        .bind(row.due_date)
        .bind(sqlx::types::Json(&row.tags))
        .bind(&row.status)
        .bind(row.focused_at)
        .bind(row.focus_deadline)
        .bind(row.focus_expired_count)
        .bind(row.created_at)
        .bind(row.updated_at)
        .bind(row.completed_at)
        .bind(row.total_tracked_secs)
        .bind(row.estimated_minutes)
        .bind(&row.calendar_event_uid)
        .bind(row.last_reminded_at)
        .bind(&row.recurrence_rule)
        .bind(&row.recurrence_parent_id)
        .bind(row.is_template)
        .bind(row.next_instance_date)
        .bind(&row.status_label_id)
        .bind(row.position)
        .bind(&row.group_id)
        .bind(&row.task_type)
        .bind(&row.energy_level)
        .bind(row.estimated_focus_blocks)
        .bind(row.actual_minutes)
        .bind(row.complexity_score)
        .bind(row.completed)
        .bind(&row.objective_id)
        .bind(row.scheduled_start)
        .bind(row.scheduled_end)
        .fetch_one(&self.pool)
        .await?;

        Ok(inserted)
    }

    /// Get a single task by id. Returns `None` if not found.
    pub async fn get(&self, id: &str) -> Result<Option<TaskRow>, StorageError> {
        let row = sqlx::query_as::<_, TaskRow>("SELECT * FROM tasks WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    /// Get a single task by id, returning `StorageError::NotFound` if missing.
    pub async fn get_or_err(&self, id: &str) -> Result<TaskRow, StorageError> {
        self.get(id).await?.ok_or_not_found(&format!("task {id}"))
    }

    get_by_ids_impl!("tasks", TaskRow);

    /// Update mutable fields on a task. Only non-None fields are overwritten.
    pub async fn update(&self, patch: &TaskPatch) -> Result<TaskRow, StorageError> {
        let row = sqlx::query_as::<_, TaskRow>(
            r#"
            UPDATE tasks SET
                title              = COALESCE(?2, title),
                description        = CASE WHEN ?3 THEN ?4 ELSE description END,
                priority           = CASE WHEN ?5 THEN ?6 ELSE priority END,
                due_date           = CASE WHEN ?7 THEN ?8 ELSE due_date END,
                tags               = COALESCE(?9, tags),
                status             = COALESCE(?10, status),
                completed_at       = CASE
                    WHEN ?10 = 'done' AND completed_at IS NULL THEN (unixepoch('now') * 1000)
                    WHEN ?10 IS NOT NULL AND ?10 != 'done' THEN NULL
                    ELSE completed_at
                END,
                calendar_event_uid = CASE WHEN ?11 THEN ?12 ELSE calendar_event_uid END,
                next_instance_date = CASE WHEN ?13 THEN ?14 ELSE next_instance_date END,
                last_reminded_at   = CASE WHEN ?15 THEN ?16 ELSE last_reminded_at END,
                estimated_minutes  = CASE WHEN ?17 THEN ?18 ELSE estimated_minutes END,
                recurrence_rule    = CASE WHEN ?19 THEN ?20 ELSE recurrence_rule END,
                area_id            = COALESCE(?21, area_id),
                project_id         = CASE WHEN ?22 THEN ?23 ELSE project_id END,
                key_result_id      = CASE WHEN ?24 THEN ?25 ELSE key_result_id END,
                status_label_id    = CASE WHEN ?26 THEN ?27 ELSE status_label_id END,
                position           = COALESCE(?28, position),
                group_id           = CASE WHEN ?29 THEN ?30 ELSE group_id END,
                task_type          = COALESCE(?31, task_type),
                energy_level       = CASE WHEN ?32 THEN ?33 ELSE energy_level END,
                complexity_score   = CASE WHEN ?34 THEN ?35 ELSE complexity_score END,
                completed          = COALESCE(?36, completed),
                actual_minutes     = CASE WHEN ?37 THEN ?38 ELSE actual_minutes END,
                objective_id       = CASE WHEN ?39 THEN ?40 ELSE objective_id END,
                scheduled_start    = CASE WHEN ?41 THEN ?42 ELSE scheduled_start END,
                scheduled_end      = CASE WHEN ?43 THEN ?44 ELSE scheduled_end END,
                updated_at         = (unixepoch('now') * 1000)
            WHERE id = ?1
            RETURNING *
            "#,
        )
        .bind(&patch.id) // ?1
        .bind(&patch.title) // ?2
        .bind(patch.description.is_some()) // ?3
        .bind(patch.description.as_ref().and_then(|d| d.as_deref())) // ?4
        .bind(patch.priority.is_some()) // ?5
        .bind(patch.priority.unwrap_or_default()) // ?6
        .bind(patch.due_date.is_some()) // ?7
        .bind(
            patch
                .due_date
                .unwrap_or_default()
                .map(|t| t.as_millisecond()),
        ) // ?8
        .bind(patch.tags.as_ref().map(sqlx::types::Json)) // ?9
        .bind(&patch.status) // ?10
        .bind(patch.calendar_event_uid.is_some()) // ?11
        .bind(patch.calendar_event_uid.as_ref().and_then(|v| v.as_deref())) // ?12
        .bind(patch.next_instance_date.is_some()) // ?13
        .bind(
            patch
                .next_instance_date
                .unwrap_or_default()
                .map(|t| t.as_millisecond()),
        ) // ?14
        .bind(patch.last_reminded_at.is_some()) // ?15
        .bind(
            patch
                .last_reminded_at
                .unwrap_or_default()
                .map(|t| t.as_millisecond()),
        ) // ?16
        .bind(patch.estimated_minutes.is_some()) // ?17
        .bind(patch.estimated_minutes.unwrap_or_default()) // ?18
        .bind(patch.recurrence_rule.is_some()) // ?19
        .bind(patch.recurrence_rule.as_ref().and_then(|v| v.as_deref())) // ?20
        .bind(&patch.area_id) // ?21
        .bind(patch.project_id.is_some()) // ?22
        .bind(patch.project_id.as_ref().and_then(|v| v.as_deref())) // ?23
        .bind(patch.key_result_id.is_some()) // ?24
        .bind(patch.key_result_id.as_ref().and_then(|v| v.as_deref())) // ?25
        .bind(patch.status_label_id.is_some()) // ?26
        .bind(patch.status_label_id.as_ref().and_then(|v| v.as_deref())) // ?27
        .bind(patch.position) // ?28
        .bind(patch.group_id.is_some()) // ?29
        .bind(patch.group_id.as_ref().and_then(|v| v.as_deref())) // ?30
        .bind(&patch.task_type) // ?31
        .bind(patch.energy_level.is_some()) // ?32
        .bind(patch.energy_level.as_ref().and_then(|v| v.as_deref())) // ?33
        .bind(patch.complexity_score.is_some()) // ?34
        .bind(patch.complexity_score.unwrap_or_default()) // ?35
        .bind(patch.completed.map(|b| b as i32)) // ?36
        .bind(patch.actual_minutes.is_some()) // ?37
        .bind(patch.actual_minutes.unwrap_or_default()) // ?38
        .bind(patch.objective_id.is_some()) // ?39
        .bind(patch.objective_id.as_ref().and_then(|v| v.as_deref())) // ?40
        .bind(patch.scheduled_start.is_some()) // ?41
        .bind(
            patch
                .scheduled_start
                .unwrap_or_default()
                .map(|t| t.as_millisecond()),
        ) // ?42
        .bind(patch.scheduled_end.is_some()) // ?43
        .bind(
            patch
                .scheduled_end
                .unwrap_or_default()
                .map(|t| t.as_millisecond()),
        ) // ?44
        .fetch_optional(&self.pool)
        .await?
        .ok_or_not_found(&format!("task {}", patch.id))?;

        Ok(row)
    }

    /// Delete a task and all its cascade dependents.
    pub async fn delete(&self, id: &str) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM tasks WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// List tasks matching the given filter criteria.
    pub async fn list(&self, filter: &TaskFilter) -> Result<Vec<TaskRow>, StorageError> {
        let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new("SELECT * FROM tasks WHERE ");

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
            for tag in tags {
                qb.push(" AND EXISTS (SELECT 1 FROM json_each(tags) WHERE value = ");
                qb.push_bind(tag);
                qb.push(")");
            }
        }

        if let Some(ref area_id) = filter.area_id {
            qb.push(" AND area_id = ");
            qb.push_bind(area_id);
        }

        if let Some(ref project_id) = filter.project_id {
            qb.push(" AND project_id = ");
            qb.push_bind(project_id);
        }

        if let Some(ref kr_id) = filter.key_result_id {
            qb.push(" AND key_result_id = ");
            qb.push_bind(kr_id);
        }

        if filter.unassigned {
            qb.push(" AND project_id IS NULL");
        }

        if filter.root_only {
            qb.push(" AND parent_id IS NULL");
        }

        if let Some(pmin) = filter.priority_min {
            qb.push(" AND priority >= ");
            qb.push_bind(pmin);
        }

        if let Some(ref after) = filter.due_after {
            qb.push(" AND due_date >= ");
            qb.push_bind(after.as_millisecond());
        }

        if let Some(ref before) = filter.due_before {
            qb.push(" AND due_date < ");
            qb.push_bind(before.as_millisecond());
        }

        if let Some(ref group) = filter.status_group {
            qb.push(" AND status_label_id IN (SELECT id FROM status_labels WHERE status_group = ");
            qb.push_bind(group);
            qb.push(")");
        }

        if let Some(ref gid) = filter.group_id {
            qb.push(" AND group_id = ");
            qb.push_bind(gid);
        }

        if let Some(ref tt) = filter.task_type {
            qb.push(" AND task_type = ");
            qb.push_bind(tt);
        }

        if let Some(ref el) = filter.energy_level {
            qb.push(" AND energy_level = ");
            qb.push_bind(el);
        }

        if let Some(completed) = filter.completed {
            qb.push(" AND completed = ");
            qb.push_bind(completed as i32);
        }

        qb.push(" ORDER BY created_at DESC");

        if let Some(limit) = filter.limit {
            qb.push(" LIMIT ");
            qb.push_bind(limit);
        }

        let rows = qb.build_query_as::<TaskRow>().fetch_all(&self.pool).await?;
        Ok(rows)
    }

    /// List all templates (is_template = true).
    pub async fn list_templates(&self) -> Result<Vec<TaskRow>, StorageError> {
        self.list(&TaskFilter {
            templates_only: true,
            ..Default::default()
        })
        .await
    }

    /// Search tasks by keyword in title or description (case-insensitive).
    pub async fn search_by_keyword(
        &self,
        query: &str,
        limit: Option<i64>,
    ) -> Result<Vec<TaskRow>, StorageError> {
        let pattern = format!("%{}%", crate::macros::escape_like(query));
        let effective_limit = limit.unwrap_or(i64::MAX);
        let rows = sqlx::query_as::<_, TaskRow>(
            r#"
            SELECT * FROM tasks
            WHERE is_template = FALSE
              AND (title LIKE ?1 ESCAPE '\' OR description LIKE ?1 ESCAPE '\')
            ORDER BY created_at DESC
            LIMIT ?2
            "#,
        )
        .bind(&pattern)
        .bind(effective_limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}
