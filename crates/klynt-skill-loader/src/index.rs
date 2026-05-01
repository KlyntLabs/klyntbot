use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum SkillSource {
    User,
    Project,
    ReforgePrivate,
    ReforgeTeam,
}

impl SkillSource {
    /// Lower priority loses to higher priority on name collision.
    pub fn priority(self) -> u8 {
        match self {
            SkillSource::User => 0,
            SkillSource::ReforgePrivate => 1,
            SkillSource::ReforgeTeam => 2,
            SkillSource::Project => 3,
        }
    }
}

#[derive(Debug, Clone)]
pub struct IndexedSkill {
    pub frontmatter: crate::frontmatter::KlyntFrontmatter,
    pub source: SkillSource,
    pub source_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct DiscoveryRoots {
    pub klyntbot_home: PathBuf,
    pub repo_id: Option<String>,
    pub repo_root: Option<PathBuf>,
    pub cwd: PathBuf,
}

#[derive(Debug, Default, Clone)]
pub struct SkillIndex {
    entries: HashMap<String, IndexedSkill>,
}

impl SkillIndex {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn get(&self, name: &str) -> Option<&IndexedSkill> {
        self.entries.get(name)
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn iter(&self) -> impl Iterator<Item = (&String, &IndexedSkill)> {
        self.entries.iter()
    }
    pub(crate) fn insert(&mut self, name: String, skill: IndexedSkill) {
        match self.entries.get(&name) {
            Some(existing) if existing.source.priority() >= skill.source.priority() => {
                tracing::debug!(
                    name = %name,
                    existing = ?existing.source,
                    incoming = ?skill.source,
                    "keeping higher-priority skill"
                );
            }
            _ => {
                self.entries.insert(name, skill);
            }
        }
    }
    pub(crate) fn merge(&mut self, other: SkillIndex) {
        for (name, skill) in other.entries {
            self.insert(name, skill);
        }
    }

    pub fn insert_for_test(
        &mut self,
        name: String,
        frontmatter: crate::frontmatter::KlyntFrontmatter,
        source: SkillSource,
        source_path: PathBuf,
    ) {
        self.entries.insert(
            name,
            IndexedSkill {
                frontmatter,
                source,
                source_path,
            },
        );
    }

    pub(crate) fn insert_for_test_or_dynamic(&mut self, name: String, skill: IndexedSkill) {
        self.entries.insert(name, skill);
    }
}
