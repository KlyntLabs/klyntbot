//! BucketAggregator — accumulates ActivityTicks into 5-minute windows,
//! then persists each completed bucket to activity_buckets.

use std::collections::HashMap;

use chrono::Utc;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use crate::repos::ProductivityRepos;
use crate::types::{ActivityBucket, ActivityTick, CategoryType, BUCKET_DURATION_SECS};

/// Align a timestamp down to the nearest 5-minute boundary.
fn bucket_start_for(ts: &chrono::DateTime<Utc>) -> chrono::DateTime<Utc> {
    let secs = ts.timestamp();
    let aligned = secs - (secs % BUCKET_DURATION_SECS);
    chrono::DateTime::from_timestamp(aligned, 0).unwrap_or(*ts)
}

/// Find the key with the highest value in a HashMap.
pub(crate) fn dominant_key(map: &HashMap<String, i64>) -> Option<String> {
    map.iter()
        .max_by_key(|(_, v)| *v)
        .map(|(k, _)| k.clone())
}

struct PendingBucket {
    bucket_start: chrono::DateTime<Utc>,
    app_counts: HashMap<String, i64>,
    site_counts: HashMap<String, i64>,
    category_counts: HashMap<String, i64>,
    productive_secs: i64,
    neutral_secs: i64,
    distracting_secs: i64,
    idle_secs: i64,
    context_switches: i64,
    tick_count: i64,
    tick_interval_secs: i64,
}

impl PendingBucket {
    fn new(bucket_start: chrono::DateTime<Utc>, tick_interval_secs: i64) -> Self {
        Self {
            bucket_start,
            app_counts: HashMap::new(),
            site_counts: HashMap::new(),
            category_counts: HashMap::new(),
            productive_secs: 0,
            neutral_secs: 0,
            distracting_secs: 0,
            idle_secs: 0,
            context_switches: 0,
            tick_count: 0,
            tick_interval_secs,
        }
    }

    fn add_tick(&mut self, tick: &ActivityTick) {
        self.tick_count += 1;
        let secs = self.tick_interval_secs;

        if tick.is_idle {
            self.idle_secs += secs;
            return;
        }

        *self.app_counts.entry(tick.app_name.clone()).or_default() += secs;
        if let Some(ref site) = tick.site_name {
            *self.site_counts.entry(site.clone()).or_default() += secs;
        }

        match tick.category_type {
            Some(CategoryType::Productive) => self.productive_secs += secs,
            Some(CategoryType::Distracting) => self.distracting_secs += secs,
            Some(CategoryType::Neutral) | None => self.neutral_secs += secs,
        }

        if let Some(ref cat) = tick.category_id {
            *self.category_counts.entry(cat.clone()).or_default() += secs;
        }

        if tick.is_context_switch {
            self.context_switches += 1;
        }
    }

    fn into_bucket(self) -> ActivityBucket {
        let dominant_app = dominant_key(&self.app_counts);
        let dominant_site = dominant_key(&self.site_counts);
        let dominant_category = dominant_key(&self.category_counts);

        let total_active = self.productive_secs + self.neutral_secs + self.distracting_secs;
        let focus_depth = if total_active > 0 {
            Some((self.productive_secs as f64 / total_active as f64).clamp(0.0, 1.0))
        } else {
            None
        };

        ActivityBucket {
            bucket_start: self.bucket_start.to_rfc3339(),
            date: self.bucket_start.format("%Y-%m-%d").to_string(),
            dominant_app,
            dominant_site,
            dominant_category,
            productive_secs: self.productive_secs,
            neutral_secs: self.neutral_secs,
            distracting_secs: self.distracting_secs,
            idle_secs: self.idle_secs,
            context_switches: self.context_switches,
            focus_depth,
            tick_count: self.tick_count,
        }
    }
}

pub struct BucketAggregator {
    cancel_token: CancellationToken,
    task_handle: Option<JoinHandle<()>>,
}

impl BucketAggregator {
    pub fn start(
        mut tick_rx: broadcast::Receiver<ActivityTick>,
        repos: ProductivityRepos,
        poll_interval_secs: u64,
        cancel: CancellationToken,
    ) -> Self {
        let cancel_clone = cancel.clone();
        let handle = tokio::spawn(async move {
            let mut current_bucket: Option<PendingBucket> = None;

            loop {
                tokio::select! {
                    _ = cancel_clone.cancelled() => {
                        // Flush current bucket
                        if let Some(bucket) = current_bucket.take() {
                            if bucket.tick_count > 0 {
                                let ab = bucket.into_bucket();
                                if let Err(e) = repos.buckets.upsert(&ab).await {
                                    warn!("BucketAggregator: failed to flush on shutdown: {e}");
                                }
                            }
                        }
                        break;
                    }
                    result = tick_rx.recv() => {
                        let tick = match result {
                            Ok(t) => t,
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                warn!("BucketAggregator lagged, skipped {n} ticks");
                                continue;
                            }
                            Err(broadcast::error::RecvError::Closed) => break,
                        };

                        let tick_bucket_start = bucket_start_for(&tick.timestamp);

                        // Check if we've moved to a new bucket
                        let need_new = current_bucket.as_ref()
                            .map(|b| b.bucket_start != tick_bucket_start)
                            .unwrap_or(true);

                        if need_new {
                            // Flush previous bucket
                            if let Some(old_bucket) = current_bucket.take() {
                                if old_bucket.tick_count > 0 {
                                    let ab = old_bucket.into_bucket();
                                    debug!("BucketAggregator: flushing bucket {}", ab.bucket_start);
                                    if let Err(e) = repos.buckets.upsert(&ab).await {
                                        warn!("BucketAggregator: failed to upsert bucket: {e}");
                                    }
                                }
                            }
                            current_bucket = Some(PendingBucket::new(tick_bucket_start, poll_interval_secs as i64));
                        }

                        if let Some(ref mut bucket) = current_bucket {
                            bucket.add_tick(&tick);
                        }
                    }
                }
            }
        });

        Self {
            cancel_token: cancel,
            task_handle: Some(handle),
        }
    }

    pub async fn stop(&mut self) {
        self.cancel_token.cancel();
        if let Some(handle) = self.task_handle.take() {
            if let Err(e) = handle.await {
                warn!("BucketAggregator task panicked: {e}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_bucket_start_alignment() {
        // 2026-03-06T10:03:27Z should align to 2026-03-06T10:00:00Z
        let ts = chrono::DateTime::parse_from_rfc3339("2026-03-06T10:03:27+00:00")
            .unwrap()
            .with_timezone(&Utc);
        let aligned = bucket_start_for(&ts);
        assert_eq!(aligned.format("%H:%M:%S").to_string(), "10:00:00");

        // Exact boundary stays
        let ts2 = chrono::DateTime::parse_from_rfc3339("2026-03-06T10:05:00+00:00")
            .unwrap()
            .with_timezone(&Utc);
        let aligned2 = bucket_start_for(&ts2);
        assert_eq!(aligned2.format("%H:%M:%S").to_string(), "10:05:00");
    }

    #[test]
    fn test_pending_bucket_accumulation() {
        let now = Utc::now();
        let mut bucket = PendingBucket::new(now, 5);

        let tick1 = ActivityTick {
            timestamp: now,
            app_name: "VS Code".to_string(),
            bundle_id: None,
            window_title: None,
            site_name: None,
            category_id: Some("coding".to_string()),
            category_type: Some(CategoryType::Productive),
            is_idle: false,
            idle_secs: 0.0,
            is_context_switch: false,
        };
        bucket.add_tick(&tick1);
        // Add a second VS Code tick so it becomes dominant over Chrome
        bucket.add_tick(&tick1);

        let tick2 = ActivityTick {
            app_name: "Chrome".to_string(),
            site_name: Some("Reddit".to_string()),
            category_type: Some(CategoryType::Distracting),
            is_context_switch: true,
            ..tick1.clone()
        };
        bucket.add_tick(&tick2);

        let idle_tick = ActivityTick {
            is_idle: true,
            idle_secs: 130.0,
            ..tick1.clone()
        };
        bucket.add_tick(&idle_tick);

        assert_eq!(bucket.tick_count, 4);
        assert_eq!(bucket.productive_secs, 10);
        assert_eq!(bucket.distracting_secs, 5);
        assert_eq!(bucket.idle_secs, 5);
        assert_eq!(bucket.context_switches, 1);

        let ab = bucket.into_bucket();
        assert_eq!(ab.dominant_app.as_deref(), Some("VS Code"));
        assert!(ab.focus_depth.is_some());
    }
}
