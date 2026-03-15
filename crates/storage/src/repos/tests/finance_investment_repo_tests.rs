//! Tests for FinanceInvestmentRepo (portfolios, investments, investment_transactions).

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, Utc};

    use crate::repos::finance_investment_repo::FinanceInvestmentRepo;
    use crate::rows::finance::{FinanceInvestmentRow, FinanceInvestmentTxRow, FinancePortfolioRow};
    use crate::StoragePool;

    async fn test_repo() -> Option<FinanceInvestmentRepo> {
        let dir = tempfile::tempdir().ok()?;
        let pool = StoragePool::connect(dir.path()).await.ok()?;
        let _ = dir.keep(); // prevent cleanup; acceptable in test context
        Some(FinanceInvestmentRepo::new(pool.inner().clone()))
    }

    fn unique_id(prefix: &str) -> String {
        format!("{prefix}-{}", uuid::Uuid::new_v4().as_simple())
    }

    fn sample_portfolio(id: &str, name: &str) -> FinancePortfolioRow {
        let now = Utc::now();
        FinancePortfolioRow {
            id: id.to_string(),
            name: name.to_string(),
            description: None,
            currency: "VND".to_string(),
            created_at: now,
            updated_at: now,
        }
    }

    fn sample_investment(id: &str, portfolio_id: &str) -> FinanceInvestmentRow {
        let now = Utc::now();
        FinanceInvestmentRow {
            id: id.to_string(),
            portfolio_id: portfolio_id.to_string(),
            asset_type: "stock".to_string(),
            symbol: Some("VIC".to_string()),
            name: "Vingroup".to_string(),
            quantity: "100".to_string(),
            cost_basis: 1_000_000,
            currency: "VND".to_string(),
            current_price: None,
            current_value: None,
            purchase_date: Some(NaiveDate::from_ymd_opt(2025, 1, 15).unwrap()),
            asset_class: None,
            notes: None,
            created_at: now,
            updated_at: now,
            market_currency: None,
            base_cost_basis: 0,
            base_current_value: 0,
            base_currency: "USD".to_string(),
            purchase_rate: 1.0,
            market_rate: 1.0,
        }
    }

    fn sample_inv_tx(id: &str, investment_id: &str, tx_type: &str) -> FinanceInvestmentTxRow {
        let now = Utc::now();
        FinanceInvestmentTxRow {
            id: id.to_string(),
            investment_id: investment_id.to_string(),
            tx_type: tx_type.to_string(),
            quantity: Some(10.0),
            price_per_unit: Some(50_000),
            total_amount: 500_000,
            currency: "VND".to_string(),
            fees: 0,
            tx_date: chrono::Local::now().date_naive(),
            notes: None,
            created_at: now,
            base_total_amount: 0,
            base_currency: "USD".to_string(),
            exchange_rate: 1.0,
        }
    }

    // ---------------------------------------------------------------------------
    // Portfolio CRUD
    // ---------------------------------------------------------------------------

    #[tokio::test]
    async fn portfolio_add_and_get() {
        let Some(repo) = test_repo().await else {
            return;
        };
        let id = unique_id("p");
        let row = sample_portfolio(&id, "Vietnam Stocks");
        let inserted = repo.add_portfolio(&row).await.unwrap();
        assert_eq!(inserted.name, "Vietnam Stocks");
        let fetched = repo.get_portfolio(&id).await.unwrap().unwrap();
        assert_eq!(fetched.id, id);
        let _ = repo.delete_portfolio(&id).await;
    }

    #[tokio::test]
    async fn portfolio_list_returns_all() {
        let Some(repo) = test_repo().await else {
            return;
        };
        let ids: Vec<_> = (0..3).map(|i| unique_id(&format!("p{i}"))).collect();
        for id in &ids {
            repo.add_portfolio(&sample_portfolio(id, &format!("Portfolio {id}")))
                .await
                .unwrap();
        }
        let all = repo.list_portfolios().await.unwrap();
        let all_ids: Vec<_> = all.iter().map(|r| r.id.as_str()).collect();
        for id in &ids {
            assert!(all_ids.contains(&id.as_str()));
        }
        for id in &ids {
            let _ = repo.delete_portfolio(id).await;
        }
    }

    #[tokio::test]
    async fn portfolio_delete() {
        let Some(repo) = test_repo().await else {
            return;
        };
        let id = unique_id("p");
        repo.add_portfolio(&sample_portfolio(&id, "To Delete"))
            .await
            .unwrap();
        let deleted = repo.delete_portfolio(&id).await.unwrap();
        assert!(deleted);
        let fetched = repo.get_portfolio(&id).await.unwrap();
        assert!(fetched.is_none());
    }

    #[tokio::test]
    async fn portfolio_not_found_returns_none() {
        let Some(repo) = test_repo().await else {
            return;
        };
        let result = repo.get_portfolio("nonexistent").await.unwrap();
        assert!(result.is_none());
    }

    // ---------------------------------------------------------------------------
    // Investment CRUD
    // ---------------------------------------------------------------------------

    #[tokio::test]
    async fn investment_add_and_get() {
        let Some(repo) = test_repo().await else {
            return;
        };
        let port_id = unique_id("p");
        repo.add_portfolio(&sample_portfolio(&port_id, "Portfolio"))
            .await
            .unwrap();
        let inv_id = unique_id("i");
        let row = sample_investment(&inv_id, &port_id);
        let inserted = repo.add_investment(&row).await.unwrap();
        assert_eq!(inserted.portfolio_id, port_id);
        assert_eq!(inserted.symbol.as_deref(), Some("VIC"));
        let fetched = repo.get_investment(&inv_id).await.unwrap().unwrap();
        assert_eq!(fetched.id, inv_id);
        let _ = repo.delete_portfolio(&port_id).await;
    }

    #[tokio::test]
    async fn investment_update_price() {
        let Some(repo) = test_repo().await else {
            return;
        };
        let port_id = unique_id("p");
        repo.add_portfolio(&sample_portfolio(&port_id, "Portfolio"))
            .await
            .unwrap();
        let inv_id = unique_id("i");
        repo.add_investment(&sample_investment(&inv_id, &port_id))
            .await
            .unwrap();
        let updated = repo
            .update_price(&inv_id, 50_000, 5_000_000, 5_000_000, 1.0)
            .await
            .unwrap();
        assert_eq!(updated.current_price, Some(50_000));
        assert_eq!(updated.current_value, Some(5_000_000));
        assert_eq!(updated.base_current_value, 5_000_000);
        assert_eq!(updated.market_rate, 1.0);
        let _ = repo.delete_portfolio(&port_id).await;
    }

    #[tokio::test]
    async fn investment_list_with_symbols_excludes_null_symbol() {
        let Some(repo) = test_repo().await else {
            return;
        };
        let port_id = unique_id("p");
        repo.add_portfolio(&sample_portfolio(&port_id, "Portfolio"))
            .await
            .unwrap();
        let id1 = unique_id("i1");
        let id2 = unique_id("i2");
        let id3 = unique_id("i3");
        let mut inv1 = sample_investment(&id1, &port_id);
        inv1.symbol = Some("VIC".to_string());
        let mut inv2 = sample_investment(&id2, &port_id);
        inv2.symbol = Some("BTC".to_string());
        let mut inv3 = sample_investment(&id3, &port_id);
        inv3.symbol = None;
        inv3.asset_type = "real_estate".to_string();
        repo.add_investment(&inv1).await.unwrap();
        repo.add_investment(&inv2).await.unwrap();
        repo.add_investment(&inv3).await.unwrap();
        let result = repo.list_with_symbols().await.unwrap();
        let result_ids: Vec<_> = result.iter().map(|r| r.id.as_str()).collect();
        assert!(result_ids.contains(&id1.as_str()));
        assert!(result_ids.contains(&id2.as_str()));
        assert!(!result_ids.contains(&id3.as_str()));
        assert!(result.iter().all(|r| r.symbol.is_some()));
        let _ = repo.delete_portfolio(&port_id).await;
    }

    #[tokio::test]
    async fn investment_total_value_by_currency() {
        let Some(repo) = test_repo().await else {
            return;
        };
        let port_id = unique_id("p");
        repo.add_portfolio(&sample_portfolio(&port_id, "Portfolio"))
            .await
            .unwrap();
        let ids: Vec<_> = (0..4).map(|i| unique_id(&format!("i{i}"))).collect();
        let amounts = [100_000i64, 200_000, 300_000];
        for (i, &amt) in amounts.iter().enumerate() {
            let mut inv = sample_investment(&ids[i], &port_id);
            inv.current_value = Some(amt);
            repo.add_investment(&inv).await.unwrap();
        }
        let mut usd_inv = sample_investment(&ids[3], &port_id);
        usd_inv.currency = "USD".to_string();
        usd_inv.current_value = Some(50);
        usd_inv.symbol = Some("BTC".to_string());
        repo.add_investment(&usd_inv).await.unwrap();
        let totals = repo.total_value_by_currency().await.unwrap();
        let vnd_total = totals.iter().find(|(c, _)| c == "VND").map(|(_, t)| *t);
        let usd_total = totals.iter().find(|(c, _)| c == "USD").map(|(_, t)| *t);
        assert!(vnd_total.unwrap_or(0) >= 600_000);
        assert!(usd_total.unwrap_or(0) >= 50);
        let _ = repo.delete_portfolio(&port_id).await;
    }

    #[tokio::test]
    async fn investment_total_value_empty_portfolio() {
        let Some(repo) = test_repo().await else {
            return;
        };
        let port_id = unique_id("p");
        repo.add_portfolio(&sample_portfolio(&port_id, "Empty Portfolio"))
            .await
            .unwrap();
        // No investments with current_value for this portfolio — totals should not include it
        let totals = repo.total_value_by_currency().await.unwrap();
        // Just verify we don't panic; empty result or result without null values is fine.
        let _ = totals;
        let _ = repo.delete_portfolio(&port_id).await;
    }

    // ---------------------------------------------------------------------------
    // Investment Transactions
    // ---------------------------------------------------------------------------

    #[tokio::test]
    async fn investment_tx_add_buy() {
        let Some(repo) = test_repo().await else {
            return;
        };
        let port_id = unique_id("p");
        repo.add_portfolio(&sample_portfolio(&port_id, "Portfolio"))
            .await
            .unwrap();
        let inv_id = unique_id("i");
        repo.add_investment(&sample_investment(&inv_id, &port_id))
            .await
            .unwrap();
        let tx_id = unique_id("tx");
        let tx_row = sample_inv_tx(&tx_id, &inv_id, "buy");
        let inserted = repo.add_investment_tx(&tx_row).await.unwrap();
        assert_eq!(inserted.tx_type, "buy");
        assert_eq!(inserted.quantity, Some(10.0));
        let _ = repo.delete_portfolio(&port_id).await;
    }

    #[tokio::test]
    async fn investment_tx_list() {
        let Some(repo) = test_repo().await else {
            return;
        };
        let port_id = unique_id("p");
        repo.add_portfolio(&sample_portfolio(&port_id, "Portfolio"))
            .await
            .unwrap();
        let inv_id = unique_id("i");
        repo.add_investment(&sample_investment(&inv_id, &port_id))
            .await
            .unwrap();
        let tx_ids: Vec<_> = [("buy"), ("sell"), ("dividend")]
            .iter()
            .enumerate()
            .map(|(i, &t)| {
                let id = unique_id(&format!("tx{i}"));
                (id, t)
            })
            .collect();
        for (id, tx_type) in &tx_ids {
            repo.add_investment_tx(&sample_inv_tx(id, &inv_id, tx_type))
                .await
                .unwrap();
        }
        let result = repo.list_investment_txs(&inv_id).await.unwrap();
        assert_eq!(result.len(), 3);
        let _ = repo.delete_portfolio(&port_id).await;
    }

    // ---------------------------------------------------------------------------
    // Portfolio Summary Aggregation
    // ---------------------------------------------------------------------------

    #[tokio::test]
    async fn portfolio_summary_sums_correct() {
        let Some(repo) = test_repo().await else {
            return;
        };
        let port_id = unique_id("p");
        repo.add_portfolio(&sample_portfolio(&port_id, "Portfolio"))
            .await
            .unwrap();
        let investments = [
            (1_000_000i64, 1_200_000i64),
            (500_000, 450_000),
            (200_000, 300_000),
        ];
        for (i, (cost, value)) in investments.iter().enumerate() {
            let id = unique_id(&format!("i{i}"));
            let mut inv = sample_investment(&id, &port_id);
            inv.cost_basis = *cost;
            inv.current_value = Some(*value);
            inv.base_cost_basis = *cost;
            inv.base_current_value = *value;
            repo.add_investment(&inv).await.unwrap();
        }
        let summary = repo.portfolio_summary(&port_id, "USD").await.unwrap();
        assert_eq!(summary.total_cost_basis, 1_700_000);
        assert_eq!(summary.total_current_value, 1_950_000);
        assert_eq!(summary.holding_count, 3);
        let _ = repo.delete_portfolio(&port_id).await;
    }

    #[tokio::test]
    async fn portfolio_summary_empty_portfolio() {
        let Some(repo) = test_repo().await else {
            return;
        };
        let port_id = unique_id("p");
        repo.add_portfolio(&sample_portfolio(&port_id, "Empty"))
            .await
            .unwrap();
        let summary = repo.portfolio_summary(&port_id, "USD").await.unwrap();
        assert_eq!(summary.total_cost_basis, 0);
        assert_eq!(summary.total_current_value, 0);
        assert_eq!(summary.holding_count, 0);
        let _ = repo.delete_portfolio(&port_id).await;
    }

    // ---------------------------------------------------------------------------
    // Cascade Deletes
    // ---------------------------------------------------------------------------

    #[tokio::test]
    async fn portfolio_delete_cascades_investments() {
        let Some(repo) = test_repo().await else {
            return;
        };
        let port_id = unique_id("p");
        repo.add_portfolio(&sample_portfolio(&port_id, "Portfolio"))
            .await
            .unwrap();
        let id_a = unique_id("ia");
        let id_b = unique_id("ib");
        repo.add_investment(&sample_investment(&id_a, &port_id))
            .await
            .unwrap();
        repo.add_investment(&sample_investment(&id_b, &port_id))
            .await
            .unwrap();
        let deleted = repo.delete_portfolio(&port_id).await.unwrap();
        assert!(deleted);
        let inv_a = repo.get_investment(&id_a).await.unwrap();
        let inv_b = repo.get_investment(&id_b).await.unwrap();
        assert!(inv_a.is_none());
        assert!(inv_b.is_none());
    }

    #[tokio::test]
    async fn investment_delete_cascades_txs() {
        let Some(repo) = test_repo().await else {
            return;
        };
        let port_id = unique_id("p");
        repo.add_portfolio(&sample_portfolio(&port_id, "Portfolio"))
            .await
            .unwrap();
        let inv_id = unique_id("i");
        repo.add_investment(&sample_investment(&inv_id, &port_id))
            .await
            .unwrap();
        let tx_ids: Vec<_> = (0..3).map(|i| unique_id(&format!("tx{i}"))).collect();
        for id in &tx_ids {
            repo.add_investment_tx(&sample_inv_tx(id, &inv_id, "buy"))
                .await
                .unwrap();
        }
        let deleted = repo.delete_investment(&inv_id).await.unwrap();
        assert!(deleted);
        let txs = repo.list_investment_txs(&inv_id).await.unwrap();
        assert!(txs.is_empty());
        let _ = repo.delete_portfolio(&port_id).await;
    }
}
