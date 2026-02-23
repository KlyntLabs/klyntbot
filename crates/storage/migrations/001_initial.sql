-- SQLite baseline: all relational tables (embeddings moved to LanceDB)
PRAGMA foreign_keys = ON;

-- ============================================================
-- Projects
-- ============================================================
CREATE TABLE projects (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    description TEXT,
    color       TEXT NOT NULL DEFAULT 'orange',
    tags        TEXT NOT NULL DEFAULT '[]',
    status      TEXT NOT NULL DEFAULT 'active',
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- ============================================================
-- Todos
-- ============================================================
CREATE TABLE todos (
    id                   TEXT PRIMARY KEY,
    title                TEXT NOT NULL,
    description          TEXT,
    priority             INTEGER,
    due_date             TEXT,
    tags                 TEXT NOT NULL DEFAULT '[]',
    status               TEXT NOT NULL DEFAULT 'todo',
    focused_at           TEXT,
    focus_deadline       TEXT,
    focus_expired_count  INTEGER NOT NULL DEFAULT 0,
    created_at           TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at           TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    completed_at         TEXT,
    parent_id            TEXT REFERENCES todos(id) ON DELETE SET NULL,
    project_id           TEXT REFERENCES projects(id) ON DELETE SET NULL,
    total_tracked_secs   INTEGER NOT NULL DEFAULT 0,
    estimated_minutes    INTEGER,
    calendar_event_uid   TEXT,
    last_reminded_at     TEXT,
    recurrence_rule      TEXT,
    recurrence_parent_id TEXT,
    is_template          INTEGER NOT NULL DEFAULT 0,
    next_instance_date   TEXT
);

CREATE INDEX idx_todos_status ON todos(status);
CREATE INDEX idx_todos_project_id ON todos(project_id);
CREATE INDEX idx_todos_parent_id ON todos(parent_id);
CREATE INDEX idx_todos_due_date ON todos(due_date);
CREATE INDEX idx_todos_focused_at ON todos(focused_at) WHERE focused_at IS NOT NULL;
CREATE INDEX idx_todos_is_template ON todos(is_template) WHERE is_template = 1;

-- ============================================================
-- Todo Attachments
-- ============================================================
CREATE TABLE todo_attachments (
    id              TEXT PRIMARY KEY,
    todo_id         TEXT NOT NULL REFERENCES todos(id) ON DELETE CASCADE,
    attachment_type TEXT NOT NULL,
    value           TEXT NOT NULL,
    title           TEXT,
    tags            TEXT NOT NULL DEFAULT '[]',
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX idx_todo_attachments_todo_id ON todo_attachments(todo_id);

-- ============================================================
-- Todo Time Entries
-- ============================================================
CREATE TABLE todo_time_entries (
    id            TEXT PRIMARY KEY,
    todo_id       TEXT NOT NULL REFERENCES todos(id) ON DELETE CASCADE,
    source        TEXT NOT NULL DEFAULT 'focus',
    started_at    TEXT NOT NULL,
    ended_at      TEXT,
    duration_secs INTEGER,
    note          TEXT
);

CREATE INDEX idx_todo_time_entries_todo_id ON todo_time_entries(todo_id);

-- ============================================================
-- Todo Dependencies
-- ============================================================
CREATE TABLE todo_dependencies (
    task_id    TEXT NOT NULL REFERENCES todos(id) ON DELETE CASCADE,
    blocker_id TEXT NOT NULL REFERENCES todos(id) ON DELETE CASCADE,
    PRIMARY KEY (task_id, blocker_id),
    CHECK (task_id != blocker_id)
);

CREATE INDEX idx_todo_dependencies_blocker_id ON todo_dependencies(blocker_id);

-- ============================================================
-- Sessions
-- ============================================================
CREATE TABLE sessions (
    key        TEXT PRIMARY KEY,
    metadata   TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- ============================================================
-- Session Messages
-- (includes tool_calls + metadata columns from later migration)
-- ============================================================
CREATE TABLE session_messages (
    id          TEXT PRIMARY KEY,
    session_key TEXT NOT NULL REFERENCES sessions(key) ON DELETE CASCADE,
    role        TEXT NOT NULL,
    content     TEXT NOT NULL,
    timestamp   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    request_id  TEXT,
    tool_calls  TEXT,
    metadata    TEXT
);

CREATE INDEX idx_session_messages_key_ts ON session_messages(session_key, timestamp);

-- ============================================================
-- Goals
-- ============================================================
CREATE TABLE goals (
    id          TEXT PRIMARY KEY,
    title       TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    status      TEXT NOT NULL DEFAULT 'Active',
    priority    INTEGER NOT NULL DEFAULT 3,
    target_date TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    metrics     TEXT NOT NULL DEFAULT '[]',
    metadata    TEXT NOT NULL DEFAULT '{}'
);

-- ============================================================
-- Goal <-> Project Links (many-to-many)
-- ============================================================
CREATE TABLE goal_project_links (
    goal_id    TEXT NOT NULL REFERENCES goals(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    PRIMARY KEY (goal_id, project_id)
);

-- ============================================================
-- Plans
-- ============================================================
CREATE TABLE plans (
    id                 TEXT PRIMARY KEY,
    session_key        TEXT NOT NULL,
    goal_id            TEXT REFERENCES goals(id) ON DELETE SET NULL,
    title              TEXT NOT NULL,
    description        TEXT NOT NULL DEFAULT '',
    status             TEXT NOT NULL DEFAULT 'Draft',
    current_step_index INTEGER NOT NULL DEFAULT 0,
    iteration_limit    INTEGER NOT NULL DEFAULT 50,
    backtrack_history  TEXT NOT NULL DEFAULT '[]',
    created_at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    completed_at       TEXT
);

CREATE INDEX idx_plans_session_status ON plans(session_key, status);

-- ============================================================
-- Plan Steps
-- ============================================================
CREATE TABLE plan_steps (
    id             TEXT PRIMARY KEY,
    plan_id        TEXT NOT NULL REFERENCES plans(id) ON DELETE CASCADE,
    step_index     INTEGER NOT NULL,
    description    TEXT NOT NULL,
    reasoning      TEXT NOT NULL DEFAULT '',
    expected_tools TEXT NOT NULL DEFAULT '[]',
    status         TEXT NOT NULL DEFAULT 'Pending',
    attempt_count  INTEGER NOT NULL DEFAULT 0,
    max_attempts   INTEGER NOT NULL DEFAULT 3,
    result         TEXT,
    started_at     TEXT,
    completed_at   TEXT
);

CREATE INDEX idx_plan_steps_plan_id ON plan_steps(plan_id);

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
-- Strategy Records
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
    response_time_ms   INTEGER NOT NULL DEFAULT 0
);

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
    schedule         TEXT NOT NULL,
    payload          TEXT NOT NULL DEFAULT '{}',
    next_run_at_ms   INTEGER,
    last_run_at_ms   INTEGER,
    last_status      TEXT,
    last_error       TEXT,
    created_at_ms    INTEGER NOT NULL DEFAULT 0,
    updated_at_ms    INTEGER NOT NULL DEFAULT 0,
    delete_after_run INTEGER NOT NULL DEFAULT 0
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
-- Memory Notes
-- ============================================================
CREATE TABLE memory_notes (
    note_key   TEXT PRIMARY KEY,
    content    TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

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
    created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
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
    updated_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
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
    updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
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
    quantity      REAL NOT NULL,
    cost_basis    INTEGER NOT NULL,
    currency      TEXT NOT NULL,
    current_price INTEGER,
    current_value INTEGER,
    purchase_date TEXT,
    notes         TEXT,
    created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
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
    created_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
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
    updated_at           TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
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
    updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX idx_finance_liabilities_currency ON finance_liabilities(currency);

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
