//! Context source implementations for system prompt assembly.
//!
//! Each source implements `ContextSource` (defined in `context_engine`) and
//! provides a section of the system prompt. Caching is handled per-source.

pub mod annotation;
pub mod area;
pub mod bootstrap;
pub mod confidence;
pub mod identity;
pub mod page_context;
pub mod persona;
pub mod productivity;
pub mod project;
pub mod session_context_source;
pub mod todo;

pub use annotation::AnnotationContextSource;
pub use area::AreaSource;
pub use bootstrap::BootstrapSource;
pub use confidence::ConfidenceSource;
pub use identity::IdentitySource;
pub use page_context::PageContextSource;
pub use persona::PersonaContextSource;
pub use productivity::ProductivityContextSource;
pub use project::ProjectContextSource;
pub use session_context_source::SessionContextSource;
pub use todo::TodoSource;
