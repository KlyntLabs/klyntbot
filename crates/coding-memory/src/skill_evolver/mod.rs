//! Project skill evolver — Detect, Synthesize, Write, Journal, Supersede.

pub mod detect;
pub mod supersede;
pub mod write;

pub use detect::{detect_candidates, WorkflowPatternCandidate};
pub use supersede::supersede_outdated_versions;
pub use write::{write_skill_md, JournalArgs, SkillWriteOutcome};
