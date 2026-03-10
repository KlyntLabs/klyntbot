-- Calendar events synced from external providers (Google Calendar, Outlook, etc.)
CREATE TABLE IF NOT EXISTS calendar_events (
    id TEXT PRIMARY KEY,
    calendar_id TEXT NOT NULL DEFAULT 'primary',
    title TEXT NOT NULL,
    description TEXT,
    started_at TEXT NOT NULL,
    ended_at TEXT NOT NULL,
    location TEXT,
    attendees_count INTEGER DEFAULT 0,
    is_recurring INTEGER DEFAULT 0,
    recurrence_id TEXT,
    source TEXT NOT NULL DEFAULT 'google',
    external_uid TEXT NOT NULL,
    session_id TEXT REFERENCES productivity_sessions(id),
    color TEXT,
    synced_at TEXT NOT NULL DEFAULT (datetime('now')),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(external_uid)
);

CREATE INDEX IF NOT EXISTS idx_calendar_events_time ON calendar_events(started_at, ended_at);
CREATE INDEX IF NOT EXISTS idx_calendar_events_session ON calendar_events(session_id) WHERE session_id IS NOT NULL;
