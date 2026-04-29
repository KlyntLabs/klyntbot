//! KCA Track 8 — hierarchical episodic compression.

use crate::repos::episodic_memory::EpisodicMemoryRepo;
use crate::types::EpisodicMemory;
use jiff::Timestamp;

#[derive(Debug, Clone, Copy)]
pub enum Tier {
    Raw,
    Hourly,
    Daily,
    Weekly,
}

impl Tier {
    pub fn as_str(&self) -> &'static str {
        match self {
            Tier::Raw => "raw",
            Tier::Hourly => "hourly",
            Tier::Daily => "daily",
            Tier::Weekly => "weekly",
        }
    }
}

#[async_trait::async_trait]
pub trait HierarchicalSummarizer: Send + Sync {
    async fn summarize(&self, items: &[EpisodicMemory], tier: Tier) -> common::Result<String>;
}

/// Roll up all unrolled raw episodics into hourly summaries.
/// Returns the number of hourly buckets created.
pub async fn roll_up_hourly(
    repo: &EpisodicMemoryRepo,
    summarizer: std::sync::Arc<dyn HierarchicalSummarizer>,
) -> common::Result<u32> {
    let unrolled = repo
        .list_unrolled_at_tier("raw", 1000)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
    if unrolled.is_empty() {
        return Ok(0);
    }

    // Group by hour bucket (UTC).
    let mut by_hour: std::collections::BTreeMap<i64, Vec<EpisodicMemory>> = Default::default();
    for ep in unrolled {
        let bucket = ep
            .recorded_at
            .parse::<Timestamp>()
            .map(|t| t.duration_since(Timestamp::UNIX_EPOCH).as_secs() / 3600)
            .unwrap_or(0);
        by_hour.entry(bucket).or_default().push(ep);
    }

    let mut created = 0u32;
    for (hour_bucket, items) in by_hour {
        let summary = summarizer
            .summarize(&items, Tier::Hourly)
            .await
            .unwrap_or_else(|_| format!("(failed to summarize {} items)", items.len()));

        let parent_id = uuid::Uuid::new_v4().to_string();
        let dom = items[0].domain.clone();
        let avg_importance = items.iter().map(|e| e.importance).sum::<f64>() / items.len() as f64;
        let recorded_at =
            Timestamp::from_second(hour_bucket * 3600 + 1800).unwrap_or_else(|_| Timestamp::now());

        repo.insert(&EpisodicMemory {
            id: parent_id.clone(),
            domain: dom,
            content: summary.clone(),
            summary: Some(summary),
            importance: avg_importance,
            occurred_at: recorded_at.to_string(),
            recorded_at: recorded_at.to_string(),
            stability: 1.5,
            last_accessed: None,
            access_count: 0,
            project_id: None,
            scope_type: "system".to_string(),
            scope_id: None,
            scope_repo_id: None,
            metadata: None,
            kind: None,
            tier: Tier::Hourly.as_str().to_string(),
            parent_id: None,
            child_count: items.len() as i64,
            rolled_up_at: None,
        })
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

        for it in &items {
            repo.set_parent_and_rolled_up(&it.id, &parent_id)
                .await
                .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        }
        created += 1;
    }

    Ok(created)
}

pub async fn roll_up_daily(
    repo: &EpisodicMemoryRepo,
    summarizer: std::sync::Arc<dyn HierarchicalSummarizer>,
) -> common::Result<u32> {
    let unrolled = repo
        .list_unrolled_at_tier("hourly", 500)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
    if unrolled.is_empty() {
        return Ok(0);
    }

    let mut by_day: std::collections::BTreeMap<i64, Vec<EpisodicMemory>> = Default::default();
    for ep in unrolled {
        let bucket = ep
            .recorded_at
            .parse::<Timestamp>()
            .map(|t| t.duration_since(Timestamp::UNIX_EPOCH).as_secs() / 86400)
            .unwrap_or(0);
        by_day.entry(bucket).or_default().push(ep);
    }

    let mut created = 0u32;
    for (day_bucket, items) in by_day {
        let summary = summarizer
            .summarize(&items, Tier::Daily)
            .await
            .unwrap_or_else(|_| format!("(failed to summarize {} items)", items.len()));
        let parent_id = uuid::Uuid::new_v4().to_string();
        let recorded_at =
            Timestamp::from_second(day_bucket * 86400 + 43200).unwrap_or_else(|_| Timestamp::now());

        repo.insert(&EpisodicMemory {
            id: parent_id.clone(),
            domain: items[0].domain.clone(),
            content: summary.clone(),
            summary: Some(summary),
            importance: items.iter().map(|e| e.importance).sum::<f64>() / items.len() as f64,
            occurred_at: recorded_at.to_string(),
            recorded_at: recorded_at.to_string(),
            stability: 2.0,
            last_accessed: None,
            access_count: 0,
            project_id: None,
            scope_type: "system".to_string(),
            scope_id: None,
            scope_repo_id: None,
            metadata: None,
            kind: None,
            tier: Tier::Daily.as_str().to_string(),
            parent_id: None,
            child_count: items.iter().map(|e| e.child_count).sum::<i64>(),
            rolled_up_at: None,
        })
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

        for it in &items {
            repo.set_parent_and_rolled_up(&it.id, &parent_id)
                .await
                .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        }
        created += 1;
    }
    Ok(created)
}

pub async fn roll_up_weekly(
    repo: &EpisodicMemoryRepo,
    summarizer: std::sync::Arc<dyn HierarchicalSummarizer>,
) -> common::Result<u32> {
    let unrolled = repo
        .list_unrolled_at_tier("daily", 200)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
    if unrolled.is_empty() {
        return Ok(0);
    }

    let mut by_week: std::collections::BTreeMap<i64, Vec<EpisodicMemory>> = Default::default();
    for ep in unrolled {
        let bucket = ep
            .recorded_at
            .parse::<Timestamp>()
            .map(|t| t.duration_since(Timestamp::UNIX_EPOCH).as_secs() / (86400 * 7))
            .unwrap_or(0);
        by_week.entry(bucket).or_default().push(ep);
    }

    let mut created = 0u32;
    for (week_bucket, items) in by_week {
        let summary = summarizer
            .summarize(&items, Tier::Weekly)
            .await
            .unwrap_or_else(|_| format!("(failed to summarize {} items)", items.len()));
        let parent_id = uuid::Uuid::new_v4().to_string();
        let recorded_at =
            Timestamp::from_second(week_bucket * 86400 * 7).unwrap_or_else(|_| Timestamp::now());

        repo.insert(&EpisodicMemory {
            id: parent_id.clone(),
            domain: items[0].domain.clone(),
            content: summary.clone(),
            summary: Some(summary),
            importance: items.iter().map(|e| e.importance).sum::<f64>() / items.len() as f64,
            occurred_at: recorded_at.to_string(),
            recorded_at: recorded_at.to_string(),
            stability: 3.0,
            last_accessed: None,
            access_count: 0,
            project_id: None,
            scope_type: "system".to_string(),
            scope_id: None,
            scope_repo_id: None,
            metadata: None,
            kind: None,
            tier: Tier::Weekly.as_str().to_string(),
            parent_id: None,
            child_count: items.iter().map(|e| e.child_count).sum::<i64>(),
            rolled_up_at: None,
        })
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

        for it in &items {
            repo.set_parent_and_rolled_up(&it.id, &parent_id)
                .await
                .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        }
        created += 1;
    }
    Ok(created)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::episodic_memory::EpisodicMemoryRepo;
    use storage::StoragePool;

    struct EchoSummarizer;
    #[async_trait::async_trait]
    impl HierarchicalSummarizer for EchoSummarizer {
        async fn summarize(&self, items: &[EpisodicMemory], tier: Tier) -> common::Result<String> {
            Ok(format!(
                "Summary of {} items at tier {:?}",
                items.len(),
                tier
            ))
        }
    }

    #[tokio::test]
    async fn roll_up_hourly_creates_parent_with_correct_child_count() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let migrations = crate::repos::cognitive_migrations();
        StoragePool::run_feature_migrations(pool.inner(), &migrations)
            .await
            .unwrap();
        let repo = EpisodicMemoryRepo::new(pool.inner().clone());

        // Insert 5 raw episodics in last hour.
        for i in 0..5 {
            repo.insert(&EpisodicMemory {
                id: format!("ep{i}"),
                domain: "test".into(),
                content: format!("event {i}"),
                summary: None,
                importance: 0.5,
                occurred_at: jiff::Timestamp::now().to_string(),
                recorded_at: jiff::Timestamp::now().to_string(),
                stability: 1.0,
                last_accessed: None,
                access_count: 0,
                project_id: None,
                scope_type: "system".into(),
                scope_id: None,
                scope_repo_id: None,
                metadata: None,
                kind: None,
                tier: "raw".into(),
                parent_id: None,
                child_count: 0,
                rolled_up_at: None,
            })
            .await
            .unwrap();
        }

        let summarizer = std::sync::Arc::new(EchoSummarizer);
        let n = roll_up_hourly(&repo, summarizer).await.unwrap();
        assert_eq!(n, 1, "1 hourly bucket created");

        let hourlies = repo.list_by_tier("hourly", 10).await.unwrap();
        assert_eq!(hourlies.len(), 1);
        assert_eq!(hourlies[0].child_count, 5);
        assert!(hourlies[0]
            .summary
            .as_deref()
            .map_or(false, |s| s.contains("5 items")));

        // Raw episodics should be marked rolled_up_at.
        let raw = repo.list_by_tier("raw", 10).await.unwrap();
        assert!(raw.iter().all(|e| e.rolled_up_at.is_some()));
    }
}
