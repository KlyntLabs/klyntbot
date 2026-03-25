//! Trait implementation adapters — dependency inversion bridges.
//!
//! Each module implements a trait defined in a lower layer (`tools`, `feature-*`,
//! `context_engine`, `cognitive`) with concrete logic that lives at L5.

pub mod agent_task;
pub mod book_index_backfill;
pub mod book_index_entity_extractor;
pub mod book_index_skill_builder;
pub mod book_index_task_builder;
pub mod book_index_updater;
pub mod book_index_wiring;
pub mod cognitive_embedder;
pub mod cognitive_handlers;
pub mod conversation_recall;
pub mod cron;
pub mod finance;
pub mod learning;
pub mod llm_summary;
pub mod mirror_handlers;
pub mod note_embedding;
pub mod productivity;
pub mod progress;
pub mod query_rewriter;
pub mod task_embedding;
