//! Tool for on-demand loading of skill instructions and reference files.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tools_core::Tool;

use crate::RoutingContext;

/// Read-only index of skill bodies and reference files.
pub struct SkillReferenceIndex {
    skill_bodies: HashMap<String, String>,
    reference_files: HashMap<String, String>,
}

impl SkillReferenceIndex {
    pub fn new(
        skill_bodies: HashMap<String, String>,
        reference_files: HashMap<String, String>,
    ) -> Self {
        Self {
            skill_bodies,
            reference_files,
        }
    }

    pub fn get_skill_body(&self, skill_name: &str) -> Option<&str> {
        self.skill_bodies.get(skill_name).map(|s| s.as_str())
    }

    pub fn get_reference(&self, skill_name: &str, ref_name: &str) -> Option<&str> {
        let key = format!("{}/{}", skill_name, ref_name);
        self.reference_files.get(&key).map(|s| s.as_str())
    }

    pub fn list_available(&self) -> String {
        use std::collections::BTreeMap;
        // Group references by skill name in a single pass
        let mut grouped: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for ref_key in self.reference_files.keys() {
            if let Some(slash_idx) = ref_key.find('/') {
                let skill = &ref_key[..slash_idx];
                let name = &ref_key[slash_idx + 1..];
                grouped.entry(skill).or_default().push(name);
            }
        }

        let mut lines = vec!["Available skill content:".to_string()];
        for name in self.skill_bodies.keys() {
            lines.push(format!("  - skill_body: {name}"));
            if let Some(refs) = grouped.get(name.as_str()) {
                for ref_name in refs {
                    lines.push(format!("    - reference: {ref_name}"));
                }
            }
        }
        lines.join("\n")
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SkillReferenceParams {
    pub action: String,
    #[serde(default)]
    pub skill_name: Option<String>,
    #[serde(default)]
    pub reference_name: Option<String>,
}

pub struct SkillReferenceTool {
    index: Arc<SkillReferenceIndex>,
}

impl SkillReferenceTool {
    pub fn new(index: Arc<SkillReferenceIndex>) -> Self {
        Self { index }
    }
}

#[async_trait]
impl Tool for SkillReferenceTool {
    fn name(&self) -> &str {
        "skill_reference"
    }

    fn description(&self) -> &str {
        "Load full skill instructions or reference files. Use when a skill summary indicates you need detailed instructions for a task."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["get_body", "get_reference", "list"],
                    "description": "Action: get_body (full skill instructions), get_reference (specific reference file), list (available content)"
                },
                "skill_name": {
                    "type": "string",
                    "description": "Name of the skill"
                },
                "reference_name": {
                    "type": "string",
                    "description": "Reference file name without .md extension"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value, _ctx: &RoutingContext) -> common::Result<String> {
        let params: SkillReferenceParams = serde_json::from_value(args).map_err(|e| {
            common::KlyntbotError::Tool(common::ToolError::InvalidParams(e.to_string()))
        })?;

        match params.action.as_str() {
            "list" => Ok(self.index.list_available()),
            "get_body" => {
                let name = params.skill_name.ok_or_else(|| {
                    common::KlyntbotError::Tool(common::ToolError::InvalidParams(
                        "skill_name required for get_body".into(),
                    ))
                })?;
                match self.index.get_skill_body(&name) {
                    Some(body) => Ok(body.to_string()),
                    None => Ok(format!(
                        "Skill '{name}' not found. Use action 'list' to see available skills."
                    )),
                }
            }
            "get_reference" => {
                let skill = params.skill_name.ok_or_else(|| {
                    common::KlyntbotError::Tool(common::ToolError::InvalidParams(
                        "skill_name required for get_reference".into(),
                    ))
                })?;
                let ref_name = params.reference_name.ok_or_else(|| {
                    common::KlyntbotError::Tool(common::ToolError::InvalidParams(
                        "reference_name required for get_reference".into(),
                    ))
                })?;
                match self.index.get_reference(&skill, &ref_name) {
                    Some(content) => Ok(content.to_string()),
                    None => Ok(format!(
                        "Reference '{ref_name}' not found for skill '{skill}'."
                    )),
                }
            }
            other => Ok(format!(
                "Unknown action '{other}'. Use 'list', 'get_body', or 'get_reference'."
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serve_builtin_skill_body() {
        let mut bodies = HashMap::new();
        bodies.insert(
            "task-management".to_string(),
            "Full task management instructions...".to_string(),
        );
        let index = SkillReferenceIndex::new(bodies, HashMap::new());
        assert_eq!(
            index.get_skill_body("task-management"),
            Some("Full task management instructions...")
        );
    }

    #[test]
    fn test_serve_reference_file() {
        let mut refs = HashMap::new();
        refs.insert(
            "task-management/todo".to_string(),
            "# Todo workflow".to_string(),
        );
        let index = SkillReferenceIndex::new(HashMap::new(), refs);
        assert_eq!(
            index.get_reference("task-management", "todo"),
            Some("# Todo workflow")
        );
    }

    #[test]
    fn test_list_available() {
        let mut bodies = HashMap::new();
        bodies.insert("general".to_string(), "body".to_string());
        let index = SkillReferenceIndex::new(bodies, HashMap::new());
        let listing = index.list_available();
        assert!(listing.contains("general"));
    }
}
