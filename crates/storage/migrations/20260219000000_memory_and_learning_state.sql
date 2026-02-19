-- G-16: Memory notes (replaces filesystem daily notes + MEMORY.md)
-- G-17: Learning state (replaces learning_state.json file)

-- ============================================================
-- Memory Notes
-- ============================================================
-- Stores daily notes (key = 'YYYY-MM-DD') and long-term memory (key = 'LONG_TERM').
-- Replaces workspace/memory/*.md filesystem layout.
CREATE TABLE memory_notes (
    note_key   TEXT PRIMARY KEY,
    content    TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- ============================================================
-- Learning State (key-value store)
-- ============================================================
-- Stores adaptive threshold state and other learning configuration.
-- Replaces ~/.klyntbot/data/learning_state.json.
CREATE TABLE learning_state (
    key        TEXT PRIMARY KEY,
    value      JSONB NOT NULL DEFAULT '{}',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
