//! Integration tests for the TodoFeature package.
//! Tests that don't require a database.

use feature_todo::{
    config::{CreationMode, TodoConfig},
    TodoFeature,
};

#[test]
fn test_migration_sql_contains_todos_table() {
    let sql = TodoFeature::migration_sql();
    assert!(
        sql.contains("CREATE TABLE IF NOT EXISTS todos"),
        "Migration SQL should create todos table"
    );
    assert!(
        sql.contains("CREATE TABLE IF NOT EXISTS todo_attachments"),
        "Migration SQL should create todo_attachments table"
    );
    assert!(
        sql.contains("CREATE TABLE IF NOT EXISTS todo_time_entries"),
        "Migration SQL should create todo_time_entries table"
    );
    assert!(
        sql.contains("CREATE TABLE IF NOT EXISTS todo_dependencies"),
        "Migration SQL should create todo_dependencies table"
    );
}

#[test]
fn test_migrations_list_is_non_empty() {
    // We need a real TodoRepo to construct a TodoFeature, but we can test
    // the migration SQL directly since migrations() just wraps the SQL.
    let sql = TodoFeature::migration_sql();
    assert!(!sql.is_empty(), "Migration SQL should not be empty");
}

#[test]
fn test_default_config_valid() {
    let cfg = TodoConfig::default();
    assert_eq!(cfg.max_focus_slots, 3);
    assert_eq!(cfg.focus_deadline_hours, 8);
    assert_eq!(cfg.timezone, "UTC");
    assert!(cfg.enrichment.enabled);
    assert!((cfg.enrichment.auto_apply_threshold - 0.70).abs() < f64::EPSILON);
    assert!(cfg.search.enabled);
    assert!((cfg.search.semantic_threshold - 0.5).abs() < f64::EPSILON);
    assert_eq!(cfg.search.rrf_k, 60);
    assert_eq!(cfg.creation_mode, CreationMode::AskFirst);
}

#[test]
fn test_config_serde_round_trip() {
    let cfg = TodoConfig::default();
    let json = serde_json::to_string(&cfg).unwrap();
    let parsed: TodoConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.max_focus_slots, cfg.max_focus_slots);
    assert_eq!(parsed.timezone, cfg.timezone);
    assert_eq!(parsed.creation_mode, cfg.creation_mode);
}

#[test]
fn test_creation_mode_values() {
    let json_ask = r#""ask-first""#;
    let mode: CreationMode = serde_json::from_str(json_ask).unwrap();
    assert_eq!(mode, CreationMode::AskFirst);

    let json_yolo = r#""yolo""#;
    let mode2: CreationMode = serde_json::from_str(json_yolo).unwrap();
    assert_eq!(mode2, CreationMode::Yolo);

    let json_party = r#""party""#;
    let mode3: CreationMode = serde_json::from_str(json_party).unwrap();
    assert_eq!(mode3, CreationMode::Party);
}

#[test]
fn test_rrule_validate_daily() {
    assert!(feature_todo::validate_rrule("FREQ=DAILY").is_ok());
}

#[test]
fn test_rrule_validate_weekly() {
    assert!(feature_todo::validate_rrule("FREQ=WEEKLY;BYDAY=MO,WE,FR").is_ok());
}

#[test]
fn test_rrule_validate_rejects_bysetpos() {
    assert!(feature_todo::validate_rrule("FREQ=MONTHLY;BYDAY=MO;BYSETPOS=1").is_err());
}

#[test]
fn test_rrule_humanize() {
    assert_eq!(feature_todo::humanize_rrule("FREQ=DAILY"), "Every day");
    assert_eq!(feature_todo::humanize_rrule("FREQ=WEEKLY;INTERVAL=2"), "Every 2 weeks");
}

#[test]
fn test_should_spawn_instance() {
    use chrono::Utc;
    let now = Utc::now();
    let past = now - chrono::Duration::hours(1);
    let future = now + chrono::Duration::hours(1);

    assert!(feature_todo::should_spawn_instance(Some(past), now));
    assert!(!feature_todo::should_spawn_instance(Some(future), now));
    assert!(!feature_todo::should_spawn_instance(None, now));
}

#[test]
fn test_search_hybrid_merge_empty() {
    let todos: Vec<feature_todo::types::Todo> = vec![];
    let semantic: Vec<(String, f64)> = vec![];
    let merged = feature_todo::search::hybrid_merge(&todos, &semantic, 60);
    assert!(merged.is_empty());
}
