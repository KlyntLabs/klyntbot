-- Session tracker feature tables

CREATE TABLE IF NOT EXISTS tracked_sessions (
    session_id TEXT PRIMARY KEY,
    project_path TEXT NOT NULL,
    project_name TEXT NOT NULL,
    jsonl_path TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'idle',
    first_message_preview TEXT,
    message_count INTEGER NOT NULL DEFAULT 0,
    git_branch TEXT,
    last_activity TIMESTAMP,
    file_offset INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_tracked_sessions_project
    ON tracked_sessions(project_name);

CREATE INDEX IF NOT EXISTS idx_tracked_sessions_status
    ON tracked_sessions(status);

CREATE INDEX IF NOT EXISTS idx_tracked_sessions_last_activity
    ON tracked_sessions(last_activity);

CREATE TABLE IF NOT EXISTS pinned_messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL REFERENCES tracked_sessions(session_id) ON DELETE CASCADE,
    message_uuid TEXT NOT NULL,
    message_content TEXT NOT NULL,
    message_role TEXT NOT NULL,
    pin_order INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(session_id, message_uuid)
);

CREATE INDEX IF NOT EXISTS idx_pinned_messages_session
    ON pinned_messages(session_id);

CREATE TABLE IF NOT EXISTS brainstorm_conversations (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES tracked_sessions(session_id) ON DELETE CASCADE,
    title TEXT,
    mode TEXT NOT NULL,
    model_key TEXT,
    agent_profile TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_brainstorm_conversations_session
    ON brainstorm_conversations(session_id);

CREATE TABLE IF NOT EXISTS brainstorm_messages (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES brainstorm_conversations(id) ON DELETE CASCADE,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    is_result_block INTEGER NOT NULL DEFAULT 0,
    edited_content TEXT,
    sent_to_cc INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_brainstorm_messages_conversation
    ON brainstorm_messages(conversation_id);

CREATE TABLE IF NOT EXISTS session_summaries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL REFERENCES tracked_sessions(session_id) ON DELETE CASCADE,
    chunk_start INTEGER NOT NULL,
    chunk_end INTEGER NOT NULL,
    summary TEXT NOT NULL,
    files_touched TEXT,
    key_decisions TEXT,
    rolling_summary TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_session_summaries_session
    ON session_summaries(session_id);
