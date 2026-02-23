//! Tests for FinanceAccountRepo.
//!
//! Tests that require a live database use `test_account_repo()` which gracefully
//! returns `None` (and skips the test) when no PostgreSQL is available.

#[cfg(test)]
mod tests {
    use crate::repos::finance_account_repo::FinanceAccountRepo;
    use crate::rows::finance::{FinanceAccountPatch, FinanceAccountRow};
    use crate::StoragePool;
    use chrono::Utc;

    async fn test_account_repo() -> Option<FinanceAccountRepo> {
        let dir = tempfile::tempdir().ok()?;
        let pool = StoragePool::connect(dir.path()).await.ok()?;
        let _ = dir.keep(); // prevent cleanup; acceptable in test context
        Some(FinanceAccountRepo::new(pool.inner().clone()))
    }

    fn sample_account(id: &str, name: &str, account_type: &str) -> FinanceAccountRow {
        let now = Utc::now();
        FinanceAccountRow {
            id: id.to_string(),
            name: name.to_string(),
            account_type: account_type.to_string(),
            currency: "VND".to_string(),
            balance: 0,
            institution: None,
            notes: None,
            is_archived: false,
            created_at: now,
            updated_at: now,
        }
    }

    fn unique_id(prefix: &str) -> String {
        format!("{prefix}-{}", uuid::Uuid::new_v4().as_simple())
    }

    // ---------------------------------------------------------------------------
    // CRUD Tests
    // ---------------------------------------------------------------------------

    /// AC 1.1: account_add creates row with correct fields; account_get retrieves it.
    #[tokio::test]
    async fn account_add_and_get() {
        let Some(repo) = test_account_repo().await else {
            return;
        };
        let id = unique_id("acct");
        let row = sample_account(&id, "My Bank", "bank");

        let inserted = repo.add(&row).await.unwrap();
        assert_eq!(inserted.name, "My Bank");
        assert_eq!(inserted.account_type, "bank");
        assert!(!inserted.is_archived);

        let fetched = repo.get(&id).await.unwrap().unwrap();
        assert_eq!(fetched.id, id);
        assert_eq!(fetched.name, "My Bank");

        let _ = repo.delete(&id).await;
    }

    /// AC 1.3 edge: get with unknown id returns None (not an error).
    #[tokio::test]
    async fn account_get_not_found_returns_none() {
        let Some(repo) = test_account_repo().await else {
            return;
        };
        let result = repo.get("nonexistent-account-id").await.unwrap();
        assert!(result.is_none());
    }

    /// AC 1.3: account_update changes name and balance, refreshes updated_at.
    #[tokio::test]
    async fn account_update_name_and_balance() {
        let Some(repo) = test_account_repo().await else {
            return;
        };
        let id = unique_id("upd");
        let row = sample_account(&id, "Old Name", "bank");
        let original = repo.add(&row).await.unwrap();

        let patch = FinanceAccountPatch {
            id: id.clone(),
            name: Some("New Name".to_string()),
            balance: Some(500_000),
            ..Default::default()
        };
        let updated = repo.update(&patch).await.unwrap();

        assert_eq!(updated.name, "New Name");
        assert_eq!(updated.balance, 500_000);
        assert!(updated.updated_at >= original.updated_at);

        let _ = repo.delete(&id).await;
    }

    /// AC 1.3 edge: update on missing id returns StorageError::NotFound.
    #[tokio::test]
    async fn account_update_not_found_returns_error() {
        let Some(repo) = test_account_repo().await else {
            return;
        };
        let patch = FinanceAccountPatch {
            id: "nonexistent-id".to_string(),
            name: Some("x".to_string()),
            ..Default::default()
        };
        let result = repo.update(&patch).await;
        assert!(matches!(result, Err(crate::StorageError::NotFound(_))));
    }

    /// AC 1.4: account_delete removes the row.
    #[tokio::test]
    async fn account_delete_existing() {
        let Some(repo) = test_account_repo().await else {
            return;
        };
        let id = unique_id("del");
        let row = sample_account(&id, "To Delete", "cash");
        repo.add(&row).await.unwrap();

        let deleted = repo.delete(&id).await.unwrap();
        assert!(deleted);

        let fetched = repo.get(&id).await.unwrap();
        assert!(fetched.is_none());
    }

    /// AC 1.4 edge: delete on unknown id returns false (not an error).
    #[tokio::test]
    async fn account_delete_not_found_returns_false() {
        let Some(repo) = test_account_repo().await else {
            return;
        };
        let result = repo.delete("nonexistent-account").await.unwrap();
        assert!(!result);
    }

    // ---------------------------------------------------------------------------
    // Filtering & Listing
    // ---------------------------------------------------------------------------

    /// AC 1.2 default behavior: list excludes archived accounts.
    #[tokio::test]
    async fn account_list_excludes_archived_by_default() {
        let Some(repo) = test_account_repo().await else {
            return;
        };
        let id1 = unique_id("lst1");
        let id2 = unique_id("lst2");
        let id3 = unique_id("lst3");

        repo.add(&sample_account(&id1, "Active 1", "bank"))
            .await
            .unwrap();
        repo.add(&sample_account(&id2, "Active 2", "cash"))
            .await
            .unwrap();
        let mut archived_row = sample_account(&id3, "Archived", "bank");
        archived_row.is_archived = true;
        repo.add(&archived_row).await.unwrap();

        let rows = repo.list(false).await.unwrap();
        let ids_in_result: Vec<_> = rows.iter().map(|r| r.id.as_str()).collect();
        assert!(ids_in_result.contains(&id1.as_str()));
        assert!(ids_in_result.contains(&id2.as_str()));
        assert!(!ids_in_result.contains(&id3.as_str()));
        assert!(rows.iter().all(|r| !r.is_archived));

        let _ = repo.delete(&id1).await;
        let _ = repo.delete(&id2).await;
        let _ = repo.delete(&id3).await;
    }

    /// AC 1.2: list with include_archived=true returns archived accounts too.
    #[tokio::test]
    async fn account_list_includes_archived_when_requested() {
        let Some(repo) = test_account_repo().await else {
            return;
        };
        let id1 = unique_id("inc1");
        let id2 = unique_id("inc2");
        let id3 = unique_id("inc3");

        repo.add(&sample_account(&id1, "A1", "bank")).await.unwrap();
        repo.add(&sample_account(&id2, "A2", "cash")).await.unwrap();
        let mut arch = sample_account(&id3, "Archived", "bank");
        arch.is_archived = true;
        repo.add(&arch).await.unwrap();

        let all = repo.list(true).await.unwrap();
        let ids: Vec<_> = all.iter().map(|r| r.id.as_str()).collect();
        assert!(ids.contains(&id1.as_str()));
        assert!(ids.contains(&id2.as_str()));
        assert!(ids.contains(&id3.as_str()));

        let _ = repo.delete(&id1).await;
        let _ = repo.delete(&id2).await;
        let _ = repo.delete(&id3).await;
    }

    /// AC 1.2 currency filter: list_by_currency returns only matching accounts.
    #[tokio::test]
    async fn account_list_by_currency_filter() {
        let Some(repo) = test_account_repo().await else {
            return;
        };
        let id_vnd1 = unique_id("cv1");
        let id_vnd2 = unique_id("cv2");
        let id_usd = unique_id("cu1");

        let mut vnd1 = sample_account(&id_vnd1, "VND 1", "bank");
        vnd1.currency = "VND".to_string();
        let mut vnd2 = sample_account(&id_vnd2, "VND 2", "cash");
        vnd2.currency = "VND".to_string();
        let mut usd = sample_account(&id_usd, "USD 1", "bank");
        usd.currency = "USD".to_string();

        repo.add(&vnd1).await.unwrap();
        repo.add(&vnd2).await.unwrap();
        repo.add(&usd).await.unwrap();

        let result = repo.list_by_currency("VND").await.unwrap();
        let ids: Vec<_> = result.iter().map(|r| r.id.as_str()).collect();
        assert!(ids.contains(&id_vnd1.as_str()));
        assert!(ids.contains(&id_vnd2.as_str()));
        assert!(!ids.contains(&id_usd.as_str()));
        assert!(result.iter().all(|r| r.currency == "VND"));

        let _ = repo.delete(&id_vnd1).await;
        let _ = repo.delete(&id_vnd2).await;
        let _ = repo.delete(&id_usd).await;
    }

    // ---------------------------------------------------------------------------
    // Aggregation
    // ---------------------------------------------------------------------------

    /// Used by net_worth (AC 5.4): total_balance_by_currency groups active accounts.
    #[tokio::test]
    async fn account_total_balance_by_currency() {
        let Some(repo) = test_account_repo().await else {
            return;
        };
        let id1 = unique_id("tb1");
        let id2 = unique_id("tb2");
        let id3 = unique_id("tb3");

        let mut a1 = sample_account(&id1, "VND A", "bank");
        a1.balance = 100_000;
        let mut a2 = sample_account(&id2, "VND B", "cash");
        a2.balance = 200_000;
        let mut a3 = sample_account(&id3, "USD A", "bank");
        a3.currency = "USD".to_string();
        a3.balance = 50;

        repo.add(&a1).await.unwrap();
        repo.add(&a2).await.unwrap();
        repo.add(&a3).await.unwrap();

        let totals = repo.total_balance_by_currency().await.unwrap();
        let vnd_total = totals.iter().find(|(c, _)| c == "VND").map(|(_, t)| *t);
        let usd_total = totals.iter().find(|(c, _)| c == "USD").map(|(_, t)| *t);

        assert!(vnd_total.unwrap_or(0) >= 300_000);
        assert!(usd_total.unwrap_or(0) >= 50);

        let _ = repo.delete(&id1).await;
        let _ = repo.delete(&id2).await;
        let _ = repo.delete(&id3).await;
    }

    /// Net worth accuracy: archived accounts excluded from totals.
    #[tokio::test]
    async fn account_total_balance_excludes_archived() {
        let Some(repo) = test_account_repo().await else {
            return;
        };
        let id_active = unique_id("tea");
        let id_arch = unique_id("teb");

        let mut active = sample_account(&id_active, "Active VND", "bank");
        active.balance = 1_000_000;
        active.currency = "VND".to_string();

        let mut archived = sample_account(&id_arch, "Archived VND", "bank");
        archived.balance = 9_999_999;
        archived.currency = "VND".to_string();
        archived.is_archived = true;

        repo.add(&active).await.unwrap();
        repo.add(&archived).await.unwrap();

        let totals = repo.total_balance_by_currency().await.unwrap();
        // Archived account (9_999_999) should NOT be included.
        // If it were, total would be >= 10_999_999 even with just these two rows.
        let vnd_total = totals.iter().find(|(c, _)| c == "VND").map(|(_, t)| *t);
        if let Some(total) = vnd_total {
            // The archived contribution (9_999_999) must not be present.
            assert!(
                total < 10_999_999,
                "archived balance leaked into total: {total}"
            );
        }

        let _ = repo.delete(&id_active).await;
        let _ = repo.delete(&id_arch).await;
    }

    /// AC 2.1 balance update: adjust_balance adds delta to existing balance.
    #[tokio::test]
    async fn account_adjust_balance_adds_delta() {
        let Some(repo) = test_account_repo().await else {
            return;
        };
        let id = unique_id("adj");
        let mut row = sample_account(&id, "Adjust Me", "bank");
        row.balance = 1_000;
        repo.add(&row).await.unwrap();

        let updated = repo.adjust_balance(&id, 500).await.unwrap();
        assert_eq!(updated.balance, 1_500);

        let _ = repo.delete(&id).await;
    }

    /// AC 2.1 overdraft: adjust_balance allows negative balance.
    #[tokio::test]
    async fn account_adjust_balance_allows_negative() {
        let Some(repo) = test_account_repo().await else {
            return;
        };
        let id = unique_id("neg");
        let mut row = sample_account(&id, "Overdraft", "bank");
        row.balance = 100;
        repo.add(&row).await.unwrap();

        let updated = repo.adjust_balance(&id, -500).await.unwrap();
        assert_eq!(updated.balance, -400);

        let _ = repo.delete(&id).await;
    }
}
