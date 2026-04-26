//! Back-end for `mirror.get_coding_alerts` MCP tool.

use common::{KlyntbotError, Result};
use serde::{Deserialize, Serialize};

/// Filter args.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodingAlertFilter {
    /// `CodingMirrorAlertKind` string form.
    pub kind: Option<String>,
    /// `MirrorAlertSeverity` string form.
    pub severity: Option<String>,
    /// Repo id (matched against payload).
    pub repo: Option<String>,
    /// Max rows.
    pub limit: u32,
}

/// One row.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodingAlertRow {
    /// Snippet id.
    pub id: String,
    /// Headline.
    pub headline: String,
    /// JSON payload.
    pub payload: String,
    /// Kind.
    pub kind: String,
    /// Severity.
    pub severity: String,
    /// When created.
    pub created_at: String,
    /// Whether dismissed.
    pub dismissed: bool,
}

/// Query.
#[derive(Debug, Clone)]
pub struct CodingAlertsQuery {
    pool: storage::StoragePool,
}

impl CodingAlertsQuery {
    /// Construct.
    pub fn new(pool: storage::StoragePool) -> Self {
        Self { pool }
    }

    /// Run.
    pub async fn query(&self, filter: &CodingAlertFilter) -> Result<Vec<CodingAlertRow>> {
        let limit = if filter.limit == 0 { 50 } else { filter.limit };
        let mut sql = String::from(
            "SELECT id, headline, body, COALESCE(coding_alert_kind, ''), \
                    COALESCE(coding_alert_severity, ''), created_at, \
                    CASE WHEN dismissed_at IS NOT NULL THEN 1 ELSE 0 END \
             FROM mirror_snippets WHERE coding_alert_kind IS NOT NULL",
        );
        let mut binds: Vec<String> = Vec::new();
        if let Some(k) = filter.kind.as_ref() {
            sql.push_str(" AND coding_alert_kind = ?");
            binds.push(k.clone());
        }
        if let Some(s) = filter.severity.as_ref() {
            sql.push_str(" AND coding_alert_severity = ?");
            binds.push(s.clone());
        }
        if let Some(repo) = filter.repo.as_ref() {
            sql.push_str(" AND body LIKE ?");
            binds.push(format!("%{repo}%"));
        }
        sql.push_str(&format!(" ORDER BY created_at DESC LIMIT {limit}"));

        let mut q =
            sqlx::query_as::<_, (String, String, String, String, String, String, i64)>(&sql);
        for b in &binds {
            q = q.bind(b);
        }
        let rows = q
            .fetch_all(self.pool.inner())
            .await
            .map_err(|e| KlyntbotError::Storage(format!("coding_alerts query: {e}")))?;
        Ok(rows
            .into_iter()
            .map(
                |(id, headline, payload, kind, severity, created_at, dismissed)| CodingAlertRow {
                    id,
                    headline,
                    payload,
                    kind,
                    severity,
                    created_at,
                    dismissed: dismissed != 0,
                },
            )
            .collect())
    }
}
