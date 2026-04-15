use std::sync::Arc;

use desktop_shared::errors::ApiError;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use skills_installer::{InstallPlan, UninstallMode, UpgradePlan};
use skills_marketplace::InstalledSkill;
use skills_registry::{AvailableVersion, GitRef, SkillPackage, SkillSource};

use crate::state::AppCore;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillBrowseRow {
    pub rank: usize,
    pub name: String,
    pub source_ref: String,
    pub installs: Option<u64>,
    pub is_klynt_native: bool,
    pub is_installed: bool,
    pub is_bundled: bool,
}

impl AppCore {
    fn require_installer(&self) -> Result<&Arc<skills_installer::Installer>, ApiError> {
        self.installer
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Skills installer not initialized"))
    }

    fn require_adapter(&self) -> Result<&Arc<skills_adapter::Adapter>, ApiError> {
        self.adapter.as_ref().ok_or_else(|| {
            ApiError::new(
                "NOT_AVAILABLE",
                "No cognitive provider configured — adapter disabled",
            )
        })
    }

    pub async fn skill_list(&self) -> Result<Vec<InstalledSkill>, ApiError> {
        let inst = self.require_installer()?;
        inst.repo.list().await.map_err(Into::into)
    }

    pub async fn skill_browse(
        &self,
        _query: Option<String>,
    ) -> Result<Vec<SkillBrowseRow>, ApiError> {
        // MVP: curated featured list + installed skills. Live skills.sh proxy comes later.
        let inst = self.require_installer()?;
        let installed = inst.repo.list().await.map_err(ApiError::from)?;
        let curated: Vec<(&str, &str)> = vec![
            ("reading-list", "klynt-skills/official/reading-list"),
            ("pkm-notebook", "klynt-skills/official/pkm-notebook"),
        ];
        let mut out: Vec<SkillBrowseRow> = Vec::new();
        for (i, (name, src)) in curated.iter().enumerate() {
            out.push(SkillBrowseRow {
                rank: i + 1,
                name: (*name).into(),
                source_ref: (*src).into(),
                installs: None,
                is_klynt_native: true,
                is_installed: installed.iter().any(|s| s.name == *name),
                is_bundled: false,
            });
        }
        for s in &installed {
            if !out.iter().any(|r| r.name == s.name) {
                out.push(SkillBrowseRow {
                    rank: out.len() + 1,
                    name: s.name.clone(),
                    source_ref: s.source_ref.clone(),
                    installs: None,
                    is_klynt_native: !s.is_adapted,
                    is_installed: true,
                    is_bundled: matches!(s.source_type, skills_marketplace::SourceType::Bundled),
                });
            }
        }
        Ok(out)
    }

    pub async fn skill_install_preview(
        &self,
        shorthand: String,
        version: Option<GitRef>,
    ) -> Result<InstallPlan, ApiError> {
        let inst = self.require_installer()?;
        let source = SkillSource::parse_shorthand(&shorthand)
            .map_err(|e| ApiError::new("VALIDATION", e.to_string()))?;
        inst.preview_install(&source, version)
            .await
            .map_err(Into::into)
    }

    pub async fn skill_install_apply(&self, plan: InstallPlan) -> Result<InstalledSkill, ApiError> {
        let inst = self.require_installer()?;
        inst.apply_install(plan).await.map_err(Into::into)
    }

    pub async fn skill_check_updates(
        &self,
        name: String,
    ) -> Result<Vec<AvailableVersion>, ApiError> {
        self.require_installer()?
            .check_updates(&name)
            .await
            .map_err(Into::into)
    }

    pub async fn skill_upgrade_preview(
        &self,
        name: String,
        target_sha: String,
    ) -> Result<UpgradePlan, ApiError> {
        self.require_installer()?
            .preview_upgrade(&name, &target_sha)
            .await
            .map_err(Into::into)
    }

    pub async fn skill_upgrade_apply(&self, plan: UpgradePlan) -> Result<InstalledSkill, ApiError> {
        self.require_installer()?
            .apply_upgrade(plan)
            .await
            .map_err(Into::into)
    }

    pub async fn skill_uninstall(&self, name: String, mode: UninstallMode) -> Result<(), ApiError> {
        self.require_installer()?
            .uninstall(&name, mode)
            .await
            .map_err(Into::into)
    }

    pub async fn skill_toggle_enabled(&self, name: String, enabled: bool) -> Result<(), ApiError> {
        let inst = self.require_installer()?;
        inst.repo
            .set_enabled(&name, enabled)
            .await
            .map_err(ApiError::from)?;
        inst.skill_store
            .write()
            .await
            .reload()
            .map_err(|e| ApiError::new("STORAGE_ERROR", e.to_string()))?;
        Ok(())
    }

    pub async fn skill_adapt_preview(&self, shorthand: String) -> Result<Value, ApiError> {
        let inst = self.require_installer()?;
        let adapter = self.require_adapter()?;
        let source = SkillSource::parse_shorthand(&shorthand)
            .map_err(|e| ApiError::new("VALIDATION", e.to_string()))?;
        let pkg: SkillPackage = inst.fetcher.fetch(&source).await.map_err(ApiError::from)?;

        let existing_dbs = inst
            .entity_store
            .list_databases()
            .await
            .map_err(ApiError::from)?
            .into_iter()
            .map(|d| (d.name, d.slug))
            .collect::<Vec<_>>();
        let out = adapter
            .adapt(&pkg, &existing_dbs)
            .await
            .map_err(ApiError::from)?;
        Ok(serde_json::to_value(serde_json::json!({
            "adaptedSkillMd": out.adapted_skill_md,
            "generatedTemplates": out.generated_templates,
            "rationale": out.rationale,
            "adapterModel": out.adapter_model,
        }))
        .unwrap())
    }
}
