use async_trait::async_trait;
use std::sync::Arc;

use crate::plugin::context::PluginContext;
use crate::plugin::AppCorePlugin;

/// Plugin that initializes the insight review infrastructure:
/// embedder, cross-domain searcher, cognitive accessor, scope resolver,
/// and the unified `InsightService`.
pub struct InsightsPlugin;

#[async_trait]
impl AppCorePlugin for InsightsPlugin {
    fn name(&self) -> &str {
        "insights"
    }

    async fn init(&self, ctx: &mut PluginContext) -> common::Result<()> {
        let pool = ctx.deps.storage_pool.inner().clone();
        let note_repo = feature_notes::repo::NoteRepo::new(pool.clone());

        // ── Insight embedder (reuses the shared EmbeddingEngine) ──
        let insight_embedder: Arc<dyn feature_insights::InsightEmbedder> =
            if let (Some(ref engine), Some(ref vs)) =
                (&ctx.deps.embedding_engine, &ctx.deps.vector_store)
            {
                Arc::new(crate::adapters::insight_embedder::InsightEmbedderImpl::new(
                    Arc::clone(engine),
                    vs.clone(),
                ))
            } else {
                Arc::new(feature_insights::NoopInsightEmbedder)
            };

        // ── Cross-domain searcher (LanceDB vector search across domains) ──
        let cross_domain_searcher: Arc<dyn feature_insights::CrossDomainSearcher> =
            if let (Some(ref vs), Some(ref engine)) =
                (&ctx.deps.vector_store, &ctx.deps.embedding_engine)
            {
                Arc::new(
                    crate::adapters::cross_domain_searcher::CrossDomainSearcherImpl::new(
                        vs.clone(),
                        Arc::clone(engine),
                        ctx.deps.repos.tasks.clone(),
                        note_repo.clone(),
                        pool.clone(),
                    ),
                )
            } else {
                Arc::new(feature_insights::NoopCrossDomainSearcher)
            };

        // ── Cognitive accessor for insight context injection ──
        let cognitive_accessor: Arc<dyn feature_insights::CognitiveAccessor> = Arc::new(
            crate::adapters::cognitive_accessor::CognitiveAccessorImpl::new(
                ::cognitive::SemanticFactRepo::new(pool.clone()),
                ::cognitive::EpisodicMemoryRepo::new(pool.clone()),
                ::cognitive::ProceduralRuleRepo::new(pool.clone()),
                ::cognitive::repos::EntityRepo::new(pool.clone()),
                ::cognitive::KnowledgeAtomRepo::new(pool.clone()),
            ),
        );

        // ── Scope resolver for insight context ──
        let scope_resolver: Arc<dyn feature_insights::ScopeResolver> =
            Arc::new(crate::adapters::scope_resolver::ScopeResolverImpl::new(
                note_repo,
                ctx.deps.vector_store.clone(),
            ));

        // ── Flashcard accessor for learning progress computation ──
        let flashcard_accessor: Arc<dyn feature_insights::FlashcardAccessor> =
            Arc::new(crate::adapters::flashcard_accessor::FlashcardAccessorImpl::new(pool.clone()));

        // ── Insight service ──
        let insight_repo = feature_insights::InsightReviewRepo::new(pool.clone());
        let insight_service = Arc::new(feature_insights::InsightService::new(
            insight_repo.clone(),
            feature_insights::InsightProgressRepo::new(pool),
            scope_resolver,
            feature_insights::SmartMergeEngine::new(insight_repo),
            feature_insights::PromptBuilder::new(Arc::clone(&cognitive_accessor)),
            flashcard_accessor,
            insight_embedder,
            cross_domain_searcher,
            feature_insights::ProgressWeights::default(),
            ctx.deps.domain_event_bus.clone(),
        ));

        ctx.insert_handle(insight_service);
        Ok(())
    }

    async fn post_init(&self, _app: &crate::state::AppCore) -> common::Result<()> {
        Ok(())
    }
}
