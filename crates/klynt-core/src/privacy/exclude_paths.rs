use globset::{Glob, GlobSet, GlobSetBuilder};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PrivacyError {
    #[error("invalid glob: {0}")]
    Glob(#[from] globset::Error),
}

pub struct PrivacyGuard {
    set: GlobSet,
    raw_patterns: Vec<String>,
}

impl PrivacyGuard {
    pub fn from_globs(globs: &[&str]) -> Result<Self, PrivacyError> {
        let mut b = GlobSetBuilder::new();
        for g in globs {
            b.add(Glob::new(g)?);
        }
        Ok(Self {
            set: b.build()?,
            raw_patterns: globs.iter().map(|s| s.to_string()).collect(),
        })
    }
    pub fn is_excluded(&self, path: &Path) -> bool {
        self.set.is_match(path)
    }
    pub fn bash_command_touches_excluded(&self, cmd: &str) -> bool {
        cmd.split(|c: char| c.is_whitespace() || c == '=' || c == '"' || c == '\'')
            .any(|tok| !tok.is_empty() && self.is_excluded(Path::new(tok)))
    }
    pub fn raw_patterns(&self) -> &[String] {
        &self.raw_patterns
    }
}
