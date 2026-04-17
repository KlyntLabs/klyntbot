//! Tests for FinanceTransactionRepo.

#[cfg(test)]
mod tests {

    use crate::repos::finance_account_repo::FinanceAccountRepo;
    use crate::repos::finance_transaction_repo::FinanceTransactionRepo;
    use crate::rows::finance::{
        FinanceAccountRow, FinanceTransactionFilter, FinanceTransactionPatch, FinanceTransactionRow,
    };
    use crate::StoragePool;

    struct TestRepos {
        accounts: FinanceAccountRepo,
        transactions: FinanceTransactionRepo,
    }

    async fn test_repos() -> Option<TestRepos> {
        let dir = tempfile::tempdir().ok()?;
        let pool = StoragePool::connect(dir.path()).await.ok()?;
        let _ = dir.keep(); // prevent cleanup; acceptable in test context
        let pg = pool.inner().clone();
        Some(TestRepos {
            accounts: FinanceAccountRepo::new(pg.clone()),
            transactions: FinanceTransactionRepo::new(pg),
        })
    }

    fn unique_id(prefix: &str) -> String {
        format!("{prefix}-{}", uuid::Uuid::new_v4().as_simple())
    }

    /// Generate a unique category string to prevent parallel test contamination.
    fn unique_cat(base: &str) -> String {
        format!("{base}-{}", &uuid::Uuid::new_v4().to_string()[..8])
    }

    fn sample_account(id: &str) -> FinanceAccountRow {
        let now: crate::sqlite_types::SqlTs = jiff::Timestamp::now().into();
        FinanceAccountRow {
            id: id.to_string(),
            name: "Test Account".to_string(),
            account_type: "bank".to_string(),
            currency: "VND".to_string(),
            balance: 0,
            institution: None,
            notes: None,
            is_archived: false,
            created_at: now,
            updated_at: now,
            base_balance: 0,
            base_currency: "USD".to_string(),
            exchange_rate: 1.0,
        }
    }

    fn sample_tx(id: &str, account_id: &str, tx_type: &str, amount: i64) -> FinanceTransactionRow {
        let now: crate::sqlite_types::SqlTs = jiff::Timestamp::now().into();
        FinanceTransactionRow {
            id: id.to_string(),
            account_id: account_id.to_string(),
            tx_type: tx_type.to_string(),
            amount,
            currency: "VND".to_string(),
            category: None,
            subcategory: None,
            counterparty: None,
            notes: None,
            tx_date: crate::sqlite_types::SqlDate::from(jiff::Zoned::now().date()),
            transfer_id: None,
            is_recurring: false,
            recurring_rule: None,
            created_at: now,
            updated_at: now,
            base_amount: amount,
            base_currency: "USD".to_string(),
            exchange_rate: 1.0,
        }
    }

    #[tokio::test]
    async fn tx_add_and_get() {
        let Some(repos) = test_repos().await else {
            return;
        };
        let acct_id = unique_id("a");
        repos.accounts.add(&sample_account(&acct_id)).await.unwrap();
        let tx_id = unique_id("tx");
        let row = sample_tx(&tx_id, &acct_id, "expense", 100_000);
        let inserted = repos.transactions.add(&row).await.unwrap();
        assert!(!inserted.id.is_empty());
        assert_eq!(inserted.account_id, acct_id);
        assert_eq!(inserted.amount, 100_000);
        let fetched = repos.transactions.get(&tx_id).await.unwrap().unwrap();
        assert_eq!(fetched.id, tx_id);
        let _ = repos.transactions.delete(&tx_id).await;
        let _ = repos.accounts.delete(&acct_id).await;
    }

    #[tokio::test]
    async fn tx_get_not_found() {
        let Some(repos) = test_repos().await else {
            return;
        };
        let result = repos.transactions.get("nonexistent-tx-id").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn tx_update_amount() {
        let Some(repos) = test_repos().await else {
            return;
        };
        let acct_id = unique_id("a");
        repos.accounts.add(&sample_account(&acct_id)).await.unwrap();
        let tx_id = unique_id("tx");
        repos
            .transactions
            .add(&sample_tx(&tx_id, &acct_id, "expense", 100_000))
            .await
            .unwrap();
        let patch = FinanceTransactionPatch {
            id: tx_id.clone(),
            amount: Some(200_000),
            ..Default::default()
        };
        let updated = repos.transactions.update(&patch).await.unwrap();
        assert_eq!(updated.amount, 200_000);
        let _ = repos.transactions.delete(&tx_id).await;
        let _ = repos.accounts.delete(&acct_id).await;
    }

    #[tokio::test]
    async fn tx_delete_returns_deleted_row() {
        let Some(repos) = test_repos().await else {
            return;
        };
        let acct_id = unique_id("a");
        repos.accounts.add(&sample_account(&acct_id)).await.unwrap();
        let tx_id = unique_id("tx");
        repos
            .transactions
            .add(&sample_tx(&tx_id, &acct_id, "income", 50_000))
            .await
            .unwrap();
        let deleted = repos.transactions.delete(&tx_id).await.unwrap();
        assert!(deleted.is_some());
        assert_eq!(deleted.unwrap().id, tx_id);
        let fetched = repos.transactions.get(&tx_id).await.unwrap();
        assert!(fetched.is_none());
        let _ = repos.accounts.delete(&acct_id).await;
    }

    #[tokio::test]
    async fn tx_delete_not_found_returns_none() {
        let Some(repos) = test_repos().await else {
            return;
        };
        let result = repos.transactions.delete("bad-id").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn tx_list_filter_by_account_id() {
        let Some(repos) = test_repos().await else {
            return;
        };
        let acct_a = unique_id("aa");
        let acct_b = unique_id("ab");
        repos.accounts.add(&sample_account(&acct_a)).await.unwrap();
        repos.accounts.add(&sample_account(&acct_b)).await.unwrap();
        let ids_a: Vec<_> = (0..3).map(|i| unique_id(&format!("ta{i}"))).collect();
        let ids_b: Vec<_> = (0..2).map(|i| unique_id(&format!("tb{i}"))).collect();
        for id in &ids_a {
            repos
                .transactions
                .add(&sample_tx(id, &acct_a, "expense", 1000))
                .await
                .unwrap();
        }
        for id in &ids_b {
            repos
                .transactions
                .add(&sample_tx(id, &acct_b, "expense", 2000))
                .await
                .unwrap();
        }
        let filter = FinanceTransactionFilter {
            account_id: Some(acct_a.clone()),
            limit: Some(100),
            ..Default::default()
        };
        let result = repos.transactions.list(&filter).await.unwrap();
        let result_ids: Vec<_> = result.iter().map(|r| r.id.as_str()).collect();
        for id in &ids_a {
            assert!(result_ids.contains(&id.as_str()));
        }
        for id in &ids_b {
            assert!(!result_ids.contains(&id.as_str()));
        }
        for id in ids_a.iter().chain(ids_b.iter()) {
            let _ = repos.transactions.delete(id).await;
        }
        let _ = repos.accounts.delete(&acct_a).await;
        let _ = repos.accounts.delete(&acct_b).await;
    }

    #[tokio::test]
    async fn tx_list_filter_by_type_expense() {
        let Some(repos) = test_repos().await else {
            return;
        };
        let acct_id = unique_id("a");
        repos.accounts.add(&sample_account(&acct_id)).await.unwrap();
        let id_e1 = unique_id("e1");
        let id_e2 = unique_id("e2");
        let id_inc = unique_id("inc");
        let id_tr = unique_id("tr");
        repos
            .transactions
            .add(&sample_tx(&id_e1, &acct_id, "expense", 1000))
            .await
            .unwrap();
        repos
            .transactions
            .add(&sample_tx(&id_e2, &acct_id, "expense", 2000))
            .await
            .unwrap();
        repos
            .transactions
            .add(&sample_tx(&id_inc, &acct_id, "income", 3000))
            .await
            .unwrap();
        repos
            .transactions
            .add(&sample_tx(&id_tr, &acct_id, "transfer", 500))
            .await
            .unwrap();
        let filter = FinanceTransactionFilter {
            account_id: Some(acct_id.clone()),
            tx_type: Some("expense".to_string()),
            limit: Some(100),
            ..Default::default()
        };
        let result = repos.transactions.list(&filter).await.unwrap();
        let ids: Vec<_> = result.iter().map(|r| r.id.as_str()).collect();
        assert!(ids.contains(&id_e1.as_str()));
        assert!(ids.contains(&id_e2.as_str()));
        assert!(!ids.contains(&id_inc.as_str()));
        assert!(!ids.contains(&id_tr.as_str()));
        assert!(result.iter().all(|r| r.tx_type == "expense"));
        for id in &[&id_e1, &id_e2, &id_inc, &id_tr] {
            let _ = repos.transactions.delete(id).await;
        }
        let _ = repos.accounts.delete(&acct_id).await;
    }

    #[tokio::test]
    async fn tx_list_filter_by_date_range() {
        let Some(repos) = test_repos().await else {
            return;
        };
        let acct_id = unique_id("a");
        repos.accounts.add(&sample_account(&acct_id)).await.unwrap();
        let id_jan = unique_id("jan");
        let id_feb = unique_id("feb");
        let id_mar = unique_id("mar");
        let mut tx_jan = sample_tx(&id_jan, &acct_id, "expense", 1000);
        tx_jan.tx_date =
            crate::sqlite_types::SqlDate::from(jiff::civil::Date::new(2026, 1, 15).unwrap());
        let mut tx_feb = sample_tx(&id_feb, &acct_id, "expense", 2000);
        tx_feb.tx_date =
            crate::sqlite_types::SqlDate::from(jiff::civil::Date::new(2026, 2, 10).unwrap());
        let mut tx_mar = sample_tx(&id_mar, &acct_id, "expense", 3000);
        tx_mar.tx_date =
            crate::sqlite_types::SqlDate::from(jiff::civil::Date::new(2026, 3, 5).unwrap());
        repos.transactions.add(&tx_jan).await.unwrap();
        repos.transactions.add(&tx_feb).await.unwrap();
        repos.transactions.add(&tx_mar).await.unwrap();
        let filter = FinanceTransactionFilter {
            account_id: Some(acct_id.clone()),
            date_from: Some(crate::sqlite_types::SqlDate::from(
                jiff::civil::Date::new(2026, 2, 1).unwrap(),
            )),
            date_to: Some(crate::sqlite_types::SqlDate::from(
                jiff::civil::Date::new(2026, 2, 28).unwrap(),
            )),
            limit: Some(100),
            ..Default::default()
        };
        let result = repos.transactions.list(&filter).await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].tx_date,
            crate::sqlite_types::SqlDate::from(jiff::civil::Date::new(2026, 2, 10).unwrap())
        );
        for id in &[&id_jan, &id_feb, &id_mar] {
            let _ = repos.transactions.delete(id).await;
        }
        let _ = repos.accounts.delete(&acct_id).await;
    }

    #[tokio::test]
    async fn tx_list_filter_by_category() {
        let Some(repos) = test_repos().await else {
            return;
        };
        let acct_id = unique_id("a");
        repos.accounts.add(&sample_account(&acct_id)).await.unwrap();
        let food_ids: Vec<_> = (0..3).map(|i| unique_id(&format!("fd{i}"))).collect();
        for id in &food_ids {
            let mut tx = sample_tx(id, &acct_id, "expense", 1000);
            tx.category = Some("food".to_string());
            repos.transactions.add(&tx).await.unwrap();
        }
        let filter = FinanceTransactionFilter {
            account_id: Some(acct_id.clone()),
            category: Some("food".to_string()),
            limit: Some(100),
            ..Default::default()
        };
        let result = repos.transactions.list(&filter).await.unwrap();
        let ids: Vec<_> = result.iter().map(|r| r.id.as_str()).collect();
        for id in &food_ids {
            assert!(ids.contains(&id.as_str()));
        }
        assert_eq!(result.len(), food_ids.len());
        for id in &food_ids {
            let _ = repos.transactions.delete(id).await;
        }
        let _ = repos.accounts.delete(&acct_id).await;
    }

    #[tokio::test]
    async fn tx_list_no_filter_returns_recent_20() {
        let Some(repos) = test_repos().await else {
            return;
        };
        let acct_id = unique_id("a");
        repos.accounts.add(&sample_account(&acct_id)).await.unwrap();
        let ids: Vec<_> = (0..25).map(|i| unique_id(&format!("t{i}"))).collect();
        for id in &ids {
            repos
                .transactions
                .add(&sample_tx(id, &acct_id, "expense", 1000))
                .await
                .unwrap();
        }
        let filter = FinanceTransactionFilter {
            account_id: Some(acct_id.clone()),
            ..Default::default()
        };
        let result = repos.transactions.list(&filter).await.unwrap();
        assert_eq!(result.len(), 20);
        for id in &ids {
            let _ = repos.transactions.delete(id).await;
        }
        let _ = repos.accounts.delete(&acct_id).await;
    }

    #[tokio::test]
    async fn tx_get_by_transfer_id_returns_both_rows() {
        let Some(repos) = test_repos().await else {
            return;
        };
        let src_id = unique_id("src");
        let dst_id = unique_id("dst");
        repos.accounts.add(&sample_account(&src_id)).await.unwrap();
        repos.accounts.add(&sample_account(&dst_id)).await.unwrap();
        let transfer_id = uuid::Uuid::new_v4().to_string();
        let tx_src_id = unique_id("txs");
        let tx_dst_id = unique_id("txd");
        let mut tx_src = sample_tx(&tx_src_id, &src_id, "expense", 100_000);
        tx_src.transfer_id = Some(transfer_id.clone());
        let mut tx_dst = sample_tx(&tx_dst_id, &dst_id, "income", 100_000);
        tx_dst.transfer_id = Some(transfer_id.clone());
        repos.transactions.add(&tx_src).await.unwrap();
        repos.transactions.add(&tx_dst).await.unwrap();
        let pair = repos
            .transactions
            .get_by_transfer_id(&transfer_id)
            .await
            .unwrap();
        assert_eq!(pair.len(), 2);
        let types: Vec<_> = pair.iter().map(|r| r.tx_type.as_str()).collect();
        assert!(types.contains(&"expense"));
        assert!(types.contains(&"income"));
        assert!(pair
            .iter()
            .all(|r| r.transfer_id.as_deref() == Some(&transfer_id)));
        let _ = repos.transactions.delete(&tx_src_id).await;
        let _ = repos.transactions.delete(&tx_dst_id).await;
        let _ = repos.accounts.delete(&src_id).await;
        let _ = repos.accounts.delete(&dst_id).await;
    }

    #[tokio::test]
    async fn tx_get_by_transfer_id_missing_returns_empty() {
        let Some(repos) = test_repos().await else {
            return;
        };
        let result = repos
            .transactions
            .get_by_transfer_id("nonexistent-transfer-id")
            .await
            .unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn tx_sum_by_category() {
        let Some(repos) = test_repos().await else {
            return;
        };
        let acct_id = unique_id("a");
        repos.accounts.add(&sample_account(&acct_id)).await.unwrap();
        let feb1 = jiff::civil::Date::new(2026, 2, 1).unwrap();
        let feb28 = jiff::civil::Date::new(2026, 2, 28).unwrap();
        let feb15 = jiff::civil::Date::new(2026, 2, 15).unwrap();
        let cat_food = unique_cat("food");
        let cat_transport = unique_cat("transport");
        let food_ids: Vec<_> = (0..3).map(|i| unique_id(&format!("sf{i}"))).collect();
        for id in &food_ids {
            let mut tx = sample_tx(id, &acct_id, "expense", 100_000);
            tx.category = Some(cat_food.clone());
            tx.tx_date = crate::sqlite_types::SqlDate::from(feb15);
            repos.transactions.add(&tx).await.unwrap();
        }
        let tr_ids: Vec<_> = (0..2).map(|i| unique_id(&format!("st{i}"))).collect();
        for id in &tr_ids {
            let mut tx = sample_tx(id, &acct_id, "expense", 50_000);
            tx.category = Some(cat_transport.clone());
            tx.tx_date = crate::sqlite_types::SqlDate::from(feb15);
            repos.transactions.add(&tx).await.unwrap();
        }
        let inc_id = unique_id("si");
        let mut tx_inc = sample_tx(&inc_id, &acct_id, "income", 500_000);
        tx_inc.category = Some(cat_food.clone());
        tx_inc.tx_date = crate::sqlite_types::SqlDate::from(feb15);
        repos.transactions.add(&tx_inc).await.unwrap();
        let sums = repos
            .transactions
            .sum_by_category(feb1, feb28, "expense", "USD")
            .await
            .unwrap();
        let food_sum = sums.iter().find(|(c, _)| c == &cat_food).map(|(_, t)| *t);
        let tr_sum = sums
            .iter()
            .find(|(c, _)| c == &cat_transport)
            .map(|(_, t)| *t);
        assert_eq!(food_sum, Some(300_000));
        assert_eq!(tr_sum, Some(100_000));
        for id in food_ids.iter().chain(tr_ids.iter()) {
            let _ = repos.transactions.delete(id).await;
        }
        let _ = repos.transactions.delete(&inc_id).await;
        let _ = repos.accounts.delete(&acct_id).await;
    }

    #[tokio::test]
    async fn tx_sum_by_period_monthly() {
        let Some(repos) = test_repos().await else {
            return;
        };
        let acct_id = unique_id("a");
        repos.accounts.add(&sample_account(&acct_id)).await.unwrap();
        let mut tx_ids = Vec::new();
        for month_offset in 0i32..6 {
            let id = unique_id(&format!("sp{month_offset}"));
            let base_month = 2i32;
            let month = (base_month - month_offset - 1).rem_euclid(12) + 1;
            let actual_year = 2026i32 + (base_month - month_offset - 1).div_euclid(12);
            let date = jiff::civil::Date::new(actual_year as i16, month as i8, 15).unwrap();
            let mut tx = sample_tx(&id, &acct_id, "expense", 10_000 * (month_offset as i64 + 1));
            tx.tx_date = crate::sqlite_types::SqlDate::from(date);
            repos.transactions.add(&tx).await.unwrap();
            tx_ids.push(id);
        }
        let result = repos
            .transactions
            .sum_by_period("expense", 6, "month", "USD")
            .await
            .unwrap();
        assert!(!result.is_empty());
        assert!(result.iter().all(|(label, _)| label.len() == 7));
        for id in &tx_ids {
            let _ = repos.transactions.delete(id).await;
        }
        let _ = repos.accounts.delete(&acct_id).await;
    }

    #[tokio::test]
    async fn tx_category_history_returns_patterns() {
        let Some(repos) = test_repos().await else {
            return;
        };
        let acct_id = unique_id("a");
        repos.accounts.add(&sample_account(&acct_id)).await.unwrap();
        let vin_ids: Vec<_> = (0..5).map(|i| unique_id(&format!("vm{i}"))).collect();
        for id in &vin_ids {
            let mut tx = sample_tx(id, &acct_id, "expense", 1000);
            tx.counterparty = Some("VinMart".to_string());
            tx.category = Some("food".to_string());
            repos.transactions.add(&tx).await.unwrap();
        }
        let grab_ids: Vec<_> = (0..3).map(|i| unique_id(&format!("gb{i}"))).collect();
        for id in &grab_ids {
            let mut tx = sample_tx(id, &acct_id, "expense", 2000);
            tx.counterparty = Some("Grab".to_string());
            tx.category = Some("transport".to_string());
            repos.transactions.add(&tx).await.unwrap();
        }
        let history = repos.transactions.category_history(10).await.unwrap();
        let vin_entry = history
            .iter()
            .find(|(cp, cat, _)| cp == "VinMart" && cat == "food");
        let grab_entry = history
            .iter()
            .find(|(cp, cat, _)| cp == "Grab" && cat == "transport");
        assert!(vin_entry.is_some());
        assert!(grab_entry.is_some());
        assert_eq!(vin_entry.unwrap().2, 5);
        assert_eq!(grab_entry.unwrap().2, 3);
        for id in vin_ids.iter().chain(grab_ids.iter()) {
            let _ = repos.transactions.delete(id).await;
        }
        let _ = repos.accounts.delete(&acct_id).await;
    }

    #[tokio::test]
    async fn tx_search_by_keyword_notes() {
        let Some(repos) = test_repos().await else {
            return;
        };
        let acct_id = unique_id("a");
        repos.accounts.add(&sample_account(&acct_id)).await.unwrap();
        let id1 = unique_id("sk1");
        let id2 = unique_id("sk2");
        let mut tx1 = sample_tx(&id1, &acct_id, "expense", 50_000);
        tx1.notes = Some("Mua hang tai VinMart".to_string());
        let mut tx2 = sample_tx(&id2, &acct_id, "expense", 30_000);
        tx2.notes = Some("Coffee with John".to_string());
        repos.transactions.add(&tx1).await.unwrap();
        repos.transactions.add(&tx2).await.unwrap();
        let filter = FinanceTransactionFilter {
            account_id: Some(acct_id.clone()),
            query: Some("vinmart".to_string()),
            limit: Some(100),
            ..Default::default()
        };
        let result = repos.transactions.list(&filter).await.unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0]
            .notes
            .as_deref()
            .unwrap_or("")
            .to_lowercase()
            .contains("vinmart"));
        let _ = repos.transactions.delete(&id1).await;
        let _ = repos.transactions.delete(&id2).await;
        let _ = repos.accounts.delete(&acct_id).await;
    }

    #[tokio::test]
    async fn tx_search_by_amount_range() {
        let Some(repos) = test_repos().await else {
            return;
        };
        let acct_id = unique_id("a");
        repos.accounts.add(&sample_account(&acct_id)).await.unwrap();
        let id1 = unique_id("ar1");
        let id2 = unique_id("ar2");
        let id3 = unique_id("ar3");
        repos
            .transactions
            .add(&sample_tx(&id1, &acct_id, "expense", 100_000))
            .await
            .unwrap();
        repos
            .transactions
            .add(&sample_tx(&id2, &acct_id, "expense", 500_000))
            .await
            .unwrap();
        repos
            .transactions
            .add(&sample_tx(&id3, &acct_id, "expense", 1_000_000))
            .await
            .unwrap();
        let filter = FinanceTransactionFilter {
            account_id: Some(acct_id.clone()),
            amount_min: Some(200_000),
            amount_max: Some(800_000),
            limit: Some(100),
            ..Default::default()
        };
        let result = repos.transactions.list(&filter).await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].amount, 500_000);
        for id in &[&id1, &id2, &id3] {
            let _ = repos.transactions.delete(id).await;
        }
        let _ = repos.accounts.delete(&acct_id).await;
    }

    #[tokio::test]
    async fn tx_add_future_date_allowed() {
        let Some(repos) = test_repos().await else {
            return;
        };
        let acct_id = unique_id("a");
        repos.accounts.add(&sample_account(&acct_id)).await.unwrap();
        let id = unique_id("fut");
        let mut row = sample_tx(&id, &acct_id, "expense", 50_000);
        let future_date = jiff::Zoned::now()
            .date()
            .checked_add(jiff::Span::new().days(30))
            .unwrap();
        row.tx_date = crate::sqlite_types::SqlDate::from(future_date);
        let inserted = repos.transactions.add(&row).await.unwrap();
        assert_eq!(
            inserted.tx_date,
            crate::sqlite_types::SqlDate::from(future_date)
        );
        let _ = repos.transactions.delete(&id).await;
        let _ = repos.accounts.delete(&acct_id).await;
    }

    #[tokio::test]
    async fn tx_add_recurring_template() {
        let Some(repos) = test_repos().await else {
            return;
        };
        let acct_id = unique_id("a");
        repos.accounts.add(&sample_account(&acct_id)).await.unwrap();
        let id = unique_id("rec");
        let mut row = sample_tx(&id, &acct_id, "expense", 100_000);
        row.is_recurring = true;
        row.recurring_rule = Some("0 0 1 * *".to_string());
        let inserted = repos.transactions.add(&row).await.unwrap();
        assert!(inserted.is_recurring);
        assert_eq!(inserted.recurring_rule.as_deref(), Some("0 0 1 * *"));
        let _ = repos.transactions.delete(&id).await;
        let _ = repos.accounts.delete(&acct_id).await;
    }
}
