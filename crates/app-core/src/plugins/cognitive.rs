use async_trait::async_trait;
use std::sync::Arc;

use crate::plugin::context::PluginContext;
use crate::plugin::AppCorePlugin;
use crate::state::AppCore;

/// Plugin wrapper for the `cognitive` crate.
pub struct CognitivePlugin;

#[async_trait]
impl AppCorePlugin for CognitivePlugin {
    fn name(&self) -> &str {
        "cognitive"
    }

    fn migrations(&self) -> Vec<tools_core::FeatureMigration> {
        cognitive::cognitive_migrations()
    }

    async fn init(&self, ctx: &mut PluginContext) -> common::Result<()> {
        let repo = ::cognitive::repos::PendingMemoryRepo::new(ctx.deps.storage_pool.inner().clone());
        if let Err(e) = repo.migrate().await {
            tracing::warn!("Pending memory migration failed: {e}");
        }
        ctx.insert_handle(Arc::new(repo));
        Ok(())
    }

    async fn post_init(&self, app: &AppCore) -> common::Result<()> {
        // Run cognitive init (skill seeding, ingestion token, file watcher, work context).
        {
            let mut config = app.config.write().await;
            crate::init::cognitive::init_cognitive(
                &mut *config,
                &app.storage_pool,
                app.activity_ingestion_service
                    .as_ref()
                    .expect("activity svc available"),
                &app.shutdown_token,
                app.embedding_engine
                    .as_ref()
                    .expect("embedding engine available")
                    .clone(),
            )
            .await;
        }

        let (Some(ref domain_event_bus), Some(ref activity_svc)) =
            (&app.domain_event_bus, &app.activity_ingestion_service)
        else {
            tracing::warn!("cognitive plugin: missing deps for post-core services, skipping");
            return Ok(());
        };
        crate::init::cognitive::spawn_post_core_services(
            app,
            domain_event_bus,
            Arc::clone(activity_svc),
            &app.shutdown_token,
        );

        // ── Register insight progress refresh cron ────────────────────────
        if let Some(ref insight_svc) = app.insight_service {
            let svc = Arc::clone(insight_svc);
            let note_repo = app.note_repo.clone();
            let rt = tokio::runtime::Handle::current();
            app.cron_executor.register(
                crate::init::cron::JOB_INSIGHT_REFRESH,
                Arc::new(move |_job: &scheduling::CronJob| {
                    let svc = Arc::clone(&svc);
                    let note_repo = note_repo.clone();
                    tokio::task::block_in_place(|| {
                        rt.block_on(async move {
                            match crate::init::cron::refresh_insight_progress(&svc, &note_repo).await {
                                Ok(Some(msg)) => Ok(Some(msg)),
                                Ok(None) => Ok(Some("No insights to refresh".to_string())),
                                Err(e) => Ok(Some(format!("Insight refresh failed: {e}"))),
                            }
                        })
                    })
                }),
            );
        }

        // ── Register nightly cross-domain batch cron ──────────────────────
        {
            let pool = app.storage_pool.clone();
            let nightly_provider = app.cognitive_provider.clone();
            let nightly_model = {
                let cfg = app.config.read().await;
                cfg.cognitive
                    .model
                    .clone()
                    .unwrap_or_else(|| cfg.agents.defaults.model.clone())
            };
            let rt = tokio::runtime::Handle::current();
            app.cron_executor.register(
                crate::init::cron::JOB_CROSS_DOMAIN_NIGHTLY,
                Arc::new(move |_job: &scheduling::CronJob| {
                    let pool = pool.clone();
                    let provider = nightly_provider.clone();
                    let model = nightly_model.clone();
                    tokio::task::block_in_place(|| {
                        rt.block_on(async move {
                            match crate::init::cron::run_nightly_batch(&pool, provider.as_ref(), &model).await {
                                Ok(Some(msg)) => Ok(Some(msg)),
                                Ok(None) => Ok(Some("No cross-domain dots today".to_string())),
                                Err(e) => Ok(Some(format!("Nightly batch failed: {e}"))),
                            }
                        })
                    })
                }),
            );
        }

        Ok(())
    }
}
