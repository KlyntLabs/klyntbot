//! Bootstrap context source — loads AGENTS.md, SOUL.md, USER.md, etc.

use std::path::PathBuf;

use async_trait::async_trait;
use context_engine::source::{ContextSource, SourceContext};
use tokio::fs;
use tokio::sync::OnceCell;
use tracing::{debug, warn};

/// Bootstrap file names loaded from the workspace directory.
const BOOTSTRAP_FILES: &[&str] = &[
    "AGENTS.md",
    "SOUL.md",
    "USER.md",
    "TOOLS.md",
    "IDENTITY.md",
    "RESPONSE.md",
    "HEARTBEAT.md",
];

/// Maximum estimated tokens allowed per bootstrap file.
const MAX_BOOTSTRAP_TOKENS_PER_FILE: usize = 2000;

/// Maximum estimated tokens allowed across all bootstrap files combined.
const MAX_BOOTSTRAP_TOKENS_TOTAL: usize = 8000;

/// Truncate content to fit within a token limit (approx 4 chars per token).
///
/// Tries to break on a whitespace boundary to avoid splitting words.
fn truncate_to_token_limit(content: &str, max_tokens: usize) -> &str {
    let max_chars = max_tokens * 4;
    if content.len() <= max_chars {
        return content;
    }
    let truncated = &content[..max_chars];
    match truncated.rfind(char::is_whitespace) {
        Some(pos) => &content[..pos],
        None => truncated,
    }
}

/// Provides bootstrap file content (cached after first load).
///
/// Reads markdown files from the workspace directory and caches them
/// permanently (they don't change at runtime).
pub struct BootstrapSource {
    workspace: PathBuf,
    cached: OnceCell<String>,
}

impl BootstrapSource {
    pub fn new(workspace: PathBuf) -> Self {
        Self {
            workspace,
            cached: OnceCell::new(),
        }
    }

    async fn load_bootstrap(&self) -> String {
        let mut sections = Vec::with_capacity(BOOTSTRAP_FILES.len());
        let mut total_tokens: usize = 0;

        for filename in BOOTSTRAP_FILES {
            let path = self.workspace.join(filename);
            if path.exists() {
                match fs::read_to_string(&path).await {
                    Ok(content) if !content.trim().is_empty() => {
                        let file_tokens = content.len().div_ceil(4);

                        // Check if adding this file would exceed the total limit
                        if total_tokens >= MAX_BOOTSTRAP_TOKENS_TOTAL {
                            warn!(
                                "Skipping bootstrap file {} — total token budget exhausted ({}/{})",
                                filename, total_tokens, MAX_BOOTSTRAP_TOKENS_TOTAL
                            );
                            continue;
                        }

                        let content = if file_tokens > MAX_BOOTSTRAP_TOKENS_PER_FILE {
                            warn!(
                                "Bootstrap file {} exceeds per-file limit (~{} tokens, max {}), truncating",
                                filename, file_tokens, MAX_BOOTSTRAP_TOKENS_PER_FILE
                            );
                            let truncated =
                                truncate_to_token_limit(&content, MAX_BOOTSTRAP_TOKENS_PER_FILE);
                            truncated.to_owned()
                        } else {
                            content
                        };

                        let used_tokens = content.len().div_ceil(4);
                        total_tokens += used_tokens;
                        debug!(
                            "Loaded bootstrap file: {} (~{} tokens, total ~{})",
                            filename, used_tokens, total_tokens
                        );
                        sections.push(content);
                    }
                    Ok(_) => {}
                    Err(e) => {
                        warn!("Failed to read {}: {}", filename, e);
                    }
                }
            }
        }

        sections.join("\n\n---\n\n")
    }
}

#[async_trait]
impl ContextSource for BootstrapSource {
    fn name(&self) -> &str {
        "bootstrap"
    }

    fn priority(&self) -> u8 {
        90
    }

    async fn provide(&self, _ctx: &SourceContext) -> Option<String> {
        let content = self
            .cached
            .get_or_init(|| async { self.load_bootstrap().await })
            .await;

        if content.is_empty() {
            None
        } else {
            Some(content.clone())
        }
    }

    fn estimated_tokens(&self) -> usize {
        MAX_BOOTSTRAP_TOKENS_TOTAL
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_to_token_limit_short() {
        let short = "Hello world, this is a short file.";
        let result = truncate_to_token_limit(short, 2000);
        assert_eq!(result, short);
    }

    #[test]
    fn test_truncate_to_token_limit_long() {
        let long = "word ".repeat(3000); // ~3000 tokens
        let result = truncate_to_token_limit(&long, 2000);
        assert!(result.len() < long.len());
        assert!(result.len() <= 2000 * 4 + 10);
    }

    #[test]
    fn test_truncate_empty() {
        let result = truncate_to_token_limit("", 2000);
        assert_eq!(result, "");
    }
}
