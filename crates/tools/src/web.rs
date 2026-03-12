//! Web tools: search and fetch.

use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;
use tracing::{debug, warn};
use url::Url;

use common::{build_http_client_with_builder, Result, ToolError};
use tools_core::{RoutingContext, ToolParams};

#[derive(Debug, ToolParams)]
pub struct WebSearchParams {
    /// Search query
    #[param(required)]
    pub query: String,

    /// Number of results (1-10)
    #[param(min = 1, max = 10)]
    pub count: Option<i64>,
}

/// Tool for web search via Brave Search API
#[derive(tools_core::Tool)]
#[tool(
    name = "web_search",
    description = "Search the web using Brave Search API. Returns titles, URLs, and snippets.",
    params = "WebSearchParams",
    category = "Web",
    tags = "search,web,internet",
    cost = "Low"
)]
pub struct WebSearchTool {
    api_key: Option<String>,
    client: Client,
    max_results: u8,
}

impl WebSearchTool {
    pub fn new(api_key: Option<String>, max_results: u8) -> Self {
        Self {
            api_key,
            client: build_http_client_with_builder(|builder| {
                builder.timeout(Duration::from_secs(30))
            })
            .unwrap(),
            max_results,
        }
    }
}

#[async_trait]
impl tools_core::ToolExecute for WebSearchTool {
    type Params = WebSearchParams;

    async fn execute(&self, params: WebSearchParams, _ctx: &RoutingContext) -> Result<String> {
        let query = &params.query;
        let count = params.count.unwrap_or(self.max_results as i64).clamp(1, 10);

        let api_key = self.api_key.as_ref().ok_or_else(|| {
            ToolError::ExecutionFailed("Brave Search API key not configured".to_string())
        })?;

        debug!("Searching web: {}", query);

        // Build URL with query parameters
        let url = format!(
            "https://api.search.brave.com/res/v1/web/search?q={}&count={}",
            urlencoding::encode(query),
            count
        );

        let response = self
            .client
            .get(&url)
            .header("X-Subscription-Token", api_key)
            .send()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Search request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(ToolError::ExecutionFailed(format!(
                "Search API returned status {}",
                response.status()
            ))
            .into());
        }

        let data: Value = response.json().await.map_err(|e| {
            ToolError::ExecutionFailed(format!("Failed to parse search response: {}", e))
        })?;

        let results = data
            .get("web")
            .and_then(|w| w.get("results"))
            .and_then(|r| r.as_array())
            .ok_or_else(|| {
                ToolError::ExecutionFailed("Invalid search response format".to_string())
            })?;

        if results.is_empty() {
            return Ok(format!("No results found for query: {}", query));
        }

        let mut output = Vec::new();

        for (i, result) in results.iter().enumerate() {
            let title = result
                .get("title")
                .and_then(|t| t.as_str())
                .unwrap_or("(no title)");
            let url = result
                .get("url")
                .and_then(|u| u.as_str())
                .unwrap_or("(no url)");
            let description = result
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("(no description)");

            output.push(format!(
                "{}. {}\n   {}\n   {}",
                i + 1,
                title,
                url,
                description
            ));
        }

        Ok(output.join("\n\n"))
    }
}

#[derive(Debug, ToolParams)]
pub struct WebFetchParams {
    /// URL to fetch
    #[param(required)]
    pub url: String,

    /// Extraction mode (default: markdown)
    pub extract_mode: Option<String>,

    /// Maximum characters to return
    #[param(min = 100)]
    pub max_chars: Option<i64>,
}

/// Tool for fetching web content
#[derive(tools_core::Tool)]
#[tool(
    name = "web_fetch",
    description = "Fetch URL and extract readable content (HTML -> text/markdown).",
    params = "WebFetchParams",
    category = "Web",
    tags = "web,fetch,url,scrape",
    cost = "Low"
)]
pub struct WebFetchTool {
    client: Client,
}

impl WebFetchTool {
    pub fn new() -> Self {
        Self {
            client: build_http_client_with_builder(|builder| {
                builder
                    .timeout(Duration::from_secs(30))
                    .redirect(reqwest::redirect::Policy::limited(5))
            })
            .unwrap(),
        }
    }
}

impl Default for WebFetchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl tools_core::ToolExecute for WebFetchTool {
    type Params = WebFetchParams;

    async fn execute(&self, params: WebFetchParams, _ctx: &RoutingContext) -> Result<String> {
        let url_str = &params.url;
        let max_chars = params.max_chars.unwrap_or(50000) as usize;

        debug!("Fetching URL: {}", url_str);

        // Validate URL
        let url = Url::parse(url_str)
            .map_err(|e| ToolError::ExecutionFailed(format!("Invalid URL: {}", e)))?;

        if url.scheme() != "http" && url.scheme() != "https" {
            return Err(ToolError::ExecutionFailed(
                "Only http and https URLs are supported".to_string(),
            )
            .into());
        }

        if url.host_str().is_none() {
            return Err(
                ToolError::ExecutionFailed("URL must have a valid domain".to_string()).into(),
            );
        }

        // Fetch content
        let response = self
            .client
            .get(url_str.as_str())
            .send()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to fetch URL: {}", e)))?;

        if !response.status().is_success() {
            return Err(ToolError::ExecutionFailed(format!("HTTP {}", response.status())).into());
        }

        let _final_url = response.url().to_string();
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_lowercase();

        let body = response.text().await.map_err(|e| {
            ToolError::ExecutionFailed(format!("Failed to read response body: {}", e))
        })?;

        // Handle JSON
        if content_type.contains("application/json") {
            match serde_json::from_str::<Value>(&body) {
                Ok(json) => {
                    let formatted = serde_json::to_string_pretty(&json).unwrap_or(body);
                    return Ok(truncate_output(&formatted, max_chars));
                }
                Err(_) => {
                    warn!("Content-Type is JSON but failed to parse");
                }
            }
        }

        // Handle HTML
        if content_type.contains("text/html")
            || body.trim_start().starts_with("<!DOCTYPE html")
            || body.trim_start().starts_with("<html")
        {
            match html2text::from_read(body.as_bytes(), 80) {
                Ok(text) => return Ok(truncate_output(&text, max_chars)),
                Err(_) => {
                    // Fall through to plain text
                }
            }
        }

        // Plain text or other
        Ok(truncate_output(&body, max_chars))
    }
}

fn truncate_output(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        text.to_string()
    } else {
        let truncated = common::truncate_at_boundary(text, max_bytes);
        format!(
            "{}... (truncated, {} more bytes)",
            truncated,
            text.len() - truncated.len()
        )
    }
}
