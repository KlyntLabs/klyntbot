use async_trait::async_trait;
use common::Result;

use super::{Operator, OperatorContext, OperatorType};
use crate::book_index::EntityInfo;

/// Decompose: break query into sub-queries via LLM.
pub struct Decompose;

impl Default for Decompose {
    fn default() -> Self {
        Self::new()
    }
}

impl Decompose {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Operator for Decompose {
    fn name(&self) -> &str {
        "Decompose"
    }
    fn operator_type(&self) -> OperatorType {
        OperatorType::Formulator
    }
    async fn execute(&self, ctx: &mut OperatorContext) -> Result<()> {
        let prompt = format!(
            "Break the following query into 2-4 independent sub-queries that can be answered separately.\n\
             Return one sub-query per line, no numbering.\n\n\
             Query: \"{}\"",
            ctx.query
        );

        let response = ctx
            .llm
            .complete(
                "You decompose complex queries into simpler sub-queries. Return one per line.",
                &prompt,
            )
            .await?;

        ctx.sub_queries = response
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .take(4)
            .collect();

        Ok(())
    }
}

/// Extract: extract entities from query, link to graph.
pub struct Extract;

impl Default for Extract {
    fn default() -> Self {
        Self::new()
    }
}

impl Extract {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Operator for Extract {
    fn name(&self) -> &str {
        "Extract"
    }
    fn operator_type(&self) -> OperatorType {
        OperatorType::Formulator
    }
    async fn execute(&self, ctx: &mut OperatorContext) -> Result<()> {
        let prompt = format!(
            "Extract key entity names from this query. Return one entity name per line, no explanation.\n\n\
             Query: \"{}\"",
            ctx.query
        );

        let response = ctx
            .llm
            .complete(
                "You extract entity names from queries. Return one name per line.",
                &prompt,
            )
            .await?;

        let entity_names: Vec<String> = response
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();

        // Look up each entity in the graph
        for name in &entity_names {
            if let Ok(matches) = ctx.book_index.entity_repo().find_by_name(name).await {
                for m in matches {
                    if !ctx.extracted_entities.iter().any(|e| e.id == m.id) {
                        ctx.extracted_entities.push(EntityInfo {
                            id: m.id,
                            name: m.name,
                            entity_type: m.entity_type,
                        });
                    }
                }
            }
        }

        Ok(())
    }
}
