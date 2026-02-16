//! Backfill embeddings for existing tasks that don't have them.
//!
//! Generates embeddings for all tasks missing from the embedding store,
//! enabling semantic search on pre-Sprint 5 data.

use anyhow::Result;
use common::utils::terminal::*;
use std::sync::Arc;
use tokio::sync::RwLock;
use tools::embedding_engine::EmbeddingHandler;
use tools::todo_store::TodoStore;
use tools::todo_types::TodoFilter;

/// Handle `klyntbot todo backfill-embeddings` command.
///
/// Loads all tasks, checks which are missing embeddings, and generates
/// embeddings for the missing ones. Idempotent — safe to run multiple times.
pub async fn handle_backfill() -> Result<()> {
    let config = config::load().await?;
    let store_path = config.todo_store_path();
    let emb_store_path = config.embedding_store_path();

    let mut store = TodoStore::new(store_path);
    let emb_store = Arc::new(RwLock::new(tools::EmbeddingStore::new(emb_store_path)));

    // Create embedding engine + handler
    let engine = Arc::new(tools::EmbeddingEngine::new());
    let handler = Arc::new(tools::EmbeddingEngineImpl::new(engine, emb_store.clone()));

    // Check if engine is available
    if !handler.is_available() {
        eprintln!(
            "{} Embedding engine not available. The model may need to download first (~420MB).",
            colorize("Error:", ERROR),
        );
        return Ok(());
    }

    // Load all tasks
    let todos = store.list(&TodoFilter::default()).await?;
    if todos.is_empty() {
        println!("No tasks found. Nothing to backfill.");
        return Ok(());
    }

    // Find tasks missing embeddings
    let todo_ids: Vec<String> = todos.iter().map(|t| t.id.clone()).collect();
    let missing = {
        let mut emb_guard = emb_store.write().await;
        // Ensure store is loaded before checking
        emb_guard.get_all().await?;
        emb_guard.ids_missing_embeddings(&todo_ids)
    };

    if missing.is_empty() {
        println!(
            "{} All {} tasks already have embeddings. Nothing to backfill.",
            colorize("✓", SUCCESS),
            todos.len(),
        );
        return Ok(());
    }

    println!(
        "Generating embeddings for {} of {} tasks...",
        missing.len(),
        todos.len(),
    );

    let mut success_count = 0;
    let mut error_count = 0;

    for (i, todo) in todos.iter().filter(|t| missing.contains(&t.id)).enumerate() {
        // Progress indicator
        if (i + 1) % 10 == 0 || i + 1 == missing.len() {
            eprint!("\r  [{}/{}]", i + 1, missing.len());
        }

        match handler.embed_todo(todo).await {
            Ok(Some(_)) => success_count += 1,
            Ok(None) => {
                error_count += 1;
                eprintln!(
                    "\n  {} Skipped {} (embedding returned None)",
                    colorize("!", WARNING),
                    todo.id,
                );
            }
            Err(e) => {
                error_count += 1;
                eprintln!("\n  {} Failed {} — {}", colorize("✗", ERROR), todo.id, e,);
            }
        }
    }

    eprintln!(); // Clear progress line
    println!();
    println!(
        "{} Backfill complete: {} embedded, {} errors.",
        colorize("✓", SUCCESS),
        success_count,
        error_count,
    );

    if error_count == 0 {
        println!(
            "All tasks now have embeddings. Semantic search is ready: {}",
            colorize("klyntbot todo search --semantic <query>", BOLD),
        );
    }

    Ok(())
}
