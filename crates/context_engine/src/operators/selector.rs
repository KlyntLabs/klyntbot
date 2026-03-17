use async_trait::async_trait;
use common::Result;

use super::{Operator, OperatorContext, OperatorType};
use crate::book_index::{ScoredNode, TreeNodeType};

/// FilterModal: filter working_set by TreeNodeType.
pub struct FilterModal {
    allowed: Vec<String>,
}

impl FilterModal {
    pub fn new(allowed: Vec<String>) -> Self {
        Self { allowed }
    }

    pub fn from_query(_query: &str) -> Self {
        // Default: allow all types
        Self {
            allowed: vec![
                "Section".into(),
                "Text".into(),
                "Table".into(),
                "Code".into(),
                "Task".into(),
                "ListItem".into(),
            ],
        }
    }
}

#[async_trait]
impl Operator for FilterModal {
    fn name(&self) -> &str {
        "FilterModal"
    }
    fn operator_type(&self) -> OperatorType {
        OperatorType::Selector
    }
    async fn execute(&self, ctx: &mut OperatorContext) -> Result<()> {
        ctx.working_set.retain(|n| {
            self.allowed
                .contains(&n.node.node_type.as_str().to_string())
        });
        Ok(())
    }
}

/// FilterRange: filter by source_id or level range.
pub struct FilterRange {
    max_level: Option<u32>,
    source_ids: Option<Vec<String>>,
}

impl FilterRange {
    pub fn new(max_level: Option<u32>, source_ids: Option<Vec<String>>) -> Self {
        Self {
            max_level,
            source_ids,
        }
    }

    pub fn from_query(_query: &str) -> Self {
        Self {
            max_level: None,
            source_ids: None,
        }
    }
}

#[async_trait]
impl Operator for FilterRange {
    fn name(&self) -> &str {
        "FilterRange"
    }
    fn operator_type(&self) -> OperatorType {
        OperatorType::Selector
    }
    async fn execute(&self, ctx: &mut OperatorContext) -> Result<()> {
        if let Some(max_level) = self.max_level {
            ctx.working_set.retain(|n| n.node.level <= max_level);
        }
        if let Some(ref source_ids) = self.source_ids {
            ctx.working_set
                .retain(|n| source_ids.contains(&n.node.source_id));
        }
        Ok(())
    }
}

/// SelectByEntity: use GT-Link to navigate from entities to tree nodes.
pub struct SelectByEntity;

impl Default for SelectByEntity {
    fn default() -> Self {
        Self::new()
    }
}

impl SelectByEntity {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Operator for SelectByEntity {
    fn name(&self) -> &str {
        "SelectByEntity"
    }
    fn operator_type(&self) -> OperatorType {
        OperatorType::Selector
    }
    async fn execute(&self, ctx: &mut OperatorContext) -> Result<()> {
        let mut nodes = Vec::new();
        for entity in &ctx.extracted_entities {
            if let Ok(linked) = ctx
                .book_index
                .gt_link_repo()
                .get_linked_nodes(&entity.id)
                .await
            {
                for tree_node in linked {
                    // Expand to subtree for section nodes
                    if matches!(tree_node.node_type, TreeNodeType::Section) {
                        if let Ok(subtree) =
                            ctx.book_index.tree_repo().get_subtree(&tree_node.id).await
                        {
                            for child in subtree {
                                if !nodes.iter().any(|n: &ScoredNode| n.node.id == child.id) {
                                    nodes.push(ScoredNode {
                                        node: child,
                                        graph_score: 0.0,
                                        text_score: 0.0,
                                        combined: 0.0,
                                    });
                                }
                            }
                        }
                    } else if !nodes.iter().any(|n| n.node.id == tree_node.id) {
                        nodes.push(ScoredNode {
                            node: tree_node,
                            graph_score: 0.0,
                            text_score: 0.0,
                            combined: 0.0,
                        });
                    }
                }
            }
        }

        // If entities yielded nodes, replace working set; otherwise keep existing
        if !nodes.is_empty() {
            nodes.truncate(ctx.max_nodes);
            ctx.working_set = nodes;
        }
        Ok(())
    }
}

/// SelectBySection: LLM picks relevant sections from root children, expand subtrees.
pub struct SelectBySection;

impl Default for SelectBySection {
    fn default() -> Self {
        Self::new()
    }
}

impl SelectBySection {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Operator for SelectBySection {
    fn name(&self) -> &str {
        "SelectBySection"
    }
    fn operator_type(&self) -> OperatorType {
        OperatorType::Selector
    }
    async fn execute(&self, ctx: &mut OperatorContext) -> Result<()> {
        // Get all root sections
        let roots = ctx
            .book_index
            .tree_repo()
            .get_root_sections(&crate::book_index::SourceType::Note)
            .await?;
        if roots.is_empty() {
            return Ok(());
        }

        // Build section summary for LLM
        let section_list: String = roots
            .iter()
            .enumerate()
            .map(|(i, r)| {
                format!(
                    "{}. {} ({})",
                    i,
                    r.title.as_deref().unwrap_or(&r.content),
                    r.source_id
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = format!(
            "Given the query: \"{}\"\n\nWhich sections are relevant? List the numbers, comma-separated.\n\nSections:\n{}",
            ctx.query, section_list
        );

        let response = ctx
            .llm
            .complete(
                "You select relevant document sections. Reply with comma-separated numbers only.",
                &prompt,
            )
            .await?;

        // Parse response for section indices
        let selected_indices: Vec<usize> = response
            .split(',')
            .filter_map(|s| s.trim().parse::<usize>().ok())
            .filter(|&i| i < roots.len())
            .collect();

        let mut nodes = Vec::new();
        for idx in selected_indices {
            if let Ok(subtree) = ctx.book_index.tree_repo().get_subtree(&roots[idx].id).await {
                for child in subtree {
                    if !nodes.iter().any(|n: &ScoredNode| n.node.id == child.id) {
                        nodes.push(ScoredNode {
                            node: child,
                            graph_score: 0.0,
                            text_score: 0.0,
                            combined: 0.0,
                        });
                    }
                }
            }
        }

        if !nodes.is_empty() {
            nodes.truncate(ctx.max_nodes);
            ctx.working_set = nodes;
        }
        Ok(())
    }
}
