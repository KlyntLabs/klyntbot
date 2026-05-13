use tools_core::FeatureMigration;

pub fn coding_background_jobs_migration() -> FeatureMigration {
    FeatureMigration {
        feature_name: "feature_coding_bash".into(),
        version: 3,
        description: "Add tty + attach columns for Phase 2.3c interactive PTY".into(),
        sql: r#"
            DROP TABLE IF EXISTS coding_background_jobs;
            CREATE TABLE coding_background_jobs (
                id                    TEXT PRIMARY KEY,
                session_id            TEXT NOT NULL,
                agent_id              TEXT NOT NULL,
                description           TEXT NOT NULL,
                command               TEXT NOT NULL,
                command_key           TEXT NOT NULL,
                cwd                   TEXT NOT NULL,
                timeout_ms            INTEGER NOT NULL,
                silent_completion     INTEGER NOT NULL DEFAULT 0,

                tty                   INTEGER NOT NULL DEFAULT 0,
                tty_rows              INTEGER,
                tty_cols              INTEGER,
                attached_user_at      TEXT,
                attach_token          TEXT,

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
                CHECK (failure_kind IS NULL OR status IN ('Failed','Cancelled','Lost')),
                CHECK (tty IN (0, 1)),
                CHECK (tty = 0 OR (tty_rows IS NOT NULL AND tty_cols IS NOT NULL)),
                CHECK (attached_user_at IS NULL OR tty = 1),
                CHECK ((attached_user_at IS NULL) = (attach_token IS NULL))
            );
            CREATE INDEX idx_cbj_session_status ON coding_background_jobs(session_id, status);
            CREATE INDEX idx_cbj_active        ON coding_background_jobs(status) WHERE status IN ('Starting','Running');
            CREATE INDEX idx_cbj_session_command_key
                ON coding_background_jobs(session_id, command_key, started_at DESC);
            CREATE INDEX idx_cbj_attached
                ON coding_background_jobs(session_id, attached_user_at)
                WHERE attached_user_at IS NOT NULL;
        "#
        .into(),
    }
}
