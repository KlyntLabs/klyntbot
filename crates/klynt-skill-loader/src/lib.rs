//! Klynt skill loader — extends `skill-system` with discovery + path-conditional + dynamic.

pub mod activator;
pub mod frontmatter;
pub mod replay;
pub mod url;

pub mod discovery;
mod dynamic;
mod index;

pub use activator::{ActivationConfig, SkillActivator};
pub use discovery::{McpResource, McpResourceClient, sanitize_repo_id, scan_mcp_server};
pub use frontmatter::{KlyntFrontmatter, Reference, ReferenceLoadMode};
pub use index::{DiscoveryRoots, IndexedSkill, SkillIndex, SkillSource};
pub use url::load_from_url;
