//! ProductivityTreeBuilder — event subscriber that creates tree nodes from
//! productivity events (FocusSessionEnded, ActivitySessionCompleted,
//! ProductivityScoreComputed) so productivity data participates in the entity
//! graph, community detection, and Knowledge Fabric Explorer.
//!
//! Like FinanceTreeBuilder, this uses **incremental appends**: each event
//! creates new leaf nodes under a daily root. Root nodes are idempotent — if
//! a duplicate-ID insert fails, it is silently skipped because the node
//! already exists.
//!
//! Tree structure per day:
//! ```text
//! productivity-daily-{YYYY-MM-DD}  (root, level 0, Section, title: "Productivity: {date}")
//!   ├── productivity-focus-{uuid}  (level 1, Text)  — "Focus session: 30 min, quality 85%, 2 interruptions"
//!   ├── productivity-activity-{date}  (level 1, Text)  — "Activity: 6.5h active, 78% productive, 12% distracting"
//!   └── productivity-score-{date}  (level 1, Text)  — "Productivity score: 72/100"
//! ```

use std::sync::Arc;

use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tracing::warn;
use uuid::Uuid;

use bus::{ContextUpdateReason, DomainEvent};

use context_engine::book_index::types::{SourceType, TreeNode, TreeNodeType};

use super::tree_builder_base::{run_event_loop, TreeBuilderCore};

pub struct ProductivityTreeBuilder {
    core: TreeBuilderCore,
}

impl ProductivityTreeBuilder {
    pub fn new(
        tree_repo: Arc<dyn context_engine::book_index::BookTreeRepo>,
        vector_store: Arc<storage::VectorStore>,
        embedder: Arc<dyn cognitive::TextEmbedder>,
        context_update_queue: Option<Arc<bus::ContextUpdateQueue>>,
        domain_event_bus: Option<Arc<bus::DomainEventBus>>,
    ) -> Self {
        Self {
            core: TreeBuilderCore::new(
                tree_repo,
                vector_store,
                embedder,
                context_update_queue,
                domain_event_bus,
            ),
        }
    }

    /// Backfill tree nodes from existing daily productivity summaries.
    ///
    /// Queries the `daily_summaries` table and calls `handle_activity_session`
    /// for each row. Insert errors for already-indexed dates are silently ignored
    /// by the underlying `persist_nodes` logic, so this is safe to run on startup.
    pub async fn backfill_from_summaries(&self, pool: &sqlx::SqlitePool) -> common::Result<usize> {
        let summaries: Vec<(String, i64, i64, i64)> = sqlx::query_as(
            "SELECT date, total_active_secs, productive_secs, distracting_secs \
             FROM daily_summaries ORDER BY date",
        )
        .fetch_all(pool)
        .await
        .map_err(|e| {
            common::ToolError::ExecutionFailed(format!("productivity backfill query: {e}"))
        })?;

        let mut count = 0;
        for (date, total_active, productive, distracting) in &summaries {
            if self
                .handle_activity_session(date, *total_active, *productive, *distracting)
                .await
                .is_ok()
            {
                count += 1;
            }
        }
        Ok(count)
    }

    /// Event-driven subscriber loop. Listens for productivity domain events
    /// and appends tree nodes for each.
    pub async fn run(
        self: Arc<Self>,
        rx: broadcast::Receiver<DomainEvent>,
        shutdown: CancellationToken,
    ) {
        run_event_loop("ProductivityTreeBuilder", rx, shutdown, |event| {
            let this = Arc::clone(&self);
            async move {
                match event {
                    DomainEvent::FocusSessionEnded {
                        duration_secs,
                        quality,
                        interruptions,
                    } => {
                        if let Err(e) = this
                            .handle_focus_session_ended(duration_secs, quality, interruptions)
                            .await
                        {
                            warn!(
                                duration_secs,
                                "ProductivityTreeBuilder: failed to process focus session: {e}"
                            );
                        }
                    }
                    DomainEvent::ActivitySessionCompleted {
                        date,
                        total_active_secs,
                        productive_secs,
                        distracting_secs,
                    } => {
                        if let Err(e) = this
                            .handle_activity_session(
                                &date,
                                total_active_secs,
                                productive_secs,
                                distracting_secs,
                            )
                            .await
                        {
                            warn!(
                                date = %date,
                                "ProductivityTreeBuilder: failed to process activity session: {e}"
                            );
                        }
                    }
                    DomainEvent::ProductivityScoreComputed { date, score } => {
                        if let Err(e) = this.handle_productivity_score(&date, score).await {
                            warn!(
                                date = %date,
                                "ProductivityTreeBuilder: failed to process productivity score: {e}"
                            );
                        }
                    }
                    _ => {}
                }
            }
        })
        .await
    }

    /// Handle a focus session ended event: upsert the daily root node and
    /// insert a unique focus session leaf node.
    pub async fn handle_focus_session_ended(
        &self,
        duration_secs: i64,
        quality: f64,
        interruptions: i32,
    ) -> common::Result<()> {
        let date = jiff::Timestamp::now().strftime("%Y-%m-%d").to_string();
        let daily_id = format!("productivity-daily-{date}");
        let focus_id = format!("productivity-focus-{}", Uuid::new_v4());

        tracing::debug!(
            duration_secs,
            quality,
            interruptions,
            focus_id = %focus_id,
            "ProductivityTreeBuilder: processing focus session"
        );

        let nodes = build_focus_nodes(
            &date,
            &daily_id,
            &focus_id,
            duration_secs,
            quality,
            interruptions,
        );

        self.core
            .persist_nodes(
                "ProductivityTreeBuilder",
                &nodes,
                &daily_id,
                ContextUpdateReason::FocusSessionEnded,
                Some(format!(
                    "Productivity tree updated: {} new node(s) for {daily_id}",
                    nodes.len()
                )),
            )
            .await
    }

    /// Handle an activity session completed event: upsert the daily root node
    /// and insert an activity summary leaf node (idempotent by date).
    pub async fn handle_activity_session(
        &self,
        date: &str,
        total_active_secs: i64,
        productive_secs: i64,
        distracting_secs: i64,
    ) -> common::Result<()> {
        let daily_id = format!("productivity-daily-{date}");
        let activity_id = format!("productivity-activity-{date}");

        tracing::debug!(
            date = %date,
            total_active_secs,
            productive_secs,
            distracting_secs,
            activity_id = %activity_id,
            "ProductivityTreeBuilder: processing activity session"
        );

        let nodes = build_activity_nodes(
            date,
            &daily_id,
            &activity_id,
            total_active_secs,
            productive_secs,
            distracting_secs,
        );

        self.core
            .persist_nodes(
                "ProductivityTreeBuilder",
                &nodes,
                &daily_id,
                ContextUpdateReason::FocusSessionEnded,
                Some(format!(
                    "Productivity tree updated: {} new node(s) for {daily_id}",
                    nodes.len()
                )),
            )
            .await
    }

    /// Handle a productivity score computed event: upsert the daily root node
    /// and insert a score leaf node (idempotent by date).
    pub async fn handle_productivity_score(&self, date: &str, score: f64) -> common::Result<()> {
        let daily_id = format!("productivity-daily-{date}");
        let score_id = format!("productivity-score-{date}");

        tracing::debug!(
            date = %date,
            score,
            score_id = %score_id,
            "ProductivityTreeBuilder: processing productivity score"
        );

        let nodes = build_score_nodes(date, &daily_id, &score_id, score);

        self.core
            .persist_nodes(
                "ProductivityTreeBuilder",
                &nodes,
                &daily_id,
                ContextUpdateReason::FocusSessionEnded,
                Some(format!(
                    "Productivity tree updated: {} new node(s) for {daily_id}",
                    nodes.len()
                )),
            )
            .await
    }
}

// ---------------------------------------------------------------------------
// Pure node construction helpers (public for unit testing)
// ---------------------------------------------------------------------------

/// Build the nodes for a `FocusSessionEnded` event.
///
/// Returns 2 nodes: daily root (level 0, idempotent) and a unique focus
/// session leaf (level 1). The leaf always uses a UUID so multiple focus
/// sessions per day are each recorded independently.
pub fn build_focus_nodes(
    date: &str,
    daily_id: &str,
    focus_id: &str,
    duration_secs: i64,
    quality: f64,
    interruptions: i32,
) -> Vec<TreeNode> {
    let duration_mins = duration_secs / 60;
    let quality_pct = (quality * 100.0).round() as u64;
    let focus_content = format!(
        "Focus session: {duration_mins} min, quality {quality_pct}%, {interruptions} interruption{}",
        if interruptions == 1 { "" } else { "s" }
    );

    vec![
        // Level 0 — daily root
        TreeNode {
            id: daily_id.to_string(),
            parent_id: None,
            node_type: TreeNodeType::Section,
            content: daily_id.to_string(),
            title: Some(format!("Productivity: {date}")),
            level: 0,
            source_type: SourceType::Productivity,
            source_id: daily_id.to_string(),
            position: 0,
            metadata: None,
        },
        // Level 1 — focus session leaf (unique UUID per session)
        TreeNode {
            id: focus_id.to_string(),
            parent_id: Some(daily_id.to_string()),
            node_type: TreeNodeType::Text,
            content: focus_content,
            title: None,
            level: 1,
            source_type: SourceType::Productivity,
            source_id: daily_id.to_string(),
            position: 1,
            metadata: None,
        },
    ]
}

/// Build the nodes for an `ActivitySessionCompleted` event.
///
/// Returns 2 nodes: daily root (level 0, idempotent) and an activity summary
/// leaf (level 1, idempotent by date). Repeated activity events for the same
/// date are deduplicated by the duplicate-ignore logic in `persist_nodes`.
pub fn build_activity_nodes(
    date: &str,
    daily_id: &str,
    activity_id: &str,
    total_active_secs: i64,
    productive_secs: i64,
    distracting_secs: i64,
) -> Vec<TreeNode> {
    let total_hours = total_active_secs as f64 / 3600.0;
    let productive_pct = if total_active_secs > 0 {
        (productive_secs as f64 / total_active_secs as f64 * 100.0).round() as u64
    } else {
        0
    };
    let distracting_pct = if total_active_secs > 0 {
        (distracting_secs as f64 / total_active_secs as f64 * 100.0).round() as u64
    } else {
        0
    };
    let activity_content = format!(
        "Activity: {total_hours:.1}h active, {productive_pct}% productive, {distracting_pct}% distracting"
    );

    vec![
        // Level 0 — daily root
        TreeNode {
            id: daily_id.to_string(),
            parent_id: None,
            node_type: TreeNodeType::Section,
            content: daily_id.to_string(),
            title: Some(format!("Productivity: {date}")),
            level: 0,
            source_type: SourceType::Productivity,
            source_id: daily_id.to_string(),
            position: 0,
            metadata: None,
        },
        // Level 1 — activity summary leaf (idempotent by date)
        TreeNode {
            id: activity_id.to_string(),
            parent_id: Some(daily_id.to_string()),
            node_type: TreeNodeType::Text,
            content: activity_content,
            title: None,
            level: 1,
            source_type: SourceType::Productivity,
            source_id: daily_id.to_string(),
            position: 2,
            metadata: None,
        },
    ]
}

/// Build the nodes for a `ProductivityScoreComputed` event.
///
/// Returns 2 nodes: daily root (level 0, idempotent) and a score leaf
/// (level 1, idempotent by date). Repeated score events for the same date
/// are deduplicated by the duplicate-ignore logic in `persist_nodes`.
pub fn build_score_nodes(date: &str, daily_id: &str, score_id: &str, score: f64) -> Vec<TreeNode> {
    let score_rounded = score.round() as u64;
    let score_content = format!("Productivity score: {score_rounded}/100");

    vec![
        // Level 0 — daily root
        TreeNode {
            id: daily_id.to_string(),
            parent_id: None,
            node_type: TreeNodeType::Section,
            content: daily_id.to_string(),
            title: Some(format!("Productivity: {date}")),
            level: 0,
            source_type: SourceType::Productivity,
            source_id: daily_id.to_string(),
            position: 0,
            metadata: None,
        },
        // Level 1 — productivity score leaf (idempotent by date)
        TreeNode {
            id: score_id.to_string(),
            parent_id: Some(daily_id.to_string()),
            node_type: TreeNodeType::Text,
            content: score_content,
            title: None,
            level: 1,
            source_type: SourceType::Productivity,
            source_id: daily_id.to_string(),
            position: 3,
            metadata: None,
        },
    ]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::adapters::tree_builder_base::compose_embedding_text;
    use super::*;

    // --- compose_embedding_text ---

    #[test]
    fn embedding_level_0_returns_title() {
        let node = TreeNode {
            id: "productivity-daily-2024-01-15".into(),
            parent_id: None,
            node_type: TreeNodeType::Section,
            content: "productivity-daily-2024-01-15".into(),
            title: Some("Productivity: 2024-01-15".into()),
            level: 0,
            source_type: SourceType::Productivity,
            source_id: "productivity-daily-2024-01-15".into(),
            position: 0,
            metadata: None,
        };
        assert_eq!(compose_embedding_text(&node), "Productivity: 2024-01-15");
    }

    #[test]
    fn embedding_level_1_returns_content() {
        let node = TreeNode {
            id: "productivity-focus-abc".into(),
            parent_id: Some("productivity-daily-2024-01-15".into()),
            node_type: TreeNodeType::Text,
            content: "Focus session: 25 min, quality 80%, 1 interruption".into(),
            title: None,
            level: 1,
            source_type: SourceType::Productivity,
            source_id: "productivity-daily-2024-01-15".into(),
            position: 1,
            metadata: None,
        };
        assert_eq!(
            compose_embedding_text(&node),
            "Focus session: 25 min, quality 80%, 1 interruption"
        );
    }

    // --- build_focus_nodes ---

    #[test]
    fn focus_nodes_structure() {
        let nodes = build_focus_nodes(
            "2024-01-15",
            "productivity-daily-2024-01-15",
            "productivity-focus-test-uuid",
            1800, // 30 mins
            0.85,
            2,
        );

        assert_eq!(nodes.len(), 2);

        // Root
        let root = &nodes[0];
        assert_eq!(root.id, "productivity-daily-2024-01-15");
        assert_eq!(root.parent_id, None);
        assert_eq!(root.level, 0);
        assert!(matches!(root.node_type, TreeNodeType::Section));
        assert_eq!(root.title.as_deref(), Some("Productivity: 2024-01-15"));
        assert_eq!(root.source_type.as_str(), "productivity");

        // Focus leaf
        let leaf = &nodes[1];
        assert_eq!(leaf.id, "productivity-focus-test-uuid");
        assert_eq!(
            leaf.parent_id.as_deref(),
            Some("productivity-daily-2024-01-15")
        );
        assert_eq!(leaf.level, 1);
        assert!(matches!(leaf.node_type, TreeNodeType::Text));
        assert!(leaf.content.contains("30 min"), "content: {}", leaf.content);
        assert!(leaf.content.contains("85%"), "content: {}", leaf.content);
        assert!(
            leaf.content.contains("2 interruptions"),
            "content: {}",
            leaf.content
        );
        assert_eq!(leaf.title, None);
    }

    #[test]
    fn focus_nodes_single_interruption_grammar() {
        let nodes = build_focus_nodes(
            "2024-01-15",
            "productivity-daily-2024-01-15",
            "productivity-focus-test",
            600,
            1.0,
            1,
        );
        let leaf = &nodes[1];
        assert!(
            leaf.content.contains("1 interruption") && !leaf.content.contains("interruptions"),
            "content: {}",
            leaf.content
        );
    }

    #[test]
    fn focus_nodes_zero_interruptions() {
        let nodes = build_focus_nodes(
            "2024-01-15",
            "productivity-daily-2024-01-15",
            "productivity-focus-test",
            3600,
            0.95,
            0,
        );
        let leaf = &nodes[1];
        assert!(
            leaf.content.contains("0 interruptions"),
            "content: {}",
            leaf.content
        );
    }

    #[test]
    fn focus_nodes_all_have_productivity_source_type() {
        let nodes = build_focus_nodes(
            "2024-01-15",
            "productivity-daily-2024-01-15",
            "productivity-focus-xyz",
            900,
            0.7,
            3,
        );
        for node in &nodes {
            assert_eq!(node.source_type.as_str(), "productivity");
            assert_eq!(node.source_id, "productivity-daily-2024-01-15");
        }
    }

    // --- build_activity_nodes ---

    #[test]
    fn activity_nodes_structure() {
        let nodes = build_activity_nodes(
            "2024-01-15",
            "productivity-daily-2024-01-15",
            "productivity-activity-2024-01-15",
            23400, // 6.5h
            18252, // ~78% productive
            2808,  // ~12% distracting
        );

        assert_eq!(nodes.len(), 2);

        // Root
        let root = &nodes[0];
        assert_eq!(root.id, "productivity-daily-2024-01-15");
        assert_eq!(root.level, 0);
        assert_eq!(root.title.as_deref(), Some("Productivity: 2024-01-15"));

        // Activity leaf
        let leaf = &nodes[1];
        assert_eq!(leaf.id, "productivity-activity-2024-01-15");
        assert_eq!(
            leaf.parent_id.as_deref(),
            Some("productivity-daily-2024-01-15")
        );
        assert_eq!(leaf.level, 1);
        assert!(matches!(leaf.node_type, TreeNodeType::Text));
        assert!(leaf.content.contains("6.5h"), "content: {}", leaf.content);
        assert!(leaf.content.contains("78%"), "content: {}", leaf.content);
        assert!(leaf.content.contains("12%"), "content: {}", leaf.content);
    }

    #[test]
    fn activity_nodes_zero_active_secs_no_divide_by_zero() {
        let nodes = build_activity_nodes(
            "2024-01-15",
            "productivity-daily-2024-01-15",
            "productivity-activity-2024-01-15",
            0,
            0,
            0,
        );
        let leaf = &nodes[1];
        assert!(leaf.content.contains("0.0h"), "content: {}", leaf.content);
        assert!(leaf.content.contains("0%"), "content: {}", leaf.content);
    }

    #[test]
    fn activity_nodes_all_have_productivity_source_type() {
        let nodes = build_activity_nodes(
            "2024-01-15",
            "productivity-daily-2024-01-15",
            "productivity-activity-2024-01-15",
            3600,
            1800,
            360,
        );
        for node in &nodes {
            assert_eq!(node.source_type.as_str(), "productivity");
            assert_eq!(node.source_id, "productivity-daily-2024-01-15");
        }
    }

    // --- build_score_nodes ---

    #[test]
    fn score_nodes_structure() {
        let nodes = build_score_nodes(
            "2024-01-15",
            "productivity-daily-2024-01-15",
            "productivity-score-2024-01-15",
            72.4,
        );

        assert_eq!(nodes.len(), 2);

        // Root
        let root = &nodes[0];
        assert_eq!(root.id, "productivity-daily-2024-01-15");
        assert_eq!(root.level, 0);
        assert_eq!(root.title.as_deref(), Some("Productivity: 2024-01-15"));

        // Score leaf
        let leaf = &nodes[1];
        assert_eq!(leaf.id, "productivity-score-2024-01-15");
        assert_eq!(
            leaf.parent_id.as_deref(),
            Some("productivity-daily-2024-01-15")
        );
        assert_eq!(leaf.level, 1);
        assert!(matches!(leaf.node_type, TreeNodeType::Text));
        // 72.4 rounds to 72
        assert!(leaf.content.contains("72/100"), "content: {}", leaf.content);
    }

    #[test]
    fn score_nodes_rounds_score() {
        let nodes = build_score_nodes(
            "2024-01-15",
            "productivity-daily-2024-01-15",
            "productivity-score-2024-01-15",
            85.6,
        );
        let leaf = &nodes[1];
        // 85.6 rounds to 86
        assert!(leaf.content.contains("86/100"), "content: {}", leaf.content);
    }

    #[test]
    fn score_nodes_all_have_productivity_source_type() {
        let nodes = build_score_nodes(
            "2024-01-15",
            "productivity-daily-2024-01-15",
            "productivity-score-2024-01-15",
            50.0,
        );
        for node in &nodes {
            assert_eq!(node.source_type.as_str(), "productivity");
            assert_eq!(node.source_id, "productivity-daily-2024-01-15");
        }
    }

    // --- deterministic IDs ---

    #[test]
    fn daily_root_id_is_deterministic_by_date() {
        let nodes_a = build_score_nodes(
            "2024-03-01",
            "productivity-daily-2024-03-01",
            "productivity-score-2024-03-01",
            60.0,
        );
        let nodes_b = build_activity_nodes(
            "2024-03-01",
            "productivity-daily-2024-03-01",
            "productivity-activity-2024-03-01",
            3600,
            1800,
            360,
        );
        // Both roots for the same date share the same ID
        assert_eq!(nodes_a[0].id, nodes_b[0].id);
        assert_eq!(nodes_a[0].id, "productivity-daily-2024-03-01");
    }

    #[test]
    fn activity_and_score_leaf_ids_are_per_date() {
        let activity_nodes = build_activity_nodes(
            "2024-03-01",
            "productivity-daily-2024-03-01",
            "productivity-activity-2024-03-01",
            3600,
            1800,
            360,
        );
        let score_nodes = build_score_nodes(
            "2024-03-01",
            "productivity-daily-2024-03-01",
            "productivity-score-2024-03-01",
            75.0,
        );
        // Leaf IDs must differ (they are different node types)
        assert_ne!(activity_nodes[1].id, score_nodes[1].id);
        assert_eq!(activity_nodes[1].id, "productivity-activity-2024-03-01");
        assert_eq!(score_nodes[1].id, "productivity-score-2024-03-01");
    }
}
