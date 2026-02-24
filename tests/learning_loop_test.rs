//! Integration tests for the learning loop features:
//! - Strategy record persistence + satisfaction backfill
//! - Goal plan-completion metrics (typed columns)

#[tokio::test]
async fn test_strategy_record_roundtrip_with_satisfaction() {
    // 1. Create in-memory storage pool
    // 2. Create a StrategyRecordRow with chat_id
    // 3. Write it via StrategyRepo.create()
    // 4. Call set_satisfaction_for_chat() to backfill satisfaction
    // 5. Verify the record has the satisfaction score
    // 6. Verify chat_id is correct

    let pool = klyntbot::StoragePool::connect_in_memory().await.unwrap();
    let repos = klyntbot::Repos::from_pool(&pool);

    let record = klyntbot::storage::StrategyRecordRow {
        id: uuid::Uuid::new_v4(),
        timestamp: chrono::Utc::now(),
        request_id: "integration-test-1".to_string(),
        predicted_strategy: "DirectResponse".to_string(),
        actual_strategy: "ToolAssisted".to_string(),
        escalation_count: 1,
        iterations_used: 3,
        max_iterations: 5,
        success: true,
        user_satisfaction: None,
        response_time_ms: 1500,
        chat_id: Some("tg:user123".to_string()),
        tool_name: None,
        tool_success: None,
        tool_duration_ms: None,
    };
    repos.strategies.create(&record).await.unwrap();

    // Backfill satisfaction (simulating a thumbs-up reaction)
    let since = chrono::Utc::now() - chrono::Duration::minutes(5);
    let updated = repos
        .strategies
        .set_satisfaction_for_chat("tg:user123", since, 1.0)
        .await
        .unwrap();
    assert!(updated, "Expected satisfaction to be set");

    // Verify
    let fetched = repos.strategies.get(record.id).await.unwrap();
    assert_eq!(fetched.user_satisfaction, Some(1.0));
    assert_eq!(fetched.chat_id, Some("tg:user123".to_string()));
    assert_eq!(fetched.predicted_strategy, "DirectResponse");
    assert_eq!(fetched.actual_strategy, "ToolAssisted");
    assert_eq!(fetched.iterations_used, 3);
}

#[tokio::test]
async fn test_goal_plan_completion_metrics_end_to_end() {
    // 1. Create a goal with typed columns (all zeros)
    // 2. Simulate completed plans via increment_completed()
    // 3. Simulate a failed plan via increment_failed()
    // 4. Verify all metrics are correct

    let pool = klyntbot::StoragePool::connect_in_memory().await.unwrap();
    let repos = klyntbot::Repos::from_pool(&pool);

    let goal_id = uuid::Uuid::new_v4();
    let row = klyntbot::storage::GoalRow {
        id: goal_id,
        title: "Ship v1.0".to_string(),
        description: "Release the first version".to_string(),
        status: "Active".to_string(),
        priority: 1,
        target_date: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        plans_completed: 0,
        plans_failed: 0,
        avg_duration_ms: None,
        last_plan_at: None,
    };
    repos.goals.create(&row).await.unwrap();

    // Simulate 2 completed plans
    repos
        .goals
        .increment_completed(goal_id, 60_000)
        .await
        .unwrap(); // 1 minute
    repos
        .goals
        .increment_completed(goal_id, 120_000)
        .await
        .unwrap(); // 2 minutes

    // Simulate 1 failed plan
    repos.goals.increment_failed(goal_id).await.unwrap();

    // Verify metrics
    let g = repos.goals.get(goal_id).await.unwrap();
    assert_eq!(g.plans_completed, 2);
    assert_eq!(g.plans_failed, 1);
    assert_eq!(g.avg_duration_ms, Some(90_000)); // (60k + 120k) / 2
    assert!(g.last_plan_at.is_some(), "Expected last_plan_at to be set");
    assert_eq!(g.title, "Ship v1.0");
}

#[tokio::test]
async fn test_satisfaction_no_match_returns_false() {
    let pool = klyntbot::StoragePool::connect_in_memory().await.unwrap();
    let repos = klyntbot::Repos::from_pool(&pool);

    let since = chrono::Utc::now() - chrono::Duration::minutes(5);
    let updated = repos
        .strategies
        .set_satisfaction_for_chat("nonexistent:chat", since, 0.0)
        .await
        .unwrap();
    assert!(!updated, "Expected no record to be updated");
}
