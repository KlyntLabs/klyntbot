-- Fix foreign key mismatch on coding_reviews.
-- The original v1 migration incorrectly referenced sessions(id); sessions
-- uses `key` as its primary key. SQLite deferred the error until any write
-- to the parent table (sessions), which broke chat_send for new sessions.
--
-- Strategy: recreate the table with the correct FK, copy data, swap.

CREATE TABLE IF NOT EXISTS coding_reviews_new (
    id            TEXT PRIMARY KEY,
    session_id    TEXT NOT NULL,
    summary       TEXT NOT NULL,
    issues_json   TEXT NOT NULL,
    target        TEXT,
    delivery      TEXT,
    created_at    TEXT NOT NULL,
    FOREIGN KEY(session_id) REFERENCES sessions(key) ON DELETE CASCADE
);

INSERT INTO coding_reviews_new SELECT * FROM coding_reviews;

DROP TABLE coding_reviews;

ALTER TABLE coding_reviews_new RENAME TO coding_reviews;

CREATE INDEX IF NOT EXISTS idx_coding_reviews_session
  ON coding_reviews(session_id, created_at DESC);
