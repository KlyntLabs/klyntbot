use std::collections::HashMap;

use context_engine::book_index::types::{SourceType, TreeNode, TreeNodeType};
use storage::TaskRow;
use uuid::Uuid;

/// Build tree nodes from a project's task hierarchy.
///
/// Creates: Project Section (level 1) → Task nodes (level 2) → Subtask nodes (level 3).
/// Task descriptions become Text children.
pub fn build_task_tree(project_id: &str, project_name: &str, tasks: &[TaskRow]) -> Vec<TreeNode> {
    let mut nodes = Vec::new();

    let project_node_id = Uuid::new_v4().to_string();
    nodes.push(TreeNode {
        id: project_node_id.clone(),
        parent_id: None,
        node_type: TreeNodeType::Section,
        content: project_name.to_string(),
        title: Some(project_name.to_string()),
        level: 1,
        source_type: SourceType::Task,
        source_id: project_id.to_string(),
        position: 0,
        metadata: None,
    });

    // Pre-build children lookup for O(1) child access instead of O(N) scans
    let mut children_map: HashMap<&str, Vec<&TaskRow>> = HashMap::new();
    let task_ids: std::collections::HashSet<&str> = tasks.iter().map(|t| t.id.as_str()).collect();

    for task in tasks {
        if let Some(ref pid) = task.parent_id {
            if task_ids.contains(pid.as_str()) {
                children_map.entry(pid.as_str()).or_default().push(task);
            }
        }
    }

    // Root tasks: no parent_id, or parent is outside this task set
    let root_tasks: Vec<&TaskRow> = tasks
        .iter()
        .filter(|t| {
            t.parent_id.is_none() || !task_ids.contains(t.parent_id.as_deref().unwrap_or(""))
        })
        .collect();

    let mut position: u32 = 1;
    for task in &root_tasks {
        add_task_node(
            &mut nodes,
            task,
            &project_node_id,
            2,
            &mut position,
            project_id,
            &children_map,
        );
    }

    nodes
}

fn add_task_node(
    nodes: &mut Vec<TreeNode>,
    task: &TaskRow,
    parent_node_id: &str,
    level: u32,
    position: &mut u32,
    project_id: &str,
    children_map: &HashMap<&str, Vec<&TaskRow>>,
) {
    let node_id = Uuid::new_v4().to_string();

    nodes.push(TreeNode {
        id: node_id.clone(),
        parent_id: Some(parent_node_id.to_string()),
        node_type: TreeNodeType::Task,
        content: task.title.clone(),
        title: Some(task.title.clone()),
        level,
        source_type: SourceType::Task,
        source_id: project_id.to_string(),
        position: *position,
        metadata: None,
    });
    *position += 1;

    if let Some(ref desc) = task.description {
        if !desc.is_empty() {
            nodes.push(TreeNode {
                id: Uuid::new_v4().to_string(),
                parent_id: Some(node_id.clone()),
                node_type: TreeNodeType::Text,
                content: desc.clone(),
                title: None,
                level: level + 1,
                source_type: SourceType::Task,
                source_id: project_id.to_string(),
                position: *position,
                metadata: None,
            });
            *position += 1;
        }
    }

    if let Some(children) = children_map.get(task.id.as_str()) {
        for child in children {
            add_task_node(
                nodes,
                child,
                &node_id,
                level + 1,
                position,
                project_id,
                children_map,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn build_project_tree() {
        let tasks = vec![
            mock_task(
                "t1",
                "Build API",
                None,
                Some("proj-1"),
                Some("Design the REST API"),
            ),
            mock_task(
                "t2",
                "Write tests",
                Some("t1"),
                Some("proj-1"),
                Some("Unit + integration tests"),
            ),
            mock_task("t3", "Deploy", Some("t1"), Some("proj-1"), None),
        ];
        let nodes = build_task_tree("proj-1", "Project Alpha", &tasks);
        // project section + t1 + t1.desc + t2 + t2.desc + t3 = 6
        assert!(
            nodes.len() >= 4,
            "Expected at least 4 nodes, got {}",
            nodes.len()
        );
        assert_eq!(nodes[0].node_type.as_str(), "Section");
        assert_eq!(nodes[0].title.as_deref(), Some("Project Alpha"));
        assert!(matches!(nodes[0].source_type, SourceType::Task));
    }

    #[test]
    fn empty_tasks_produces_only_root() {
        let nodes = build_task_tree("proj-1", "Empty Project", &[]);
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].node_type.as_str(), "Section");
    }

    fn mock_task(
        id: &str,
        title: &str,
        parent_id: Option<&str>,
        project_id: Option<&str>,
        desc: Option<&str>,
    ) -> TaskRow {
        TaskRow {
            id: id.to_string(),
            title: title.to_string(),
            description: desc.map(|d| d.to_string()),
            area_id: "area-1".to_string(),
            project_id: project_id.map(|p| p.to_string()),
            key_result_id: None,
            parent_id: parent_id.map(|p| p.to_string()),
            priority: None,
            due_date: None,
            tags: vec![],
            status: "todo".to_string(),
            focused_at: None,
            focus_deadline: None,
            focus_expired_count: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            completed_at: None,
            total_tracked_secs: 0,
            estimated_minutes: None,
            calendar_event_uid: None,
            last_reminded_at: None,
            recurrence_rule: None,
            recurrence_parent_id: None,
            is_template: false,
            next_instance_date: None,
            status_label_id: None,
            position: 0,
            group_id: None,
            task_type: "standard".to_string(),
            acceptance_criteria: None,
            agent_config: None,
            execution_state: "idle".to_string(),
            spawned_execution_id: None,
            context_snapshot: None,
            energy_level: None,
            estimated_focus_blocks: None,
            actual_minutes: None,
            complexity_score: None,
            completed: false,
            objective_id: None,
        }
    }
}
