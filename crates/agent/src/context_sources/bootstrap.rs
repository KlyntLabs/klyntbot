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

        for filename in BOOTSTRAP_FILES {
            let path = self.workspace.join(filename);
            if path.exists() {
                match fs::read_to_string(&path).await {
                    Ok(content) if !content.trim().is_empty() => {
                        debug!("Loaded bootstrap file: {}", filename);
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
        200
    }
}
