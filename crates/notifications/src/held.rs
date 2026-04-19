//! Writes `held_notifications` rows and schedules a companion
//! `scheduled_fires(kind='held_release')` row via Phase 2's FireStore.

use jiff::Timestamp;
use serde_json::json;
use uuid::Uuid;

use scheduling::{FireSpec, FireStore};
use storage::repos::held_notifications::HeldNotificationsRepo;
use storage::rows::held_notification::HeldNotificationRow;

use crate::channel::{NotificationPayload, Priority};
use crate::error::Result;

#[derive(Clone)]
pub struct HeldReleaseService {
    held: HeldNotificationsRepo,
    fire_store: FireStore,
}

impl HeldReleaseService {
    pub fn new(held: HeldNotificationsRepo, fire_store: FireStore) -> Self {
        Self { held, fire_store }
    }

    pub async fn hold(
        &self,
        alarm_id: &str,
        channels: &[String],
        payload: &NotificationPayload,
        release_at: Timestamp,
    ) -> Result<String> {
        let id = format!("held_{}", Uuid::new_v4());
        let priority_str = match payload.priority {
            Priority::Normal => "normal",
            Priority::Urgent => "urgent",
        };
        let row = HeldNotificationRow {
            id: id.clone(),
            alarm_id: alarm_id.into(),
            channels: json!(channels),
            payload: json!({
                "title": payload.title,
                "body": payload.body,
                "priority": priority_str,
            }),
            release_at_ms: release_at.as_millisecond(),
            released: false,
            held_at_ms: Timestamp::now().as_millisecond(),
        };
        self.held.insert(&row).await?;

        self.fire_store
            .schedule(FireSpec {
                fire_at: release_at,
                kind: "held_release".into(),
                ref_id: Some(id.clone()),
                payload: json!({ "held_id": id }),
                dedup_prefix: Some(format!("held:{id}:")),
            })
            .await?;

        Ok(id)
    }

    pub async fn release_due(&self, now: Timestamp) -> Result<Vec<ReleaseBatch>> {
        let rows = self.held.list_pending_before(now.as_millisecond()).await?;
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let channels: Vec<String> = serde_json::from_value(r.channels).unwrap_or_default();
            out.push(ReleaseBatch {
                held_id: r.id,
                alarm_id: r.alarm_id,
                channels,
                payload: r.payload,
            });
        }
        Ok(out)
    }

    pub async fn mark_released(&self, held_id: &str) -> Result<()> {
        self.held.mark_released(held_id).await?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ReleaseBatch {
    pub held_id: String,
    pub alarm_id: String,
    pub channels: Vec<String>,
    pub payload: serde_json::Value,
}
