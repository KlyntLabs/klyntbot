//! High-level service over `ScheduledFiresRepo`.
//!
//! Owns UUID generation, jiff::Timestamp ↔ epoch-ms translation, and exposes
//! the two-phase commit protocol as jiff-native methods. Downstream code sees
//! only `jiff::Timestamp`; epoch-ms lives inside the repo.

use jiff::Timestamp;
use serde_json::Value;
use storage::repos::scheduled_fires::ScheduledFiresRepo;
use storage::rows::scheduled_fire::ScheduledFireRow;
use uuid::Uuid;

use crate::error::SchedulerError;

#[derive(Debug, Clone)]
pub struct FireSpec {
    pub fire_at: Timestamp,
    pub kind: String,
    pub ref_id: Option<String>,
    pub payload: Value,
    pub dedup_prefix: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FireStore {
    repo: ScheduledFiresRepo,
}

impl FireStore {
    pub fn new(repo: ScheduledFiresRepo) -> Self {
        Self { repo }
    }

    pub async fn schedule(&self, spec: FireSpec) -> Result<String, SchedulerError> {
        let id = format!("fire_{}", Uuid::new_v4().simple());
        let now_ms = Timestamp::now().as_millisecond();
        let row = ScheduledFireRow {
            id: id.clone(),
            fire_at_ms: spec.fire_at.as_millisecond(),
            kind: spec.kind,
            ref_id: spec.ref_id,
            payload: spec.payload,
            dedup_prefix: spec.dedup_prefix,
            fired: false,
            firing_started_at_ms: None,
            fired_at_ms: None,
            created_at_ms: now_ms,
        };
        self.repo.insert(&row).await?;
        Ok(id)
    }

    pub async fn next_pending_fire_at(&self) -> Result<Option<Timestamp>, SchedulerError> {
        let ms = self.repo.next_pending_fire_at_ms().await?;
        Ok(ms.and_then(|m| Timestamp::from_millisecond(m).ok()))
    }

    pub async fn list_due(&self, now: Timestamp) -> Result<Vec<ScheduledFireRow>, SchedulerError> {
        Ok(self
            .repo
            .list_pending_up_to_ms(now.as_millisecond())
            .await?)
    }

    pub async fn begin_firing(&self, id: &str, now: Timestamp) -> Result<bool, SchedulerError> {
        Ok(self.repo.begin_firing(id, now.as_millisecond()).await?)
    }

    pub async fn mark_fired(&self, id: &str, now: Timestamp) -> Result<(), SchedulerError> {
        self.repo.mark_fired(id, now.as_millisecond()).await?;
        Ok(())
    }

    pub async fn recover_in_flight(&self) -> Result<Vec<ScheduledFireRow>, SchedulerError> {
        Ok(self.repo.list_in_flight().await?)
    }

    pub async fn cancel_by_prefix(&self, prefix: &str) -> Result<u64, SchedulerError> {
        Ok(self.repo.cancel_by_prefix(prefix).await?)
    }

    pub async fn cancel_by_kind_ref(
        &self,
        kind: &str,
        ref_id: &str,
    ) -> Result<u64, SchedulerError> {
        Ok(self.repo.cancel_by_kind_ref(kind, ref_id).await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::Timestamp;
    use storage::pool::StoragePool;

    async fn setup() -> FireStore {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        StoragePool::run_feature_migrations(
            pool.inner(),
            &[tools_core::FeatureMigration {
                feature_name: "scheduling".into(),
                version: 1,
                description: "scheduled_fires".into(),
                sql: include_str!("../../migrations/001_scheduled_fires.sql").into(),
            }],
        )
        .await
        .unwrap();
        FireStore::new(storage::repos::scheduled_fires::ScheduledFiresRepo::new(
            pool.inner().clone(),
        ))
    }

    #[tokio::test]
    async fn schedule_inserts_a_pending_row() {
        let store = setup().await;
        let t = Timestamp::from_millisecond(1_800_000_000_000).unwrap();
        let id = store
            .schedule(FireSpec {
                fire_at: t,
                kind: "task_alarm".into(),
                ref_id: Some("task_1".into()),
                payload: serde_json::json!({ "msg": "hi" }),
                dedup_prefix: Some("task:1:".into()),
            })
            .await
            .unwrap();
        let next = store.next_pending_fire_at().await.unwrap();
        assert_eq!(next, Some(t));
        assert!(!id.is_empty());
    }

    #[tokio::test]
    async fn cancel_by_prefix_removes_only_matching() {
        let store = setup().await;
        store
            .schedule(FireSpec {
                fire_at: Timestamp::from_millisecond(1000).unwrap(),
                kind: "task_alarm".into(),
                ref_id: None,
                payload: serde_json::json!({}),
                dedup_prefix: Some("task:1:".into()),
            })
            .await
            .unwrap();
        store
            .schedule(FireSpec {
                fire_at: Timestamp::from_millisecond(2000).unwrap(),
                kind: "task_alarm".into(),
                ref_id: None,
                payload: serde_json::json!({}),
                dedup_prefix: Some("task:2:".into()),
            })
            .await
            .unwrap();
        assert_eq!(store.cancel_by_prefix("task:1:").await.unwrap(), 1);
    }
}
