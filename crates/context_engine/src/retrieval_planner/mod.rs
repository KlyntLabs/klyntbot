pub mod classifier;

use std::sync::Arc;

use common::Result;

pub use classifier::{classify_heuristic, classify_with_llm_fallback, QueryCategory};

use crate::book_index::{BookIndex, BookRetrievalConfig};
use crate::operators::formulator::{Decompose, Extract};
use crate::operators::reasoner::{GraphReasoning, SkylineRanker, TextRanker};
use crate::operators::selector::{FilterModal, FilterRange, SelectByEntity};
use crate::operators::synthesizer::{Map, Reduce, SubQueryExecutor};
use crate::operators::{Operator, OperatorLlm};

/// A plan consisting of a query category and a sequence of operators.
pub struct RetrievalPlan {
    pub category: QueryCategory,
    pub operators: Vec<Box<dyn Operator>>,
}

/// The retrieval planner classifies queries and generates tailored operator pipelines.
pub struct RetrievalPlanner {
    pub book_index: Arc<BookIndex>,
    llm: Arc<dyn OperatorLlm>,
    _config: BookRetrievalConfig,
}

impl RetrievalPlanner {
    pub fn new(
        book_index: Arc<BookIndex>,
        llm: Arc<dyn OperatorLlm>,
        config: BookRetrievalConfig,
    ) -> Self {
        Self {
            book_index,
            llm,
            _config: config,
        }
    }

    pub fn llm(&self) -> Arc<dyn OperatorLlm> {
        self.llm.clone()
    }

    pub async fn plan(&self, query: &str) -> Result<RetrievalPlan> {
        let category = classify_with_llm_fallback(query, self.llm.as_ref()).await;
        let operators = self.generate_plan(query, &category);
        Ok(RetrievalPlan {
            category,
            operators,
        })
    }

    fn generate_plan(&self, _query: &str, category: &QueryCategory) -> Vec<Box<dyn Operator>> {
        match category {
            QueryCategory::SingleHop => vec![
                Box::new(Extract::new()),
                Box::new(SelectByEntity::new()),
                Box::new(GraphReasoning::new()),
                Box::new(TextRanker::new()),
                Box::new(SkylineRanker::new()),
                Box::new(Reduce::new()),
            ],
            QueryCategory::MultiHop => vec![
                Box::new(Decompose::new()),
                Box::new(SubQueryExecutor::new(|| {
                    vec![
                        Box::new(Extract::new()) as Box<dyn Operator>,
                        Box::new(SelectByEntity::new()),
                        Box::new(GraphReasoning::new()),
                        Box::new(TextRanker::new()),
                        Box::new(SkylineRanker::new()),
                    ]
                })),
                Box::new(Map::new()),
                Box::new(Reduce::new()),
            ],
            QueryCategory::GlobalAggregation => vec![
                Box::new(FilterModal::from_query("")),
                Box::new(FilterRange::from_query("")),
                Box::new(Map::new()),
                Box::new(Reduce::new()),
            ],
            QueryCategory::PassThrough => vec![],
        }
    }
}
