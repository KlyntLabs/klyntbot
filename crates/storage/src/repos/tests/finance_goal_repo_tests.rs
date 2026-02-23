//! Tests for FinanceGoalRepo.

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use crate::repos::finance_goal_repo::FinanceGoalRepo;
    use crate::rows::finance::{FinanceGoalPatch, FinanceGoalRow};
    use crate::StoragePool;

    async fn test_repo() -> Option<FinanceGoalRepo> {
        let dir = tempfile::tempdir().ok()?;
        let pool = StoragePool::connect(dir.path()).await.ok()?;
        let _ = dir.keep(); // prevent cleanup; acceptable in test context
        Some(FinanceGoalRepo::new(pool.inner().clone()))
    }

    fn unique_id(prefix: &str) -> String {
        format!("{prefix}-{}", uuid::Uuid::new_v4().as_simple())
    }

    fn sample_goal(id: &str, goal_type: &str, target: i64) -> FinanceGoalRow {
        let now = Utc::now();
        FinanceGoalRow {
            id: id.to_string(),
            name: format!("Goal {id}"),
            goal_type: goal_type.to_string(),
            target_amount: target,
            current_amount: 0,
            currency: "VND".to_string(),
            status: "active".to_string(),
            deadline: None,
            monthly_contribution: None,
            expected_return_rate: None,
            inflation_rate: None,
            notes: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn goal_add_and_get() {
        let Some(repo) = test_repo().await else {
            return;
        };
        let id = unique_id("g");
        let row = sample_goal(&id, "savings", 50_000_000);
        let inserted = repo.add(&row).await.unwrap();
        assert_eq!(inserted.status, "active");
        assert_eq!(inserted.target_amount, 50_000_000);
        let fetched = repo.get(&id).await.unwrap().unwrap();
        assert_eq!(fetched.id, id);
        let _ = repo.delete(&id).await;
    }

    #[tokio::test]
    async fn goal_update_current_amount() {
        let Some(repo) = test_repo().await else {
            return;
        };
        let id = unique_id("g");
        repo.add(&sample_goal(&id, "savings", 50_000_000))
            .await
            .unwrap();
        let patch = FinanceGoalPatch {
            id: id.clone(),
            current_amount: Some(10_000_000),
            ..Default::default()
        };
        let updated = repo.update(&patch).await.unwrap();
        assert_eq!(updated.current_amount, 10_000_000);
        let _ = repo.delete(&id).await;
    }

    #[tokio::test]
    async fn goal_update_progress_method() {
        let Some(repo) = test_repo().await else {
            return;
        };
        let id = unique_id("g");
        repo.add(&sample_goal(&id, "savings", 50_000_000))
            .await
            .unwrap();
        let updated = repo.update_progress(&id, 25_000_000).await.unwrap();
        assert_eq!(updated.current_amount, 25_000_000);
        let _ = repo.delete(&id).await;
    }

    #[tokio::test]
    async fn goal_update_status() {
        let Some(repo) = test_repo().await else {
            return;
        };
        let id = unique_id("g");
        repo.add(&sample_goal(&id, "savings", 50_000_000))
            .await
            .unwrap();
        let patch = FinanceGoalPatch {
            id: id.clone(),
            status: Some("achieved".to_string()),
            ..Default::default()
        };
        let updated = repo.update(&patch).await.unwrap();
        assert_eq!(updated.status, "achieved");
        let _ = repo.delete(&id).await;
    }

    #[tokio::test]
    async fn goal_list_active() {
        let Some(repo) = test_repo().await else {
            return;
        };
        let id1 = unique_id("g1");
        let id2 = unique_id("g2");
        let id3 = unique_id("g3");
        let id4 = unique_id("g4");
        repo.add(&sample_goal(&id1, "savings", 10_000_000))
            .await
            .unwrap();
        repo.add(&sample_goal(&id2, "purchase", 20_000_000))
            .await
            .unwrap();
        let mut achieved = sample_goal(&id3, "savings", 5_000_000);
        achieved.status = "achieved".to_string();
        repo.add(&achieved).await.unwrap();
        let mut abandoned = sample_goal(&id4, "custom", 1_000_000);
        abandoned.status = "abandoned".to_string();
        repo.add(&abandoned).await.unwrap();
        let result = repo.list_active().await.unwrap();
        let ids: Vec<_> = result.iter().map(|r| r.id.as_str()).collect();
        assert!(ids.contains(&id1.as_str()));
        assert!(ids.contains(&id2.as_str()));
        assert!(!ids.contains(&id3.as_str()));
        assert!(!ids.contains(&id4.as_str()));
        assert!(result.iter().all(|r| r.status == "active"));
        for id in &[&id1, &id2, &id3, &id4] {
            let _ = repo.delete(id).await;
        }
    }

    #[tokio::test]
    async fn goal_delete() {
        let Some(repo) = test_repo().await else {
            return;
        };
        let id = unique_id("g");
        repo.add(&sample_goal(&id, "savings", 10_000_000))
            .await
            .unwrap();
        let deleted = repo.delete(&id).await.unwrap();
        assert!(deleted);
        let fetched = repo.get(&id).await.unwrap();
        assert!(fetched.is_none());
    }

    #[tokio::test]
    async fn goal_get_not_found() {
        let Some(repo) = test_repo().await else {
            return;
        };
        let result = repo.get("nonexistent-goal-id").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn goal_update_not_found() {
        let Some(repo) = test_repo().await else {
            return;
        };
        let patch = FinanceGoalPatch {
            id: "bad-id".to_string(),
            current_amount: Some(1),
            ..Default::default()
        };
        let result = repo.update(&patch).await;
        assert!(matches!(result, Err(crate::StorageError::NotFound(_))));
    }
}
