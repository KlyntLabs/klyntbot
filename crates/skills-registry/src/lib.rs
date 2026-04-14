//! skills-registry: resolve SkillSources into SkillPackages (fetch + parse).

pub mod fetcher;
pub mod package;
pub mod source;

pub use package::{ReferenceFile, SkillPackage, TemplateFile};
pub use source::{GitRef, SkillSource};
