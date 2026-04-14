use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use skills_registry::SkillPackage;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallPlan {
    pub package: SkillPackage,
    pub files_to_write: Vec<FileWrite>,
    pub databases_to_bootstrap: Vec<TemplatePreview>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileWrite {
    pub relative_path: PathBuf,
    pub content_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplatePreview {
    pub template_name: String,
    pub database_name: String,
    pub field_count: usize,
}

impl InstallPlan {
    pub fn skill_only(mut self) -> Self {
        self.databases_to_bootstrap.clear();
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpgradePlan {
    pub name: String,
    pub from_sha: String,
    pub to_sha: String,
    pub diff: skills_registry::diff::DiffResult,
    pub new_bootstraps: Vec<TemplatePreview>,
}
