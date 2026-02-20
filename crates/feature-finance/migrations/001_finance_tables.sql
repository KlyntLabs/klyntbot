-- Finance feature migration: create all finance tables
-- All statements use IF NOT EXISTS for idempotency.

CREATE TABLE IF NOT EXISTS finance_accounts (
    id VARCHAR(36) PRIMARY KEY,
    name TEXT NOT NULL,
    account_type VARCHAR(50) NOT NULL,
    currency VARCHAR(10) NOT NULL,
    balance BIGINT NOT NULL DEFAULT 0,
    institution TEXT,
    notes TEXT,
    is_archived BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS finance_transactions (
    id VARCHAR(36) PRIMARY KEY,
    account_id VARCHAR(36) NOT NULL REFERENCES finance_accounts(id) ON DELETE CASCADE,
    tx_type VARCHAR(50) NOT NULL,
    amount BIGINT NOT NULL,
    currency VARCHAR(10) NOT NULL,
    category TEXT,
    subcategory TEXT,
    counterparty TEXT,
    notes TEXT,
    tx_date DATE NOT NULL,
    transfer_id VARCHAR(36),
    is_recurring BOOLEAN NOT NULL DEFAULT FALSE,
    recurring_rule TEXT,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_finance_transactions_account_id
    ON finance_transactions (account_id);
CREATE INDEX IF NOT EXISTS idx_finance_transactions_tx_date
    ON finance_transactions (tx_date);

CREATE TABLE IF NOT EXISTS finance_budgets (
    id VARCHAR(36) PRIMARY KEY,
    name TEXT NOT NULL,
    amount BIGINT NOT NULL,
    currency VARCHAR(10) NOT NULL,
    period VARCHAR(50) NOT NULL,
    category TEXT,
    method VARCHAR(50) NOT NULL DEFAULT 'standard',
    jar_type VARCHAR(50),
    start_date DATE NOT NULL,
    end_date DATE,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    alert_threshold INTEGER NOT NULL DEFAULT 80,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS finance_portfolios (
    id VARCHAR(36) PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    currency VARCHAR(10) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS finance_investments (
    id VARCHAR(36) PRIMARY KEY,
    portfolio_id VARCHAR(36) NOT NULL REFERENCES finance_portfolios(id) ON DELETE CASCADE,
    asset_type VARCHAR(50) NOT NULL,
    symbol TEXT,
    name TEXT NOT NULL,
    quantity DOUBLE PRECISION NOT NULL,
    cost_basis BIGINT NOT NULL,
    currency VARCHAR(10) NOT NULL,
    current_price BIGINT,
    current_value BIGINT,
    purchase_date DATE,
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_finance_investments_portfolio_id
    ON finance_investments (portfolio_id);

CREATE TABLE IF NOT EXISTS finance_investment_txs (
    id VARCHAR(36) PRIMARY KEY,
    investment_id VARCHAR(36) NOT NULL REFERENCES finance_investments(id) ON DELETE CASCADE,
    tx_type VARCHAR(50) NOT NULL,
    quantity DOUBLE PRECISION,
    price_per_unit BIGINT,
    total_amount BIGINT NOT NULL,
    currency VARCHAR(10) NOT NULL,
    fees BIGINT NOT NULL DEFAULT 0,
    tx_date DATE NOT NULL,
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS finance_goals (
    id VARCHAR(36) PRIMARY KEY,
    name TEXT NOT NULL,
    goal_type VARCHAR(50) NOT NULL,
    target_amount BIGINT NOT NULL,
    current_amount BIGINT NOT NULL DEFAULT 0,
    currency VARCHAR(10) NOT NULL,
    status VARCHAR(50) NOT NULL DEFAULT 'active',
    deadline DATE,
    monthly_contribution BIGINT,
    expected_return_rate DOUBLE PRECISION,
    inflation_rate DOUBLE PRECISION,
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS finance_liabilities (
    id VARCHAR(36) PRIMARY KEY,
    name TEXT NOT NULL,
    liability_type VARCHAR(50) NOT NULL,
    principal BIGINT NOT NULL,
    remaining BIGINT NOT NULL,
    currency VARCHAR(10) NOT NULL,
    interest_rate DOUBLE PRECISION,
    monthly_payment BIGINT,
    due_date DATE,
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);
