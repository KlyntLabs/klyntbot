use async_trait::async_trait;
use common::Result;

use super::{Operator, OperatorContext, OperatorType};

/// Map: per-node LLM call for partial answer.
pub struct Map;

impl Default for Map {
    fn default() -> Self {
        Self::new()
    }
}

impl Map {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Operator for Map {
    fn name(&self) -> &str {
        "Map"
    }
    fn operator_type(&self) -> OperatorType {
        OperatorType::Synthesizer
    }
    async fn execute(&self, ctx: &mut OperatorContext) -> Result<()> {
        let nodes_to_map: Vec<_> = ctx
            .working_set
            .iter()
            .take(ctx.max_map_nodes)
            .cloned()
            .collect();

        let llm = ctx.llm.clone();
        let query = ctx.query.clone();

        let mut handles = Vec::new();
        for scored_node in nodes_to_map {
            let llm = llm.clone();
            let q = query.clone();
            handles.push(tokio::spawn(async move {
                let prompt = format!(
                    "Based on this content, extract information relevant to the query.\n\n\
                     Query: \"{}\"\n\n\
                     Content:\n{}",
                    q, scored_node.node.content
                );
                llm.complete(
                    "You extract relevant information from document sections. Be concise.",
                    &prompt,
                )
                .await
            }));
        }

        for handle in handles {
            match handle.await {
                Ok(Ok(answer)) => ctx.partial_answers.push(answer),
                Ok(Err(e)) => tracing::warn!("Map partial failed: {e}"),
                Err(e) => tracing::warn!("Map task panicked: {e}"),
            }
        }

        Ok(())
    }
}

/// Reduce: aggregate partial answers into final response.
pub struct Reduce;

impl Default for Reduce {
    fn default() -> Self {
        Self::new()
    }
}

impl Reduce {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Operator for Reduce {
    fn name(&self) -> &str {
        "Reduce"
    }
    fn operator_type(&self) -> OperatorType {
        OperatorType::Synthesizer
    }
    async fn execute(&self, ctx: &mut OperatorContext) -> Result<()> {
        if ctx.partial_answers.is_empty() && ctx.working_set.is_empty() {
            return Ok(());
        }

        // If we have partial answers (from Map), synthesize them
        if !ctx.partial_answers.is_empty() {
            let partials = ctx.partial_answers.join("\n---\n");
            let prompt = format!(
                "Synthesize these partial answers into a coherent response to the query.\n\n\
                 Query: \"{}\"\n\n\
                 Partial answers:\n{}",
                ctx.query, partials
            );

            let answer = ctx
                .llm
                .complete(
                    "You synthesize multiple partial answers into a coherent response.",
                    &prompt,
                )
                .await?;
            ctx.final_answer = Some(answer);
        }

        Ok(())
    }
}

/// SubQueryExecutor: runs a SingleHop pipeline per sub-query in parallel.
pub struct SubQueryExecutor {
    /// Factory function returning operators for a single-hop query.
    pipeline_factory: Box<dyn Fn() -> Vec<Box<dyn Operator>> + Send + Sync>,
}

impl SubQueryExecutor {
    pub fn new(factory: impl Fn() -> Vec<Box<dyn Operator>> + Send + Sync + 'static) -> Self {
        Self {
            pipeline_factory: Box::new(factory),
        }
    }
}

#[async_trait]
impl Operator for SubQueryExecutor {
    fn name(&self) -> &str {
        "SubQueryExecutor"
    }
    fn operator_type(&self) -> OperatorType {
        OperatorType::Synthesizer
    }
    async fn execute(&self, ctx: &mut OperatorContext) -> Result<()> {
        // For each sub-query, run a pipeline and collect results
        // We run them sequentially since they share the BookIndex
        for sub_query in ctx.sub_queries.clone() {
            let operators = (self.pipeline_factory)();
            let mut sub_ctx = OperatorContext::new(
                &sub_query,
                ctx.book_index.clone(),
                ctx.llm.clone(),
                ctx.max_nodes,
                ctx.max_map_nodes,
                ctx.operator_timeout.as_millis() as u64,
            );

            super::execute_pipeline(&operators, &mut sub_ctx).await?;

            // Merge results into parent context
            for node in sub_ctx.working_set {
                if !ctx.working_set.iter().any(|n| n.node.id == node.node.id) {
                    ctx.working_set.push(node);
                }
            }
            ctx.partial_answers.extend(sub_ctx.partial_answers);
        }

        ctx.working_set.truncate(ctx.max_nodes);
        Ok(())
    }
}
