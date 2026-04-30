use crate::decision::Decision;
use std::path::Path;

pub struct Policy;

impl Policy {
    pub fn empty() -> Self {
        Self
    }
    pub fn load_from_dir(_path: &Path) -> Result<Self, std::io::Error> {
        Ok(Self)
    }
    pub fn eval(&self, _argv: &[&str], _cwd: Option<&Path>) -> Decision {
        Decision::FallThrough
    }
    pub fn append_session_allow_prefix(&self, _prefixes: &[&str]) {}
}
