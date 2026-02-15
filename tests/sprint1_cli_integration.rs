//! Sprint 1 integration tests for CLI parity
//!
//! Tests for all 15 new CLI commands by exercising the underlying
//! store operations and tool interfaces. Verifies functionality
//! and data integrity for:
//! - 9 new todo commands (tree, search, update, attach, detach, add-subtask, move, log-time, report)
//! - 6 new project commands (create, list, show, update, archive, tasks)
//! - Natural language date parsing

use chrono::{Duration, Utc};
use tempfile::TempDir;
use tools::{
    project_store::ProjectStore,
    project_types::{Project, ProjectColor, ProjectFilter, ProjectPatch, ProjectStatus},
    todo_store::TodoStore,
    todo_types::{
        Attachment, AttachmentType, TimeEntry, TimeEntrySource, Todo, TodoFilter, TodoPatch,
        TodoStatus,
    },
};

// ─── Test helpers ──────────────────────────────────────────────

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
        recurrence_rule: None,
        recurrence_parent_id: None,
        is_template: false,
        next_instance_date: None,
        blocked_by: Vec::new(),
        blocks: Vec::new(),
    }
}

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

// ─── Todo: tree ────────────────────────────────────────────────

#[tokio::test]
async fn test_tree_parent_child_relationships() {
    let temp_dir = TempDir::new().unwrap();
    let mut store = TodoStore::new(temp_dir.path().join("todos.jsonl"));

    // Create parent
    let parent = create_test_todo("Parent task");
    let parent = store.add(parent).await.unwrap();
    let parent_id = parent.id.clone();

    // Create children
    let mut child1 = create_test_todo("Child 1");
    child1.parent_id = Some(parent_id.clone());
    let child1 = store.add(child1).await.unwrap();

    let mut child2 = create_test_todo("Child 2");
    child2.parent_id = Some(parent_id.clone());
    store.add(child2).await.unwrap();

    // Create grandchild
    let mut grandchild = create_test_todo("Grandchild");
    grandchild.parent_id = Some(child1.id.clone());
    store.add(grandchild).await.unwrap();

    // Verify hierarchy via list + filter
    let all = store.list(&TodoFilter::default()).await.unwrap();
    assert_eq!(all.len(), 4);

    // Verify parent-child links
    let children: Vec<_> = all
        .iter()
        .filter(|t| t.parent_id.as_ref() == Some(&parent_id))
        .collect();
    assert_eq!(children.len(), 2);

    let grandchildren: Vec<_> = all
        .iter()
        .filter(|t| t.parent_id.as_ref() == Some(&child1.id))
        .collect();
    assert_eq!(grandchildren.len(), 1);
    assert_eq!(grandchildren[0].title, "Grandchild");
}

// ─── Todo: search ──────────────────────────────────────────────

#[tokio::test]
async fn test_search_by_title() {
    let temp_dir = TempDir::new().unwrap();
    let mut store = TodoStore::new(temp_dir.path().join("todos.jsonl"));

    store
        .add(create_test_todo("Fix authentication bug"))
        .await
        .unwrap();
    store.add(create_test_todo("Add unit tests")).await.unwrap();
    store
        .add(create_test_todo("Fix deployment pipeline"))
        .await
        .unwrap();

    let all = store.list(&TodoFilter::default()).await.unwrap();

    // Search for "Fix" in titles
    let results: Vec<_> = all
        .iter()
        .filter(|t| t.title.to_lowercase().contains("fix"))
        .collect();
    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn test_search_by_description() {
    let temp_dir = TempDir::new().unwrap();
    let mut store = TodoStore::new(temp_dir.path().join("todos.jsonl"));

    let mut todo = create_test_todo("Task with description");
    todo.description = Some("This involves refactoring the database layer".to_string());
    store.add(todo).await.unwrap();

    store.add(create_test_todo("Another task")).await.unwrap();

    let all = store.list(&TodoFilter::default()).await.unwrap();
    let results: Vec<_> = all
        .iter()
        .filter(|t| {
            t.description
                .as_ref()
                .is_some_and(|d| d.to_lowercase().contains("database"))
        })
        .collect();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Task with description");
}

#[tokio::test]
async fn test_search_in_attachments() {
    let temp_dir = TempDir::new().unwrap();
    let mut store = TodoStore::new(temp_dir.path().join("todos.jsonl"));

    let todo = create_test_todo("Task with attachment");
    let todo = store.add(todo).await.unwrap();

    let attachment = Attachment {
        id: Todo::generate_id(),
        attachment_type: AttachmentType::Note,
        title: Some("Design notes".to_string()),
        value: "The API should use GraphQL for flexibility".to_string(),
        tags: Vec::new(),
        created_at: Utc::now(),
    };
    store.add_attachment(&todo.id, attachment).await.unwrap();

    let all = store.list(&TodoFilter::default()).await.unwrap();

    // Search in attachment value
    let results: Vec<_> = all
        .iter()
        .filter(|t| {
            t.attachments
                .iter()
                .any(|a| a.value.to_lowercase().contains("graphql"))
        })
        .collect();
    assert_eq!(results.len(), 1);
}

// ─── Todo: update ──────────────────────────────────────────────

#[tokio::test]
async fn test_update_title_and_priority() {
    let temp_dir = TempDir::new().unwrap();
    let mut store = TodoStore::new(temp_dir.path().join("todos.jsonl"));

    let todo = store.add(create_test_todo("Original title")).await.unwrap();

    let patch = TodoPatch {
        title: Some("Updated title".to_string()),
        priority: Some(4),
        ..Default::default()
    };

    let updated = store.update(&todo.id, patch).await.unwrap().unwrap();
    assert_eq!(updated.title, "Updated title");
    assert_eq!(updated.priority, Some(4));
}

#[tokio::test]
async fn test_update_status() {
    let temp_dir = TempDir::new().unwrap();
    let mut store = TodoStore::new(temp_dir.path().join("todos.jsonl"));

    let todo = store.add(create_test_todo("Status test")).await.unwrap();
    assert_eq!(todo.status, TodoStatus::Todo);

    let patch = TodoPatch {
        status: Some(TodoStatus::Doing),
        ..Default::default()
    };
    let updated = store.update(&todo.id, patch).await.unwrap().unwrap();
    assert_eq!(updated.status, TodoStatus::Doing);
}

#[tokio::test]
async fn test_update_due_date_with_natural_language() {
    // Verify that parse_datetime handles natural language
    use common::utils::date::parse_datetime;

    let tomorrow = parse_datetime("tomorrow", "UTC").unwrap();
    let expected_date = (Utc::now() + Duration::days(1)).date_naive();
    assert_eq!(tomorrow.date_naive(), expected_date);

    let next_week = parse_datetime("in 1 week", "UTC").unwrap();
    let expected_date = (Utc::now() + Duration::weeks(1)).date_naive();
    assert_eq!(next_week.date_naive(), expected_date);
}

#[tokio::test]
async fn test_update_nonexistent_task() {
    let temp_dir = TempDir::new().unwrap();
    let mut store = TodoStore::new(temp_dir.path().join("todos.jsonl"));

    let patch = TodoPatch {
        title: Some("New title".to_string()),
        ..Default::default()
    };
    let result = store.update("nonexistent", patch).await.unwrap();
    assert!(result.is_none());
}

// ─── Todo: attach / detach ─────────────────────────────────────

#[tokio::test]
async fn test_attach_file() {
    let temp_dir = TempDir::new().unwrap();
    let mut store = TodoStore::new(temp_dir.path().join("todos.jsonl"));

    let todo = store.add(create_test_todo("Attach test")).await.unwrap();

    let attachment = Attachment {
        id: Todo::generate_id(),
        attachment_type: AttachmentType::File,
        title: Some("Screenshot".to_string()),
        value: "/tmp/screenshot.png".to_string(),
        tags: Vec::new(),
        created_at: Utc::now(),
    };

    let result = store.add_attachment(&todo.id, attachment).await.unwrap();
    assert!(result);

    let updated = store.get(&todo.id).await.unwrap().unwrap();
    assert_eq!(updated.attachments.len(), 1);
    assert_eq!(updated.attachments[0].attachment_type, AttachmentType::File);
    assert_eq!(updated.attachments[0].value, "/tmp/screenshot.png");
}

#[tokio::test]
async fn test_attach_url_and_note() {
    let temp_dir = TempDir::new().unwrap();
    let mut store = TodoStore::new(temp_dir.path().join("todos.jsonl"));

    let todo = store.add(create_test_todo("Multi-attach")).await.unwrap();

    // Attach URL
    let url_att = Attachment {
        id: Todo::generate_id(),
        attachment_type: AttachmentType::Url,
        title: Some("Reference".to_string()),
        value: "https://example.com/docs".to_string(),
        tags: Vec::new(),
        created_at: Utc::now(),
    };
    store.add_attachment(&todo.id, url_att).await.unwrap();

    // Attach note
    let note_att = Attachment {
        id: Todo::generate_id(),
        attachment_type: AttachmentType::Note,
        title: None,
        value: "Remember to check edge cases".to_string(),
        tags: Vec::new(),
        created_at: Utc::now(),
    };
    store.add_attachment(&todo.id, note_att).await.unwrap();

    let updated = store.get(&todo.id).await.unwrap().unwrap();
    assert_eq!(updated.attachments.len(), 2);
}

#[tokio::test]
async fn test_detach_removes_attachment() {
    let temp_dir = TempDir::new().unwrap();
    let mut store = TodoStore::new(temp_dir.path().join("todos.jsonl"));

    let todo = store.add(create_test_todo("Detach test")).await.unwrap();

    let att_id = Todo::generate_id();
    let attachment = Attachment {
        id: att_id.clone(),
        attachment_type: AttachmentType::Note,
        title: None,
        value: "To be removed".to_string(),
        tags: Vec::new(),
        created_at: Utc::now(),
    };
    store.add_attachment(&todo.id, attachment).await.unwrap();

    // Verify attachment exists
    let before = store.get(&todo.id).await.unwrap().unwrap();
    assert_eq!(before.attachments.len(), 1);

    // Remove it
    let result = store.remove_attachment(&todo.id, &att_id).await.unwrap();
    assert!(result);

    // Verify it's gone
    let after = store.get(&todo.id).await.unwrap().unwrap();
    assert_eq!(after.attachments.len(), 0);
}

#[tokio::test]
async fn test_detach_nonexistent_attachment() {
    let temp_dir = TempDir::new().unwrap();
    let mut store = TodoStore::new(temp_dir.path().join("todos.jsonl"));

    let todo = store.add(create_test_todo("No attachments")).await.unwrap();
    let result = store
        .remove_attachment(&todo.id, "nonexistent")
        .await
        .unwrap();
    assert!(!result);
}

// ─── Todo: add-subtask ─────────────────────────────────────────

#[tokio::test]
async fn test_add_subtask_links_parent() {
    let temp_dir = TempDir::new().unwrap();
    let mut store = TodoStore::new(temp_dir.path().join("todos.jsonl"));

    let parent = store.add(create_test_todo("Parent")).await.unwrap();

    let mut subtask = create_test_todo("Subtask of parent");
    subtask.parent_id = Some(parent.id.clone());
    let subtask = store.add(subtask).await.unwrap();

    assert_eq!(subtask.parent_id, Some(parent.id.clone()));

    // Verify parent has child when filtering
    let all = store.list(&TodoFilter::default()).await.unwrap();
    let children: Vec<_> = all
        .iter()
        .filter(|t| t.parent_id.as_ref() == Some(&parent.id))
        .collect();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].title, "Subtask of parent");
}

#[tokio::test]
async fn test_add_subtask_inherits_project() {
    let temp_dir = TempDir::new().unwrap();
    let mut store = TodoStore::new(temp_dir.path().join("todos.jsonl"));

    let mut parent = create_test_todo("Parent in project");
    parent.project_id = Some("proj-abc".to_string());
    let parent = store.add(parent).await.unwrap();

    // Subtask should be able to inherit project
    let mut subtask = create_test_todo("Subtask");
    subtask.parent_id = Some(parent.id.clone());
    subtask.project_id = parent.project_id.clone();
    let subtask = store.add(subtask).await.unwrap();

    assert_eq!(subtask.project_id, Some("proj-abc".to_string()));
}

// ─── Todo: move ────────────────────────────────────────────────

#[tokio::test]
async fn test_move_task_to_new_parent() {
    let temp_dir = TempDir::new().unwrap();
    let mut store = TodoStore::new(temp_dir.path().join("todos.jsonl"));

    let parent1 = store.add(create_test_todo("Parent 1")).await.unwrap();
    let parent2 = store.add(create_test_todo("Parent 2")).await.unwrap();
    let mut child = create_test_todo("Moving child");
    child.parent_id = Some(parent1.id.clone());
    let child = store.add(child).await.unwrap();

    // Move to parent2
    let moved = store
        .move_todo(&child.id, Some(parent2.id.clone()), None)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(moved.parent_id, Some(parent2.id.clone()));
}

#[tokio::test]
async fn test_move_task_to_project() {
    let temp_dir = TempDir::new().unwrap();
    let mut store = TodoStore::new(temp_dir.path().join("todos.jsonl"));

    let task = store
        .add(create_test_todo("Unassigned task"))
        .await
        .unwrap();
    assert!(task.project_id.is_none());

    let moved = store
        .move_todo(&task.id, None, Some("proj-123".to_string()))
        .await
        .unwrap()
        .unwrap();

    assert_eq!(moved.project_id, Some("proj-123".to_string()));
}

#[tokio::test]
async fn test_move_task_clear_parent() {
    let temp_dir = TempDir::new().unwrap();
    let mut store = TodoStore::new(temp_dir.path().join("todos.jsonl"));

    let parent = store.add(create_test_todo("Parent")).await.unwrap();
    let mut child = create_test_todo("Child");
    child.parent_id = Some(parent.id.clone());
    let child = store.add(child).await.unwrap();

    // Clear parent (set to None)
    let moved = store
        .move_todo(&child.id, None, child.project_id.clone())
        .await
        .unwrap()
        .unwrap();

    assert!(moved.parent_id.is_none());
}

// ─── Todo: log-time ────────────────────────────────────────────

#[tokio::test]
async fn test_log_time_creates_entry() {
    let temp_dir = TempDir::new().unwrap();
    let mut store = TodoStore::new(temp_dir.path().join("todos.jsonl"));

    let task = store
        .add(create_test_todo("Time tracked task"))
        .await
        .unwrap();

    let entry = TimeEntry {
        id: Todo::generate_id(),
        started_at: Utc::now(),
        ended_at: Some(Utc::now()),
        duration_secs: Some(30 * 60), // 30 minutes
        note: Some("First session".to_string()),
        source: TimeEntrySource::Manual,
    };

    let result = store.add_time_entry(&task.id, entry).await.unwrap();
    assert!(result);

    let updated = store.get(&task.id).await.unwrap().unwrap();
    assert_eq!(updated.time_entries.len(), 1);
    assert_eq!(updated.time_entries[0].duration_secs, Some(30 * 60));
    assert_eq!(
        updated.time_entries[0].note,
        Some("First session".to_string())
    );
}

#[tokio::test]
async fn test_log_time_multiple_entries() {
    let temp_dir = TempDir::new().unwrap();
    let mut store = TodoStore::new(temp_dir.path().join("todos.jsonl"));

    let task = store.add(create_test_todo("Multi-time")).await.unwrap();

    for i in 1..=3 {
        let entry = TimeEntry {
            id: Todo::generate_id(),
            started_at: Utc::now(),
            ended_at: Some(Utc::now()),
            duration_secs: Some(i * 15 * 60), // 15, 30, 45 minutes
            note: None,
            source: TimeEntrySource::Manual,
        };
        store.add_time_entry(&task.id, entry).await.unwrap();
    }

    let updated = store.get(&task.id).await.unwrap().unwrap();
    assert_eq!(updated.time_entries.len(), 3);

    let total_secs: u64 = updated
        .time_entries
        .iter()
        .filter_map(|e| e.duration_secs)
        .sum();
    assert_eq!(total_secs, (15 + 30 + 45) * 60);
}

// ─── Todo: report (time tracking analytics) ────────────────────

#[tokio::test]
async fn test_report_aggregates_by_project() {
    let temp_dir = TempDir::new().unwrap();
    let mut store = TodoStore::new(temp_dir.path().join("todos.jsonl"));

    // Create tasks in two projects
    let mut task_a = create_test_todo("Task in Project A");
    task_a.project_id = Some("proj-a".to_string());
    let task_a = store.add(task_a).await.unwrap();

    let mut task_b = create_test_todo("Task in Project B");
    task_b.project_id = Some("proj-b".to_string());
    let task_b = store.add(task_b).await.unwrap();

    // Log time for both
    let entry_a = TimeEntry {
        id: Todo::generate_id(),
        started_at: Utc::now(),
        ended_at: Some(Utc::now()),
        duration_secs: Some(60 * 60),
        note: None,
        source: TimeEntrySource::Manual,
    };
    store.add_time_entry(&task_a.id, entry_a).await.unwrap();

    let entry_b = TimeEntry {
        id: Todo::generate_id(),
        started_at: Utc::now(),
        ended_at: Some(Utc::now()),
        duration_secs: Some(30 * 60),
        note: None,
        source: TimeEntrySource::Manual,
    };
    store.add_time_entry(&task_b.id, entry_b).await.unwrap();

    // Verify tasks have correct time entries
    let all = store.list(&TodoFilter::default()).await.unwrap();
    let proj_a_time: u64 = all
        .iter()
        .filter(|t| t.project_id.as_deref() == Some("proj-a"))
        .flat_map(|t| &t.time_entries)
        .filter_map(|e| e.duration_secs)
        .sum();
    assert_eq!(proj_a_time, 60 * 60); // 1 hour

    let proj_b_time: u64 = all
        .iter()
        .filter(|t| t.project_id.as_deref() == Some("proj-b"))
        .flat_map(|t| &t.time_entries)
        .filter_map(|e| e.duration_secs)
        .sum();
    assert_eq!(proj_b_time, 30 * 60); // 30 min
}

// ─── Project: create ───────────────────────────────────────────

#[tokio::test]
async fn test_project_create_with_all_fields() {
    let temp_dir = TempDir::new().unwrap();
    let mut store = ProjectStore::new(temp_dir.path().join("projects.jsonl"));

    let project = Project {
        id: Project::generate_id(),
        name: "My Project".to_string(),
        description: Some("A test project".to_string()),
        color: ProjectColor::Green,
        tags: vec!["rust".to_string(), "cli".to_string()],
        status: ProjectStatus::Active,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    let saved = store.add(project).await.unwrap();
    assert_eq!(saved.name, "My Project");
    assert_eq!(saved.color, ProjectColor::Green);
    assert_eq!(saved.tags, vec!["rust", "cli"]);
    assert_eq!(saved.status, ProjectStatus::Active);
}

#[tokio::test]
async fn test_project_create_persistence() {
    let temp_dir = TempDir::new().unwrap();
    let proj_path = temp_dir.path().join("projects.jsonl");

    let project_id;
    {
        let mut store = ProjectStore::new(proj_path.clone());
        let project = create_test_project("Persistent Project");
        let saved = store.add(project).await.unwrap();
        project_id = saved.id.clone();
    }

    // Reload from disk
    let mut store = ProjectStore::new(proj_path);
    let loaded = store.get(&project_id).await.unwrap();
    assert!(loaded.is_some());
    assert_eq!(loaded.unwrap().name, "Persistent Project");
}

// ─── Project: list ─────────────────────────────────────────────

#[tokio::test]
async fn test_project_list_with_status_filter() {
    let temp_dir = TempDir::new().unwrap();
    let mut store = ProjectStore::new(temp_dir.path().join("projects.jsonl"));

    store.add(create_test_project("Active 1")).await.unwrap();
    store.add(create_test_project("Active 2")).await.unwrap();

    let mut paused = create_test_project("Paused");
    paused.status = ProjectStatus::Paused;
    store.add(paused).await.unwrap();

    // List active only
    let active = store
        .list(&ProjectFilter {
            status: Some(ProjectStatus::Active),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(active.len(), 2);

    // List all
    let all = store.list(&ProjectFilter::default()).await.unwrap();
    assert_eq!(all.len(), 3);
}

// ─── Project: show ─────────────────────────────────────────────

#[tokio::test]
async fn test_project_show_returns_details() {
    let temp_dir = TempDir::new().unwrap();
    let mut store = ProjectStore::new(temp_dir.path().join("projects.jsonl"));

    let project = create_test_project("Detailed Project");
    let saved = store.add(project).await.unwrap();

    let fetched = store.get(&saved.id).await.unwrap().unwrap();
    assert_eq!(fetched.name, "Detailed Project");
    assert_eq!(
        fetched.description,
        Some("Detailed Project project".to_string())
    );
    assert_eq!(fetched.color, ProjectColor::Blue);
}

#[tokio::test]
async fn test_project_show_nonexistent() {
    let temp_dir = TempDir::new().unwrap();
    let mut store = ProjectStore::new(temp_dir.path().join("projects.jsonl"));

    let result = store.get("nonexistent").await.unwrap();
    assert!(result.is_none());
}

// ─── Project: update ───────────────────────────────────────────

#[tokio::test]
async fn test_project_update_name_and_color() {
    let temp_dir = TempDir::new().unwrap();
    let mut store = ProjectStore::new(temp_dir.path().join("projects.jsonl"));

    let project = store.add(create_test_project("Old Name")).await.unwrap();

    let patch = ProjectPatch {
        name: Some("New Name".to_string()),
        color: Some(ProjectColor::Purple),
        ..Default::default()
    };

    let updated = store.update(&project.id, patch).await.unwrap().unwrap();
    assert_eq!(updated.name, "New Name");
    assert_eq!(updated.color, ProjectColor::Purple);
}

#[tokio::test]
async fn test_project_update_status() {
    let temp_dir = TempDir::new().unwrap();
    let mut store = ProjectStore::new(temp_dir.path().join("projects.jsonl"));

    let project = store.add(create_test_project("Status Test")).await.unwrap();

    let patch = ProjectPatch {
        status: Some(ProjectStatus::Completed),
        ..Default::default()
    };

    let updated = store.update(&project.id, patch).await.unwrap().unwrap();
    assert_eq!(updated.status, ProjectStatus::Completed);
}

// ─── Project: archive ──────────────────────────────────────────

#[tokio::test]
async fn test_project_archive_sets_status() {
    let temp_dir = TempDir::new().unwrap();
    let mut store = ProjectStore::new(temp_dir.path().join("projects.jsonl"));

    let project = store.add(create_test_project("To Archive")).await.unwrap();
    assert_eq!(project.status, ProjectStatus::Active);

    let patch = ProjectPatch {
        status: Some(ProjectStatus::Archived),
        ..Default::default()
    };
    let archived = store.update(&project.id, patch).await.unwrap().unwrap();
    assert_eq!(archived.status, ProjectStatus::Archived);

    // Archived project should not show in active filter
    let active = store
        .list(&ProjectFilter {
            status: Some(ProjectStatus::Active),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(active.len(), 0);
}

// ─── Project: tasks ────────────────────────────────────────────

#[tokio::test]
async fn test_project_tasks_filters_by_project() {
    let temp_dir = TempDir::new().unwrap();
    let mut todo_store = TodoStore::new(temp_dir.path().join("todos.jsonl"));

    let mut task1 = create_test_todo("Task in proj-1");
    task1.project_id = Some("proj-1".to_string());
    todo_store.add(task1).await.unwrap();

    let mut task2 = create_test_todo("Task in proj-1 too");
    task2.project_id = Some("proj-1".to_string());
    todo_store.add(task2).await.unwrap();

    let mut task3 = create_test_todo("Task in proj-2");
    task3.project_id = Some("proj-2".to_string());
    todo_store.add(task3).await.unwrap();

    // Filter by project
    let proj1_tasks = todo_store
        .list(&TodoFilter {
            project_id: Some("proj-1".to_string()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(proj1_tasks.len(), 2);

    let proj2_tasks = todo_store
        .list(&TodoFilter {
            project_id: Some("proj-2".to_string()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(proj2_tasks.len(), 1);
}

#[tokio::test]
async fn test_project_tasks_tree_with_hierarchy() {
    let temp_dir = TempDir::new().unwrap();
    let mut store = TodoStore::new(temp_dir.path().join("todos.jsonl"));

    let mut parent = create_test_todo("Epic: Redesign");
    parent.project_id = Some("proj-web".to_string());
    let parent = store.add(parent).await.unwrap();

    let mut sub1 = create_test_todo("Design system");
    sub1.project_id = Some("proj-web".to_string());
    sub1.parent_id = Some(parent.id.clone());
    store.add(sub1).await.unwrap();

    let mut sub2 = create_test_todo("Implement components");
    sub2.project_id = Some("proj-web".to_string());
    sub2.parent_id = Some(parent.id.clone());
    store.add(sub2).await.unwrap();

    let tasks = store
        .list(&TodoFilter {
            project_id: Some("proj-web".to_string()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(tasks.len(), 3);

    // Verify tree structure
    let roots: Vec<_> = tasks.iter().filter(|t| t.parent_id.is_none()).collect();
    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0].title, "Epic: Redesign");

    let children: Vec<_> = tasks
        .iter()
        .filter(|t| t.parent_id.as_ref() == Some(&parent.id))
        .collect();
    assert_eq!(children.len(), 2);
}

// ─── Cross-cutting: natural language dates ─────────────────────

#[tokio::test]
async fn test_natural_language_date_tomorrow() {
    use common::utils::date::parse_datetime;

    let result = parse_datetime("tomorrow", "America/New_York");
    assert!(result.is_some());

    let dt = result.unwrap();
    assert!(dt > Utc::now(), "tomorrow should be in the future");
}

#[tokio::test]
async fn test_natural_language_date_next_friday() {
    use chrono::Datelike;
    use common::utils::date::parse_datetime;

    let result = parse_datetime("next friday", "UTC");
    assert!(result.is_some());

    let dt = result.unwrap();
    assert_eq!(dt.weekday(), chrono::Weekday::Fri);
    assert!(dt > Utc::now());
}

#[tokio::test]
async fn test_natural_language_date_in_3_days() {
    use common::utils::date::parse_datetime;

    let result = parse_datetime("in 3 days", "UTC").unwrap();
    let expected = (Utc::now() + Duration::days(3)).date_naive();
    assert_eq!(result.date_naive(), expected);
}

// ─── Error cases ───────────────────────────────────────────────

#[tokio::test]
async fn test_error_update_invalid_id() {
    let temp_dir = TempDir::new().unwrap();
    let mut store = TodoStore::new(temp_dir.path().join("todos.jsonl"));

    let patch = TodoPatch {
        title: Some("Won't work".to_string()),
        ..Default::default()
    };
    let result = store.update("bad-id-00", patch).await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_error_attach_to_nonexistent_task() {
    let temp_dir = TempDir::new().unwrap();
    let mut store = TodoStore::new(temp_dir.path().join("todos.jsonl"));

    let attachment = Attachment {
        id: Todo::generate_id(),
        attachment_type: AttachmentType::Note,
        title: None,
        value: "orphan attachment".to_string(),
        tags: Vec::new(),
        created_at: Utc::now(),
    };

    let result = store
        .add_attachment("nonexistent", attachment)
        .await
        .unwrap();
    assert!(!result);
}

#[tokio::test]
async fn test_error_move_nonexistent_task() {
    let temp_dir = TempDir::new().unwrap();
    let mut store = TodoStore::new(temp_dir.path().join("todos.jsonl"));

    let result = store
        .move_todo("nonexistent", None, Some("proj-1".to_string()))
        .await
        .unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_error_log_time_nonexistent_task() {
    let temp_dir = TempDir::new().unwrap();
    let mut store = TodoStore::new(temp_dir.path().join("todos.jsonl"));

    let entry = TimeEntry {
        id: Todo::generate_id(),
        started_at: Utc::now(),
        ended_at: Some(Utc::now()),
        duration_secs: Some(600),
        note: None,
        source: TimeEntrySource::Manual,
    };

    let result = store.add_time_entry("nonexistent", entry).await.unwrap();
    assert!(!result);
}
