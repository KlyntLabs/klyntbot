//! Wire the four Phase-5 coding mirror sources to the workspace SignalRouter.

use ai_core::{MirrorSignalSource, MirrorSubscriberRunner, SignalConsumer};
use coding_memory::mirror::coding_meta_rules::CodingMetaRulesSource;
use coding_memory::mirror::coding_routing::CodingRoutingSource;
use coding_memory::mirror::pattern_effectiveness::{
    PatternEffectivenessLogRepo, PatternEffectivenessSource,
};
use coding_memory::mirror::stale_memory::StaleMemorySource;
use cognitive::mirror::MirrorRepo;
use common::Result;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

/// Bundle returned by [`register_coding_sources`].
pub struct RegisteredCodingSources {
    /// `SignalConsumer` runners ready to extend the SignalRouter consumer list.
    pub consumers: Vec<Arc<dyn SignalConsumer>>,
    /// Flush-loop handles for any source that declared a flush interval.
    pub flush_handles: Vec<JoinHandle<()>>,
}

/// Build the four coding-specific signal sources, wrap each in a
/// [`MirrorSubscriberRunner`], and return the runners as `SignalConsumer`s
/// plus any spawned flush-loop handles.
pub async fn register_coding_sources(
    pool: storage::StoragePool,
    mirror_repo: MirrorRepo,
    shutdown: CancellationToken,
) -> Result<RegisteredCodingSources> {
    let log = PatternEffectivenessLogRepo::new(pool.clone());

    let mut consumers: Vec<Arc<dyn SignalConsumer>> = Vec::new();
    let mut flush_handles: Vec<JoinHandle<()>> = Vec::new();

    macro_rules! register {
        ($source:expr) => {{
            let source = Arc::new($source);
            let runner = MirrorSubscriberRunner::new(source, shutdown.clone());
            if let Some(h) = runner.clone().spawn_declared_flush_loop() {
                flush_handles.push(h);
            }
            consumers.push(runner as Arc<dyn SignalConsumer>);
        }};
    }

    register!(PatternEffectivenessSource::new(pool.clone(), log));
    register!(StaleMemorySource::new(pool.clone()));
    register!(CodingMetaRulesSource::new(mirror_repo.clone()));
    register!(CodingRoutingSource::new(mirror_repo));

    Ok(RegisteredCodingSources {
        consumers,
        flush_handles,
    })
}

/// Trait-object signature kept for any callers that only need the raw sources
/// (e.g. tests). New callers should prefer [`register_coding_sources`] above.
#[doc(hidden)]
pub async fn coding_sources_for_test(
    pool: storage::StoragePool,
    mirror_repo: MirrorRepo,
) -> Result<Vec<Arc<dyn MirrorSignalSource>>> {
    let log = PatternEffectivenessLogRepo::new(pool.clone());
    Ok(vec![
        Arc::new(PatternEffectivenessSource::new(pool.clone(), log)) as Arc<dyn MirrorSignalSource>,
        Arc::new(StaleMemorySource::new(pool.clone())) as Arc<dyn MirrorSignalSource>,
        Arc::new(CodingMetaRulesSource::new(mirror_repo.clone())) as Arc<dyn MirrorSignalSource>,
        Arc::new(CodingRoutingSource::new(mirror_repo)) as Arc<dyn MirrorSignalSource>,
    ])
}
