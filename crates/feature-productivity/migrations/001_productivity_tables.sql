-- Activity categories: user-defined or AI-inferred
CREATE TABLE IF NOT EXISTS activity_categories (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL,
    category_type TEXT NOT NULL DEFAULT 'productive',
    color         TEXT,
    icon          TEXT,
    rules         TEXT,
    is_system     BOOLEAN NOT NULL DEFAULT FALSE,
    created_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Default categories
INSERT OR IGNORE INTO activity_categories (id, name, category_type, rules, is_system) VALUES
    ('coding', 'Coding', 'productive', '{"appNames":["Visual Studio Code","Code","Xcode","IntelliJ IDEA","WebStorm","Terminal","iTerm2","Warp","Alacritty","kitty","Ghostty","Hyper","Rio","WezTerm"],"bundleIds":["com.microsoft.VSCode","com.apple.Terminal","com.googlecode.iterm2","com.mitchellh.ghostty"],"urlPatterns":[]}', TRUE),
    ('communication', 'Communication', 'neutral', '{"appNames":["Slack","Discord","Telegram","WhatsApp","Messages","Microsoft Teams","Zoom","Signal"],"bundleIds":["com.tinyspeck.slackmacgap","com.hnc.Discord","ru.keepcoder.Telegram"],"urlPatterns":[]}', TRUE),
    ('browsing', 'Browsing', 'neutral', '{"appNames":["Safari","Firefox","Google Chrome","Arc","Brave Browser","Orion","Vivaldi"],"bundleIds":["com.apple.Safari","org.mozilla.firefox","com.google.Chrome"],"urlPatterns":[]}', TRUE),
    ('design', 'Design', 'productive', '{"appNames":["Figma","Sketch","Adobe Photoshop","Adobe Illustrator","Affinity Designer"],"bundleIds":["com.figma.Desktop"],"urlPatterns":[]}', TRUE),
    ('documentation', 'Documentation', 'productive', '{"appNames":["Notion","Obsidian","Bear","Typora","Pages"],"bundleIds":["notion.id","md.obsidian"],"urlPatterns":["docs.google.com","notion.so"]}', TRUE),
    ('entertainment', 'Entertainment', 'distracting', '{"appNames":[],"bundleIds":[],"urlPatterns":["youtube.com","netflix.com","twitter.com","x.com","reddit.com","tiktok.com","instagram.com","facebook.com"]}', TRUE),
    ('email', 'Email', 'neutral', '{"appNames":["Mail","Spark","Airmail","Superhuman"],"bundleIds":["com.apple.mail"],"urlPatterns":["mail.google.com","outlook.live.com"]}', TRUE),
    ('project_management', 'Project Management', 'productive', '{"appNames":["Linear","Jira","Asana","Trello","ClickUp","Monday","Height"],"bundleIds":["com.linear"],"urlPatterns":["linear.app","jira.atlassian.com","asana.com","trello.com"]}', TRUE),
    ('reference', 'Reference', 'productive', '{"appNames":[],"bundleIds":[],"urlPatterns":["developer.apple.com","docs.rs","crates.io","stackoverflow.com","mdn.mozilla.org"]}', TRUE);

-- Activity events: raw window tracking data (high-frequency)
CREATE TABLE IF NOT EXISTS activity_events (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    app_name      TEXT NOT NULL,
    window_title  TEXT,
    site_name     TEXT,
    bundle_id     TEXT,
    url           TEXT,
    category_id   TEXT REFERENCES activity_categories(id),
    started_at    TEXT NOT NULL,
    ended_at      TEXT,
    duration_secs INTEGER,
    is_idle       BOOLEAN NOT NULL DEFAULT FALSE,
    metadata      TEXT
);

CREATE INDEX IF NOT EXISTS idx_activity_events_started ON activity_events(started_at DESC);
CREATE INDEX IF NOT EXISTS idx_activity_events_category ON activity_events(category_id);
CREATE INDEX IF NOT EXISTS idx_activity_events_app ON activity_events(app_name, started_at DESC);
CREATE INDEX IF NOT EXISTS idx_activity_events_site ON activity_events(site_name, started_at DESC);

-- Focus sessions: explicit deep work periods
CREATE TABLE IF NOT EXISTS focus_sessions (
    id            TEXT PRIMARY KEY,
    action_id     TEXT,
    project_id    TEXT,
    session_type  TEXT NOT NULL DEFAULT 'focus',
    target_mins   INTEGER,
    started_at    TEXT NOT NULL,
    ended_at      TEXT,
    actual_mins   INTEGER,
    interruptions INTEGER NOT NULL DEFAULT 0,
    distraction_events TEXT,
    quality_score REAL,
    completed     BOOLEAN NOT NULL DEFAULT FALSE,
    notes         TEXT
);

CREATE INDEX IF NOT EXISTS idx_focus_sessions_started ON focus_sessions(started_at DESC);
CREATE INDEX IF NOT EXISTS idx_focus_sessions_action ON focus_sessions(action_id);

-- Daily aggregated summaries
CREATE TABLE IF NOT EXISTS daily_summaries (
    date                 TEXT PRIMARY KEY,
    total_active_secs    INTEGER NOT NULL DEFAULT 0,
    total_focus_secs     INTEGER NOT NULL DEFAULT 0,
    total_break_secs     INTEGER NOT NULL DEFAULT 0,
    total_idle_secs      INTEGER NOT NULL DEFAULT 0,
    productive_secs      INTEGER NOT NULL DEFAULT 0,
    neutral_secs         INTEGER NOT NULL DEFAULT 0,
    distracting_secs     INTEGER NOT NULL DEFAULT 0,
    focus_sessions_count INTEGER NOT NULL DEFAULT 0,
    avg_session_quality  REAL,
    interruptions_count  INTEGER NOT NULL DEFAULT 0,
    context_switches     INTEGER NOT NULL DEFAULT 0,
    top_apps             TEXT,
    top_categories       TEXT,
    productivity_score   REAL,
    ai_summary           TEXT,
    computed_at          TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Nudge history: prevents over-nudging
CREATE TABLE IF NOT EXISTS nudge_history (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    nudge_type    TEXT NOT NULL,
    message       TEXT NOT NULL,
    channel       TEXT,
    acknowledged  BOOLEAN NOT NULL DEFAULT FALSE,
    created_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_nudge_history_type_created ON nudge_history(nudge_type, created_at DESC);

-- Goals table for daily/weekly targets
CREATE TABLE IF NOT EXISTS productivity_goals (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    goal_type     TEXT NOT NULL DEFAULT 'daily',
    metric        TEXT NOT NULL,
    target_value  REAL NOT NULL,
    enabled       BOOLEAN NOT NULL DEFAULT TRUE,
    created_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Manual time entries
CREATE TABLE IF NOT EXISTS time_entries (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    description   TEXT NOT NULL,
    category_id   TEXT REFERENCES activity_categories(id),
    project_id    TEXT,
    started_at    TEXT NOT NULL,
    duration_secs INTEGER NOT NULL,
    source        TEXT NOT NULL DEFAULT 'manual',
    created_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_time_entries_started ON time_entries(started_at DESC);

-- Distraction learned rules: AI-classified content patterns
CREATE TABLE IF NOT EXISTS distraction_learned_rules (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    pattern TEXT NOT NULL,
    pattern_type TEXT NOT NULL CHECK (pattern_type IN ('title_keyword', 'app_name', 'url_pattern')),
    classification TEXT NOT NULL CHECK (classification IN ('educational', 'work_research')),
    confidence REAL NOT NULL DEFAULT 0.5,
    hit_count INTEGER NOT NULL DEFAULT 1,
    last_used_at TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_learned_rules_pattern ON distraction_learned_rules (pattern, pattern_type);
