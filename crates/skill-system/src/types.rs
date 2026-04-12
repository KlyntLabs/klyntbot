use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

const MCP_WILDCARD: &str = "*";
const ASK_USER_TOOL_NAME: &str = "ask_user";

/// Callback type for embedding text. Avoids depending on cognitive::TextEmbedder.
pub type EmbedFn = Arc<dyn Fn(&str) -> common::Result<Vec<f32>> + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SkillType {
    #[default]
    Skill,
    Orchestrator,
    Persona,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillScope {
    BuiltIn,
    User,
    Project,
}

#[derive(Debug, Clone)]
pub struct SkillPackage {
    pub name: String,
    pub description: String,
    pub skill_type: SkillType,
    pub scope: SkillScope,
    pub location: PathBuf,
    pub body: String,
    pub metadata: SkillMetadata,
    /// Bundled resource file paths (scripts/, references/, assets/) — enumerated at discovery.
    pub resources: Vec<String>,
    pub loaded_at: SystemTime,
    pub trusted: bool,
    /// One-line summary for progressive skill catalog. From frontmatter or first sentence of body.
    pub summary: String,
}

impl SkillPackage {
    /// Returns None if all tools allowed (tools field omitted/null).
    /// Returns Some(set) with ask_user always included when tools is explicit.
    pub fn allowed_tool_names(&self) -> Option<HashSet<String>> {
        let tools = self.metadata.klyntbot.as_ref()?.tools.as_ref()?;
        let mut set: HashSet<String> = tools.iter().cloned().collect();
        set.insert(ASK_USER_TOOL_NAME.to_string());
        Some(set)
    }

    /// Check if this skill allows tools from the given MCP server name.
    pub fn allows_mcp_server(&self, server_name: &str) -> bool {
        self.metadata
            .klyntbot
            .as_ref()
            .map(|k| {
                k.mcp_tools
                    .iter()
                    .any(|s| s == MCP_WILDCARD || s == server_name)
            })
            .unwrap_or(false)
    }

    /// Max iterations for ReAct loop. Falls back to default 10.
    pub fn max_iterations(&self) -> u32 {
        self.metadata
            .klyntbot
            .as_ref()
            .and_then(|k| k.max_iterations)
            .unwrap_or(10)
    }

    /// Skills to delegate to (orchestrator only).
    pub fn can_delegate_to(&self) -> &[String] {
        self.metadata
            .klyntbot
            .as_ref()
            .map(|k| k.can_delegate_to.as_slice())
            .unwrap_or(&[])
    }

    /// Always-loaded reference file names (resolved to references/<name>.md).
    pub fn always_skills(&self) -> &[String] {
        self.metadata
            .klyntbot
            .as_ref()
            .map(|k| k.always_skills.as_slice())
            .unwrap_or(&[])
    }

    /// Trigger phrases for routing boost.
    pub fn triggers(&self) -> &[String] {
        self.metadata
            .klyntbot
            .as_ref()
            .map(|k| k.triggers.as_slice())
            .unwrap_or(&[])
    }

    /// Schema hints for field semantics.
    pub fn schema_hints(&self) -> Option<&HashMap<String, SchemaHint>> {
        self.metadata.klyntbot.as_ref()?.schema_hints.as_ref()
    }

    /// Salience declaration for event classification.
    pub fn salience_declaration(&self) -> Option<&SalienceDeclaration> {
        self.metadata.klyntbot.as_ref()?.salience.as_ref()
    }

    /// Context injection rules.
    pub fn context_rules(&self) -> Option<&ContextRules> {
        self.metadata.klyntbot.as_ref()?.context_rules.as_ref()
    }
}

#[derive(Debug, Clone, Default)]
pub struct SkillMetadata {
    pub license: Option<String>,
    pub compatibility: Option<String>,
    pub custom: HashMap<String, serde_json::Value>,
    pub klyntbot: Option<KlyntbotMeta>,
}

#[derive(Debug, Clone, Default)]
pub struct KlyntbotMeta {
    pub skill_type: SkillType,
    pub tools: Option<Vec<String>>,
    pub mcp_tools: Vec<String>,
    pub can_delegate_to: Vec<String>,
    pub max_iterations: Option<u32>,
    pub always_skills: Vec<String>,
    /// Skills this one may chain to (e.g., task-management → productivity).
    pub invokes: Vec<String>,
    /// Trigger phrases that boost this skill during routing.
    pub triggers: Vec<String>,
    /// Short summary for progressive skill loading (Tier 1 catalog).
    pub summary: Option<String>,
    /// Per-field semantic hints for AI subsystems.
    pub schema_hints: Option<HashMap<String, SchemaHint>>,
    /// Salience classification rules for entity events.
    pub salience: Option<SalienceDeclaration>,
    /// Context injection rules for prompt assembly.
    pub context_rules: Option<ContextRules>,
}

/// Hints about a field's semantic role — used by AI subsystems (salience, context, reforge).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SchemaHint {
    #[serde(default)]
    pub lifecycle: Option<bool>,
    #[serde(default)]
    pub completion_values: Option<Vec<String>>,
    #[serde(default)]
    pub active_values: Option<Vec<String>>,
    #[serde(default)]
    pub temporal: Option<bool>,
    #[serde(default)]
    pub urgency_source: Option<bool>,
    #[serde(default)]
    pub ranking: Option<bool>,
    #[serde(default)]
    pub behavioral: Option<bool>,
    #[serde(default)]
    pub grouping: Option<bool>,
    #[serde(default)]
    pub budget_field: Option<bool>,
}

/// Salience declarations — tells the cognitive system how to classify entity events.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SalienceDeclaration {
    #[serde(default)]
    pub extract_on: Vec<SalienceRule>,
    #[serde(default)]
    pub accumulate_on: Vec<SalienceRule>,
}

/// A single salience rule matching a field change or event type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SalienceRule {
    #[serde(default)]
    pub field: Option<String>,
    #[serde(default)]
    pub event: Option<String>,
    #[serde(default)]
    pub to_values: Option<Vec<String>>,
    #[serde(default)]
    pub importance: Option<f64>,
}

/// Rules for context injection — how to select and format active entities for the LLM prompt.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextRules {
    pub active_filter: Option<String>,
    pub sort_by: Option<String>,
    pub max_items: Option<usize>,
    pub format: Option<String>,
}

#[derive(Debug, Clone)]
pub enum SkillChange {
    Added(String),
    Removed(String),
    Updated(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_type_default_is_skill() {
        assert!(matches!(SkillType::default(), SkillType::Skill));
    }

    #[test]
    fn test_klyntbot_meta_tools_none_means_all_allowed() {
        let meta = KlyntbotMeta::default();
        assert!(meta.tools.is_none(), "None = all tools allowed");
    }

    #[test]
    fn test_klyntbot_meta_tools_empty_means_deny_all() {
        let meta = KlyntbotMeta {
            tools: Some(vec![]),
            ..Default::default()
        };
        assert_eq!(meta.tools.as_ref().unwrap().len(), 0, "Some([]) = deny all");
    }

    #[test]
    fn test_skill_package_allowed_tool_names_none_means_full_access() {
        let pkg = SkillPackage {
            name: "test".into(),
            description: "test".into(),
            skill_type: SkillType::Skill,
            scope: SkillScope::BuiltIn,
            location: PathBuf::new(),
            body: String::new(),
            metadata: SkillMetadata::default(),
            resources: Vec::new(),
            loaded_at: SystemTime::now(),
            trusted: true,
            summary: String::new(),
        };
        assert!(pkg.allowed_tool_names().is_none());
    }

    #[test]
    fn test_skill_package_allowed_tool_names_explicit_list() {
        let pkg = SkillPackage {
            metadata: SkillMetadata {
                klyntbot: Some(KlyntbotMeta {
                    tools: Some(vec!["tasks".into(), "notes".into()]),
                    ..Default::default()
                }),
                ..Default::default()
            },
            name: "test".into(),
            description: "test".into(),
            skill_type: SkillType::Skill,
            scope: SkillScope::BuiltIn,
            location: PathBuf::new(),
            body: String::new(),
            resources: Vec::new(),
            loaded_at: SystemTime::now(),
            trusted: true,
            summary: String::new(),
        };
        let allowed = pkg.allowed_tool_names().unwrap();
        assert!(allowed.contains("tasks"));
        assert!(allowed.contains("notes"));
        assert!(allowed.contains("ask_user"));
        assert!(!allowed.contains("finance"));
    }

    #[test]
    fn test_allows_mcp_server_wildcard() {
        let pkg = SkillPackage {
            metadata: SkillMetadata {
                klyntbot: Some(KlyntbotMeta {
                    mcp_tools: vec!["*".into()],
                    ..Default::default()
                }),
                ..Default::default()
            },
            name: "t".into(),
            description: "t".into(),
            skill_type: SkillType::Skill,
            scope: SkillScope::BuiltIn,
            location: PathBuf::new(),
            body: String::new(),
            resources: Vec::new(),
            loaded_at: SystemTime::now(),
            trusted: true,
            summary: String::new(),
        };
        assert!(pkg.allows_mcp_server("anything"));
    }

    #[test]
    fn test_allows_mcp_server_empty_denies() {
        let pkg = SkillPackage {
            metadata: SkillMetadata::default(),
            name: "t".into(),
            description: "t".into(),
            skill_type: SkillType::Skill,
            scope: SkillScope::BuiltIn,
            location: PathBuf::new(),
            body: String::new(),
            resources: Vec::new(),
            loaded_at: SystemTime::now(),
            trusted: true,
            summary: String::new(),
        };
        assert!(!pkg.allows_mcp_server("linear"));
    }
}
