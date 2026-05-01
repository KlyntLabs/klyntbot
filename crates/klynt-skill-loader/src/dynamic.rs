use crate::frontmatter::KlyntFrontmatter;
use crate::index::{IndexedSkill, SkillIndex, SkillSource};
use common::{ConfigError, KlyntbotError, Result};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const DEFAULT_DIRS_TO_SKIP: &[&str] = &[".git", "target", "node_modules", "dist", "build"];

pub(crate) struct DynamicWalker {
    seen_dirs: HashSet<PathBuf>,
    dirs_to_skip: HashSet<String>,
}

impl DynamicWalker {
    pub fn new() -> Self {
        Self {
            seen_dirs: HashSet::new(),
            dirs_to_skip: DEFAULT_DIRS_TO_SKIP.iter().map(|s| s.to_string()).collect(),
        }
    }

    pub fn seen_dirs_len(&self) -> usize {
        self.seen_dirs.len()
    }

    /// Walk from `path` upward to `cwd_boundary` looking for new
    /// `.klyntbot/skills/` directories. Newly-found skills are inserted
    /// into the existing `index` with source `Project`.
    pub fn discover_above(
        &mut self,
        path: &Path,
        cwd_boundary: &Path,
        index: &mut SkillIndex,
    ) -> Result<Vec<String>> {
        let mut newly = Vec::new();
        let start = if path.is_file() {
            path.parent().unwrap_or(cwd_boundary)
        } else {
            path
        };
        let mut current = start;
        loop {
            if !current.starts_with(cwd_boundary) {
                break;
            }
            if let Some(name) = current.file_name().and_then(|s| s.to_str()) {
                if self.dirs_to_skip.contains(name) {
                    break;
                }
            }
            self.try_scan_skills_dir(&current.join(".klyntbot/skills"), index, &mut newly)?;
            self.seen_dirs.insert(current.to_path_buf());
            match current.parent() {
                Some(p) if p != current => current = p,
                _ => break,
            }
            if current == cwd_boundary {
                self.try_scan_skills_dir(&current.join(".klyntbot/skills"), index, &mut newly)?;
                break;
            }
        }
        Ok(newly)
    }

    fn try_scan_skills_dir(
        &mut self,
        candidate: &Path,
        index: &mut SkillIndex,
        newly: &mut Vec<String>,
    ) -> Result<()> {
        if !candidate.is_dir() || self.seen_dirs.contains(candidate) {
            return Ok(());
        }
        self.seen_dirs.insert(candidate.to_path_buf());
        for (name, skill) in scan_dir(candidate)? {
            if index.get(&name).is_none() {
                newly.push(name.clone());
                index.insert_for_test_or_dynamic(name, skill);
            }
        }
        Ok(())
    }
}

fn scan_dir(dir: &Path) -> Result<Vec<(String, IndexedSkill)>> {
    let mut out = Vec::new();
    let entries = std::fs::read_dir(dir).map_err(|e| {
        KlyntbotError::Config(ConfigError::Invalid(format!(
            "dynamic scan {}: {e}",
            dir.display()
        )))
    })?;
    for entry in entries {
        let entry = entry.map_err(|e| {
            KlyntbotError::Config(ConfigError::Invalid(format!(
                "dynamic scan {}: {e}",
                dir.display()
            )))
        })?;
        let skill_md = entry.path().join("SKILL.md");
        let raw = match std::fs::read_to_string(&skill_md) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(path = %skill_md.display(), error = %e, "skip");
                continue;
            }
        };
        match KlyntFrontmatter::parse(&raw) {
            Ok((fm, _)) => {
                let name = fm.name.clone();
                out.push((
                    name,
                    IndexedSkill {
                        frontmatter: fm,
                        source: SkillSource::Project,
                        source_path: skill_md,
                    },
                ));
            }
            Err(e) => tracing::warn!(path = %skill_md.display(), error = %e, "skip malformed"),
        }
    }
    Ok(out)
}
