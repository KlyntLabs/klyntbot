//! RoutingMirrorSubscriber — accumulates SkillRouted events and flushes hourly snapshots.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};
use uuid::Uuid;

use bus::DomainEvent;
use jiff::Timestamp;

use crate::mirror::{
    snippet_from_alert, MirrorAlert, MirrorRepo, NarrativeSnippet, RoutingSnapshot, SkillRouteStats,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const MAX_TRIGGER_PHRASES: usize = 100;

// ---------------------------------------------------------------------------
// Internal accumulator entry
// ---------------------------------------------------------------------------

struct SkillRouteAccum {
    count: u32,
    confidence_sum: f64,
    trigger_hits: HashMap<String, u32>,
}

impl SkillRouteAccum {
    fn new() -> Self {
        Self {
            count: 0,
            confidence_sum: 0.0,
            trigger_hits: HashMap::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// RoutingMirrorSubscriber
// ---------------------------------------------------------------------------

/// Accumulates `SkillRouted` domain events in-memory and flushes hourly
/// [`RoutingSnapshot`]s to the mirror repository.
///
/// Drift detection runs on each flush: if the fallback rate exceeds 0.70 or
/// any skill shifts more than 15pp from its 7-day average, a
/// [`NarrativeSnippet`] is written to the repo.
pub struct RoutingMirrorSubscriber {
    accumulator: DashMap<String, SkillRouteAccum>,
    total_count: AtomicU32,
    low_confidence_count: AtomicU32,
    repo: Option<MirrorRepo>,
}

impl RoutingMirrorSubscriber {
    /// Create a subscriber backed by a real [`MirrorRepo`].
    pub fn new(repo: MirrorRepo) -> Self {
        Self {
            accumulator: DashMap::new(),
            total_count: AtomicU32::new(0),
            low_confidence_count: AtomicU32::new(0),
            repo: Some(repo),
        }
    }

    /// Create a test instance with no repo (flush is a no-op for storage).
    #[cfg(test)]
    pub fn new_for_test() -> Self {
        Self {
            accumulator: DashMap::new(),
            total_count: AtomicU32::new(0),
            low_confidence_count: AtomicU32::new(0),
            repo: None,
        }
    }

    // -----------------------------------------------------------------------
    // Accumulation
    // -----------------------------------------------------------------------

    /// Record a single skill routing event.
    pub fn accumulate(&self, skill_name: &str, confidence: f64, triggers: Vec<String>) {
        self.total_count.fetch_add(1, Ordering::Relaxed);
        // Confidence < 0.6 counts as low-confidence.
        if confidence < 0.6 {
            self.low_confidence_count.fetch_add(1, Ordering::Relaxed);
        }
        let mut entry = self
            .accumulator
            .entry(skill_name.to_string())
            .or_insert_with(SkillRouteAccum::new);
        entry.count += 1;
        entry.confidence_sum += confidence;
        if entry.trigger_hits.len() < MAX_TRIGGER_PHRASES {
            for trigger in triggers {
                *entry.trigger_hits.entry(trigger).or_insert(0) += 1;
            }
        }
    }

    // -----------------------------------------------------------------------
    // Snapshot building
    // -----------------------------------------------------------------------

    /// Build a [`RoutingSnapshot`] from the current accumulator state without
    /// clearing it.
    pub fn build_snapshot(&self) -> RoutingSnapshot {
        let total = self.total_count.load(Ordering::Relaxed);
        let low_conf = self.low_confidence_count.load(Ordering::Relaxed);

        let fallback_rate = if total > 0 {
            low_conf as f64 / total as f64
        } else {
            0.0
        };

        let mut distribution: HashMap<String, SkillRouteStats> = HashMap::new();
        let mut global_conf_sum = 0.0_f64;
        let mut global_count = 0_u32;

        for entry in self.accumulator.iter() {
            let skill = entry.key().clone();
            let accum = entry.value();

            let percentage = if total > 0 {
                accum.count as f64 / total as f64 * 100.0
            } else {
                0.0
            };

            let avg_confidence = if accum.count > 0 {
                accum.confidence_sum / accum.count as f64
            } else {
                0.0
            };

            // Top triggers: sort by hit frequency, take up to 5.
            let mut sorted_triggers: Vec<(String, u32)> = accum
                .trigger_hits
                .iter()
                .map(|(k, v)| (k.clone(), *v))
                .collect();
            sorted_triggers.sort_by(|a, b| b.1.cmp(&a.1));
            let top_triggers: Vec<String> = sorted_triggers
                .into_iter()
                .take(5)
                .map(|(k, _)| k)
                .collect();

            global_conf_sum += accum.confidence_sum;
            global_count += accum.count;

            distribution.insert(
                skill,
                SkillRouteStats {
                    count: accum.count,
                    percentage,
                    avg_confidence,
                    top_triggers,
                },
            );
        }

        let avg_routing_confidence = if global_count > 0 {
            global_conf_sum / global_count as f64
        } else {
            0.0
        };

        RoutingSnapshot {
            id: Uuid::new_v4(),
            captured_at: Timestamp::now(),
            window_hours: 1,
            total_messages: total,
            distribution,
            fallback_rate,
            avg_routing_confidence,
            low_confidence_count: low_conf,
            user_feedback: None,
        }
    }

    // -----------------------------------------------------------------------
    // Drift detection
    // -----------------------------------------------------------------------

    /// Check the current snapshot against historical baselines for drift.
    ///
    /// Returns `Some(MirrorAlert)` when:
    /// - `fallback_rate > 0.70`, or
    /// - any skill's percentage shifted > 15pp from its historical average.
    ///
    /// Returns `None` when there is no history to compare against.
    pub fn detect_drift(
        &self,
        snapshot: &RoutingSnapshot,
        history: &[RoutingSnapshot],
    ) -> Option<MirrorAlert> {
        if history.is_empty() {
            return None;
        }

        // Check fallback rate first.
        if snapshot.fallback_rate > 0.70 {
            return Some(MirrorAlert::RoutingDrift {
                skill: "fallback".to_string(),
                delta: snapshot.fallback_rate * 100.0,
                suggestion: "Many recent messages had low routing confidence — consider \
                             reviewing your skill configuration"
                    .to_string(),
            });
        }

        // Check per-skill percentage shift against historical average.
        for (skill, stats) in &snapshot.distribution {
            let historical_pcts: Vec<f64> = history
                .iter()
                .filter_map(|h| h.distribution.get(skill))
                .map(|s| s.percentage)
                .collect();

            if historical_pcts.is_empty() {
                continue;
            }

            let avg_pct: f64 = historical_pcts.iter().sum::<f64>() / historical_pcts.len() as f64;
            let delta = (stats.percentage - avg_pct).abs();
            if delta > 15.0 {
                return Some(MirrorAlert::RoutingDrift {
                    skill: skill.clone(),
                    delta,
                    suggestion: format!(
                        "Consider reviewing the '{skill}' skill's trigger configuration"
                    ),
                });
            }
        }

        None
    }

    // -----------------------------------------------------------------------
    // Reset
    // -----------------------------------------------------------------------

    /// Clear all accumulated data and reset counters.
    pub fn reset(&self) {
        self.accumulator.clear();
        self.total_count.store(0, Ordering::Relaxed);
        self.low_confidence_count.store(0, Ordering::Relaxed);
    }

    // -----------------------------------------------------------------------
    // Flush
    // -----------------------------------------------------------------------

    /// Build snapshot, detect drift, write to repo, reset accumulator.
    pub async fn flush(&self) {
        let snapshot = self.build_snapshot();
        debug!(
            total_messages = snapshot.total_messages,
            fallback_rate = snapshot.fallback_rate,
            "Mirror routing flush"
        );

        if let Some(repo) = &self.repo {
            // Fetch history for drift detection.
            let history = match repo.get_routing_history(7).await {
                Ok(h) => h,
                Err(e) => {
                    warn!("Mirror: failed to fetch routing history: {e}");
                    vec![]
                }
            };

            let drift = self.detect_drift(&snapshot, &history);

            // Persist snapshot.
            if let Err(e) = repo.insert_routing_snapshot(&snapshot).await {
                warn!("Mirror: failed to persist routing snapshot: {e}");
            }

            // Persist drift snippet if any.
            if let Some(alert) = drift {
                info!(?alert, "Mirror: routing drift detected");
                let snippet: NarrativeSnippet = snippet_from_alert(&alert);
                if let Err(e) = repo.insert_snippet(&snippet).await {
                    warn!("Mirror: failed to persist drift snippet: {e}");
                }
            }
        }

        self.reset();
    }

    // -----------------------------------------------------------------------
    // Run loop
    // -----------------------------------------------------------------------

    /// Start the event-driven subscriber loop.
    ///
    /// - Listens for [`DomainEvent::SkillRouted`] on `rx` and calls
    ///   [`accumulate`](Self::accumulate).
    /// - Flushes every hour via `tokio::time::interval`.
    /// - Stops cleanly when `shutdown` is cancelled.
    pub async fn run(
        self: Arc<Self>,
        mut rx: broadcast::Receiver<DomainEvent>,
        shutdown: CancellationToken,
    ) {
        let mut interval = tokio::time::interval(Duration::from_secs(3600));
        // Skip missed ticks (e.g. if flush took longer than expected).
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        // Consume the immediate first tick so the first flush is after 1 hour.
        interval.tick().await;

        loop {
            tokio::select! {
                _ = shutdown.cancelled() => {
                    info!("RoutingMirrorSubscriber: shutdown received, flushing before exit");
                    self.flush().await;
                    break;
                }
                _ = interval.tick() => {
                    self.flush().await;
                }
                result = rx.recv() => {
                    match result {
                        Ok(DomainEvent::SkillRouted {
                            skill_name,
                            confidence,
                            trigger_phrases,
                            ..
                        }) => {
                            self.accumulate(&skill_name, confidence, trigger_phrases);
                        }
                        Ok(_) => {
                            // Not a SkillRouted event — ignore.
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            warn!("RoutingMirrorSubscriber: lagged, skipped {n} events");
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            info!("RoutingMirrorSubscriber: event channel closed");
                            break;
                        }
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_accumulate_skill_routed() {
        let subscriber = RoutingMirrorSubscriber::new_for_test();
        subscriber.accumulate("general", 0.85, vec!["hello".to_string()]);
        subscriber.accumulate("general", 0.90, vec![]);
        subscriber.accumulate("finance-management", 0.75, vec!["budget".to_string()]);

        let snapshot = subscriber.build_snapshot();
        assert_eq!(snapshot.total_messages, 3);
        assert_eq!(snapshot.distribution.len(), 2);
        assert_eq!(snapshot.distribution["general"].count, 2);
    }

    #[test]
    fn test_drift_detection_high_fallback() {
        let subscriber = RoutingMirrorSubscriber::new_for_test();
        // Add 10 events with confidence < 0.6 (low) to push fallback_rate over 0.70.
        for _ in 0..8 {
            subscriber.accumulate("general", 0.4, vec![]);
        }
        for _ in 0..2 {
            subscriber.accumulate("general", 0.9, vec![]);
        }

        let snapshot = subscriber.build_snapshot();
        assert!(snapshot.fallback_rate > 0.70);

        // Need at least one historical snapshot to trigger drift.
        let history = vec![snapshot.clone()];
        // Build a new snapshot with different data as "current".
        let subscriber2 = RoutingMirrorSubscriber::new_for_test();
        for _ in 0..9 {
            subscriber2.accumulate("general", 0.4, vec![]);
        }
        subscriber2.accumulate("general", 0.9, vec![]);
        let current = subscriber2.build_snapshot();

        let alert = subscriber2.detect_drift(&current, &history);
        assert!(
            alert.is_some(),
            "Expected drift alert for high fallback rate"
        );
        match alert.unwrap() {
            MirrorAlert::RoutingDrift { skill, .. } => {
                assert_eq!(skill, "fallback");
            }
            other => panic!("Expected RoutingDrift alert, got {other:?}"),
        }
    }

    #[test]
    fn test_drift_detection_no_baseline() {
        let subscriber = RoutingMirrorSubscriber::new_for_test();
        subscriber.accumulate("general", 0.85, vec![]);
        let snapshot = subscriber.build_snapshot();

        // No history → no alert.
        let alert = subscriber.detect_drift(&snapshot, &[]);
        assert!(alert.is_none(), "No drift alert expected with no history");
    }

    #[test]
    fn test_reset_clears_accumulator() {
        let subscriber = RoutingMirrorSubscriber::new_for_test();
        subscriber.accumulate("general", 0.85, vec![]);
        subscriber.reset();

        let snapshot = subscriber.build_snapshot();
        assert_eq!(snapshot.total_messages, 0);
        assert!(snapshot.distribution.is_empty());
    }

    #[test]
    fn test_top_triggers_sorted() {
        let subscriber = RoutingMirrorSubscriber::new_for_test();
        subscriber.accumulate("general", 0.9, vec!["hi".to_string(), "hello".to_string()]);
        subscriber.accumulate("general", 0.9, vec!["hi".to_string()]);
        subscriber.accumulate("general", 0.9, vec!["hi".to_string()]);

        let snapshot = subscriber.build_snapshot();
        let stats = &snapshot.distribution["general"];
        // "hi" appeared 3 times, "hello" once — "hi" should be first.
        assert_eq!(stats.top_triggers[0], "hi");
    }

    #[test]
    fn test_avg_confidence_computed() {
        let subscriber = RoutingMirrorSubscriber::new_for_test();
        subscriber.accumulate("general", 0.8, vec![]);
        subscriber.accumulate("general", 0.6, vec![]);

        let snapshot = subscriber.build_snapshot();
        let stats = &snapshot.distribution["general"];
        let expected = (0.8 + 0.6) / 2.0;
        assert!((stats.avg_confidence - expected).abs() < 1e-9);
    }
}
