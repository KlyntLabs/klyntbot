//! Trait implementation adapters — dependency inversion bridges.
//!
//! Each module implements a trait defined in a lower layer (`tools`, `feature-*`,
//! `context_engine`, `cognitive`) with concrete logic that lives at L5.

pub mod agent_task;
pub mod book_index_wiring;
pub mod cognitive_embedder;
pub mod cognitive_handlers;
pub mod conversation_recall;
pub mod cron;
pub mod finance;
pub mod learning;
pub mod llm_summary;
pub mod note_embedding;
pub mod productivity;
pub mod progress;
pub mod task_embedding;
