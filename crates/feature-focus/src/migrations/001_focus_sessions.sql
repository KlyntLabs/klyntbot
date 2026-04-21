CREATE TABLE IF NOT EXISTS focus_sessions (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    mode        TEXT NOT NULL,
    started_at  TEXT NOT NULL,          -- RFC 3339
    ends_at     TEXT NOT NULL,
    ended_at    TEXT,                   -- NULL while active
    alarm_id    TEXT,                   -- ID returned by TemporalScheduler
    source      TEXT NOT NULL DEFAULT 'launcher'
);

CREATE UNIQUE INDEX IF NOT EXISTS ix_focus_sessions_active
    ON focus_sessions(mode) WHERE ended_at IS NULL;
