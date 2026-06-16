use async_trait::async_trait;
use std::sync::Arc;

use crate::plugin::context::PluginContext;
use crate::plugin::AppCorePlugin;
use crate::state::AppCore;

/// Bundle of all notes initialization results for FeatureHost storage.
pub struct NotesInitResult {
    pub note_embedding_handler:
        Option<Arc<dyn feature_notes::handlers::embedding::NoteEmbeddingHandler>>,
}

/// Plugin wrapper for the `feature-notes` crate.
pub struct NotesPlugin;

#[async_trait]
impl AppCorePlugin for NotesPlugin {
    fn name(&self) -> &str {
        "notes"
    }

    fn migrations(&self) -> Vec<tools_core::FeatureMigration> {
        feature_notes::notes_migrations()
    }

    async fn init(&self, ctx: &mut PluginContext) -> common::Result<()> {
        ctx.register_ai_feature(feature_notes::NotesFeature::register);
        ctx.add_feature_translator(
            feature_notes::events::try_from_domain_event,
            ai_core::RecallDomain::Notes,
        );

        let note_embedding_handler = if let (Some(ref engine), Some(ref vs)) =
            (&ctx.deps.embedding_engine, &ctx.deps.vector_store)
        {
            Some(Arc::new(
                ::agent::adapters::note_embedding::NoteEmbeddingAdapter::new(
                    Arc::clone(engine),
                    vs.clone(),
                ),
            )
                as Arc<
                    dyn feature_notes::handlers::embedding::NoteEmbeddingHandler,
                >)
        } else {
            None
        };

        // Register notes tool into the plugin-built registry
        let note_repo = feature_notes::repo::NoteRepo::new(ctx.deps.storage_pool.inner().clone());
        let mut notes_tool = feature_notes::tool::NotesTool::new(note_repo);
        if let Some(ref bus) = ctx.deps.domain_event_bus {
            notes_tool = notes_tool.with_domain_bus(Arc::clone(bus));
        }
        ctx.register_tool(notes_tool);
        tracing::info!("Notes tool registered");

        ctx.insert_handle(Arc::new(NotesInitResult {
            note_embedding_handler,
        }));
        Ok(())
    }

    async fn post_init(&self, app: &AppCore) -> common::Result<()> {
        // ── Background note embedding catch-up ────────────────────────────
        if let Some(handler) = app.note_embedding_handler() {
            let handler = Arc::clone(&handler);
            let repo = app.note_repo.clone();
            let token = app.shutdown_token.clone();
            tokio::spawn(async move {
                // Delay long enough for the app to settle. The ONNX embedding
                // model is ~420 MB; loading it at startup inflates idle RSS.
                // 120s matches EMBEDDING_IDLE_SECS so the model unloads quickly
                // if no further embeddings are needed.
                tokio::time::sleep(std::time::Duration::from_secs(120)).await;
                if token.is_cancelled() {
                    return;
                }

                match repo.list_notes_needing_embedding(50).await {
                    Ok(notes) => {
                        if !notes.is_empty() {
                            tracing::info!(
                                count = notes.len(),
                                "embedding notes without embeddings (background)"
                            );
                        }
                        for note in notes {
                            if token.is_cancelled() {
                                break;
                            }
                            if let Err(e) = handler.embed_note(&note).await {
                                tracing::debug!("background embed failed for {}: {e}", note.id);
                            } else {
                                let _ = repo.update_embedding_timestamp(&note.id).await;
                            }
                        }
                    }
                    Err(e) => tracing::warn!("failed to list notes for embedding: {e}"),
                }
            });
        }

        Ok(())
    }
}
