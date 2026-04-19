//! Subscribes to `DomainEvent::AlarmFired` and dispatches via channel registry.

use std::sync::Arc;

use jiff::Timestamp;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use bus::{DomainEvent, DomainEventBus};
use storage::repos::held_notifications::HeldNotificationsRepo;
use storage::repos::notification_log::NotificationLogRepo;

use crate::channel::{ChannelRegistry, NotificationPayload, Priority};
use crate::error::Result;
use crate::held::HeldReleaseService;
use crate::quiet_hours::QuietHoursPolicy;
use crate::retry::RetryPolicy;

pub struct NotificationDispatcher {
    bus: Arc<DomainEventBus>,
    channels: ChannelRegistry,
    default_channels: Vec<String>,
    quiet_hours: Option<QuietHoursPolicy>,
    log_repo: NotificationLogRepo,
    held_repo: HeldNotificationsRepo,
    held_release: HeldReleaseService,
    retry: RetryPolicy,
}

pub struct NotificationDispatcherHandle {
    pub join: JoinHandle<()>,
    pub shutdown: CancellationToken,
}

impl NotificationDispatcher {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        bus: Arc<DomainEventBus>,
        channels: ChannelRegistry,
        default_channels: Vec<String>,
        quiet_hours: Option<QuietHoursPolicy>,
        log_repo: NotificationLogRepo,
        held_repo: HeldNotificationsRepo,
        held_release: HeldReleaseService,
        retry: RetryPolicy,
    ) -> Self {
        Self {
            bus,
            channels,
            default_channels,
            quiet_hours,
            log_repo,
            held_repo,
            held_release,
            retry,
        }
    }

    /// Spawn the event-loop task. Returns the handle for graceful shutdown.
    pub fn start(self) -> NotificationDispatcherHandle {
        let shutdown = CancellationToken::new();
        let token = shutdown.clone();
        let mut rx = self.bus.subscribe();
        let svc = Arc::new(self);

        let join = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = token.cancelled() => {
                        info!("notification dispatcher shutting down");
                        break;
                    }
                    ev = rx.recv() => {
                        match ev {
                            Ok(DomainEvent::AlarmFired {
                                fire_id,
                                kind,
                                ref_id: _,
                                payload_json,
                                fired_at_ms: _,
                            }) => {
                                let result = if kind == "held_release" {
                                    svc.handle_held_release(&payload_json).await
                                } else {
                                    svc.handle_alarm_fired(&fire_id, &kind, &payload_json).await
                                };
                                if let Err(e) = result {
                                    warn!("dispatch failure for {fire_id}: {e}");
                                }
                            }
                            Ok(DomainEvent::HeldNotificationReleased { .. }) => {
                                // observability hook — release dispatch is driven by AlarmFired(kind="held_release")
                            }
                            Ok(_) => {}
                            Err(e) => {
                                warn!("bus recv error: {e}");
                                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                            }
                        }
                    }
                }
            }
        });

        NotificationDispatcherHandle { join, shutdown }
    }

    async fn handle_alarm_fired(
        &self,
        fire_id: &str,
        kind: &str,
        payload_json: &str,
    ) -> Result<()> {
        let payload = parse_payload(fire_id, payload_json);
        let channels = self.resolve_channels(&payload);

        // Quiet hours gate
        let now = Timestamp::now();
        if let Some(qh) = &self.quiet_hours {
            if qh.enabled()
                && qh.is_in_quiet_hours(now)?
                && !(payload.priority == Priority::Urgent && qh.override_for_urgent())
            {
                let release_at = qh.next_window_end(now)?;
                self.held_release
                    .hold(fire_id, &channels, &payload, release_at)
                    .await?;
                return Ok(());
            }
        }

        for channel_name in channels {
            self.dispatch_one(&channel_name, &payload).await;
        }
        debug!("dispatched alarm {fire_id} kind={kind}");
        Ok(())
    }

    async fn handle_held_release(&self, payload_json: &str) -> Result<()> {
        let v: serde_json::Value =
            serde_json::from_str(payload_json).unwrap_or(serde_json::Value::Null);
        let held_id = v
            .get("held_id")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        if held_id.is_empty() {
            return Ok(());
        }

        // Fetch all pending rows (unconstrained by release_at_ms) so we can release
        // a specific held_id regardless of when it was originally scheduled to release.
        // The scheduler firing the alarm is the authoritative release signal.
        let all_pending = self.held_repo.list_pending_before(i64::MAX).await?;
        let Some(row) = all_pending.into_iter().find(|r| r.id == held_id) else {
            debug!("held_release fired for unknown or already-released held_id={held_id}");
            return Ok(());
        };

        let channels: Vec<String> = match serde_json::from_value(row.channels) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(held_id = %held_id, error = %e, "malformed channels JSON; skipping release");
                return Ok(());
            }
        };
        let payload = NotificationPayload {
            alarm_id: row.alarm_id.clone(),
            title: row
                .payload
                .get("title")
                .and_then(|x| x.as_str())
                .unwrap_or("Reminder")
                .to_string(),
            body: row
                .payload
                .get("body")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
            priority: match row.payload.get("priority").and_then(|x| x.as_str()) {
                Some("urgent") => Priority::Urgent,
                _ => Priority::Normal,
            },
        };
        for ch in &channels {
            self.dispatch_one(ch, &payload).await;
        }
        self.held_release.mark_released(&held_id).await?;
        self.bus.publish(DomainEvent::HeldNotificationReleased {
            held_id,
            alarm_id: row.alarm_id,
            channels,
        });
        Ok(())
    }

    async fn dispatch_one(&self, channel_name: &str, payload: &NotificationPayload) {
        let now_ms = Timestamp::now().as_millisecond();
        let inserted = match self
            .log_repo
            .try_insert(&payload.alarm_id, channel_name, now_ms)
            .await
        {
            Ok(v) => v,
            Err(e) => {
                warn!("log insert failed: {e}");
                return;
            }
        };
        if !inserted {
            debug!(
                "duplicate suppressed alarm={} channel={}",
                payload.alarm_id, channel_name
            );
            return;
        }

        let Some(ch) = self.channels.get(channel_name) else {
            warn!("unknown channel {channel_name}");
            return;
        };

        let mut attempt = 0u32;
        loop {
            attempt += 1;
            match ch.deliver(payload).await {
                Ok(()) => {
                    let _ = self
                        .log_repo
                        .record_ack(
                            &payload.alarm_id,
                            channel_name,
                            Timestamp::now().as_millisecond(),
                        )
                        .await;
                    return;
                }
                Err(e) if attempt < self.retry.max_attempts => {
                    let delay = self.retry.delay_for(attempt + 1);
                    warn!(
                        "delivery attempt {attempt} failed for {}/{}: {e}; retrying in {:?}",
                        payload.alarm_id, channel_name, delay
                    );
                    tokio::time::sleep(delay).await;
                }
                Err(e) => {
                    let msg = e.to_string();
                    warn!(
                        "delivery permanently failed {}/{}: {msg}",
                        payload.alarm_id, channel_name
                    );
                    let _ = self
                        .log_repo
                        .record_error(&payload.alarm_id, channel_name, &msg)
                        .await;
                    self.bus.publish(DomainEvent::NotificationDeliveryFailed {
                        alarm_id: payload.alarm_id.clone(),
                        channel: channel_name.to_string(),
                        error: msg,
                        attempts: attempt,
                    });
                    return;
                }
            }
        }
    }

    fn resolve_channels(&self, _payload: &NotificationPayload) -> Vec<String> {
        self.default_channels.clone()
    }
}

fn parse_payload(alarm_id: &str, payload_json: &str) -> NotificationPayload {
    let v: serde_json::Value = match serde_json::from_str(payload_json) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(alarm_id = %alarm_id, error = %e, "malformed alarm payload JSON; using defaults");
            serde_json::Value::Null
        }
    };
    let title = v
        .get("title")
        .and_then(|x| x.as_str())
        .unwrap_or("Reminder")
        .to_string();
    let body = v
        .get("body")
        .or_else(|| v.get("message"))
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let priority = match v.get("priority").and_then(|x| x.as_str()) {
        Some("urgent") => Priority::Urgent,
        _ => Priority::Normal,
    };
    NotificationPayload {
        alarm_id: alarm_id.into(),
        title,
        body,
        priority,
    }
}
