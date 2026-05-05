//! `CausalEdgeDetector` — three deterministic rules.

use crate::causal::CausalEdgeRepo;
use crate::scope::CausalEdge;
use cognitive::EpisodicMemoryRepo;
use std::sync::Arc;

/// Detector handle.
#[derive(Debug)]
pub struct CausalEdgeDetector {
    pub(crate) edges: Arc<CausalEdgeRepo>,
    pub(crate) episodes: Arc<EpisodicMemoryRepo>,
}

impl CausalEdgeDetector {
    /// Construct.
    #[must_use]
    pub fn new(edges: Arc<CausalEdgeRepo>, episodes: Arc<EpisodicMemoryRepo>) -> Self {
        Self { edges, episodes }
    }

    /// Run all three detection rules for one session. Returns count of edges inserted.
    pub async fn detect_for_session(&self, session_id: &str) -> common::Result<u32> {
        let (a, b, c) = tokio::try_join!(
            self.detect_test_flip(session_id),
            self.detect_fix_attempt_test_correlation(session_id),
            self.detect_problem_hash_chain(session_id),
        )?;
        let mut all = a;
        all.extend(b);
        all.extend(c);
        let count = u32::try_from(all.len()).unwrap_or(u32::MAX);
        self.edges.insert_many(&all).await?;
        Ok(count)
    }

    async fn detect_test_flip(&self, session_id: &str) -> common::Result<Vec<CausalEdge>> {
        let pool = self.episodes.pool();
        let rows: Vec<(String, String, String, String)> =
            sqlx::query_as::<_, (String, String, String, String)>(
                "SELECT id, content, recorded_at, COALESCE(scope_repo_id, '') \
             FROM episodic_memories \
             WHERE kind = 'test_run' \
               AND json_extract(content, '$.sessionId') = ?1 \
             ORDER BY recorded_at ASC",
            )
            .bind(session_id)
            .fetch_all(pool)
            .await
            .map_err(|e| common::KlyntbotError::Storage(format!("test runs: {e}")))?;

        let mut out = Vec::new();
        for window in rows.windows(2) {
            let (prev_id, prev_content, _, _) = &window[0];
            let (curr_id, curr_content, _, _) = &window[1];
            let prev_passed: bool = serde_json::from_str::<serde_json::Value>(prev_content)
                .ok()
                .and_then(|v| v.get("failed").and_then(|n| n.as_u64()).map(|n| n == 0))
                .unwrap_or(false);
            let curr_failed: bool = serde_json::from_str::<serde_json::Value>(curr_content)
                .ok()
                .and_then(|v| v.get("failed").and_then(|n| n.as_u64()).map(|n| n > 0))
                .unwrap_or(false);
            if prev_passed && curr_failed {
                out.push(CausalEdge {
                    id: uuid::Uuid::new_v4(),
                    from_id: uuid::Uuid::parse_str(prev_id).unwrap_or_else(|_| uuid::Uuid::nil()),
                    to_id: uuid::Uuid::parse_str(curr_id).unwrap_or_else(|_| uuid::Uuid::nil()),
                    edge_kind: crate::scope::CausalEdgeKind::FlippedToFail,
                    confidence: 0.85,
                    inferred_at: jiff::Timestamp::now(),
                });
            }
        }
        Ok(out)
    }

    async fn detect_fix_attempt_test_correlation(
        &self,
        session_id: &str,
    ) -> common::Result<Vec<CausalEdge>> {
        let pool = self.episodes.pool();
        let rows: Vec<(String, String, String, String)> =
            sqlx::query_as::<_, (String, String, String, String)>(
                "SELECT id, kind, content, recorded_at \
             FROM episodic_memories \
             WHERE json_extract(content, '$.sessionId') = ?1 \
               AND kind IN ('fix_attempt','test_run') \
             ORDER BY recorded_at ASC",
            )
            .bind(session_id)
            .fetch_all(pool)
            .await
            .map_err(|e| common::KlyntbotError::Storage(format!("fix-test pairs: {e}")))?;

        let mut by_turn: std::collections::HashMap<
            String,
            Vec<(String, String, serde_json::Value)>,
        > = std::collections::HashMap::new();
        for (id, kind, content, _) in rows {
            let v: serde_json::Value = match content.parse() {
                Ok(v) => v,
                Err(_) => continue,
            };
            let turn = v
                .get("turnId")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string();
            by_turn.entry(turn).or_default().push((id, kind, v));
        }

        let mut out = Vec::new();
        for (_turn, items) in by_turn {
            let fix = items.iter().find(|(_, k, _)| k == "fix_attempt");
            let test = items.iter().find(|(_, k, _)| k == "test_run");
            if let (Some((fix_id, _, _)), Some((test_id, _, test_v))) = (fix, test) {
                let failed = test_v.get("failed").and_then(|n| n.as_u64()).unwrap_or(0);
                let kind = if failed == 0 {
                    crate::scope::CausalEdgeKind::FixedBy
                } else {
                    crate::scope::CausalEdgeKind::Broke
                };
                out.push(CausalEdge {
                    id: uuid::Uuid::new_v4(),
                    from_id: uuid::Uuid::parse_str(fix_id).unwrap_or_else(|_| uuid::Uuid::nil()),
                    to_id: uuid::Uuid::parse_str(test_id).unwrap_or_else(|_| uuid::Uuid::nil()),
                    edge_kind: kind,
                    confidence: 0.7,
                    inferred_at: jiff::Timestamp::now(),
                });
            }
        }
        Ok(out)
    }

    async fn detect_problem_hash_chain(&self, session_id: &str) -> common::Result<Vec<CausalEdge>> {
        let pool = self.episodes.pool();
        let rows: Vec<(String, Option<String>)> = sqlx::query_as::<_, (String, Option<String>)>(
            "SELECT id, json_extract(metadata, '$.problemHash') AS h \
             FROM episodic_memories \
             WHERE kind = 'fix_attempt' \
               AND json_extract(content, '$.sessionId') = ?1 \
               AND h IS NOT NULL \
             ORDER BY recorded_at ASC",
        )
        .bind(session_id)
        .fetch_all(pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("hash chain: {e}")))?;

        let mut by_hash: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        for (id, h) in rows {
            if let Some(h) = h {
                by_hash.entry(h).or_default().push(id);
            }
        }
        let mut out = Vec::new();
        for (_h, ids) in by_hash {
            for window in ids.windows(2) {
                out.push(CausalEdge {
                    id: uuid::Uuid::new_v4(),
                    from_id: uuid::Uuid::parse_str(&window[0])
                        .unwrap_or_else(|_| uuid::Uuid::nil()),
                    to_id: uuid::Uuid::parse_str(&window[1]).unwrap_or_else(|_| uuid::Uuid::nil()),
                    edge_kind: crate::scope::CausalEdgeKind::SharesRootCause,
                    confidence: 0.6,
                    inferred_at: jiff::Timestamp::now(),
                });
            }
        }
        Ok(out)
    }
}
