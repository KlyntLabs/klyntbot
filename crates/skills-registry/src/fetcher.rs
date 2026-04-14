//! Fetches SkillPackages from a SkillSource. Stub for Task 5.

use common::Result;
use crate::{SkillPackage, SkillSource};

pub struct Fetcher;

impl Fetcher {
    pub fn new() -> Self {
        Self
    }

    pub async fn fetch(&self, _source: &SkillSource) -> Result<SkillPackage> {
        todo!("Task 5")
    }
}
