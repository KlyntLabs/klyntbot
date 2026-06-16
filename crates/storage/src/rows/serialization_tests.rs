//! Serialization tests for all storage row types.
//!
//! Verifies that every row type exposes `Serialize` + `#[serde(rename_all = "camelCase")]`
//! so the dashboard REST API can return camelCase JSON without any manual mapping.

#[cfg(test)]
mod tests {
    use crate::sqlite_types::{SqlDate, SqlTs};
    use jiff::{civil::Date, Timestamp};
    use uuid::Uuid;

    use crate::repos::project_repo::ProjectWithStats;
    use crate::repos::strategy::{OverallStats, ToolStatsRow};
    use crate::repos::TaskSummary;
    use crate::rows::area::AreaRow;

    use crate::rows::cron::CronJobRow;
    use crate::rows::key_result::KeyResultRow;
    use crate::rows::learning::{
        DecisionLogRow, LearningStateRow, OutcomeRow, StrategyRecordRow, StrategySummaryRow,
    };
    use crate::rows::objective::ObjectiveRow;
    use crate::rows::project::ProjectRow;
    use crate::rows::session::{SessionListRow, SessionMessageRow, SessionRow};
    use crate::rows::usage::UsageRecordRow;

    // ── Helpers ──────────────────────────────────────────────────────────────

    /// Assert that every key in a `serde_json::Value::Object` is camelCase.
    /// Fails the test if any key contains an underscore (snake_case).
    fn assert_no_snake_case_keys(value: &serde_json::Value, context: &str) {
        if let serde_json::Value::Object(map) = value {
            for key in map.keys() {
                assert!(
                    !key.contains('_'),
                    "snake_case key `{}` found in {} — expected camelCase",
                    key,
                    context
                );
                assert_no_snake_case_keys(&map[key], context);
            }
        }
    }

    fn sample_project_row() -> ProjectRow {
        ProjectRow {
            id: "proj-1".to_string(),
            area_id: "area-1".to_string(),
            name: "Test project".to_string(),
            description: None,
            color: "#ff0000".to_string(),
            tags: vec![],
            status: "active".to_string(),
            created_at: SqlTs::from(Timestamp::now()),
            updated_at: SqlTs::from(Timestamp::now()),
            workflow_id: None,
            instructions: None,
            ai_personality: None,
            user_role: None,
            start_date: None,
            target_end_date: None,
            settings: None,
        }
    }

    // ── Comprehensive camelCase check ────────────────────────────────────────

    /// Verifies every row type serializes to camelCase JSON (no underscored keys).
    #[test]
    fn all_row_types_serialize_to_camel_case() {
        let now: SqlTs = Timestamp::now().into();
        let _today: SqlDate = Date::new(2026, 1, 1).unwrap().into();

        let rows: Vec<(&str, serde_json::Value)> = vec![
            (
                "AreaRow",
                serde_json::to_value(&AreaRow {
                    id: "area-1".to_string(),
                    name: "Work".to_string(),
                    description: None,
                    color: "blue".to_string(),
                    icon: None,
                    position: 0,
                    status: "active".to_string(),
                    created_at: now,
                    updated_at: now,
                })
                .unwrap(),
            ),
            (
                "ProjectRow",
                serde_json::to_value(sample_project_row()).unwrap(),
            ),
            (
                "ObjectiveRow",
                serde_json::to_value(&ObjectiveRow {
                    id: "obj-1".to_string(),
                    project_id: "proj-1".to_string(),
                    title: "Test".to_string(),
                    description: None,
                    status: "active".to_string(),
                    priority: None,
                    due_date: None,
                    progress: 0.0,
                    created_at: now,
                    updated_at: now,
                    completed_at: None,
                })
                .unwrap(),
            ),
            (
                "KeyResultRow",
                serde_json::to_value(&KeyResultRow {
                    id: "kr-1".to_string(),
                    objective_id: "obj-1".to_string(),
                    title: "Test".to_string(),
                    description: None,
                    status: "active".to_string(),
                    tracking_mode: "action".to_string(),
                    target_value: None,
                    current_value: 0.0,
                    unit: None,
                    progress: 0.0,
                    due_date: None,
                    created_at: now,
                    updated_at: now,
                    completed_at: None,
                })
                .unwrap(),
            ),
            (
                "SessionRow",
                serde_json::to_value(&SessionRow {
                    key: "s".to_string(),
                    mode: "assistant".to_string(),
                    metadata: serde_json::json!({}),
                    created_at: now,
                    updated_at: now,
                    last_event_at: None,
                    project_id: None,
                    conversation_type: None,
                    pinned: false,
                    approval_mode: "default".to_string(),
                    total_cost_usd: 0.0,
                    total_tokens: 0,
                    parent_session_id: None,
                    workspace_id: None,
                    forked_from_id: None,
                    summary_message_id: None,
                    ephemeral: 0,
                    archived_at: None,
                })
                .unwrap(),
            ),
            (
                "SessionMessageRow",
                serde_json::to_value(&SessionMessageRow {
                    id: Uuid::new_v4(),
                    session_key: "s".to_string(),
                    role: "user".to_string(),
                    content: "hi".to_string(),
                    parts: None,
                    turn_id: None,
                    finish_reason: None,
                    timestamp: now,
                    request_id: None,
                    tool_calls: None,
                    metadata: None,
                })
                .unwrap(),
            ),
            (
                "SessionListRow",
                serde_json::to_value(&SessionListRow {
                    key: "s".to_string(),
                    metadata: serde_json::json!({}),
                    created_at: now,
                    updated_at: now,
                    message_count: 0,
                    project_id: None,
                    conversation_type: None,
                    pinned: false,
                })
                .unwrap(),
            ),
            (
                "CronJobRow",
                serde_json::to_value(&CronJobRow {
                    id: "c".to_string(),
                    name: "n".to_string(),
                    enabled: true,
                    origin: "system".to_string(),
                    schedule: serde_json::json!({}),
                    payload: serde_json::json!({}),
                    next_run_at_ms: None,
                    last_run_at_ms: None,
                    last_status: None,
                    last_error: None,
                    created_at_ms: 0,
                    updated_at_ms: 0,
                    delete_after_run: false,
                    intent_window: None,
                    intent_pending_since_ms: None,
                })
                .unwrap(),
            ),
            (
                "UsageRecordRow",
                serde_json::to_value(&UsageRecordRow {
                    id: Uuid::new_v4(),
                    timestamp: now,
                    request_id: "r".to_string(),
                    model: "m".to_string(),
                    provider: "p".to_string(),
                    prompt_tokens: 0,
                    completion_tokens: 0,
                    cache_read_tokens: 0,
                    cache_write_tokens: 0,
                    estimated_cost_usd: 0.0,
                    channel: "cli".to_string(),
                    strategy: "direct".to_string(),
                })
                .unwrap(),
            ),
            (
                "OutcomeRow",
                serde_json::to_value(&OutcomeRow {
                    id: "o".to_string(),
                    session_key: "s".to_string(),
                    tool_name: "todo_add".to_string(),
                    success: true,
                    error_category: None,
                    duration_ms: 100,
                    confidence_score: None,
                    confidence_dimensions: None,
                    created_at: now,
                })
                .unwrap(),
            ),
            (
                "StrategyRecordRow",
                serde_json::to_value(&StrategyRecordRow {
                    id: Uuid::new_v4(),
                    timestamp: now,
                    request_id: "r".to_string(),
                    predicted_strategy: "direct".to_string(),
                    actual_strategy: "direct".to_string(),
                    escalation_count: 0,
                    iterations_used: 1,
                    max_iterations: 5,
                    success: true,
                    user_satisfaction: None,
                    response_time_ms: 500,
                    chat_id: None,
                    tool_name: None,
                    tool_success: None,
                    tool_duration_ms: None,
                    complexity_signals: serde_json::json!({}),
                    execution_mode: None,
                    retrieved_memory_count: None,
                    safety_cap_hit: false,
                    turns_used: 0,
                    loop_detected: false,
                    loop_tools: None,
                    context_fill_pct: None,
                })
                .unwrap(),
            ),
            (
                "LearningStateRow",
                serde_json::to_value(&LearningStateRow {
                    key: "k".to_string(),
                    value: serde_json::json!({}),
                    updated_at: now,
                })
                .unwrap(),
            ),
            (
                "StrategySummaryRow",
                serde_json::to_value(&StrategySummaryRow {
                    predicted_strategy: "direct".to_string(),
                    sample_count: 10,
                    correct_count: 9,
                    avg_escalations: 0.1,
                })
                .unwrap(),
            ),
            (
                "DecisionLogRow",
                serde_json::to_value(&DecisionLogRow {
                    id: "d".to_string(),
                    session_key: "s".to_string(),
                    iteration: 1,
                    tool_names: serde_json::json!([]),
                    user_message_preview: "hi".to_string(),
                    assessment: serde_json::json!({}),
                    outcome: None,
                    created_at: now,
                })
                .unwrap(),
            ),
            (
                "TaskSummary",
                serde_json::to_value(&TaskSummary {
                    todo: 5,
                    doing: 2,
                    done: 10,
                    total: 17,
                })
                .unwrap(),
            ),
            (
                "ProjectWithStats",
                serde_json::to_value(&ProjectWithStats {
                    project: sample_project_row(),
                    task_count_todo: 0,
                    task_count_doing: 0,
                    task_count_done: 0,
                    task_count_total: 0,
                })
                .unwrap(),
            ),
            (
                "OverallStats",
                serde_json::to_value(&OverallStats {
                    total_records: 0,
                    accuracy: 1.0,
                    avg_response_time_ms: 0,
                    avg_satisfaction: None,
                })
                .unwrap(),
            ),
            (
                "ToolStatsRow",
                serde_json::to_value(&ToolStatsRow {
                    tool_name: "todo_add".to_string(),
                    total_calls: 0,
                    success_count: 0,
                    avg_duration_ms: 0,
                })
                .unwrap(),
            ),
        ];

        for (name, value) in &rows {
            assert_no_snake_case_keys(value, name);
        }
    }

    #[test]
    fn scheduled_fire_row_round_trips_payload() {
        use crate::rows::scheduled_fire::ScheduledFireRow;
        let row = ScheduledFireRow {
            id: "fire_abc".into(),
            fire_at_ms: 1_800_000_000_000,
            kind: "cron_job".into(),
            ref_id: Some("job_xyz".into()),
            payload: serde_json::json!({ "message": "hi", "channels": ["tray"] }),
            dedup_prefix: Some("cron:job_xyz:".into()),
            fired: false,
            firing_started_at_ms: None,
            fired_at_ms: None,
            suppressed_by: None,
            created_at_ms: 1_800_000_000_000 - 1000,
        };
        let json = serde_json::to_string(&row).unwrap();
        let parsed: ScheduledFireRow = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, row.id);
        assert_eq!(parsed.payload["message"], "hi");
    }

    #[test]
    fn notification_log_row_round_trips() {
        use crate::rows::notification_log::NotificationLogRow;
        let row = NotificationLogRow {
            alarm_id: "fire_abc".into(),
            channel: "os_native".into(),
            sent_at_ms: 1_800_000_000_000,
            ack_at_ms: None,
            error: None,
        };
        let s = serde_json::to_string(&row).unwrap();
        let parsed: NotificationLogRow = serde_json::from_str(&s).unwrap();
        assert_eq!(parsed.alarm_id, "fire_abc");
        assert_eq!(parsed.channel, "os_native");
    }
}
