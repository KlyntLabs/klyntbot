# Banking Sync Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add real-time bank transaction import via Casso/SePay webhooks into the existing finance feature, with smart notifications.

**Architecture:** Extend `feature-finance` with a `banking/` submodule. New axum HTTP server in `serve.rs` receives webhook POSTs, validates them, deduplicates via `bank_ref_id`, writes to `finance_transactions`, and evaluates notification rules. A `BankProvider` trait abstracts Casso/SePay (and future providers).

**Tech Stack:** Rust, axum 0.8, tower-http, sqlx (SQLite), reqwest, serde, tokio, chrono, regex

---

### Task 1: Add axum to workspace dependencies

**Files:**
- Modify: `Cargo.toml` (workspace root, lines 64-107)
- Modify: `crates/feature-finance/Cargo.toml`

**Step 1: Add axum + tower-http to workspace deps**

In `Cargo.toml`, after the `dashmap = "6"` line (line 107), add:

```toml
axum = "0.8"
tower = "0.5"
tower-http = { version = "0.6", features = ["cors", "limit"] }
```

**Step 2: Add to feature-finance Cargo.toml**

In `crates/feature-finance/Cargo.toml`, add to `[dependencies]`:

```toml
axum.workspace = true
tower.workspace = true
tower-http.workspace = true
regex.workspace = true
```

**Step 3: Verify it compiles**

Run: `cargo check -p feature-finance`
Expected: compiles with no errors

**Step 4: Commit**

```bash
git add Cargo.toml crates/feature-finance/Cargo.toml
git commit -m "chore(deps): add axum, tower-http for banking webhooks"
```

---

### Task 2: Database migration — new tables + alter finance_transactions

**Files:**
- Create: `crates/storage/migrations/003_banking_sync.sql`

**Step 1: Write the migration SQL**

```sql
-- ============================================================
-- Banking Sync
-- ============================================================

-- Linked bank accounts (connected via Casso/SePay)
CREATE TABLE IF NOT EXISTS banking_linked_accounts (
    id                    TEXT PRIMARY KEY,
    finance_account_id    TEXT NOT NULL REFERENCES finance_accounts(id) ON DELETE CASCADE,
    provider              TEXT NOT NULL,            -- 'casso' | 'sepay'
    provider_account_id   TEXT NOT NULL,            -- account ID from the provider
    bank_name             TEXT NOT NULL,            -- e.g. 'Vietcombank'
    account_number_masked TEXT NOT NULL,            -- e.g. '****6789'
    last_synced_at        TEXT,                     -- last successful webhook/sync
    is_active             INTEGER NOT NULL DEFAULT 1,
    created_at            TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX idx_banking_linked_accounts_finance_account
    ON banking_linked_accounts(finance_account_id);
CREATE INDEX idx_banking_linked_accounts_provider
    ON banking_linked_accounts(provider, provider_account_id);
CREATE UNIQUE INDEX idx_banking_linked_accounts_provider_unique
    ON banking_linked_accounts(provider, provider_account_id);

-- Notification rules for smart alerts
CREATE TABLE IF NOT EXISTS banking_notification_rules (
    id                 TEXT PRIMARY KEY,
    linked_account_id  TEXT REFERENCES banking_linked_accounts(id) ON DELETE CASCADE,
    rule_type          TEXT NOT NULL,               -- 'threshold' | 'pattern' | 'all'
    threshold_amount   INTEGER,                     -- notify if abs(amount) >= this
    pattern            TEXT,                         -- regex match on description
    channel            TEXT NOT NULL DEFAULT 'telegram',
    is_active          INTEGER NOT NULL DEFAULT 1,
    created_at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX idx_banking_notification_rules_account
    ON banking_notification_rules(linked_account_id);

-- Add source tracking to existing finance_transactions
ALTER TABLE finance_transactions ADD COLUMN source TEXT NOT NULL DEFAULT 'manual';
ALTER TABLE finance_transactions ADD COLUMN bank_ref_id TEXT;

CREATE UNIQUE INDEX idx_finance_transactions_bank_ref_id
    ON finance_transactions(bank_ref_id) WHERE bank_ref_id IS NOT NULL;
```

**Step 2: Verify migration applies on a fresh DB**

Run: `cargo nextest run -p storage -E 'test(connect)' --nocapture`
Expected: PASS (StoragePool::connect_in_memory runs all migrations)

**Step 3: Commit**

```bash
git add crates/storage/migrations/003_banking_sync.sql
git commit -m "feat(storage): add banking sync migration (linked accounts, notification rules)"
```

---

### Task 3: Row structs for banking tables

**Files:**
- Create: `crates/storage/src/rows/banking.rs`
- Modify: `crates/storage/src/rows/mod.rs` (add `pub mod banking;`)
- Modify: `crates/storage/src/rows/finance.rs` (add `source` + `bank_ref_id` to `FinanceTransactionRow`)

**Step 1: Write the test (add to existing storage test module)**

In `crates/storage/src/repos/tests/mod.rs`, ensure the test module can import banking row types. The actual repo tests come in Task 4. For now, verify the row structs compile.

**Step 2: Create `crates/storage/src/rows/banking.rs`**

```rust
//! Row structs for banking sync tables:
//! `banking_linked_accounts`, `banking_notification_rules`.

use chrono::{DateTime, Utc};
use sqlx::FromRow;

/// Row struct for the `banking_linked_accounts` table.
#[derive(Debug, Clone, FromRow)]
pub struct BankingLinkedAccountRow {
    pub id: String,
    pub finance_account_id: String,
    pub provider: String,
    pub provider_account_id: String,
    pub bank_name: String,
    pub account_number_masked: String,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

/// Row struct for the `banking_notification_rules` table.
#[derive(Debug, Clone, FromRow)]
pub struct BankingNotificationRuleRow {
    pub id: String,
    pub linked_account_id: Option<String>,
    pub rule_type: String,
    pub threshold_amount: Option<i64>,
    pub pattern: Option<String>,
    pub channel: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}
```

**Step 3: Register in `crates/storage/src/rows/mod.rs`**

Add `pub mod banking;` in alphabetical order (before `calendar`).

**Step 4: Add `source` and `bank_ref_id` to `FinanceTransactionRow`**

In `crates/storage/src/rows/finance.rs`, add to `FinanceTransactionRow` after `updated_at`:

```rust
    pub source: String,
    pub bank_ref_id: Option<String>,
```

Also update `FinanceTransactionPatch` — no change needed since these fields are set at insert time only (source is immutable, bank_ref_id is set once).

**Step 5: Verify it compiles**

Run: `cargo check -p storage`
Expected: compiles (may have warnings about unused fields — that's fine)

**Step 6: Commit**

```bash
git add crates/storage/src/rows/banking.rs crates/storage/src/rows/mod.rs crates/storage/src/rows/finance.rs
git commit -m "feat(storage): add banking row structs and source/bank_ref_id to transactions"
```

---

### Task 4: Banking repos (BankingLinkedAccountRepo + BankingNotificationRuleRepo)

**Files:**
- Create: `crates/storage/src/repos/banking_linked_account_repo.rs`
- Create: `crates/storage/src/repos/banking_notification_rule_repo.rs`
- Modify: `crates/storage/src/repos/mod.rs` (register + add to Repos)
- Create: `crates/storage/src/repos/tests/banking_linked_account_repo_tests.rs`
- Create: `crates/storage/src/repos/tests/banking_notification_rule_repo_tests.rs`

**Step 1: Write the failing test for BankingLinkedAccountRepo**

Create `crates/storage/src/repos/tests/banking_linked_account_repo_tests.rs`:

```rust
use crate::repos::BankingLinkedAccountRepo;
use crate::rows::banking::BankingLinkedAccountRow;
use crate::StoragePool;

#[tokio::test]
async fn test_banking_linked_account_crud() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repo = BankingLinkedAccountRepo::new(pool.inner().clone());

    // First create a finance account to link to
    let fa_repo = crate::repos::FinanceAccountRepo::new(pool.inner().clone());
    let fa = crate::rows::finance::FinanceAccountRow {
        id: "fa-1".to_string(),
        name: "VCB Checking".to_string(),
        account_type: "bank".to_string(),
        currency: "VND".to_string(),
        balance: 10_000_000,
        institution: Some("Vietcombank".to_string()),
        notes: None,
        is_archived: false,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        source: "manual".to_string(),
        bank_ref_id: None,
    };
    fa_repo.add(&fa).await.unwrap();

    // Add linked account
    let row = BankingLinkedAccountRow {
        id: "bla-1".to_string(),
        finance_account_id: "fa-1".to_string(),
        provider: "casso".to_string(),
        provider_account_id: "casso-acc-123".to_string(),
        bank_name: "Vietcombank".to_string(),
        account_number_masked: "****6789".to_string(),
        last_synced_at: None,
        is_active: true,
        created_at: chrono::Utc::now(),
    };
    let inserted = repo.add(&row).await.unwrap();
    assert_eq!(inserted.id, "bla-1");
    assert_eq!(inserted.provider, "casso");

    // Get
    let fetched = repo.get("bla-1").await.unwrap().unwrap();
    assert_eq!(fetched.bank_name, "Vietcombank");

    // Find by provider + account number
    let found = repo.find_by_provider_account("casso", "casso-acc-123").await.unwrap();
    assert!(found.is_some());

    // List active
    let active = repo.list_active().await.unwrap();
    assert_eq!(active.len(), 1);

    // Deactivate
    repo.set_active("bla-1", false).await.unwrap();
    let active = repo.list_active().await.unwrap();
    assert_eq!(active.len(), 0);

    // Update last_synced_at
    repo.update_last_synced("bla-1").await.unwrap();
    let updated = repo.get("bla-1").await.unwrap().unwrap();
    assert!(updated.last_synced_at.is_some());
}
```

Register in `crates/storage/src/repos/tests/mod.rs`:
```rust
mod banking_linked_account_repo_tests;
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p storage -E 'test(banking_linked_account_crud)'`
Expected: FAIL — `BankingLinkedAccountRepo` not found

**Step 3: Implement BankingLinkedAccountRepo**

Create `crates/storage/src/repos/banking_linked_account_repo.rs`:

```rust
//! Repository for the `banking_linked_accounts` table.

use sqlx::SqlitePool;

use crate::error::StorageError;
use crate::rows::banking::BankingLinkedAccountRow;

/// Repository for banking linked account CRUD and lookup.
#[derive(Debug, Clone)]
pub struct BankingLinkedAccountRepo {
    pool: SqlitePool,
}

impl BankingLinkedAccountRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn add(
        &self,
        row: &BankingLinkedAccountRow,
    ) -> Result<BankingLinkedAccountRow, StorageError> {
        let inserted = sqlx::query_as::<_, BankingLinkedAccountRow>(
            r#"
            INSERT INTO banking_linked_accounts (
                id, finance_account_id, provider, provider_account_id,
                bank_name, account_number_masked, last_synced_at, is_active, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            RETURNING *
            "#,
        )
        .bind(&row.id)
        .bind(&row.finance_account_id)
        .bind(&row.provider)
        .bind(&row.provider_account_id)
        .bind(&row.bank_name)
        .bind(&row.account_number_masked)
        .bind(row.last_synced_at)
        .bind(row.is_active)
        .bind(row.created_at)
        .fetch_one(&self.pool)
        .await?;
        Ok(inserted)
    }

    pub async fn get(
        &self,
        id: &str,
    ) -> Result<Option<BankingLinkedAccountRow>, StorageError> {
        let row = sqlx::query_as::<_, BankingLinkedAccountRow>(
            "SELECT * FROM banking_linked_accounts WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn find_by_provider_account(
        &self,
        provider: &str,
        provider_account_id: &str,
    ) -> Result<Option<BankingLinkedAccountRow>, StorageError> {
        let row = sqlx::query_as::<_, BankingLinkedAccountRow>(
            r#"
            SELECT * FROM banking_linked_accounts
            WHERE provider = ? AND provider_account_id = ? AND is_active = TRUE
            "#,
        )
        .bind(provider)
        .bind(provider_account_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Find active linked account by matching the masked account number suffix.
    /// Used by webhook handlers to resolve which account a transaction belongs to.
    pub async fn find_by_account_number(
        &self,
        provider: &str,
        account_number: &str,
    ) -> Result<Option<BankingLinkedAccountRow>, StorageError> {
        // Match by the last 4 digits (masked portion)
        let suffix = if account_number.len() >= 4 {
            &account_number[account_number.len() - 4..]
        } else {
            account_number
        };
        let mask = format!("****{}", suffix);
        let row = sqlx::query_as::<_, BankingLinkedAccountRow>(
            r#"
            SELECT * FROM banking_linked_accounts
            WHERE provider = ? AND account_number_masked = ? AND is_active = TRUE
            "#,
        )
        .bind(provider)
        .bind(&mask)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn list_active(&self) -> Result<Vec<BankingLinkedAccountRow>, StorageError> {
        let rows = sqlx::query_as::<_, BankingLinkedAccountRow>(
            "SELECT * FROM banking_linked_accounts WHERE is_active = TRUE ORDER BY created_at",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn list_all(&self) -> Result<Vec<BankingLinkedAccountRow>, StorageError> {
        let rows = sqlx::query_as::<_, BankingLinkedAccountRow>(
            "SELECT * FROM banking_linked_accounts ORDER BY created_at",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn set_active(&self, id: &str, active: bool) -> Result<bool, StorageError> {
        let result = sqlx::query(
            "UPDATE banking_linked_accounts SET is_active = ? WHERE id = ?",
        )
        .bind(active)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn update_last_synced(&self, id: &str) -> Result<bool, StorageError> {
        let now = chrono::Utc::now();
        let result = sqlx::query(
            "UPDATE banking_linked_accounts SET last_synced_at = ? WHERE id = ?",
        )
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete(&self, id: &str) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM banking_linked_accounts WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
```

**Step 4: Write failing test for BankingNotificationRuleRepo**

Create `crates/storage/src/repos/tests/banking_notification_rule_repo_tests.rs`:

```rust
use crate::repos::BankingNotificationRuleRepo;
use crate::rows::banking::BankingNotificationRuleRow;
use crate::StoragePool;

#[tokio::test]
async fn test_banking_notification_rule_crud() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repo = BankingNotificationRuleRepo::new(pool.inner().clone());

    // Add threshold rule (global, no linked account)
    let row = BankingNotificationRuleRow {
        id: "rule-1".to_string(),
        linked_account_id: None,
        rule_type: "threshold".to_string(),
        threshold_amount: Some(1_000_000),
        pattern: None,
        channel: "telegram".to_string(),
        is_active: true,
        created_at: chrono::Utc::now(),
    };
    let inserted = repo.add(&row).await.unwrap();
    assert_eq!(inserted.rule_type, "threshold");

    // Add pattern rule
    let row2 = BankingNotificationRuleRow {
        id: "rule-2".to_string(),
        linked_account_id: None,
        rule_type: "pattern".to_string(),
        threshold_amount: None,
        pattern: Some("salary|luong".to_string()),
        channel: "telegram".to_string(),
        is_active: true,
        created_at: chrono::Utc::now(),
    };
    repo.add(&row2).await.unwrap();

    // List active rules
    let rules = repo.list_active(None).await.unwrap();
    assert_eq!(rules.len(), 2);

    // Delete
    repo.delete("rule-2").await.unwrap();
    let rules = repo.list_active(None).await.unwrap();
    assert_eq!(rules.len(), 1);
}
```

Register in tests/mod.rs:
```rust
mod banking_notification_rule_repo_tests;
```

**Step 5: Implement BankingNotificationRuleRepo**

Create `crates/storage/src/repos/banking_notification_rule_repo.rs`:

```rust
//! Repository for the `banking_notification_rules` table.

use sqlx::SqlitePool;

use crate::error::StorageError;
use crate::rows::banking::BankingNotificationRuleRow;

#[derive(Debug, Clone)]
pub struct BankingNotificationRuleRepo {
    pool: SqlitePool,
}

impl BankingNotificationRuleRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn add(
        &self,
        row: &BankingNotificationRuleRow,
    ) -> Result<BankingNotificationRuleRow, StorageError> {
        let inserted = sqlx::query_as::<_, BankingNotificationRuleRow>(
            r#"
            INSERT INTO banking_notification_rules (
                id, linked_account_id, rule_type, threshold_amount,
                pattern, channel, is_active, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            RETURNING *
            "#,
        )
        .bind(&row.id)
        .bind(&row.linked_account_id)
        .bind(&row.rule_type)
        .bind(row.threshold_amount)
        .bind(&row.pattern)
        .bind(&row.channel)
        .bind(row.is_active)
        .bind(row.created_at)
        .fetch_one(&self.pool)
        .await?;
        Ok(inserted)
    }

    /// List active rules. If `linked_account_id` is Some, also includes
    /// global rules (where linked_account_id IS NULL).
    pub async fn list_active(
        &self,
        linked_account_id: Option<&str>,
    ) -> Result<Vec<BankingNotificationRuleRow>, StorageError> {
        let rows = match linked_account_id {
            Some(id) => {
                sqlx::query_as::<_, BankingNotificationRuleRow>(
                    r#"
                    SELECT * FROM banking_notification_rules
                    WHERE is_active = TRUE
                      AND (linked_account_id = ? OR linked_account_id IS NULL)
                    ORDER BY created_at
                    "#,
                )
                .bind(id)
                .fetch_all(&self.pool)
                .await?
            }
            None => {
                sqlx::query_as::<_, BankingNotificationRuleRow>(
                    r#"
                    SELECT * FROM banking_notification_rules
                    WHERE is_active = TRUE
                    ORDER BY created_at
                    "#,
                )
                .fetch_all(&self.pool)
                .await?
            }
        };
        Ok(rows)
    }

    pub async fn delete(&self, id: &str) -> Result<bool, StorageError> {
        let result =
            sqlx::query("DELETE FROM banking_notification_rules WHERE id = ?")
                .bind(id)
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected() > 0)
    }
}
```

**Step 6: Register repos in `crates/storage/src/repos/mod.rs`**

Add module declarations (alphabetically):
```rust
pub mod banking_linked_account_repo;
pub mod banking_notification_rule_repo;
```

Add re-exports:
```rust
pub use banking_linked_account_repo::BankingLinkedAccountRepo;
pub use banking_notification_rule_repo::BankingNotificationRuleRepo;
```

Add fields to `Repos` struct (after finance repos):
```rust
    // Banking repos
    pub banking_linked_accounts: BankingLinkedAccountRepo,
    pub banking_notification_rules: BankingNotificationRuleRepo,
```

Add to `from_pool`:
```rust
            banking_linked_accounts: BankingLinkedAccountRepo::new(db.clone()),
            banking_notification_rules: BankingNotificationRuleRepo::new(db.clone()),
```

**Step 7: Run tests**

Run: `cargo nextest run -p storage -E 'test(banking)'`
Expected: PASS

**Step 8: Commit**

```bash
git add crates/storage/src/repos/banking_linked_account_repo.rs \
        crates/storage/src/repos/banking_notification_rule_repo.rs \
        crates/storage/src/repos/mod.rs \
        crates/storage/src/repos/tests/
git commit -m "feat(storage): add banking linked account and notification rule repos"
```

---

### Task 5: Update FinanceTransactionRepo for source + bank_ref_id

**Files:**
- Modify: `crates/storage/src/repos/finance_transaction_repo.rs` (update INSERT to include source, bank_ref_id)
- Modify: existing transaction repo tests

**Step 1: Write a test for dedup by bank_ref_id**

Add to the existing transaction repo test file:

```rust
#[tokio::test]
async fn test_transaction_bank_ref_id_dedup() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    // ... create account, insert tx with bank_ref_id = "casso-123"
    // ... attempt to insert again with same bank_ref_id
    // ... assert second insert fails with unique constraint
}
```

**Step 2: Update the INSERT query in `finance_transaction_repo.rs`**

Add `source` and `bank_ref_id` to the `add()` method's INSERT statement and bind list. Add a `find_by_bank_ref_id()` method:

```rust
pub async fn find_by_bank_ref_id(
    &self,
    bank_ref_id: &str,
) -> Result<Option<FinanceTransactionRow>, StorageError> {
    let row = sqlx::query_as::<_, FinanceTransactionRow>(
        "SELECT * FROM finance_transactions WHERE bank_ref_id = ?",
    )
    .bind(bank_ref_id)
    .fetch_optional(&self.pool)
    .await?;
    Ok(row)
}
```

**Step 3: Fix any existing tests** that construct `FinanceTransactionRow` — they now need `source: "manual".to_string()` and `bank_ref_id: None`.

**Step 4: Run all storage tests**

Run: `cargo nextest run -p storage`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/storage/src/repos/finance_transaction_repo.rs crates/storage/src/repos/tests/
git commit -m "feat(storage): add source + bank_ref_id to finance transactions for dedup"
```

---

### Task 6: BankingConfig schema

**Files:**
- Create: `crates/config/src/schema/banking.rs`
- Modify: `crates/config/src/schema/mod.rs` (register module)
- Modify: `crates/config/src/schema/core.rs` (add `banking` field to Config)

**Step 1: Create `crates/config/src/schema/banking.rs`**

```rust
//! Banking sync configuration.

use serde::{Deserialize, Serialize};

use super::core::Secret;

/// Banking sync configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BankingConfig {
    /// Enable/disable banking sync (default: false — opt-in).
    #[serde(default)]
    pub enabled: bool,

    #[serde(default)]
    pub providers: BankingProvidersConfig,

    #[serde(default)]
    pub notifications: BankingNotificationConfig,
}

impl Default for BankingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            providers: Default::default(),
            notifications: Default::default(),
        }
    }
}

/// Per-provider configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BankingProvidersConfig {
    #[serde(default)]
    pub casso: BankingProviderEntry,
    #[serde(default)]
    pub sepay: BankingProviderEntry,
}

/// Single provider entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BankingProviderEntry {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub api_key: Secret<String>,
    /// Security token for webhook signature verification.
    #[serde(default)]
    pub security_token: Secret<String>,
}

impl Default for BankingProviderEntry {
    fn default() -> Self {
        Self {
            enabled: false,
            api_key: Secret::default(),
            security_token: Secret::default(),
        }
    }
}

/// Default notification channel configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BankingNotificationConfig {
    /// Channel to send notifications to (default: "telegram").
    #[serde(default = "default_notification_channel")]
    pub default_channel: String,
    /// Chat ID for the default channel.
    #[serde(default)]
    pub default_chat_id: String,
}

impl Default for BankingNotificationConfig {
    fn default() -> Self {
        Self {
            default_channel: default_notification_channel(),
            default_chat_id: String::new(),
        }
    }
}

fn default_notification_channel() -> String {
    "telegram".to_string()
}
```

**Step 2: Register in mod.rs**

In `crates/config/src/schema/mod.rs`, add `mod banking;` (after `agents`) and `pub use self::banking::*;` (after `pub use self::agents::*;`).

**Step 3: Add to Config struct**

In `crates/config/src/schema/core.rs`, add import:
```rust
use super::banking::BankingConfig;
```

Add field to `Config` struct (after `finance`):
```rust
    #[serde(default)]
    pub banking: BankingConfig,
```

**Step 4: Add test**

Add to the test module in `crates/config/src/schema/mod.rs`:
```rust
    #[test]
    fn test_banking_config_default() {
        let config = Config::default();
        assert!(!config.banking.enabled);
        assert!(!config.banking.providers.casso.enabled);
        assert!(!config.banking.providers.sepay.enabled);
    }

    #[test]
    fn test_banking_config_deserializes_from_partial() {
        let json = r#"{
            "banking": {
                "enabled": true,
                "providers": {
                    "casso": { "enabled": true, "apiKey": "test-key" }
                }
            },
            "agents": {
                "defaults": {
                    "workspace": "~/.klyntbot/workspace",
                    "model": "anthropic/claude-opus-4-5",
                    "maxTokens": 8192,
                    "temperature": 0.7,
                    "maxToolIterations": 20
                }
            }
        }"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert!(config.banking.enabled);
        assert!(config.banking.providers.casso.enabled);
        assert_eq!(config.banking.providers.casso.api_key.expose(), "test-key");
        assert!(!config.banking.providers.sepay.enabled);
    }
```

**Step 5: Run tests**

Run: `cargo nextest run -p config`
Expected: PASS

**Step 6: Commit**

```bash
git add crates/config/src/schema/banking.rs crates/config/src/schema/mod.rs crates/config/src/schema/core.rs
git commit -m "feat(config): add BankingConfig schema (providers, notifications)"
```

---

### Task 7: BankProvider trait + normalized types

**Files:**
- Create: `crates/feature-finance/src/banking/mod.rs`
- Modify: `crates/feature-finance/src/lib.rs` (add `pub mod banking;`)

**Step 1: Create `crates/feature-finance/src/banking/mod.rs`**

```rust
//! Banking sync: provider abstraction, webhook handling, transaction pipeline.

pub mod casso;
pub mod pipeline;
pub mod sepay;
pub mod webhook;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use common::Result;
use serde::{Deserialize, Serialize};

/// Normalized bank transaction (provider-agnostic).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BankTransaction {
    /// Provider's unique transaction ID (for dedup).
    pub ref_id: String,
    /// Amount in minor units (positive = credit, negative = debit).
    pub amount: i64,
    /// Balance after this transaction (if provided by the provider).
    pub balance_after: Option<i64>,
    /// Raw transfer content / description.
    pub description: String,
    /// Counterparty name (if available).
    pub counterparty: Option<String>,
    /// Counterparty account number (if available).
    pub counterparty_account: Option<String>,
    /// Counterparty bank name (if available).
    pub counterparty_bank: Option<String>,
    /// Bank name (e.g. "Vietcombank").
    pub bank_name: String,
    /// Account number at the bank.
    pub account_number: String,
    /// When the transaction occurred.
    pub timestamp: DateTime<Utc>,
}

/// Balance snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BankBalance {
    pub amount: i64,
    pub currency: String,
    pub as_of: DateTime<Utc>,
}

/// Abstraction over banking data providers (Casso, SePay, future direct APIs).
#[async_trait]
pub trait BankProvider: Send + Sync {
    /// Provider name (e.g. "casso", "sepay").
    fn name(&self) -> &str;

    /// Validate incoming webhook payload (check signature / security token).
    fn validate_webhook(
        &self,
        headers: &axum::http::HeaderMap,
        body: &[u8],
    ) -> Result<()>;

    /// Parse webhook body into normalized transactions.
    fn parse_webhook(&self, body: &[u8]) -> Result<Vec<BankTransaction>>;

    /// Fetch current balance for a linked account (on-demand REST call).
    async fn fetch_balance(&self, provider_account_id: &str) -> Result<BankBalance>;

    /// Fetch recent transactions (polling fallback).
    async fn fetch_transactions(
        &self,
        provider_account_id: &str,
        since: Option<DateTime<Utc>>,
    ) -> Result<Vec<BankTransaction>>;
}
```

**Step 2: Register in lib.rs**

In `crates/feature-finance/src/lib.rs`, add `pub mod banking;`.

**Step 3: Create stub files so it compiles**

Create empty stubs for the submodules (will be implemented in subsequent tasks):
- `crates/feature-finance/src/banking/casso.rs` — `// Casso provider implementation`
- `crates/feature-finance/src/banking/sepay.rs` — `// SePay provider implementation`
- `crates/feature-finance/src/banking/pipeline.rs` — `// Transaction pipeline`
- `crates/feature-finance/src/banking/webhook.rs` — `// Webhook HTTP handlers`

**Step 4: Verify compilation**

Run: `cargo check -p feature-finance`
Expected: compiles

**Step 5: Commit**

```bash
git add crates/feature-finance/src/banking/ crates/feature-finance/src/lib.rs
git commit -m "feat(finance): add BankProvider trait and normalized types"
```

---

### Task 8: CassoProvider implementation

**Files:**
- Modify: `crates/feature-finance/src/banking/casso.rs`

**Step 1: Write the test**

Add tests at the bottom of `casso.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_casso_webhook_payload() {
        let body = serde_json::json!({
            "error": 0,
            "data": [{
                "id": 6785,
                "tid": "FT23456789",
                "description": "NGUYEN VAN A chuyen tien",
                "amount": 500000,
                "cusum_balance": 10500000,
                "when": "2026-02-24 10:30:00",
                "bank_sub_acc_id": "0123456789",
                "subAccId": "0123456789",
                "bankName": "Vietcombank",
                "bankAbbreviation": "VCB",
                "corresponsiveName": "NGUYEN VAN A",
                "corresponsiveAccount": "9876543210",
                "corresponsiveBankId": "",
                "corresponsiveBankName": "MB Bank"
            }]
        });

        let provider = CassoProvider::new("test-token".to_string(), "test-key".to_string());
        let txs = provider.parse_webhook(body.to_string().as_bytes()).unwrap();

        assert_eq!(txs.len(), 1);
        assert_eq!(txs[0].ref_id, "6785");
        assert_eq!(txs[0].amount, 500000);
        assert_eq!(txs[0].balance_after, Some(10500000));
        assert_eq!(txs[0].counterparty, Some("NGUYEN VAN A".to_string()));
        assert_eq!(txs[0].counterparty_bank, Some("MB Bank".to_string()));
        assert_eq!(txs[0].bank_name, "Vietcombank");
        assert_eq!(txs[0].account_number, "0123456789");
    }

    #[test]
    fn test_validate_casso_webhook_valid_token() {
        let provider = CassoProvider::new("my-secret-token".to_string(), "key".to_string());
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("secure-token", "my-secret-token".parse().unwrap());

        assert!(provider.validate_webhook(&headers, b"{}").is_ok());
    }

    #[test]
    fn test_validate_casso_webhook_invalid_token() {
        let provider = CassoProvider::new("my-secret-token".to_string(), "key".to_string());
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("secure-token", "wrong-token".parse().unwrap());

        assert!(provider.validate_webhook(&headers, b"{}").is_err());
    }
}
```

**Step 2: Run to verify failure**

Run: `cargo nextest run -p feature-finance -E 'test(casso)'`
Expected: FAIL

**Step 3: Implement CassoProvider**

Implement the struct with `validate_webhook` (check `secure-token` header), `parse_webhook` (deserialize Casso JSON format), `fetch_balance` (GET /v2/accounts/:id), `fetch_transactions` (GET /v2/accounts/:id/transactions). The Casso webhook JSON structure:

```json
{"error": 0, "data": [{"id": int, "tid": str, "description": str, "amount": int, "cusum_balance": int, "when": "YYYY-MM-DD HH:MM:SS", "bank_sub_acc_id": str, "bankName": str, "corresponsiveName": str, "corresponsiveAccount": str, "corresponsiveBankName": str}]}
```

**Step 4: Run tests**

Run: `cargo nextest run -p feature-finance -E 'test(casso)'`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/feature-finance/src/banking/casso.rs
git commit -m "feat(finance): implement CassoProvider (webhook parse + validate)"
```

---

### Task 9: SePayProvider implementation

**Files:**
- Modify: `crates/feature-finance/src/banking/sepay.rs`

**Step 1: Write the test**

Same pattern as Task 8 but for SePay's JSON format:

```json
{"id": int, "gateway": str, "transactionDate": "YYYY-MM-DD HH:MM:SS", "accountNumber": str, "transferType": "in"|"out", "transferAmount": int, "accumulated": int, "code": str, "content": str, "referenceCode": str, "description": str}
```

Tests:
- `test_parse_sepay_webhook_payload`
- `test_validate_sepay_webhook_valid_token`
- `test_validate_sepay_webhook_invalid_token`
- SePay uses `Authorization` header with API key for validation.

**Step 2-4: Implement + test** (same flow as Task 8)

**Step 5: Commit**

```bash
git add crates/feature-finance/src/banking/sepay.rs
git commit -m "feat(finance): implement SePayProvider (webhook parse + validate)"
```

---

### Task 10: Transaction pipeline (dedup -> store -> notify)

**Files:**
- Modify: `crates/feature-finance/src/banking/pipeline.rs`

**Step 1: Write the test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pipeline_stores_new_transaction() {
        // Setup: in-memory pool, create a finance account + linked account
        // Run pipeline with one BankTransaction
        // Assert: finance_transactions has 1 row with source="casso"
        // Assert: finance account balance updated
    }

    #[tokio::test]
    async fn test_pipeline_deduplicates_by_bank_ref_id() {
        // Setup: insert a transaction with bank_ref_id="ref-1"
        // Run pipeline with BankTransaction having ref_id="ref-1"
        // Assert: still only 1 row (no duplicate)
        // Assert: pipeline returns skipped count
    }

    #[tokio::test]
    async fn test_pipeline_evaluates_threshold_rule() {
        // Setup: create notification rule with threshold=1_000_000
        // Run pipeline with amount=2_000_000
        // Assert: notification was triggered (use mock dispatcher)
    }

    #[tokio::test]
    async fn test_pipeline_skips_below_threshold() {
        // Setup: threshold=1_000_000
        // Run pipeline with amount=500_000
        // Assert: no notification
    }
}
```

**Step 2: Implement the pipeline**

```rust
pub struct TransactionPipeline {
    finance_transactions: FinanceTransactionRepo,
    finance_accounts: FinanceAccountRepo,
    linked_accounts: BankingLinkedAccountRepo,
    notification_rules: BankingNotificationRuleRepo,
    notification_tx: Option<tokio::sync::mpsc::Sender<NotificationEvent>>,
}

pub struct PipelineResult {
    pub stored: usize,
    pub skipped: usize,  // duplicates
    pub failed: usize,
    pub notified: usize,
}

pub struct NotificationEvent {
    pub title: String,
    pub body: String,
    pub channel: String,
}

impl TransactionPipeline {
    pub async fn process(
        &self,
        provider_name: &str,
        transactions: Vec<BankTransaction>,
    ) -> Result<PipelineResult> { ... }
}
```

The pipeline:
1. For each `BankTransaction`, check `find_by_bank_ref_id` — skip if exists
2. Resolve linked account via `find_by_account_number`
3. Insert into `finance_transactions` with `source=provider_name`, `bank_ref_id=ref_id`
4. `adjust_balance` on the finance account
5. Evaluate notification rules: check threshold and pattern rules
6. Send `NotificationEvent` if any rule matches

**Step 3: Run tests**

Run: `cargo nextest run -p feature-finance -E 'test(pipeline)'`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/feature-finance/src/banking/pipeline.rs
git commit -m "feat(finance): implement transaction pipeline (dedup, store, notify)"
```

---

### Task 11: Webhook HTTP handlers (axum)

**Files:**
- Modify: `crates/feature-finance/src/banking/webhook.rs`

**Step 1: Write the test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_webhook_health_endpoint() {
        let state = test_webhook_state().await;
        let app = webhook_router(state);
        let req = Request::get("/webhooks/health").body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_webhook_rejects_invalid_signature() {
        let state = test_webhook_state().await;
        let app = webhook_router(state);
        let req = Request::post("/webhooks/casso")
            .header("secure-token", "wrong")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"error":0,"data":[]}"#))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }
}
```

**Step 2: Implement webhook router**

```rust
use axum::{Router, routing::{get, post}, extract::State, http::StatusCode, body::Bytes};
use std::sync::Arc;
use std::collections::HashMap;

use super::{BankProvider, pipeline::TransactionPipeline};

pub struct WebhookState {
    pub providers: HashMap<String, Arc<dyn BankProvider>>,
    pub pipeline: Arc<TransactionPipeline>,
}

pub fn webhook_router(state: Arc<WebhookState>) -> Router {
    Router::new()
        .route("/webhooks/health", get(health))
        .route("/webhooks/casso", post(handle_casso))
        .route("/webhooks/sepay", post(handle_sepay))
        .with_state(state)
}

async fn health() -> StatusCode {
    StatusCode::OK
}

async fn handle_casso(
    State(state): State<Arc<WebhookState>>,
    headers: axum::http::HeaderMap,
    body: Bytes,
) -> StatusCode {
    handle_webhook(state, "casso", headers, &body).await
}

async fn handle_sepay(
    State(state): State<Arc<WebhookState>>,
    headers: axum::http::HeaderMap,
    body: Bytes,
) -> StatusCode {
    handle_webhook(state, "sepay", headers, &body).await
}

async fn handle_webhook(
    state: Arc<WebhookState>,
    provider_name: &str,
    headers: axum::http::HeaderMap,
    body: &[u8],
) -> StatusCode {
    let provider = match state.providers.get(provider_name) {
        Some(p) => p,
        None => return StatusCode::NOT_FOUND,
    };

    if let Err(_) = provider.validate_webhook(&headers, body) {
        return StatusCode::UNAUTHORIZED;
    }

    let transactions = match provider.parse_webhook(body) {
        Ok(txs) => txs,
        Err(_) => return StatusCode::BAD_REQUEST,
    };

    match state.pipeline.process(provider_name, transactions).await {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
```

**Step 3: Run tests**

Run: `cargo nextest run -p feature-finance -E 'test(webhook)'`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/feature-finance/src/banking/webhook.rs
git commit -m "feat(finance): add axum webhook handlers for banking providers"
```

---

### Task 12: Integrate webhook server into `klyntbot serve`

**Files:**
- Modify: `crates/cli/src/serve.rs`
- Modify: `crates/cli/Cargo.toml` (add axum dep if not already inherited)

**Step 1: Add webhook server startup to `handle_serve()`**

After the channel manager setup but before the shutdown signal, add:

```rust
// Start banking webhook server if enabled
let webhook_handle = if config.banking.enabled {
    let webhook_state = feature_finance::banking::webhook::build_webhook_state(
        &config.banking,
        repos.clone(),
    );
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    info!("Banking webhook server starting on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    Some(tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, webhook_state.into_make_service()).await {
            error!("Webhook server error: {}", e);
        }
    }))
} else {
    None
};
```

Add to the shutdown section:
```rust
if let Some(handle) = webhook_handle {
    handle.abort();
}
```

Update the services printout:
```rust
if config.banking.enabled {
    println!("  Banking webhooks (port {})", port);
}
```

**Step 2: Verify compilation**

Run: `cargo check -p cli`
Expected: compiles

**Step 3: Commit**

```bash
git add crates/cli/src/serve.rs crates/cli/Cargo.toml
git commit -m "feat(cli): integrate banking webhook server into klyntbot serve"
```

---

### Task 13: Banking tool actions in FinanceTool

**Files:**
- Modify: `crates/feature-finance/src/tool/mod.rs` (add banking-* actions to dispatch)

**Step 1: Write tests for the tool actions**

Test `banking-link`, `banking-list`, `banking-alert-add`, `banking-alert-list` via the tool's `execute()` method with JSON args.

**Step 2: Add actions to the `execute()` dispatch**

Add matches for:
- `"banking-link"` — validate provider name, create finance account + linked account
- `"banking-unlink"` — set `is_active = false` on linked account
- `"banking-list"` — list all linked accounts with sync status
- `"banking-sync"` — call `provider.fetch_balance()` and update
- `"banking-alert-add"` — create notification rule
- `"banking-alert-list"` — list active rules
- `"banking-alert-remove"` — delete rule

Add the actions to `fn parameters()` JSON schema and `fn description()`.

**Step 3: Run tests**

Run: `cargo nextest run -p feature-finance -E 'test(banking)'`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/feature-finance/src/tool/
git commit -m "feat(finance): add banking-* tool actions (link, unlink, list, sync, alerts)"
```

---

### Task 14: Integration test — full webhook-to-notification flow

**Files:**
- Create: `tests/banking_integration.rs` (workspace-level integration test)

**Step 1: Write end-to-end test**

```rust
//! Integration test: webhook POST → pipeline → transaction stored → notification triggered

#[tokio::test]
async fn test_full_banking_webhook_flow() {
    // 1. Create in-memory storage pool
    // 2. Create a finance account + linked banking account
    // 3. Create a notification rule (threshold 500_000)
    // 4. Build webhook router with CassoProvider
    // 5. Send a POST /webhooks/casso with valid Casso payload (amount=1_000_000)
    // 6. Assert: response is 200
    // 7. Assert: finance_transactions has 1 row with source="casso"
    // 8. Assert: finance account balance was adjusted
    // 9. Assert: notification was dispatched (mock channel)
}

#[tokio::test]
async fn test_webhook_dedup_on_replay() {
    // Same setup, send same webhook twice
    // Assert: still only 1 transaction row
    // Assert: second response is still 200 (idempotent)
}
```

**Step 2: Run**

Run: `cargo nextest run --test banking_integration`
Expected: PASS

**Step 3: Run full test suite**

Run: `cargo nextest run --workspace`
Expected: PASS, 0 failures

**Step 4: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

**Step 5: Commit**

```bash
git add tests/banking_integration.rs
git commit -m "test: add banking webhook integration tests"
```

---

### Task 15: Final verification + cleanup

**Step 1: Run full build**

Run: `cargo build --workspace`
Expected: compiles

**Step 2: Run full test suite + doctests**

Run: `cargo nextest run --workspace && cargo test --workspace --doc`
Expected: all PASS

**Step 3: Run clippy + fmt**

Run: `cargo clippy --workspace --all-targets --all-features && cargo fmt --all --check`
Expected: 0 warnings, formatting OK

**Step 4: Final commit if any cleanup needed**

```bash
git commit -m "chore: banking sync cleanup and final verification"
```

---

## Task Dependency Graph

```
Task 1 (deps)
  └→ Task 2 (migration)
       └→ Task 3 (row structs)
            ├→ Task 4 (banking repos)
            └→ Task 5 (update finance tx repo)
                 └→ Task 6 (config)
                      └→ Task 7 (trait + types)
                           ├→ Task 8 (Casso provider)
                           └→ Task 9 (SePay provider)
                                └→ Task 10 (pipeline)
                                     └→ Task 11 (webhook handlers)
                                          └→ Task 12 (serve.rs integration)
                                               └→ Task 13 (tool actions)
                                                    └→ Task 14 (integration test)
                                                         └→ Task 15 (verification)
```
