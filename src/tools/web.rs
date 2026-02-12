//! Web tools: search and fetch.

use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;
use tracing::{debug, warn};
use url::Url;

use super::Tool;
use crate::error::{Result, ToolError};

/// Tool for web search via Brave Search API
pub struct WebSearchTool {
    api_key: Option<String>,
    client: Client,
    max_results: u8,
}

impl WebSearchTool {
    pub fn new(api_key: Option<String>, max_results: u8) -> Self {
        Self {
            api_key,
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap(),
            max_results,
        }
    }
}

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Search the web using Brave Search API. Returns titles, URLs, and snippets."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query"
                },
                "count": {
                    "type": "integer",
                    "description": "Number of results (1-10)",
                    "minimum": 1,
                    "maximum": 10
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, args: Value) -> Result<String> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("missing 'query' parameter".to_string()))?;

        let count = args
            .get("count")
            .and_then(|v| v.as_i64())
            .unwrap_or(self.max_results as i64)
            .clamp(1, 10);

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
            )).into());
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

/// Tool for fetching web content
pub struct WebFetchTool {
    client: Client,
}

impl WebFetchTool {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .redirect(reqwest::redirect::Policy::limited(5))
                .build()
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
impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "web_fetch"
    }

    fn description(&self) -> &str {
        "Fetch URL and extract readable content (HTML -> text/markdown)."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "URL to fetch"
                },
                "extract_mode": {
                    "type": "string",
                    "enum": ["markdown", "text"],
                    "description": "Extraction mode (default: markdown)"
                },
                "max_chars": {
                    "type": "integer",
                    "description": "Maximum characters to return",
                    "minimum": 100
                }
            },
            "required": ["url"]
        })
    }

    async fn execute(&self, args: Value) -> Result<String> {
        let url_str = args
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("missing 'url' parameter".to_string()))?;

        let max_chars = args
            .get("max_chars")
            .and_then(|v| v.as_i64())
            .unwrap_or(50000) as usize;

        debug!("Fetching URL: {}", url_str);

        // Validate URL
        let url = Url::parse(url_str)
            .map_err(|e| ToolError::ExecutionFailed(format!("Invalid URL: {}", e)))?;

        if url.scheme() != "http" && url.scheme() != "https" {
            return Err(ToolError::ExecutionFailed("Only http and https URLs are supported".to_string()).into());
        }

        if url.host_str().is_none() {
            return Err(ToolError::ExecutionFailed("URL must have a valid domain".to_string()).into());
        }

        // Fetch content
        let response = self
            .client
            .get(url_str)
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

fn truncate_output(text: &str, max_chars: usize) -> String {
    if text.len() <= max_chars {
        text.to_string()
    } else {
        format!(
            "{}... (truncated, {} more chars)",
            &text[..max_chars],
            text.len() - max_chars
        )
    }
}
