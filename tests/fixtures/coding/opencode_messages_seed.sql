CREATE TABLE IF NOT EXISTS messages (
    id INTEGER PRIMARY KEY,
    session_id TEXT NOT NULL,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    tool_calls TEXT,
    tool_call_id TEXT,
    metadata TEXT,
    created_at TEXT NOT NULL
);

INSERT INTO messages (session_id, role, content, created_at) VALUES
('opencode-s1', 'user', 'fix the auth bug', datetime('now', '-5 minutes')),
('opencode-s1', 'assistant', 'I will fix the auth bug by adding proper error handling.', datetime('now', '-4 minutes')),
('opencode-s1', 'tool', '{"ok": true, "file": "src/auth.rs"}', datetime('now', '-3 minutes')),
('opencode-s1', 'assistant', 'The auth bug has been fixed. All tests pass.', datetime('now', '-2 minutes'));
