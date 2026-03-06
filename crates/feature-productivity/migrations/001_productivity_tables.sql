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
    -- ── App-based categories ────────────────────────────────────────────
    ('coding', 'Coding', 'productive',
     '{"appNames":["Visual Studio Code","Code","Xcode","IntelliJ IDEA","WebStorm","Terminal","iTerm2","Warp","Alacritty","kitty","Ghostty","Hyper","Rio","WezTerm","Cursor","Zed","Sublime Text","Vim","Neovim","Emacs","Nova","Fleet","Android Studio","PyCharm","GoLand","CLion","RustRover","DataGrip","Windsurf"],"bundleIds":["com.microsoft.VSCode","com.apple.Terminal","com.googlecode.iterm2","com.mitchellh.ghostty","com.todesktop.230313mzl4w4u92","dev.zed.Zed"],"urlPatterns":[]}',
     TRUE),
    ('communication', 'Communication', 'neutral',
     '{"appNames":["Slack","Discord","Telegram","WhatsApp","Messages","Microsoft Teams","Zoom","Signal","Lark","Feishu","Google Chat","Element","Wire"],"bundleIds":["com.tinyspeck.slackmacgap","com.hnc.Discord","ru.keepcoder.Telegram","com.facebook.archon"],"urlPatterns":["slack.com","discord.com","telegram.org","web.whatsapp.com","teams.microsoft.com","meet.google.com","zoom.us"]}',
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
