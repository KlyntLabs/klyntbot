use tools_core::FeatureMigration;

pub fn coding_background_jobs_migration() -> FeatureMigration {
    FeatureMigration {
        feature_name: "feature_coding_bash".into(),
        version: 1,
        description: "Create coding_background_jobs table".into(),
        sql: r#"
            CREATE TABLE IF NOT EXISTS coding_background_jobs (
                id                    TEXT PRIMARY KEY,
                session_id            TEXT NOT NULL,
                agent_id              TEXT NOT NULL,
                description           TEXT NOT NULL,
                command               TEXT NOT NULL,
                cwd                   TEXT NOT NULL,
                timeout_ms            INTEGER NOT NULL,
                silent_completion     INTEGER NOT NULL DEFAULT 0,
                status                TEXT NOT NULL,
                exit_code             INTEGER,
                failure_kind          TEXT,
                failure_detail        TEXT,
                failure_extracted     TEXT,
                started_at            TEXT NOT NULL,
                finished_at           TEXT,
                total_bytes_emitted   INTEGER NOT NULL DEFAULT 0,
                bisect_count          INTEGER NOT NULL DEFAULT 0,
                log_path              TEXT NOT NULL,
                final_path            TEXT,
                last_polled_at        TEXT,
                last_seen_offset      INTEGER NOT NULL DEFAULT 0,
                CHECK (status IN ('Starting','Running','Completed','Failed','Cancelled','Lost')),
                CHECK (failure_kind IS NULL OR status IN ('Failed','Cancelled','Lost'))
            );
            CREATE INDEX IF NOT EXISTS idx_cbj_session_status ON coding_background_jobs(session_id, status);
            CREATE INDEX IF NOT EXISTS idx_cbj_active ON coding_background_jobs(status) WHERE status IN ('Starting','Running');
        "#
        .into(),
    }
}
