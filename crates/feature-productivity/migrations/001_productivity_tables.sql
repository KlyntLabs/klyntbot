-- Consolidated productivity tracking schema (v1–v7 merged)
-- All ALTER TABLE columns folded into original CREATE TABLE statements.

-- ── Activity categories + seeds ──────────────────────────────────────────────

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

INSERT OR IGNORE INTO activity_categories (id, name, category_type, rules, is_system) VALUES
    -- ── App-based categories ────────────────────────────────────────────
    ('coding', 'Coding', 'productive',
     '{"appNames":["Visual Studio Code","Code","Xcode","IntelliJ IDEA","WebStorm","Terminal","iTerm2","Warp","Alacritty","kitty","Ghostty","Hyper","Rio","WezTerm","Cursor","Zed","Sublime Text","Vim","Neovim","Emacs","Nova","Fleet","Android Studio","PyCharm","GoLand","CLion","RustRover","DataGrip","Windsurf"],"bundleIds":["com.microsoft.VSCode","com.apple.Terminal","com.googlecode.iterm2","com.mitchellh.ghostty","com.todesktop.230313mzl4w4u92","dev.zed.Zed"],"urlPatterns":[]}',
     TRUE),
    ('communication', 'Communication', 'neutral',
     '{"appNames":["Slack","Discord","Telegram","WhatsApp","Messages","Microsoft Teams","Zoom","Signal","Lark","Google Chat","Element","Wire"],"bundleIds":["com.tinyspeck.slackmacgap","com.hnc.Discord","ru.keepcoder.Telegram","com.facebook.archon"],"urlPatterns":["slack.com","discord.com","telegram.org","web.whatsapp.com","teams.microsoft.com","meet.google.com","zoom.us"]}',
     TRUE),
    ('browsing', 'Browsing', 'neutral',
     '{"appNames":["Safari","Firefox","Google Chrome","Arc","Brave Browser","Orion","Vivaldi","Microsoft Edge","Opera","Chromium","Zen Browser"],"bundleIds":["com.apple.Safari","org.mozilla.firefox","com.google.Chrome","company.thebrowser.browser","com.brave.browser","com.microsoft.edgemac","com.operasoftware.opera","com.vivaldi.vivaldi"],"urlPatterns":[]}',
     TRUE),
    ('design', 'Design', 'productive',
     '{"appNames":["Figma","Sketch","Adobe Photoshop","Adobe Illustrator","Affinity Designer","Affinity Photo","Pixelmator Pro","Canva"],"bundleIds":["com.figma.Desktop"],"urlPatterns":["figma.com","canva.com"]}',
     TRUE),
    ('documentation', 'Documentation', 'productive',
     '{"appNames":["Notion","Obsidian","Bear","Typora","Pages","iA Writer","Ulysses"],"bundleIds":["notion.id","md.obsidian"],"urlPatterns":["docs.google.com","notion.so","obsidian.md","gitbook.io","readme.io","confluence.atlassian.com","hackmd.io","overleaf.com"]}',
     TRUE),
    ('email', 'Email', 'neutral',
     '{"appNames":["Mail","Spark","Airmail","Superhuman"],"bundleIds":["com.apple.mail"],"urlPatterns":["mail.google.com","outlook.live.com","outlook.office.com","protonmail.com"]}',
     TRUE),
    ('music', 'Music', 'neutral',
     '{"appNames":["Spotify","Apple Music","Music"],"bundleIds":["com.spotify.client","com.apple.Music"],"urlPatterns":["spotify.com","music.apple.com","music.youtube.com","soundcloud.com","bandcamp.com","tidal.com","deezer.com","pandora.com"]}',
     TRUE),
    ('gaming', 'Gaming', 'distracting',
     '{"appNames":["Steam","Epic Games Launcher","Battle.net","GOG Galaxy","Riot Client"],"bundleIds":["com.valvesoftware.steam"],"urlPatterns":["store.steampowered.com","epicgames.com","itch.io","gog.com","battle.net","riotgames.com","ea.com","ubisoft.com"]}',
     TRUE),

    -- ── Site-specific categories (matched via browser window title) ─────
    ('ai_tools', 'AI Tools', 'productive',
     '{"appNames":[],"bundleIds":[],"urlPatterns":["claude.ai","chatgpt.com","openai.com","gemini.google.com","perplexity.ai","copilot.microsoft.com","huggingface.co","replicate.com","anthropic.com","bard.google.com","poe.com","character.ai","midjourney.com","stability.ai","ollama.com","together.ai","groq.com","mistral.ai","cohere.com","cursor.com","v0.dev","bolt.new"]}',
     TRUE),
    ('social_media', 'Social Media', 'distracting',
     '{"appNames":[],"bundleIds":[],"urlPatterns":["facebook.com","instagram.com","twitter.com","x.com","tiktok.com","linkedin.com","threads.net","snapchat.com","pinterest.com","tumblr.com","mastodon.social","bsky.app","bluesky.com","weibo.com","vk.com"]}',
     TRUE),
    ('video_streaming', 'Video & Streaming', 'distracting',
     '{"appNames":[],"bundleIds":[],"urlPatterns":["youtube.com","netflix.com","twitch.tv","disneyplus.com","hulu.com","primevideo.com","max.com","peacocktv.com","crunchyroll.com","bilibili.com","vimeo.com","dailymotion.com","kick.com"]}',
     TRUE),
    ('news_forums', 'News & Forums', 'neutral',
     '{"appNames":[],"bundleIds":[],"urlPatterns":["reddit.com","news.ycombinator.com","medium.com","substack.com","dev.to","lobste.rs","bbc.com","cnn.com","nytimes.com","theguardian.com","reuters.com","arstechnica.com","techcrunch.com","theverge.com","wired.com","producthunt.com","indiehackers.com"]}',
     TRUE),
    ('developer_tools', 'Developer Tools', 'productive',
     '{"appNames":[],"bundleIds":[],"urlPatterns":["github.com","gitlab.com","bitbucket.org","stackoverflow.com","stackexchange.com","crates.io","docs.rs","npmjs.com","pypi.org","rubygems.org","developer.apple.com","developer.mozilla.org","mdn.mozilla.org","web.dev","bundlephobia.com","pkg.go.dev","hex.pm","codepen.io","codesandbox.io","replit.com","regex101.com","excalidraw.com"]}',
     TRUE),
    ('cloud_devops', 'Cloud & DevOps', 'productive',
     '{"appNames":[],"bundleIds":[],"urlPatterns":["console.aws.amazon.com","portal.azure.com","console.cloud.google.com","vercel.com","netlify.com","railway.app","fly.io","render.com","heroku.com","digitalocean.com","cloudflare.com","supabase.com","planetscale.com","neon.tech","upstash.com","docker.com","grafana.com","datadog.com","sentry.io"]}',
     TRUE),
    ('project_management', 'Project Management', 'productive',
     '{"appNames":["Linear","Jira","Asana","Trello","ClickUp","Monday","Height","Basecamp"],"bundleIds":["com.linear"],"urlPatterns":["linear.app","jira.atlassian.com","asana.com","trello.com","clickup.com","monday.com","basecamp.com","shortcut.com","height.app","plane.so"]}',
     TRUE),
    ('shopping', 'Shopping', 'distracting',
     '{"appNames":[],"bundleIds":[],"urlPatterns":["amazon.com","ebay.com","etsy.com","aliexpress.com","wish.com","walmart.com","target.com","bestbuy.com","newegg.com","shopee.com","lazada.com","taobao.com","jd.com"]}',
     TRUE),
    ('finance', 'Finance', 'neutral',
     '{"appNames":[],"bundleIds":[],"urlPatterns":["paypal.com","stripe.com","wise.com","revolut.com","robinhood.com","coinbase.com","binance.com","tradingview.com","bloomberg.com","wsj.com","investopedia.com","mint.com","ynab.com"]}',
     TRUE),
    ('learning', 'Learning', 'productive',
     '{"appNames":[],"bundleIds":[],"urlPatterns":["udemy.com","coursera.org","edx.org","khanacademy.org","brilliant.org","codecademy.com","freecodecamp.org","leetcode.com","hackerrank.com","exercism.org","egghead.io","pluralsight.com","skillshare.com","duolingo.com","wikipedia.org"]}',
     TRUE),
    ('entertainment', 'Entertainment', 'distracting',
     '{"appNames":[],"bundleIds":[],"urlPatterns":["9gag.com","imgur.com","buzzfeed.com","boredpanda.com","knowyourmeme.com"]}',
     TRUE);

-- ── Activity events ──────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS activity_events (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    app_name         TEXT NOT NULL,
    window_title     TEXT,
    site_name        TEXT,
    bundle_id        TEXT,
    url              TEXT,
    category_id      TEXT REFERENCES activity_categories(id),
    started_at       TEXT NOT NULL,
    ended_at         TEXT,
    duration_secs    INTEGER,
    is_idle          BOOLEAN NOT NULL DEFAULT FALSE,
    metadata         TEXT,
    project_id       TEXT REFERENCES productivity_projects(id),
    focus_session_id TEXT REFERENCES productivity_sessions(id)
);

CREATE INDEX IF NOT EXISTS idx_activity_events_started ON activity_events(started_at DESC);
CREATE INDEX IF NOT EXISTS idx_activity_events_category ON activity_events(category_id);
CREATE INDEX IF NOT EXISTS idx_activity_events_app ON activity_events(app_name, started_at DESC);
CREATE INDEX IF NOT EXISTS idx_activity_events_site ON activity_events(site_name, started_at DESC);
CREATE INDEX IF NOT EXISTS idx_activity_events_started_idle ON activity_events(started_at, is_idle);
CREATE INDEX IF NOT EXISTS idx_activity_events_project ON activity_events(started_at, project_id);
CREATE INDEX IF NOT EXISTS idx_activity_events_focus_session
    ON activity_events(focus_session_id)
    WHERE focus_session_id IS NOT NULL;

-- ── Focus sessions ───────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS focus_sessions (
    id                 TEXT PRIMARY KEY,
    action_id          TEXT,
    project_id         TEXT,
    session_type       TEXT NOT NULL DEFAULT 'focus',
    target_mins        INTEGER,
    started_at         TEXT NOT NULL,
    ended_at           TEXT,
    actual_mins        INTEGER,
    interruptions      INTEGER NOT NULL DEFAULT 0,
    distraction_events TEXT,
    quality_score      REAL,
    completed          BOOLEAN NOT NULL DEFAULT FALSE,
    notes              TEXT,
    source             TEXT NOT NULL DEFAULT 'manual'
);

CREATE INDEX IF NOT EXISTS idx_focus_sessions_started ON focus_sessions(started_at DESC);
CREATE INDEX IF NOT EXISTS idx_focus_sessions_action ON focus_sessions(action_id);

-- ── Daily summaries ──────────────────────────────────────────────────────────

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
    top_projects         TEXT,
    productivity_score   REAL,
    ai_summary           TEXT,
    computed_at          TEXT NOT NULL DEFAULT (datetime('now')),
    deep_work_blocks     INTEGER NOT NULL DEFAULT 0,
    deep_work_secs       INTEGER NOT NULL DEFAULT 0,
    avg_recovery_secs    REAL
);

-- ── Nudge history ────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS nudge_history (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    nudge_type    TEXT NOT NULL,
    message       TEXT NOT NULL,
    channel       TEXT,
    acknowledged  BOOLEAN NOT NULL DEFAULT FALSE,
    created_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_nudge_history_type_created ON nudge_history(nudge_type, created_at DESC);

-- ── Goals ────────────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS productivity_goals (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    goal_type     TEXT NOT NULL DEFAULT 'daily',
    metric        TEXT NOT NULL,
    target_value  REAL NOT NULL,
    enabled       BOOLEAN NOT NULL DEFAULT TRUE,
    created_at    TEXT NOT NULL DEFAULT (datetime('now')),
    project_id    TEXT REFERENCES productivity_projects(id)
);

-- ── Time entries ─────────────────────────────────────────────────────────────

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

-- ── Distraction rules ────────────────────────────────────────────────────────

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

-- ── Activity buckets ─────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS activity_buckets (
    bucket_start      TEXT NOT NULL,
    date              TEXT NOT NULL,
    dominant_app      TEXT,
    dominant_site     TEXT,
    dominant_category TEXT,
    productive_secs   INTEGER NOT NULL DEFAULT 0,
    neutral_secs      INTEGER NOT NULL DEFAULT 0,
    distracting_secs  INTEGER NOT NULL DEFAULT 0,
    idle_secs         INTEGER NOT NULL DEFAULT 0,
    context_switches  INTEGER NOT NULL DEFAULT 0,
    focus_depth       REAL,
    tick_count        INTEGER NOT NULL DEFAULT 0,
    dominant_project  TEXT,
    PRIMARY KEY (bucket_start)
);

CREATE INDEX IF NOT EXISTS idx_buckets_date ON activity_buckets(date);

-- ── Distraction patterns ─────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS distraction_patterns (
    id                      INTEGER PRIMARY KEY AUTOINCREMENT,
    date                    TEXT NOT NULL,
    hour_of_day             INTEGER NOT NULL,
    hours_active_today      REAL NOT NULL,
    mins_since_break        REAL NOT NULL,
    preceding_app           TEXT,
    preceding_category      TEXT,
    preceding_duration_mins REAL,
    distraction_app         TEXT NOT NULL,
    distraction_category    TEXT,
    recovery_secs           INTEGER,
    created_at              TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_distraction_date ON distraction_patterns(date);

-- ── Insight cards ────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS insight_cards (
    id              TEXT PRIMARY KEY,
    insight_type    TEXT NOT NULL,
    title           TEXT NOT NULL,
    body            TEXT NOT NULL,
    sentiment       TEXT NOT NULL,
    metric_value    REAL,
    baseline_value  REAL,
    date            TEXT NOT NULL,
    dismissed       BOOLEAN NOT NULL DEFAULT FALSE,
    generated_at    TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_insights_date ON insight_cards(date);
CREATE UNIQUE INDEX IF NOT EXISTS idx_insights_type_date ON insight_cards(insight_type, date);

-- ── Projects ─────────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS productivity_projects (
    id               TEXT PRIMARY KEY,
    display_name     TEXT NOT NULL,
    path             TEXT NOT NULL UNIQUE,
    url_patterns     TEXT,
    color            TEXT,
    is_auto_detected INTEGER NOT NULL DEFAULT 1,
    created_at       TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ── Intelligence layer ───────────────────────────────────────────────────────
-- tracking rules, sessions, quality scores, forecasts, narratives,
-- voice journals, categorization cache, privacy rules, rule evolution

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

CREATE TABLE IF NOT EXISTS productivity_quality_scores (
    id                  TEXT PRIMARY KEY,
    score_date          TEXT NOT NULL,
    session_id          TEXT REFERENCES productivity_sessions(id) ON DELETE CASCADE,
    overall_score       REAL NOT NULL,
    focus_depth         REAL NOT NULL DEFAULT 0.0,
    okr_alignment       REAL NOT NULL DEFAULT 0.0,
    distraction_inv     REAL NOT NULL DEFAULT 0.0,
    task_completion     REAL NOT NULL DEFAULT 0.0,
    continuity          REAL NOT NULL DEFAULT 0.0,
    weights_json        TEXT,
    explanation         TEXT,
    created_at          TEXT NOT NULL DEFAULT (datetime('now')),
    deep_work_ratio     REAL NOT NULL DEFAULT 0.0,
    avg_session_length  REAL NOT NULL DEFAULT 0.0,
    meeting_focus_ratio REAL NOT NULL DEFAULT 0.0
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_quality_daily ON productivity_quality_scores(score_date) WHERE session_id IS NULL;
CREATE INDEX IF NOT EXISTS idx_quality_session ON productivity_quality_scores(session_id);

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

CREATE TABLE IF NOT EXISTS productivity_narratives (
    id                 TEXT PRIMARY KEY,
    narrative_date     TEXT NOT NULL UNIQUE,
    narrative_text     TEXT NOT NULL,
    key_moments        TEXT,
    sentiment          TEXT CHECK (sentiment IN ('positive', 'neutral', 'warning', 'negative')),
    total_focus_mins   INTEGER NOT NULL DEFAULT 0,
    total_meeting_mins INTEGER NOT NULL DEFAULT 0,
    total_break_mins   INTEGER NOT NULL DEFAULT 0,
    quality_score      REAL,
    top_categories     TEXT,
    created_at         TEXT NOT NULL DEFAULT (datetime('now'))
);

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

CREATE TABLE IF NOT EXISTS productivity_privacy_rules (
    id          TEXT PRIMARY KEY,
    rule_type   TEXT NOT NULL CHECK (rule_type IN ('exclude_app', 'redact_title', 'exclude_url')),
    pattern     TEXT NOT NULL,
    match_mode  TEXT NOT NULL DEFAULT 'exact' CHECK (match_mode IN ('exact', 'prefix', 'contains', 'regex')),
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

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

-- ── Calendar events ──────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS calendar_events (
    id              TEXT PRIMARY KEY,
    calendar_id     TEXT NOT NULL DEFAULT 'primary',
    title           TEXT NOT NULL,
    description     TEXT,
    started_at      TEXT NOT NULL,
    ended_at        TEXT NOT NULL,
    location        TEXT,
    attendees_count INTEGER DEFAULT 0,
    is_recurring    INTEGER DEFAULT 0,
    recurrence_id   TEXT,
    source          TEXT NOT NULL DEFAULT 'google',
    external_uid    TEXT NOT NULL,
    session_id      TEXT REFERENCES productivity_sessions(id),
    color           TEXT,
    synced_at       TEXT NOT NULL DEFAULT (datetime('now')),
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(external_uid, calendar_id)
);

CREATE INDEX IF NOT EXISTS idx_calendar_events_time ON calendar_events(started_at, ended_at);
CREATE INDEX IF NOT EXISTS idx_calendar_events_date ON calendar_events(date(started_at));
CREATE INDEX IF NOT EXISTS idx_calendar_events_session ON calendar_events(session_id) WHERE session_id IS NOT NULL;

-- ── Weekly assessments ───────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS weekly_assessments (
    id                     TEXT PRIMARY KEY,
    week_start             TEXT NOT NULL,
    week_end               TEXT NOT NULL,
    avg_score              REAL,
    total_focus_mins       INTEGER,
    total_productive_secs  INTEGER,
    total_distracting_secs INTEGER,
    top_apps               TEXT,
    summary                TEXT,
    created_at             TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_weekly_assessments_week ON weekly_assessments(week_start);

-- ── Seed: tracking rules ─────────────────────────────────────────────────────

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
