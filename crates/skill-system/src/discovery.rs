use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;

use crate::parser::parse_skill_md;
use crate::types::*;

// --- Built-in skills compiled into the binary ---

macro_rules! include_skill {
    ($name:expr) => {
        (
            $name,
            include_str!(concat!("../../../skills/", $name, "/SKILL.md")),
        )
    };
}

pub const BUILTIN_SKILLS: &[(&str, &str)] = &[
    include_skill!("general"),
    include_skill!("task-management"),
    include_skill!("finance-management"),
    include_skill!("automation"),
    include_skill!("communication"),
    include_skill!("language-learning"),
];

macro_rules! include_skill_reference {
    ($skill:expr, $ref_name:expr) => {
        (
            $skill,
            $ref_name,
            include_str!(concat!(
                "../../../skills/",
                $skill,
                "/references/",
                $ref_name,
                ".md"
            )),
        )
    };
}

pub const BUILTIN_SKILL_REFERENCES: &[(&str, &str, &str)] = &[
    include_skill_reference!("general", "search"),
    include_skill_reference!("general", "skill-creator"),
    include_skill_reference!("general", "browser"),
    include_skill_reference!("general", "memory"),
    include_skill_reference!("general", "summarize"),
    include_skill_reference!("task-management", "todo"),
    include_skill_reference!("task-management", "daily-planner"),
    include_skill_reference!("task-management", "task-decompose"),
    include_skill_reference!("task-management", "project-management"),
    include_skill_reference!("task-management", "weekly-review"),
    include_skill_reference!("task-management", "retrospective"),
    include_skill_reference!("task-management", "reports"),
    include_skill_reference!("finance-management", "budgeting"),
    include_skill_reference!("finance-management", "spending-intelligence"),
    include_skill_reference!("finance-management", "analytics-actions"),
    include_skill_reference!("finance-management", "fire-planning"),
    include_skill_reference!("finance-management", "portfolio-analysis"),
    include_skill_reference!("finance-management", "financial-health"),
    include_skill_reference!("automation", "cron"),
    include_skill_reference!("communication", "messaging"),
    include_skill_reference!("communication", "notification"),
];

/// Build a reference files HashMap from BUILTIN_SKILL_REFERENCES.
pub fn builtin_reference_map() -> HashMap<String, String> {
    let mut refs = HashMap::new();
    for (skill_name, ref_name, content) in BUILTIN_SKILL_REFERENCES {
        let key = format!("builtin::{skill_name}/references/{ref_name}.md");
        refs.insert(key, content.to_string());
    }
    refs
}

/// Info about a built-in skill for the UI layer.
pub struct BuiltinSkillInfo {
    pub name: &'static str,
    pub content: &'static str,
    pub references: Vec<BuiltinReferenceInfo>,
}

/// Info about a built-in reference file.
pub struct BuiltinReferenceInfo {
    pub name: &'static str,
    pub content: &'static str,
}

/// Returns raw built-in skill content for the UI layer.
pub fn builtin_skills_info() -> Vec<BuiltinSkillInfo> {
    BUILTIN_SKILLS
        .iter()
        .map(|(name, content)| {
            let references = BUILTIN_SKILL_REFERENCES
                .iter()
                .filter(|(skill, _, _)| *skill == *name)
                .map(|(_, ref_name, ref_content)| BuiltinReferenceInfo {
                    name: ref_name,
                    content: ref_content,
                })
                .collect();
            BuiltinSkillInfo {
                name,
                content,
                references,
            }
        })
        .collect()
}

/// Where to discover skills from.
pub enum SkillSource {
    /// Built-in skills: Vec<(name, full SKILL.md content)>
    BuiltIn(Vec<(String, String)>),
    /// Filesystem directory to scan for skill subdirs.
    Directory(std::path::PathBuf, SkillScope),
    /// Persona skill files: Vec<(name, PERSONA.md content)>
    Personas(Vec<(String, String)>),
    /// Inline test source: Vec<(name, content)> with a scope.
    #[cfg(test)]
    Inline(Vec<(String, String)>, SkillScope),
}

impl SkillCatalog {
    /// Async discovery — scans filesystem sources.
    pub async fn discover(sources: &[SkillSource]) -> common::Result<Self> {
        let mut skills: HashMap<String, Arc<SkillPackage>> = HashMap::new();

        for source in sources {
            match source {
                SkillSource::BuiltIn(entries) => {
                    for (name, content) in entries {
                        let location = std::path::PathBuf::from(format!("builtin::{name}"));
                        match parse_skill_md(content, location, SkillScope::BuiltIn) {
                            Ok(pkg) => {
                                skills.insert(name.clone(), Arc::new(pkg));
                            }
                            Err(e) => {
                                tracing::warn!(skill = %name, "Skipping built-in skill: {e}");
                            }
                        }
                    }
                }
                SkillSource::Directory(dir, scope) => {
                    Self::scan_directory(dir, *scope, &mut skills).await?;
                }
                SkillSource::Personas(entries) => {
                    Self::process_persona_entries(entries, &mut skills);
                }
                #[cfg(test)]
                SkillSource::Inline(entries, scope) => {
                    Self::process_inline_entries(entries, *scope, &mut skills);
                }
            }
        }

        Ok(Self {
            skills,
            embeddings: HashMap::new(),
            loaded_at: SystemTime::now(),
        })
    }

    /// Synchronous discovery for built-in-only sources (test helper).
    /// Only supports BuiltIn and Inline sources — errors on Directory.
    pub fn discover_sync(sources: &[SkillSource]) -> common::Result<Self> {
        let mut skills: HashMap<String, Arc<SkillPackage>> = HashMap::new();
        for source in sources {
            match source {
                SkillSource::BuiltIn(entries) => {
                    for (name, content) in entries {
                        let location = std::path::PathBuf::from(format!("builtin::{name}"));
                        match parse_skill_md(content, location, SkillScope::BuiltIn) {
                            Ok(pkg) => {
                                skills.insert(name.clone(), Arc::new(pkg));
                            }
                            Err(e) => {
                                tracing::warn!(skill = %name, "Skipping: {e}");
                            }
                        }
                    }
                }
                SkillSource::Directory(_, _) => {
                    return Err(common::ConfigError::Invalid(
                        "Directory sources require async discover()".into(),
                    )
                    .into());
                }
                SkillSource::Personas(entries) => {
                    Self::process_persona_entries(entries, &mut skills);
                }
                #[cfg(test)]
                SkillSource::Inline(entries, scope) => {
                    Self::process_inline_entries(entries, *scope, &mut skills);
                }
            }
        }
        Ok(Self {
            skills,
            embeddings: HashMap::new(),
            loaded_at: SystemTime::now(),
        })
    }

    #[cfg(test)]
    fn process_inline_entries(
        entries: &[(String, String)],
        scope: SkillScope,
        skills: &mut HashMap<String, Arc<SkillPackage>>,
    ) {
        for (name, content) in entries {
            let location = std::path::PathBuf::from(format!("inline::{name}"));
            match parse_skill_md(content, location, scope) {
                Ok(pkg) => {
                    if let Some(existing) = skills.get(name) {
                        if scope_priority(scope) > scope_priority(existing.scope) {
                            tracing::info!(skill = %name, "Skill shadowed by higher-priority scope");
                            skills.insert(name.clone(), Arc::new(pkg));
                        }
                    } else {
                        skills.insert(name.clone(), Arc::new(pkg));
                    }
                }
                Err(e) => {
                    tracing::warn!(skill = %name, "Skipping: {e}");
                }
            }
        }
    }

    fn process_persona_entries(
        entries: &[(String, String)],
        skills: &mut HashMap<String, Arc<SkillPackage>>,
    ) {
        for (name, content) in entries {
            match crate::persona::parse_persona_skill(content) {
                Ok(parsed) => {
                    let pkg = SkillPackage {
                        name: parsed.name.clone(),
                        description: parsed.description.clone(),
                        skill_type: SkillType::Persona,
                        scope: SkillScope::BuiltIn,
                        location: std::path::PathBuf::from(format!("persona::{name}")),
                        summary: crate::parser::extract_first_sentence(&parsed.body),
                        body: parsed.body,
                        metadata: SkillMetadata::default(),
                        resources: Vec::new(),
                        loaded_at: SystemTime::now(),
                        trusted: true,
                    };
                    skills.insert(parsed.name, Arc::new(pkg));
                }
                Err(e) => {
                    tracing::warn!(persona = %name, "Skipping persona skill: {e}");
                }
            }
        }
    }

    pub fn persona_skills(&self) -> Vec<&Arc<SkillPackage>> {
        self.skills
            .values()
            .filter(|s| matches!(s.skill_type, SkillType::Persona) && s.trusted)
            .collect()
    }

    /// Scan safety bounds per Agent Skills spec.
    const MAX_SCAN_DEPTH: usize = 4;
    const MAX_SCAN_DIRS: usize = 2000;

    /// Directories to skip during scanning (per spec + common conventions).
    const SKIP_DIRS: &'static [&'static str] = &[
        "node_modules",
        "__pycache__",
        ".venv",
        "target",
        "dist",
        "build",
        ".next",
    ];

    async fn scan_directory(
        dir: &Path,
        scope: SkillScope,
        skills: &mut HashMap<String, Arc<SkillPackage>>,
    ) -> common::Result<()> {
        if !dir.exists() {
            return Ok(());
        }
        let mut dirs_scanned: usize = 0;
        Self::scan_directory_recursive(dir, scope, skills, 0, &mut dirs_scanned).await
    }

    async fn scan_directory_recursive(
        dir: &Path,
        scope: SkillScope,
        skills: &mut HashMap<String, Arc<SkillPackage>>,
        depth: usize,
        dirs_scanned: &mut usize,
    ) -> common::Result<()> {
        if depth > Self::MAX_SCAN_DEPTH || *dirs_scanned > Self::MAX_SCAN_DIRS {
            if *dirs_scanned > Self::MAX_SCAN_DIRS {
                tracing::warn!(
                    path = %dir.display(),
                    "Skill scanning stopped: exceeded {} directory limit",
                    Self::MAX_SCAN_DIRS
                );
            }
            return Ok(());
        }

        let mut entries = tokio::fs::read_dir(dir)
            .await
            .map_err(|e| common::ConfigError::Invalid(format!("Reading skills dir: {e}")))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| common::ConfigError::Invalid(format!("Reading entry: {e}")))?
        {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if dir_name.starts_with('.') || Self::SKIP_DIRS.contains(&dir_name) {
                continue;
            }
            *dirs_scanned += 1;

            let skill_md = path.join("SKILL.md");
            if skill_md.exists() {
                // Found a skill directory — parse it
                let content = match tokio::fs::read_to_string(&skill_md).await {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::warn!(path = %skill_md.display(), "Failed to read SKILL.md: {e}");
                        continue;
                    }
                };

                match parse_skill_md(&content, path.clone(), scope) {
                    Ok(mut pkg) => {
                        // Enumerate bundled resources for Tier 3 access
                        pkg.resources = enumerate_resources(&path).await;
                        let name = pkg.name.clone();
                        if let Some(existing) = skills.get(&name) {
                            if scope_priority(scope) > scope_priority(existing.scope) {
                                tracing::info!(skill = %name, "Skill shadowed by higher-priority scope");
                                skills.insert(name, Arc::new(pkg));
                            }
                        } else {
                            skills.insert(name, Arc::new(pkg));
                        }
                    }
                    Err(e) => {
                        tracing::warn!(path = %skill_md.display(), "Skipping skill: {e}");
                    }
                }
            } else {
                // No SKILL.md — recurse into subdirectory (handles nested layouts)
                Box::pin(Self::scan_directory_recursive(
                    &path,
                    scope,
                    skills,
                    depth + 1,
                    dirs_scanned,
                ))
                .await?;
            }
        }
        Ok(())
    }

    /// Lazily precompute description embeddings on first call. No-op if already done.
    pub async fn ensure_embeddings(&mut self, embed: &EmbedFn) {
        if !self.embeddings.is_empty() {
            return;
        }
        self.precompute_embeddings(embed).await;
    }

    /// Precompute description embeddings for semantic matching.
    pub async fn precompute_embeddings(&mut self, embed: &EmbedFn) {
        let mut embeddings = HashMap::new();
        for (name, pkg) in &self.skills {
            match embed(&pkg.description) {
                Ok(vec) => {
                    embeddings.insert(name.clone(), vec);
                }
                Err(e) => {
                    tracing::warn!(skill = %name, "Failed to embed: {e}");
                }
            }
        }
        tracing::debug!("Precomputed embeddings for {} skills", embeddings.len());
        self.embeddings = embeddings;
    }

    pub fn get(&self, name: &str) -> Option<&Arc<SkillPackage>> {
        self.skills.get(name)
    }

    pub fn orchestrators(&self) -> Vec<&Arc<SkillPackage>> {
        self.skills
            .values()
            .filter(|p| p.skill_type == SkillType::Orchestrator && p.trusted)
            .collect()
    }

    pub fn regular_skills(&self) -> Vec<&Arc<SkillPackage>> {
        self.skills
            .values()
            .filter(|p| p.skill_type == SkillType::Skill && p.trusted)
            .collect()
    }

    /// Generate XML catalog for Tier 1 injection into system prompt.
    /// Includes location for file-read activation and resource listing.
    pub fn catalog_prompt(&self) -> String {
        let mut lines = vec!["<available_skills>".to_string()];
        let mut sorted: Vec<_> = self.skills.values().filter(|p| p.trusted).collect();
        sorted.sort_by_key(|p| &p.name);
        for pkg in sorted {
            let type_str = match pkg.skill_type {
                SkillType::Persona => continue,
                SkillType::Orchestrator => "orchestrator",
                SkillType::Skill => "skill",
            };
            let location = pkg.location.display();
            lines.push(format!(
                "  <skill name=\"{}\" type=\"{}\" location=\"{}\">",
                pkg.name, type_str, location
            ));
            lines.push(format!(
                "    <description>{}</description>",
                pkg.description.trim()
            ));
            if !pkg.resources.is_empty() {
                lines.push("    <resources>".to_string());
                for res in &pkg.resources {
                    lines.push(format!("      <file>{res}</file>"));
                }
                lines.push("    </resources>".to_string());
            }
            lines.push("  </skill>".to_string());
        }
        lines.push("</available_skills>".to_string());
        lines.join("\n")
    }

    pub fn all_skills(&self) -> impl Iterator<Item = &Arc<SkillPackage>> {
        self.skills.values()
    }

    pub fn loaded_at(&self) -> SystemTime {
        self.loaded_at
    }
}

/// Enumerate bundled resources (scripts/, references/, assets/) in a skill directory.
async fn enumerate_resources(skill_dir: &Path) -> Vec<String> {
    let mut resources = Vec::new();
    for subdir in &["scripts", "references", "assets"] {
        let path = skill_dir.join(subdir);
        if !path.exists() {
            continue;
        }
        if let Ok(mut entries) = tokio::fs::read_dir(&path).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let file_path = entry.path();
                if file_path.is_file() {
                    if let Some(name) = file_path.file_name().and_then(|n| n.to_str()) {
                        resources.push(format!("{subdir}/{name}"));
                    }
                }
            }
        }
    }
    resources.sort();
    resources
}

fn scope_priority(scope: SkillScope) -> u8 {
    match scope {
        SkillScope::BuiltIn => 0,
        SkillScope::User => 1,
        SkillScope::Project => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_source_produces_skills() {
        let builtin = vec![
            (
                "general".to_string(),
                "---\nname: general\ndescription: General assistant\nmetadata:\n  klyntbot:\n    type: orchestrator\n---\nInstructions.".to_string(),
            ),
            (
                "search".to_string(),
                "---\nname: search\ndescription: Web search.\n---\nSearch instructions.".to_string(),
            ),
        ];
        let source = SkillSource::BuiltIn(builtin);
        let catalog = SkillCatalog::discover_sync(&[source]).unwrap();
        assert_eq!(catalog.skills.len(), 2);
        assert!(catalog.get("general").is_some());
        assert!(catalog.get("search").is_some());
        assert_eq!(
            catalog.get("general").unwrap().skill_type,
            SkillType::Orchestrator
        );
    }

    #[test]
    fn test_higher_scope_shadows_lower() {
        let builtin = vec![(
            "search".to_string(),
            "---\nname: search\ndescription: Built-in search.\n---\nBuiltin body.".to_string(),
        )];
        let user = vec![(
            "search".to_string(),
            "---\nname: search\ndescription: User search override.\n---\nUser body.".to_string(),
        )];
        let sources = vec![
            SkillSource::BuiltIn(builtin),
            SkillSource::Inline(user, SkillScope::User),
        ];
        let catalog = SkillCatalog::discover_sync(&sources).unwrap();
        assert_eq!(catalog.skills.len(), 1);
        let pkg = catalog.get("search").unwrap();
        assert!(pkg.description.contains("User search override"));
        assert_eq!(pkg.scope, SkillScope::User);
    }

    #[test]
    fn test_catalog_prompt_xml() {
        let builtin = vec![
            (
                "general".to_string(),
                "---\nname: general\ndescription: General assistant.\nmetadata:\n  klyntbot:\n    type: orchestrator\n---\nBody.".to_string(),
            ),
            (
                "search".to_string(),
                "---\nname: search\ndescription: Web search.\n---\nBody.".to_string(),
            ),
        ];
        let source = SkillSource::BuiltIn(builtin);
        let catalog = SkillCatalog::discover_sync(&[source]).unwrap();
        let prompt = catalog.catalog_prompt();
        assert!(prompt.contains("<available_skills>"));
        assert!(prompt.contains("name=\"general\""));
        assert!(prompt.contains("type=\"orchestrator\""));
        assert!(prompt.contains("name=\"search\""));
        assert!(prompt.contains("type=\"skill\""));
        assert!(prompt.contains("<description>"));
        assert!(prompt.contains("</available_skills>"));
    }

    #[test]
    fn test_builtin_references_include_reports() {
        let ref_map = builtin_reference_map();
        assert!(ref_map.contains_key("builtin::task-management/references/reports.md"));
        assert!(ref_map.contains_key("builtin::finance-management/references/financial-health.md"));
    }

    #[test]
    fn test_orchestrators_and_regular_skills() {
        let builtin = vec![
            (
                "general".to_string(),
                "---\nname: general\ndescription: General.\nmetadata:\n  klyntbot:\n    type: orchestrator\n---\nBody.".to_string(),
            ),
            (
                "search".to_string(),
                "---\nname: search\ndescription: Search.\n---\nBody.".to_string(),
            ),
        ];
        let source = SkillSource::BuiltIn(builtin);
        let catalog = SkillCatalog::discover_sync(&[source]).unwrap();
        assert_eq!(catalog.orchestrators().len(), 1);
        assert_eq!(catalog.regular_skills().len(), 1);
    }
}
