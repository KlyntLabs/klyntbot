//! FinanceTreeBuilder — event subscriber that creates tree nodes from financial
//! events (TransactionRecorded, BudgetAlert) so finance data participates in
//! the entity graph, community detection, and Knowledge Fabric Explorer.
//!
//! Unlike NoteTreeBuilder (full rebuild on change), FinanceTreeBuilder does
//! **incremental appends**: each transaction/alert creates new leaf nodes under
//! a daily root. Root and category nodes are idempotent — if a duplicate-ID
//! insert fails, it is silently skipped because the node already exists.
//!
//! Tree structure per day:
//! ```text
//! finance-daily-{YYYY-MM-DD}  (root, level 0, Section)
//!   ├── finance-cat-{date}-{category}  (level 1, Section)
//!   │   ├── finance-txn-{uuid}  (level 2, Text)  — "$45.20 in food"
//!   │   └── finance-alert-{date}-{category}  (level 2, Text)  — "Budget alert: food at 90%"
//! ```

use std::sync::Arc;

use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tracing::warn;
use uuid::Uuid;

use bus::{ContextUpdateReason, DomainEvent};

use context_engine::book_index::types::{SourceType, TreeNode, TreeNodeType};

use super::tree_builder_base::{run_event_loop, slugify, TreeBuilderCore};

pub struct FinanceTreeBuilder {
    core: TreeBuilderCore,
}

impl FinanceTreeBuilder {
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

    /// Event-driven subscriber loop. Listens for `TransactionRecorded` and
    /// `BudgetAlert` events and appends tree nodes for each.
    pub async fn run(
        self: Arc<Self>,
        rx: broadcast::Receiver<DomainEvent>,
        shutdown: CancellationToken,
    ) {
        run_event_loop("FinanceTreeBuilder", rx, shutdown, |event| {
            let this = Arc::clone(&self);
            async move {
                match event {
                    DomainEvent::TransactionRecorded {
                        category,
                        amount,
                        is_over_budget,
                    } => {
                        if let Err(e) = this
                            .handle_transaction(&category, amount, is_over_budget)
                            .await
                        {
                            warn!(
                                category = %category,
                                "FinanceTreeBuilder: failed to process transaction: {e}"
                            );
                        }
                    }
                    DomainEvent::BudgetAlert {
                        category,
                        spent,
                        limit,
                    } => {
                        if let Err(e) = this.handle_budget_alert(&category, spent, limit).await {
                            warn!(
                                category = %category,
                                "FinanceTreeBuilder: failed to process budget alert: {e}"
                            );
                        }
                    }
                    _ => {}
                }
            }
        })
        .await
    }

    /// Handle a recorded transaction: upsert the daily root and category nodes,
    /// then insert a unique transaction leaf node.
    pub async fn handle_transaction(
        &self,
        category: &str,
        amount: f64,
        is_over_budget: bool,
    ) -> common::Result<()> {
        let date = jiff::Timestamp::now().strftime("%Y-%m-%d").to_string();
        let daily_id = format!("finance-daily-{date}");
        let cat_id = format!("finance-cat-{date}-{}", slugify(category));
        let txn_id = format!("finance-txn-{}", Uuid::new_v4());

        tracing::debug!(
            category = %category,
            amount,
            txn_id = %txn_id,
            "FinanceTreeBuilder: processing transaction"
        );

        let nodes = build_transaction_nodes(
            &date,
            &daily_id,
            &cat_id,
            &txn_id,
            category,
            amount,
            is_over_budget,
        );

        self.core
            .persist_nodes(
                "FinanceTreeBuilder",
                &nodes,
                &daily_id,
                ContextUpdateReason::BudgetThresholdCrossed,
                Some(format!(
                    "Finance tree updated: {} new node(s) for {daily_id}",
                    nodes.len()
                )),
            )
            .await
    }

    /// Handle a budget alert: upsert the daily root and category nodes, then
    /// insert a unique alert leaf node.
    pub async fn handle_budget_alert(
        &self,
        category: &str,
        spent: f64,
        limit: f64,
    ) -> common::Result<()> {
        let date = jiff::Timestamp::now().strftime("%Y-%m-%d").to_string();
        let daily_id = format!("finance-daily-{date}");
        let cat_id = format!("finance-cat-{date}-{}", slugify(category));
        let alert_id = format!("finance-alert-{date}-{}", slugify(category));

        tracing::debug!(
            category = %category,
            spent,
            limit,
            alert_id = %alert_id,
            "FinanceTreeBuilder: processing budget alert"
        );

        let nodes = build_alert_nodes(&date, &daily_id, &cat_id, &alert_id, category, spent, limit);

        self.core
            .persist_nodes(
                "FinanceTreeBuilder",
                &nodes,
                &daily_id,
                ContextUpdateReason::BudgetThresholdCrossed,
                Some(format!(
                    "Finance tree updated: {} new node(s) for {daily_id}",
                    nodes.len()
                )),
            )
            .await
    }
}

// ---------------------------------------------------------------------------
// Pure node construction helpers (public for unit testing)
// ---------------------------------------------------------------------------

/// Build the nodes for a `TransactionRecorded` event.
///
/// Returns up to 3 nodes: daily root (level 0), category (level 1),
/// transaction leaf (level 2). Root and category nodes are idempotent by ID;
/// the transaction leaf uses a UUID so it is always unique.
pub fn build_transaction_nodes(
    date: &str,
    daily_id: &str,
    cat_id: &str,
    txn_id: &str,
    category: &str,
    amount: f64,
    is_over_budget: bool,
) -> Vec<TreeNode> {
    let over_budget_marker = if is_over_budget {
        " ⚠️ over budget"
    } else {
        ""
    };
    let txn_content = format!("${amount:.2} in {category}{over_budget_marker}");

    vec![
        // Level 0 — daily root
        TreeNode {
            id: daily_id.to_string(),
            parent_id: None,
            node_type: TreeNodeType::Section,
            content: daily_id.to_string(),
            title: Some(format!("Finance: {date}")),
            level: 0,
            source_type: SourceType::Finance,
            source_id: daily_id.to_string(),
            position: 0,
            metadata: None,
        },
        // Level 1 — category node
        TreeNode {
            id: cat_id.to_string(),
            parent_id: Some(daily_id.to_string()),
            node_type: TreeNodeType::Section,
            content: category.to_string(),
            title: Some(category.to_string()),
            level: 1,
            source_type: SourceType::Finance,
            source_id: daily_id.to_string(),
            position: 1,
            metadata: None,
        },
        // Level 2 — transaction leaf
        TreeNode {
            id: txn_id.to_string(),
            parent_id: Some(cat_id.to_string()),
            node_type: TreeNodeType::Text,
            content: txn_content,
            title: None,
            level: 2,
            source_type: SourceType::Finance,
            source_id: daily_id.to_string(),
            position: 2,
            metadata: None,
        },
    ]
}

/// Build the nodes for a `BudgetAlert` event.
///
/// Returns up to 3 nodes: daily root (level 0), category (level 1),
/// alert leaf (level 2). The alert node ID is deterministic per
/// (date, category) so repeated alerts on the same day for the same category
/// are deduplicated by the duplicate-ignore logic in `persist_nodes`.
pub fn build_alert_nodes(
    date: &str,
    daily_id: &str,
    cat_id: &str,
    alert_id: &str,
    category: &str,
    spent: f64,
    limit: f64,
) -> Vec<TreeNode> {
    let pct = if limit > 0.0 {
        (spent / limit * 100.0).round() as u64
    } else {
        100
    };
    let alert_content = format!("Budget alert: {category} at {pct}% (${spent:.2} of ${limit:.2})");

    vec![
        // Level 0 — daily root
        TreeNode {
            id: daily_id.to_string(),
            parent_id: None,
            node_type: TreeNodeType::Section,
            content: daily_id.to_string(),
            title: Some(format!("Finance: {date}")),
            level: 0,
            source_type: SourceType::Finance,
            source_id: daily_id.to_string(),
            position: 0,
            metadata: None,
        },
        // Level 1 — category node
        TreeNode {
            id: cat_id.to_string(),
            parent_id: Some(daily_id.to_string()),
            node_type: TreeNodeType::Section,
            content: category.to_string(),
            title: Some(category.to_string()),
            level: 1,
            source_type: SourceType::Finance,
            source_id: daily_id.to_string(),
            position: 1,
            metadata: None,
        },
        // Level 2 — alert leaf
        TreeNode {
            id: alert_id.to_string(),
            parent_id: Some(cat_id.to_string()),
            node_type: TreeNodeType::Text,
            content: alert_content,
            title: None,
            level: 2,
            source_type: SourceType::Finance,
            source_id: daily_id.to_string(),
            position: 2,
            metadata: None,
        },
    ]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::tree_builder_base::{compose_embedding_text, slugify};

    // --- slugify ---

    #[test]
    fn slugify_simple_word() {
        assert_eq!(slugify("food"), "food");
    }

    #[test]
    fn slugify_with_spaces() {
        assert_eq!(slugify("eating out"), "eating-out");
    }

    #[test]
    fn slugify_uppercase_and_specials() {
        assert_eq!(slugify("Coffee & Tea"), "coffee-tea");
    }

    #[test]
    fn slugify_collapses_multiple_separators() {
        assert_eq!(slugify("a  --  b"), "a-b");
    }

    // --- compose_embedding_text ---

    #[test]
    fn embedding_level_0_returns_title() {
        let node = TreeNode {
            id: "finance-daily-2024-01-15".into(),
            parent_id: None,
            node_type: TreeNodeType::Section,
            content: "finance-daily-2024-01-15".into(),
            title: Some("Finance: 2024-01-15".into()),
            level: 0,
            source_type: SourceType::Finance,
            source_id: "finance-daily-2024-01-15".into(),
            position: 0,
            metadata: None,
        };
        assert_eq!(compose_embedding_text(&node), "Finance: 2024-01-15");
    }

    #[test]
    fn embedding_level_1_returns_category_title() {
        let node = TreeNode {
            id: "finance-cat-2024-01-15-food".into(),
            parent_id: Some("finance-daily-2024-01-15".into()),
            node_type: TreeNodeType::Section,
            content: "food".into(),
            title: Some("food".into()),
            level: 1,
            source_type: SourceType::Finance,
            source_id: "finance-daily-2024-01-15".into(),
            position: 1,
            metadata: None,
        };
        assert_eq!(compose_embedding_text(&node), "food");
    }

    #[test]
    fn embedding_level_2_returns_content() {
        let node = TreeNode {
            id: "finance-txn-abc123".into(),
            parent_id: Some("finance-cat-2024-01-15-food".into()),
            node_type: TreeNodeType::Text,
            content: "$45.20 in food".into(),
            title: None,
            level: 2,
            source_type: SourceType::Finance,
            source_id: "finance-daily-2024-01-15".into(),
            position: 2,
            metadata: None,
        };
        assert_eq!(compose_embedding_text(&node), "$45.20 in food");
    }

    // --- build_transaction_nodes ---

    #[test]
    fn transaction_nodes_structure() {
        let nodes = build_transaction_nodes(
            "2024-01-15",
            "finance-daily-2024-01-15",
            "finance-cat-2024-01-15-food",
            "finance-txn-test-uuid",
            "food",
            45.20,
            false,
        );

        assert_eq!(nodes.len(), 3);

        // Root
        let root = &nodes[0];
        assert_eq!(root.id, "finance-daily-2024-01-15");
        assert_eq!(root.parent_id, None);
        assert_eq!(root.level, 0);
        assert!(matches!(root.node_type, TreeNodeType::Section));
        assert_eq!(root.title.as_deref(), Some("Finance: 2024-01-15"));

        // Category
        let cat = &nodes[1];
        assert_eq!(cat.id, "finance-cat-2024-01-15-food");
        assert_eq!(cat.parent_id.as_deref(), Some("finance-daily-2024-01-15"));
        assert_eq!(cat.level, 1);
        assert!(matches!(cat.node_type, TreeNodeType::Section));
        assert_eq!(cat.title.as_deref(), Some("food"));

        // Transaction leaf
        let txn = &nodes[2];
        assert_eq!(txn.id, "finance-txn-test-uuid");
        assert_eq!(
            txn.parent_id.as_deref(),
            Some("finance-cat-2024-01-15-food")
        );
        assert_eq!(txn.level, 2);
        assert!(matches!(txn.node_type, TreeNodeType::Text));
        assert_eq!(txn.content, "$45.20 in food");
        assert_eq!(txn.title, None);
    }

    #[test]
    fn transaction_nodes_over_budget_marker() {
        let nodes = build_transaction_nodes(
            "2024-01-15",
            "finance-daily-2024-01-15",
            "finance-cat-2024-01-15-food",
            "finance-txn-over",
            "food",
            120.00,
            true,
        );
        let leaf = &nodes[2];
        assert!(
            leaf.content.contains("over budget"),
            "Expected over budget marker in: {}",
            leaf.content
        );
    }

    #[test]
    fn transaction_nodes_all_have_finance_source_type() {
        let nodes = build_transaction_nodes(
            "2024-01-15",
            "finance-daily-2024-01-15",
            "finance-cat-2024-01-15-food",
            "finance-txn-x",
            "food",
            10.0,
            false,
        );
        for node in &nodes {
            assert_eq!(node.source_type.as_str(), "finance");
            assert_eq!(node.source_id, "finance-daily-2024-01-15");
        }
    }

    // --- build_alert_nodes ---

    #[test]
    fn alert_nodes_structure() {
        let nodes = build_alert_nodes(
            "2024-01-15",
            "finance-daily-2024-01-15",
            "finance-cat-2024-01-15-food",
            "finance-alert-2024-01-15-food",
            "food",
            90.0,
            100.0,
        );

        assert_eq!(nodes.len(), 3);

        let alert = &nodes[2];
        assert_eq!(alert.id, "finance-alert-2024-01-15-food");
        assert_eq!(
            alert.parent_id.as_deref(),
            Some("finance-cat-2024-01-15-food")
        );
        assert_eq!(alert.level, 2);
        assert!(matches!(alert.node_type, TreeNodeType::Text));
        assert!(
            alert.content.contains("90%"),
            "Expected 90% in: {}",
            alert.content
        );
        assert!(alert.content.contains("food"));
    }

    #[test]
    fn alert_nodes_zero_limit_is_100_pct() {
        let nodes = build_alert_nodes(
            "2024-01-15",
            "finance-daily-2024-01-15",
            "finance-cat-2024-01-15-misc",
            "finance-alert-2024-01-15-misc",
            "misc",
            50.0,
            0.0,
        );
        let alert = &nodes[2];
        assert!(
            alert.content.contains("100%"),
            "Expected 100% for zero limit: {}",
            alert.content
        );
    }

    #[test]
    fn alert_nodes_content_includes_amounts() {
        let nodes = build_alert_nodes(
            "2024-01-15",
            "finance-daily-2024-01-15",
            "finance-cat-2024-01-15-dining",
            "finance-alert-2024-01-15-dining",
            "dining",
            75.50,
            100.0,
        );
        let alert = &nodes[2];
        assert!(
            alert.content.contains("$75.50"),
            "content: {}",
            alert.content
        );
        assert!(
            alert.content.contains("$100.00"),
            "content: {}",
            alert.content
        );
    }
}
