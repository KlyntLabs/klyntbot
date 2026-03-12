//! CRUD operations for the `actions` table.

use crate::error::{OptionExt, StorageError};
use crate::rows::action::ActionRow;

use super::{ActionPatch, ActionRepo};

impl ActionRepo {
    /// Insert a new action. Returns the inserted row.
    pub async fn add(&self, row: &ActionRow) -> Result<ActionRow, StorageError> {
        let inserted = sqlx::query_as::<_, ActionRow>(
            r#"
            INSERT INTO actions (
                id, title, description, area_id, project_id, key_result_id,
                parent_id, priority, due_date, tags, status,
                focused_at, focus_deadline, focus_expired_count,
                created_at, updated_at, completed_at,
                total_tracked_secs, estimated_minutes,
                calendar_event_uid, last_reminded_at,
                recurrence_rule, recurrence_parent_id, is_template, next_instance_date,
                status_label_id, position, group_id
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6,
                ?7, ?8, ?9, ?10, ?11,
                ?12, ?13, ?14,
                ?15, ?16, ?17,
                ?18, ?19,
                ?20, ?21,
                ?22, ?23, ?24, ?25,
                ?26, ?27, ?28
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
        .fetch_one(&self.pool)
        .await?;

        Ok(inserted)
    }

    /// Get a single action by id. Returns `None` if not found.
    pub async fn get(&self, id: &str) -> Result<Option<ActionRow>, StorageError> {
        let row = sqlx::query_as::<_, ActionRow>("SELECT * FROM actions WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    /// Get a single action by id, returning `StorageError::NotFound` if missing.
    pub async fn get_or_err(&self, id: &str) -> Result<ActionRow, StorageError> {
        self.get(id).await?.ok_or_not_found(&format!("action {id}"))
    }

    /// Fetch actions by a list of IDs. Missing IDs are silently skipped.
    pub async fn get_by_ids(&self, ids: &[String]) -> Result<Vec<ActionRow>, StorageError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new("SELECT * FROM actions WHERE id IN (");
        let mut sep = qb.separated(", ");
        for id in ids {
            sep.push_bind(id);
        }
        qb.push(")");
        let rows = qb
            .build_query_as::<ActionRow>()
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// Update mutable fields on an action. Only non-None fields are overwritten.
    pub async fn update(&self, patch: &ActionPatch) -> Result<ActionRow, StorageError> {
        let row = sqlx::query_as::<_, ActionRow>(
            r#"
            UPDATE actions SET
                title              = COALESCE(?2, title),
                description        = CASE WHEN ?3 THEN ?4 ELSE description END,
                priority           = CASE WHEN ?5 THEN ?6 ELSE priority END,
                due_date           = CASE WHEN ?7 THEN ?8 ELSE due_date END,
                tags               = COALESCE(?9, tags),
                status             = COALESCE(?10, status),
                completed_at       = CASE
                    WHEN ?10 = 'done' AND completed_at IS NULL THEN datetime('now')
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
                updated_at         = datetime('now')
            WHERE id = ?1
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
        .bind(patch.tags.as_ref().map(sqlx::types::Json))
        .bind(&patch.status)
        .bind(patch.calendar_event_uid.is_some())
        .bind(patch.calendar_event_uid.as_ref().and_then(|v| v.as_deref()))
        .bind(patch.next_instance_date.is_some())
        .bind(patch.next_instance_date.unwrap_or_default())
        .bind(patch.last_reminded_at.is_some())
        .bind(patch.last_reminded_at.unwrap_or_default())
        .bind(patch.estimated_minutes.is_some())
        .bind(patch.estimated_minutes.unwrap_or_default())
        .bind(patch.recurrence_rule.is_some())
        .bind(patch.recurrence_rule.as_ref().and_then(|v| v.as_deref()))
        .bind(&patch.area_id)
        .bind(patch.project_id.is_some())
        .bind(patch.project_id.as_ref().and_then(|v| v.as_deref()))
        .bind(patch.key_result_id.is_some())
        .bind(patch.key_result_id.as_ref().and_then(|v| v.as_deref()))
        .bind(patch.status_label_id.is_some())
        .bind(patch.status_label_id.as_ref().and_then(|v| v.as_deref()))
        .bind(patch.position)
        .bind(patch.group_id.is_some())
        .bind(patch.group_id.as_ref().and_then(|v| v.as_deref()))
        .fetch_optional(&self.pool)
        .await?
        .ok_or_not_found(&format!("action {}", patch.id))?;

        Ok(row)
    }

    /// Delete an action and all its cascade dependents.
    pub async fn delete(&self, id: &str) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM actions WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
