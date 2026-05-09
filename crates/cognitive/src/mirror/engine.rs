//! MirrorEngine — lifecycle manager for the Mirror self-reflection layer.
//!
//! Constructs six `MirrorSignalSource` impls and wraps each in a
//! `MirrorSubscriberRunner`. Returns the runners (as `SignalConsumer`s) for
//! hand-off to the global `SignalRouter`, plus flush-loop handles the caller
//! must keep alive.

use std::sync::Arc;

use ai_core::{MirrorSubscriberRunner, SignalConsumer};
use dashmap::DashMap;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::mirror::{
    sources::{
        ApprovalHistorySource, BackgroundJobSignalSource, ConfigArchiverSource, CostCeilingSource,
        FinanceSpendingDriftSource, MetaRuleSignalSource, RoutingSignalSource,
        TaskFocusPatternSource, TodoSignalSource, TrialPreviewSource,
    },
    AutotunerBridge, MirrorFacade, MirrorRepo, NarrativeHandler,
};
use crate::repos::{EpisodicMemoryRepo, ProceduralRuleRepo};

pub struct StartedMirror {
    pub facade: MirrorFacade,
    pub consumers: Vec<Arc<dyn SignalConsumer>>,
    pub flush_handles: Vec<JoinHandle<()>>,
    pub shutdown: CancellationToken,
}

pub struct MirrorEngine;

impl MirrorEngine {
    pub fn start(
        repo: MirrorRepo,
        narrative_handler: Option<Arc<dyn NarrativeHandler>>,
        autotuner_bridge: Option<Arc<dyn AutotunerBridge>>,
        episodic_repo: Option<EpisodicMemoryRepo>,
        rule_repo: Option<ProceduralRuleRepo>,
        trial_evaluator: Option<Arc<dyn crate::mirror::types::EarlyTrialEvaluator>>,
        approval_history_repo: Option<Arc<storage::repos::CodingApprovalHistoryRepo>>,
        approval_pattern_repo: Option<Arc<storage::repos::ApprovalPatternHistoryRepo>>,
        bash_repo: Option<Arc<storage::repos::BashJobRepo>>,
    ) -> StartedMirror {
        let shutdown = CancellationToken::new();
        let active_timers: Arc<DashMap<String, JoinHandle<()>>> = Arc::new(DashMap::new());

        // Build each source.
        let routing = Arc::new(RoutingSignalSource::new(repo.clone()));
        let meta_rule = Arc::new(MetaRuleSignalSource::new(repo.clone()));
        let config_archiver = Arc::new(ConfigArchiverSource::new(
            repo.clone(),
            autotuner_bridge.clone(),
        ));
        let trial = Arc::new(TrialPreviewSource::new(
            repo.clone(),
            active_timers.clone(),
            trial_evaluator,
        ));
        let task_focus = Arc::new(TaskFocusPatternSource::new(repo.clone()));
        let finance_drift = Arc::new(FinanceSpendingDriftSource::new(repo.clone()));
        let todo = Arc::new(TodoSignalSource::new(repo.clone()));

        // Wrap each in a runner; spawn flush loops for sources that declare an interval.
        let mut consumers: Vec<Arc<dyn SignalConsumer>> = Vec::new();
        let mut flush_handles: Vec<JoinHandle<()>> = Vec::new();

        macro_rules! register {
            ($source:expr) => {{
                let runner = MirrorSubscriberRunner::new($source, shutdown.clone());
                if let Some(h) = runner.clone().spawn_declared_flush_loop() {
                    flush_handles.push(h);
                }
                consumers.push(runner as Arc<dyn SignalConsumer>);
            }};
        }
        register!(routing);
        register!(meta_rule);
        register!(config_archiver);
        register!(trial);
        register!(task_focus);
        register!(finance_drift);
        register!(todo);

        let mut ah_source: Option<Arc<ApprovalHistorySource>> = None;
        if let (Some(ah_repo), Some(ap_repo)) = (approval_history_repo, approval_pattern_repo) {
            let approval_history = Arc::new(ApprovalHistorySource::new(ap_repo, ah_repo));
            ah_source = Some(approval_history.clone());
            register!(approval_history);
        }

        let cost_ceiling = Arc::new(CostCeilingSource::new(repo.clone()));
        register!(cost_ceiling);

        if let (Some(ep), Some(br)) = (&episodic_repo, &bash_repo) {
            let bg_job_source = Arc::new(BackgroundJobSignalSource::new(ep.clone(), br.clone()));
            register!(bg_job_source);
        }

        // Build the facade (unchanged API; drop the now-unused domain_event_bus).
        let mut facade = MirrorFacade::new(repo);
        facade = facade.with_active_timers(active_timers);
        if let Some(handler) = narrative_handler {
            facade = facade.with_narrative_handler(handler);
        }
        if let Some(bridge) = autotuner_bridge {
            facade = facade.with_autotuner_bridge(bridge);
        }
        if let Some(episodic) = episodic_repo {
            facade = facade.with_episodic_repo(episodic);
        }
        if let Some(r) = rule_repo {
            facade = facade.with_rule_repo(r);
        }
        if let Some(ah) = ah_source {
            facade = facade.with_approval_history(ah);
        }

        StartedMirror {
            facade,
            consumers,
            flush_handles,
            shutdown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn start_produces_eight_consumers() {
        let repo = crate::mirror::test_mirror_repo().await;
        let built = MirrorEngine::start(repo, None, None, None, None, None, None, None, None);
        assert_eq!(
            built.consumers.len(),
            8,
            "routing + meta_rule + config_archiver + trial + task_focus + finance_drift + cost_ceiling + todo"
        );
        for h in built.flush_handles.iter() {
            assert!(!h.is_finished());
        }
        built.shutdown.cancel();
        for h in built.flush_handles {
            h.await.unwrap();
        }
    }
}
