//! Context source implementations for system prompt assembly.
//!
//! Each source implements `ContextSource` (defined in `context_engine`) and
//! provides a section of the system prompt. Caching is handled per-source.

pub mod agent;
pub mod area;
pub mod bootstrap;
pub mod confidence;
pub mod identity;
pub mod memory;
pub mod page_context;
pub mod persona;
pub mod skills;
pub mod todo;

pub use agent::AgentContextSource;
pub use area::AreaSource;
pub use bootstrap::BootstrapSource;
pub use confidence::ConfidenceSource;
pub use identity::IdentitySource;
pub use memory::MemorySource;
pub use page_context::PageContextSource;
pub use persona::PersonaContextSource;
pub use skills::{SkillContentSource, SkillSummarySource};
pub use todo::TodoSource;
