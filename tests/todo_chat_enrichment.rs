//! Chat enrichment flow validation tests
//!
//! These tests validate that the todo tool provides correct enrichment guidance
//! in chat scenarios (Phase 1/2 implementation).

use klyntbot::tools::todo::TodoTool;
use klyntbot::tools::todo_store::TodoStore;
use klyntbot::tools::{RoutingContext, Tool};
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::RwLock;

/// Helper to create a test TodoTool
async fn create_test_tool() -> (TodoTool, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("todos.jsonl");
    let store = Arc::new(RwLock::new(TodoStore::new(file_path)));
    let tool = TodoTool::new(
        store, 3,   // max_focus_slots
        18,  // focus_deadline_hours
        0.5, // confidence_threshold
    );
    (tool, temp_dir)
}

fn ctx() -> RoutingContext {
    RoutingContext::new(
        klyntbot::common::ChannelName::new("telegram"),
        klyntbot::common::ChatId::new("test-chat"),
    )
}

#[tokio::test]
async fn test_low_confidence_triggers_enrichment_options() {
    // SCENARIO: User creates a task with low confidence (< 50%)
    // EXPECTED: Tool output includes AI INSTRUCTION with enrichment options

    let (tool, _dir) = create_test_tool().await;

    // Create a minimal task (title only, 2 words = 0% confidence)
    let args = serde_json::json!({
        "action": "add",
        "title": "fix bug"
    });

    let result = tool.execute(args, &ctx()).await.unwrap();

    // Verify task was created
    assert!(
        result.contains("Task created"),
        "Should confirm task creation"
    );
    assert!(result.contains("fix bug"), "Should include task title");

    // Verify confidence score shown
    assert!(
        result.contains("confidence: 0%") || result.contains("confidence: 0.0"),
        "Should show 0% confidence for minimal task"
    );

    // Verify new ask_user integration guidance is provided
    assert!(
        result.contains("[Low confidence task]"),
        "Should indicate low confidence task"
    );
    assert!(
        result.contains("ask_user tool"),
        "Should suggest using ask_user tool"
    );
    assert!(
        result.contains("Suggest specific questions"),
        "Should guide LLM to ask specific questions"
    );

    // Verify confidence breakdown is shown
    assert!(
        result.contains("Confidence breakdown:"),
        "Should show confidence breakdown"
    );
    // Verify breakdown shows unchecked fields
    assert!(
        result.contains("⬜"),
        "Should show unchecked items for missing fields"
    );

    println!("\n=== Low Confidence Task Output ===\n{}\n", result);
}

#[tokio::test]
async fn test_medium_confidence_shows_suggestion() {
    // SCENARIO: User creates task with medium confidence (50-80%)
    // EXPECTED: Task created, shows confidence breakdown, suggests improvements

    let (tool, _dir) = create_test_tool().await;

    // Create a partial task (title + priority + tags = 55% confidence)
    let args = serde_json::json!({
        "action": "add",
        "title": "fix the authentication bug",  // 4 words = 25%
        "priority": 4,                           // +15%
        "tags": ["backend"]                      // +15%
        // Total = 55%
    });

    let result = tool.execute(args, &ctx()).await.unwrap();

    // Verify task created
    assert!(result.contains("Task created"), "Should confirm creation");

    // Verify confidence is shown as ~55%
    assert!(
        result.contains("confidence: 5") || result.contains("55%"),
        "Should show ~55% confidence: {}",
        result
    );

    // Verify confidence breakdown shown
    assert!(
        result.contains("Confidence breakdown:"),
        "Should show breakdown for medium confidence"
    );

    // Verify improvement suggestion (not full AI INSTRUCTION)
    assert!(
        result.contains("💡") || result.contains("This task could be improved"),
        "Should suggest improvements for medium confidence"
    );

    println!("\n=== Medium Confidence Task Output ===\n{}\n", result);
}

#[tokio::test]
async fn test_high_confidence_no_enrichment_prompt() {
    // SCENARIO: User creates task with high confidence (>= 80%)
    // EXPECTED: Task created, no enrichment prompts

    let (tool, _dir) = create_test_tool().await;

    // Create a complete task (all fields = 100%)
    let args = serde_json::json!({
        "action": "add",
        "title": "fix the authentication timeout bug",  // 5 words = 25%
        "description": "Auth tokens expire during long sessions",  // >10 chars = 25%
        "priority": 4,                                  // +15%
        "due_date": "2026-02-20T12:00:00Z",            // +20%
        "tags": ["backend", "urgent"]                   // +15%
        // Total = 100%
    });

    let result = tool.execute(args, &ctx()).await.unwrap();

    // Verify task created
    assert!(result.contains("Task created"), "Should confirm creation");

    // Verify high confidence shown
    assert!(
        result.contains("confidence: 100%") || result.contains("confidence: 1"),
        "Should show 100% confidence: {}",
        result
    );

    // Verify NO enrichment prompts for high confidence
    assert!(
        !result.contains("🤖 AI INSTRUCTION"),
        "Should NOT show AI INSTRUCTION for high confidence task"
    );
    assert!(
        !result.contains("This task has low confidence"),
        "Should NOT prompt enrichment for high confidence"
    );

    println!("\n=== High Confidence Task Output ===\n{}\n", result);
}

#[tokio::test]
async fn test_confidence_breakdown_format() {
    // SCENARIO: Verify the confidence breakdown shows correct field status
    // EXPECTED: Checkboxes show which fields are set/missing

    let (tool, _dir) = create_test_tool().await;

    // Create task with only description and priority
    let args = serde_json::json!({
        "action": "add",
        "title": "task",  // 1 word, doesn't count
        "description": "This is a longer description",  // Counts
        "priority": 3  // Counts
        // No due date, no tags
    });

    let result = tool.execute(args, &ctx()).await.unwrap();

    // Should show confidence breakdown
    assert!(result.contains("Confidence breakdown:"));

    // Verify field-by-field breakdown with checkboxes or similar indicators
    // The format uses [x] for present, [ ] for missing (from todo.rs:476-539)

    // Title should be unchecked (only 1 word, needs >3)
    assert!(
        result.contains("[ ] Title quality")
            || result.contains("Title quality") && result.contains("needs >3 words"),
        "Title should show as missing/incomplete"
    );

    // Description should be checked (>10 chars)
    assert!(
        result.contains("[x] Description")
            || result.contains("Description") && result.contains("set"),
        "Description should show as present"
    );

    // Priority should be checked
    assert!(
        result.contains("[x] Priority") || result.contains("Priority") && result.contains("P3"),
        "Priority should show as set"
    );

    // Due date should be unchecked
    assert!(
        result.contains("[ ] Due date")
            || result.contains("Due date") && result.contains("not set"),
        "Due date should show as missing"
    );

    // Tags should be unchecked
    assert!(
        result.contains("[ ] Tags") || result.contains("Tags") && result.contains("none"),
        "Tags should show as missing"
    );

    println!("\n=== Confidence Breakdown Format ===\n{}\n", result);
}

#[tokio::test]
async fn test_update_shows_confidence_progress() {
    // SCENARIO: User updates a task to add missing fields
    // EXPECTED: Update output shows new confidence and remaining gaps

    let (tool, _dir) = create_test_tool().await;

    // Create low confidence task
    let add_result = tool
        .execute(
            serde_json::json!({
                "action": "add",
                "title": "fix bug"
            }),
            &ctx(),
        )
        .await
        .unwrap();

    // Extract task ID
    let id_start = add_result.find("ID: ").unwrap() + 4;
    let id_end = add_result[id_start..].find(',').unwrap() + id_start;
    let task_id = &add_result[id_start..id_end];

    // Update to add description and priority
    let update_result = tool
        .execute(
            serde_json::json!({
                "action": "update",
                "id": task_id,
                "description": "Auth tokens expire too fast",
                "priority": 4
            }),
            &ctx(),
        )
        .await
        .unwrap();

    // Should show updated confidence
    assert!(
        update_result.contains("confidence:") && update_result.contains("%"),
        "Update should show new confidence percentage"
    );

    // Should show what's still missing
    assert!(
        update_result.contains("Still missing:") || update_result.contains("💡"),
        "Update should indicate remaining improvements"
    );

    println!("\n=== Update Confidence Progress ===\n{}\n", update_result);
}

#[tokio::test]
async fn test_show_action_includes_improvement_suggestions() {
    // SCENARIO: User shows a low-confidence task
    // EXPECTED: Show output includes improvement suggestions

    let (tool, _dir) = create_test_tool().await;

    // Create minimal task
    let add_result = tool
        .execute(
            serde_json::json!({
                "action": "add",
                "title": "test task"
            }),
            &ctx(),
        )
        .await
        .unwrap();

    let id_start = add_result.find("ID: ").unwrap() + 4;
    let id_end = add_result[id_start..].find(',').unwrap() + id_start;
    let task_id = &add_result[id_start..id_end];

    // Show the task
    let show_result = tool
        .execute(
            serde_json::json!({
                "action": "show",
                "id": task_id
            }),
            &ctx(),
        )
        .await
        .unwrap();

    // Should include confidence breakdown
    assert!(
        show_result.contains("Confidence breakdown:"),
        "Show should include confidence breakdown"
    );

    // Should suggest improvements for low confidence
    assert!(
        show_result.contains("💡 Improvement suggestions:"),
        "Show should suggest improvements for low confidence task"
    );

    // Should mention YOLO or Party mode
    assert!(
        show_result.contains("🤖 AI INSTRUCTION")
            && (show_result.contains("YOLO") || show_result.contains("Party")),
        "Show should suggest enrichment modes"
    );

    println!(
        "\n=== Show with Improvement Suggestions ===\n{}\n",
        show_result
    );
}

#[tokio::test]
async fn test_confidence_threshold_boundary() {
    // SCENARIO: Test tasks at exactly 50% confidence threshold
    // EXPECTED: < 50% triggers full enrichment, >= 50% shows lighter suggestion

    let (tool, _dir) = create_test_tool().await;

    // Task at exactly 50% (title 25% + description 25%)
    let args_50 = serde_json::json!({
        "action": "add",
        "title": "fix the auth bug",  // 4 words = 25%
        "description": "tokens expire too fast"  // >10 chars = 25%
    });

    let result_50 = tool.execute(args_50, &ctx()).await.unwrap();

    // At 50%, should NOT trigger full AI INSTRUCTION (threshold is < 0.5)
    assert!(
        !result_50.contains("🤖 AI INSTRUCTION") || result_50.contains("could be improved"),
        "50% confidence should not trigger full enrichment prompt"
    );

    // Task at 45% (title 25% + priority 15% + tags 15% - title quality = 30%)
    let args_45 = serde_json::json!({
        "action": "add",
        "title": "fix",  // 1 word = 0%
        "priority": 4,   // +15%
        "tags": ["backend"]  // +15%
        // Total = 30%
    });

    let result_45 = tool.execute(args_45, &ctx()).await.unwrap();

    // Below 50% should trigger ask_user guidance
    assert!(
        result_45.contains("[Low confidence task]"),
        "30% confidence should trigger ask_user guidance"
    );

    println!("\n=== 50% Threshold Result ===\n{}\n", result_50);
    println!("\n=== 30% Below Threshold Result ===\n{}\n", result_45);
}

#[tokio::test]
async fn test_missing_fields_list_accuracy() {
    // SCENARIO: Verify missing fields are correctly identified
    // EXPECTED: Only actually missing fields appear in the list

    let (tool, _dir) = create_test_tool().await;

    // Task with only tags
    let args = serde_json::json!({
        "action": "add",
        "title": "bug",  // Too short
        "tags": ["urgent"]
    });

    let result = tool.execute(args, &ctx()).await.unwrap();

    // Should include confidence breakdown
    assert!(
        result.contains("Confidence breakdown:"),
        "Should include confidence breakdown"
    );

    // Should mention title quality (1 word, needs >3)
    assert!(
        result.contains("Title quality") && result.contains("needs >3 words"),
        "Should flag short title: {}",
        result
    );

    // Should mention description not set
    assert!(
        result.contains("Description") && result.contains("not set"),
        "Should show description not set: {}",
        result
    );

    // Should mention priority not set
    assert!(
        result.contains("Priority") && result.contains("not set"),
        "Should show priority not set: {}",
        result
    );

    // Should mention due date not set
    assert!(
        result.contains("Due date") && result.contains("not set"),
        "Should show due date not set: {}",
        result
    );

    // Tags should be marked as set (with checkmark)
    assert!(
        result.contains("Tags") && (result.contains("✅ Tags") || !result.contains("⬜ Tags")),
        "Tags should be marked as present: {}",
        result
    );

    println!("\n=== Missing Fields List ===\n{}\n", result);
}
