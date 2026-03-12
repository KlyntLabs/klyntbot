//! List and search operations for the `actions` table.

use crate::error::StorageError;
use crate::rows::action::ActionRow;

use super::{ActionFilter, ActionRepo};

impl ActionRepo {
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

        if filter.root_only {
            qb.push(" AND parent_id IS NULL");
        }

        if let Some(pmin) = filter.priority_min {
            qb.push(" AND priority >= ");
            qb.push_bind(pmin);
        }

        if let Some(ref after) = filter.due_after {
            qb.push(" AND due_date >= ");
            qb.push_bind(after);
        }

        if let Some(ref before) = filter.due_before {
            qb.push(" AND due_date < ");
            qb.push_bind(before);
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
}
