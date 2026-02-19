//! Tests for FinanceLiabilityRepo.

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use crate::repos::finance_liability_repo::FinanceLiabilityRepo;
    use crate::rows::finance::{FinanceLiabilityPatch, FinanceLiabilityRow};
    use crate::StoragePool;

    async fn test_repo() -> Option<FinanceLiabilityRepo> {
        let url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://localhost/klyntbot_test".to_string());
        match StoragePool::connect(&url).await {
            Ok(pool) => Some(FinanceLiabilityRepo::new(pool.inner().clone())),
            Err(_) => None,
        }
    }

    fn unique_id(prefix: &str) -> String {
        format!("{prefix}-{}", uuid::Uuid::new_v4().as_simple())
    }

    fn sample_liability(
        id: &str,
        liability_type: &str,
        principal: i64,
        remaining: i64,
    ) -> FinanceLiabilityRow {
        let now = Utc::now();
        FinanceLiabilityRow {
            id: id.to_string(),
            name: format!("Liability {id}"),
            liability_type: liability_type.to_string(),
            principal,
            remaining,
            currency: "VND".to_string(),
            interest_rate: None,
            monthly_payment: None,
            due_date: None,
            notes: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn liability_add_and_get() {
        let Some(repo) = test_repo().await else {
            return;
        };
        let id = unique_id("l");
        let row = sample_liability(&id, "mortgage", 800_000_000, 750_000_000);
        let inserted = repo.add(&row).await.unwrap();
        assert_eq!(inserted.liability_type, "mortgage");
        assert_eq!(inserted.principal, 800_000_000);
        let fetched = repo.get(&id).await.unwrap().unwrap();
        assert_eq!(fetched.id, id);
        let _ = repo.delete(&id).await;
    }

    #[tokio::test]
    async fn liability_list_all() {
        let Some(repo) = test_repo().await else {
            return;
        };
        let id1 = unique_id("l1");
        let id2 = unique_id("l2");
        let id3 = unique_id("l3");
        repo.add(&sample_liability(
            &id1,
            "mortgage",
            800_000_000,
            750_000_000,
        ))
        .await
        .unwrap();
        repo.add(&sample_liability(
            &id2,
            "credit_card",
            10_000_000,
            5_000_000,
        ))
        .await
        .unwrap();
        repo.add(&sample_liability(
            &id3,
            "personal_loan",
            20_000_000,
            15_000_000,
        ))
        .await
        .unwrap();
        let result = repo.list_all().await.unwrap();
        let ids: Vec<_> = result.iter().map(|r| r.id.as_str()).collect();
        assert!(ids.contains(&id1.as_str()));
        assert!(ids.contains(&id2.as_str()));
        assert!(ids.contains(&id3.as_str()));
        for id in &[&id1, &id2, &id3] {
            let _ = repo.delete(id).await;
        }
    }

    #[tokio::test]
    async fn liability_update_remaining() {
        let Some(repo) = test_repo().await else {
            return;
        };
        let id = unique_id("l");
        repo.add(&sample_liability(&id, "mortgage", 800_000_000, 750_000_000))
            .await
            .unwrap();
        let patch = FinanceLiabilityPatch {
            id: id.clone(),
            remaining: Some(740_000_000),
            ..Default::default()
        };
        let updated = repo.update(&patch).await.unwrap();
        assert_eq!(updated.remaining, 740_000_000);
        let _ = repo.delete(&id).await;
    }

    #[tokio::test]
    async fn liability_delete() {
        let Some(repo) = test_repo().await else {
            return;
        };
        let id = unique_id("l");
        repo.add(&sample_liability(&id, "credit_card", 5_000_000, 3_000_000))
            .await
            .unwrap();
        let deleted = repo.delete(&id).await.unwrap();
        assert!(deleted);
        let fetched = repo.get(&id).await.unwrap();
        assert!(fetched.is_none());
    }

    #[tokio::test]
    async fn liability_total_remaining_by_currency() {
        let Some(repo) = test_repo().await else {
            return;
        };
        let id1 = unique_id("l1");
        let id2 = unique_id("l2");
        let id3 = unique_id("l3");
        repo.add(&sample_liability(
            &id1,
            "mortgage",
            800_000_000,
            800_000_000,
        ))
        .await
        .unwrap();
        repo.add(&sample_liability(
            &id2,
            "credit_card",
            10_000_000,
            5_000_000,
        ))
        .await
        .unwrap();
        let mut usd_liab = sample_liability(&id3, "personal_loan", 2_000, 1_000);
        usd_liab.currency = "USD".to_string();
        repo.add(&usd_liab).await.unwrap();
        let totals = repo.total_remaining_by_currency().await.unwrap();
        let vnd_total = totals.iter().find(|(c, _)| c == "VND").map(|(_, t)| *t);
        let usd_total = totals.iter().find(|(c, _)| c == "USD").map(|(_, t)| *t);
        assert!(vnd_total.unwrap_or(0) >= 805_000_000);
        assert!(usd_total.unwrap_or(0) >= 1_000);
        for id in &[&id1, &id2, &id3] {
            let _ = repo.delete(id).await;
        }
    }

    #[tokio::test]
    async fn liability_total_remaining_empty() {
        let Some(repo) = test_repo().await else {
            return;
        };
        // Just verify it doesn't error; may return rows if other tests left data
        let result = repo.total_remaining_by_currency().await;
        assert!(result.is_ok());
    }
}
