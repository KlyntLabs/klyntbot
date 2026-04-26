//! Coding meta-rule detectors.
//!
//! Monitors `FixAttemptFailed` events and emits coding-specific mirror alerts
//! when thresholds are crossed.

use std::collections::HashMap;
use tokio::sync::Mutex;

use ai_core::{AiSignal, MirrorSignalSource, MirrorSnapshotSpec};
use async_trait::async_trait;
use tracing::warn;

use cognitive::mirror::{snippet_from_alert, MirrorAlert, MirrorAlertSeverity, MirrorRepo};

/// Source that detects coding meta-rule violations from domain events.
pub struct CodingMetaRulesSource {
    repo: MirrorRepo,
    /// Tracks the highest `attempt_count` seen per `problem_hash`.
    fix_failures: Mutex<HashMap<String, u32>>,
}

impl CodingMetaRulesSource {
    /// Construct.
    pub fn new(repo: MirrorRepo) -> Self {
        Self {
            repo,
            fix_failures: Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl MirrorSignalSource for CodingMetaRulesSource {
    fn spec(&self) -> MirrorSnapshotSpec {
        MirrorSnapshotSpec {
            name: "coding_meta_rules",
            subscribed_kinds: &["FixAttemptFailed"],
            flush_interval_secs: None,
        }
    }

    fn name(&self) -> &'static str {
        "coding-meta-rules-source"
    }

    async fn accumulate(&self, signal: &AiSignal) -> common::Result<()> {
        if let Some(bus::DomainEvent::FixAttemptFailed {
            problem_hash,
            attempt_count,
            ..
        }) = &signal.raw_event
        {
            let should_emit = {
                let mut failures = self.fix_failures.lock().await;
                let prev = failures.get(problem_hash).copied().unwrap_or(0);
                let emit = *attempt_count >= 3 && prev < 3;
                failures.insert(problem_hash.clone(), *attempt_count);
                drop(failures);
                emit
            };

            if should_emit {
                let alert = MirrorAlert::Coding {
                    kind: "problem_class_refactor".into(),
                    severity: MirrorAlertSeverity::High,
                    payload: serde_json::json!({ "problem_hash": problem_hash }),
                };
                let snippet = snippet_from_alert(&alert);
                if let Err(e) = self.repo.insert_snippet(&snippet).await {
                    warn!("CodingMetaRulesSource: failed to insert snippet: {e}");
                }
            }
        }
        Ok(())
    }

    async fn flush(&self) -> common::Result<()> {
        // Event-driven only — no periodic flush.
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Deferred detectors (Task 21 — 3 of 4)
// ---------------------------------------------------------------------------
//
// These three detectors are blocked on upstream domain events that the
// workspace does not yet emit. Each requires both a new `DomainEvent` variant
// in `bus::domain_events` AND an emitter wired into the appropriate signal
// path (distiller, problem-class repo, counterfactual extractor). They are
// tracked here as a single source of truth so the gap is discoverable from
// the implementation site.
//
// 1. LowerDeadEndThreshold
//    - Trigger: `DomainEvent::DeadEndDetected { problem_hash, .. }`
//    - Action: emit `MirrorAlert::Coding { kind: "lower_dead_end_threshold" }`
//      so the autotuner can reduce the dead-end memory creation threshold.
//
// 2. PromoteToGlobal
//    - Trigger: `DomainEvent::LocalPatternStabilized { pattern_id, repo_id, hits }`
//    - Action: emit `MirrorAlert::Coding { kind: "promote_pattern_to_global" }`
//      so a repo-scoped pattern with sustained cross-context hits is promoted
//      to global scope.
//
// 3. PromoteCounterfactualVisibility
//    - Trigger: `DomainEvent::CounterfactualDerived { memory_id, .. }`
//    - Action: emit `MirrorAlert::Coding { kind: "raise_counterfactual_visibility" }`
//      so counterfactual memories surface higher in retrieval.
//
// To activate any of these: add the event variant to `bus::DomainEvent`,
// emit it from the relevant pipeline, then add a detector struct here that
// implements `MirrorSignalSource` (model after `CodingMetaRulesSource`).
