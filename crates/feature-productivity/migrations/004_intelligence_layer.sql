-- Intelligence Layer tables: tracking rules, sessions, quality scores, forecasts,
-- narratives, voice journals, categorization cache, privacy rules, rule evolution log.

-- Tracking rules: user-editable + auto-evolved classification rules
CREATE TABLE IF NOT EXISTS productivity_tracking_rules (
    id            TEXT PRIMARY KEY,
    rule_type     TEXT NOT NULL CHECK (rule_type IN ('app', 'url', 'title', 'compound')),
    match_field   TEXT NOT NULL,
    match_pattern TEXT NOT NULL,
    match_mode    TEXT NOT NULL DEFAULT 'exact' CHECK (match_mode IN ('exact', 'prefix', 'contains', 'regex')),
    category      TEXT NOT NULL,
    session_type  TEXT NOT NULL DEFAULT 'focus' CHECK (session_type IN ('focus', 'meeting', 'break')),
    priority      INTEGER NOT NULL DEFAULT 100,
    source        TEXT NOT NULL DEFAULT 'system' CHECK (source IN ('system', 'user', 'learned')),
    confidence    REAL NOT NULL DEFAULT 1.0,
    hit_count     INTEGER NOT NULL DEFAULT 0,
    last_hit_at   TEXT,
    is_active     BOOLEAN NOT NULL DEFAULT TRUE,
    created_at    TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_tracking_rules_active ON productivity_tracking_rules(is_active, rule_type, priority);
CREATE INDEX IF NOT EXISTS idx_tracking_rules_category ON productivity_tracking_rules(category);

-- Unified sessions: focus, meeting, break
CREATE TABLE IF NOT EXISTS productivity_sessions (
    id                TEXT PRIMARY KEY,
    session_type      TEXT NOT NULL CHECK (session_type IN ('focus', 'meeting', 'break', 'pomodoro')),
    started_at        TEXT NOT NULL,
    ended_at          TEXT,
    duration_secs     INTEGER,
    dominant_category TEXT,
    category_purity   REAL,
    quality_score     REAL,
    source            TEXT NOT NULL DEFAULT 'auto' CHECK (source IN ('auto', 'manual', 'predicted', 'auto_detected', 'pomodoro')),
    app_breakdown     TEXT,
    context_switches  INTEGER NOT NULL DEFAULT 0,
    distraction_count INTEGER NOT NULL DEFAULT 0,
    predicted_energy  REAL,
    okr_alignment     REAL,
    notes             TEXT,
    tags              TEXT,
    action_id         TEXT,
    project_id        TEXT,
    target_mins       INTEGER,
    actual_mins       INTEGER,
    interruptions     INTEGER DEFAULT 0,
    distraction_events TEXT,
    completed         INTEGER DEFAULT 0,
    created_at        TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at        TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_prod_sessions_started ON productivity_sessions(started_at DESC);
CREATE INDEX IF NOT EXISTS idx_prod_sessions_type ON productivity_sessions(session_type, started_at DESC);
CREATE INDEX IF NOT EXISTS idx_prod_sessions_active ON productivity_sessions(ended_at) WHERE ended_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_productivity_sessions_action ON productivity_sessions(action_id) WHERE action_id IS NOT NULL;

-- Quality scores: daily + per-session
CREATE TABLE IF NOT EXISTS productivity_quality_scores (
    id              TEXT PRIMARY KEY,
    score_date      TEXT NOT NULL,
    session_id      TEXT REFERENCES productivity_sessions(id) ON DELETE CASCADE,
    overall_score   REAL NOT NULL,
    focus_depth     REAL NOT NULL DEFAULT 0.0,
    okr_alignment   REAL NOT NULL DEFAULT 0.0,
    distraction_inv REAL NOT NULL DEFAULT 0.0,
    task_completion REAL NOT NULL DEFAULT 0.0,
    continuity      REAL NOT NULL DEFAULT 0.0,
    weights_json    TEXT,
    explanation     TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_quality_daily ON productivity_quality_scores(score_date) WHERE session_id IS NULL;
CREATE INDEX IF NOT EXISTS idx_quality_session ON productivity_quality_scores(session_id);

-- Forecasts: energy, focus window, meeting load, burnout risk
CREATE TABLE IF NOT EXISTS productivity_forecasts (
    id               TEXT PRIMARY KEY,
    forecast_date    TEXT NOT NULL,
    forecast_type    TEXT NOT NULL CHECK (forecast_type IN ('energy', 'focus_window', 'meeting_load', 'burnout_risk')),
    window_start     TEXT,
    window_end       TEXT,
    predicted_value  REAL NOT NULL,
    confidence       REAL NOT NULL DEFAULT 0.5,
    stability        REAL NOT NULL DEFAULT 1.0,
    auto_protected   BOOLEAN NOT NULL DEFAULT FALSE,
    user_overrode    BOOLEAN NOT NULL DEFAULT FALSE,
    actual_value     REAL,
    prediction_error REAL,
    created_at       TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_forecasts_date ON productivity_forecasts(forecast_date, forecast_type);

-- Narratives: daily stories
CREATE TABLE IF NOT EXISTS productivity_narratives (
    id                TEXT PRIMARY KEY,
    narrative_date    TEXT NOT NULL UNIQUE,
    narrative_text    TEXT NOT NULL,
    key_moments       TEXT,
    sentiment         TEXT CHECK (sentiment IN ('positive', 'neutral', 'warning', 'negative')),
    total_focus_mins  INTEGER NOT NULL DEFAULT 0,
    total_meeting_mins INTEGER NOT NULL DEFAULT 0,
    total_break_mins  INTEGER NOT NULL DEFAULT 0,
    quality_score     REAL,
    top_categories    TEXT,
    created_at        TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Voice journals
CREATE TABLE IF NOT EXISTS productivity_voice_journals (
    id              TEXT PRIMARY KEY,
    recorded_at     TEXT NOT NULL,
    duration_secs   INTEGER NOT NULL DEFAULT 0,
    transcript      TEXT,
    extracted_facts TEXT,
    sentiment       TEXT,
    session_id      TEXT REFERENCES productivity_sessions(id) ON DELETE SET NULL,
    processed       BOOLEAN NOT NULL DEFAULT FALSE,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_voice_journals_recorded ON productivity_voice_journals(recorded_at DESC);

-- Categorization cache: AI fallback results
CREATE TABLE IF NOT EXISTS productivity_categorization_cache (
    cache_key    TEXT PRIMARY KEY,
    category     TEXT NOT NULL,
    session_type TEXT NOT NULL DEFAULT 'focus',
    confidence   REAL NOT NULL DEFAULT 0.5,
    source       TEXT NOT NULL DEFAULT 'rule' CHECK (source IN ('rule', 'ai', 'user_override')),
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at   TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_cache_expires ON productivity_categorization_cache(expires_at);

-- Privacy rules: user-defined exclusions
CREATE TABLE IF NOT EXISTS productivity_privacy_rules (
    id          TEXT PRIMARY KEY,
    rule_type   TEXT NOT NULL CHECK (rule_type IN ('exclude_app', 'redact_title', 'exclude_url')),
    pattern     TEXT NOT NULL,
    match_mode  TEXT NOT NULL DEFAULT 'exact' CHECK (match_mode IN ('exact', 'prefix', 'contains', 'regex')),
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Rule evolution log: audit trail for rule changes
CREATE TABLE IF NOT EXISTS productivity_rule_evolution_log (
    id              TEXT PRIMARY KEY,
    rule_id         TEXT NOT NULL REFERENCES productivity_tracking_rules(id) ON DELETE CASCADE,
    action          TEXT NOT NULL CHECK (action IN ('created', 'promoted', 'demoted', 'merged', 'deactivated')),
    old_confidence  REAL,
    new_confidence  REAL,
    old_category    TEXT,
    new_category    TEXT,
    trigger_source  TEXT,
    evidence_count  INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_rule_evolution_rule ON productivity_rule_evolution_log(rule_id, created_at DESC);

-- Data migration: copy existing focus_sessions → productivity_sessions
INSERT OR IGNORE INTO productivity_sessions (id, session_type, started_at, ended_at, duration_secs, quality_score, source, notes, action_id, project_id, target_mins, actual_mins, interruptions, distraction_events, completed, created_at)
SELECT
    id,
    session_type,
    started_at,
    ended_at,
    CASE WHEN actual_mins IS NOT NULL THEN actual_mins * 60 ELSE NULL END,
    quality_score,
    source,
    notes,
    action_id,
    project_id,
    target_mins,
    actual_mins,
    interruptions,
    distraction_events,
    completed,
    started_at
FROM focus_sessions;

-- Seed default tracking rules from existing activity_categories
-- App-based rules (exact match)
INSERT OR IGNORE INTO productivity_tracking_rules (id, rule_type, match_field, match_pattern, match_mode, category, session_type, priority, source) VALUES
    -- Coding (productive → focus)
    ('sys-app-vscode',        'app', 'app_name', 'Visual Studio Code', 'exact', 'coding', 'focus', 10, 'system'),
    ('sys-app-code',          'app', 'app_name', 'Code',               'exact', 'coding', 'focus', 10, 'system'),
    ('sys-app-cursor',        'app', 'app_name', 'Cursor',             'exact', 'coding', 'focus', 10, 'system'),
    ('sys-app-xcode',         'app', 'app_name', 'Xcode',              'exact', 'coding', 'focus', 10, 'system'),
    ('sys-app-intellij',      'app', 'app_name', 'IntelliJ IDEA',      'exact', 'coding', 'focus', 10, 'system'),
    ('sys-app-webstorm',      'app', 'app_name', 'WebStorm',           'exact', 'coding', 'focus', 10, 'system'),
    ('sys-app-terminal',      'app', 'app_name', 'Terminal',            'exact', 'coding', 'focus', 10, 'system'),
    ('sys-app-iterm2',        'app', 'app_name', 'iTerm2',             'exact', 'coding', 'focus', 10, 'system'),
    ('sys-app-warp',          'app', 'app_name', 'Warp',               'exact', 'coding', 'focus', 10, 'system'),
    ('sys-app-ghostty',       'app', 'app_name', 'Ghostty',            'exact', 'coding', 'focus', 10, 'system'),
    ('sys-app-zed',           'app', 'app_name', 'Zed',                'exact', 'coding', 'focus', 10, 'system'),
    ('sys-app-windsurf',      'app', 'app_name', 'Windsurf',           'exact', 'coding', 'focus', 10, 'system'),
    ('sys-app-sublime',       'app', 'app_name', 'Sublime Text',       'exact', 'coding', 'focus', 10, 'system'),
    ('sys-app-androidstudio', 'app', 'app_name', 'Android Studio',     'exact', 'coding', 'focus', 10, 'system'),
    ('sys-app-pycharm',       'app', 'app_name', 'PyCharm',            'exact', 'coding', 'focus', 10, 'system'),
    ('sys-app-rustrover',     'app', 'app_name', 'RustRover',          'exact', 'coding', 'focus', 10, 'system'),
    -- Communication (neutral → meeting)
    ('sys-app-slack',         'app', 'app_name', 'Slack',              'exact', 'communication', 'meeting', 20, 'system'),
    ('sys-app-discord',       'app', 'app_name', 'Discord',            'exact', 'communication', 'meeting', 20, 'system'),
    ('sys-app-telegram',      'app', 'app_name', 'Telegram',           'exact', 'communication', 'meeting', 20, 'system'),
    ('sys-app-teams',         'app', 'app_name', 'Microsoft Teams',    'exact', 'communication', 'meeting', 20, 'system'),
    ('sys-app-zoom',          'app', 'app_name', 'Zoom',               'exact', 'communication', 'meeting', 20, 'system'),
    ('sys-app-messages',      'app', 'app_name', 'Messages',           'exact', 'communication', 'meeting', 20, 'system'),
    -- Design (productive → focus)
    ('sys-app-figma',         'app', 'app_name', 'Figma',              'exact', 'design', 'focus', 10, 'system'),
    ('sys-app-sketch',        'app', 'app_name', 'Sketch',             'exact', 'design', 'focus', 10, 'system'),
    ('sys-app-photoshop',     'app', 'app_name', 'Adobe Photoshop',    'exact', 'design', 'focus', 10, 'system'),
    -- Documentation (productive → focus)
    ('sys-app-notion',        'app', 'app_name', 'Notion',             'exact', 'documentation', 'focus', 15, 'system'),
    ('sys-app-obsidian',      'app', 'app_name', 'Obsidian',           'exact', 'documentation', 'focus', 15, 'system'),
    -- Email (neutral → focus)
    ('sys-app-mail',          'app', 'app_name', 'Mail',               'exact', 'email', 'focus', 30, 'system'),
    ('sys-app-spark',         'app', 'app_name', 'Spark',              'exact', 'email', 'focus', 30, 'system'),
    -- Music (neutral → break)
    ('sys-app-spotify',       'app', 'app_name', 'Spotify',            'exact', 'music', 'break', 50, 'system'),
    ('sys-app-applemusic',    'app', 'app_name', 'Music',              'exact', 'music', 'break', 50, 'system'),
    -- Project Management (productive → focus)
    ('sys-app-linear',        'app', 'app_name', 'Linear',             'exact', 'project_management', 'focus', 20, 'system'),
    -- Browsers (neutral → focus, low priority since URL rules override)
    ('sys-app-safari',        'app', 'app_name', 'Safari',             'exact', 'browsing', 'focus', 90, 'system'),
    ('sys-app-chrome',        'app', 'app_name', 'Google Chrome',      'exact', 'browsing', 'focus', 90, 'system'),
    ('sys-app-arc',           'app', 'app_name', 'Arc',                'exact', 'browsing', 'focus', 90, 'system'),
    ('sys-app-firefox',       'app', 'app_name', 'Firefox',            'exact', 'browsing', 'focus', 90, 'system'),
    ('sys-app-brave',         'app', 'app_name', 'Brave Browser',      'exact', 'browsing', 'focus', 90, 'system'),
    ('sys-app-zen',           'app', 'app_name', 'Zen Browser',        'exact', 'browsing', 'focus', 90, 'system');

-- URL-based rules (prefix match, higher priority overrides browser app match)
INSERT OR IGNORE INTO productivity_tracking_rules (id, rule_type, match_field, match_pattern, match_mode, category, session_type, priority, source) VALUES
    -- Developer Tools (productive → focus)
    ('sys-url-github',        'url', 'url', 'github.com',             'contains', 'developer_tools', 'focus', 5, 'system'),
    ('sys-url-gitlab',        'url', 'url', 'gitlab.com',             'contains', 'developer_tools', 'focus', 5, 'system'),
    ('sys-url-stackoverflow', 'url', 'url', 'stackoverflow.com',      'contains', 'developer_tools', 'focus', 5, 'system'),
    -- AI Tools (productive → focus)
    ('sys-url-claude',        'url', 'url', 'claude.ai',              'contains', 'ai_tools', 'focus', 5, 'system'),
    ('sys-url-chatgpt',       'url', 'url', 'chatgpt.com',            'contains', 'ai_tools', 'focus', 5, 'system'),
    ('sys-url-perplexity',    'url', 'url', 'perplexity.ai',          'contains', 'ai_tools', 'focus', 5, 'system'),
    -- Documentation URLs
    ('sys-url-googledocs',    'url', 'url', 'docs.google.com',        'contains', 'documentation', 'focus', 10, 'system'),
    ('sys-url-notion',        'url', 'url', 'notion.so',              'contains', 'documentation', 'focus', 10, 'system'),
    -- Cloud & DevOps
    ('sys-url-vercel',        'url', 'url', 'vercel.com',             'contains', 'cloud_devops', 'focus', 10, 'system'),
    ('sys-url-aws',           'url', 'url', 'console.aws.amazon.com', 'contains', 'cloud_devops', 'focus', 10, 'system'),
    -- Project Management URLs
    ('sys-url-linear',        'url', 'url', 'linear.app',             'contains', 'project_management', 'focus', 10, 'system'),
    ('sys-url-jira',          'url', 'url', 'jira.atlassian.com',     'contains', 'project_management', 'focus', 10, 'system'),
    -- Learning (productive → focus)
    ('sys-url-udemy',         'url', 'url', 'udemy.com',              'contains', 'learning', 'focus', 15, 'system'),
    ('sys-url-coursera',      'url', 'url', 'coursera.org',           'contains', 'learning', 'focus', 15, 'system'),
    -- Social Media (distracting → break)
    ('sys-url-twitter',       'url', 'url', 'x.com',                  'contains', 'social_media', 'break', 5, 'system'),
    ('sys-url-facebook',      'url', 'url', 'facebook.com',           'contains', 'social_media', 'break', 5, 'system'),
    ('sys-url-instagram',     'url', 'url', 'instagram.com',          'contains', 'social_media', 'break', 5, 'system'),
    ('sys-url-linkedin',      'url', 'url', 'linkedin.com',           'contains', 'social_media', 'break', 5, 'system'),
    ('sys-url-reddit',        'url', 'url', 'reddit.com',             'contains', 'news_forums', 'break', 20, 'system'),
    ('sys-url-hackernews',    'url', 'url', 'news.ycombinator.com',   'contains', 'news_forums', 'break', 20, 'system'),
    -- Video Streaming (distracting → break)
    ('sys-url-youtube',       'url', 'url', 'youtube.com',            'contains', 'video_streaming', 'break', 5, 'system'),
    ('sys-url-netflix',       'url', 'url', 'netflix.com',            'contains', 'video_streaming', 'break', 5, 'system'),
    ('sys-url-twitch',        'url', 'url', 'twitch.tv',              'contains', 'video_streaming', 'break', 5, 'system'),
    -- Shopping (distracting → break)
    ('sys-url-amazon',        'url', 'url', 'amazon.com',             'contains', 'shopping', 'break', 10, 'system'),
    -- Email URLs
    ('sys-url-gmail',         'url', 'url', 'mail.google.com',        'contains', 'email', 'focus', 15, 'system'),
    ('sys-url-outlook',       'url', 'url', 'outlook.live.com',       'contains', 'email', 'focus', 15, 'system'),
    -- Communication URLs
    ('sys-url-slack',         'url', 'url', 'slack.com',              'contains', 'communication', 'meeting', 10, 'system'),
    ('sys-url-discord',       'url', 'url', 'discord.com',            'contains', 'communication', 'meeting', 10, 'system'),
    ('sys-url-meet',          'url', 'url', 'meet.google.com',        'contains', 'communication', 'meeting', 5, 'system'),
    ('sys-url-zoomus',        'url', 'url', 'zoom.us',                'contains', 'communication', 'meeting', 5, 'system'),
    -- Finance (neutral → focus)
    ('sys-url-tradingview',   'url', 'url', 'tradingview.com',        'contains', 'finance', 'focus', 20, 'system'),
    -- Entertainment (distracting → break)
    ('sys-url-9gag',          'url', 'url', '9gag.com',               'contains', 'entertainment', 'break', 5, 'system');

-- Calendar events table
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
    external_uid TEXT,
    session_id TEXT REFERENCES productivity_sessions(id),
    color TEXT,
    synced_at TEXT NOT NULL DEFAULT (datetime('now')),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(external_uid, calendar_id)
);

CREATE INDEX IF NOT EXISTS idx_calendar_events_time ON calendar_events(started_at, ended_at);
CREATE INDEX IF NOT EXISTS idx_calendar_events_date ON calendar_events(date(started_at));
CREATE INDEX IF NOT EXISTS idx_calendar_events_session ON calendar_events(session_id) WHERE session_id IS NOT NULL;

-- Weekly assessments: aggregated weekly productivity data
CREATE TABLE IF NOT EXISTS weekly_assessments (
    id                    TEXT PRIMARY KEY,
    week_start            TEXT NOT NULL,
    week_end              TEXT NOT NULL,
    avg_score             REAL,
    total_focus_mins      INTEGER,
    total_productive_secs INTEGER,
    total_distracting_secs INTEGER,
    top_apps              TEXT,
    summary               TEXT,
    created_at            TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_weekly_assessments_week ON weekly_assessments(week_start);
