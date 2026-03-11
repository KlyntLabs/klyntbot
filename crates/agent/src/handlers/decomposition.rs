//! LLM-backed DecompositionHandler implementation.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;
use common::Result;
use providers::{ChatParams, DynProvider, Message, ResponseFormat};
use tracing::debug;

use feature_tasks::handlers::DecompositionHandler;
use feature_tasks::types::{
    DecompositionContext, DecompositionResult, DecompositionTree, PlannedSubtask, Task,
    ValidationWarning, ValidationWarningKind,
};

const DECOMPOSITION_PROMPT: &str = include_str!("../templates/decomposition_prompt.md");

/// LLM-powered decomposition handler.
pub struct LlmDecompositionHandler {
    provider: DynProvider,
    model: String,
    #[allow(dead_code)] // Used in later chunks for persistence
    repo: storage::TaskRepo,
    #[allow(dead_code)] // Used in later chunks for domain events
    domain_bus: Option<Arc<bus::DomainEventBus>>,
}

impl LlmDecompositionHandler {
    pub fn new(
        provider: DynProvider,
        model: String,
        repo: storage::TaskRepo,
        domain_bus: Option<Arc<bus::DomainEventBus>>,
    ) -> Self {
        Self {
            provider,
            model,
            repo,
            domain_bus,
        }
    }
}

/// LLM response structure for decomposition.
#[derive(serde::Deserialize)]
struct LlmDecompositionResponse {
    confidence: f64,
    reasoning: String,
    subtasks: Vec<LlmPlannedSubtask>,
    total_estimated_mins: Option<i32>,
}

#[derive(serde::Deserialize)]
struct LlmPlannedSubtask {
    temp_id: String,
    title: String,
    description: Option<String>,
    acceptance_criteria: Option<String>,
    estimated_minutes: Option<i32>,
    energy_level: Option<String>,
    priority: Option<i16>,
    task_type: Option<String>,
    #[serde(default)]
    dependencies: Vec<String>,
    #[serde(default)]
    children: Vec<LlmPlannedSubtask>,
}

#[async_trait]
impl DecompositionHandler for LlmDecompositionHandler {
    async fn decompose(
        &self,
        task: &Task,
        context: &DecompositionContext,
    ) -> Result<DecompositionResult> {
        let prompt = build_prompt(task, context);

        let params = ChatParams::new(&self.model)
            .with_temperature(0.2)
            .with_max_tokens(4096)
            .with_response_format(ResponseFormat::JsonObject);

        let messages = vec![
            Message::system("You are a task decomposition assistant. Return only valid JSON."),
            Message::user(prompt),
        ];

        let response = self.provider.chat(&messages, None, &params).await?;
        let content = response.content.unwrap_or_default();
        let json_str = common::utils::strip_llm_fences(&content);

        let parsed: LlmDecompositionResponse = serde_json::from_str(json_str).map_err(|e| {
            common::ToolError::ExecutionFailed(format!("Decomposition JSON parse failed: {e}"))
        })?;

        let mut tree = convert_tree(&parsed);
        let mut warnings = Vec::new();
        let mut confidence = parsed.confidence.clamp(0.0, 1.0);

        validate_and_repair_tree(&mut tree, context, &mut warnings, &mut confidence);

        // Floor confidence at 0.3
        confidence = confidence.max(0.3);

        debug!(
            task_id = %task.id,
            confidence,
            subtask_count = tree.subtasks.len(),
            "Decomposition complete"
        );

        Ok(DecompositionResult {
            tree,
            confidence,
            reasoning: parsed.reasoning,
            validation_warnings: warnings,
        })
    }
}

fn build_prompt(task: &Task, ctx: &DecompositionContext) -> String {
    let mut prompt = DECOMPOSITION_PROMPT.to_string();
    prompt = prompt.replace("{{title}}", &task.title);
    prompt = prompt.replace(
        "{{description}}",
        task.description.as_deref().unwrap_or("(none)"),
    );
    prompt = prompt.replace(
        "{{acceptance_criteria}}",
        task.acceptance_criteria.as_deref().unwrap_or("(none)"),
    );
    prompt = prompt.replace(
        "{{estimated_minutes}}",
        &task
            .estimated_minutes
            .map(|m| m.to_string())
            .unwrap_or_else(|| "(none)".to_string()),
    );
    prompt = prompt.replace(
        "{{energy_level}}",
        task.energy_level
            .as_ref()
            .map(|e| e.to_string())
            .as_deref()
            .unwrap_or("(none)"),
    );
    prompt = prompt.replace(
        "{{priority}}",
        &task
            .priority
            .map(|p| format!("P{p}"))
            .unwrap_or_else(|| "(none)".to_string()),
    );
    prompt = prompt.replace(
        "{{project_context}}",
        ctx.project_context.as_deref().unwrap_or("(none)"),
    );
    prompt = prompt.replace(
        "{{existing_subtasks}}",
        &if ctx.existing_subtasks.is_empty() {
            "(none)".to_string()
        } else {
            ctx.existing_subtasks.join(", ")
        },
    );
    prompt = prompt.replace(
        "{{cognitive_facts}}",
        &if ctx.cognitive_facts.is_empty() {
            String::new()
        } else {
            ctx.cognitive_facts.join("\n")
        },
    );
    prompt = prompt.replace("{{max_depth}}", &ctx.max_depth.to_string());
    prompt = prompt.replace(
        "{{max_subtasks_per_level}}",
        &ctx.max_subtasks_per_level.to_string(),
    );
    prompt
}

fn convert_subtask(llm: &LlmPlannedSubtask) -> PlannedSubtask {
    PlannedSubtask {
        temp_id: llm.temp_id.clone(),
        title: llm.title.clone(),
        description: llm.description.clone(),
        acceptance_criteria: llm.acceptance_criteria.clone(),
        estimated_minutes: llm.estimated_minutes,
        energy_level: llm.energy_level.as_deref().and_then(|s| s.parse().ok()),
        priority: llm.priority,
        task_type: llm.task_type.as_deref().and_then(|s| s.parse().ok()),
        dependencies: llm.dependencies.clone(),
        children: llm.children.iter().map(convert_subtask).collect(),
    }
}

fn convert_tree(llm: &LlmDecompositionResponse) -> DecompositionTree {
    DecompositionTree {
        subtasks: llm.subtasks.iter().map(convert_subtask).collect(),
        total_estimated_mins: llm.total_estimated_mins,
    }
}

/// Collect all temp_ids from the tree.
fn collect_ids(tree: &DecompositionTree) -> HashSet<String> {
    let mut ids = HashSet::new();
    fn walk(subtask: &PlannedSubtask, ids: &mut HashSet<String>) {
        ids.insert(subtask.temp_id.clone());
        for child in &subtask.children {
            walk(child, ids);
        }
    }
    for s in &tree.subtasks {
        walk(s, &mut ids);
    }
    ids
}

/// Build adjacency map from tree (temp_id → deps snapshot before repair).
fn build_dep_graph(tree: &DecompositionTree) -> HashMap<String, Vec<String>> {
    let mut graph = HashMap::new();
    fn walk(subtask: &PlannedSubtask, graph: &mut HashMap<String, Vec<String>>) {
        graph.insert(subtask.temp_id.clone(), subtask.dependencies.clone());
        for child in &subtask.children {
            walk(child, graph);
        }
    }
    for s in &tree.subtasks {
        walk(s, &mut graph);
    }
    graph
}

/// Validate and repair the tree in-place: remove invalid/circular deps, check counts.
fn validate_and_repair_tree(
    tree: &mut DecompositionTree,
    ctx: &DecompositionContext,
    warnings: &mut Vec<ValidationWarning>,
    confidence: &mut f64,
) {
    let valid_ids = collect_ids(tree);
    let graph = build_dep_graph(tree);
    let total_count = valid_ids.len();

    // Check duplicate temp_ids (collect_ids deduplicates, so compare with walk count)
    let mut walk_count = 0usize;
    fn count_nodes(subtask: &PlannedSubtask, count: &mut usize) {
        *count += 1;
        for child in &subtask.children {
            count_nodes(child, count);
        }
    }
    for s in &tree.subtasks {
        count_nodes(s, &mut walk_count);
    }
    if walk_count != valid_ids.len() {
        warnings.push(ValidationWarning {
            kind: ValidationWarningKind::DuplicateTitle,
            message: format!(
                "Duplicate temp_ids detected ({} nodes, {} unique)",
                walk_count,
                valid_ids.len()
            ),
            subtask_temp_id: None,
        });
        *confidence -= 0.05;
    }

    // Repair deps in-place on the tree
    fn repair_deps(
        subtask: &mut PlannedSubtask,
        valid_ids: &HashSet<String>,
        graph: &HashMap<String, Vec<String>>,
        warnings: &mut Vec<ValidationWarning>,
        confidence: &mut f64,
    ) {
        subtask.dependencies.retain(|dep| {
            if !valid_ids.contains(dep) {
                return false;
            }
            // Direct cycle check: if dep depends on us
            if let Some(dep_deps) = graph.get(dep) {
                if dep_deps.iter().any(|d| d == &subtask.temp_id) {
                    warnings.push(ValidationWarning {
                        kind: ValidationWarningKind::CircularDependency,
                        message: format!(
                            "Circular dependency between {} and {dep}",
                            subtask.temp_id
                        ),
                        subtask_temp_id: Some(subtask.temp_id.clone()),
                    });
                    *confidence -= 0.15;
                    return false;
                }
            }
            true
        });
        for child in &mut subtask.children {
            repair_deps(child, valid_ids, graph, warnings, confidence);
        }
    }
    for s in &mut tree.subtasks {
        repair_deps(s, &valid_ids, &graph, warnings, confidence);
    }

    // Check excessive count
    if total_count > ctx.max_subtasks_per_level as usize * 2 {
        warnings.push(ValidationWarning {
            kind: ValidationWarningKind::TooManySubtasks,
            message: format!(
                "Too many subtasks: {} (limit ~{})",
                total_count, ctx.max_subtasks_per_level
            ),
            subtask_temp_id: None,
        });
        *confidence -= 0.10;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_subtask() -> PlannedSubtask {
        PlannedSubtask {
            temp_id: String::new(),
            title: String::new(),
            description: None,
            acceptance_criteria: None,
            estimated_minutes: None,
            energy_level: None,
            priority: None,
            task_type: None,
            dependencies: vec![],
            children: vec![],
        }
    }

    #[test]
    fn test_build_prompt_includes_task_title() {
        let mut task = Task::default_instance();
        task.title = "Build the widget".to_string();
        let ctx = DecompositionContext {
            max_depth: 2,
            max_subtasks_per_level: 10,
            existing_subtasks: vec![],
            project_context: None,
            cognitive_facts: vec![],
            user_energy_profile: None,
            calendar_context: vec![],
        };
        let prompt = build_prompt(&task, &ctx);
        assert!(prompt.contains("Build the widget"));
    }

    #[test]
    fn test_validate_detects_circular_deps() {
        let mut tree = DecompositionTree {
            subtasks: vec![
                PlannedSubtask {
                    temp_id: "sub-1".to_string(),
                    title: "A".to_string(),
                    dependencies: vec!["sub-2".to_string()],
                    ..default_subtask()
                },
                PlannedSubtask {
                    temp_id: "sub-2".to_string(),
                    title: "B".to_string(),
                    dependencies: vec!["sub-1".to_string()],
                    ..default_subtask()
                },
            ],
            total_estimated_mins: None,
        };
        let ctx = DecompositionContext {
            max_depth: 2,
            max_subtasks_per_level: 10,
            existing_subtasks: vec![],
            project_context: None,
            cognitive_facts: vec![],
            user_energy_profile: None,
            calendar_context: vec![],
        };
        let mut warnings = vec![];
        let mut confidence = 0.9;
        validate_and_repair_tree(&mut tree, &ctx, &mut warnings, &mut confidence);

        assert!(!warnings.is_empty());
        assert!(confidence < 0.9);
        assert!(warnings
            .iter()
            .any(|w| matches!(w.kind, ValidationWarningKind::CircularDependency)));
        // Verify the circular dep was actually removed from the tree
        assert!(
            tree.subtasks[0].dependencies.is_empty() || tree.subtasks[1].dependencies.is_empty()
        );
    }

    #[test]
    fn test_convert_tree_from_llm_response() {
        let llm = LlmDecompositionResponse {
            confidence: 0.85,
            reasoning: "test".to_string(),
            subtasks: vec![LlmPlannedSubtask {
                temp_id: "sub-1".to_string(),
                title: "First subtask".to_string(),
                description: None,
                acceptance_criteria: None,
                estimated_minutes: Some(30),
                energy_level: Some("medium".to_string()),
                priority: Some(2),
                task_type: Some("manual".to_string()),
                dependencies: vec![],
                children: vec![],
            }],
            total_estimated_mins: Some(30),
        };
        let tree = convert_tree(&llm);
        assert_eq!(tree.subtasks.len(), 1);
        assert_eq!(tree.subtasks[0].title, "First subtask");
        assert_eq!(tree.subtasks[0].estimated_minutes, Some(30));
    }

    #[test]
    fn test_validate_detects_too_many_subtasks() {
        let mut tree = DecompositionTree {
            subtasks: (0..25)
                .map(|i| PlannedSubtask {
                    temp_id: format!("sub-{i}"),
                    title: format!("Task {i}"),
                    ..default_subtask()
                })
                .collect(),
            total_estimated_mins: None,
        };
        let ctx = DecompositionContext {
            max_depth: 2,
            max_subtasks_per_level: 10,
            existing_subtasks: vec![],
            project_context: None,
            cognitive_facts: vec![],
            user_energy_profile: None,
            calendar_context: vec![],
        };
        let mut warnings = vec![];
        let mut confidence = 0.9;
        validate_and_repair_tree(&mut tree, &ctx, &mut warnings, &mut confidence);

        assert!(warnings
            .iter()
            .any(|w| matches!(w.kind, ValidationWarningKind::TooManySubtasks)));
    }
}
