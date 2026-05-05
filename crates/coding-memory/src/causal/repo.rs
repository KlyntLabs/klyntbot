//! `CausalEdgeRepo` — CRUD over `memory_causal_edges`.

use crate::scope::{CausalEdge, CausalEdgeKind};
use jiff::Timestamp;
use storage::StoragePool;
use uuid::Uuid;

/// One ≥3-edge group keyed by problem_hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProblemHashGroup {
    /// Hash key.
    pub problem_hash: String,
    /// Edge ids in the group.
    pub edge_ids: Vec<Uuid>,
}

/// Repository for `memory_causal_edges`.
#[derive(Debug, Clone)]
pub struct CausalEdgeRepo {
    pool: StoragePool,
}

impl CausalEdgeRepo {
    /// Construct.
    #[must_use]
    pub fn new(pool: StoragePool) -> Self {
        Self { pool }
    }

    /// Insert a new edge. Idempotent on `id`.
    pub async fn insert(&self, edge: &CausalEdge) -> common::Result<()> {
        self.insert_many(std::slice::from_ref(edge)).await
    }

    /// Insert many edges in a single transaction. Idempotent on `id`.
    pub async fn insert_many(&self, edges: &[CausalEdge]) -> common::Result<()> {
        if edges.is_empty() {
            return Ok(());
        }
        let mut tx = self
            .pool
            .inner()
            .begin()
            .await
            .map_err(|e| common::KlyntbotError::Storage(format!("causal tx: {e}")))?;
        for edge in edges {
            sqlx::query(
                "INSERT OR IGNORE INTO memory_causal_edges \
                 (id, from_id, to_id, edge_kind, confidence, inferred_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )
            .bind(edge.id.to_string())
            .bind(edge.from_id.to_string())
            .bind(edge.to_id.to_string())
            .bind(kind_str(edge.edge_kind))
            .bind(edge.confidence as f64)
            .bind(edge.inferred_at.to_string())
            .execute(&mut *tx)
            .await
            .map_err(|e| common::KlyntbotError::Storage(format!("causal insert: {e}")))?;
        }
        tx.commit()
            .await
            .map_err(|e| common::KlyntbotError::Storage(format!("causal commit: {e}")))?;
        Ok(())
    }

    /// All edges where `from_id = subject`.
    pub async fn by_from(&self, subject: Uuid) -> common::Result<Vec<CausalEdge>> {
        self.fetch_with("from_id", subject).await
    }

    /// All edges where `to_id = subject`.
    pub async fn by_to(&self, subject: Uuid) -> common::Result<Vec<CausalEdge>> {
        self.fetch_with("to_id", subject).await
    }

    async fn fetch_with(&self, column: &str, subject: Uuid) -> common::Result<Vec<CausalEdge>> {
        let sql = format!(
            "SELECT id, from_id, to_id, edge_kind, confidence, inferred_at \
             FROM memory_causal_edges WHERE {column} = ?1"
        );
        let rows: Vec<(String, String, String, String, f64, String)> = sqlx::query_as(&sql)
            .bind(subject.to_string())
            .fetch_all(self.pool.inner())
            .await
            .map_err(|e| common::KlyntbotError::Storage(format!("causal fetch: {e}")))?;
        rows.into_iter().map(parse_row).collect()
    }

    /// Groups of ≥`min_count` edges sharing a `problem_hash` (read from
    /// `episodic_memories.metadata->>'problemHash'` of either endpoint).
    /// Used by Phase 2.5 promotion.
    pub async fn groups_by_problem_hash(
        &self,
        repo: Option<&str>,
        since: Timestamp,
        min_count: u32,
    ) -> common::Result<Vec<ProblemHashGroup>> {
        let sql = "
            WITH edges_with_hash AS (
              SELECT mce.id AS edge_id,
                     COALESCE(
                       json_extract(em_from.metadata, '$.problemHash'),
                       json_extract(em_to.metadata,   '$.problemHash')
                     ) AS problem_hash,
                     COALESCE(em_from.scope_repo_id, em_to.scope_repo_id) AS repo_id
              FROM memory_causal_edges mce
              LEFT JOIN episodic_memories em_from ON em_from.id = mce.from_id
              LEFT JOIN episodic_memories em_to   ON em_to.id   = mce.to_id
              WHERE mce.inferred_at >= ?1
            )
            SELECT problem_hash, GROUP_CONCAT(edge_id) AS edge_ids, COUNT(*) AS cnt
            FROM edges_with_hash
            WHERE problem_hash IS NOT NULL
              AND (?2 IS NULL OR repo_id = ?2)
            GROUP BY problem_hash
            HAVING cnt >= ?3
        ";
        let rows: Vec<(String, String, i64)> = sqlx::query_as(sql)
            .bind(since.to_string())
            .bind(repo)
            .bind(min_count as i64)
            .fetch_all(self.pool.inner())
            .await
            .map_err(|e| common::KlyntbotError::Storage(format!("causal groups: {e}")))?;
        Ok(rows
            .into_iter()
            .map(|(problem_hash, edge_ids_csv, _)| ProblemHashGroup {
                problem_hash,
                edge_ids: edge_ids_csv
                    .split(',')
                    .filter_map(|s| Uuid::parse_str(s).ok())
                    .collect(),
            })
            .collect())
    }
}

fn kind_str(k: CausalEdgeKind) -> &'static str {
    match k {
        CausalEdgeKind::Broke => "broke",
        CausalEdgeKind::FixedBy => "fixed_by",
        CausalEdgeKind::FlippedToFail => "flipped_to_fail",
        CausalEdgeKind::SharesRootCause => "shares_root_cause",
        CausalEdgeKind::Enabled => "enabled",
    }
}

fn parse_kind(s: &str) -> common::Result<CausalEdgeKind> {
    match s {
        "broke" => Ok(CausalEdgeKind::Broke),
        "fixed_by" => Ok(CausalEdgeKind::FixedBy),
        "flipped_to_fail" => Ok(CausalEdgeKind::FlippedToFail),
        "shares_root_cause" => Ok(CausalEdgeKind::SharesRootCause),
        "enabled" => Ok(CausalEdgeKind::Enabled),
        other => Err(common::KlyntbotError::Storage(format!(
            "unknown causal kind: {other}"
        ))),
    }
}

fn parse_row(
    (id, from_id, to_id, kind, confidence, inferred_at): (
        String,
        String,
        String,
        String,
        f64,
        String,
    ),
) -> common::Result<CausalEdge> {
    Ok(CausalEdge {
        id: Uuid::parse_str(&id)
            .map_err(|e| common::KlyntbotError::Storage(format!("uuid: {e}")))?,
        from_id: Uuid::parse_str(&from_id)
            .map_err(|e| common::KlyntbotError::Storage(format!("uuid: {e}")))?,
        to_id: Uuid::parse_str(&to_id)
            .map_err(|e| common::KlyntbotError::Storage(format!("uuid: {e}")))?,
        edge_kind: parse_kind(&kind)?,
        confidence: confidence as f32,
        inferred_at: inferred_at
            .parse()
            .map_err(|e: jiff::Error| common::KlyntbotError::Storage(format!("ts: {e}")))?,
    })
}
