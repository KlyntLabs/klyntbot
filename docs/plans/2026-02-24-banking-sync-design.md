# Banking Sync — Auto-Import Bank Transactions

**Date**: 2026-02-24
**Status**: Approved
**Approach**: Extend `feature-finance` with `banking/` submodule

## Summary

Add real-time bank transaction import to klyntbot's finance feature via third-party aggregator webhooks (Casso, SePay). Transactions from Vietnamese banks (VCB, MB, ACB, etc.) are automatically recorded into the existing finance transaction system, with smart notifications for high-value or pattern-matched transactions.

## Decisions

| Decision | Choice |
|----------|--------|
| Use case | Personal finance tracking (auto-import) |
| Providers | Both Casso + SePay, abstracted behind `BankProvider` trait |
| Data flow | Webhook-first (axum HTTP listener in `klyntbot serve`) |
| On transaction | Smart notify (threshold + pattern rules) |
| Account management | Config for initial setup + chat for runtime |
| Architecture | Extend `feature-finance` with `banking/` submodule |

## Context

### Vietnamese Banking API Landscape

VCB (Vietcombank) has no public API for personal accounts. The practical options are third-party aggregator services:

| Service | VCB Support | Multi-Bank | Balance Check | Webhooks | Free Tier |
|---------|------------|-----------|--------------|----------|-----------|
| Casso | Yes (direct) | 10+ banks | REST API | Real-time POST | 30 tx/month |
| SePay | Via Bank Hub | All VN banks | REST API | Real-time POST | 500 tx/month (1 yr) |

Vietnam's Circular 64 mandates all banks expose Open APIs by March 2027 (Phase 1 info queries by March 2026). The `BankProvider` trait is designed to accommodate direct bank APIs when they become available.

### Existing Infrastructure

- `feature-finance` crate with 40+ tool actions, 6 repos (accounts, transactions, budgets)
- `FinanceTransactionRow` already has `amount`, `counterparty`, `notes`, `tx_date`, `category`
- `FinanceAccountRow` has `institution` field for bank name
- `NotificationDispatcher` for pushing alerts to channels
- Cron/scheduling system available as polling fallback
- No HTTP server currently in `klyntbot serve` — axum will be added

## Data Model

### New Tables

#### `banking_linked_accounts`

Tracks which bank accounts are connected via which provider.

| Column | Type | Purpose |
|--------|------|---------|
| `id` | TEXT PK | UUID |
| `finance_account_id` | TEXT FK | Links to `finance_accounts.id` |
| `provider` | TEXT | `"casso"` or `"sepay"` |
| `provider_account_id` | TEXT | Account ID from the provider |
| `bank_name` | TEXT | e.g. `"Vietcombank"` |
| `account_number_masked` | TEXT | e.g. `"****6789"` |
| `last_synced_at` | DATETIME | Last successful webhook/sync |
| `is_active` | BOOL | Enable/disable sync |
| `created_at` | DATETIME | |

#### `banking_notification_rules`

Smart notification rules.

| Column | Type | Purpose |
|--------|------|---------|
| `id` | TEXT PK | UUID |
| `linked_account_id` | TEXT FK | Which bank account (or NULL for all) |
| `rule_type` | TEXT | `"threshold"`, `"pattern"`, `"all"` |
| `threshold_amount` | INTEGER | Notify if `abs(amount) >= threshold` |
| `pattern` | TEXT | Regex match on description/counterparty |
| `channel` | TEXT | Where to notify (`"telegram"`, `"discord"`) |
| `is_active` | BOOL | |

### Changes to Existing Tables

**`finance_transactions`** — add 2 columns:
- `source` TEXT DEFAULT `'manual'` — values: `"manual"`, `"casso"`, `"sepay"`
- `bank_ref_id` TEXT UNIQUE — provider's transaction ID for deduplication

## Provider Abstraction

### `BankProvider` trait

```rust
#[async_trait]
pub trait BankProvider: Send + Sync {
    fn name(&self) -> &str;
    fn validate_webhook(&self, headers: &HeaderMap, body: &[u8]) -> Result<()>;
    fn parse_webhook(&self, body: &[u8]) -> Result<Vec<BankTransaction>>;
    async fn fetch_balance(&self, account_id: &str) -> Result<BankBalance>;
    async fn fetch_transactions(
        &self,
        account_id: &str,
        since: Option<DateTime<Utc>>,
    ) -> Result<Vec<BankTransaction>>;
}
```

### Normalized Types

```rust
pub struct BankTransaction {
    pub ref_id: String,
    pub amount: i64,
    pub balance_after: Option<i64>,
    pub description: String,
    pub counterparty: Option<String>,
    pub counterparty_account: Option<String>,
    pub counterparty_bank: Option<String>,
    pub bank_name: String,
    pub account_number: String,
    pub timestamp: DateTime<Utc>,
}

pub struct BankBalance {
    pub amount: i64,
    pub currency: String,
    pub as_of: DateTime<Utc>,
}
```

### Implementations

- `CassoProvider` — validates via Casso security header, parses Casso JSON
- `SePayProvider` — validates via SePay header, parses SePay JSON

## Webhook Server & Transaction Pipeline

### HTTP Endpoints

Added to `handle_serve()` as a new spawned axum task:

```
POST /webhooks/casso    → CassoProvider::validate_webhook + parse_webhook
POST /webhooks/sepay    → SePayProvider::validate_webhook + parse_webhook
GET  /webhooks/health   → 200 OK
```

Binds to the `--port` flag from `klyntbot serve`. Only started when `config.banking.enabled == true`.

### Transaction Pipeline

```
Webhook POST arrives
  │
  ├─ 1. Validate signature (provider.validate_webhook)
  ├─ 2. Parse to Vec<BankTransaction> (provider.parse_webhook)
  │
  └─ For each BankTransaction:
       ├─ 3. Dedup: check bank_ref_id in finance_transactions → skip if exists
       ├─ 4. Resolve account: match account_number → banking_linked_accounts
       │      → reject if no linked account found
       ├─ 5. Write: insert into finance_transactions with source="casso"|"sepay"
       ├─ 6. Update balance: adjust_balance on the linked finance_account
       └─ 7. Evaluate notification rules:
              ├─ Check threshold rules (amount >= threshold?)
              ├─ Check pattern rules (description matches regex?)
              └─ If matched → NotificationDispatcher.notify() to configured channel
```

### Shared State (Axum Extractors)

```rust
struct WebhookState {
    providers: HashMap<String, Arc<dyn BankProvider>>,
    repos: Repos,
    linked_account_repo: BankingLinkedAccountRepo,
    notification_rule_repo: BankingNotificationRuleRepo,
    notification_dispatcher: Arc<NotificationDispatcher>,
}
```

### Error Handling

- Invalid signature → 401 Unauthorized (no retry)
- Unknown account → log warning, return 200 (providers retry on non-200)
- Duplicate `bank_ref_id` → skip silently, return 200
- DB write failure → 500 Internal Server Error (provider will retry)

## Chat Management (Tool Actions)

### Banking Account Management

| Action | Description |
|--------|-------------|
| `banking-link` | Link a bank account: specify provider, API key, account number |
| `banking-unlink` | Deactivate a linked account (soft disable, keeps history) |
| `banking-list` | List all linked bank accounts with sync status |
| `banking-sync` | Manual balance check via provider REST API |

### Notification Rules

| Action | Description |
|--------|-------------|
| `banking-alert-add` | Create a notification rule |
| `banking-alert-list` | List active notification rules |
| `banking-alert-remove` | Delete a notification rule |

## Configuration

```json
{
  "banking": {
    "enabled": true,
    "webhookPort": 8080,
    "providers": {
      "casso": {
        "enabled": true,
        "apiKey": "secret:casso-api-key",
        "securityToken": "secret:casso-webhook-token"
      },
      "sepay": {
        "enabled": false,
        "apiKey": "",
        "securityToken": ""
      }
    },
    "notifications": {
      "defaultChannel": "telegram",
      "defaultChatId": "your-chat-id"
    }
  }
}
```

API keys use `Secret<String>` (redacted in Debug/Display).

## File Layout

### New Files

```
crates/feature-finance/src/
  banking/
    mod.rs              — BankProvider trait, BankTransaction, BankBalance types
    casso.rs            — CassoProvider implementation
    sepay.rs            — SePayProvider implementation
    webhook.rs          — Axum router, handlers, WebhookState
    pipeline.rs         — Transaction pipeline (dedup → store → notify)
    repo.rs             — BankingLinkedAccountRepo, BankingNotificationRuleRepo

crates/config/src/schema/
  banking.rs            — BankingConfig, BankingProvidersConfig

crates/storage/src/
  rows/banking.rs       — BankingLinkedAccountRow, BankingNotificationRuleRow
  repos/banking_linked_account_repo.rs
  repos/banking_notification_rule_repo.rs
  migrations/XXX_banking_sync.sql
```

### Modified Files

| File | Change |
|------|--------|
| `crates/cli/src/serve.rs` | Spawn axum webhook server |
| `crates/feature-finance/src/lib.rs` | Export `banking` module |
| `crates/feature-finance/src/tool/mod.rs` | Add `banking-*` actions |
| `crates/config/src/schema/mod.rs` | Add `BankingConfig` to root config |
| `crates/storage/src/rows/mod.rs` | Export banking row types |
| `crates/storage/src/repos/mod.rs` | Add banking repos to `Repos` |
| `Cargo.toml` (feature-finance) | Add `axum`, `tower-http` deps |

## Security

- **API keys**: `Secret<String>`, never logged or displayed
- **Webhook validation**: Every inbound webhook verified against provider security tokens
- **Account numbers**: Stored masked (`****6789`); full number only at the provider
- **No bank credentials**: Casso/SePay handle bank connections; we only store their API keys
- **Rate limiting**: Axum middleware, 60 req/min per IP via `tower::limit`

## Future Extensions

- Direct bank Open APIs when Circular 64 is implemented (2026-2027)
- PayOS/VietQR integration for payment confirmation
- Polling fallback via cron for providers without webhook support
- Auto-categorization of transactions using LLM
