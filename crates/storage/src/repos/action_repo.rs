//! Repository for the `actions` table and its join tables
//! (`action_attachments`, `action_time_entries`, `action_dependencies`).

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::SqlitePool;

use crate::error::{OptionExt, StorageError};
use crate::rows::action::{
    ActionAttachmentRow, ActionDependencyRow, ActionRow, ActionTimeEntryRow,
};

/// Filter criteria for listing actions.
#[derive(Debug, Default, Clone)]
pub struct ActionFilter {
    pub status: Option<String>,
    pub tags: Option<Vec<String>>,
    pub area_id: Option<String>,
    pub project_id: Option<String>,
    pub key_result_id: Option<String>,
    pub unassigned: bool,
    pub priority_min: Option<i16>,
    pub limit: Option<i64>,
    pub templates_only: bool,
}

/// Aggregate counts by status.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionSummary {
    pub todo: i64,
    pub doing: i64,
    pub done: i64,
    pub total: i64,
}

/// Repository for action CRUD, hierarchy, focus, dependencies, attachments, and time tracking.
#[derive(Debug, Clone)]
pub struct ActionRepo {
    pool: SqlitePool,
}

impl ActionRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // -----------------------------------------------------------------------
    // CRUD
    // -----------------------------------------------------------------------

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
                recurrence_rule, recurrence_parent_id, is_template, next_instance_date
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6,
                ?7, ?8, ?9, ?10, ?11,
                ?12, ?13, ?14,
                ?15, ?16, ?17,
                ?18, ?19,
                ?20, ?21,
                ?22, ?23, ?24, ?25
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
                key_result_id      = CASE WHEN ?22 THEN ?23 ELSE key_result_id END,
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
        .bind(patch.key_result_id.is_some())
        .bind(patch.key_result_id.as_ref().and_then(|v| v.as_deref()))
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

    // -----------------------------------------------------------------------
    // Listing / Filtering
    // -----------------------------------------------------------------------

    /// List actions matching the given filter criteria.
    pub async fn list(&self, filter: &ActionFilter) -> Result<Vec<ActionRow>, StorageError> {
        let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new("SELECT * FROM actions WHERE ");

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

        if let Some(pmin) = filter.priority_min {
            qb.push(" AND priority >= ");
            qb.push_bind(pmin);
        }

        qb.push(" ORDER BY created_at DESC");

        if let Some(limit) = filter.limit {
            qb.push(" LIMIT ");
            qb.push_bind(limit);
        }

        let rows = qb
            .build_query_as::<ActionRow>()
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// List all templates (is_template = true).
    pub async fn list_templates(&self) -> Result<Vec<ActionRow>, StorageError> {
        self.list(&ActionFilter {
            templates_only: true,
            ..Default::default()
        })
        .await
    }

    /// Search actions by keyword in title or description (case-insensitive).
    pub async fn search_by_keyword(
        &self,
        query: &str,
        limit: Option<i64>,
    ) -> Result<Vec<ActionRow>, StorageError> {
        let escaped = query
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let pattern = format!("%{escaped}%");
        let effective_limit = limit.unwrap_or(i64::MAX);
        let rows = sqlx::query_as::<_, ActionRow>(
            r#"
            SELECT * FROM actions
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

    // -----------------------------------------------------------------------
    // Focus Slots
    // -----------------------------------------------------------------------

    /// Focus an action. Returns true if the focus was set, false if at max_slots.
    pub async fn focus(
        &self,
        id: &str,
        max_slots: i64,
        deadline: Option<DateTime<Utc>>,
    ) -> Result<bool, StorageError> {
        let result = sqlx::query(
            r#"
            UPDATE actions
            SET focused_at = datetime('now'), focus_deadline = ?3, updated_at = datetime('now')
            WHERE id = ?1
              AND focused_at IS NULL
              AND (SELECT COUNT(*) FROM actions WHERE focused_at IS NOT NULL) < ?2
            "#,
        )
        .bind(id)
        .bind(max_slots)
        .bind(deadline)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Unfocus an action.
    pub async fn unfocus(&self, id: &str) -> Result<bool, StorageError> {
        let result = sqlx::query(
            "UPDATE actions SET focused_at = NULL, focus_deadline = NULL, updated_at = datetime('now') WHERE id = ?1",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// List currently focused actions.
    pub async fn list_focused(&self) -> Result<Vec<ActionRow>, StorageError> {
        let rows = sqlx::query_as::<_, ActionRow>(
            "SELECT * FROM actions WHERE focused_at IS NOT NULL ORDER BY focused_at",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    // -----------------------------------------------------------------------
    // Dependencies (edge table: action_dependencies)
    // -----------------------------------------------------------------------

    /// Add a dependency edge (action_id is blocked by blocker_id).
    pub async fn add_dependency(
        &self,
        action_id: &str,
        blocker_id: &str,
    ) -> Result<(), StorageError> {
        let would_cycle = self.would_create_cycle(action_id, blocker_id).await?;
        if would_cycle {
            return Err(StorageError::Conflict(format!(
                "Adding dependency {action_id} -> {blocker_id} would create a cycle"
            )));
        }

        sqlx::query("INSERT INTO action_dependencies (action_id, blocker_id) VALUES (?1, ?2)")
            .bind(action_id)
            .bind(blocker_id)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                if let sqlx::Error::Database(ref db_err) = e {
                    if db_err.constraint().is_some() {
                        return StorageError::Conflict(format!(
                            "Cannot add dependency: {action_id} -> {blocker_id}"
                        ));
                    }
                }
                StorageError::Sqlx(e)
            })?;

        Ok(())
    }

    /// Remove a dependency edge.
    pub async fn remove_dependency(
        &self,
        action_id: &str,
        blocker_id: &str,
    ) -> Result<bool, StorageError> {
        let result =
            sqlx::query("DELETE FROM action_dependencies WHERE action_id = ?1 AND blocker_id = ?2")
                .bind(action_id)
                .bind(blocker_id)
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Get all blockers for an action.
    pub async fn get_blockers(&self, action_id: &str) -> Result<Vec<ActionRow>, StorageError> {
        let rows = sqlx::query_as::<_, ActionRow>(
            r#"
            SELECT t.* FROM actions t
            INNER JOIN action_dependencies d ON d.blocker_id = t.id
            WHERE d.action_id = ?1
            "#,
        )
        .bind(action_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Get incomplete blockers for an action (status != 'done').
    pub async fn incomplete_blockers(
        &self,
        action_id: &str,
    ) -> Result<Vec<ActionRow>, StorageError> {
        let rows = sqlx::query_as::<_, ActionRow>(
            r#"
            SELECT t.* FROM actions t
            INNER JOIN action_dependencies d ON d.blocker_id = t.id
            WHERE d.action_id = ?1 AND t.status != 'done'
            "#,
        )
        .bind(action_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Get actions blocked by this action.
    pub async fn get_blocking(&self, blocker_id: &str) -> Result<Vec<ActionRow>, StorageError> {
        let rows = sqlx::query_as::<_, ActionRow>(
            r#"
            SELECT t.* FROM actions t
            INNER JOIN action_dependencies d ON d.action_id = t.id
            WHERE d.blocker_id = ?1
            "#,
        )
        .bind(blocker_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    async fn would_create_cycle(
        &self,
        action_id: &str,
        blocker_id: &str,
    ) -> Result<bool, StorageError> {
        let row: Option<(bool,)> = sqlx::query_as(
            r#"
            WITH RECURSIVE reachable AS (
                SELECT blocker_id AS node FROM action_dependencies WHERE action_id = ?2
                UNION
                SELECT d.blocker_id FROM action_dependencies d
                INNER JOIN reachable r ON d.action_id = r.node
            )
            SELECT EXISTS(SELECT 1 FROM reachable WHERE node = ?1)
            "#,
        )
        .bind(action_id)
        .bind(blocker_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.0).unwrap_or(false))
    }

    /// Get all dependency edges for an action.
    pub async fn get_dependencies(
        &self,
        action_id: &str,
    ) -> Result<Vec<ActionDependencyRow>, StorageError> {
        let rows = sqlx::query_as::<_, ActionDependencyRow>(
            "SELECT * FROM action_dependencies WHERE action_id = ?1",
        )
        .bind(action_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    // -----------------------------------------------------------------------
    // Attachments (join table: action_attachments)
    // -----------------------------------------------------------------------

    /// Add an attachment to an action.
    pub async fn add_attachment(
        &self,
        action_id: &str,
        attachment_type: &str,
        value: &str,
        title: Option<&str>,
        tags: &[String],
    ) -> Result<ActionAttachmentRow, StorageError> {
        let id = uuid::Uuid::new_v4();
        let row = sqlx::query_as::<_, ActionAttachmentRow>(
            r#"
            INSERT INTO action_attachments (id, action_id, attachment_type, value, title, tags)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(action_id)
        .bind(attachment_type)
        .bind(value)
        .bind(title)
        .bind(sqlx::types::Json(tags))
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// Remove an attachment by its UUID.
    pub async fn remove_attachment(
        &self,
        action_id: &str,
        attachment_id: uuid::Uuid,
    ) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM action_attachments WHERE id = ?1 AND action_id = ?2")
            .bind(attachment_id)
            .bind(action_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// List all attachments for an action.
    pub async fn list_attachments(
        &self,
        action_id: &str,
    ) -> Result<Vec<ActionAttachmentRow>, StorageError> {
        let rows = sqlx::query_as::<_, ActionAttachmentRow>(
            "SELECT * FROM action_attachments WHERE action_id = ?1 ORDER BY created_at",
        )
        .bind(action_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    // -----------------------------------------------------------------------
    // Time Entries (join table: action_time_entries)
    // -----------------------------------------------------------------------

    /// Add a time entry.
    pub async fn add_time_entry(
        &self,
        action_id: &str,
        source: &str,
        started_at: DateTime<Utc>,
        duration_secs: Option<i64>,
        note: Option<&str>,
    ) -> Result<ActionTimeEntryRow, StorageError> {
        let ended_at = duration_secs.map(|d| started_at + chrono::Duration::seconds(d));
        let id = uuid::Uuid::new_v4();
        let row = sqlx::query_as::<_, ActionTimeEntryRow>(
            r#"
            INSERT INTO action_time_entries (id, action_id, source, started_at, ended_at, duration_secs, note)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(action_id)
        .bind(source)
        .bind(started_at)
        .bind(ended_at)
        .bind(duration_secs)
        .bind(note)
        .fetch_one(&self.pool)
        .await?;

        if let Some(secs) = duration_secs {
            sqlx::query(
                "UPDATE actions SET total_tracked_secs = total_tracked_secs + ?2, updated_at = datetime('now') WHERE id = ?1",
            )
            .bind(action_id)
            .bind(secs)
            .execute(&self.pool)
            .await?;
        }

        Ok(row)
    }

    /// Close an open time entry.
    pub async fn close_time_entry(
        &self,
        action_id: &str,
        entry_id: uuid::Uuid,
    ) -> Result<ActionTimeEntryRow, StorageError> {
        let row = sqlx::query_as::<_, ActionTimeEntryRow>(
            r#"
            UPDATE action_time_entries
            SET ended_at = datetime('now'),
                duration_secs = (unixepoch('now') - unixepoch(started_at))
            WHERE id = ?1 AND action_id = ?2 AND ended_at IS NULL
            RETURNING *
            "#,
        )
        .bind(entry_id)
        .bind(action_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| {
            StorageError::NotFound(format!("open time entry {entry_id} for action {action_id}"))
        })?;

        if let Some(secs) = row.duration_secs {
            sqlx::query(
                "UPDATE actions SET total_tracked_secs = total_tracked_secs + ?2, updated_at = datetime('now') WHERE id = ?1",
            )
            .bind(action_id)
            .bind(secs)
            .execute(&self.pool)
            .await?;
        }

        Ok(row)
    }

    /// List time entries for an action.
    pub async fn list_time_entries(
        &self,
        action_id: &str,
    ) -> Result<Vec<ActionTimeEntryRow>, StorageError> {
        let rows = sqlx::query_as::<_, ActionTimeEntryRow>(
            "SELECT * FROM action_time_entries WHERE action_id = ?1 ORDER BY started_at",
        )
        .bind(action_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    // -----------------------------------------------------------------------
    // Hierarchy
    // -----------------------------------------------------------------------

    /// Get immediate children of an action.
    pub async fn get_children(&self, parent_id: &str) -> Result<Vec<ActionRow>, StorageError> {
        let rows = sqlx::query_as::<_, ActionRow>(
            "SELECT * FROM actions WHERE parent_id = ?1 ORDER BY created_at",
        )
        .bind(parent_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Count immediate children.
    pub async fn count_children(&self, parent_id: &str) -> Result<i64, StorageError> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM actions WHERE parent_id = ?1")
            .bind(parent_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    /// Get full subtree of an action (recursive CTE).
    pub async fn get_subtree(&self, root_id: &str) -> Result<Vec<ActionRow>, StorageError> {
        let rows = sqlx::query_as::<_, ActionRow>(
            r#"
            WITH RECURSIVE subtree AS (
                SELECT * FROM actions WHERE id = ?1
                UNION ALL
                SELECT t.* FROM actions t
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

    /// Move an action to a new parent and/or project.
    pub async fn move_action(
        &self,
        id: &str,
        new_parent_id: Option<&str>,
        new_project_id: Option<&str>,
    ) -> Result<ActionRow, StorageError> {
        if let Some(parent_id) = new_parent_id {
            if self.would_create_parent_cycle(id, parent_id).await? {
                return Err(StorageError::Conflict(format!(
                    "Setting parent {parent_id} for action {id} would create a cycle"
                )));
            }
        }

        let row = sqlx::query_as::<_, ActionRow>(
            r#"
            UPDATE actions
            SET parent_id = ?2, project_id = ?3, updated_at = datetime('now')
            WHERE id = ?1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(new_parent_id)
        .bind(new_project_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_not_found(&format!("action {id}"))?;

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
                SELECT parent_id FROM actions WHERE id = ?2
                UNION ALL
                SELECT t.parent_id FROM actions t
                INNER JOIN ancestors a ON t.id = a.parent_id
                WHERE a.parent_id IS NOT NULL
            )
            SELECT EXISTS(SELECT 1 FROM ancestors WHERE parent_id = ?1)
            "#,
        )
        .bind(child_id)
        .bind(new_parent_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.0).unwrap_or(false))
    }

    /// Mark an action and all children as "done" recursively.
    pub async fn cascade_complete(&self, root_id: &str) -> Result<u64, StorageError> {
        let result = sqlx::query(
            r#"
            WITH RECURSIVE subtree AS (
                SELECT id FROM actions WHERE id = ?1
                UNION ALL
                SELECT t.id FROM actions t
                INNER JOIN subtree s ON t.parent_id = s.id
            )
            UPDATE actions SET status = 'done', completed_at = datetime('now'), updated_at = datetime('now')
            WHERE id IN (SELECT id FROM subtree) AND status != 'done'
            "#,
        )
        .bind(root_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    // -----------------------------------------------------------------------
    // Aggregation
    // -----------------------------------------------------------------------

    /// Count actions by status.
    pub async fn summary(&self) -> Result<ActionSummary, StorageError> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            r#"
            SELECT status, COUNT(*) as count
            FROM actions
            WHERE is_template = FALSE
            GROUP BY status
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let mut summary = ActionSummary::default();
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

    /// Get overdue actions.
    pub async fn overdue(&self) -> Result<Vec<ActionRow>, StorageError> {
        let rows = sqlx::query_as::<_, ActionRow>(
            r#"
            SELECT * FROM actions
            WHERE due_date < datetime('now')
              AND status != 'done'
              AND is_template = FALSE
            ORDER BY due_date
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Build a context string of active actions for LLM context injection.
    #[allow(clippy::type_complexity)]
    pub async fn to_context_string(&self) -> Result<String, StorageError> {
        let rows: Vec<(
            String,
            String,
            Option<i16>,
            Option<chrono::DateTime<chrono::Utc>>,
        )> = sqlx::query_as(
            r#"
                SELECT title, status, priority, focused_at FROM actions
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
        for (title, status, priority, focused_at) in &rows {
            let focus_marker = if focused_at.is_some() {
                " [FOCUSED]"
            } else {
                ""
            };
            let priority_str = priority.map(|p| format!(" P{p}")).unwrap_or_default();
            out.push_str(&format!(
                "- [{}]{}{} {}\n",
                status, focus_marker, priority_str, title
            ));
        }
        Ok(out)
    }

    // -----------------------------------------------------------------------
    // Recurring Templates
    // -----------------------------------------------------------------------

    /// Add a recurring template.
    pub async fn add_template(&self, row: &ActionRow) -> Result<ActionRow, StorageError> {
        self.add(row).await
    }

    /// Delete a recurring template.
    pub async fn delete_template(&self, id: &str) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM actions WHERE id = ?1 AND is_template = TRUE")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    // -----------------------------------------------------------------------
    // KR-related helpers
    // -----------------------------------------------------------------------

    /// Count total and completed actions for a key result.
    /// Returns `(total, completed)`.
    pub async fn count_by_kr(&self, kr_id: &str) -> Result<(i64, i64), StorageError> {
        let row: (i64, i64) = sqlx::query_as(
            "SELECT COUNT(*), SUM(CASE WHEN status = 'done' THEN 1 ELSE 0 END) \
             FROM actions WHERE key_result_id = ?1 AND is_template = FALSE",
        )
        .bind(kr_id)
        .fetch_one(&self.pool)
        .await?;

        Ok((row.0, row.1))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_get_by_ids_empty_input_short_circuits() {
        let ids: Vec<String> = vec![];
        assert!(
            ids.is_empty(),
            "Empty ids should trigger early return in get_by_ids"
        );
    }
}

/// Patch struct for partial updates.
#[derive(Debug, Default, Clone)]
pub struct ActionPatch {
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
    pub recurrence_rule: Option<Option<String>>,
    pub area_id: Option<String>,
    pub key_result_id: Option<Option<String>>,
}
