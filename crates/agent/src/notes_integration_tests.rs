//! Integration tests for NotesTool registration and lifecycle in the agent crate.
//!
//! These tests verify that the notes feature is properly wired into the agent's
//! tool registry, migrations run correctly, and the full CRUD lifecycle works
//! through the same code path the agent runtime uses.

use common::{ChannelName, ChatId};
use feature_notes::{repo::NoteRepo, tool::NotesTool, NotesFeature};
use serde_json::json;
use tools_core::{RoutingContext, ToolRegistry};

fn ctx() -> RoutingContext {
    RoutingContext::new(ChannelName::from("test"), ChatId::from("user-1"))
}

/// Set up an in-memory SQLite pool with notes migrations applied,
/// mirroring what builder.rs does at startup.
async fn setup_with_registry() -> (ToolRegistry, sqlx::SqlitePool) {
    // Use StoragePool::connect_in_memory() to get core migrations (including _feature_migrations table)
    let storage_pool = storage::StoragePool::connect_in_memory().await.unwrap();
    let pool = storage_pool.inner().clone();

    // Run feature migrations (same as builder.rs)
    storage::StoragePool::run_feature_migrations(&pool, &NotesFeature::migrations_static())
        .await
        .unwrap();

    let repo = NoteRepo::new(pool.clone());
    let mut registry = ToolRegistry::new();
    registry.register(NotesTool::new(repo));

    (registry, pool)
}

/// Helper: execute a notes tool action through the registry
async fn exec(registry: &ToolRegistry, args: serde_json::Value) -> String {
    registry
        .execute("notes", args, &ctx())
        .await
        .expect("tool execution should succeed")
}

#[tokio::test]
async fn test_notes_tool_registered_and_discoverable() {
    let (registry, _pool) = setup_with_registry().await;

    assert!(registry.has("notes"), "notes tool should be registered");

    let defs = registry.get_definitions();
    let notes_def = defs
        .iter()
        .find(|d| d["function"]["name"].as_str() == Some("notes"));
    assert!(
        notes_def.is_some(),
        "notes should appear in tool definitions"
    );

    let def = notes_def.unwrap();
    let desc = def["function"]["description"].as_str().unwrap_or("");
    assert!(
        desc.contains("create_note"),
        "description should list actions"
    );
}

#[tokio::test]
async fn test_create_and_get_note() {
    let (registry, _pool) = setup_with_registry().await;

    let result = exec(
        &registry,
        json!({"action": "create_note", "title": "Meeting Notes", "body": "Discussed Q2 roadmap"}),
    )
    .await;
    assert!(result.contains("Created note"));
    assert!(result.contains("Meeting Notes"));

    // Extract the ID from "Created note \"Meeting Notes\" (id: <uuid>)"
    let id = result
        .split("id: ")
        .nth(1)
        .unwrap()
        .trim_end_matches(')')
        .to_string();

    let get_result = exec(&registry, json!({"action": "get_note", "id": id})).await;
    assert!(get_result.contains("Meeting Notes"));
    assert!(get_result.contains("Discussed Q2 roadmap"));
}

#[tokio::test]
async fn test_full_crud_lifecycle() {
    let (registry, _pool) = setup_with_registry().await;

    // Create
    let result = exec(
        &registry,
        json!({"action": "create_note", "title": "TODO", "body": "Buy groceries"}),
    )
    .await;
    let id = result
        .split("id: ")
        .nth(1)
        .unwrap()
        .trim_end_matches(')')
        .to_string();

    // Update
    let update_result = exec(
        &registry,
        json!({"action": "update_note", "id": id, "title": "Shopping List", "body": "Buy groceries and milk"}),
    )
    .await;
    assert!(update_result.contains("Updated note"));
    assert!(update_result.contains("Shopping List"));

    // List
    let list_result = exec(&registry, json!({"action": "list_notes"})).await;
    assert!(list_result.contains("Shopping List"));
    assert!(list_result.contains("1 note(s)"));

    // Delete
    let delete_result = exec(
        &registry,
        json!({"action": "delete_note", "id": id}),
    )
    .await;
    assert!(delete_result.contains("Deleted note"));

    // Verify deleted
    let list_after = exec(&registry, json!({"action": "list_notes"})).await;
    assert!(
        list_after.contains("No notes"),
        "should be empty after delete"
    );
}

#[tokio::test]
async fn test_notebook_and_note_association() {
    let (registry, _pool) = setup_with_registry().await;

    // Create notebook
    let nb_result = exec(
        &registry,
        json!({"action": "create_notebook", "title": "Work", "icon": "💼"}),
    )
    .await;
    assert!(nb_result.contains("Created notebook"));
    let nb_id = nb_result
        .split("id: ")
        .nth(1)
        .unwrap()
        .trim_end_matches(')')
        .to_string();

    // Create note in notebook
    let note_result = exec(
        &registry,
        json!({"action": "create_note", "title": "Sprint Planning", "body": "Sprint 42 goals", "notebook_id": nb_id}),
    )
    .await;
    assert!(note_result.contains("Created note"));

    // Create note outside notebook
    exec(
        &registry,
        json!({"action": "create_note", "title": "Random Thought", "body": "..."}),
    )
    .await;

    // List notebooks
    let list_nb = exec(&registry, json!({"action": "list_notebooks"})).await;
    assert!(list_nb.contains("Work"));
    assert!(list_nb.contains("1 notes"));

    // List notes filtered by notebook
    let filtered = exec(
        &registry,
        json!({"action": "list_notes", "notebook_id": nb_id}),
    )
    .await;
    assert!(filtered.contains("Sprint Planning"));
    assert!(!filtered.contains("Random Thought"));

    // List all notes (unfiltered)
    let all = exec(&registry, json!({"action": "list_notes"})).await;
    assert!(all.contains("2 note(s)"));
}

#[tokio::test]
async fn test_tagging_and_search() {
    let (registry, _pool) = setup_with_registry().await;

    // Create notes with tags
    let result = exec(
        &registry,
        json!({"action": "create_note", "title": "Rust Tips", "body": "Use pattern matching for enums", "tags": ["rust", "programming"]}),
    )
    .await;
    let rust_id = result
        .split("id: ")
        .nth(1)
        .unwrap()
        .trim_end_matches(')')
        .to_string();

    exec(
        &registry,
        json!({"action": "create_note", "title": "Go Tips", "body": "Use interfaces for polymorphism", "tags": ["go", "programming"]}),
    )
    .await;

    // Get note should show tags
    let get = exec(&registry, json!({"action": "get_note", "id": rust_id})).await;
    assert!(get.contains("rust"));
    assert!(get.contains("programming"));

    // Search by title
    let search = exec(
        &registry,
        json!({"action": "search_notes", "query": "Rust"}),
    )
    .await;
    assert!(search.contains("Rust Tips"));
    assert!(!search.contains("Go Tips"));

    // Search by body content
    let body_search = exec(
        &registry,
        json!({"action": "search_notes", "query": "pattern matching"}),
    )
    .await;
    assert!(body_search.contains("Rust Tips"));

    // Update tags
    let tag_result = exec(
        &registry,
        json!({"action": "tag_note", "id": rust_id, "tags": ["rust", "tips", "advanced"]}),
    )
    .await;
    assert!(tag_result.contains("3 tag(s)"));
}

#[tokio::test]
async fn test_note_linking() {
    let (registry, _pool) = setup_with_registry().await;

    // Create three notes
    let r1 = exec(
        &registry,
        json!({"action": "create_note", "title": "Architecture", "body": "System design"}),
    )
    .await;
    let id1 = r1
        .split("id: ")
        .nth(1)
        .unwrap()
        .trim_end_matches(')')
        .to_string();

    let r2 = exec(
        &registry,
        json!({"action": "create_note", "title": "Database Schema", "body": "Tables and indexes"}),
    )
    .await;
    let id2 = r2
        .split("id: ")
        .nth(1)
        .unwrap()
        .trim_end_matches(')')
        .to_string();

    let r3 = exec(
        &registry,
        json!({"action": "create_note", "title": "API Design", "body": "REST endpoints"}),
    )
    .await;
    let id3 = r3
        .split("id: ")
        .nth(1)
        .unwrap()
        .trim_end_matches(')')
        .to_string();

    // Link architecture note to both others
    let link_result = exec(
        &registry,
        json!({"action": "link_notes", "id": id1, "target_ids": [id2, id3]}),
    )
    .await;
    assert!(link_result.contains("2 target(s)"));

    // Verify links appear in get_note
    let get = exec(&registry, json!({"action": "get_note", "id": id1})).await;
    assert!(get.contains("Links to:"));
}

#[tokio::test]
async fn test_error_handling() {
    let (registry, _pool) = setup_with_registry().await;

    // Unknown action
    let result = registry
        .execute("notes", json!({"action": "fly"}), &ctx())
        .await;
    assert!(result.is_err());

    // Missing action
    let result = registry
        .execute("notes", json!({"title": "oops"}), &ctx())
        .await;
    assert!(result.is_err());

    // Get non-existent note
    let result = registry
        .execute(
            "notes",
            json!({"action": "get_note", "id": "nonexistent"}),
            &ctx(),
        )
        .await;
    assert!(result.is_err());

    // Delete non-existent note
    let result = registry
        .execute(
            "notes",
            json!({"action": "delete_note", "id": "nonexistent"}),
            &ctx(),
        )
        .await;
    assert!(result.is_err());

    // Tag non-existent note
    let result = registry
        .execute(
            "notes",
            json!({"action": "tag_note", "id": "nonexistent", "tags": ["x"]}),
            &ctx(),
        )
        .await;
    assert!(result.is_err());

    // Link non-existent source note
    let result = registry
        .execute(
            "notes",
            json!({"action": "link_notes", "id": "nonexistent", "target_ids": ["a"]}),
            &ctx(),
        )
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_pinning_notes() {
    let (registry, _pool) = setup_with_registry().await;

    let result = exec(
        &registry,
        json!({"action": "create_note", "title": "Important", "body": "Don't forget"}),
    )
    .await;
    let id = result
        .split("id: ")
        .nth(1)
        .unwrap()
        .trim_end_matches(')')
        .to_string();

    // Pin the note
    exec(
        &registry,
        json!({"action": "update_note", "id": id, "pinned": true}),
    )
    .await;

    // Verify pinned
    let get = exec(&registry, json!({"action": "get_note", "id": id})).await;
    assert!(get.contains("Pinned"));

    // Pinned notes should appear first in list
    exec(
        &registry,
        json!({"action": "create_note", "title": "Regular", "body": "Normal note"}),
    )
    .await;

    let list = exec(&registry, json!({"action": "list_notes"})).await;
    let important_pos = list.find("Important").unwrap();
    let regular_pos = list.find("Regular").unwrap();
    assert!(
        important_pos < regular_pos,
        "Pinned note should appear before regular note"
    );
}
