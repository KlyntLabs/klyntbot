use std::path::PathBuf;
use std::sync::Arc;

use common::{KlyntbotError, Result};
use tracing::{info, warn};

use bus::{DomainEvent, DomainEventBus};
use entity_store::store::EntityStore;
use skill_system::SkillStore;
use skills_marketplace::{InstalledSkill, InstalledSkillsRepo, SourceType};
use skills_registry::{Fetcher, GitRef, SkillPackage, SkillSource};

use crate::plan::{FileWrite, InstallPlan, TemplatePreview};

pub struct Installer {
    pub skills_dir: PathBuf,
    pub fetcher: Arc<Fetcher>,
    pub repo: InstalledSkillsRepo,
    pub entity_store: Arc<EntityStore>,
    pub skill_store: Arc<tokio::sync::RwLock<SkillStore>>,
    pub event_bus: Arc<DomainEventBus>,
}

impl Installer {
    /// Refresh the `disabled` overlay on the shared `SkillStore` from the
    /// `installed_skills` table. Call after toggling an `enabled` flag, and at
    /// startup, so `format_listing`, `names`, `get`, `get_body`, and
    /// `build_reference_index` filter out disabled skills.
    pub async fn refresh_disabled(&self) -> Result<()> {
        let installed = self.repo.list().await?;
        let disabled: std::collections::HashSet<String> = installed
            .into_iter()
            .filter(|s| !s.enabled)
            .map(|s| s.name)
            .collect();
        self.skill_store.write().await.set_disabled(disabled);
        Ok(())
    }

    pub async fn preview_install(
        &self,
        source: &SkillSource,
        version: Option<GitRef>,
    ) -> Result<InstallPlan> {
        let effective = match version {
            Some(r) => override_ref(source.clone(), r),
            None => source.clone(),
        };
        let pkg = self.fetcher.fetch(&effective).await?;
        Ok(build_plan(pkg))
    }

    pub async fn apply_install(&self, plan: InstallPlan) -> Result<InstalledSkill> {
        let dir = self.skills_dir.join(&plan.package.name);
        let mut written_paths: Vec<PathBuf> = Vec::new();
        let mut created_dbs: Vec<String> = Vec::new();

        let attempt: Result<InstalledSkill> = async {
            write_package(&dir, &plan.package, &mut written_paths).await?;
            for tpl in &plan.databases_to_bootstrap {
                let template = plan
                    .package
                    .templates
                    .iter()
                    .find(|t| t.name == tpl.template_name)
                    .ok_or_else(|| {
                        KlyntbotError::Storage(format!("template {} missing", tpl.template_name))
                    })?;
                let manifest: entity_store::templates::TemplateManifest =
                    serde_json::from_value(template.manifest.clone()).map_err(|e| {
                        KlyntbotError::Storage(format!("template {}: {e}", tpl.template_name))
                    })?;
                let ids = entity_store::templates::install_template(
                    self.entity_store.as_ref(),
                    &manifest,
                )
                .await?;
                created_dbs.extend(ids);
            }
            let row = InstalledSkill {
                name: plan.package.name.clone(),
                source_type: source_type_of(&plan.package.source),
                source_ref: source_ref_string(&plan.package.source),
                installed_version: plan
                    .package
                    .semver
                    .clone()
                    .unwrap_or_else(|| "0.0.0".into()),
                installed_sha: plan.package.resolved_sha.clone(),
                enabled: true,
                is_adapted: false,
                bootstrapped_databases: created_dbs.clone(),
                installed_at: chrono::Utc::now().to_rfc3339(),
                updated_at: chrono::Utc::now().to_rfc3339(),
            };
            self.repo.insert(&row).await?;
            self.skill_store
                .write()
                .await
                .reload()
                .map_err(|e| KlyntbotError::Storage(e.to_string()))?;
            self.event_bus.publish(DomainEvent::SkillInstalled {
                name: row.name.clone(),
                source: row.source_ref.clone(),
                version: row.installed_version.clone(),
            });
            info!(name = %row.name, "skill installed");
            Ok(row)
        }
        .await;

        match attempt {
            Ok(row) => Ok(row),
            Err(e) => {
                warn!(error = %e, "install failed — rolling back");
                for p in written_paths.iter().rev() {
                    let _ = tokio::fs::remove_file(p).await;
                }
                let _ = tokio::fs::remove_dir_all(&dir).await;
                for db_id in &created_dbs {
                    let _ = self.entity_store.delete_database(db_id).await;
                }
                Err(e)
            }
        }
    }

    pub async fn check_updates(
        &self,
        name: &str,
    ) -> Result<Vec<skills_registry::AvailableVersion>> {
        let Some(row) = self.repo.get(name).await? else {
            return Err(KlyntbotError::Storage(format!(
                "skill '{name}' not installed"
            )));
        };
        let (owner, repo, subpath) = parse_github_ref(&row.source_ref).ok_or_else(|| {
            KlyntbotError::Storage("only github sources support check_updates".into())
        })?;
        let uf = skills_registry::UpdatesFetcher::new("https://api.github.com".into());
        uf.list_newer(&owner, &repo, &subpath, &row.installed_sha)
            .await
    }

    pub async fn preview_upgrade(
        &self,
        name: &str,
        target_sha: &str,
    ) -> Result<crate::plan::UpgradePlan> {
        let row = self
            .repo
            .get(name)
            .await?
            .ok_or_else(|| KlyntbotError::Storage(format!("skill '{name}' not installed")))?;
        let (owner, repo, subpath) = parse_github_ref(&row.source_ref)
            .ok_or_else(|| KlyntbotError::Storage("only github upgrades supported".into()))?;

        let current_pkg = self
            .fetcher
            .fetch(&SkillSource::LocalPath {
                path: self.skills_dir.join(&row.name),
            })
            .await
            .ok();

        let target = self
            .fetcher
            .fetch(&SkillSource::Github {
                owner,
                repo,
                subpath,
                r#ref: GitRef::Commit {
                    sha: target_sha.into(),
                },
            })
            .await?;

        let diff = if let Some(ref current) = current_pkg {
            skills_registry::diff::diff_packages(current, &target)
        } else {
            skills_registry::diff::DiffResult::default()
        };

        let current_tpl_names: std::collections::HashSet<_> = current_pkg
            .as_ref()
            .map(|c| c.templates.iter().map(|t| t.name.clone()).collect())
            .unwrap_or_default();
        let new_bootstraps: Vec<crate::plan::TemplatePreview> = target
            .templates
            .iter()
            .filter(|t| !current_tpl_names.contains(&t.name))
            .map(|t| crate::plan::TemplatePreview {
                template_name: t.name.clone(),
                database_name: t.name.clone(),
                field_count: 0,
            })
            .collect();

        Ok(crate::plan::UpgradePlan {
            name: name.into(),
            from_sha: row.installed_sha,
            to_sha: target_sha.into(),
            diff,
            new_bootstraps,
            target_package: target,
        })
    }

    pub async fn apply_upgrade(&self, plan: crate::plan::UpgradePlan) -> Result<InstalledSkill> {
        let row = self.repo.get(&plan.name).await?.ok_or_else(|| {
            KlyntbotError::Storage(format!("skill '{}' not installed", plan.name))
        })?;

        let target = plan.target_package;

        let dir = self.skills_dir.join(&plan.name);
        let mut written = Vec::new();
        tokio::fs::remove_dir_all(&dir).await.ok();
        write_package(&dir, &target, &mut written).await?;

        // Bootstrap newly-declared templates only.
        let mut bootstrapped = row.bootstrapped_databases.clone();
        for new_tpl in &plan.new_bootstraps {
            let tpl = target
                .templates
                .iter()
                .find(|t| t.name == new_tpl.template_name)
                .ok_or_else(|| KlyntbotError::Storage("new template missing in target".into()))?;
            let manifest: entity_store::templates::TemplateManifest =
                serde_json::from_value(tpl.manifest.clone())
                    .map_err(|e| KlyntbotError::Storage(e.to_string()))?;
            let ids =
                entity_store::templates::install_template(self.entity_store.as_ref(), &manifest)
                    .await?;
            bootstrapped.extend(ids);
        }

        let new_version = target
            .semver
            .clone()
            .unwrap_or_else(|| row.installed_version.clone());
        self.repo
            .update_version(&plan.name, &new_version, &plan.to_sha, &bootstrapped)
            .await?;
        self.skill_store
            .write()
            .await
            .reload()
            .map_err(|e| KlyntbotError::Storage(e.to_string()))?;

        self.event_bus.publish(DomainEvent::SkillUpgraded {
            name: plan.name.clone(),
            from_version: row.installed_version,
            to_version: new_version,
        });

        Ok(self.repo.get(&plan.name).await?.unwrap())
    }

    pub async fn uninstall(&self, name: &str, mode: crate::uninstall::UninstallMode) -> Result<()> {
        crate::uninstall::uninstall(
            mode,
            name,
            &self.skills_dir,
            &self.repo,
            Arc::clone(&self.entity_store),
            Arc::clone(&self.skill_store),
            Arc::clone(&self.event_bus),
        )
        .await
    }
}

fn override_ref(mut s: SkillSource, r: GitRef) -> SkillSource {
    if let SkillSource::Github { r#ref, .. } = &mut s {
        *r#ref = r;
    }
    s
}

fn source_type_of(s: &SkillSource) -> SourceType {
    match s {
        SkillSource::Github { .. } => SourceType::Github,
        SkillSource::SkillsSh { .. } => SourceType::SkillsSh,
        SkillSource::LocalPath { .. } => SourceType::Local,
        SkillSource::Bundled { .. } => SourceType::Bundled,
    }
}

fn source_ref_string(s: &SkillSource) -> String {
    match s {
        SkillSource::Github {
            owner,
            repo,
            subpath,
            r#ref,
        } => {
            let base = format!("{owner}/{repo}/{subpath}");
            match r#ref {
                GitRef::Commit { sha } => format!("{base}@{sha}"),
                _ => base,
            }
        }
        SkillSource::SkillsSh { slug } => slug.clone(),
        SkillSource::LocalPath { path } => path.display().to_string(),
        SkillSource::Bundled { name } => format!("bundled:{name}"),
    }
}

fn build_plan(pkg: SkillPackage) -> InstallPlan {
    let mut files: Vec<FileWrite> = vec![FileWrite {
        relative_path: PathBuf::from("SKILL.md"),
        content_size: pkg.skill_md_content.len(),
    }];
    for r in &pkg.references {
        files.push(FileWrite {
            relative_path: PathBuf::from("references").join(
                r.path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("ref.md"),
            ),
            content_size: r.content.len(),
        });
    }
    for t in &pkg.templates {
        files.push(FileWrite {
            relative_path: PathBuf::from("templates").join(&t.name),
            content_size: t.manifest.to_string().len(),
        });
    }
    let databases_to_bootstrap: Vec<TemplatePreview> = pkg
        .templates
        .iter()
        .map(|t| {
            let db_name = t
                .manifest
                .get("databases")
                .and_then(|d| d.as_array())
                .and_then(|a| a.first())
                .and_then(|d| d.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or(&t.name)
                .to_string();
            let field_count = t
                .manifest
                .get("databases")
                .and_then(|d| d.as_array())
                .and_then(|a| a.first())
                .and_then(|d| d.get("fields"))
                .and_then(|f| f.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            TemplatePreview {
                template_name: t.name.clone(),
                database_name: db_name,
                field_count,
            }
        })
        .collect();

    let warnings = Vec::new();
    InstallPlan {
        package: pkg,
        files_to_write: files,
        databases_to_bootstrap,
        warnings,
    }
}

async fn write_package(
    dir: &std::path::Path,
    pkg: &SkillPackage,
    written: &mut Vec<PathBuf>,
) -> Result<()> {
    tokio::fs::create_dir_all(dir)
        .await
        .map_err(|e| KlyntbotError::Storage(format!("create_dir {}: {e}", dir.display())))?;
    let skill_path = dir.join("SKILL.md");
    tokio::fs::write(&skill_path, &pkg.skill_md_content)
        .await
        .map_err(|e| KlyntbotError::Storage(format!("write {}: {e}", skill_path.display())))?;
    written.push(skill_path);

    if !pkg.references.is_empty() {
        let refs_dir = dir.join("references");
        tokio::fs::create_dir_all(&refs_dir)
            .await
            .map_err(|e| KlyntbotError::Storage(e.to_string()))?;
        for r in &pkg.references {
            let filename = r
                .path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("ref.md");
            let p = refs_dir.join(filename);
            tokio::fs::write(&p, &r.content)
                .await
                .map_err(|e| KlyntbotError::Storage(e.to_string()))?;
            written.push(p);
        }
    }

    if !pkg.templates.is_empty() {
        let tpls_dir = dir.join("templates");
        tokio::fs::create_dir_all(&tpls_dir)
            .await
            .map_err(|e| KlyntbotError::Storage(e.to_string()))?;
        for t in &pkg.templates {
            let p = tpls_dir.join(&t.name);
            let contents = serde_json::to_string_pretty(&t.manifest)
                .map_err(|e| KlyntbotError::Storage(e.to_string()))?;
            tokio::fs::write(&p, contents)
                .await
                .map_err(|e| KlyntbotError::Storage(e.to_string()))?;
            written.push(p);
        }
    }

    Ok(())
}

/// Parse a stored source_ref back into (owner, repo, subpath, optional_sha).
/// Format: `owner/repo/subpath[@sha]` — the `@sha` suffix is optional (older rows omit it).
fn parse_github_ref(s: &str) -> Option<(String, String, String)> {
    let (path_part, _sha) = s.split_once('@').unwrap_or((s, ""));
    match SkillSource::parse_shorthand(path_part) {
        Ok(SkillSource::Github {
            owner,
            repo,
            subpath,
            ..
        }) => Some((owner, repo, subpath)),
        _ => None,
    }
}
