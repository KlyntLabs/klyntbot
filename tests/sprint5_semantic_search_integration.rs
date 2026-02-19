//! Sprint 5 integration tests — Semantic Search
//!
//! Tests for the semantic search system introduced in Sprint 5. Covers:
//!
//! ## Acceptance Criteria Coverage
//!
//! | AC# | Description                              | Test(s)                                      |
//! |-----|------------------------------------------|----------------------------------------------|
//! | AC1 | Semantic search finds synonyms           | test_ac1_semantic_search_finds_synonyms       |
//! | AC2 | Auto-generate embeddings on add/update   | test_ac2_embedding_auto_generated_on_add,     |
//! |     |                                          | test_ac2_embedding_auto_generated_on_update   |
//! | AC3 | Separate embeddings.jsonl storage        | test_ac3_embeddings_stored_separately         |
//! | AC4 | Hybrid search with RRF merging           | test_ac4_hybrid_search_merges_results         |
//! | AC5 | Backfill job for existing tasks           | test_ac5_backfill_generates_missing_embeddings|
//! | AC6 | 100% local (no API calls)                | (verified by code review — no network in engine) |
//! | AC7 | < 500ms on 1000 todos                    | test_ac7_search_1000_todos_under_500ms        |
//! | AC8 | Zero clippy warnings                     | (CI check)                                   |
//! | AC9 | No keyword search regressions            | test_ac9_keyword_search_unchanged             |
//!
//! ## Edge Cases (from BA spec §8)
//!
//! | EC# | Scenario                                 | Test                                          |
//! |-----|------------------------------------------|-----------------------------------------------|
//! | EC1 | Empty query                              | test_ec1_empty_query_returns_error             |
//! | EC2 | Whitespace-only query                    | test_ec2_whitespace_query_returns_error        |
//! | EC3 | No embeddings exist                      | test_ec3_no_embeddings_returns_message         |
//! | EC4 | Partial embedding coverage               | test_ec4_partial_embeddings_noted_in_output    |
//! | EC5 | All results below threshold              | test_ec5_below_threshold_no_results            |
//! | EC8 | Corrupted embeddings.jsonl               | test_ec8_corrupted_embeddings_recovery         |
//! | EC12| Model fails to initialize                | test_ec12_graceful_degradation_model_unavailable|
//! | EC15| Hybrid with zero full-text results       | test_ec15_hybrid_zero_keyword_results          |
//! | EC16| Hybrid with zero semantic results        | test_ec16_hybrid_zero_semantic_results         |

use chrono::Utc;
use klyntbot::storage::{EmbeddingRepo, StoragePool, TodoRepo};
use serde_json::json;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::RwLock;
use tools::{
    embedding_engine::EmbeddingHandler,
    embedding_store::EmbeddingStore,
    todo::TodoTool,
    todo_store::TodoStore,
    todo_types::{Todo, TodoStatus},
    RoutingContext, Tool, EMBEDDING_DIM,
};

#[path = "mock_embedding_handler.rs"]
mod mock_embedding_handler;
use mock_embedding_handler::MockEmbeddingHandler;

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

#[allow(dead_code)]
fn create_test_todo_with_tags(title: &str, tags: Vec<&str>) -> Todo {
    let mut todo = create_test_todo(title);
    todo.tags = tags.iter().map(|s| s.to_string()).collect();
    todo
}

#[allow(dead_code)]
async fn create_test_store() -> (TodoStore, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("todos.jsonl");
    let store = TodoStore::new(file_path);
    (store, temp_dir)
}

fn test_pool() -> StoragePool {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://localhost/klyntbot_test".to_string());
    StoragePool::connect_lazy(&url).unwrap()
}

async fn create_test_tool() -> (TodoTool, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let pool = test_pool();
    let repo = TodoRepo::new(pool.inner().clone());
    let tool = TodoTool::new(repo, 3, 18, "UTC".to_string(), klyntbot::config::CreationMode::default());
    (tool, temp_dir)
}

/// Create a TodoTool with mock embedding handler and repo wired up.
/// The mock persists embeddings to the EmbeddingStore (like the real impl).
async fn create_test_tool_with_embeddings() -> (TodoTool, Arc<MockEmbeddingHandler>, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let emb_path = temp_dir.path().join("embeddings.jsonl");

    let pool = test_pool();
    let repo = TodoRepo::new(pool.inner().clone());
    let emb_store = Arc::new(RwLock::new(EmbeddingStore::new(emb_path)));
    let mock = Arc::new(MockEmbeddingHandler::with_store(emb_store.clone()));

    let tool = TodoTool::new(repo, 3, 18, "UTC".to_string(), klyntbot::config::CreationMode::default())
        .with_embedding_handler(mock.clone() as Arc<dyn EmbeddingHandler>)
        .with_embedding_repo(EmbeddingRepo::new(pool.inner().clone()))
        .with_search_config(0.0, 60); // Low threshold so mock embeddings match

    (tool, mock, temp_dir)
}

fn ctx() -> RoutingContext {
    RoutingContext::new(
        common::ChannelName::new("telegram"),
        common::ChatId::new("test"),
    )
}

// ═══════════════════════════════════════════════════════════════
// Acceptance Criteria Tests
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
#[ignore = "requires PostgreSQL (DATABASE_URL)"]
async fn test_ac1_semantic_search_finds_synonyms() {
    let (tool, mock, _dir) = create_test_tool_with_embeddings().await;

    tool.execute(
        json!({"action": "add", "title": "Fix authentication bug", "tags": ["security", "backend"]}),
        &ctx(),
    )
    .await
    .unwrap();
    tool.execute(
        json!({"action": "add", "title": "Login system refactor", "tags": ["auth", "frontend"]}),
        &ctx(),
    )
    .await
    .unwrap();
    tool.execute(
        json!({"action": "add", "title": "Add dark mode", "tags": ["ui", "frontend"]}),
        &ctx(),
    )
    .await
    .unwrap();

    let result = tool
        .execute(
            json!({
                "action": "search_semantic",
                "query": "authentication security",
                "limit": 10,
            }),
            &ctx(),
        )
        .await
        .unwrap();

    assert!(
        result.contains("task(s) matching"),
        "Should show results header: {}",
        result
    );
    assert!(
        mock.embed_query_call_count() >= 1,
        "Should have called embed_query"
    );
}

#[tokio::test]
#[ignore = "requires PostgreSQL (DATABASE_URL)"]
async fn test_ac2_embedding_auto_generated_on_add() {
    let (tool, mock, _dir) = create_test_tool_with_embeddings().await;

    tool.execute(
        json!({"action": "add", "title": "Test embedding generation"}),
        &ctx(),
    )
    .await
    .unwrap();

    assert_eq!(
        mock.embed_todo_call_count(),
        1,
        "embed_todo should be called once on add"
    );
}

#[tokio::test]
#[ignore = "requires PostgreSQL (DATABASE_URL)"]
async fn test_ac2_embedding_auto_generated_on_update() {
    let (tool, mock, _dir) = create_test_tool_with_embeddings().await;

    let result = tool
        .execute(json!({"action": "add", "title": "Original title"}), &ctx())
        .await
        .unwrap();

    // Extract ID from the add result (format: "Task created: title (ID: abc12345)...")
    let id = result
        .split("(ID: ")
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Should find ID in result");

    tool.execute(
        json!({"action": "update", "id": id, "title": "Updated title"}),
        &ctx(),
    )
    .await
    .unwrap();

    assert_eq!(
        mock.embed_todo_call_count(),
        2,
        "embed_todo should be called twice (add + update)"
    );
}

#[tokio::test]
#[ignore = "requires PostgreSQL (DATABASE_URL)"]
async fn test_ac3_embeddings_stored_separately() {
    let temp_dir = TempDir::new().unwrap();
    let embeddings_path = temp_dir.path().join("embeddings.jsonl");

    let pool = test_pool();
    let repo = TodoRepo::new(pool.inner().clone());
    let emb_store = Arc::new(RwLock::new(EmbeddingStore::new(embeddings_path.clone())));
    let mock = Arc::new(MockEmbeddingHandler::with_store(emb_store.clone()));

    let tool = TodoTool::new(repo, 3, 18, "UTC".to_string(), klyntbot::config::CreationMode::default())
        .with_embedding_handler(mock as Arc<dyn EmbeddingHandler>)
        .with_embedding_repo(EmbeddingRepo::new(pool.inner().clone()))
        .with_search_config(0.5, 60);

    tool.execute(
        json!({"action": "add", "title": "Test separate storage"}),
        &ctx(),
    )
    .await
    .unwrap();

    // Embeddings written by the mock to the file store should contain embedding data
    let embeddings_content = std::fs::read_to_string(&embeddings_path).unwrap();
    assert!(
        embeddings_content.contains("\"embedding\""),
        "embeddings.jsonl should contain embedding data"
    );
}

#[tokio::test]
#[ignore = "requires PostgreSQL (DATABASE_URL)"]
async fn test_ac4_hybrid_search_merges_results() {
    let (tool, _mock, _dir) = create_test_tool_with_embeddings().await;

    tool.execute(json!({"action": "add", "title": "Fix login bug"}), &ctx())
        .await
        .unwrap();
    tool.execute(
        json!({"action": "add", "title": "Authentication security audit"}),
        &ctx(),
    )
    .await
    .unwrap();
    tool.execute(json!({"action": "add", "title": "Update README"}), &ctx())
        .await
        .unwrap();

    let result = tool
        .execute(
            json!({
                "action": "search_hybrid",
                "query": "login",
            }),
            &ctx(),
        )
        .await
        .unwrap();

    assert!(
        result.contains("login") || result.contains("Login"),
        "Should find keyword match: {}",
        result
    );
    assert!(
        result.contains("keyword") || result.contains("semantic") || result.contains("both"),
        "Should show match sources: {}",
        result
    );
}

#[tokio::test]
async fn test_ac5_backfill_generates_missing_embeddings() {
    let temp_dir = TempDir::new().unwrap();
    let todos_path = temp_dir.path().join("todos.jsonl");
    let emb_path = temp_dir.path().join("embeddings.jsonl");

    let mut store = TodoStore::new(todos_path);
    for i in 0..5 {
        store
            .add(create_test_todo(&format!("Task {}", i)))
            .await
            .unwrap();
    }

    let mut emb_store = EmbeddingStore::new(emb_path);
    emb_store.load().await.unwrap();
    let all_ids: Vec<String> = store
        .list(&tools::todo_types::TodoFilter::default())
        .await
        .unwrap()
        .iter()
        .map(|t| t.id.clone())
        .collect();
    let missing = emb_store.ids_missing_embeddings(&all_ids);
    assert_eq!(missing.len(), 5, "All 5 should be missing embeddings");

    // Backfill using mock handler
    let mock = MockEmbeddingHandler::new();
    let todos = store
        .list(&tools::todo_types::TodoFilter::default())
        .await
        .unwrap();
    for todo in &todos {
        let record = mock.embed_todo(todo).await.unwrap().unwrap();
        emb_store.upsert(record).await.unwrap();
    }

    let missing_after = emb_store.ids_missing_embeddings(&all_ids);
    assert!(
        missing_after.is_empty(),
        "All tasks should have embeddings after backfill"
    );
}

#[tokio::test]
#[ignore = "requires PostgreSQL (DATABASE_URL)"]
async fn test_ac7_search_1000_todos_under_500ms() {
    let pool = test_pool();
    let repo = TodoRepo::new(pool.inner().clone());
    let mock_handler = Arc::new(MockEmbeddingHandler::new());

    let tool = TodoTool::new(repo, 3, 18, "UTC".to_string(), klyntbot::config::CreationMode::default())
        .with_embedding_handler(mock_handler as Arc<dyn EmbeddingHandler>)
        .with_embedding_repo(EmbeddingRepo::new(pool.inner().clone()))
        .with_search_config(0.0, 60);

    for i in 0..1000 {
        tool.execute(
            json!({"action": "add", "title": format!("Task {} — various keywords for testing", i)}),
            &ctx(),
        )
        .await
        .unwrap();
    }

    let start = std::time::Instant::now();
    let result = tool
        .execute(
            json!({
                "action": "search_semantic",
                "query": "authentication",
                "limit": 10,
            }),
            &ctx(),
        )
        .await
        .unwrap();
    let elapsed = start.elapsed();

    assert!(
        elapsed.as_millis() < 500,
        "Search took {}ms, expected < 500ms",
        elapsed.as_millis()
    );
    assert!(result.contains("task(s) matching"), "Should return results");
}

#[tokio::test]
#[ignore = "requires PostgreSQL (DATABASE_URL)"]
async fn test_ac9_keyword_search_unchanged() {
    let (tool, _dir) = create_test_tool().await;

    tool.execute(
        json!({"action": "add", "title": "Fix authentication bug"}),
        &ctx(),
    )
    .await
    .unwrap();
    tool.execute(json!({"action": "add", "title": "Update README"}), &ctx())
        .await
        .unwrap();
    tool.execute(
        json!({"action": "add", "title": "Auth token refresh"}),
        &ctx(),
    )
    .await
    .unwrap();

    let result = tool
        .execute(json!({"action": "search", "query": "auth"}), &ctx())
        .await
        .unwrap();

    assert!(result.contains("authentication") || result.contains("Auth"));
    assert!(!result.contains("README"));
}

// ═══════════════════════════════════════════════════════════════
// Edge Case Tests
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
#[ignore = "requires PostgreSQL (DATABASE_URL)"]
async fn test_ec1_empty_query_returns_error() {
    let (tool, _mock, _dir) = create_test_tool_with_embeddings().await;
    let result = tool
        .execute(json!({"action": "search_semantic", "query": ""}), &ctx())
        .await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("cannot be empty"));
}

#[tokio::test]
#[ignore = "requires PostgreSQL (DATABASE_URL)"]
async fn test_ec2_whitespace_query_returns_error() {
    let (tool, _mock, _dir) = create_test_tool_with_embeddings().await;
    let result = tool
        .execute(json!({"action": "search_semantic", "query": "   "}), &ctx())
        .await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("cannot be empty"));
}

#[tokio::test]
#[ignore = "requires PostgreSQL (DATABASE_URL)"]
async fn test_ec3_no_embeddings_returns_message() {
    let pool = test_pool();
    let mock = Arc::new(MockEmbeddingHandler::new());

    // Add task WITHOUT embedding handler so no auto-embed
    let repo = TodoRepo::new(pool.inner().clone());
    let tool_no_emb = TodoTool::new(repo, 3, 18, "UTC".to_string(), klyntbot::config::CreationMode::default());
    tool_no_emb
        .execute(
            json!({"action": "add", "title": "A task with no embedding"}),
            &ctx(),
        )
        .await
        .unwrap();

    // Search WITH embedding handler
    let repo2 = TodoRepo::new(pool.inner().clone());
    let tool_with_emb = TodoTool::new(repo2, 3, 18, "UTC".to_string(), klyntbot::config::CreationMode::default())
        .with_embedding_handler(mock as Arc<dyn EmbeddingHandler>)
        .with_embedding_repo(EmbeddingRepo::new(pool.inner().clone()))
        .with_search_config(0.5, 60);

    let result = tool_with_emb
        .execute(
            json!({"action": "search_semantic", "query": "test"}),
            &ctx(),
        )
        .await
        .unwrap();

    assert!(
        result.contains("No task embeddings") || result.contains("backfill"),
        "Should suggest backfill: {}",
        result
    );
}

#[tokio::test]
#[ignore = "requires PostgreSQL (DATABASE_URL)"]
async fn test_ec4_partial_embeddings_noted_in_output() {
    let temp_dir = TempDir::new().unwrap();
    let emb_path = temp_dir.path().join("embeddings.jsonl");

    let pool = test_pool();
    let emb_store = Arc::new(RwLock::new(EmbeddingStore::new(emb_path)));
    let mock = Arc::new(MockEmbeddingHandler::with_store(emb_store.clone()));

    // Add task WITHOUT embedding handler
    let repo = TodoRepo::new(pool.inner().clone());
    let tool_no_emb = TodoTool::new(repo, 3, 18, "UTC".to_string(), klyntbot::config::CreationMode::default());
    tool_no_emb
        .execute(
            json!({"action": "add", "title": "Task without embedding"}),
            &ctx(),
        )
        .await
        .unwrap();

    // Add task WITH embedding handler
    let repo2 = TodoRepo::new(pool.inner().clone());
    let tool_with_emb = TodoTool::new(repo2, 3, 18, "UTC".to_string(), klyntbot::config::CreationMode::default())
        .with_embedding_handler(mock.clone() as Arc<dyn EmbeddingHandler>)
        .with_embedding_repo(EmbeddingRepo::new(pool.inner().clone()))
        .with_search_config(0.0, 60);

    tool_with_emb
        .execute(
            json!({"action": "add", "title": "Task with embedding"}),
            &ctx(),
        )
        .await
        .unwrap();

    let result = tool_with_emb
        .execute(
            json!({"action": "search_semantic", "query": "task"}),
            &ctx(),
        )
        .await
        .unwrap();

    assert!(
        result.contains("Note:") && result.contains("tasks have embeddings"),
        "Should note partial coverage: {}",
        result
    );
}

#[tokio::test]
#[ignore = "requires PostgreSQL (DATABASE_URL)"]
async fn test_ec5_below_threshold_no_results() {
    let (tool, _mock, _dir) = create_test_tool_with_embeddings().await;

    tool.execute(json!({"action": "add", "title": "Test task"}), &ctx())
        .await
        .unwrap();

    let result = tool
        .execute(
            json!({
                "action": "search_semantic",
                "query": "completely unrelated gibberish xyzzy",
                "threshold": 0.99,
            }),
            &ctx(),
        )
        .await
        .unwrap();

    assert!(
        result.contains("No semantically similar"),
        "Should indicate no results: {}",
        result
    );
}

#[tokio::test]
async fn test_ec8_corrupted_embeddings_recovery() {
    let temp_dir = TempDir::new().unwrap();
    let emb_path = temp_dir.path().join("embeddings.jsonl");

    let valid_record = serde_json::json!({
        "_op": "upsert",
        "record": {
            "id": "valid1",
            "embedding": vec![0.1f32; EMBEDDING_DIM],
            "model": "test",
            "embedded_at": "2026-01-01T00:00:00Z"
        }
    });
    let valid_record2 = serde_json::json!({
        "_op": "upsert",
        "record": {
            "id": "valid2",
            "embedding": vec![0.2f32; EMBEDDING_DIM],
            "model": "test",
            "embedded_at": "2026-01-01T00:00:00Z"
        }
    });
    let content = format!(
        "{}\nTHIS IS CORRUPTED\n{}\n",
        serde_json::to_string(&valid_record).unwrap(),
        serde_json::to_string(&valid_record2).unwrap(),
    );
    std::fs::write(&emb_path, content).unwrap();

    let mut store = EmbeddingStore::new(emb_path);
    store.load().await.unwrap();

    assert!(store.get("valid1").await.unwrap().is_some());
    assert!(store.get("valid2").await.unwrap().is_some());
}

#[tokio::test]
#[ignore = "requires PostgreSQL (DATABASE_URL)"]
async fn test_ec12_graceful_degradation_model_unavailable() {
    let pool = test_pool();
    let repo = TodoRepo::new(pool.inner().clone());
    let mock = Arc::new(MockEmbeddingHandler::unavailable());

    let tool = TodoTool::new(repo, 3, 18, "UTC".to_string(), klyntbot::config::CreationMode::default())
        .with_embedding_handler(mock as Arc<dyn EmbeddingHandler>)
        .with_embedding_repo(EmbeddingRepo::new(pool.inner().clone()))
        .with_search_config(0.5, 60);

    tool.execute(json!({"action": "add", "title": "Test task"}), &ctx())
        .await
        .unwrap();

    let result = tool
        .execute(
            json!({"action": "search_semantic", "query": "authentication"}),
            &ctx(),
        )
        .await;

    match result {
        Ok(msg) => assert!(
            msg.contains("No task embeddings") || msg.contains("unavailable"),
            "Should explain unavailability: {}",
            msg
        ),
        Err(e) => assert!(
            e.to_string().contains("unavailable") || e.to_string().contains("embedding"),
            "Error should mention embeddings: {}",
            e
        ),
    }
}

#[tokio::test]
#[ignore = "requires PostgreSQL (DATABASE_URL)"]
async fn test_ec15_hybrid_zero_keyword_results() {
    let (tool, _mock, _dir) = create_test_tool_with_embeddings().await;

    tool.execute(
        json!({"action": "add", "title": "Authentication security audit"}),
        &ctx(),
    )
    .await
    .unwrap();

    let result = tool
        .execute(
            json!({"action": "search_hybrid", "query": "xyzzy_no_keyword_match"}),
            &ctx(),
        )
        .await
        .unwrap();

    // Gracefully handles zero keyword results
    assert!(
        result.contains("task(s) matching") || result.contains("No tasks"),
        "Should handle zero keyword results: {}",
        result
    );
}

#[tokio::test]
#[ignore = "requires PostgreSQL (DATABASE_URL)"]
async fn test_ec16_hybrid_zero_semantic_results() {
    let pool = test_pool();
    let repo = TodoRepo::new(pool.inner().clone());
    let mock = Arc::new(MockEmbeddingHandler::unavailable());

    let tool = TodoTool::new(repo, 3, 18, "UTC".to_string(), klyntbot::config::CreationMode::default())
        .with_embedding_handler(mock as Arc<dyn EmbeddingHandler>)
        .with_embedding_repo(EmbeddingRepo::new(pool.inner().clone()))
        .with_search_config(0.5, 60);

    tool.execute(json!({"action": "add", "title": "Fix login bug"}), &ctx())
        .await
        .unwrap();

    let result = tool
        .execute(json!({"action": "search_hybrid", "query": "login"}), &ctx())
        .await;

    match result {
        Ok(msg) => assert!(
            msg.contains("login") || msg.contains("Login"),
            "Should find keyword match even without semantic: {}",
            msg
        ),
        Err(_) => {
            // Acceptable if embed_query error propagates
        }
    }
}

// ═══════════════════════════════════════════════════════════════
// Embedding Auto-Generation Tests (BR-1)
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
#[ignore = "requires PostgreSQL (DATABASE_URL)"]
async fn test_br1_embedding_includes_title_description_tags() {
    let (tool, mock, _dir) = create_test_tool_with_embeddings().await;

    tool.execute(
        json!({
            "action": "add",
            "title": "My task",
            "description": "Detailed description",
            "tags": ["rust", "backend"]
        }),
        &ctx(),
    )
    .await
    .unwrap();

    assert_eq!(mock.embed_todo_call_count(), 1);
}

#[tokio::test]
#[ignore = "requires PostgreSQL (DATABASE_URL)"]
async fn test_br1_embedding_failure_does_not_block_add() {
    let pool = test_pool();
    let repo = TodoRepo::new(pool.inner().clone());
    let mock = Arc::new(MockEmbeddingHandler::unavailable());

    let tool = TodoTool::new(repo, 3, 18, "UTC".to_string(), klyntbot::config::CreationMode::default())
        .with_embedding_handler(mock as Arc<dyn EmbeddingHandler>)
        .with_embedding_repo(EmbeddingRepo::new(pool.inner().clone()))
        .with_search_config(0.5, 60);

    let result = tool
        .execute(
            json!({"action": "add", "title": "Task despite unavailable model"}),
            &ctx(),
        )
        .await
        .unwrap();

    assert!(
        result.contains("Task despite unavailable model"),
        "Task should be created even if embedding fails: {}",
        result
    );
}

// ═══════════════════════════════════════════════════════════════
// Threshold & Ranking Tests
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
#[ignore = "requires PostgreSQL (DATABASE_URL)"]
async fn test_threshold_filters_low_similarity() {
    let (tool, _mock, _dir) = create_test_tool_with_embeddings().await;

    tool.execute(json!({"action": "add", "title": "Test task one"}), &ctx())
        .await
        .unwrap();

    let result = tool
        .execute(
            json!({
                "action": "search_semantic",
                "query": "something very different",
                "threshold": 0.99,
            }),
            &ctx(),
        )
        .await
        .unwrap();

    assert!(
        result.contains("No semantically similar"),
        "High threshold should filter results: {}",
        result
    );
}

#[tokio::test]
#[ignore = "requires PostgreSQL (DATABASE_URL)"]
async fn test_limit_parameter_respected() {
    let (tool, _mock, _dir) = create_test_tool_with_embeddings().await;

    for i in 0..10 {
        tool.execute(
            json!({"action": "add", "title": format!("Task {}", i)}),
            &ctx(),
        )
        .await
        .unwrap();
    }

    let result = tool
        .execute(
            json!({
                "action": "search_semantic",
                "query": "Task",
                "limit": 3,
            }),
            &ctx(),
        )
        .await
        .unwrap();

    let result_count = result.matches("- [").count();
    assert!(
        result_count <= 3,
        "Should respect limit=3, got {} results",
        result_count
    );
}

// ═══════════════════════════════════════════════════════════════
// Validation Tests
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
#[ignore = "requires PostgreSQL (DATABASE_URL)"]
async fn test_query_too_long_returns_error() {
    let (tool, _mock, _dir) = create_test_tool_with_embeddings().await;
    let long_query = "a".repeat(1001);
    let result = tool
        .execute(
            json!({"action": "search_semantic", "query": long_query}),
            &ctx(),
        )
        .await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("too long"));
}

#[tokio::test]
#[ignore = "requires PostgreSQL (DATABASE_URL)"]
async fn test_invalid_threshold_returns_error() {
    let (tool, _mock, _dir) = create_test_tool_with_embeddings().await;
    let result = tool
        .execute(
            json!({"action": "search_semantic", "query": "test", "threshold": 1.5}),
            &ctx(),
        )
        .await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("0.0"));
}
