//! Task search command — keyword, semantic, and hybrid search.

use anyhow::Result;
use common::utils::terminal::*;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;
use tools::todo::TodoTool;
use tools::todo_store::TodoStore;
use tools::todo_types::{Todo, TodoFilter};
use tools::{RoutingContext, Tool};

use super::render_priority;

pub async fn handle_search(query: String, include_attachments: bool) -> Result<()> {
    let config = config::load().await?;
    let store_path = config.todo_store_path();
    let mut store = TodoStore::new(store_path);

    let todos = store.list(&TodoFilter::default()).await?;
    let query_lower = query.to_lowercase();

    let results: Vec<(&Todo, MatchLocation)> = todos
        .iter()
        .filter_map(|t| find_match(t, &query_lower, include_attachments))
        .collect();

    if results.is_empty() {
        println!("No tasks found matching '{}'.", query);
        return Ok(());
    }

    println!(
        "{} task(s) matching '{}':",
        results.len(),
        colorize(&query, BOLD),
    );
    println!();

    for (todo, match_loc) in &results {
        print!("{} ", colorize(&todo.id, TOOL));

        // Status indicator
        match todo.status {
            tools::todo_types::TodoStatus::Done => print!("{} ", colorize("✓", SUCCESS)),
            tools::todo_types::TodoStatus::Doing => print!("{} ", colorize("●", BRAND)),
            _ => print!("  "),
        }

        print!("{}", colorize(&todo.title, BOLD));

        if let Some(pri) = todo.priority {
            print!("  {}", render_priority(pri));
        }

        println!();

        // Show match context
        match match_loc {
            MatchLocation::Title => {}
            MatchLocation::Description(excerpt) => {
                println!("     {} {}", colorize("in description:", DIM), excerpt,);
            }
            MatchLocation::Attachment(title) => {
                println!("     {} {}", colorize("in attachment:", DIM), title,);
            }
        }
    }

    Ok(())
}

enum MatchLocation {
    Title,
    Description(String),
    Attachment(String),
}

fn find_match<'a>(
    todo: &'a Todo,
    query: &str,
    include_attachments: bool,
) -> Option<(&'a Todo, MatchLocation)> {
    // Search title
    if todo.title.to_lowercase().contains(query) {
        return Some((todo, MatchLocation::Title));
    }

    // Search description
    if let Some(desc) = &todo.description {
        if desc.to_lowercase().contains(query) {
            let excerpt = excerpt_around(desc, query, 40);
            return Some((todo, MatchLocation::Description(excerpt)));
        }
    }

    // Search attachments
    if include_attachments {
        for att in &todo.attachments {
            if att.value.to_lowercase().contains(query) {
                let label = att
                    .title
                    .clone()
                    .unwrap_or_else(|| att.value.chars().take(40).collect());
                return Some((todo, MatchLocation::Attachment(label)));
            }
            if let Some(title) = &att.title {
                if title.to_lowercase().contains(query) {
                    return Some((todo, MatchLocation::Attachment(title.clone())));
                }
            }
        }
    }

    None
}

/// Handle `--semantic` flag: route to TodoTool's search_semantic action.
pub async fn handle_semantic_search(
    query: String,
    threshold: Option<f64>,
    limit: Option<u64>,
) -> Result<()> {
    let config = config::load().await?;
    let store_path = config.todo_store_path();
    let emb_store_path = config.embedding_store_path();

    let store = Arc::new(RwLock::new(TodoStore::new(store_path)));
    let emb_store = Arc::new(RwLock::new(tools::EmbeddingStore::new(emb_store_path)));

    // Create embedding engine + handler
    let engine = Arc::new(tools::EmbeddingEngine::new());
    let handler = Arc::new(tools::EmbeddingEngineImpl::new(engine, emb_store.clone()));

    let tool = TodoTool::new(
        store,
        config.todo.focus.max_slots,
        config.todo.focus.deadline_hours,
        config.timezone.clone(),
    )
    .with_embedding_handler(handler)
    .with_embedding_store(emb_store)
    .with_search_config(
        config.todo.search.semantic_threshold,
        config.todo.search.rrf_k,
    );

    let mut args = json!({
        "action": "search_semantic",
        "query": query,
    });

    if let Some(t) = threshold {
        args["threshold"] = json!(t);
    }
    if let Some(l) = limit {
        args["limit"] = json!(l);
    }

    let ctx = RoutingContext::new(
        common::ChannelName::new("cli"),
        common::ChatId::new("search"),
    );

    match tool.execute(args, &ctx).await {
        Ok(output) => println!("{}", output),
        Err(e) => eprintln!("Error: {}", e),
    }

    Ok(())
}

/// Handle `--hybrid` flag: route to TodoTool's search_hybrid action.
pub async fn handle_hybrid_search(query: String, limit: Option<u64>) -> Result<()> {
    let config = config::load().await?;
    let store_path = config.todo_store_path();
    let emb_store_path = config.embedding_store_path();

    let store = Arc::new(RwLock::new(TodoStore::new(store_path)));
    let emb_store = Arc::new(RwLock::new(tools::EmbeddingStore::new(emb_store_path)));

    let engine = Arc::new(tools::EmbeddingEngine::new());
    let handler = Arc::new(tools::EmbeddingEngineImpl::new(engine, emb_store.clone()));

    let tool = TodoTool::new(
        store,
        config.todo.focus.max_slots,
        config.todo.focus.deadline_hours,
        config.timezone.clone(),
    )
    .with_embedding_handler(handler)
    .with_embedding_store(emb_store)
    .with_search_config(
        config.todo.search.semantic_threshold,
        config.todo.search.rrf_k,
    );

    let mut args = json!({
        "action": "search_hybrid",
        "query": query,
    });

    if let Some(l) = limit {
        args["limit"] = json!(l);
    }

    let ctx = RoutingContext::new(
        common::ChannelName::new("cli"),
        common::ChatId::new("search"),
    );

    match tool.execute(args, &ctx).await {
        Ok(output) => println!("{}", output),
        Err(e) => eprintln!("Error: {}", e),
    }

    Ok(())
}

/// Extract a short excerpt around the first occurrence of `query` in `text`.
fn excerpt_around(text: &str, query: &str, context_chars: usize) -> String {
    let lower = text.to_lowercase();
    let Some(pos) = lower.find(query) else {
        return text.chars().take(context_chars * 2).collect();
    };

    let start = pos.saturating_sub(context_chars);
    let end = (pos + query.len() + context_chars).min(text.len());

    // Align to char boundaries
    let start = text
        .char_indices()
        .map(|(i, _)| i)
        .find(|&i| i >= start)
        .unwrap_or(0);
    let end = text
        .char_indices()
        .map(|(i, _)| i)
        .rfind(|&i| i <= end)
        .unwrap_or(text.len());

    let mut excerpt = String::new();
    if start > 0 {
        excerpt.push('…');
    }
    excerpt.push_str(&text[start..end]);
    if end < text.len() {
        excerpt.push('…');
    }
    excerpt
}
