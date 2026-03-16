-- Feature migration: finance tables (IF NOT EXISTS — core migration owns these)
CREATE TABLE IF NOT EXISTS finance_accounts (
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

CREATE INDEX IF NOT EXISTS idx_finance_accounts_currency ON finance_accounts(currency);
CREATE INDEX IF NOT EXISTS idx_finance_accounts_is_archived ON finance_accounts(is_archived) WHERE is_archived = 0;

CREATE TABLE IF NOT EXISTS finance_transactions (
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

CREATE INDEX IF NOT EXISTS idx_finance_tx_account_id ON finance_transactions(account_id);
CREATE INDEX IF NOT EXISTS idx_finance_tx_tx_date ON finance_transactions(tx_date);
CREATE INDEX IF NOT EXISTS idx_finance_tx_tx_type ON finance_transactions(tx_type);
CREATE INDEX IF NOT EXISTS idx_finance_tx_category ON finance_transactions(category);
CREATE INDEX IF NOT EXISTS idx_finance_tx_transfer ON finance_transactions(transfer_id) WHERE transfer_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS finance_budgets (
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

CREATE INDEX IF NOT EXISTS idx_finance_budgets_is_active ON finance_budgets(is_active) WHERE is_active = 1;
CREATE INDEX IF NOT EXISTS idx_finance_budgets_category ON finance_budgets(category);

CREATE TABLE IF NOT EXISTS finance_portfolios (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    description TEXT,
    currency    TEXT NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE TABLE IF NOT EXISTS finance_investments (
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

CREATE INDEX IF NOT EXISTS idx_finance_investments_portfolio_id ON finance_investments(portfolio_id);
CREATE INDEX IF NOT EXISTS idx_finance_investments_symbol ON finance_investments(symbol) WHERE symbol IS NOT NULL;

CREATE TABLE IF NOT EXISTS finance_investment_transactions (
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

CREATE INDEX IF NOT EXISTS idx_finance_inv_txs_investment_id ON finance_investment_transactions(investment_id);
CREATE INDEX IF NOT EXISTS idx_finance_inv_txs_tx_date ON finance_investment_transactions(tx_date);

CREATE TABLE IF NOT EXISTS finance_goals (
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

CREATE INDEX IF NOT EXISTS idx_finance_goals_status ON finance_goals(status);

CREATE TABLE IF NOT EXISTS finance_liabilities (
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

CREATE INDEX IF NOT EXISTS idx_finance_liabilities_currency ON finance_liabilities(currency);

-- Exchange rate cache
CREATE TABLE IF NOT EXISTS finance_exchange_rates (
    from_currency  TEXT NOT NULL,
    to_currency    TEXT NOT NULL,
    rate           REAL NOT NULL,
    fetched_at     TEXT NOT NULL,
    PRIMARY KEY (from_currency, to_currency)
);
CREATE INDEX IF NOT EXISTS idx_exchange_rates_staleness
    ON finance_exchange_rates (to_currency, from_currency, fetched_at);

-- Allocation targets
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

-- Net worth snapshots
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
