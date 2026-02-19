//! Tests for FinanceBudgetRepo.

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, Utc};

    use crate::repos::finance_account_repo::FinanceAccountRepo;
    use crate::repos::finance_budget_repo::FinanceBudgetRepo;
    use crate::repos::finance_transaction_repo::FinanceTransactionRepo;
    use crate::rows::finance::{
        FinanceAccountRow, FinanceBudgetPatch, FinanceBudgetRow, FinanceTransactionRow,
    };
    use crate::StoragePool;

    struct TestRepos {
        accounts: FinanceAccountRepo,
        transactions: FinanceTransactionRepo,
        budgets: FinanceBudgetRepo,
    }

    async fn test_repos() -> Option<TestRepos> {
        let url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://localhost/klyntbot_test".to_string());
        match StoragePool::connect(&url).await {
            Ok(pool) => {
                let pg = pool.inner().clone();
                Some(TestRepos {
                    accounts: FinanceAccountRepo::new(pg.clone()),
                    transactions: FinanceTransactionRepo::new(pg.clone()),
                    budgets: FinanceBudgetRepo::new(pg),
                })
            }
            Err(_) => None,
        }
    }

    fn unique_id(prefix: &str) -> String {
        format!("{prefix}-{}", uuid::Uuid::new_v4().as_simple())
    }

    fn sample_budget(
        id: &str,
        name: &str,
        amount: i64,
        category: Option<&str>,
    ) -> FinanceBudgetRow {
        let now = Utc::now();
        FinanceBudgetRow {
            id: id.to_string(),
            name: name.to_string(),
            amount,
            currency: "VND".to_string(),
            period: "monthly".to_string(),
            category: category.map(|s| s.to_string()),
            method: "standard".to_string(),
            jar_type: None,
            start_date: NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(),
            end_date: None,
            is_active: true,
            alert_threshold: 80,
            created_at: now,
            updated_at: now,
        }
    }

    fn sample_account(id: &str) -> FinanceAccountRow {
        let now = Utc::now();
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
        }
    }

    fn sample_tx_with_category(
        id: &str,
        account_id: &str,
        tx_type: &str,
        amount: i64,
        category: &str,
        tx_date: NaiveDate,
    ) -> FinanceTransactionRow {
        let now = Utc::now();
        FinanceTransactionRow {
            id: id.to_string(),
            account_id: account_id.to_string(),
            tx_type: tx_type.to_string(),
            amount,
            currency: "VND".to_string(),
            category: Some(category.to_string()),
            subcategory: None,
            counterparty: None,
            notes: None,
            tx_date,
            transfer_id: None,
            is_recurring: false,
            recurring_rule: None,
            created_at: now,
            updated_at: now,
        }
    }

    // ---------------------------------------------------------------------------
    // CRUD
    // ---------------------------------------------------------------------------

    #[tokio::test]
    async fn budget_add_and_get() {
        let Some(repos) = test_repos().await else {
            return;
        };
        let id = unique_id("b");
        let row = sample_budget(&id, "Monthly Food", 5_000_000, Some("food"));
        let inserted = repos.budgets.add(&row).await.unwrap();
        assert_eq!(inserted.amount, 5_000_000);
        assert!(inserted.is_active);
        let fetched = repos.budgets.get(&id).await.unwrap().unwrap();
        assert_eq!(fetched.id, id);
        let _ = repos.budgets.delete(&id).await;
    }

    #[tokio::test]
    async fn budget_update_name_and_amount() {
        let Some(repos) = test_repos().await else {
            return;
        };
        let id = unique_id("b");
        repos
            .budgets
            .add(&sample_budget(&id, "Old Name", 1_000_000, Some("food")))
            .await
            .unwrap();
        let patch = FinanceBudgetPatch {
            id: id.clone(),
            name: Some("New Name".to_string()),
            amount: Some(2_000_000),
            ..Default::default()
        };
        let updated = repos.budgets.update(&patch).await.unwrap();
        assert_eq!(updated.name, "New Name");
        assert_eq!(updated.amount, 2_000_000);
        let _ = repos.budgets.delete(&id).await;
    }

    #[tokio::test]
    async fn budget_delete_existing() {
        let Some(repos) = test_repos().await else {
            return;
        };
        let id = unique_id("b");
        repos
            .budgets
            .add(&sample_budget(&id, "To Delete", 1_000_000, None))
            .await
            .unwrap();
        let deleted = repos.budgets.delete(&id).await.unwrap();
        assert!(deleted);
        let fetched = repos.budgets.get(&id).await.unwrap();
        assert!(fetched.is_none());
    }

    #[tokio::test]
    async fn budget_delete_not_found() {
        let Some(repos) = test_repos().await else {
            return;
        };
        let result = repos.budgets.delete("nonexistent-budget-id").await.unwrap();
        assert!(!result);
    }

    // ---------------------------------------------------------------------------
    // Listing
    // ---------------------------------------------------------------------------

    #[tokio::test]
    async fn budget_list_active_excludes_inactive() {
        let Some(repos) = test_repos().await else {
            return;
        };
        let id1 = unique_id("b1");
        let id2 = unique_id("b2");
        let id3 = unique_id("b3");
        repos
            .budgets
            .add(&sample_budget(&id1, "Active 1", 1_000_000, Some("food")))
            .await
            .unwrap();
        repos
            .budgets
            .add(&sample_budget(
                &id2,
                "Active 2",
                2_000_000,
                Some("transport"),
            ))
            .await
            .unwrap();
        let mut inactive = sample_budget(&id3, "Inactive", 3_000_000, Some("entertainment"));
        inactive.is_active = false;
        repos.budgets.add(&inactive).await.unwrap();
        let result = repos.budgets.list_active().await.unwrap();
        let ids: Vec<_> = result.iter().map(|r| r.id.as_str()).collect();
        assert!(ids.contains(&id1.as_str()));
        assert!(ids.contains(&id2.as_str()));
        assert!(!ids.contains(&id3.as_str()));
        assert!(result.iter().all(|r| r.is_active));
        for id in &[&id1, &id2, &id3] {
            let _ = repos.budgets.delete(id).await;
        }
    }

    #[tokio::test]
    async fn budget_get_by_category() {
        let Some(repos) = test_repos().await else {
            return;
        };
        let id = unique_id("b");
        repos
            .budgets
            .add(&sample_budget(&id, "Food Budget", 5_000_000, Some("food")))
            .await
            .unwrap();
        let result = repos.budgets.get_by_category("food").await.unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().category.as_deref(), Some("food"));
        let _ = repos.budgets.delete(&id).await;
    }

    #[tokio::test]
    async fn budget_get_by_category_not_found() {
        let Some(repos) = test_repos().await else {
            return;
        };
        let result = repos.budgets.get_by_category("education").await.unwrap();
        assert!(result.is_none());
    }

    // ---------------------------------------------------------------------------
    // Budget Usage SQL Join
    // ---------------------------------------------------------------------------

    #[tokio::test]
    async fn budget_usage_no_transactions() {
        let Some(repos) = test_repos().await else {
            return;
        };
        let budget_id = unique_id("b");
        repos
            .budgets
            .add(&sample_budget(
                &budget_id,
                "Entertainment",
                2_000_000,
                Some("entertainment"),
            ))
            .await
            .unwrap();
        let usage = repos.budgets.budget_usage(&budget_id).await.unwrap();
        assert_eq!(usage.spent, 0);
        let _ = repos.budgets.delete(&budget_id).await;
    }

    #[tokio::test]
    async fn budget_usage_with_matching_transactions() {
        let Some(repos) = test_repos().await else {
            return;
        };
        let acct_id = unique_id("a");
        repos.accounts.add(&sample_account(&acct_id)).await.unwrap();
        let budget_id = unique_id("b");
        repos
            .budgets
            .add(&sample_budget(
                &budget_id,
                "Food Budget",
                5_000_000,
                Some("food"),
            ))
            .await
            .unwrap();
        let feb15 = NaiveDate::from_ymd_opt(2026, 2, 15).unwrap();
        let ids: Vec<_> = [(100_000i64), (200_000), (300_000)]
            .iter()
            .enumerate()
            .map(|(i, &amt)| {
                let id = unique_id(&format!("t{i}"));
                (id, amt)
            })
            .collect();
        for (id, amt) in &ids {
            repos
                .transactions
                .add(&sample_tx_with_category(
                    id, &acct_id, "expense", *amt, "food", feb15,
                ))
                .await
                .unwrap();
        }
        let usage = repos.budgets.budget_usage(&budget_id).await.unwrap();
        assert_eq!(usage.spent, 600_000);
        for (id, _) in &ids {
            let _ = repos.transactions.delete(id).await;
        }
        let _ = repos.budgets.delete(&budget_id).await;
        let _ = repos.accounts.delete(&acct_id).await;
    }

    #[tokio::test]
    async fn budget_usage_only_counts_expenses() {
        let Some(repos) = test_repos().await else {
            return;
        };
        let acct_id = unique_id("a");
        repos.accounts.add(&sample_account(&acct_id)).await.unwrap();
        let budget_id = unique_id("b");
        repos
            .budgets
            .add(&sample_budget(
                &budget_id,
                "Food Budget",
                5_000_000,
                Some("food"),
            ))
            .await
            .unwrap();
        let feb15 = NaiveDate::from_ymd_opt(2026, 2, 15).unwrap();
        let exp_id = unique_id("e");
        let inc_id = unique_id("i");
        repos
            .transactions
            .add(&sample_tx_with_category(
                &exp_id, &acct_id, "expense", 200_000, "food", feb15,
            ))
            .await
            .unwrap();
        repos
            .transactions
            .add(&sample_tx_with_category(
                &inc_id, &acct_id, "income", 500_000, "food", feb15,
            ))
            .await
            .unwrap();
        let usage = repos.budgets.budget_usage(&budget_id).await.unwrap();
        assert_eq!(usage.spent, 200_000);
        let _ = repos.transactions.delete(&exp_id).await;
        let _ = repos.transactions.delete(&inc_id).await;
        let _ = repos.budgets.delete(&budget_id).await;
        let _ = repos.accounts.delete(&acct_id).await;
    }

    #[tokio::test]
    async fn budget_usage_category_null_sums_all() {
        let Some(repos) = test_repos().await else {
            return;
        };
        let acct_id = unique_id("a");
        repos.accounts.add(&sample_account(&acct_id)).await.unwrap();
        let budget_id = unique_id("b");
        repos
            .budgets
            .add(&sample_budget(&budget_id, "Total Budget", 10_000_000, None))
            .await
            .unwrap();
        let feb15 = NaiveDate::from_ymd_opt(2026, 2, 15).unwrap();
        let id1 = unique_id("t1");
        let id2 = unique_id("t2");
        let id3 = unique_id("t3");
        repos
            .transactions
            .add(&sample_tx_with_category(
                &id1, &acct_id, "expense", 100_000, "food", feb15,
            ))
            .await
            .unwrap();
        repos
            .transactions
            .add(&sample_tx_with_category(
                &id2,
                &acct_id,
                "expense",
                50_000,
                "transport",
                feb15,
            ))
            .await
            .unwrap();
        repos
            .transactions
            .add(&sample_tx_with_category(
                &id3,
                &acct_id,
                "expense",
                200_000,
                "entertainment",
                feb15,
            ))
            .await
            .unwrap();
        let usage = repos.budgets.budget_usage(&budget_id).await.unwrap();
        assert_eq!(usage.spent, 350_000);
        for id in &[&id1, &id2, &id3] {
            let _ = repos.transactions.delete(id).await;
        }
        let _ = repos.budgets.delete(&budget_id).await;
        let _ = repos.accounts.delete(&acct_id).await;
    }

    #[tokio::test]
    async fn all_budget_usage_returns_all_active() {
        let Some(repos) = test_repos().await else {
            return;
        };
        let id1 = unique_id("b1");
        let id2 = unique_id("b2");
        let id3 = unique_id("b3");
        repos
            .budgets
            .add(&sample_budget(&id1, "Food", 1_000_000, Some("food")))
            .await
            .unwrap();
        repos
            .budgets
            .add(&sample_budget(
                &id2,
                "Transport",
                500_000,
                Some("transport"),
            ))
            .await
            .unwrap();
        repos
            .budgets
            .add(&sample_budget(
                &id3,
                "Entertainment",
                800_000,
                Some("entertainment"),
            ))
            .await
            .unwrap();
        let result = repos.budgets.all_budget_usage().await.unwrap();
        let ids: Vec<_> = result.iter().map(|r| r.id.as_str()).collect();
        assert!(ids.contains(&id1.as_str()));
        assert!(ids.contains(&id2.as_str()));
        assert!(ids.contains(&id3.as_str()));
        for id in &[&id1, &id2, &id3] {
            let _ = repos.budgets.delete(id).await;
        }
    }

    #[tokio::test]
    async fn budget_usage_not_found_returns_error() {
        let Some(repos) = test_repos().await else {
            return;
        };
        let result = repos.budgets.budget_usage("bad-budget-id").await;
        assert!(matches!(result, Err(crate::StorageError::NotFound(_))));
    }
}
