//! Context source implementations for system prompt assembly.
//!
//! Each source implements `ContextSource` (defined in `context_engine`) and
//! provides a section of the system prompt. Caching is handled per-source.

pub mod bootstrap;
pub mod confidence;
pub mod goal;
pub mod identity;
pub mod memory;
pub mod skills;
pub mod todo;

pub use bootstrap::BootstrapSource;
pub use confidence::ConfidenceSource;
pub use goal::GoalSource;
pub use identity::IdentitySource;
pub use memory::MemorySource;
pub use skills::{SkillContentSource, SkillSummarySource};
pub use todo::TodoSource;
