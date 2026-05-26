//! RoutingSignalSource — accumulates SkillRouted AiSignals and flushes hourly.
//! Replaces the old RoutingMirrorSubscriber that subscribed to DomainEventBus directly.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};

use ai_core::{mirror::mirror_flush_secs, AiSignal, MirrorSignalSource, MirrorSnapshotSpec};
use async_trait::async_trait;
use dashmap::DashMap;
use jiff::Timestamp;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::mirror::{MirrorAlert, MirrorRepo, RoutingSnapshot, SkillRouteStats};

const MAX_TRIGGER_PHRASES: usize = 100;

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

/// Accumulates `SkillRouted` AiSignals in-memory and flushes hourly
/// [`RoutingSnapshot`]s to the mirror repository.
pub struct RoutingSignalSource {
    accumulator: DashMap<String, SkillRouteAccum>,
    total_count: AtomicU32,
    low_confidence_count: AtomicU32,
    repo: MirrorRepo,
}

impl RoutingSignalSource {
    pub fn new(repo: MirrorRepo) -> Self {
        Self {
            accumulator: DashMap::new(),
            total_count: AtomicU32::new(0),
            low_confidence_count: AtomicU32::new(0),
            repo,
        }
    }

    fn accumulate_signal(&self, skill_name: &str, confidence: f64, triggers: &[String]) {
        self.total_count.fetch_add(1, Ordering::Relaxed);
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
                *entry.trigger_hits.entry(trigger.clone()).or_insert(0) += 1;
            }
        }
    }

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

    fn detect_drift(
        &self,
        snapshot: &RoutingSnapshot,
        history: &[RoutingSnapshot],
    ) -> Option<MirrorAlert> {
        if history.is_empty() {
            return None;
        }

        if snapshot.fallback_rate > 0.70 {
            return Some(MirrorAlert::RoutingDrift {
                skill: "fallback".to_string(),
                delta: snapshot.fallback_rate * 100.0,
                suggestion: "Many recent messages had low routing confidence — consider \
                             reviewing your skill configuration"
                    .to_string(),
            });
        }

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

    fn reset(&self) {
        self.accumulator.clear();
        self.total_count.store(0, Ordering::Relaxed);
        self.low_confidence_count.store(0, Ordering::Relaxed);
    }
}

#[async_trait]
impl MirrorSignalSource for RoutingSignalSource {
    fn spec(&self) -> MirrorSnapshotSpec {
        MirrorSnapshotSpec {
            name: "routing",
            subscribed_kinds: &["SkillRouted"],
            flush_interval_secs: Some(mirror_flush_secs(3600)),
        }
    }

    fn name(&self) -> &'static str {
        "routing-signal-source"
    }

    async fn accumulate(&self, signal: &AiSignal) -> common::Result<()> {
        if let Some(bus::DomainEvent::SkillRouted {
            skill_name,
            confidence,
            trigger_phrases,
            ..
        }) = &signal.raw_event
        {
            self.accumulate_signal(skill_name, *confidence, trigger_phrases);
        }
        Ok(())
    }

    async fn flush(&self) -> common::Result<()> {
        let snapshot = self.build_snapshot();
        if snapshot.total_messages == 0 {
            return Ok(());
        }
        debug!(
            total_messages = snapshot.total_messages,
            fallback_rate = snapshot.fallback_rate,
            "Mirror routing flush"
        );

        let history = match self.repo.get_routing_history(7).await {
            Ok(h) => h,
            Err(e) => {
                warn!("Mirror: failed to fetch routing history: {e}");
                vec![]
            }
        };

        let drift = self.detect_drift(&snapshot, &history);

        if let Err(e) = self.repo.insert_routing_snapshot(&snapshot).await {
            warn!("Mirror: failed to persist routing snapshot: {e}");
        }

        if let Some(alert) = drift {
            info!(?alert, "Mirror: routing drift detected");
            if let Err(e) = self.repo.insert_snippet_from_alert(&alert).await {
                warn!("Mirror: failed to persist drift snippet: {e}");
            }
        }

        self.reset();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ai_core::{AiMetrics, RecallDomain, SalienceVerdict};
    use std::sync::Arc;

    fn skill_routed_signal(skill: &str, confidence: f64) -> AiSignal {
        AiSignal {
            domain: RecallDomain::General,
            event_kind: "SkillRouted",
            importance: 0.3,
            salience: SalienceVerdict::Accumulate,
            content: String::new(),
            entity: None,
            timestamp: jiff::Timestamp::now(),
            raw_event: Some(bus::DomainEvent::SkillRouted {
                skill_name: skill.to_string(),
                confidence,
                source: "keyword".into(),
                trigger_phrases: vec!["hello".into()],
                session_key: "s".into(),
            }),
            metrics: AiMetrics::default(),
            coaching_signal: false,
            coaching_rule: None,
            metric_samples: Vec::new(),
        }
    }

    #[tokio::test]
    async fn accumulates_skill_routed_signals() {
        let repo = crate::mirror::test_mirror_repo().await;
        let source = Arc::new(RoutingSignalSource::new(repo));

        source
            .accumulate(&skill_routed_signal("general", 0.85))
            .await
            .unwrap();
        source
            .accumulate(&skill_routed_signal("general", 0.85))
            .await
            .unwrap();
        source
            .accumulate(&skill_routed_signal("tasks", 0.75))
            .await
            .unwrap();

        let snapshot = source.build_snapshot();
        assert_eq!(snapshot.total_messages, 3);
        assert_eq!(snapshot.distribution["general"].count, 2);
    }

    #[tokio::test]
    async fn test_drift_detection_high_fallback() {
        let repo = crate::mirror::test_mirror_repo().await;
        let source = RoutingSignalSource::new(repo);
        for _ in 0..8 {
            source.accumulate_signal("general", 0.4, &[]);
        }
        for _ in 0..2 {
            source.accumulate_signal("general", 0.9, &[]);
        }

        let snapshot = source.build_snapshot();
        assert!(snapshot.fallback_rate > 0.70);

        let history = vec![snapshot.clone()];
        let source2 = {
            let repo = crate::mirror::test_mirror_repo().await;
            RoutingSignalSource::new(repo)
        };
        for _ in 0..9 {
            source2.accumulate_signal("general", 0.4, &[]);
        }
        source2.accumulate_signal("general", 0.9, &[]);
        let current = source2.build_snapshot();

        let alert = source2.detect_drift(&current, &history);
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
}
