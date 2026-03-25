use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;
use context_engine::source::{ContextSource, SourceContext};
use tokio::sync::RwLock;

use crate::types::SkillPackage;

/// Injects active orchestrator instructions + always-loaded skills + activated skills.
/// Long-lived shared-state design (same pattern as old AgentContextSource).
///
/// Features per Agent Skills spec:
/// - Progressive disclosure (Tier 2: full SKILL.md on activation)
/// - Deduplication (tracks activated skills per session)
/// - Context compaction protection (marked as protected)
/// - Resource enumeration (lists bundled files alongside instructions)
pub struct SkillContextSource {
    active_orchestrator: Arc<RwLock<Option<Arc<SkillPackage>>>>,
    activated_skills: Arc<RwLock<Vec<Arc<SkillPackage>>>>,
    /// Preloaded reference files keyed by "builtin::{skill}/references/{name}.md"
    /// or "{absolute_path}/references/{name}.md" for filesystem skills.
    reference_files: Arc<HashMap<String, String>>,
    /// Tracks which skills have been injected in the current session to prevent duplicates.
    activated_names: Arc<RwLock<HashSet<String>>>,
}

impl SkillContextSource {
    pub fn new(
        active_orchestrator: Arc<RwLock<Option<Arc<SkillPackage>>>>,
        activated_skills: Arc<RwLock<Vec<Arc<SkillPackage>>>>,
        reference_files: Arc<HashMap<String, String>>,
    ) -> Self {
        Self {
            active_orchestrator,
            activated_skills,
            reference_files,
            activated_names: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// Load always_skills reference files from the orchestrator's directory.
    /// Filters multi-token references by relevance to the user message.
    fn always_skill_content(&self, orchestrator: &SkillPackage, message: Option<&str>) -> Vec<String> {
        let mut content = Vec::new();
        let msg_lower = message.map(|m| m.to_lowercase());

        for skill_name in orchestrator.always_skills() {
            // Relevance check: single-token refs always load, multi-token refs need a match
            if let Some(ref msg) = msg_lower {
                if !is_reference_relevant(skill_name, msg) {
                    tracing::debug!(
                        skill = %skill_name,
                        "Skipping always-skill reference (not relevant to message)"
                    );
                    continue;
                }
            }

            let fs_key = format!(
                "{}/references/{}.md",
                orchestrator.location.display(),
                skill_name
            );
            let builtin_key = format!(
                "builtin::{}/references/{}.md",
                orchestrator.name, skill_name
            );
            let text = self
                .reference_files
                .get(&fs_key)
                .or_else(|| self.reference_files.get(&builtin_key));

            if let Some(text) = text {
                content.push(format!("# Skill: {}\n\n{}", skill_name, text));
            } else {
                tracing::debug!(
                    skill = %skill_name,
                    orchestrator = %orchestrator.name,
                    "Always-skill reference not found (tried: {fs_key}, {builtin_key})"
                );
            }
        }
        content
    }

    /// Format resource listing XML for a skill (Tier 3 enumeration).
    fn resource_listing(pkg: &SkillPackage) -> String {
        if pkg.resources.is_empty() {
            return String::new();
        }
        let mut lines = vec!["\n<skill_resources>".to_string()];
        for res in &pkg.resources {
            lines.push(format!("  <file>{res}</file>"));
        }
        lines.push("</skill_resources>".to_string());
        lines.push(format!("Skill directory: {}", pkg.location.display()));
        lines.push("Relative paths in this skill are relative to the skill directory.".to_string());
        lines.join("\n")
    }
}

#[async_trait]
impl ContextSource for SkillContextSource {
    fn name(&self) -> &str {
        "skill_profile"
    }

    fn priority(&self) -> u8 {
        35
    }

    /// Skill context is protected from context compaction per Agent Skills spec.
    /// "Skill instructions are durable behavioral guidance — losing them mid-conversation
    /// silently degrades the agent's performance."
    fn protected(&self) -> bool {
        true
    }

    async fn provide(&self, ctx: &SourceContext) -> Option<String> {
        let orchestrator_guard = self.active_orchestrator.read().await;
        let orchestrator = orchestrator_guard.as_ref()?;

        let mut sections = Vec::new();
        let mut names_guard = self.activated_names.write().await;

        // Orchestrator body (wrapped in identifying tags per spec)
        if !orchestrator.body.is_empty() {
            let already_active = !names_guard.insert(orchestrator.name.clone());
            if !already_active {
                sections.push(format!(
                    "<skill_content name=\"{}\">\n# Agent: {}\n\n{}{}",
                    orchestrator.name,
                    orchestrator.name,
                    orchestrator.body,
                    Self::resource_listing(orchestrator)
                ));
                sections.push("</skill_content>".to_string());
            }
        }

        // Always-loaded skills
        let message_text = ctx.message.as_deref();
        sections.extend(self.always_skill_content(orchestrator, message_text));

        // Per-message activated skills: inject SUMMARY only (progressive loading)
        let skills_guard = self.activated_skills.read().await;
        for pkg in skills_guard.iter() {
            let already_active = !names_guard.insert(pkg.name.clone());
            if already_active {
                tracing::debug!(skill = %pkg.name, "Skipping duplicate skill activation");
                continue;
            }
            // Progressive: summary + resource listing (not full body)
            sections.push(format!(
                "<skill_content name=\"{}\" mode=\"summary\">\n# Skill: {} (activated)\n\n{}\n\nUse the `skill_reference` tool to load full instructions if needed.{}",
                pkg.name,
                pkg.name,
                pkg.summary,
                Self::resource_listing(pkg)
            ));
            sections.push("</skill_content>".to_string());
        }

        if sections.is_empty() {
            None
        } else {
            Some(sections.join("\n\n---\n\n"))
        }
    }

    fn estimated_tokens(&self) -> usize {
        // Compute from actual content rather than hardcoding 500.
        // Use blocking try_read to avoid async in a sync trait method;
        // falls back to a conservative estimate if locks are held.
        let orchestrator_tokens = self
            .active_orchestrator
            .try_read()
            .ok()
            .and_then(|guard| guard.as_ref().map(|pkg| pkg.body.len() / 4))
            .unwrap_or(500);

        let activated_tokens = self
            .activated_skills
            .try_read()
            .ok()
            .map(|guard| guard.iter().map(|pkg| pkg.body.len() / 4).sum::<usize>())
            .unwrap_or(0);

        // Add overhead for XML tags, headings, resource listings
        let overhead = 50;
        orchestrator_tokens + activated_tokens + overhead
    }
}

/// Check if a reference name is relevant to the user message.
/// Single-token references (no hyphens) are always loaded.
/// Multi-token references require at least one token match.
fn is_reference_relevant(reference_name: &str, message_lower: &str) -> bool {
    let tokens: Vec<&str> = reference_name.split('-').collect();
    if tokens.len() == 1 {
        return true;
    }
    tokens.iter().any(|t| t.len() > 2 && message_lower.contains(t))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::SkillSource;
    use crate::types::SkillCatalog;

    fn make_catalog_with_always_skills() -> (SkillCatalog, HashMap<String, String>) {
        let skills = vec![(
            "task-mgmt".to_string(),
            "---\nname: task-mgmt\ndescription: Task management.\nmetadata:\n  klyntbot:\n    type: orchestrator\n    always_skills: [todo]\n---\nYou are the task agent.".to_string(),
        )];
        let source = SkillSource::BuiltIn(skills);
        let catalog = SkillCatalog::discover_sync(&[source]).unwrap();

        let mut reference_files = HashMap::new();
        reference_files.insert(
            "builtin::task-mgmt/references/todo.md".to_string(),
            "# Todo Workflow\n\nCreate tasks using the task tool.".to_string(),
        );
        (catalog, reference_files)
    }

    #[tokio::test]
    async fn test_context_source_injects_orchestrator_body() {
        let (catalog, refs) = make_catalog_with_always_skills();
        let pkg = catalog.get("task-mgmt").unwrap().clone();
        let active = Arc::new(RwLock::new(Some(pkg)));
        let activated = Arc::new(RwLock::new(vec![]));
        let source = SkillContextSource::new(active, activated, Arc::new(refs));

        let ctx = SourceContext {
            channel: "test".into(),
            chat_id: "1".into(),
            message: None,
            intent_summary: None,
            project_id: None,
        };
        let result = source.provide(&ctx).await.unwrap();
        assert!(
            result.contains("task agent"),
            "Should contain orchestrator body"
        );
        assert!(
            result.contains("<skill_content"),
            "Should wrap in skill_content tags"
        );
    }

    #[tokio::test]
    async fn test_context_source_injects_always_skills() {
        let (catalog, refs) = make_catalog_with_always_skills();
        let pkg = catalog.get("task-mgmt").unwrap().clone();
        let active = Arc::new(RwLock::new(Some(pkg)));
        let activated = Arc::new(RwLock::new(vec![]));
        let source = SkillContextSource::new(active, activated, Arc::new(refs));

        let ctx = SourceContext {
            channel: "test".into(),
            chat_id: "1".into(),
            message: None,
            intent_summary: None,
            project_id: None,
        };
        let result = source.provide(&ctx).await.unwrap();
        assert!(
            result.contains("Todo Workflow"),
            "Should contain always-loaded skill content"
        );
    }

    #[tokio::test]
    async fn test_context_source_is_protected() {
        let active = Arc::new(RwLock::new(None));
        let activated = Arc::new(RwLock::new(vec![]));
        let source = SkillContextSource::new(active, activated, Arc::new(HashMap::new()));
        assert!(
            source.protected(),
            "Skill context should be protected from compaction"
        );
    }

    #[tokio::test]
    async fn test_estimated_tokens_scales_with_content() {
        let (catalog, refs) = make_catalog_with_always_skills();
        let pkg = catalog.get("task-mgmt").unwrap().clone();
        let body_len = pkg.body.len();

        let active = Arc::new(RwLock::new(Some(pkg)));
        let activated = Arc::new(RwLock::new(vec![]));
        let source = SkillContextSource::new(active, activated, Arc::new(refs));

        let estimate = source.estimated_tokens();
        // Should be roughly body_len/4 + overhead, NOT the old hardcoded 500
        assert!(estimate > 50, "estimate should include overhead at minimum");
        // Should scale with the orchestrator body length
        let expected_min = body_len / 4;
        assert!(
            estimate >= expected_min,
            "estimate ({estimate}) should be at least body_len/4 ({expected_min})"
        );
    }

    #[tokio::test]
    async fn test_activated_skills_inject_summary_not_full_body() {
        let skills = vec![
            (
                "general".to_string(),
                "---\nname: general\ndescription: General.\nmetadata:\n  klyntbot:\n    type: orchestrator\n---\nOrchestrator body.".to_string(),
            ),
            (
                "helper".to_string(),
                "---\nname: helper\ndescription: Helper skill.\nmetadata:\n  klyntbot:\n    type: skill\n    summary: Helper does X and Y.\n---\nThis is a very long full body that should NOT appear in activated skill context. It contains detailed instructions.".to_string(),
            ),
        ];
        let source = SkillSource::BuiltIn(skills);
        let catalog = SkillCatalog::discover_sync(&[source]).unwrap();

        let orchestrator = catalog.get("general").unwrap().clone();
        let helper = catalog.get("helper").unwrap().clone();

        let active = Arc::new(RwLock::new(Some(orchestrator)));
        let activated = Arc::new(RwLock::new(vec![helper]));
        let source = SkillContextSource::new(active, activated, Arc::new(HashMap::new()));

        let ctx = SourceContext {
            channel: "test".into(),
            chat_id: "1".into(),
            message: None,
            intent_summary: None,
            project_id: None,
        };
        let result = source.provide(&ctx).await.unwrap();

        assert!(result.contains("Orchestrator body"), "orchestrator gets full body");
        assert!(result.contains("Helper does X and Y"), "activated skill shows summary");
        assert!(!result.contains("very long full body"), "activated skill should NOT show full body");
    }

    #[tokio::test]
    async fn test_always_skills_filtered_by_relevance() {
        let skills = vec![(
            "task-mgmt".to_string(),
            "---\nname: task-mgmt\ndescription: Task management.\nmetadata:\n  klyntbot:\n    type: orchestrator\n    always_skills: [todo, daily-planner]\n---\nYou are the task agent.".to_string(),
        )];
        let source_data = SkillSource::BuiltIn(skills);
        let catalog = SkillCatalog::discover_sync(&[source_data]).unwrap();

        let mut reference_files = HashMap::new();
        reference_files.insert(
            "builtin::task-mgmt/references/todo.md".to_string(),
            "# Todo Workflow\nCreate and manage tasks.".to_string(),
        );
        reference_files.insert(
            "builtin::task-mgmt/references/daily-planner.md".to_string(),
            "# Daily Planner\nPlan your day with time blocking.".to_string(),
        );

        let pkg = catalog.get("task-mgmt").unwrap().clone();
        let active = Arc::new(RwLock::new(Some(pkg)));
        let activated = Arc::new(RwLock::new(vec![]));
        let ctx_source = SkillContextSource::new(active, activated, Arc::new(reference_files));

        let ctx = SourceContext {
            channel: "test".into(),
            chat_id: "1".into(),
            message: Some("mark my task as done".to_string()),
            intent_summary: None,
            project_id: None,
        };
        let result = ctx_source.provide(&ctx).await.unwrap();

        assert!(result.contains("Todo Workflow"), "single-token ref should always load");
        assert!(!result.contains("Daily Planner"), "multi-token ref should be filtered when irrelevant");
    }
}
