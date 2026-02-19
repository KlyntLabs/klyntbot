-- Finance module tables: accounts, transactions, budgets, portfolios,
-- investments, investment_transactions, goals, liabilities.
-- All monetary amounts stored as BIGINT in smallest currency unit.

-- ============================================================
-- Finance Accounts
-- ============================================================
CREATE TABLE finance_accounts (
    id           TEXT PRIMARY KEY,
    name         TEXT NOT NULL,
    account_type TEXT NOT NULL, -- cash, bank, ewallet, crypto_wallet, brokerage, other
    currency     TEXT NOT NULL,
    balance      BIGINT NOT NULL DEFAULT 0,
    institution  TEXT,
    notes        TEXT,
    is_archived  BOOLEAN NOT NULL DEFAULT FALSE,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_finance_accounts_currency    ON finance_accounts(currency);
CREATE INDEX idx_finance_accounts_is_archived ON finance_accounts(is_archived) WHERE is_archived = FALSE;

-- ============================================================
-- Finance Transactions
-- ============================================================
CREATE TABLE finance_transactions (
    id             TEXT PRIMARY KEY,
    account_id     TEXT NOT NULL REFERENCES finance_accounts(id) ON DELETE CASCADE,
    tx_type        TEXT NOT NULL, -- income, expense, transfer
    amount         BIGINT NOT NULL,
    currency       TEXT NOT NULL,
    category       TEXT,
    subcategory    TEXT,
    counterparty   TEXT,
    notes          TEXT,
    tx_date        DATE NOT NULL DEFAULT CURRENT_DATE,
    transfer_id    TEXT,
    is_recurring   BOOLEAN NOT NULL DEFAULT FALSE,
    recurring_rule TEXT,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_finance_tx_account_id ON finance_transactions(account_id);
CREATE INDEX idx_finance_tx_tx_date    ON finance_transactions(tx_date);
CREATE INDEX idx_finance_tx_tx_type    ON finance_transactions(tx_type);
CREATE INDEX idx_finance_tx_category   ON finance_transactions(category);
CREATE INDEX idx_finance_tx_transfer   ON finance_transactions(transfer_id) WHERE transfer_id IS NOT NULL;

-- ============================================================
-- Finance Budgets
-- ============================================================
CREATE TABLE finance_budgets (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    amount          BIGINT NOT NULL,
    currency        TEXT NOT NULL,
    period          TEXT NOT NULL, -- monthly, weekly, yearly, custom
    category        TEXT,          -- NULL = total budget across all categories
    method          TEXT NOT NULL DEFAULT 'standard', -- standard, six_jar
    jar_type        TEXT,
    start_date      DATE NOT NULL DEFAULT CURRENT_DATE,
    end_date        DATE,
    is_active       BOOLEAN NOT NULL DEFAULT TRUE,
    alert_threshold INT NOT NULL DEFAULT 80,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_finance_budgets_is_active ON finance_budgets(is_active) WHERE is_active = TRUE;
CREATE INDEX idx_finance_budgets_category  ON finance_budgets(category);

-- ============================================================
-- Finance Portfolios
-- ============================================================
CREATE TABLE finance_portfolios (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    description TEXT,
    currency    TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- ============================================================
-- Finance Investments
-- ============================================================
CREATE TABLE finance_investments (
    id            TEXT PRIMARY KEY,
    portfolio_id  TEXT NOT NULL REFERENCES finance_portfolios(id) ON DELETE CASCADE,
    asset_type    TEXT NOT NULL, -- stock, etf, crypto, real_estate, bond, other
    symbol        TEXT,
    name          TEXT NOT NULL,
    quantity      DOUBLE PRECISION NOT NULL,
    cost_basis    BIGINT NOT NULL,
    currency      TEXT NOT NULL,
    current_price BIGINT,
    current_value BIGINT,
    purchase_date DATE,
    notes         TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_finance_investments_portfolio_id ON finance_investments(portfolio_id);
CREATE INDEX idx_finance_investments_symbol        ON finance_investments(symbol) WHERE symbol IS NOT NULL;

-- ============================================================
-- Finance Investment Transactions
-- ============================================================
CREATE TABLE finance_investment_transactions (
    id            TEXT PRIMARY KEY,
    investment_id TEXT NOT NULL REFERENCES finance_investments(id) ON DELETE CASCADE,
    tx_type       TEXT NOT NULL, -- buy, sell, dividend, rental_income, interest, split
    quantity      DOUBLE PRECISION,
    price_per_unit BIGINT,
    total_amount  BIGINT NOT NULL,
    currency      TEXT NOT NULL,
    fees          BIGINT NOT NULL DEFAULT 0,
    tx_date       DATE NOT NULL DEFAULT CURRENT_DATE,
    notes         TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_finance_inv_txs_investment_id ON finance_investment_transactions(investment_id);
CREATE INDEX idx_finance_inv_txs_tx_date        ON finance_investment_transactions(tx_date);

-- ============================================================
-- Finance Goals
-- ============================================================
CREATE TABLE finance_goals (
    id                   TEXT PRIMARY KEY,
    name                 TEXT NOT NULL,
    goal_type            TEXT NOT NULL, -- savings, purchase, debt_payoff, fire, custom
    target_amount        BIGINT NOT NULL,
    current_amount       BIGINT NOT NULL DEFAULT 0,
    currency             TEXT NOT NULL,
    status               TEXT NOT NULL DEFAULT 'active', -- active, achieved, abandoned
    deadline             DATE,
    monthly_contribution BIGINT,
    expected_return_rate DOUBLE PRECISION,
    inflation_rate       DOUBLE PRECISION,
    notes                TEXT,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_finance_goals_status ON finance_goals(status);

-- ============================================================
-- Finance Liabilities
-- ============================================================
CREATE TABLE finance_liabilities (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    liability_type  TEXT NOT NULL, -- mortgage, credit_card, personal_loan, student_loan, other
    principal       BIGINT NOT NULL,
    remaining       BIGINT NOT NULL,
    currency        TEXT NOT NULL,
    interest_rate   DOUBLE PRECISION,
    monthly_payment BIGINT,
    due_date        DATE,
    notes           TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_finance_liabilities_currency ON finance_liabilities(currency);
