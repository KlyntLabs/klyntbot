pub mod formulator;
pub mod reasoner;
pub mod selector;
pub mod synthesizer;

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use common::Result;

use crate::book_index::{BookIndex, EntityInfo, ScoredNode};

/// Local trait for LLM calls within the operator pipeline.
/// Avoids a direct dependency on `providers` crate (layer violation).
#[async_trait]
pub trait OperatorLlm: Send + Sync {
    async fn complete(&self, system: &str, prompt: &str) -> Result<String>;
}

#[derive(Debug, Clone, PartialEq)]
pub enum OperatorType {
    Formulator,
    Selector,
    Reasoner,
    Synthesizer,
}

/// Core trait for composable retrieval operators.
#[async_trait]
pub trait Operator: Send + Sync {
    fn name(&self) -> &str;
    fn operator_type(&self) -> OperatorType;
    async fn execute(&self, ctx: &mut OperatorContext) -> Result<()>;
}

/// Mutable pipeline state passed through operators.
pub struct OperatorContext {
    pub query: String,
    pub sub_queries: Vec<String>,
    pub extracted_entities: Vec<EntityInfo>,

    pub working_set: Vec<ScoredNode>,

    pub partial_answers: Vec<String>,
    pub final_answer: Option<String>,

    pub book_index: Arc<BookIndex>,
    pub llm: Arc<dyn OperatorLlm>,

    pub max_nodes: usize,
    pub max_map_nodes: usize,
    pub token_budget: usize,
    pub operator_timeout: Duration,
}

impl OperatorContext {
    pub fn new(
        query: &str,
        book_index: Arc<BookIndex>,
        llm: Arc<dyn OperatorLlm>,
        max_nodes: usize,
        max_map_nodes: usize,
        operator_timeout_ms: u64,
    ) -> Self {
        Self {
            query: query.to_string(),
            sub_queries: Vec::new(),
            extracted_entities: Vec::new(),
            working_set: Vec::new(),
            partial_answers: Vec::new(),
            final_answer: None,
            book_index,
            llm,
            max_nodes,
            max_map_nodes,
            token_budget: 4096,
            operator_timeout: Duration::from_millis(operator_timeout_ms),
        }
    }
}

/// Execute a pipeline of operators with per-operator timeout.
pub async fn execute_pipeline(
    operators: &[Box<dyn Operator>],
    ctx: &mut OperatorContext,
) -> Result<()> {
    for op in operators {
        match tokio::time::timeout(ctx.operator_timeout, op.execute(ctx)).await {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => {
                tracing::warn!("Operator '{}' failed: {e}", op.name());
                break;
            }
            Err(_) => {
                tracing::warn!("Operator '{}' timed out", op.name());
                break;
            }
        }
    }
    Ok(())
}
