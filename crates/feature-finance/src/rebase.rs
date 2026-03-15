//! Home currency change — re-computes all base_* fields across every finance table.
//!
//! When the user switches their default currency (e.g. USD → EUR), every row's
//! `base_*` columns must be recalculated using fresh exchange rates. This module
//! orchestrates that process.

use sqlx::SqlitePool;
use storage::StorageError;

use crate::price_service::PriceService;
use crate::rate_cache::RateCache;

/// Summary of a rebase operation.
pub struct RebaseResult {
    /// Number of tables updated.
    pub tables_updated: usize,
    /// Total rows updated across all tables.
    pub rows_updated: usize,
    /// Per-table failure messages (empty on full success).
    pub failures: Vec<String>,
}

/// Sentinel row key written to exchange_rates table during rebase.
/// If present on startup, the previous rebase was interrupted.
const REBASE_SENTINEL_FROM: &str = "__REBASE__";
const REBASE_SENTINEL_TO: &str = "__REBASE__";

/// Convenience wrapper: sqlx::Error → StorageError → KlyntbotError.
fn se(e: sqlx::Error) -> common::KlyntbotError {
    StorageError::from(e).into()
}

/// Re-compute all `base_*` columns in every finance table for a new base currency.
///
/// Steps:
/// 1. Write sentinel row to detect interrupted rebases
/// 2. Collect all distinct source currencies from every table
/// 3. Prefetch rates for all source currencies → new_base
/// 4. Update each table in a single transaction
/// 5. Remove sentinel
pub async fn rebase_all_tables(
    pool: &SqlitePool,
    rate_cache: &RateCache,
    price_service: &PriceService,
    new_base: &str,
) -> common::Result<RebaseResult> {
    let new_base_upper = new_base.to_uppercase();

    // Step 1: Write sentinel
    rate_cache
        .repo()
        .upsert(REBASE_SENTINEL_FROM, REBASE_SENTINEL_TO, 0.0)
        .await?;

    // Step 2: Collect distinct source currencies from all tables
    let mut all_currencies: Vec<String> = Vec::new();
    collect_distinct_currencies(pool, &mut all_currencies).await?;

    // Also collect market_currency from investments
    let market_currencies: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT market_currency FROM finance_investments WHERE market_currency IS NOT NULL",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    for (cur,) in market_currencies {
        if !all_currencies.iter().any(|c| c.eq_ignore_ascii_case(&cur)) {
            all_currencies.push(cur);
        }
    }

    // Deduplicate and remove the new base itself
    all_currencies.sort();
    all_currencies.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    all_currencies.retain(|c| !c.eq_ignore_ascii_case(&new_base_upper));

    // Step 3: Prefetch rates (foreign → new_base)
    let _prefetched = price_service
        .prefetch_rates(&new_base_upper, &all_currencies)
        .await?;

    // Build a rate lookup: source_currency → rate_to_new_base
    let mut rate_map = std::collections::HashMap::<String, f64>::new();
    for currency in &all_currencies {
        let rate = price_service.get_rate(currency, &new_base_upper).await?;
        rate_map.insert(currency.to_uppercase(), rate);
    }

    let mut result = RebaseResult {
        tables_updated: 0,
        rows_updated: 0,
        failures: Vec::new(),
    };

    // Step 4: Rebase each table

    // 4a: accounts — (balance → base_balance)
    match rebase_table(
        pool,
        "finance_accounts",
        "currency",
        &[("balance", "base_balance")],
        &new_base_upper,
        &rate_map,
    )
    .await
    {
        Ok(n) => {
            result.tables_updated += 1;
            result.rows_updated += n;
        }
        Err(e) => result.failures.push(format!("finance_accounts: {e}")),
    }

    // 4b: transactions — (amount → base_amount)
    match rebase_table(
        pool,
        "finance_transactions",
        "currency",
        &[("amount", "base_amount")],
        &new_base_upper,
        &rate_map,
    )
    .await
    {
        Ok(n) => {
            result.tables_updated += 1;
            result.rows_updated += n;
        }
        Err(e) => result.failures.push(format!("finance_transactions: {e}")),
    }

    // 4c: budgets — (amount → base_amount)
    match rebase_table(
        pool,
        "finance_budgets",
        "currency",
        &[("amount", "base_amount")],
        &new_base_upper,
        &rate_map,
    )
    .await
    {
        Ok(n) => {
            result.tables_updated += 1;
            result.rows_updated += n;
        }
        Err(e) => result.failures.push(format!("finance_budgets: {e}")),
    }

    // 4d: goals — (target_amount → base_target_amount, current_amount → base_current_amount)
    match rebase_table(
        pool,
        "finance_goals",
        "currency",
        &[
            ("target_amount", "base_target_amount"),
            ("current_amount", "base_current_amount"),
        ],
        &new_base_upper,
        &rate_map,
    )
    .await
    {
        Ok(n) => {
            result.tables_updated += 1;
            result.rows_updated += n;
        }
        Err(e) => result.failures.push(format!("finance_goals: {e}")),
    }

    // 4e: liabilities — (principal → base_principal, remaining → base_remaining)
    match rebase_table(
        pool,
        "finance_liabilities",
        "currency",
        &[
            ("principal", "base_principal"),
            ("remaining", "base_remaining"),
        ],
        &new_base_upper,
        &rate_map,
    )
    .await
    {
        Ok(n) => {
            result.tables_updated += 1;
            result.rows_updated += n;
        }
        Err(e) => result.failures.push(format!("finance_liabilities: {e}")),
    }

    // 4f: investment_transactions — (total_amount → base_total_amount)
    match rebase_table(
        pool,
        "finance_investment_transactions",
        "currency",
        &[("total_amount", "base_total_amount")],
        &new_base_upper,
        &rate_map,
    )
    .await
    {
        Ok(n) => {
            result.tables_updated += 1;
            result.rows_updated += n;
        }
        Err(e) => result
            .failures
            .push(format!("finance_investment_transactions: {e}")),
    }

    // 4g: investments — special dual-rate handling
    match rebase_investment_table(pool, &new_base_upper, &rate_map).await {
        Ok(n) => {
            result.tables_updated += 1;
            result.rows_updated += n;
        }
        Err(e) => result.failures.push(format!("finance_investments: {e}")),
    }

    // Step 5: Remove sentinel
    let _ = rate_cache
        .repo()
        .delete_sentinel(REBASE_SENTINEL_FROM)
        .await;

    Ok(result)
}

/// Collect distinct `currency` values from all standard finance tables.
async fn collect_distinct_currencies(
    pool: &SqlitePool,
    out: &mut Vec<String>,
) -> common::Result<()> {
    let tables = [
        "finance_accounts",
        "finance_transactions",
        "finance_budgets",
        "finance_goals",
        "finance_liabilities",
        "finance_investment_transactions",
        "finance_investments",
    ];

    for table in tables {
        let query = format!("SELECT DISTINCT currency FROM {table} WHERE currency IS NOT NULL");
        let rows: Vec<(String,)> = sqlx::query_as(&query).fetch_all(pool).await.map_err(se)?;
        for (cur,) in rows {
            if !out.iter().any(|c| c.eq_ignore_ascii_case(&cur)) {
                out.push(cur);
            }
        }
    }

    Ok(())
}

/// Generic rebase for tables with a single currency column and one or more amount pairs.
///
/// For each row, computes `rate = rate_map[currency]` (or 1.0 for same-currency),
/// then sets `base_col = ROUND(source_col * rate)`, `base_currency = new_base`,
/// `exchange_rate = rate`.
async fn rebase_table(
    pool: &SqlitePool,
    table: &str,
    currency_col: &str,
    amount_pairs: &[(&str, &str)],
    new_base: &str,
    rate_map: &std::collections::HashMap<String, f64>,
) -> common::Result<usize> {
    let mut tx = pool.begin().await.map_err(se)?;

    // For each distinct currency in this table, run a single UPDATE
    let query =
        format!("SELECT DISTINCT {currency_col} FROM {table} WHERE {currency_col} IS NOT NULL");
    let currencies: Vec<(String,)> = sqlx::query_as(&query)
        .fetch_all(&mut *tx)
        .await
        .map_err(se)?;

    let mut total_rows = 0usize;

    for (currency,) in &currencies {
        let rate = if currency.eq_ignore_ascii_case(new_base) {
            1.0
        } else {
            *rate_map.get(&currency.to_uppercase()).unwrap_or(&1.0)
        };

        // Build SET clause
        let set_parts: Vec<String> = amount_pairs
            .iter()
            .map(|(src, dst)| format!("{dst} = CAST(ROUND({src} * {rate}) AS INTEGER)"))
            .collect();
        let set_clause = set_parts.join(", ");

        let update_sql = format!(
            "UPDATE {table} SET {set_clause}, base_currency = ?, exchange_rate = ? WHERE {currency_col} = ?"
        );

        let result = sqlx::query(&update_sql)
            .bind(new_base)
            .bind(rate)
            .bind(currency)
            .execute(&mut *tx)
            .await
            .map_err(se)?;

        total_rows += result.rows_affected() as usize;
    }

    tx.commit().await.map_err(se)?;
    Ok(total_rows)
}

/// Special rebase for `finance_investments` which has dual rates:
/// - `purchase_rate` = currency → new_base (for cost_basis)
/// - `market_rate` = market_currency → new_base (for current_value)
async fn rebase_investment_table(
    pool: &SqlitePool,
    new_base: &str,
    rate_map: &std::collections::HashMap<String, f64>,
) -> common::Result<usize> {
    let mut tx = pool.begin().await.map_err(se)?;

    // Get all distinct (currency, market_currency) pairs
    let pairs: Vec<(String, Option<String>)> =
        sqlx::query_as("SELECT DISTINCT currency, market_currency FROM finance_investments")
            .fetch_all(&mut *tx)
            .await
            .map_err(se)?;

    let mut total_rows = 0usize;

    for (purchase_currency, market_currency) in &pairs {
        let purchase_rate = if purchase_currency.eq_ignore_ascii_case(new_base) {
            1.0
        } else {
            *rate_map
                .get(&purchase_currency.to_uppercase())
                .unwrap_or(&1.0)
        };

        let mkt_cur = market_currency
            .as_deref()
            .unwrap_or(purchase_currency.as_str());
        let market_rate = if mkt_cur.eq_ignore_ascii_case(new_base) {
            1.0
        } else {
            *rate_map.get(&mkt_cur.to_uppercase()).unwrap_or(&1.0)
        };

        let sql = if market_currency.is_some() {
            format!(
                "UPDATE finance_investments SET \
                 base_cost_basis = CAST(ROUND(cost_basis * {purchase_rate}) AS INTEGER), \
                 base_current_value = CAST(ROUND(COALESCE(current_value, 0) * {market_rate}) AS INTEGER), \
                 base_currency = ?, \
                 purchase_rate = ?, \
                 market_rate = ? \
                 WHERE currency = ? AND market_currency = ?"
            )
        } else {
            format!(
                "UPDATE finance_investments SET \
                 base_cost_basis = CAST(ROUND(cost_basis * {purchase_rate}) AS INTEGER), \
                 base_current_value = CAST(ROUND(COALESCE(current_value, 0) * {market_rate}) AS INTEGER), \
                 base_currency = ?, \
                 purchase_rate = ?, \
                 market_rate = ? \
                 WHERE currency = ? AND market_currency IS NULL"
            )
        };

        let mut query = sqlx::query(&sql)
            .bind(new_base)
            .bind(purchase_rate)
            .bind(market_rate)
            .bind(purchase_currency);

        if market_currency.is_some() {
            query = query.bind(market_currency.as_deref().unwrap());
        }

        let result = query.execute(&mut *tx).await.map_err(se)?;
        total_rows += result.rows_affected() as usize;
    }

    tx.commit().await.map_err(se)?;
    Ok(total_rows)
}
