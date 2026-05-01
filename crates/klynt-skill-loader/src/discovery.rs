use crate::frontmatter::KlyntFrontmatter;
use crate::index::{DiscoveryRoots, IndexedSkill, SkillIndex, SkillSource};
use common::{ConfigError, KlyntbotError, Result};
use std::fs;
use std::path::{Path, PathBuf};

impl SkillIndex {
    /// Discover all skills under the four static roots (§8.911-918).
    pub fn discover(roots: &DiscoveryRoots) -> Result<Self> {
        let mut idx = SkillIndex::new();
        idx.merge(scan_root(
            roots.klyntbot_home.join("skills"),
            SkillSource::User,
        )?);
        if let Some(repo_id) = &roots.repo_id {
            idx.merge(scan_root(
                roots
                    .klyntbot_home
                    .join("project-skills")
                    .join(sanitize_repo_id(repo_id)),
                SkillSource::ReforgePrivate,
            )?);
        }
        if let Some(repo_root) = &roots.repo_root {
            idx.merge(scan_root(
                repo_root.join(".klyntbot/skills"),
                SkillSource::Project,
            )?);
            idx.merge(scan_root(
                repo_root.join(".klyntbot/team-skills"),
                SkillSource::ReforgeTeam,
            )?);
        }
        let cwd_path = roots.cwd.join(".klyntbot/skills");
        if Some(&cwd_path)
            != roots
                .repo_root
                .as_ref()
                .map(|r| r.join(".klyntbot/skills"))
                .as_ref()
        {
            idx.merge(scan_root(cwd_path, SkillSource::Project)?);
        }
        Ok(idx)
    }
}

/// Path-safe repo-id (replaces `/` and `:` so a github URL becomes one segment).
pub fn sanitize_repo_id(repo_id: &str) -> String {
    repo_id
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn scan_root(dir: PathBuf, source: SkillSource) -> Result<SkillIndex> {
    if !dir.is_dir() {
        return Ok(SkillIndex::new());
    }
    let mut idx = SkillIndex::new();
    let entries = fs::read_dir(&dir).map_err(|e| {
        KlyntbotError::Config(ConfigError::Invalid(format!(
            "reading {}: {e}",
            dir.display()
        )))
    })?;
    for entry in entries {
        let entry = entry.map_err(|e| {
            KlyntbotError::Config(ConfigError::Invalid(format!(
                "reading {}: {e}",
                dir.display()
            )))
        })?;
        let path = entry.path();
        let skill_md = path.join("SKILL.md");
        match parse_skill(&skill_md, source) {
            Ok(skill) => idx.insert(skill.frontmatter.name.clone(), skill),
            Err(e) => tracing::warn!(
                path = %skill_md.display(),
                error = %e,
                "skipping malformed SKILL.md"
            ),
        }
    }
    Ok(idx)
}

fn parse_skill(skill_md: &Path, source: SkillSource) -> Result<IndexedSkill> {
    let raw = fs::read_to_string(skill_md).map_err(|e| {
        KlyntbotError::Config(ConfigError::Invalid(format!(
            "reading {}: {e}",
            skill_md.display()
        )))
    })?;
    let (frontmatter, _body) = KlyntFrontmatter::parse(&raw)?;
    Ok(IndexedSkill {
        frontmatter,
        source,
        source_path: skill_md.to_path_buf(),
    })
}
