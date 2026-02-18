//! GraphQL Query resolver tests for the dashboard crate.
//!
//! Tests all read operations: todos, projects, goals, plans, system status, search.
//! Each test maps to specific acceptance criteria (AC-XX.Y) from the test plan.

mod common;

use async_graphql::Request;
use tools::todo_types::TodoStatus;

// Helper: execute a GQL query string and return the JSON data map.
macro_rules! execute {
    ($schema:expr, $query:expr) => {{
        let res = $schema.execute(Request::new($query)).await;
        assert!(res.errors.is_empty(), "GraphQL errors: {:?}", res.errors);
        res.data.into_json().unwrap()
    }};
}

// Helper: execute and expect at least one error.
macro_rules! execute_err {
    ($schema:expr, $query:expr) => {{
        let res = $schema.execute(Request::new($query)).await;
        assert!(
            !res.errors.is_empty(),
            "Expected GraphQL errors but got none"
        );
        res
    }};
}

// ═══════════════════════════════════════════════════════════════════
// Todo Queries
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_todos_query_returns_all_todos() {
    // AC-03.1: Board shows all tasks
    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = common::create_test_context(&tmp).await;
    {
        let mut store = ctx.todo_store.write().await;
        store.add(common::make_todo("Task 1")).await.unwrap();
        store.add(common::make_todo("Task 2")).await.unwrap();
        store.add(common::make_todo("Task 3")).await.unwrap();
    }
    let schema = common::create_test_schema(ctx);

    let data = execute!(schema, "{ todos { totalCount nodes { id title } } }");

    assert_eq!(data["todos"]["totalCount"], 3);
    assert_eq!(data["todos"]["nodes"].as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn test_todos_query_with_status_filter_todo() {
    // AC-03.1: Filter by status
    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = common::create_test_context(&tmp).await;
    {
        let mut store = ctx.todo_store.write().await;
        let mut t1 = common::make_todo("Todo task");
        t1.status = TodoStatus::Todo;
        let mut t2 = common::make_todo("Doing task");
        t2.status = TodoStatus::Doing;
        let mut t3 = common::make_todo("Done task");
        t3.status = TodoStatus::Done;
        store.add(t1).await.unwrap();
        store.add(t2).await.unwrap();
        store.add(t3).await.unwrap();
    }
    let schema = common::create_test_schema(ctx);

    let data = execute!(
        schema,
        r#"{ todos(filter: { status: TODO }) { totalCount nodes { status } } }"#
    );

    assert_eq!(data["todos"]["totalCount"], 1);
    let nodes = data["todos"]["nodes"].as_array().unwrap();
    assert!(nodes
        .iter()
        .all(|n| n["status"].as_str().unwrap() == "TODO"));
}

#[tokio::test]
async fn test_todos_query_with_status_filter_doing() {
    // AC-03.1: Filter by Doing status
    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = common::create_test_context(&tmp).await;
    {
        let mut store = ctx.todo_store.write().await;
        let mut t1 = common::make_todo("Todo task");
        t1.status = TodoStatus::Todo;
        let mut t2 = common::make_todo("Doing task 1");
        t2.status = TodoStatus::Doing;
        let mut t3 = common::make_todo("Doing task 2");
        t3.status = TodoStatus::Doing;
        store.add(t1).await.unwrap();
        store.add(t2).await.unwrap();
        store.add(t3).await.unwrap();
    }
    let schema = common::create_test_schema(ctx);

    let data = execute!(
        schema,
        r#"{ todos(filter: { status: DOING }) { totalCount } }"#
    );
    assert_eq!(data["todos"]["totalCount"], 2);
}

#[tokio::test]
async fn test_todos_query_with_status_filter_done() {
    // AC-03.1: Filter by Done status
    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = common::create_test_context(&tmp).await;
    {
        let mut store = ctx.todo_store.write().await;
        let mut t1 = common::make_todo("Done 1");
        t1.status = TodoStatus::Done;
        let mut t2 = common::make_todo("Done 2");
        t2.status = TodoStatus::Done;
        let t3 = common::make_todo("Active");
        store.add(t1).await.unwrap();
        store.add(t2).await.unwrap();
        store.add(t3).await.unwrap();
    }
    let schema = common::create_test_schema(ctx);

    let data = execute!(
        schema,
        r#"{ todos(filter: { status: DONE }) { totalCount } }"#
    );
    assert_eq!(data["todos"]["totalCount"], 2);
}

#[tokio::test]
async fn test_todos_query_with_priority_filter() {
    // AC-03.1: Filter by priority range
    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = common::create_test_context(&tmp).await;
    {
        let mut store = ctx.todo_store.write().await;
        let mut t1 = common::make_todo("High prio");
        t1.priority = Some(1);
        let mut t2 = common::make_todo("Med prio");
        t2.priority = Some(3);
        let mut t3 = common::make_todo("Low prio");
        t3.priority = Some(4);
        store.add(t1).await.unwrap();
        store.add(t2).await.unwrap();
        store.add(t3).await.unwrap();
    }
    let schema = common::create_test_schema(ctx);

    let data = execute!(
        schema,
        r#"{ todos(filter: { priorityMin: 3 }) { totalCount } }"#
    );
    // priorityMin: 3 means priority >= 3 (lower number = higher priority in this codebase)
    // The native filter is `priority_min: Some(3)` meaning priority value >= 3
    assert_eq!(data["todos"]["totalCount"], 2);
}

#[tokio::test]
async fn test_todos_query_with_project_filter() {
    // AC-09.3: Project detail shows task tree
    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = common::create_test_context(&tmp).await;
    {
        let mut store = ctx.todo_store.write().await;
        let mut t1 = common::make_todo("Proj A task");
        t1.project_id = Some("proj-001".to_string());
        let mut t2 = common::make_todo("Proj B task");
        t2.project_id = Some("proj-002".to_string());
        let t3 = common::make_todo("No project");
        store.add(t1).await.unwrap();
        store.add(t2).await.unwrap();
        store.add(t3).await.unwrap();
    }
    let schema = common::create_test_schema(ctx);

    let data = execute!(
        schema,
        r#"{ todos(filter: { projectId: "proj-001" }) { totalCount nodes { projectId } } }"#
    );
    assert_eq!(data["todos"]["totalCount"], 1);
    assert_eq!(
        data["todos"]["nodes"][0]["projectId"].as_str().unwrap(),
        "proj-001"
    );
}

#[tokio::test]
async fn test_todos_query_with_tag_filter() {
    // AC-03.1: Filter by tag
    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = common::create_test_context(&tmp).await;
    {
        let mut store = ctx.todo_store.write().await;
        let mut t1 = common::make_todo("Bug task");
        t1.tags = vec!["bug".to_string()];
        let mut t2 = common::make_todo("Feature task");
        t2.tags = vec!["feature".to_string()];
        store.add(t1).await.unwrap();
        store.add(t2).await.unwrap();
    }
    let schema = common::create_test_schema(ctx);

    let data = execute!(
        schema,
        r#"{ todos(filter: { tag: "bug" }) { totalCount } }"#
    );
    assert_eq!(data["todos"]["totalCount"], 1);
}

#[tokio::test]
async fn test_todos_query_pagination() {
    // AC-03.1: Paginated results
    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = common::create_test_context(&tmp).await;
    {
        let mut store = ctx.todo_store.write().await;
        for i in 0..12 {
            store
                .add(common::make_todo(&format!("Task {}", i)))
                .await
                .unwrap();
        }
    }
    let schema = common::create_test_schema(ctx);

    let data = execute!(
        schema,
        r#"{ todos(limit: 5, offset: 0) { totalCount nodes { id } } }"#
    );
    assert_eq!(data["todos"]["totalCount"], 12);
    assert_eq!(data["todos"]["nodes"].as_array().unwrap().len(), 5);
}

#[tokio::test]
async fn test_single_todo_query() {
    // AC-03.3: Click card opens detail
    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = common::create_test_context(&tmp).await;
    let todo_id = {
        let mut store = ctx.todo_store.write().await;
        let mut t = common::make_todo("Detail task");
        t.id = "todo-001".to_string();
        t.description = Some("A description".to_string());
        t.priority = Some(2);
        store.add(t).await.unwrap().id
    };
    let schema = common::create_test_schema(ctx);

    let data = execute!(
        schema,
        &format!(
            r#"{{ todo(id: "{}") {{ id title description priority status }} }}"#,
            todo_id
        )
    );
    assert_eq!(data["todo"]["id"].as_str().unwrap(), "todo-001");
    assert_eq!(data["todo"]["title"].as_str().unwrap(), "Detail task");
    assert_eq!(
        data["todo"]["description"].as_str().unwrap(),
        "A description"
    );
    assert_eq!(data["todo"]["priority"].as_i64().unwrap(), 2);
}

#[tokio::test]
async fn test_single_todo_query_not_found() {
    // AC-03.3: Error handling for missing todo
    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = common::create_test_context(&tmp).await;
    let schema = common::create_test_schema(ctx);

    let data = execute!(schema, r#"{ todo(id: "nonexistent") { id } }"#);
    assert!(data["todo"].is_null());
}

#[tokio::test]
async fn test_todo_tree_query() {
    // AC-06.1: Subtasks shown as nested list
    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = common::create_test_context(&tmp).await;
    {
        let mut store = ctx.todo_store.write().await;
        let mut parent = common::make_todo("Parent");
        parent.id = "parent-001".to_string();
        let mut child1 = common::make_todo("Child 1");
        child1.parent_id = Some("parent-001".to_string());
        let mut child2 = common::make_todo("Child 2");
        child2.parent_id = Some("parent-001".to_string());
        store.add(parent).await.unwrap();
        store.add(child1).await.unwrap();
        store.add(child2).await.unwrap();
    }
    let schema = common::create_test_schema(ctx);

    let data = execute!(
        schema,
        r#"{ todoTree { todo { id title } children { todo { title } } } }"#
    );

    let nodes = data["todoTree"].as_array().unwrap();
    // There should be 1 root (parent-001)
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0]["todo"]["id"].as_str().unwrap(), "parent-001");
    // Parent should have 2 children
    assert_eq!(nodes[0]["children"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn test_todo_tree_query_with_project_filter() {
    // AC-09.3: Project task tree filtered
    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = common::create_test_context(&tmp).await;
    {
        let mut store = ctx.todo_store.write().await;
        let mut pa = common::make_todo("Proj A root");
        pa.project_id = Some("proj-a".to_string());
        let mut pb = common::make_todo("Proj B root");
        pb.project_id = Some("proj-b".to_string());
        store.add(pa).await.unwrap();
        store.add(pb).await.unwrap();
    }
    let schema = common::create_test_schema(ctx);

    let data = execute!(
        schema,
        r#"{ todoTree(projectId: "proj-a") { todo { projectId } } }"#
    );
    let nodes = data["todoTree"].as_array().unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0]["todo"]["projectId"].as_str().unwrap(), "proj-a");
}

#[tokio::test]
async fn test_todo_summary_query() {
    // AC-01.1: Dashboard summary cards
    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = common::create_test_context(&tmp).await;
    {
        let mut store = ctx.todo_store.write().await;
        for _ in 0..3 {
            store.add(common::make_todo("Todo")).await.unwrap();
        }
        for _ in 0..2 {
            let mut t = common::make_todo("Doing");
            t.status = TodoStatus::Doing;
            store.add(t).await.unwrap();
        }
        let mut done = common::make_todo("Done");
        done.status = TodoStatus::Done;
        store.add(done).await.unwrap();
    }
    let schema = common::create_test_schema(ctx);

    let data = execute!(
        schema,
        r#"{ todoSummary { total byStatus { status count } overdueCount focusActive } }"#
    );

    assert_eq!(data["todoSummary"]["total"].as_i64().unwrap(), 6);
    let by_status = data["todoSummary"]["byStatus"].as_array().unwrap();
    assert!(!by_status.is_empty());
}

// ═══════════════════════════════════════════════════════════════════
// Search Queries
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_search_todos_keyword_mode() {
    // AC-04.1: Keyword search
    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = common::create_test_context(&tmp).await;
    {
        let mut store = ctx.todo_store.write().await;
        store
            .add(common::make_todo("Fix authentication bug"))
            .await
            .unwrap();
        store
            .add(common::make_todo("Add user profile"))
            .await
            .unwrap();
    }
    let schema = common::create_test_schema(ctx);

    let data = execute!(
        schema,
        r#"{ searchTodos(query: "auth", mode: KEYWORD) { todo { title } } }"#
    );
    let results = data["searchTodos"].as_array().unwrap();
    assert_eq!(results.len(), 1);
    assert!(results[0]["todo"]["title"]
        .as_str()
        .unwrap()
        .contains("auth"));
}

#[tokio::test]
async fn test_search_todos_keyword_no_results() {
    // AC-04.4: Empty search results
    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = common::create_test_context(&tmp).await;
    {
        let mut store = ctx.todo_store.write().await;
        store.add(common::make_todo("Normal task")).await.unwrap();
    }
    let schema = common::create_test_schema(ctx);

    let data = execute!(
        schema,
        r#"{ searchTodos(query: "xyznonexistent", mode: KEYWORD) { todo { id } } }"#
    );
    assert_eq!(data["searchTodos"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_search_todos_semantic_mode() {
    // AC-04.2: Semantic search (stubbed — returns empty)
    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = common::create_test_context(&tmp).await;
    let schema = common::create_test_schema(ctx);

    // Semantic search is a stub that returns empty — no panic expected
    let data = execute!(
        schema,
        r#"{ searchTodos(query: "login problems", mode: SEMANTIC) { todo { id } } }"#
    );
    let _ = data["searchTodos"].as_array().unwrap();
}

#[tokio::test]
async fn test_search_todos_hybrid_mode() {
    // AC-04.3: Hybrid search (stubbed — returns empty)
    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = common::create_test_context(&tmp).await;
    let schema = common::create_test_schema(ctx);

    let data = execute!(
        schema,
        r#"{ searchTodos(query: "authentication bug", mode: HYBRID) { todo { id } } }"#
    );
    let _ = data["searchTodos"].as_array().unwrap();
}

// ═══════════════════════════════════════════════════════════════════
// Project Queries
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_projects_query_returns_all() {
    // AC-09.1: Project grid shows cards
    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = common::create_test_context(&tmp).await;
    {
        let mut store = ctx.project_store.write().await;
        store
            .add(common::make_project("Project Alpha"))
            .await
            .unwrap();
        store
            .add(common::make_project("Project Beta"))
            .await
            .unwrap();
        store
            .add(common::make_project("Project Gamma"))
            .await
            .unwrap();
    }
    let schema = common::create_test_schema(ctx);

    let data = execute!(schema, r#"{ projects { id name status } }"#);
    let projects = data["projects"].as_array().unwrap();
    assert_eq!(projects.len(), 3);
}

#[tokio::test]
async fn test_projects_query_with_status_filter() {
    // AC-09.1: Filter by project status
    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = common::create_test_context(&tmp).await;
    {
        let mut store = ctx.project_store.write().await;
        let active = common::make_project("Active Project");
        let mut archived = common::make_project("Archived Project");
        archived.status = tools::project_types::ProjectStatus::Archived;
        store.add(active).await.unwrap();
        store.add(archived).await.unwrap();
    }
    let schema = common::create_test_schema(ctx);

    let data = execute!(
        schema,
        r#"{ projects(filter: { status: ACTIVE }) { id status } }"#
    );
    let projects = data["projects"].as_array().unwrap();
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0]["status"].as_str().unwrap(), "ACTIVE");
}

#[tokio::test]
async fn test_single_project_query() {
    // AC-09.3: Project detail
    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = common::create_test_context(&tmp).await;
    let project_id = {
        let mut store = ctx.project_store.write().await;
        let mut p = common::make_project("My Project");
        p.id = "proj-001".to_string();
        p.description = Some("A test project".to_string());
        store.add(p).await.unwrap().id
    };
    let schema = common::create_test_schema(ctx);

    let data = execute!(
        schema,
        &format!(
            r#"{{ project(id: "{}") {{ id name description status }} }}"#,
            project_id
        )
    );
    assert_eq!(data["project"]["id"].as_str().unwrap(), "proj-001");
    assert_eq!(data["project"]["name"].as_str().unwrap(), "My Project");
    assert_eq!(
        data["project"]["description"].as_str().unwrap(),
        "A test project"
    );
}

// ═══════════════════════════════════════════════════════════════════
// Goal Queries (stubs — dev-2 implements goal store)
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_goals_query_returns_all() {
    // AC-10.1: Goal cards with progress bars
    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = common::create_test_context(&tmp).await;
    let schema = common::create_test_schema(ctx);

    // Empty store → empty result (goal seeding is dev-2's responsibility)
    let data = execute!(schema, "{ goals { id } }");
    let _ = data["goals"].as_array().unwrap();
}

#[tokio::test]
async fn test_single_goal_query_with_metrics() {
    // AC-10.1: Goal detail with metrics
    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = common::create_test_context(&tmp).await;
    let schema = common::create_test_schema(ctx);

    // Nonexistent goal → null
    let data = execute!(
        schema,
        r#"{ goal(id: "00000000-0000-0000-0000-000000000000") { id } }"#
    );
    assert!(data["goal"].is_null());
}

// ═══════════════════════════════════════════════════════════════════
// Plan Queries (stubs — dev-2 implements plan store)
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_plans_query_returns_all() {
    // AC-13.1: Plan list with status badges
    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = common::create_test_context(&tmp).await;
    let schema = common::create_test_schema(ctx);

    let data = execute!(schema, "{ plans { id } }");
    let _ = data["plans"].as_array().unwrap();
}

#[tokio::test]
async fn test_plans_query_with_status_filter() {
    // AC-13.1: Filter plans by status
    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = common::create_test_context(&tmp).await;
    let schema = common::create_test_schema(ctx);

    let data = execute!(schema, r#"{ plans(filter: { status: EXECUTING }) { id } }"#);
    let _ = data["plans"].as_array().unwrap();
}

#[tokio::test]
async fn test_single_plan_query_with_steps() {
    // AC-13.2: Plan detail with steps
    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = common::create_test_context(&tmp).await;
    let schema = common::create_test_schema(ctx);

    let data = execute!(
        schema,
        r#"{ plan(id: "00000000-0000-0000-0000-000000000000") { id } }"#
    );
    assert!(data["plan"].is_null());
}

// ═══════════════════════════════════════════════════════════════════
// System Queries
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_system_status_query() {
    // AC-18.1: System status cards
    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = common::create_test_context(&tmp).await;
    let schema = common::create_test_schema(ctx);

    let data = execute!(
        schema,
        "{ systemStatus { agentRunning version uptimeSecs } }"
    );
    assert!(!data["systemStatus"]["version"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn test_config_query() {
    // AC-16.1: Settings tabs render
    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = common::create_test_context(&tmp).await;
    let schema = common::create_test_schema(ctx);

    let data = execute!(
        schema,
        "{ config { enrichmentEnabled searchEnabled rrfK embeddingModel } }"
    );
    let _ = &data["config"];
}

#[tokio::test]
async fn test_sessions_query() {
    // AC-15.4: Session management
    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = common::create_test_context(&tmp).await;
    let schema = common::create_test_schema(ctx);

    let data = execute!(schema, "{ sessions { id } }");
    let _ = data["sessions"].as_array().unwrap();
}

#[tokio::test]
async fn test_cron_jobs_query() {
    // AC-18.1: System status
    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = common::create_test_context(&tmp).await;
    let schema = common::create_test_schema(ctx);

    let data = execute!(schema, "{ cronJobs { name } }");
    let _ = data["cronJobs"].as_array().unwrap();
}
