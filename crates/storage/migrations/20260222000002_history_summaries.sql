CREATE TABLE IF NOT EXISTS history_summaries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_key TEXT NOT NULL,
    range_start INT NOT NULL,
    range_end INT NOT NULL,
    summary_text TEXT NOT NULL,
    model TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(session_key, range_start, range_end)
);
