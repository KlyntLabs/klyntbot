use ai_core::mirror::{MirrorSignalSource, MirrorSnapshotSpec};
use bus::DomainEvent;
use bus::ToolExecutionEvent;
use futures_util::future::join_all;
use std::sync::Arc;
use storage::repos::ApprovalPatternHistoryRepo;

const MIN_APPROVAL_COUNT: u32 = 3;
const MIN_APPROVAL_RATE: f32 = 0.80;
const RECENCY_WINDOW_DAYS: i64 = 30;

/// Scope of a suggested pattern, used to build the `GrantScope` on the consumer side.
#[derive(Debug, Clone)]
pub enum PatternScope {
    ExactPath(String),
    ParentFolder(String),
    Glob(String),
}

pub struct ApprovalHistorySource {
    pattern_repo: Arc<ApprovalPatternHistoryRepo>,
    pending: dashmap::DashMap<String, PendingReq>,
}

struct PendingReq;

/// A suggested grant pattern derived from approval history.
#[derive(Debug, Clone)]
pub struct SuggestedPattern {
    /// Human-readable label, e.g. "Edit on src/components/**".
    pub label: String,
    /// The scope of the matched pattern.
    pub scope: PatternScope,
    /// Number of prior approvals matching this pattern.
    pub approval_count: u32,
}

impl ApprovalHistorySource {
    pub fn new(pattern_repo: Arc<ApprovalPatternHistoryRepo>) -> Self {
        Self {
            pattern_repo,
            pending: dashmap::DashMap::new(),
        }
    }

    /// Record a pending approval request.
    pub fn observe_request(
        &self,
        request_id: &str,
        _tool: &str,
        _args_hash: &str,
        _layer: &str,
        _repo_id: &str,
    ) {
        self.pending.insert(request_id.to_string(), PendingReq);
    }

    /// Record an approval resolution (no-op since legacy repo was removed).
    pub async fn observe_resolution(&self, _request_id: &str, _decision: &str, _decided_by: &str) {}

    /// Persist a resolved approval directly to the pattern history table.
    pub async fn persist_pattern_observation(
        &self,
        user_id: &str,
        tool_name: &str,
        path: Option<&str>,
        decision: &str,
        pattern_used: Option<&str>,
        occurred_at: i64,
    ) {
        let _ = self
            .pattern_repo
            .record(storage::repos::PatternHistoryEntry {
                user_id: user_id.to_string(),
                tool_name: tool_name.to_string(),
                path: path.map(String::from),
                decision: decision.to_string(),
                pattern_used: pattern_used.map(String::from),
                occurred_at,
            })
            .await;
    }

    /// Suggest a grant pattern for a tool call based on approval history.
    /// Evaluates four candidate patterns (exact path, parent folder, prefix/**, **/*.ext)
    /// and returns the highest-scoring one that meets the threshold criteria.
    pub async fn suggest_pattern(
        &self,
        tool: &str,
        path: Option<&str>,
    ) -> Option<SuggestedPattern> {
        let path = path?;
        let user_id = "default";

        let candidates = build_path_candidates(path);
        // Deduplicate by SQL path_like pattern to avoid redundant DB round-trips.
        let mut seen = std::collections::HashSet::new();
        let unique_candidates: Vec<_> = candidates
            .into_iter()
            .filter(|(scope, _, _)| {
                let path_like = match scope {
                    PatternScope::ExactPath(p) => p.clone(),
                    PatternScope::ParentFolder(p) => format!("{}/%", p.trim_end_matches('/')),
                    PatternScope::Glob(g) => g.clone(),
                };
                seen.insert(path_like)
            })
            .collect();

        // Run independent DB queries concurrently.
        let futures: Vec<_> = unique_candidates
            .into_iter()
            .map(|(scope, label, specificity)| async move {
                let path_like = match &scope {
                    PatternScope::ExactPath(p) => p.clone(),
                    PatternScope::ParentFolder(p) => format!("{}/%", p.trim_end_matches('/')),
                    PatternScope::Glob(g) => g.clone(),
                };
                let stats = self
                    .pattern_repo
                    .pattern_stats(user_id, tool, &path_like, RECENCY_WINDOW_DAYS)
                    .await
                    .ok()?;
                Some((scope, label, specificity, stats))
            })
            .collect();

        let results = join_all(futures).await;
        let mut best: Option<(f32, SuggestedPattern)> = None;
        for result in results.into_iter().flatten() {
            let (scope, label, specificity, (approvals, total)) = result;
            if total < MIN_APPROVAL_COUNT {
                continue;
            }
            let rate = approvals as f32 / total as f32;
            if rate < MIN_APPROVAL_RATE {
                continue;
            }

            let score = approvals as f32 * specificity;
            if best
                .as_ref()
                .map_or(true, |(best_score, _)| score > *best_score)
            {
                best = Some((
                    score,
                    SuggestedPattern {
                        label: format!("{} ({} approvals)", label, approvals),
                        scope,
                        approval_count: approvals,
                    },
                ));
            }
        }

        best.map(|(_, pattern)| pattern)
    }

    /// Subscribe to the domain event bus and process approval-related events.
    pub async fn run(&self, bus: Arc<bus::DomainEventBus>) {
        let mut rx = bus.subscribe();
        while let Ok(evt) = rx.recv().await {
            match evt {
                DomainEvent::ToolExecution(ToolExecutionEvent::ApprovalRequested {
                    request_id,
                    tool,
                    args_hash,
                    layer,
                    repo_id,
                }) => {
                    self.observe_request(
                        &request_id,
                        &tool,
                        &args_hash,
                        &layer,
                        repo_id.as_deref().unwrap_or(""),
                    );
                }
                DomainEvent::ToolExecution(ToolExecutionEvent::ApprovalResolved {
                    request_id,
                    user_id,
                    tool_name,
                    path,
                    decision,
                    pattern_used,
                    occurred_at,
                    decided_by,
                    ..
                }) => {
                    let _ = self
                        .observe_resolution(&request_id, &decision, &decided_by)
                        .await;
                    self.persist_pattern_observation(
                        user_id.as_deref().unwrap_or("default"),
                        &tool_name,
                        path.as_deref(),
                        &decision,
                        pattern_used.as_deref(),
                        occurred_at,
                    )
                    .await;
                }
                _ => {}
            }
        }
    }
}

/// Build candidate patterns for a given path with specificity weights.
/// Returns tuples of (PatternScope, human_label, specificity_weight).
fn build_path_candidates(path: &str) -> Vec<(PatternScope, String, f32)> {
    let mut candidates = Vec::with_capacity(4);

    // 1. Exact path (highest specificity)
    candidates.push((
        PatternScope::ExactPath(path.to_string()),
        format!("Allow on {}", path),
        4.0,
    ));

    // 2. Parent folder
    if let Some(parent) = std::path::Path::new(path).parent() {
        let parent_str = parent.to_string_lossy().to_string();
        if !parent_str.is_empty() {
            candidates.push((
                PatternScope::ParentFolder(parent_str.clone()),
                format!("Allow on {}/**", parent_str.trim_end_matches('/')),
                3.0,
            ));
        }
    }

    // 3. Prefix glob (e.g., src/components/**)
    if let Some(prefix) = path.rfind('/') {
        let prefix = &path[..prefix];
        if !prefix.is_empty() {
            candidates.push((
                PatternScope::Glob(format!("{}/%", prefix)),
                format!("Allow on {}/**", prefix),
                2.0,
            ));
        }
    }

    // 4. Extension glob
    if let Some(ext) = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
    {
        candidates.push((
            PatternScope::Glob(format!("%.{}", ext)),
            format!("Allow on **/*.{}", ext),
            1.5,
        ));
    }

    candidates
}

#[async_trait::async_trait]
impl MirrorSignalSource for ApprovalHistorySource {
    fn spec(&self) -> MirrorSnapshotSpec {
        MirrorSnapshotSpec {
            name: "approval_history",
            subscribed_kinds: &[],
            flush_interval_secs: None,
        }
    }

    fn name(&self) -> &'static str {
        "approval_history"
    }

    async fn accumulate(&self, _signal: &ai_core::AiSignal) -> common::Result<()> {
        Ok(())
    }

    async fn flush(&self) -> common::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage::StoragePool;

    async fn setup_source() -> (ApprovalHistorySource, sqlx::SqlitePool) {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let inner = pool.inner().clone();
        let pattern_repo = Arc::new(ApprovalPatternHistoryRepo::new(inner.clone()));
        (ApprovalHistorySource::new(pattern_repo), inner)
    }

    #[tokio::test]
    async fn cold_start_returns_none() {
        let (source, _) = setup_source().await;
        let result = source.suggest_pattern("edit", Some("src/foo.rs")).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn exact_path_match_after_threshold() {
        let (source, pool) = setup_source().await;
        let repo = ApprovalPatternHistoryRepo::new(pool);
        let now = jiff::Timestamp::now().as_second();
        for _ in 0..3 {
            repo.record(storage::repos::PatternHistoryEntry {
                user_id: "default".into(),
                tool_name: "edit".into(),
                path: Some("src/components/Button.tsx".into()),
                decision: "allow".into(),
                pattern_used: None,
                occurred_at: now,
            })
            .await
            .unwrap();
        }

        let result = source
            .suggest_pattern("edit", Some("src/components/Button.tsx"))
            .await;
        assert!(result.is_some());
        let pattern = result.unwrap();
        assert!(pattern.label.contains("src/components/Button.tsx"));
        assert!(matches!(pattern.scope, PatternScope::ExactPath(_)));
    }

    #[tokio::test]
    async fn parent_folder_matches_child_paths() {
        let (source, pool) = setup_source().await;
        let repo = ApprovalPatternHistoryRepo::new(pool);
        let now = jiff::Timestamp::now().as_second();
        for _ in 0..3 {
            repo.record(storage::repos::PatternHistoryEntry {
                user_id: "default".into(),
                tool_name: "edit".into(),
                path: Some("src/components/Modal.tsx".into()),
                decision: "allow".into(),
                pattern_used: None,
                occurred_at: now,
            })
            .await
            .unwrap();
        }

        // Parent folder should match a new child path because Modal has 3 allows
        let folder = source
            .suggest_pattern("edit", Some("src/components/Card.tsx"))
            .await;
        assert!(folder.is_some());
    }

    #[tokio::test]
    async fn denial_ratio_blocks_suggestion() {
        let (source, pool) = setup_source().await;
        let repo = ApprovalPatternHistoryRepo::new(pool);
        let now = jiff::Timestamp::now().as_second();
        for _ in 0..2 {
            repo.record(storage::repos::PatternHistoryEntry {
                user_id: "default".into(),
                tool_name: "edit".into(),
                path: Some("src/utils.ts".into()),
                decision: "allow".into(),
                pattern_used: None,
                occurred_at: now,
            })
            .await
            .unwrap();
        }
        repo.record(storage::repos::PatternHistoryEntry {
            user_id: "default".into(),
            tool_name: "edit".into(),
            path: Some("src/utils.ts".into()),
            decision: "deny".into(),
            pattern_used: None,
            occurred_at: now,
        })
        .await
        .unwrap();

        let result = source.suggest_pattern("edit", Some("src/utils.ts")).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn specificity_prefers_exact_over_folder() {
        let (source, pool) = setup_source().await;
        let repo = ApprovalPatternHistoryRepo::new(pool);
        let now = jiff::Timestamp::now().as_second();
        for _ in 0..3 {
            repo.record(storage::repos::PatternHistoryEntry {
                user_id: "default".into(),
                tool_name: "edit".into(),
                path: Some("src/components/Button.tsx".into()),
                decision: "allow".into(),
                pattern_used: None,
                occurred_at: now,
            })
            .await
            .unwrap();
        }

        let result = source
            .suggest_pattern("edit", Some("src/components/Button.tsx"))
            .await;
        assert!(result.is_some());
        let pattern = result.unwrap();
        // Exact path (3 approvals * 4.0 specificity = 12.0) should beat
        // parent folder (3 approvals * 3.0 specificity = 9.0).
        assert!(matches!(pattern.scope, PatternScope::ExactPath(_)));
    }
}
