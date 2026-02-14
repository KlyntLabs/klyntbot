//! Integration tests for todo-project workflows
//!
//! Tests the full lifecycle of todos and projects working together:
//! - Creating projects and assigning tasks
//! - Moving tasks between projects
//! - Parent-child hierarchies
//! - Time tracking across project tasks
//! - Cascade completion with projects

use chrono::Utc;
use tempfile::TempDir;
use tools::{
    project_store::ProjectStore,
    project_types::{Project, ProjectColor, ProjectFilter, ProjectStatus},
    todo_store::TodoStore,
    todo_types::{TimeEntry, TimeEntrySource, Todo, TodoFilter, TodoStatus},
};

fn create_test_project(name: &str) -> Project {
    Project {
        id: Project::generate_id(),
        name: name.to_string(),
        description: Some(format!("{} project", name)),
        color: ProjectColor::Blue,
        tags: vec!["test".to_string()],
        status: ProjectStatus::Active,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

fn create_test_todo(title: &str) -> Todo {
    Todo {
        id: Todo::generate_id(),
        title: title.to_string(),
        description: None,
        priority: None,
        due_date: None,
        tags: vec![],
        status: TodoStatus::Todo,
        focused_at: None,
        focus_deadline: None,
        focus_expired_count: 0,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        completed_at: None,
        parent_id: None,
        project_id: None,
        attachments: Vec::new(),
        time_entries: Vec::new(),
        total_tracked_secs: 0,
        estimated_minutes: None,
        calendar_event_uid: None,
        last_reminded_at: None,
    }
}

#[tokio::test]
async fn test_create_project_and_assign_tasks() {
    // End-to-end: Create project, add tasks, verify assignment
    let temp_dir = TempDir::new().unwrap();
    let proj_path = temp_dir.path().join("projects.jsonl");
    let todo_path = temp_dir.path().join("todos.jsonl");

    let mut proj_store = ProjectStore::new(proj_path);
    let mut todo_store = TodoStore::new(todo_path);

    // Create project
    let project = create_test_project("Website Redesign");
    let project = proj_store.add(project).await.unwrap();
    let proj_id = project.id.clone();

    // Create tasks in this project
    let mut task1 = create_test_todo("Design mockups");
    task1.project_id = Some(proj_id.clone());
    let task1 = todo_store.add(task1).await.unwrap();

    let mut task2 = create_test_todo("Implement frontend");
    task2.project_id = Some(proj_id.clone());
    let task2 = todo_store.add(task2).await.unwrap();

    // Verify tasks are assigned to project
    let filter = TodoFilter {
        project_id: Some(proj_id.clone()),
        ..Default::default()
    };
    let project_tasks = todo_store.list(&filter).await.unwrap();

    assert_eq!(project_tasks.len(), 2);
    assert!(project_tasks.iter().any(|t| t.id == task1.id));
    assert!(project_tasks.iter().any(|t| t.id == task2.id));
    assert!(project_tasks
        .iter()
        .all(|t| t.project_id.as_ref() == Some(&proj_id)));
}

#[tokio::test]
async fn test_move_task_between_projects() {
    // Test moving a task from one project to another
    let temp_dir = TempDir::new().unwrap();
    let proj_path = temp_dir.path().join("projects.jsonl");
    let todo_path = temp_dir.path().join("todos.jsonl");

    let mut proj_store = ProjectStore::new(proj_path);
    let mut todo_store = TodoStore::new(todo_path);

    // Create two projects
    let proj_a = proj_store
        .add(create_test_project("Project A"))
        .await
        .unwrap();
    let proj_b = proj_store
        .add(create_test_project("Project B"))
        .await
        .unwrap();

    // Create task in Project A
    let mut task = create_test_todo("Movable task");
    task.project_id = Some(proj_a.id.clone());
    let task = todo_store.add(task).await.unwrap();
    let task_id = task.id.clone();

    // Move to Project B
    let moved = todo_store
        .move_todo(&task_id, None, Some(proj_b.id.clone()))
        .await
        .unwrap()
        .unwrap();

    assert_eq!(moved.project_id, Some(proj_b.id.clone()));

    // Verify it's now in Project B's list
    let filter_b = TodoFilter {
        project_id: Some(proj_b.id.clone()),
        ..Default::default()
    };
    let proj_b_tasks = todo_store.list(&filter_b).await.unwrap();
    assert_eq!(proj_b_tasks.len(), 1);
    assert_eq!(proj_b_tasks[0].id, task_id);

    // Verify it's no longer in Project A's list
    let filter_a = TodoFilter {
        project_id: Some(proj_a.id.clone()),
        ..Default::default()
    };
    let proj_a_tasks = todo_store.list(&filter_a).await.unwrap();
    assert_eq!(proj_a_tasks.len(), 0);
}

#[tokio::test]
async fn test_parent_child_hierarchy_within_project() {
    // Test creating subtask hierarchies within a project
    let temp_dir = TempDir::new().unwrap();
    let todo_path = temp_dir.path().join("todos.jsonl");
    let mut todo_store = TodoStore::new(todo_path);

    // Create parent task
    let parent = create_test_todo("Epic: User Authentication");
    let parent = todo_store.add(parent).await.unwrap();
    let parent_id = parent.id.clone();

    // Create subtasks
    let mut child1 = create_test_todo("Implement login form");
    child1.parent_id = Some(parent_id.clone());
    let child1 = todo_store.add(child1).await.unwrap();

    let mut child2 = create_test_todo("Add password reset");
    child2.parent_id = Some(parent_id.clone());
    let child2 = todo_store.add(child2).await.unwrap();

    // Verify children are found
    let filter = TodoFilter {
        parent_id: Some(parent_id.clone()),
        ..Default::default()
    };
    let children = todo_store.list(&filter).await.unwrap();

    assert_eq!(children.len(), 2);
    assert!(children
        .iter()
        .all(|c| c.parent_id.as_ref() == Some(&parent_id)));
    assert!(children.iter().any(|c| c.id == child1.id));
    assert!(children.iter().any(|c| c.id == child2.id));
}

#[tokio::test]
async fn test_cascade_complete_affects_all_subtasks() {
    // End-to-end: Complete parent, verify all children cascade to Done
    let temp_dir = TempDir::new().unwrap();
    let todo_path = temp_dir.path().join("todos.jsonl");
    let mut todo_store = TodoStore::new(todo_path);

    // Create parent and 3 children
    let parent = create_test_todo("Parent task");
    let parent = todo_store.add(parent).await.unwrap();
    let parent_id = parent.id.clone();

    let mut child1 = create_test_todo("Child 1");
    child1.parent_id = Some(parent_id.clone());
    let child1 = todo_store.add(child1).await.unwrap();

    let mut child2 = create_test_todo("Child 2");
    child2.parent_id = Some(parent_id.clone());
    let child2 = todo_store.add(child2).await.unwrap();

    let mut child3 = create_test_todo("Child 3");
    child3.parent_id = Some(parent_id.clone());
    let child3 = todo_store.add(child3).await.unwrap();

    // Cascade complete
    let completed_ids = todo_store.cascade_complete(&parent_id).await.unwrap();

    assert_eq!(completed_ids.len(), 3);
    assert!(completed_ids.contains(&child1.id));
    assert!(completed_ids.contains(&child2.id));
    assert!(completed_ids.contains(&child3.id));

    // Verify all children are now Done
    let child1_updated = todo_store.get(&child1.id).await.unwrap().unwrap();
    let child2_updated = todo_store.get(&child2.id).await.unwrap().unwrap();
    let child3_updated = todo_store.get(&child3.id).await.unwrap().unwrap();

    assert_eq!(child1_updated.status, TodoStatus::Done);
    assert_eq!(child2_updated.status, TodoStatus::Done);
    assert_eq!(child3_updated.status, TodoStatus::Done);
    assert!(child1_updated.completed_at.is_some());
    assert!(child2_updated.completed_at.is_some());
    assert!(child3_updated.completed_at.is_some());
}

#[tokio::test]
async fn test_cascade_complete_skips_already_done_subtasks() {
    // Edge case: cascading when some children are already Done
    let temp_dir = TempDir::new().unwrap();
    let todo_path = temp_dir.path().join("todos.jsonl");
    let mut todo_store = TodoStore::new(todo_path);

    let parent = create_test_todo("Parent");
    let parent = todo_store.add(parent).await.unwrap();
    let parent_id = parent.id.clone();

    let mut child1 = create_test_todo("Already done child");
    child1.parent_id = Some(parent_id.clone());
    child1.status = TodoStatus::Done;
    child1.completed_at = Some(Utc::now());
    let child1 = todo_store.add(child1).await.unwrap();

    let mut child2 = create_test_todo("Active child");
    child2.parent_id = Some(parent_id.clone());
    let child2 = todo_store.add(child2).await.unwrap();

    // Cascade complete
    let completed_ids = todo_store.cascade_complete(&parent_id).await.unwrap();

    // Should only complete child2 (child1 already done)
    assert_eq!(completed_ids.len(), 1);
    assert!(completed_ids.contains(&child2.id));
    assert!(!completed_ids.contains(&child1.id));
}

#[tokio::test]
async fn test_time_tracking_across_project_tasks() {
    // End-to-end: Track time on multiple tasks in a project
    let temp_dir = TempDir::new().unwrap();
    let todo_path = temp_dir.path().join("todos.jsonl");
    let mut todo_store = TodoStore::new(todo_path);

    // Create tasks
    let task1 = create_test_todo("Task 1");
    let task1 = todo_store.add(task1).await.unwrap();
    let task1_id = task1.id.clone();

    let task2 = create_test_todo("Task 2");
    let task2 = todo_store.add(task2).await.unwrap();
    let task2_id = task2.id.clone();

    // Add time entries
    let now = Utc::now();
    let entry1 = TimeEntry {
        id: "entry1".to_string(),
        started_at: now - chrono::Duration::hours(2),
        ended_at: Some(now - chrono::Duration::hours(1)),
        duration_secs: Some(3600), // 1 hour
        note: Some("Work on task 1".to_string()),
        source: TimeEntrySource::Focus,
    };

    let entry2 = TimeEntry {
        id: "entry2".to_string(),
        started_at: now - chrono::Duration::hours(1),
        ended_at: Some(now),
        duration_secs: Some(3600), // 1 hour
        note: Some("Work on task 2".to_string()),
        source: TimeEntrySource::Focus,
    };

    todo_store.add_time_entry(&task1_id, entry1).await.unwrap();
    todo_store.add_time_entry(&task2_id, entry2).await.unwrap();

    // Verify time entries are added
    let task1_updated = todo_store.get(&task1_id).await.unwrap().unwrap();
    let task2_updated = todo_store.get(&task2_id).await.unwrap().unwrap();

    assert_eq!(task1_updated.time_entries.len(), 1);
    assert_eq!(task2_updated.time_entries.len(), 1);
    assert_eq!(task1_updated.time_entries[0].id, "entry1");
    assert_eq!(task2_updated.time_entries[0].id, "entry2");
    assert_eq!(task1_updated.time_entries[0].duration_secs, Some(3600));
    assert_eq!(task2_updated.time_entries[0].duration_secs, Some(3600));

    // Note: total_tracked_secs is only updated by close_time_entry, not add_time_entry
    // This is the current implementation behavior
}

#[tokio::test]
async fn test_empty_project_list() {
    // Edge case: listing from empty project store
    let temp_dir = TempDir::new().unwrap();
    let proj_path = temp_dir.path().join("projects.jsonl");
    let mut proj_store = ProjectStore::new(proj_path);

    let projects = proj_store.list(&ProjectFilter::default()).await.unwrap();
    assert_eq!(projects.len(), 0);
}

#[tokio::test]
async fn test_filter_todos_by_nonexistent_project() {
    // Edge case: filtering by project that doesn't exist
    let temp_dir = TempDir::new().unwrap();
    let todo_path = temp_dir.path().join("todos.jsonl");
    let mut todo_store = TodoStore::new(todo_path);

    // Add some tasks without project
    todo_store.add(create_test_todo("Task 1")).await.unwrap();
    todo_store.add(create_test_todo("Task 2")).await.unwrap();

    // Filter by nonexistent project
    let filter = TodoFilter {
        project_id: Some("nonexistent-proj-id".to_string()),
        ..Default::default()
    };

    let results = todo_store.list(&filter).await.unwrap();
    assert_eq!(results.len(), 0);
}

#[tokio::test]
async fn test_circular_parent_prevention() {
    // Error condition: trying to set a task as its own parent
    let temp_dir = TempDir::new().unwrap();
    let todo_path = temp_dir.path().join("todos.jsonl");
    let mut todo_store = TodoStore::new(todo_path);

    let task = create_test_todo("Self-referential task");
    let task = todo_store.add(task).await.unwrap();
    let task_id = task.id.clone();

    // Try to set task as its own parent
    let result = todo_store
        .move_todo(&task_id, Some(task_id.clone()), None)
        .await;

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("own parent"));
}

#[tokio::test]
async fn test_delete_project_with_tasks() {
    // Integration: delete project when tasks reference it
    let temp_dir = TempDir::new().unwrap();
    let proj_path = temp_dir.path().join("projects.jsonl");
    let todo_path = temp_dir.path().join("todos.jsonl");

    let mut proj_store = ProjectStore::new(proj_path);
    let mut todo_store = TodoStore::new(todo_path);

    // Create project with tasks
    let project = proj_store
        .add(create_test_project("Deletable"))
        .await
        .unwrap();
    let proj_id = project.id.clone();

    let mut task = create_test_todo("Task in deletable project");
    task.project_id = Some(proj_id.clone());
    let task = todo_store.add(task).await.unwrap();

    // Delete the project
    let deleted = proj_store.delete(&proj_id).await.unwrap();
    assert!(deleted);

    // Project should be gone
    let retrieved_proj = proj_store.get(&proj_id).await.unwrap();
    assert!(retrieved_proj.is_none());

    // Task still exists (orphaned reference)
    let retrieved_task = todo_store.get(&task.id).await.unwrap();
    assert!(retrieved_task.is_some());
    assert_eq!(retrieved_task.unwrap().project_id, Some(proj_id));
}

#[tokio::test]
async fn test_persistence_across_store_instances() {
    // Integration: verify data persists when creating new store instances
    let temp_dir = TempDir::new().unwrap();
    let proj_path = temp_dir.path().join("projects.jsonl");
    let todo_path = temp_dir.path().join("todos.jsonl");

    let proj_id;
    let task_id;

    // First instance: create data
    {
        let mut proj_store = ProjectStore::new(proj_path.clone());
        let mut todo_store = TodoStore::new(todo_path.clone());

        let project = proj_store
            .add(create_test_project("Persistent"))
            .await
            .unwrap();
        proj_id = project.id.clone();

        let mut task = create_test_todo("Persistent task");
        task.project_id = Some(proj_id.clone());
        let task = todo_store.add(task).await.unwrap();
        task_id = task.id.clone();
    }

    // Second instance: verify data exists
    {
        let mut proj_store = ProjectStore::new(proj_path);
        let mut todo_store = TodoStore::new(todo_path);

        let loaded_proj = proj_store.get(&proj_id).await.unwrap();
        assert!(loaded_proj.is_some());
        assert_eq!(loaded_proj.unwrap().name, "Persistent");

        let loaded_task = todo_store.get(&task_id).await.unwrap();
        assert!(loaded_task.is_some());
        assert_eq!(loaded_task.unwrap().project_id, Some(proj_id));
    }
}
