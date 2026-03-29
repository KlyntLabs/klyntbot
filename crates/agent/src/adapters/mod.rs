//! Trait implementation adapters — dependency inversion bridges.
//!
//! Each module implements a trait defined in a lower layer (`tools`, `feature-*`,
//! `context_engine`, `cognitive`) with concrete logic that lives at L5.

pub mod agent_task;
pub mod book_index_skill_builder;
pub mod book_index_task_builder;
pub mod cognitive_embedder;
pub mod cognitive_handlers;
pub mod community_builder;
pub mod community_search;
pub mod conversation_recall;
pub mod cron;
pub mod entity_tree_linker;
pub mod finance;
pub mod finance_tree_builder;
pub mod learning;
pub mod learning_tree_builder;
pub mod llm_summary;
pub mod mirror_handlers;
pub mod note_embedding;
pub mod note_tree_builder;
pub mod okr_tree_builder;
pub mod productivity;
pub mod productivity_tree_builder;
pub mod progress;
pub mod query_rewriter;
pub mod task_embedding;
pub mod task_tree_builder;
pub mod tree_node_search;
