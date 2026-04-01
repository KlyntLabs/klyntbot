-- Phoneme mastery tracking (FSRS-5 parameters per phoneme per language).
CREATE TABLE IF NOT EXISTS phoneme_mastery (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    phoneme TEXT NOT NULL,
    language TEXT NOT NULL,
    stability REAL NOT NULL DEFAULT 0.0,
    difficulty REAL NOT NULL DEFAULT 0.0,
    encounters INTEGER NOT NULL DEFAULT 0,
    recent_errors INTEGER NOT NULL DEFAULT 0,
    last_reviewed_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(phoneme, language)
);

-- Per-turn pronunciation logs.
CREATE TABLE IF NOT EXISTS pronunciation_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    transcript TEXT NOT NULL,
    overall_score REAL NOT NULL,
    phoneme_scores_json TEXT NOT NULL DEFAULT '[]',
    tone_scores_json TEXT NOT NULL DEFAULT '[]',
    feedback_level TEXT NOT NULL DEFAULT 'summary',
    language TEXT NOT NULL,
    audio_duration_ms INTEGER,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Exam attempt tracking (IELTS, HSK, etc.).
CREATE TABLE IF NOT EXISTS exam_attempts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    exam_type TEXT NOT NULL,
    section TEXT,
    score REAL,
    max_score REAL,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
