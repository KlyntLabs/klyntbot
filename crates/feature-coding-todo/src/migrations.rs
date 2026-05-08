//! FeatureMigration for the coding_todos table.

use tools_core::FeatureMigration;

pub fn coding_todo_migration() -> FeatureMigration {
    FeatureMigration {
        feature_name: "feature_coding_todo".into(),
        version: 1,
        description: "Create coding_todos table".into(),
        sql: r#"
        CREATE TABLE IF NOT EXISTS coding_todos (
            thread_id  TEXT NOT NULL,
            agent_id   TEXT NOT NULL,
            items_json TEXT NOT NULL DEFAULT '[]',
            proposed_in_plan_session TEXT,
            updated_at TEXT NOT NULL,
            PRIMARY KEY (thread_id, agent_id)
        );

        CREATE INDEX IF NOT EXISTS idx_coding_todos_thread
            ON coding_todos(thread_id);
        "#
        .into(),
    }
}
