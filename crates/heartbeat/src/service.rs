//! Heartbeat service - periodic agent wake-up to check for tasks.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::{debug, error, info};

/// Default interval: 30 minutes
pub const DEFAULT_HEARTBEAT_INTERVAL_S: u64 = 30 * 60;

/// The prompt sent to agent during heartbeat
pub const HEARTBEAT_PROMPT: &str = r#"Read HEARTBEAT.md in your workspace (if it exists).
Follow any instructions or tasks listed there.
If nothing needs attention, reply with just: HEARTBEAT_OK"#;

/// Token that indicates "nothing to do"
pub const HEARTBEAT_OK_TOKEN: &str = "HEARTBEAT_OK";

/// Check if HEARTBEAT.md has no actionable content
fn is_heartbeat_empty(content: Option<&str>) -> bool {
    let Some(content) = content else {
        return true;
    };

    // Lines to skip: empty, headers, HTML comments, empty/completed checkboxes
    for line in content.lines() {
        let line = line.trim();

        // Skip empty lines
        if line.is_empty() {
            continue;
        }

        // Skip headers
        if line.starts_with('#') {
            continue;
        }

        // Skip HTML comments
        if line.starts_with("<!--") {
            continue;
        }

        // Skip checkboxes
        if line == "- [ ]" || line == "* [ ]" || line == "- [x]" || line == "* [x]" {
            continue;
        }

        // Found actionable content
        return false;
    }

    true
}

/// Callback type for heartbeat execution
pub type HeartbeatCallback =
    Arc<dyn Fn(&str) -> Result<String, Box<dyn std::error::Error>> + Send + Sync>;

/// Periodic heartbeat service that wakes the agent to check for tasks
///
/// The agent reads HEARTBEAT.md from the workspace and executes any
/// tasks listed there. If nothing needs attention, it replies HEARTBEAT_OK.
pub struct HeartbeatService {
    workspace: PathBuf,
    on_heartbeat: Option<HeartbeatCallback>,
    interval_s: u64,
    enabled: bool,
    running: Arc<RwLock<bool>>,
    task: Arc<RwLock<Option<JoinHandle<()>>>>,
}

impl HeartbeatService {
    /// Create a new heartbeat service
    pub fn new(workspace: impl Into<PathBuf>, interval_s: u64, enabled: bool) -> Self {
        Self {
            workspace: workspace.into(),
            on_heartbeat: None,
            interval_s,
            enabled,
            running: Arc::new(RwLock::new(false)),
            task: Arc::new(RwLock::new(None)),
        }
    }

    /// Set the heartbeat callback
    pub fn set_callback(&mut self, callback: HeartbeatCallback) {
        self.on_heartbeat = Some(callback);
    }

    /// Get the heartbeat file path
    pub fn heartbeat_file(&self) -> PathBuf {
        self.workspace.join("HEARTBEAT.md")
    }

    /// Read HEARTBEAT.md content
    async fn read_heartbeat_file(&self) -> Option<String> {
        let path = self.heartbeat_file();
        tokio::fs::read_to_string(&path).await.ok()
    }

    /// Start the heartbeat service
    pub async fn start(&self) {
        if !self.enabled {
            info!("Heartbeat disabled");
            return;
        }

        *self.running.write().await = true;

        let self_clone = self.clone_service();
        let task = tokio::spawn(async move {
            self_clone.run_loop().await;
        });

        *self.task.write().await = Some(task);

        info!("Heartbeat started (every {}s)", self.interval_s);
    }

    /// Stop the heartbeat service
    pub async fn stop(&self) {
        *self.running.write().await = false;

        let mut task = self.task.write().await;
        if let Some(task) = task.take() {
            task.abort();
        }
    }

    /// Clone service for async tasks (shared ownership)
    fn clone_service(&self) -> Self {
        Self {
            workspace: self.workspace.clone(),
            on_heartbeat: self.on_heartbeat.clone(),
            interval_s: self.interval_s,
            enabled: self.enabled,
            running: self.running.clone(),
            task: self.task.clone(),
        }
    }

    /// Main heartbeat loop
    async fn run_loop(&self) {
        while *self.running.read().await {
            tokio::time::sleep(Duration::from_secs(self.interval_s)).await;

            if *self.running.read().await {
                self.tick().await;
            }
        }
    }

    /// Execute a single heartbeat tick
    async fn tick(&self) {
        let content = self.read_heartbeat_file().await;

        // Skip if HEARTBEAT.md is empty or doesn't exist
        if is_heartbeat_empty(content.as_deref()) {
            debug!("Heartbeat: no tasks (HEARTBEAT.md empty)");
            return;
        }

        info!("Heartbeat: checking for tasks...");

        if let Some(callback) = &self.on_heartbeat {
            match callback(HEARTBEAT_PROMPT) {
                Ok(response) => {
                    // Check if agent said "nothing to do"
                    let response_upper = response.to_uppercase().replace('_', "");
                    let token_upper = HEARTBEAT_OK_TOKEN.replace('_', "");

                    if response_upper.contains(&token_upper) {
                        info!("Heartbeat: OK (no action needed)");
                    } else {
                        info!("Heartbeat: completed task");
                    }
                }
                Err(e) => {
                    error!("Heartbeat execution failed: {}", e);
                }
            }
        }
    }

    /// Manually trigger a heartbeat
    pub async fn trigger_now(&self) -> Option<String> {
        if let Some(callback) = &self.on_heartbeat {
            match callback(HEARTBEAT_PROMPT) {
                Ok(response) => Some(response),
                Err(e) => {
                    error!("Heartbeat trigger failed: {}", e);
                    None
                }
            }
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_heartbeat_empty() {
        // These should be considered empty (no actionable content)
        assert!(is_heartbeat_empty(None));
        assert!(is_heartbeat_empty(Some("")));
        assert!(is_heartbeat_empty(Some("# Header")));
        assert!(is_heartbeat_empty(Some("<!-- Comment -->\n\n# Title")));
        assert!(is_heartbeat_empty(Some("# Header\n\n- [ ]\n* [ ]"))); // Empty checkboxes only
        assert!(is_heartbeat_empty(Some("# Header\n\n- [x]"))); // Completed checkboxes

        // These should NOT be empty (have actionable content)
        assert!(!is_heartbeat_empty(Some("- [ ] Todo"))); // Checkbox with text
        assert!(!is_heartbeat_empty(Some("Do this task")));
        assert!(!is_heartbeat_empty(Some("# Header\n\nActual content")));
        assert!(!is_heartbeat_empty(Some("# Header\n\n- [ ] Do something"))); // Checkbox with task
    }
}
