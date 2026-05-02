use crate::frontmatter::KlyntFrontmatter;
use crate::index::{IndexedSkill, SkillIndex};
use common::{ConfigError, KlyntbotError, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};
use lru::LruCache;
use std::collections::HashSet;
use std::num::NonZeroUsize;
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct ActivationConfig {
    pub always_activate: Vec<String>,
    pub never_activate: Vec<String>,
    pub max_active_skills: usize,
}

impl ActivationConfig {
    pub fn from_coding_config(c: &config::schema::CodingSkillsConfig) -> Self {
        Self {
            always_activate: c.always_activate.clone(),
            never_activate: c.never_activate.clone(),
            max_active_skills: c.max_active_skills.try_into().unwrap_or(30),
        }
    }
}

pub(crate) struct ConditionalSkill {
    pub name: String,
    pub glob_set: GlobSet,
    #[allow(dead_code)]
    pub source_path: std::path::PathBuf,
}

pub struct SkillActivator {
    index: SkillIndex,
    config: ActivationConfig,
    conditionals: Vec<ConditionalSkill>,
    active: HashSet<String>,
    walker: crate::dynamic::DynamicWalker,
    /// Cache path -> matching skill names (independent of active set).
    /// Eliminates redundant glob matching when the same paths are touched
    /// repeatedly within a session.
    path_match_cache: LruCache<std::path::PathBuf, Vec<String>>,
}

impl SkillActivator {
    pub fn new(index: SkillIndex, config: ActivationConfig) -> Result<Self> {
        let never: HashSet<&str> = config.never_activate.iter().map(String::as_str).collect();
        let mut conditionals = Vec::new();
        let mut active = HashSet::new();

        for name in &config.always_activate {
            if !never.contains(name.as_str()) && index.get(name).is_some() {
                active.insert(name.clone());
            }
        }

        for (name, skill) in index.iter() {
            if never.contains(name.as_str()) || skill.frontmatter.paths.is_empty() {
                continue;
            }
            conditionals.push(build_conditional_skill(name, skill)?);
        }

        Ok(Self {
            index,
            config,
            conditionals,
            active,
            walker: crate::dynamic::DynamicWalker::new(),
            path_match_cache: LruCache::new(NonZeroUsize::new(256).unwrap()),
        })
    }

    /// Returns names of skills newly-activated by this touch (empty if none).
    pub fn touch_path(&mut self, path: &Path) -> Result<Vec<String>> {
        // Check LRU cache for path -> matching skill names.
        let matches = if let Some(cached) = self.path_match_cache.get(path) {
            cached.clone()
        } else {
            let mut m = Vec::new();
            for c in &self.conditionals {
                if c.glob_set.is_match(path) {
                    m.push(c.name.clone());
                }
            }
            self.path_match_cache.put(path.to_path_buf(), m.clone());
            m
        };

        let mut newly = Vec::new();
        for name in matches {
            if self.active.contains(&name) {
                continue;
            }
            if self.config.max_active_skills > 0
                && self.active.len() >= self.config.max_active_skills
            {
                break;
            }
            self.active.insert(name.clone());
            newly.push(name);
        }
        Ok(newly)
    }

    /// Touch a path, dynamically discover new skill dirs above it, then activate.
    pub fn touch_path_with_discovery(
        &mut self,
        path: &Path,
        roots: &crate::index::DiscoveryRoots,
    ) -> Result<Vec<String>> {
        let cwd_boundary = roots.repo_root.as_deref().unwrap_or(roots.cwd.as_path());
        let newly_indexed = self
            .walker
            .discover_above(path, cwd_boundary, &mut self.index)?;
        for name in &newly_indexed {
            if let Some(skill) = self.index.get(name) {
                if skill.frontmatter.paths.is_empty() {
                    continue;
                }
                if let Ok(cs) = build_conditional_skill(name, skill) {
                    self.conditionals.push(cs);
                }
            }
        }
        // New conditionals added → prior path→match cache is stale.
        if !newly_indexed.is_empty() {
            self.path_match_cache.clear();
        }
        self.touch_path(path)
    }

    pub fn active_set(&self) -> &HashSet<String> {
        &self.active
    }

    pub fn lookup(&self, name: &str) -> Option<&IndexedSkill> {
        self.index.get(name)
    }

    pub fn frontmatter(&self, name: &str) -> Option<&KlyntFrontmatter> {
        self.index.get(name).map(|s| &s.frontmatter)
    }

    pub fn iter_index(&self) -> impl Iterator<Item = (&String, &IndexedSkill)> {
        self.index.iter()
    }

    pub fn dynamic_seen_dirs_len(&self) -> usize {
        self.walker.seen_dirs_len()
    }
}

fn build_conditional_skill(name: &str, skill: &IndexedSkill) -> Result<ConditionalSkill> {
    let mut builder = GlobSetBuilder::new();
    for pat in &skill.frontmatter.paths {
        let glob = Glob::new(pat).map_err(|e| {
            KlyntbotError::Config(ConfigError::Invalid(format!(
                "invalid path glob `{pat}` in skill `{name}`: {e}"
            )))
        })?;
        builder.add(glob);
    }
    let glob_set = builder.build().map_err(|e| {
        KlyntbotError::Config(ConfigError::Invalid(format!(
            "globset build for `{name}`: {e}"
        )))
    })?;
    Ok(ConditionalSkill {
        name: name.to_string(),
        glob_set,
        source_path: skill.source_path.clone(),
    })
}
