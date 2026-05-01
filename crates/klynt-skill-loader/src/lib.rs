//! Klynt skill loader — extends `skill-system` with discovery + path-conditional + dynamic.

pub mod activator;
pub mod frontmatter;
pub mod replay;

mod discovery;
mod dynamic;
mod index;

pub use activator::{ActivationConfig, SkillActivator};
pub use discovery::sanitize_repo_id;
pub use frontmatter::{KlyntFrontmatter, Reference, ReferenceLoadMode};
pub use index::{DiscoveryRoots, IndexedSkill, SkillIndex, SkillSource};
