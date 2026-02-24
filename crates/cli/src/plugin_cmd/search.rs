//! `klyntbot plugin search` — query the plugin registry.

use anyhow::{Context, Result};

/// Search the plugin registry for plugins matching a query.
pub async fn run(query: &str, registry_url: &str) -> Result<()> {
    let client = reqwest::Client::new();
    let index: serde_json::Value = client
        .get(registry_url)
        .header("User-Agent", "klyntbot")
        .send()
        .await?
        .error_for_status()
        .context("failed to fetch plugin registry")?
        .json()
        .await?;

    let plugins = index["plugins"]
        .as_array()
        .context("registry has no plugins array")?;

    let query_lower = query.to_lowercase();
    let matches: Vec<_> = plugins
        .iter()
        .filter(|p| {
            let id = p["id"].as_str().unwrap_or("");
            let name = p["name"].as_str().unwrap_or("");
            let desc = p["description"].as_str().unwrap_or("");
            id.to_lowercase().contains(&query_lower)
                || name.to_lowercase().contains(&query_lower)
                || desc.to_lowercase().contains(&query_lower)
        })
        .collect();

    if matches.is_empty() {
        println!("No plugins found matching '{query}'.");
        return Ok(());
    }

    println!("{:<24} {:<10} DESCRIPTION", "ID", "VERSION");
    println!("{}", "-".repeat(60));

    for plugin in &matches {
        println!(
            "{:<24} {:<10} {}",
            plugin["id"].as_str().unwrap_or("?"),
            plugin["latest_version"].as_str().unwrap_or("?"),
            plugin["description"].as_str().unwrap_or(""),
        );
    }

    println!("\n{} result(s).", matches.len());
    Ok(())
}
