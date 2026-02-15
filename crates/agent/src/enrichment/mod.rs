//! Smart Enrichment Engine
//!
//! Auto-infers priority, predicts duration, and suggests due dates for tasks.
//! Implements `EnrichmentHandler` (defined in tools crate) via dependency inversion.
//!
//! Submodules:
//! - `engine` — orchestrator that coordinates individual enrichment strategies
//! - `priority` — keyword/context-based priority inference
//! - `duration` — heuristic duration prediction from title, tags, and historical data
//! - `scheduling` — due-date suggestion based on workload and calendar availability

pub mod duration;
pub mod engine;
pub mod priority;
pub mod scheduling;

pub use engine::EnrichmentEngine;
