-- SQLite baseline: all relational tables (embeddings moved to LanceDB)
PRAGMA foreign_keys = ON;

-- ============================================================
-- PARA: Areas (top-level containers)
-- ============================================================
CREATE TABLE areas (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE,
    description TEXT,
    color       TEXT NOT NULL DEFAULT 'blue',
    icon        TEXT,
    position    INTEGER NOT NULL DEFAULT 0,
    status      TEXT NOT NULL DEFAULT 'active',
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- ============================================================
-- Status Workflows
-- ============================================================
CREATE TABLE IF NOT EXISTS status_workflows (
    id                TEXT PRIMARY KEY,
    name              TEXT NOT NULL,
    is_template       INTEGER NOT NULL DEFAULT 0,
    is_global_default INTEGER NOT NULL DEFAULT 0,
    created_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- ============================================================
-- Status Labels (belong to workflows)
-- ============================================================
CREATE TABLE IF NOT EXISTS status_labels (
    id           TEXT PRIMARY KEY,
    workflow_id  TEXT NOT NULL REFERENCES status_workflows(id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    color        TEXT NOT NULL DEFAULT '#6b7280',
    status_group TEXT NOT NULL CHECK(status_group IN ('not_started', 'active', 'done', 'stuck')),
    position     INTEGER NOT NULL DEFAULT 0,
    created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
CREATE INDEX idx_status_labels_workflow_id ON status_labels(workflow_id);

-- ============================================================
-- Projects (belong to areas)
-- ============================================================
CREATE TABLE projects (
    id          TEXT PRIMARY KEY,
    area_id     TEXT NOT NULL REFERENCES areas(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    description TEXT,
    color       TEXT NOT NULL DEFAULT 'orange',
    tags        TEXT NOT NULL DEFAULT '[]',
    status      TEXT NOT NULL DEFAULT 'active',
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    instructions    TEXT,
    ai_personality  TEXT,
    user_role       TEXT,
    start_date      TEXT,
    target_end_date TEXT,
    settings        TEXT,
    workflow_id TEXT REFERENCES status_workflows(id) ON DELETE SET NULL
);
CREATE INDEX idx_projects_area_id ON projects(area_id);
CREATE INDEX idx_projects_status ON projects(status);

-- ============================================================
-- OKR: Objectives (belong to projects)
-- ============================================================
CREATE TABLE objectives (
    id           TEXT PRIMARY KEY,
    project_id   TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    title        TEXT NOT NULL,
    description  TEXT,
    status       TEXT NOT NULL DEFAULT 'active',
    priority     INTEGER,
    due_date     TEXT,
    progress     REAL NOT NULL DEFAULT 0.0,
    created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    completed_at TEXT
);
CREATE INDEX idx_objectives_project_id ON objectives(project_id);
CREATE INDEX idx_objectives_status ON objectives(status);

-- ============================================================
-- OKR: Key Results (belong to objectives)
-- ============================================================
CREATE TABLE key_results (
    id            TEXT PRIMARY KEY,
    objective_id  TEXT NOT NULL REFERENCES objectives(id) ON DELETE CASCADE,
    title         TEXT NOT NULL,
    description   TEXT,
    status        TEXT NOT NULL DEFAULT 'active',
    tracking_mode TEXT NOT NULL DEFAULT 'action',
    target_value  REAL,
    current_value REAL NOT NULL DEFAULT 0.0,
    unit          TEXT,
    progress      REAL NOT NULL DEFAULT 0.0,
    due_date      TEXT,
    created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    completed_at  TEXT
);
CREATE INDEX idx_key_results_objective_id ON key_results(objective_id);
CREATE INDEX idx_key_results_status ON key_results(status);

-- ============================================================
-- Task Groups
-- ============================================================
CREATE TABLE IF NOT EXISTS task_groups (
    id          TEXT PRIMARY KEY,
    project_id  TEXT REFERENCES projects(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    color       TEXT,
    position    INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
CREATE INDEX idx_task_groups_project_id ON task_groups(project_id);

-- ============================================================
-- Sessions
-- ============================================================
CREATE TABLE sessions (
    key        TEXT PRIMARY KEY,
    metadata   TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    project_id        TEXT REFERENCES projects(id),
    conversation_type TEXT DEFAULT 'general',
    pinned            INTEGER DEFAULT 0,
    squad_id          TEXT
);
CREATE INDEX IF NOT EXISTS idx_sessions_squad ON sessions(squad_id);

-- ============================================================
-- Session Messages
-- ============================================================
CREATE TABLE session_messages (
    id          TEXT PRIMARY KEY,
    session_key TEXT NOT NULL REFERENCES sessions(key) ON DELETE CASCADE,
    role        TEXT NOT NULL,
    content     TEXT NOT NULL,
    timestamp   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    request_id  TEXT,
    tool_calls  TEXT,
    metadata    TEXT,
    persona_id  TEXT
);
CREATE INDEX idx_session_messages_key_ts ON session_messages(session_key, timestamp);

-- ============================================================
-- Session Memory (per-session scratchpad for conversation state)
-- ============================================================
CREATE TABLE session_memory (
    session_key TEXT PRIMARY KEY REFERENCES sessions(key) ON DELETE CASCADE,
    content     TEXT NOT NULL DEFAULT '',
    turn_count  INTEGER NOT NULL DEFAULT 0,
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- ============================================================
-- Learning Outcomes
-- ============================================================
CREATE TABLE learning_outcomes (
    id                     TEXT PRIMARY KEY,
    session_key            TEXT NOT NULL,
    tool_name              TEXT NOT NULL,
    success                INTEGER NOT NULL,
    error_category         TEXT,
    duration_ms            INTEGER NOT NULL,
    confidence_score       REAL,
    confidence_dimensions  TEXT,
    execution_mode         TEXT NOT NULL DEFAULT '"chat"',
    created_at             TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
CREATE INDEX idx_learning_outcomes_created_at ON learning_outcomes(created_at);

-- ============================================================
-- Strategy Records (includes columns from migrations 002-004)
-- ============================================================
CREATE TABLE strategy_records (
    id                 TEXT PRIMARY KEY,
    timestamp          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    request_id         TEXT NOT NULL,
    predicted_strategy TEXT NOT NULL,
    actual_strategy    TEXT NOT NULL,
    escalation_count   INTEGER NOT NULL DEFAULT 0,
    iterations_used    INTEGER NOT NULL DEFAULT 0,
    max_iterations     INTEGER NOT NULL DEFAULT 0,
    success            INTEGER NOT NULL,
    user_satisfaction  REAL,
    response_time_ms   INTEGER NOT NULL DEFAULT 0,
    chat_id            TEXT,
    tool_name          TEXT,
    tool_success       INTEGER,
    tool_duration_ms   INTEGER,
    complexity_signals TEXT NOT NULL DEFAULT '{}',
    execution_mode     TEXT,
    retrieved_memory_count INTEGER,
    rewrite_triggered  INTEGER DEFAULT 0,
    rewrite_source     TEXT,
    budget_exhausted   INTEGER DEFAULT 0,
    turns_used         INTEGER DEFAULT 0,
    loop_detected      INTEGER DEFAULT 0,
    loop_tools         TEXT,
    context_fill_pct   REAL
);
CREATE INDEX idx_strategy_records_chat_id ON strategy_records(chat_id);
CREATE INDEX idx_strategy_records_timestamp ON strategy_records(timestamp);

-- ============================================================
-- Enrichment Feedback
-- ============================================================
CREATE TABLE enrichment_feedback (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id         TEXT NOT NULL,
    field           TEXT NOT NULL,
    suggested_value TEXT NOT NULL,
    actual_value    TEXT,
    accepted        INTEGER NOT NULL,
    confidence      REAL NOT NULL,
    timestamp       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- ============================================================
-- Usage Records (cost tracking)
-- ============================================================
CREATE TABLE usage_records (
    id                 TEXT PRIMARY KEY,
    timestamp          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    request_id         TEXT NOT NULL,
    model              TEXT NOT NULL,
    provider           TEXT NOT NULL,
    prompt_tokens      INTEGER NOT NULL DEFAULT 0,
    completion_tokens  INTEGER NOT NULL DEFAULT 0,
    cache_read_tokens  INTEGER NOT NULL DEFAULT 0,
    cache_write_tokens INTEGER NOT NULL DEFAULT 0,
    estimated_cost_usd REAL NOT NULL DEFAULT 0,
    channel            TEXT NOT NULL DEFAULT '',
    strategy           TEXT NOT NULL DEFAULT ''
);
CREATE INDEX idx_usage_records_timestamp ON usage_records(timestamp);

-- ============================================================
-- Cron Jobs
-- ============================================================
CREATE TABLE cron_jobs (
    id               TEXT PRIMARY KEY,
    name             TEXT NOT NULL,
    enabled          INTEGER NOT NULL DEFAULT 1,
    origin           TEXT NOT NULL DEFAULT 'system',
    schedule         TEXT NOT NULL,
    payload          TEXT NOT NULL DEFAULT '{}',
    next_run_at_ms   INTEGER,
    last_run_at_ms   INTEGER,
    last_status      TEXT,
    last_error       TEXT,
    created_at_ms    INTEGER NOT NULL DEFAULT 0,
    updated_at_ms    INTEGER NOT NULL DEFAULT 0,
    delete_after_run INTEGER NOT NULL DEFAULT 0,
    intent_window    TEXT,
    intent_pending_since_ms INTEGER
);

-- ============================================================
-- DND Override (crash recovery for focus sessions)
-- ============================================================
CREATE TABLE dnd_override (
    id               INTEGER PRIMARY KEY DEFAULT 1,
    original_state   TEXT NOT NULL,
    overridden_at    TEXT NOT NULL,
    session_id       TEXT
);

-- ============================================================
-- Calendar Sync State
-- ============================================================
CREATE TABLE calendar_sync_state (
    provider_id  TEXT PRIMARY KEY,
    sync_token   TEXT,
    last_sync_at TEXT
);

-- ============================================================
-- Calendar Event Cache
-- ============================================================
CREATE TABLE calendar_event_cache (
    uid         TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    summary     TEXT NOT NULL,
    description TEXT,
    start_at    TEXT NOT NULL,
    end_at      TEXT NOT NULL,
    source      TEXT NOT NULL DEFAULT 'CalDAV',
    etag        TEXT,
    status      TEXT,
    cached_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    PRIMARY KEY (uid, provider_id)
);
CREATE INDEX idx_calendar_event_cache_provider ON calendar_event_cache(provider_id);
CREATE INDEX idx_calendar_event_cache_start ON calendar_event_cache(start_at);
CREATE INDEX idx_calendar_event_cache_uid ON calendar_event_cache(uid);

-- ============================================================
-- Learning State (key-value store)
-- ============================================================
CREATE TABLE learning_state (
    key        TEXT PRIMARY KEY,
    value      TEXT NOT NULL DEFAULT '{}',
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- ============================================================
-- Decision Log
-- ============================================================
CREATE TABLE decision_log (
    id                    TEXT PRIMARY KEY,
    session_key           TEXT NOT NULL,
    iteration             INTEGER NOT NULL,
    tool_names            TEXT NOT NULL DEFAULT '[]',
    user_message_preview  TEXT NOT NULL DEFAULT '',
    assessment            TEXT NOT NULL DEFAULT '{}',
    outcome               TEXT,
    created_at            TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
CREATE INDEX idx_decision_log_session ON decision_log(session_key);
CREATE INDEX idx_decision_log_created ON decision_log(created_at DESC);

-- ============================================================
-- Finance Accounts
-- ============================================================
CREATE TABLE finance_accounts (
    id           TEXT PRIMARY KEY,
    name         TEXT NOT NULL,
    account_type TEXT NOT NULL,
    currency     TEXT NOT NULL,
    balance      INTEGER NOT NULL DEFAULT 0,
    institution  TEXT,
    notes        TEXT,
    is_archived  INTEGER NOT NULL DEFAULT 0,
    created_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    base_balance   INTEGER NOT NULL DEFAULT 0,
    base_currency  TEXT NOT NULL DEFAULT 'USD',
    exchange_rate  REAL NOT NULL DEFAULT 1.0
);
CREATE INDEX idx_finance_accounts_currency ON finance_accounts(currency);
CREATE INDEX idx_finance_accounts_is_archived ON finance_accounts(is_archived) WHERE is_archived = 0;

-- ============================================================
-- Finance Transactions
-- ============================================================
CREATE TABLE finance_transactions (
    id             TEXT PRIMARY KEY,
    account_id     TEXT NOT NULL REFERENCES finance_accounts(id) ON DELETE CASCADE,
    tx_type        TEXT NOT NULL,
    amount         INTEGER NOT NULL,
    currency       TEXT NOT NULL,
    category       TEXT,
    subcategory    TEXT,
    counterparty   TEXT,
    notes          TEXT,
    tx_date        TEXT NOT NULL DEFAULT (date('now')),
    transfer_id    TEXT,
    is_recurring   INTEGER NOT NULL DEFAULT 0,
    recurring_rule TEXT,
    created_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    base_amount    INTEGER NOT NULL DEFAULT 0,
    base_currency  TEXT NOT NULL DEFAULT 'USD',
    exchange_rate  REAL NOT NULL DEFAULT 1.0
);
CREATE INDEX idx_finance_tx_account_id ON finance_transactions(account_id);
CREATE INDEX idx_finance_tx_tx_date ON finance_transactions(tx_date);
CREATE INDEX idx_finance_tx_tx_type ON finance_transactions(tx_type);
CREATE INDEX idx_finance_tx_category ON finance_transactions(category);
CREATE INDEX idx_finance_tx_transfer ON finance_transactions(transfer_id) WHERE transfer_id IS NOT NULL;

-- ============================================================
-- Finance Budgets
-- ============================================================
CREATE TABLE finance_budgets (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    amount          INTEGER NOT NULL,
    currency        TEXT NOT NULL,
    period          TEXT NOT NULL,
    category        TEXT,
    method          TEXT NOT NULL DEFAULT 'standard',
    jar_type        TEXT,
    start_date      TEXT NOT NULL DEFAULT (date('now')),
    end_date        TEXT,
    is_active       INTEGER NOT NULL DEFAULT 1,
    alert_threshold INTEGER NOT NULL DEFAULT 80,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    base_amount     INTEGER NOT NULL DEFAULT 0,
    base_currency   TEXT NOT NULL DEFAULT 'USD',
    exchange_rate   REAL NOT NULL DEFAULT 1.0
);
CREATE INDEX idx_finance_budgets_is_active ON finance_budgets(is_active) WHERE is_active = 1;
CREATE INDEX idx_finance_budgets_category ON finance_budgets(category);

-- ============================================================
-- Finance Portfolios
-- ============================================================
CREATE TABLE finance_portfolios (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    description TEXT,
    currency    TEXT NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- ============================================================
-- Finance Investments
-- ============================================================
CREATE TABLE finance_investments (
    id            TEXT PRIMARY KEY,
    portfolio_id  TEXT NOT NULL REFERENCES finance_portfolios(id) ON DELETE CASCADE,
    asset_type    TEXT NOT NULL,
    symbol        TEXT,
    name          TEXT NOT NULL,
    quantity      TEXT NOT NULL,
    cost_basis    INTEGER NOT NULL,
    currency      TEXT NOT NULL,
    current_price INTEGER,
    current_value INTEGER,
    purchase_date TEXT,
    asset_class   TEXT,
    notes         TEXT,
    created_at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    market_currency    TEXT,
    base_cost_basis    INTEGER NOT NULL DEFAULT 0,
    base_current_value INTEGER NOT NULL DEFAULT 0,
    base_currency      TEXT NOT NULL DEFAULT 'USD',
    purchase_rate      REAL NOT NULL DEFAULT 1.0,
    market_rate        REAL NOT NULL DEFAULT 1.0
);
CREATE INDEX idx_finance_investments_portfolio_id ON finance_investments(portfolio_id);
CREATE INDEX idx_finance_investments_symbol ON finance_investments(symbol) WHERE symbol IS NOT NULL;

-- ============================================================
-- Finance Investment Transactions
-- ============================================================
CREATE TABLE finance_investment_transactions (
    id             TEXT PRIMARY KEY,
    investment_id  TEXT NOT NULL REFERENCES finance_investments(id) ON DELETE CASCADE,
    tx_type        TEXT NOT NULL,
    quantity       REAL,
    price_per_unit INTEGER,
    total_amount   INTEGER NOT NULL,
    currency       TEXT NOT NULL,
    fees           INTEGER NOT NULL DEFAULT 0,
    tx_date        TEXT NOT NULL DEFAULT (date('now')),
    notes          TEXT,
    created_at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    base_total_amount  INTEGER NOT NULL DEFAULT 0,
    base_currency      TEXT NOT NULL DEFAULT 'USD',
    exchange_rate      REAL NOT NULL DEFAULT 1.0
);
CREATE INDEX idx_finance_inv_txs_investment_id ON finance_investment_transactions(investment_id);
CREATE INDEX idx_finance_inv_txs_tx_date ON finance_investment_transactions(tx_date);

-- ============================================================
-- Finance Goals
-- ============================================================
CREATE TABLE finance_goals (
    id                   TEXT PRIMARY KEY,
    name                 TEXT NOT NULL,
    goal_type            TEXT NOT NULL,
    target_amount        INTEGER NOT NULL,
    current_amount       INTEGER NOT NULL DEFAULT 0,
    currency             TEXT NOT NULL,
    status               TEXT NOT NULL DEFAULT 'active',
    deadline             TEXT,
    monthly_contribution INTEGER,
    expected_return_rate REAL,
    inflation_rate       REAL,
    notes                TEXT,
    created_at           TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at           TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    base_target_amount   INTEGER NOT NULL DEFAULT 0,
    base_current_amount  INTEGER NOT NULL DEFAULT 0,
    base_currency        TEXT NOT NULL DEFAULT 'USD',
    exchange_rate        REAL NOT NULL DEFAULT 1.0
);
CREATE INDEX idx_finance_goals_status ON finance_goals(status);

-- ============================================================
-- Finance Liabilities
-- ============================================================
CREATE TABLE finance_liabilities (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    liability_type  TEXT NOT NULL,
    principal       INTEGER NOT NULL,
    remaining       INTEGER NOT NULL,
    currency        TEXT NOT NULL,
    interest_rate   REAL,
    monthly_payment INTEGER,
    due_date        TEXT,
    notes           TEXT,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    base_principal  INTEGER NOT NULL DEFAULT 0,
    base_remaining  INTEGER NOT NULL DEFAULT 0,
    base_currency   TEXT NOT NULL DEFAULT 'USD',
    exchange_rate   REAL NOT NULL DEFAULT 1.0
);
CREATE INDEX idx_finance_liabilities_currency ON finance_liabilities(currency);

-- ============================================================
-- Finance Exchange Rates (cache)
-- ============================================================
CREATE TABLE IF NOT EXISTS finance_exchange_rates (
    from_currency  TEXT NOT NULL,
    to_currency    TEXT NOT NULL,
    rate           REAL NOT NULL,
    fetched_at     TEXT NOT NULL,
    PRIMARY KEY (from_currency, to_currency)
);
CREATE INDEX IF NOT EXISTS idx_exchange_rates_staleness
    ON finance_exchange_rates (to_currency, from_currency, fetched_at);

-- ============================================================
-- Finance Allocation Targets
-- ============================================================
CREATE TABLE IF NOT EXISTS finance_allocation_targets (
    id TEXT PRIMARY KEY,
    portfolio_id TEXT NOT NULL REFERENCES finance_portfolios(id) ON DELETE CASCADE,
    asset_class TEXT NOT NULL,
    target_weight TEXT NOT NULL,
    tolerance_band TEXT NOT NULL DEFAULT '0.05',
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    UNIQUE(portfolio_id, asset_class)
);

-- ============================================================
-- Finance Net Worth Snapshots
-- ============================================================
CREATE TABLE IF NOT EXISTS finance_net_worth_snapshots (
    id TEXT PRIMARY KEY,
    snapshot_date TEXT NOT NULL,
    currency TEXT NOT NULL DEFAULT 'USD',
    accounts_total INTEGER NOT NULL,
    investments_total INTEGER NOT NULL,
    liabilities_total INTEGER NOT NULL,
    net_worth INTEGER NOT NULL,
    breakdown TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    UNIQUE(snapshot_date, currency)
);
CREATE INDEX IF NOT EXISTS idx_net_worth_snapshots_date ON finance_net_worth_snapshots(snapshot_date);
CREATE INDEX IF NOT EXISTS idx_net_worth_snapshots_currency_date ON finance_net_worth_snapshots(currency, snapshot_date);

-- ============================================================
-- Feature Migration Tracking
-- ============================================================
CREATE TABLE IF NOT EXISTS _feature_migrations (
    feature_name TEXT NOT NULL,
    version      INTEGER NOT NULL,
    description  TEXT NOT NULL DEFAULT '',
    applied_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    PRIMARY KEY (feature_name, version)
);

-- ============================================================
-- Agent Tasks (subagent coordination)
-- ============================================================
CREATE TABLE agent_tasks (
    id              TEXT PRIMARY KEY,
    session_key     TEXT NOT NULL,
    description     TEXT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'pending'
                    CHECK(status IN ('pending','claimed','running','completed','failed')),
    owner_agent_id  TEXT,
    parent_task_id  TEXT REFERENCES agent_tasks(id) ON DELETE CASCADE,
    result          TEXT,
    error           TEXT,
    blocked_by      TEXT NOT NULL DEFAULT '[]',
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
CREATE INDEX idx_agent_tasks_session ON agent_tasks(session_key);
CREATE INDEX idx_agent_tasks_status ON agent_tasks(status);
CREATE INDEX idx_agent_tasks_owner ON agent_tasks(owner_agent_id);

-- ============================================================
-- Tool Usage Analytics
-- ============================================================
CREATE TABLE tool_usage (
    id              TEXT PRIMARY KEY,
    tool_name       TEXT NOT NULL,
    action          TEXT,
    session_key     TEXT,
    channel         TEXT,
    intent_category TEXT,
    success         INTEGER NOT NULL DEFAULT 1,
    duration_ms     INTEGER,
    error_message   TEXT,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
CREATE INDEX idx_tool_usage_tool ON tool_usage(tool_name);
CREATE INDEX idx_tool_usage_created ON tool_usage(created_at);

-- ============================================================
-- Entity Links (cross-feature linking)
-- ============================================================
CREATE TABLE entity_links (
    id          TEXT PRIMARY KEY,
    source_kind TEXT NOT NULL,
    source_id   TEXT NOT NULL,
    target_kind TEXT NOT NULL,
    target_id   TEXT NOT NULL,
    link_type   TEXT NOT NULL DEFAULT 'related',
    metadata    TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    UNIQUE(source_kind, source_id, target_kind, target_id, link_type)
);

CREATE INDEX idx_entity_links_source ON entity_links(source_kind, source_id);
CREATE INDEX idx_entity_links_target ON entity_links(target_kind, target_id);

-- ============================================================
-- Project Sources (AI context sources per project)
-- ============================================================
CREATE TABLE project_sources (
    id          TEXT PRIMARY KEY,
    project_id  TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    source_type TEXT NOT NULL,
    title       TEXT NOT NULL,
    content     TEXT,
    url         TEXT,
    file_path   TEXT,
    embedding_id TEXT,
    metadata    TEXT,
    tags        TEXT DEFAULT '[]',
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX idx_project_sources_project ON project_sources(project_id);

-- ── Session context ──────────────────────────────────────────
CREATE TABLE IF NOT EXISTS session_context (
    session_key  TEXT PRIMARY KEY REFERENCES sessions(key) ON DELETE CASCADE,
    context_type TEXT NOT NULL DEFAULT 'general',
    entity_kind  TEXT,
    entity_id    TEXT,
    area_id      TEXT,
    project_id   TEXT,
    is_ephemeral INTEGER NOT NULL DEFAULT 0,
    is_pinned    INTEGER NOT NULL DEFAULT 0,
    created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_session_context_area ON session_context(area_id);
CREATE INDEX IF NOT EXISTS idx_session_context_entity ON session_context(entity_kind, entity_id);
CREATE INDEX IF NOT EXISTS idx_session_context_ephemeral ON session_context(is_ephemeral, is_pinned);

-- ── Interaction log ──────────────────────────────────────────
CREATE TABLE IF NOT EXISTS interaction_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL DEFAULT (datetime('now')),
    agent_name TEXT NOT NULL,
    tool_names TEXT NOT NULL DEFAULT '[]',
    channel TEXT NOT NULL,
    duration_ms INTEGER
);

CREATE INDEX IF NOT EXISTS idx_interaction_log_timestamp
    ON interaction_log(timestamp DESC);

-- ── Status workflows: seed data ──────────────────────────────

-- Global default workflow
INSERT INTO status_workflows (id, name, is_template, is_global_default)
VALUES ('wf_default', 'Default', 0, 1);

INSERT INTO status_labels (id, workflow_id, name, color, status_group, position) VALUES
    ('sl_backlog',     'wf_default', 'Backlog',     '#6b7280', 'not_started', 0),
    ('sl_todo',        'wf_default', 'Todo',        '#3b82f6', 'not_started', 1),
    ('sl_in_progress', 'wf_default', 'In Progress', '#eab308', 'active',      2),
    ('sl_in_review',   'wf_default', 'In Review',   '#f97316', 'active',      3),
    ('sl_done',        'wf_default', 'Done',        '#22c55e', 'done',        4),
    ('sl_blocked',     'wf_default', 'Blocked',     '#ef4444', 'stuck',       5);

-- Template: Simple
INSERT INTO status_workflows (id, name, is_template, is_global_default)
VALUES ('wf_simple', 'Simple', 1, 0);

INSERT INTO status_labels (id, workflow_id, name, color, status_group, position) VALUES
    ('sl_simple_todo',        'wf_simple', 'Todo',        '#3b82f6', 'not_started', 0),
    ('sl_simple_in_progress', 'wf_simple', 'In Progress', '#eab308', 'active',      1),
    ('sl_simple_done',        'wf_simple', 'Done',        '#22c55e', 'done',        2);

-- Template: Software Dev
INSERT INTO status_workflows (id, name, is_template, is_global_default)
VALUES ('wf_swdev', 'Software Dev', 1, 0);

INSERT INTO status_labels (id, workflow_id, name, color, status_group, position) VALUES
    ('sl_swdev_backlog',     'wf_swdev', 'Backlog',     '#6b7280', 'not_started', 0),
    ('sl_swdev_todo',        'wf_swdev', 'Todo',        '#3b82f6', 'not_started', 1),
    ('sl_swdev_in_progress', 'wf_swdev', 'In Progress', '#eab308', 'active',      2),
    ('sl_swdev_in_review',   'wf_swdev', 'In Review',   '#f97316', 'active',      3),
    ('sl_swdev_done',        'wf_swdev', 'Done',        '#22c55e', 'done',        4),
    ('sl_swdev_blocked',     'wf_swdev', 'Blocked',     '#ef4444', 'stuck',       5);

-- Template: Content Creation
INSERT INTO status_workflows (id, name, is_template, is_global_default)
VALUES ('wf_content', 'Content Creation', 1, 0);

INSERT INTO status_labels (id, workflow_id, name, color, status_group, position) VALUES
    ('sl_content_idea',      'wf_content', 'Idea',      '#a855f7', 'not_started', 0),
    ('sl_content_drafting',  'wf_content', 'Drafting',  '#eab308', 'active',      1),
    ('sl_content_editing',   'wf_content', 'Editing',   '#f97316', 'active',      2),
    ('sl_content_published', 'wf_content', 'Published', '#22c55e', 'done',        3);

-- ── Custom columns ───────────────────────────────────────────
CREATE TABLE IF NOT EXISTS custom_columns (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    column_type TEXT NOT NULL CHECK(column_type IN (
        'text','number','date','dropdown','multi_select','checkbox',
        'tags','link','rating','progress','duration','currency'
    )),
    options_json TEXT,
    position INTEGER NOT NULL DEFAULT 0,
    width INTEGER DEFAULT 150,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS custom_column_values (
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    column_id TEXT NOT NULL REFERENCES custom_columns(id) ON DELETE CASCADE,
    value_json TEXT NOT NULL,
    PRIMARY KEY (task_id, column_id)
);

-- Brain signal feedback (dedup, dismissal tracking, adaptive dampening)
CREATE TABLE IF NOT EXISTS brain_signal_feedback (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    signal_type TEXT NOT NULL,
    entity_pair TEXT NOT NULL,
    action      TEXT NOT NULL,
    timestamp   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_brain_signal_entity_pair ON brain_signal_feedback(entity_pair, timestamp);

-- Generic key-value store for user preferences / small persisted scalars
CREATE TABLE IF NOT EXISTS user_preferences (
    key        TEXT PRIMARY KEY,
    value      TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
CREATE INDEX IF NOT EXISTS idx_brain_signal_action ON brain_signal_feedback(action, timestamp);

-- Cross-domain insights generated nightly for morning briefing
CREATE TABLE IF NOT EXISTS cross_domain_insights (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    date        TEXT NOT NULL,
    insight_text TEXT NOT NULL,
    dot_refs    TEXT NOT NULL,
    surfaced    INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- ── Retrieval feedback ─────────────────────────────────────────
-- Tracks which retrieved facts the LLM referenced, for autotuner evaluation
CREATE TABLE IF NOT EXISTS retrieval_feedback (
    id TEXT PRIMARY KEY,
    retrieved_fact_ids TEXT NOT NULL,
    referenced_fact_ids TEXT NOT NULL,
    precision REAL NOT NULL,
    session_key TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    score_breakdown TEXT           -- JSON: per-component scores for the top-K facts
);
CREATE INDEX IF NOT EXISTS idx_retrieval_feedback_session ON retrieval_feedback(session_key);
CREATE INDEX IF NOT EXISTS idx_retrieval_feedback_created ON retrieval_feedback(created_at);

-- ============================================================
-- Reforge Cycle State
-- ============================================================
CREATE TABLE reforge_state (
    id              TEXT PRIMARY KEY DEFAULT 'singleton',
    last_run_at     TEXT,
    last_run_stats  TEXT,
    run_count       INTEGER NOT NULL DEFAULT 0
);

INSERT INTO reforge_state (id) VALUES ('singleton');

-- ============================================================
-- Skill Version History
-- ============================================================
CREATE TABLE skill_versions (
    id          TEXT PRIMARY KEY,
    skill_name  TEXT NOT NULL,
    version     INTEGER NOT NULL,
    file_path   TEXT NOT NULL,
    content     TEXT NOT NULL,
    diff        TEXT,
    source      TEXT NOT NULL,
    reason      TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX idx_skill_versions_name ON skill_versions(skill_name, version);
CREATE INDEX idx_skill_versions_lookup ON skill_versions(skill_name, file_path, version);

-- ============================================================
-- Reforge Self-Feedback: suggestions and patterns from previous cycles
-- ============================================================
CREATE TABLE IF NOT EXISTS reforge_suggestions (
    id TEXT PRIMARY KEY,
    suggestion_type TEXT NOT NULL,
    content TEXT NOT NULL,
    reason TEXT NOT NULL DEFAULT '',
    confidence REAL NOT NULL DEFAULT 0.0,
    cycle_run_at TEXT NOT NULL,
    acted_upon INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_reforge_suggestions_type
    ON reforge_suggestions(suggestion_type, created_at);

-- ============================================================
-- Response validation warnings from the agent output validator.
-- ============================================================
CREATE TABLE IF NOT EXISTS response_warnings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    request_id TEXT NOT NULL,
    warning_type TEXT NOT NULL,
    detail TEXT,
    chat_id TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_response_warnings_created
    ON response_warnings(created_at);
