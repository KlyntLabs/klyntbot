//! skills-registry: resolve SkillSources into SkillPackages (fetch + parse).

pub mod diff;
pub mod fetcher;
pub mod package;
pub mod source;
pub mod updates;

pub use package::{ReferenceFile, SkillPackage, TemplateFile};
pub use source::{GitRef, SkillSource};
pub use updates::{AvailableVersion, UpdatesFetcher};
