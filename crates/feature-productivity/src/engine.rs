//! ProductivityEngine — orchestrator that owns the ActivityTracker
//! and all broadcast subscribers. Single entry point for desktop crate.

use std::sync::Arc;

use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;

use bus::DomainEventBus;

use crate::auto_focus::{AutoFocusDetector, AutoFocusSession};
use crate::batch_writer::BatchWriter;
use crate::bucket_aggregator::BucketAggregator;
use crate::config::ProductivityConfig;
use crate::distraction_analyzer::DistractionAnalyzer;
use crate::project_detector::ProjectDetector;
use crate::repos::ProductivityRepos;
use crate::tracker::categorizer::Categorizer;
use crate::tracker::ActivityTracker;
use crate::types::ActivityTick;

const BROADCAST_CAPACITY: usize = 128;

pub struct ProductivityEngine {
    tracker: ActivityTracker,
    batch_writer: Option<BatchWriter>,
    bucket_aggregator: Option<BucketAggregator>,
    auto_focus: Option<AutoFocusDetector>,
    distraction_analyzer: Option<DistractionAnalyzer>,
    cancel_token: CancellationToken,
    auto_focus_rx: Option<mpsc::Receiver<AutoFocusSession>>,
    tick_sender: broadcast::Sender<ActivityTick>,
    domain_bus: Option<Arc<DomainEventBus>>,
}

impl ProductivityEngine {
    pub fn new(
        config: ProductivityConfig,
        repos: ProductivityRepos,
        categorizer: Categorizer,
    ) -> Self {
        Self::new_with_bus(config, repos, categorizer, None)
    }

    pub fn new_with_bus(
        config: ProductivityConfig,
        repos: ProductivityRepos,
        categorizer: Categorizer,
        domain_bus: Option<Arc<DomainEventBus>>,
    ) -> Self {
        let (tick_tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        let cancel = CancellationToken::new();

        let detector = ProjectDetector::new();
        let tracker = ActivityTracker::new(
            config.clone(),
            categorizer,
            detector,
            repos.clone(),
            tick_tx.clone(),
        );

        // BatchWriter
        let batch_writer = BatchWriter::start(
            tick_tx.subscribe(),
            repos.clone(),
            config.privacy.clone(),
            config.tracking.batch_write_interval_secs,
            cancel.child_token(),
        );

        // BucketAggregator
        let bucket_aggregator = BucketAggregator::start(
            tick_tx.subscribe(),
            repos.clone(),
            config.tracking.poll_interval_secs,
            cancel.child_token(),
        );

        // AutoFocusDetector
        let (auto_focus_tx, auto_focus_rx) = mpsc::channel(16);
        let auto_focus = if config.focus.auto_detect_enabled {
            Some(AutoFocusDetector::start(
                tick_tx.subscribe(),
                auto_focus_tx,
                config.focus.clone(),
                config.tracking.poll_interval_secs,
                cancel.child_token(),
            ))
        } else {
            None
        };

        // DistractionAnalyzer
        let distraction_analyzer = DistractionAnalyzer::start_with_bus(
            tick_tx.subscribe(),
            repos.clone(),
            domain_bus.clone(),
            cancel.child_token(),
        );

        Self {
            tracker,
            batch_writer: Some(batch_writer),
            bucket_aggregator: Some(bucket_aggregator),
            auto_focus,
            distraction_analyzer: Some(distraction_analyzer),
            cancel_token: cancel,
            auto_focus_rx: Some(auto_focus_rx),
            tick_sender: tick_tx,
            domain_bus,
        }
    }

    /// Take the auto-focus session receiver (for desktop crate to consume).
    pub fn take_auto_focus_rx(&mut self) -> Option<mpsc::Receiver<AutoFocusSession>> {
        self.auto_focus_rx.take()
    }

    /// Get a new broadcast subscriber (for DashboardEmitter).
    pub fn subscribe(&self) -> broadcast::Receiver<ActivityTick> {
        self.tick_sender.subscribe()
    }

    pub fn start(&mut self) {
        self.tracker.start();
    }

    pub async fn stop(&mut self) {
        self.tracker.stop().await;
        self.cancel_token.cancel();
        // Stop all background workers in parallel — they all react to the same cancel token
        let bw_fut = async {
            if let Some(mut bw) = self.batch_writer.take() {
                bw.stop().await;
            }
        };
        let ba_fut = async {
            if let Some(mut ba) = self.bucket_aggregator.take() {
                ba.stop().await;
            }
        };
        let af_fut = async {
            if let Some(mut af) = self.auto_focus.take() {
                af.stop().await;
            }
        };
        let da_fut = async {
            if let Some(mut da) = self.distraction_analyzer.take() {
                da.stop().await;
            }
        };
        tokio::join!(bw_fut, ba_fut, af_fut, da_fut);
    }

    pub fn domain_bus(&self) -> Option<&Arc<DomainEventBus>> {
        self.domain_bus.as_ref()
    }

    pub fn categorizer(&self) -> &std::sync::Arc<tokio::sync::RwLock<Categorizer>> {
        self.tracker.categorizer()
    }
}
