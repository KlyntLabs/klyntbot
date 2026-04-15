use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

use skill_system::store::SkillFrontmatter;
use skill_system::types::KlyntbotMeta;

use crate::source::SkillSource;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillPackage {
    pub name: String,
    pub source: SkillSource,
    pub resolved_sha: String,
    pub semver: Option<String>,
    pub skill_md_content: String,
    pub frontmatter: SkillFrontmatter,
    #[serde(skip)]
    pub klyntbot_meta: Option<KlyntbotMeta>,
    pub references: Vec<ReferenceFile>,
    pub templates: Vec<TemplateFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceFile {
    pub path: PathBuf,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateFile {
    pub name: String,
    pub manifest: Value,
}

