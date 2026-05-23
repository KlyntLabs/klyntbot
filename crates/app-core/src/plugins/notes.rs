use async_trait::async_trait;
use std::sync::Arc;

use crate::plugin::context::PluginContext;
use crate::plugin::AppCorePlugin;
use crate::state::AppCore;

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

    async fn init(&self, _ctx: &mut PluginContext) -> common::Result<()> {
        Ok(())
    }

    async fn post_init(&self, app: &AppCore) -> common::Result<()> {
        let mut notes_tool = feature_notes::tool::NotesTool::new(app.note_repo.clone());
        if let Some(ref bus) = app.domain_event_bus {
            notes_tool = notes_tool.with_domain_bus(Arc::clone(bus));
        }
        let reg = app.agent.tool_registry();
        let mut registry = reg.write().await;
        registry.register(notes_tool);
        tracing::info!("Notes tool registered");

        // ── Background note embedding catch-up ────────────────────────────
        if let Some(ref handler) = app.note_embedding_handler {
            let handler = Arc::clone(handler);
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
