use crate::AppCore;
use common::{ConfigError, KlyntbotError, Result};
use klynt_skill_loader::{KlyntFrontmatter, SkillIndex, SkillSource};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct SkillListItem {
    pub name: String,
    pub description: String,
    pub source: String,
    pub source_path: String,
    pub tags: Vec<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct SkillInfo {
    pub name: String,
    pub description: String,
    pub allowed_tools: Vec<String>,
    pub paths: Vec<String>,
    pub tags: Vec<String>,
    pub sensitivity: Option<String>,
    pub source: String,
    pub source_path: String,
    pub references: Vec<SkillReferenceInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct SkillReferenceInfo {
    pub name: String,
    pub file: String,
    pub load: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct SkillValidationResult {
    pub ok: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl From<SkillInfo> for SkillListItem {
    fn from(info: SkillInfo) -> Self {
        Self {
            name: info.name,
            description: info.description,
            source: info.source,
            source_path: info.source_path,
            tags: info.tags,
            enabled: true,
        }
    }
}

impl AppCore {
    pub async fn klyntbot_home(&self) -> PathBuf {
        self.config.read().await.data_dir_path()
    }

    async fn with_activator<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&klynt_skill_loader::SkillActivator) -> Result<T>,
    {
        let guard = self.coding_skill_activator.lock().await;
        let act = guard.as_ref().ok_or_else(|| {
            KlyntbotError::Config(ConfigError::Invalid(
                "skill activator not initialized".into(),
            ))
        })?;
        f(act)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn coding_skills_list(&self) -> Result<Vec<SkillListItem>> {
        let cfg = self.config.read().await;
        let never: std::collections::HashSet<String> =
            cfg.coding.skills.never_activate.iter().cloned().collect();
        self.with_activator(|activator| {
            let mut items = Vec::new();
            for (name, skill) in activator.iter_index() {
                items.push(SkillListItem {
                    name: name.clone(),
                    description: skill.frontmatter.description.clone(),
                    source: format!("{:?}", skill.source).to_lowercase(),
                    source_path: skill.source_path.display().to_string(),
                    tags: skill.frontmatter.tags.clone(),
                    enabled: !never.contains(name.as_str()),
                });
            }
            items.sort_by(|a, b| a.name.cmp(&b.name));
            Ok(items)
        })
        .await
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn coding_skills_info(&self, name: &str) -> Result<SkillInfo> {
        self.with_activator(|activator| {
            let skill = activator.lookup(name).ok_or_else(|| {
                KlyntbotError::Config(ConfigError::Invalid(format!("unknown skill: {name}")))
            })?;
            Ok(SkillInfo {
                name: skill.frontmatter.name.clone(),
                description: skill.frontmatter.description.clone(),
                allowed_tools: skill.frontmatter.allowed_tools.clone(),
                paths: skill.frontmatter.paths.clone(),
                tags: skill.frontmatter.tags.clone(),
                sensitivity: skill.frontmatter.sensitivity.clone(),
                source: format!("{:?}", skill.source).to_lowercase(),
                source_path: skill.source_path.display().to_string(),
                references: skill
                    .frontmatter
                    .references
                    .iter()
                    .map(|r| SkillReferenceInfo {
                        name: r.name.clone(),
                        file: r.file.clone(),
                        load: format!("{:?}", r.load).to_lowercase(),
                    })
                    .collect(),
            })
        })
        .await
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn coding_skills_install(&self, source: String) -> Result<SkillListItem> {
        let target_dir = self.klyntbot_home().await.join("skills");
        std::fs::create_dir_all(&target_dir).map_err(|e| {
            KlyntbotError::Config(ConfigError::Invalid(format!("create skills dir: {e}")))
        })?;
        let installed_name = if source.starts_with("http://") || source.starts_with("https://") {
            install_from_url(&source, &target_dir).await?
        } else {
            install_from_local_path(&PathBuf::from(&source), &target_dir)?
        };
        self.coding_skills_reload().await?;
        self.coding_skills_info(&installed_name)
            .await
            .map(|info| info.into())
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn coding_skills_update(&self, name: &str) -> Result<SkillListItem> {
        self.coding_skills_reload().await?;
        let info = self.coding_skills_info(name).await?;
        Ok(info.into())
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn coding_skills_uninstall(&self, name: &str) -> Result<()> {
        let dir = self
            .with_activator(|activator| {
                let skill = activator.lookup(name).ok_or_else(|| {
                    KlyntbotError::Config(ConfigError::Invalid(format!("unknown skill: {name}")))
                })?;
                if !matches!(skill.source, SkillSource::User) {
                    return Err(KlyntbotError::Config(ConfigError::Invalid(
                        "can only uninstall User-source skills (project skills live in repo)"
                            .into(),
                    )));
                }
                skill
                    .source_path
                    .parent()
                    .map(|p| p.to_path_buf())
                    .ok_or_else(|| {
                        KlyntbotError::Config(ConfigError::Invalid(
                            "skill path has no parent dir".into(),
                        ))
                    })
            })
            .await?;
        std::fs::remove_dir_all(&dir).map_err(|e| {
            KlyntbotError::Config(ConfigError::Invalid(format!(
                "remove {}: {e}",
                dir.display()
            )))
        })?;
        self.coding_skills_reload().await
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn coding_skills_toggle(&self, name: &str, enabled: bool) -> Result<()> {
        let mut cfg = self.config.write().await;
        let changed = if enabled {
            let had = cfg.coding.skills.never_activate.contains(&name.to_string());
            cfg.coding.skills.never_activate.retain(|n| n != name);
            had
        } else {
            let had = !cfg.coding.skills.never_activate.iter().any(|n| n == name);
            if had {
                cfg.coding.skills.never_activate.push(name.to_string());
            }
            had
        };
        drop(cfg);
        if changed {
            self.coding_skills_reload().await
        } else {
            Ok(())
        }
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn coding_skills_validate(&self, name: &str) -> Result<SkillValidationResult> {
        self.with_activator(|activator| {
            let mut errors = Vec::new();
            let mut warnings = Vec::new();
            match activator.lookup(name) {
                None => errors.push(format!("unknown skill: {name}")),
                Some(skill) => {
                    let raw = std::fs::read_to_string(&skill.source_path).map_err(|e| {
                        KlyntbotError::Config(ConfigError::Invalid(format!(
                            "re-read SKILL.md: {e}"
                        )))
                    })?;
                    if let Err(e) = KlyntFrontmatter::parse(&raw) {
                        errors.push(format!("frontmatter invalid: {e}"));
                    }
                    for path_glob in &skill.frontmatter.paths {
                        if globset::Glob::new(path_glob).is_err() {
                            errors.push(format!("invalid path glob: {path_glob}"));
                        }
                    }
                    if skill.frontmatter.allowed_tools.is_empty() {
                        warnings.push("no allowed-tools declared (skill has access to all)".into());
                    }
                }
            }
            Ok(SkillValidationResult {
                ok: errors.is_empty(),
                errors,
                warnings,
            })
        })
        .await
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn coding_skills_reload(&self) -> Result<()> {
        let new_index = self.discover_skills().await?;
        let cfg = self.config.read().await;
        let activation_cfg =
            klynt_skill_loader::ActivationConfig::from_coding_config(&cfg.coding.skills);
        drop(cfg);
        let mut act = self.coding_skill_activator.lock().await;
        *act = Some(klynt_skill_loader::SkillActivator::new(
            new_index,
            activation_cfg,
        )?);
        Ok(())
    }

    pub(crate) async fn discover_skills(&self) -> Result<SkillIndex> {
        let roots = klynt_skill_loader::DiscoveryRoots {
            klyntbot_home: self.klyntbot_home().await,
            repo_id: None,
            repo_root: None,
            cwd: std::env::current_dir().unwrap_or_else(|_| ".".into()),
        };
        SkillIndex::discover(&roots)
    }
}

fn install_from_local_path(src: &std::path::Path, target_dir: &std::path::Path) -> Result<String> {
    let skill_md = src.join("SKILL.md");
    if !skill_md.exists() {
        return Err(KlyntbotError::Config(ConfigError::Invalid(format!(
            "no SKILL.md at {}",
            src.display()
        ))));
    }
    let raw = std::fs::read_to_string(&skill_md)
        .map_err(|e| KlyntbotError::Config(ConfigError::Invalid(format!("read source: {e}"))))?;
    let (fm, _) = KlyntFrontmatter::parse(&raw)?;
    let dst = target_dir.join(&fm.name);
    if dst.exists() {
        return Err(KlyntbotError::Config(ConfigError::Invalid(format!(
            "skill `{}` already installed",
            fm.name
        ))));
    }
    fs_copy_dir_all(src, &dst)?;
    Ok(fm.name)
}

fn fs_copy_dir_all(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(dst)
        .map_err(|e| KlyntbotError::Config(ConfigError::Invalid(format!("create dst: {e}"))))?;
    for entry in std::fs::read_dir(src)
        .map_err(|e| KlyntbotError::Config(ConfigError::Invalid(format!("read src: {e}"))))?
    {
        let entry = entry
            .map_err(|e| KlyntbotError::Config(ConfigError::Invalid(format!("read src: {e}"))))?;
        let ft = entry
            .file_type()
            .map_err(|e| KlyntbotError::Config(ConfigError::Invalid(format!("file_type: {e}"))))?;
        let dst_entry = dst.join(entry.file_name());
        if ft.is_dir() {
            fs_copy_dir_all(&entry.path(), &dst_entry)?;
        } else {
            std::fs::copy(entry.path(), &dst_entry)
                .map_err(|e| KlyntbotError::Config(ConfigError::Invalid(format!("copy: {e}"))))?;
        }
    }
    Ok(())
}

async fn install_from_url(url: &str, target_dir: &std::path::Path) -> Result<String> {
    let temp = tempfile::tempdir()
        .map_err(|e| KlyntbotError::Config(ConfigError::Invalid(format!("tempdir: {e}"))))?;
    let status = std::process::Command::new("git")
        .args(["clone", "--depth", "1", url, temp.path().to_str().unwrap()])
        .status()
        .map_err(|e| KlyntbotError::Config(ConfigError::Invalid(format!("git clone: {e}"))))?;
    if !status.success() {
        return Err(KlyntbotError::Config(ConfigError::Invalid(format!(
            "git clone failed: {url}"
        ))));
    }
    install_from_local_path(temp.path(), target_dir)
}
