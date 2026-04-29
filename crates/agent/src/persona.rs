//! Persona system — loads `.md` persona files with YAML frontmatter,
//! resolves scopes against PARA entities, and chains personas for sessions.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::Deserialize;
use storage::rows::session_context::SessionContextRow;
use tokio::fs;
use tracing::{debug, warn};

/// A parsed persona definition.
#[derive(Debug, Clone)]
pub struct Persona {
    pub name: String,
    /// Markdown body after frontmatter — the actual instructions.
    pub instructions: String,
    pub scope: PersonaScope,
    /// Skill names this persona activates.
    pub skills: Vec<String>,
    pub tone: Option<String>,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Determines when a persona applies.
#[derive(Debug, Clone, PartialEq)]
pub enum PersonaScope {
    /// Applies to all sessions (no scope in frontmatter).
    Global,
    /// Applies to sessions in a specific area.
    Area { name: String },
    /// Applies to sessions for a specific project.
    Project { name: String },
    /// Applies to sessions for a feature kind (e.g. "finance", "finance.budgets").
    Feature { kind: String },
}

/// A chain of personas resolved for a specific session, ordered global → specific.
#[derive(Debug, Clone)]
pub struct PersonaChain {
    pub personas: Vec<Persona>,
    pub merged_skills: HashSet<String>,
    pub merged_instructions: String,
    pub tone: Option<String>,
}

impl PersonaChain {
    /// Returns true if this chain has no personas.
    pub fn is_empty(&self) -> bool {
        self.personas.is_empty()
    }
}

/// Cached scope resolution — maps persona name → entity IDs.
#[derive(Debug, Clone)]
struct ResolvedScope {
    area_id: Option<String>,
    project_id: Option<String>,
}

/// Loads and manages persona `.md` files from disk.
pub struct PersonaManager {
    personas: Vec<Persona>,
    resolved_scopes: HashMap<String, ResolvedScope>,
    personas_dir: PathBuf,
}

impl PersonaManager {
    /// Load all persona files from a directory.
    pub async fn load(personas_dir: &Path) -> Self {
        let personas = Self::read_dir(personas_dir).await;
        Self {
            personas,
            resolved_scopes: HashMap::new(),
            personas_dir: personas_dir.to_path_buf(),
        }
    }

    /// Re-read persona files from the original directory. Preserves the
    /// previously resolved scopes (area/project IDs) — those are stable
    /// references that don't need to be recomputed for tone/instructions edits.
    /// Call this before serving a chat turn so the user's edits to
    /// `personas/*.md` take effect without restarting.
    pub async fn reload_personas(&mut self) {
        self.personas = Self::read_dir(&self.personas_dir).await;
    }

    async fn read_dir(personas_dir: &Path) -> Vec<Persona> {
        let mut personas = Vec::new();

        if !personas_dir.exists() {
            debug!("Personas directory does not exist: {:?}", personas_dir);
            return personas;
        }

        let mut entries = match fs::read_dir(personas_dir).await {
            Ok(e) => e,
            Err(e) => {
                warn!("Failed to read personas directory: {}", e);
                return personas;
            }
        };

        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("md") {
                match load_persona_file(&path).await {
                    Ok(persona) => {
                        debug!("Loaded persona: {}", persona.name);
                        personas.push(persona);
                    }
                    Err(e) => {
                        warn!("Failed to load persona from {:?}: {}", path, e);
                    }
                }
            }
        }

        debug!("Loaded {} personas", personas.len());
        personas
    }

    /// Resolve persona scopes against the database (area/project name → ID).
    pub async fn resolve_scopes(&mut self, repos: &storage::repos::Repos) {
        let areas = repos.areas.list(None).await.unwrap_or_default();
        let projects = repos
            .projects
            .list(&storage::repos::ProjectFilter::default())
            .await
            .unwrap_or_default();

        for persona in &self.personas {
            match &persona.scope {
                PersonaScope::Area { name } => {
                    if let Some(area) = areas.iter().find(|a| a.name == *name) {
                        self.resolved_scopes.insert(
                            persona.name.clone(),
                            ResolvedScope {
                                area_id: Some(area.id.clone()),
                                project_id: None,
                            },
                        );
                    }
                }
                PersonaScope::Project { name } => {
                    if let Some(project) = projects.iter().find(|p| p.name == *name) {
                        self.resolved_scopes.insert(
                            persona.name.clone(),
                            ResolvedScope {
                                area_id: Some(project.area_id.clone()),
                                project_id: Some(project.id.clone()),
                            },
                        );
                    }
                }
                PersonaScope::Global | PersonaScope::Feature { .. } => {}
            }
        }
    }

    /// Build a persona chain for a given session context.
    ///
    /// Returns personas ordered: global first, most specific last.
    /// Later personas override tone; skills and instructions merge.
    pub fn for_session(&self, context: &SessionContextRow) -> PersonaChain {
        let mut matched: Vec<&Persona> = Vec::new();

        for persona in &self.personas {
            if self.persona_matches(persona, context) {
                matched.push(persona);
            }
        }

        // Sort: Global first, then Area, then Project/Feature
        matched.sort_by_key(|p| match &p.scope {
            PersonaScope::Global => 0,
            PersonaScope::Area { .. } => 1,
            PersonaScope::Project { .. } => 2,
            PersonaScope::Feature { .. } => 2,
        });

        let mut merged_skills = HashSet::new();
        let mut instructions_parts = Vec::new();
        let mut tone = None;

        for persona in &matched {
            for skill in &persona.skills {
                merged_skills.insert(skill.clone());
            }
            if !persona.instructions.trim().is_empty() {
                instructions_parts.push(persona.instructions.clone());
            }
            if persona.tone.is_some() {
                tone = persona.tone.clone();
            }
        }

        let merged_instructions = instructions_parts.join("\n\n---\n\n");

        PersonaChain {
            personas: matched.into_iter().cloned().collect(),
            merged_skills,
            merged_instructions,
            tone,
        }
    }

    /// Hot-reload personas from disk.
    pub async fn reload(&mut self, personas_dir: &Path) {
        let reloaded = Self::load(personas_dir).await;
        self.personas = reloaded.personas;
        // Keep resolved scopes — caller should call resolve_scopes() again if needed.
    }

    /// Get all loaded personas.
    pub fn all(&self) -> &[Persona] {
        &self.personas
    }

    fn persona_matches(&self, persona: &Persona, context: &SessionContextRow) -> bool {
        match &persona.scope {
            PersonaScope::Global => true,
            PersonaScope::Area { .. } => {
                if let Some(resolved) = self.resolved_scopes.get(&persona.name) {
                    resolved.area_id.as_deref() == context.area_id.as_deref()
                        && context.area_id.is_some()
                } else {
                    false
                }
            }
            PersonaScope::Project { .. } => {
                if let Some(resolved) = self.resolved_scopes.get(&persona.name) {
                    resolved.project_id.as_deref() == context.project_id.as_deref()
                        && context.project_id.is_some()
                } else {
                    false
                }
            }
            PersonaScope::Feature { kind } => context.entity_kind.as_deref().is_some_and(|ek| {
                ek == kind
                    || (ek.starts_with(kind.as_str())
                        && ek.as_bytes().get(kind.len()) == Some(&b'.'))
            }),
        }
    }
}

/// YAML frontmatter schema for persona files.
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct PersonaFrontmatter {
    name: Option<String>,
    scope: Option<HashMap<String, String>>,
    skills: Vec<String>,
    tone: Option<String>,
    #[serde(flatten)]
    extra: HashMap<String, serde_json::Value>,
}

async fn load_persona_file(path: &Path) -> Result<Persona, String> {
    let content = fs::read_to_string(path)
        .await
        .map_err(|e| format!("read error: {e}"))?;

    let file_stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    parse_persona(file_stem, &content)
}

fn global_persona(name: &str, instructions: &str) -> Persona {
    Persona {
        name: name.to_string(),
        instructions: instructions.to_string(),
        scope: PersonaScope::Global,
        skills: Vec::new(),
        tone: None,
        metadata: HashMap::new(),
    }
}

fn parse_persona(default_name: &str, content: &str) -> Result<Persona, String> {
    let lines: Vec<&str> = content.lines().collect();

    if lines.is_empty() || !lines[0].trim().starts_with("---") {
        return Ok(global_persona(default_name, content));
    }

    // Find end of frontmatter
    let mut end_idx = 0;
    for (i, line) in lines.iter().enumerate().skip(1) {
        if line.trim().starts_with("---") {
            end_idx = i;
            break;
        }
    }

    if end_idx == 0 {
        return Ok(global_persona(default_name, content));
    }

    let frontmatter_str = lines[1..end_idx].join("\n");
    let fm: PersonaFrontmatter =
        serde_yaml::from_str(&frontmatter_str).map_err(|e| format!("YAML parse error: {e}"))?;

    let instructions = lines[(end_idx + 1)..].join("\n");

    let scope = match fm.scope {
        Some(map) => {
            if let Some(area) = map.get("area") {
                PersonaScope::Area { name: area.clone() }
            } else if let Some(project) = map.get("project") {
                PersonaScope::Project {
                    name: project.clone(),
                }
            } else if let Some(feature) = map.get("feature") {
                PersonaScope::Feature {
                    kind: feature.clone(),
                }
            } else {
                PersonaScope::Global
            }
        }
        None => PersonaScope::Global,
    };

    let name = fm.name.unwrap_or_else(|| default_name.to_string());

    Ok(Persona {
        name,
        instructions,
        scope,
        skills: fm.skills,
        tone: fm.tone,
        metadata: fm.extra,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn make_context(
        session_key: &str,
        entity_kind: Option<&str>,
        entity_id: Option<&str>,
        area_id: Option<&str>,
        project_id: Option<&str>,
    ) -> SessionContextRow {
        SessionContextRow {
            session_key: session_key.to_string(),
            context_type: "general".to_string(),
            entity_kind: entity_kind.map(|s| s.to_string()),
            entity_id: entity_id.map(|s| s.to_string()),
            area_id: area_id.map(|s| s.to_string()),
            project_id: project_id.map(|s| s.to_string()),
            is_ephemeral: false,
            is_pinned: false,
            created_at: storage::SqlTs(jiff::Timestamp::now()),
            updated_at: storage::SqlTs(jiff::Timestamp::now()),
        }
    }

    #[test]
    fn persona_parse_yaml_frontmatter() {
        let content = r#"---
name: finance-expert
scope:
  feature: finance
skills:
  - finance
  - summarize
tone: professional
---

You are a financial advisor helping manage budgets and investments.
"#;
        let persona = parse_persona("default", content).unwrap();
        assert_eq!(persona.name, "finance-expert");
        assert_eq!(
            persona.scope,
            PersonaScope::Feature {
                kind: "finance".to_string()
            }
        );
        assert_eq!(persona.skills, vec!["finance", "summarize"]);
        assert_eq!(persona.tone.as_deref(), Some("professional"));
        assert!(persona.instructions.contains("financial advisor"));
    }

    #[test]
    fn persona_parse_no_frontmatter() {
        let content = "Just raw instructions with no frontmatter.";
        let persona = parse_persona("raw", content).unwrap();
        assert_eq!(persona.name, "raw");
        assert_eq!(persona.scope, PersonaScope::Global);
        assert!(persona.skills.is_empty());
        assert!(persona.instructions.contains("raw instructions"));
    }

    #[test]
    fn persona_parse_global_scope() {
        let content = r#"---
name: base
tone: friendly
---

Global instructions.
"#;
        let persona = parse_persona("base", content).unwrap();
        assert_eq!(persona.scope, PersonaScope::Global);
        assert_eq!(persona.tone.as_deref(), Some("friendly"));
    }

    #[test]
    fn persona_parse_area_scope() {
        let content = r#"---
scope:
  area: "Engineering"
---

Engineering area persona.
"#;
        let persona = parse_persona("eng", content).unwrap();
        assert_eq!(
            persona.scope,
            PersonaScope::Area {
                name: "Engineering".to_string()
            }
        );
    }

    #[test]
    fn persona_parse_project_scope() {
        let content = r#"---
scope:
  project: "CryptoGuard"
skills:
  - todo
---

CryptoGuard project persona.
"#;
        let persona = parse_persona("cg", content).unwrap();
        assert_eq!(
            persona.scope,
            PersonaScope::Project {
                name: "CryptoGuard".to_string()
            }
        );
        assert_eq!(persona.skills, vec!["todo"]);
    }

    #[tokio::test]
    async fn persona_load_all_from_directory() {
        let dir = TempDir::new().unwrap();

        // Write two persona files
        let mut f1 = std::fs::File::create(dir.path().join("global.md")).unwrap();
        writeln!(f1, "---\nname: global\n---\nGlobal instructions.").unwrap();

        let mut f2 = std::fs::File::create(dir.path().join("finance.md")).unwrap();
        writeln!(
            f2,
            "---\nname: finance\nscope:\n  feature: finance\nskills:\n  - finance\n---\nFinance instructions."
        )
        .unwrap();

        let mgr = PersonaManager::load(dir.path()).await;
        assert_eq!(mgr.all().len(), 2);
    }

    #[tokio::test]
    async fn persona_empty_dir_returns_empty() {
        let dir = TempDir::new().unwrap();
        let mgr = PersonaManager::load(dir.path()).await;
        assert!(mgr.all().is_empty());
    }

    #[tokio::test]
    async fn persona_nonexistent_dir_returns_empty() {
        let mgr = PersonaManager::load(Path::new("/nonexistent/path")).await;
        assert!(mgr.all().is_empty());
    }

    #[test]
    fn persona_for_session_global_always_matches() {
        let global = Persona {
            name: "global".to_string(),
            instructions: "Be helpful.".to_string(),
            scope: PersonaScope::Global,
            skills: vec!["todo".to_string()],
            tone: Some("friendly".to_string()),
            metadata: HashMap::new(),
        };
        let mgr = PersonaManager {
            personas: vec![global],
            resolved_scopes: HashMap::new(),
            personas_dir: std::path::PathBuf::new(),
        };

        let ctx = make_context("test:1", None, None, None, None);
        let chain = mgr.for_session(&ctx);
        assert_eq!(chain.personas.len(), 1);
        assert!(chain.merged_skills.contains("todo"));
        assert_eq!(chain.tone.as_deref(), Some("friendly"));
    }

    #[test]
    fn persona_for_session_feature_match() {
        let global = Persona {
            name: "global".to_string(),
            instructions: "Be helpful.".to_string(),
            scope: PersonaScope::Global,
            skills: vec![],
            tone: Some("friendly".to_string()),
            metadata: HashMap::new(),
        };
        let finance = Persona {
            name: "finance".to_string(),
            instructions: "Focus on finance.".to_string(),
            scope: PersonaScope::Feature {
                kind: "finance".to_string(),
            },
            skills: vec!["finance".to_string()],
            tone: Some("professional".to_string()),
            metadata: HashMap::new(),
        };
        let mgr = PersonaManager {
            personas: vec![global, finance],
            resolved_scopes: HashMap::new(),
            personas_dir: std::path::PathBuf::new(),
        };

        // Finance session: both global and finance match
        let ctx = make_context("test:2", Some("finance.budgets"), None, None, None);
        let chain = mgr.for_session(&ctx);
        assert_eq!(chain.personas.len(), 2);
        assert!(chain.merged_skills.contains("finance"));
        // Last persona overrides tone
        assert_eq!(chain.tone.as_deref(), Some("professional"));
        assert!(chain.merged_instructions.contains("Be helpful."));
        assert!(chain.merged_instructions.contains("Focus on finance."));

        // Non-finance session: only global matches
        let ctx2 = make_context("test:3", Some("task"), Some("t-1"), None, None);
        let chain2 = mgr.for_session(&ctx2);
        assert_eq!(chain2.personas.len(), 1);
        assert_eq!(chain2.personas[0].name, "global");
    }

    #[test]
    fn persona_for_session_cascade_order() {
        let global = Persona {
            name: "global".to_string(),
            instructions: "Global.".to_string(),
            scope: PersonaScope::Global,
            skills: vec!["base".to_string()],
            tone: None,
            metadata: HashMap::new(),
        };
        let area = Persona {
            name: "eng".to_string(),
            instructions: "Engineering.".to_string(),
            scope: PersonaScope::Area {
                name: "Engineering".to_string(),
            },
            skills: vec!["todo".to_string()],
            tone: Some("concise".to_string()),
            metadata: HashMap::new(),
        };
        let project = Persona {
            name: "cg".to_string(),
            instructions: "CryptoGuard.".to_string(),
            scope: PersonaScope::Project {
                name: "CryptoGuard".to_string(),
            },
            skills: vec!["finance".to_string()],
            tone: Some("technical".to_string()),
            metadata: HashMap::new(),
        };

        let mut resolved = HashMap::new();
        resolved.insert(
            "eng".to_string(),
            ResolvedScope {
                area_id: Some("a-1".to_string()),
                project_id: None,
            },
        );
        resolved.insert(
            "cg".to_string(),
            ResolvedScope {
                area_id: Some("a-1".to_string()),
                project_id: Some("p-1".to_string()),
            },
        );

        let mgr = PersonaManager {
            personas: vec![global, area, project],
            resolved_scopes: resolved,
        };

        let ctx = make_context(
            "test:4",
            Some("project"),
            Some("p-1"),
            Some("a-1"),
            Some("p-1"),
        );
        let chain = mgr.for_session(&ctx);

        // All three should match, in order: global → area → project
        assert_eq!(chain.personas.len(), 3);
        assert_eq!(chain.personas[0].name, "global");
        assert_eq!(chain.personas[1].name, "eng");
        assert_eq!(chain.personas[2].name, "cg");
        // Skills merged from all
        assert!(chain.merged_skills.contains("base"));
        assert!(chain.merged_skills.contains("todo"));
        assert!(chain.merged_skills.contains("finance"));
        // Tone from most specific
        assert_eq!(chain.tone.as_deref(), Some("technical"));
    }
}
