//! Browser automation tool using the agent-browser CLI.

use async_trait::async_trait;
use config::TrustLevel;
use serde_json::Value;
use tracing::{debug, warn};

use super::{PermissionLevel, RoutingContext, Tool};
use crate::params::ParamExtractor;
use common::{Result, ToolError};

// ── Write-action detection ────────────────────────────────────────────────────

/// Returns `true` if this action + element label combination is a write action
/// that should be guarded in Autonomous (or Strict) mode.
pub fn is_write_action(action: &str, element_label: &str) -> bool {
    // submit_and_confirm is always a write action
    if action == "submit_and_confirm" {
        return true;
    }

    let label = element_label.to_lowercase();

    // Dangerous click targets
    if action == "click" {
        let dangerous = [
            "submit", "checkout", "buy", "purchase", "confirm",
            "place order", "delete", "remove", "send", "pay",
        ];
        if dangerous.iter().any(|k| label.contains(k)) {
            return true;
        }
    }

    // Payment field fills
    if action == "fill" || action == "type" {
        let payment = ["card number", "cvv", "cvc", "expiry", "billing"];
        if payment.iter().any(|k| label.contains(k)) {
            return true;
        }
    }

    false
}

// ── SnapshotElement ───────────────────────────────────────────────────────────

/// A single element reference parsed from a snapshot.
#[derive(Debug)]
pub struct SnapshotElement {
    pub ref_id: String,
    pub kind:   String,
    pub label:  String,
}

// ── BrowserTool ───────────────────────────────────────────────────────────────

/// Tool for browser automation via the agent-browser CLI.
pub struct BrowserTool {
    trust_level: TrustLevel,
    /// Path to the agent-browser binary.
    binary_path: String,
}

impl BrowserTool {
    /// Construct a `BrowserTool`, checking that the `agent-browser` binary is
    /// available on `PATH`. Returns an error if not found.
    pub fn new(trust_level: TrustLevel) -> Result<Self> {
        let binary_path = find_agent_browser()?;
        Ok(Self { trust_level, binary_path })
    }

    /// Construct without binary check — for tests only.
    #[cfg(test)]
    pub fn new_unchecked(trust_level: TrustLevel) -> Self {
        Self {
            trust_level,
            binary_path: "agent-browser".to_string(),
        }
    }

    /// Returns `true` if this action should be blocked pending user confirmation.
    pub fn should_guard(&self, action: &str, element_label: &str) -> bool {
        match self.trust_level {
            TrustLevel::Full => false,
            TrustLevel::Autonomous => is_write_action(action, element_label),
            TrustLevel::Strict => {
                // In strict mode: guard every click, fill, type, and submit
                matches!(action, "click" | "fill" | "type" | "submit_and_confirm")
            }
        }
    }

    /// Map action name to agent-browser CLI arguments.
    pub fn build_args(action: &str, extra: &[&str]) -> Vec<String> {
        let cmd = match action {
            "navigate" => "open",
            other      => other,
        };
        let mut args = vec![cmd.to_string()];
        args.extend(extra.iter().map(|s| s.to_string()));
        args
    }

    /// Parse agent-browser snapshot output into structured element refs.
    /// Expected line format: `@e1 button "Search"`
    pub fn parse_snapshot(raw: &str) -> Vec<SnapshotElement> {
        raw.lines()
            .filter_map(|line| {
                let line = line.trim();
                if !line.starts_with('@') { return None; }
                let mut parts = line.splitn(3, ' ');
                let ref_id = parts.next()?.to_string();
                let kind   = parts.next().unwrap_or("").to_string();
                let label  = parts.next()
                    .unwrap_or("")
                    .trim_matches('"')
                    .to_string();
                Some(SnapshotElement { ref_id, kind, label })
            })
            .collect()
    }

    /// Format the message returned to the LLM when a write action is guarded.
    pub fn guard_message(action: &str, label: &str) -> String {
        format!(
            "[CONFIRMATION_REQUIRED] About to {} '{}'. \
             Use the ask_user tool to confirm with the user, \
             then call browser with the same action once confirmed.",
            action, label
        )
    }

    /// Execute an agent-browser subprocess command with a 30-second timeout.
    async fn run(&self, args: &[String]) -> Result<String> {
        debug!("agent-browser {:?}", args);

        let output = tokio::time::timeout(
            tokio::time::Duration::from_secs(30),
            tokio::process::Command::new(&self.binary_path).args(args).output(),
        )
        .await
        .map_err(|_| ToolError::ExecutionFailed(
            "Browser action timed out after 30s".to_string()
        ))?
        .map_err(|e| {
            warn!("agent-browser connection error: {}", e);
            ToolError::ExecutionFailed(
                "Browser session unavailable. Try navigating again.".to_string()
            )
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let msg = if stderr.is_empty() {
                format!("agent-browser exited with status {}", output.status)
            } else {
                stderr
            };
            return Err(ToolError::ExecutionFailed(msg).into());
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}

/// Locate the agent-browser binary. Returns its path or a helpful error.
fn find_agent_browser() -> Result<String> {
    let which_cmd = if cfg!(target_os = "windows") { "where" } else { "which" };

    if let Ok(output) = std::process::Command::new(which_cmd)
        .arg("agent-browser")
        .output()
    {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Ok(path.lines().next().unwrap_or("agent-browser").to_string());
            }
        }
    }

    Err(ToolError::ExecutionFailed(
        "Browser tool requires agent-browser. Run: klyntbot init --packs".to_string(),
    )
    .into())
}

#[async_trait]
impl Tool for BrowserTool {
    fn name(&self) -> &str {
        "browser"
    }

    fn description(&self) -> &str {
        "Control a web browser to navigate pages, interact with elements, fill forms, \
         and complete real-world tasks (booking, shopping, account management). \
         Always call snapshot before interacting with elements to get @e references."
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Elevated
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": [
                        "navigate", "snapshot", "click", "type", "fill",
                        "press", "scroll", "wait", "get_text", "screenshot", "eval",
                        "fill_form", "login_flow", "submit_and_confirm"
                    ],
                    "description": "Browser action to perform"
                },
                "url": {
                    "type": "string",
                    "description": "URL to navigate to (navigate, login_flow)"
                },
                "element": {
                    "type": "string",
                    "description": "Element reference (@e1) or text label (click, type, fill, get_text, submit_and_confirm)"
                },
                "text": {
                    "type": "string",
                    "description": "Text to type (type action)"
                },
                "value": {
                    "type": "string",
                    "description": "Value to fill into a field (fill action)"
                },
                "key": {
                    "type": "string",
                    "description": "Keyboard key to press, e.g. Enter, Tab, Escape (press action)"
                },
                "direction": {
                    "type": "string",
                    "enum": ["up", "down", "left", "right"],
                    "description": "Scroll direction (scroll action)"
                },
                "amount": {
                    "type": "integer",
                    "description": "Pixels to scroll (scroll action, optional)"
                },
                "condition": {
                    "type": "string",
                    "description": "Wait condition: element selector, text, or URL fragment (wait action)"
                },
                "script": {
                    "type": "string",
                    "description": "JavaScript expression to evaluate (eval action)"
                },
                "fields": {
                    "type": "object",
                    "description": "Field label → value map for fill_form, e.g. {\"Email\": \"user@example.com\"}"
                },
                "username": {
                    "type": "string",
                    "description": "Username or email for login_flow"
                },
                "password": {
                    "type": "string",
                    "description": "Password for login_flow"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value, _ctx: &RoutingContext) -> Result<String> {
        let p = ParamExtractor::new(&args);
        let action = p.required_str("action")?;

        debug!("browser action: {}", action);

        match action {
            "navigate"           => self.act_navigate(&p).await,
            "snapshot"           => self.act_snapshot().await,
            "click"              => self.act_click(&p).await,
            "type"               => self.act_type(&p).await,
            "fill"               => self.act_fill(&p).await,
            "press"              => self.act_press(&p).await,
            "scroll"             => self.act_scroll(&p).await,
            "wait"               => self.act_wait(&p).await,
            "get_text"           => self.act_get_text(&p).await,
            "screenshot"         => self.act_screenshot().await,
            "eval"               => self.act_eval(&p).await,
            "fill_form"          => self.act_fill_form(&p).await,
            "login_flow"         => self.act_login_flow(&p).await,
            "submit_and_confirm" => self.act_submit_and_confirm(&p).await,
            unknown => Err(ToolError::InvalidParams(
                format!("Unknown browser action: {}", unknown)
            ).into()),
        }
    }
}

// ── Primitive action implementations ─────────────────────────────────────────

impl BrowserTool {
    async fn act_navigate(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let url = p.required_str("url")?;
        let args = Self::build_args("navigate", &[url]);
        self.run(&args).await
    }

    async fn act_snapshot(&self) -> Result<String> {
        self.run(&["snapshot".to_string()]).await
    }

    async fn act_click(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let element = p.required_str("element")?;
        if self.should_guard("click", element) {
            return Ok(Self::guard_message("click", element));
        }
        let args = Self::build_args("click", &[element]);
        self.run(&args).await
    }

    async fn act_type(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let element = p.required_str("element")?;
        let text    = p.required_str("text")?;
        if self.should_guard("type", element) {
            return Ok(Self::guard_message("type", element));
        }
        let args = Self::build_args("type", &[element, text]);
        self.run(&args).await
    }

    async fn act_fill(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let element = p.required_str("element")?;
        let value   = p.required_str("value")?;
        if self.should_guard("fill", element) {
            return Ok(Self::guard_message("fill", element));
        }
        let args = Self::build_args("fill", &[element, value]);
        self.run(&args).await
    }

    async fn act_press(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let key  = p.required_str("key")?;
        let args = Self::build_args("press", &[key]);
        self.run(&args).await
    }

    async fn act_scroll(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let direction = p.required_str("direction")?;
        let amount = p.i64_or("amount", 0)?;
        let args = if amount > 0 {
            let amount_str = amount.to_string();
            Self::build_args("scroll", &[direction, &amount_str])
        } else {
            Self::build_args("scroll", &[direction])
        };
        self.run(&args).await
    }

    async fn act_wait(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let condition = p.required_str("condition")?;
        let args = Self::build_args("wait", &[condition]);
        self.run(&args).await
    }

    async fn act_get_text(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let element = p.required_str("element")?;
        // agent-browser: `get text @e1`
        let args = vec!["get".to_string(), "text".to_string(), element.to_string()];
        self.run(&args).await
    }

    async fn act_screenshot(&self) -> Result<String> {
        self.run(&["screenshot".to_string()]).await
    }

    async fn act_eval(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let script = p.required_str("script")?;
        let args = Self::build_args("eval", &[script]);
        self.run(&args).await
    }

    // ── Composite stubs (implemented in Task 5) ───────────────────────────────

    async fn act_fill_form(&self, _p: &ParamExtractor<'_>) -> Result<String> {
        Err(ToolError::ExecutionFailed("not yet implemented".into()).into())
    }
    async fn act_login_flow(&self, _p: &ParamExtractor<'_>) -> Result<String> {
        Err(ToolError::ExecutionFailed("not yet implemented".into()).into())
    }
    async fn act_submit_and_confirm(&self, _p: &ParamExtractor<'_>) -> Result<String> {
        Err(ToolError::ExecutionFailed("not yet implemented".into()).into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use config::TrustLevel;

    // ── TrustLevel (imported from config) ─────────────────────────────────────

    #[test]
    fn test_trust_level_autonomous_is_default() {
        assert_eq!(TrustLevel::default(), TrustLevel::Autonomous);
    }

    // ── Write-action guard ────────────────────────────────────────────────────

    #[test]
    fn test_submit_and_confirm_always_write() {
        assert!(is_write_action("submit_and_confirm", ""));
        assert!(is_write_action("submit_and_confirm", "Anything"));
    }

    #[test]
    fn test_click_dangerous_labels() {
        assert!(is_write_action("click", "Place Order"));
        assert!(is_write_action("click", "Checkout Now"));
        assert!(is_write_action("click", "Buy Now"));
        assert!(is_write_action("click", "Confirm Purchase"));
        assert!(is_write_action("click", "Delete Account"));
        assert!(is_write_action("click", "Send Message"));
        assert!(is_write_action("click", "Pay $49.99"));
    }

    #[test]
    fn test_click_safe_labels_not_write() {
        assert!(!is_write_action("click", "Search"));
        assert!(!is_write_action("click", "Next"));
        assert!(!is_write_action("click", "View Cart"));
        assert!(!is_write_action("click", "Add to Cart"));
        assert!(!is_write_action("click", "Learn More"));
    }

    #[test]
    fn test_fill_payment_fields_write() {
        assert!(is_write_action("fill", "Card Number"));
        assert!(is_write_action("fill", "CVV"));
        assert!(is_write_action("fill", "Expiry Date"));
        assert!(is_write_action("fill", "Billing Address"));
    }

    #[test]
    fn test_fill_regular_fields_not_write() {
        assert!(!is_write_action("fill", "Email"));
        assert!(!is_write_action("fill", "Username"));
        assert!(!is_write_action("fill", "Search"));
        assert!(!is_write_action("fill", "City"));
    }

    #[test]
    fn test_navigate_snapshot_never_write() {
        assert!(!is_write_action("navigate", ""));
        assert!(!is_write_action("snapshot", ""));
        assert!(!is_write_action("screenshot", ""));
        assert!(!is_write_action("get_text", ""));
    }

    #[test]
    fn test_trust_level_full_never_guards() {
        let tool = BrowserTool::new_unchecked(TrustLevel::Full);
        assert!(!tool.should_guard("click", "Place Order"));
        assert!(!tool.should_guard("submit_and_confirm", ""));
    }

    #[test]
    fn test_trust_level_autonomous_guards_dangerous() {
        let tool = BrowserTool::new_unchecked(TrustLevel::Autonomous);
        assert!(tool.should_guard("click", "Place Order"));
        assert!(!tool.should_guard("click", "Search"));
    }

    #[test]
    fn test_trust_level_strict_guards_all_writes() {
        let tool = BrowserTool::new_unchecked(TrustLevel::Strict);
        // Strict: guards any click/fill/type/submit
        assert!(tool.should_guard("click", "Search"));   // safe label but strict mode
        assert!(tool.should_guard("fill", "Email"));     // non-payment field but strict mode
    }

    #[test]
    fn test_trust_level_strict_does_not_guard_readonly() {
        let tool = BrowserTool::new_unchecked(TrustLevel::Strict);
        // Read-only actions not guarded even in strict
        assert!(!tool.should_guard("navigate", ""));
        assert!(!tool.should_guard("snapshot", ""));
        assert!(!tool.should_guard("screenshot", ""));
    }

    // ── Pure helper tests ─────────────────────────────────────────────────────

    #[test]
    fn test_build_args_navigate() {
        let args = BrowserTool::build_args("navigate", &["https://example.com"]);
        assert_eq!(args, vec!["open", "https://example.com"]);
    }

    #[test]
    fn test_build_args_click() {
        let args = BrowserTool::build_args("click", &["@e3"]);
        assert_eq!(args, vec!["click", "@e3"]);
    }

    #[test]
    fn test_build_args_type() {
        let args = BrowserTool::build_args("type", &["@e2", "hello world"]);
        assert_eq!(args, vec!["type", "@e2", "hello world"]);
    }

    #[test]
    fn test_parse_snapshot_finds_elements() {
        let raw = "@e1 button \"Search\"\n@e2 input \"Email\"\n@e3 button \"Submit\"";
        let elems = BrowserTool::parse_snapshot(raw);
        assert_eq!(elems.len(), 3);
        assert_eq!(elems[0].ref_id, "@e1");
        assert_eq!(elems[0].label, "Search");
        assert_eq!(elems[1].ref_id, "@e2");
        assert_eq!(elems[1].label, "Email");
    }

    #[test]
    fn test_parse_snapshot_empty() {
        let elems = BrowserTool::parse_snapshot("");
        assert!(elems.is_empty());
    }

    #[test]
    fn test_guard_message_format() {
        let msg = BrowserTool::guard_message("click", "Place Order");
        assert!(msg.contains("[CONFIRMATION_REQUIRED]"));
        assert!(msg.contains("Place Order"));
        assert!(msg.contains("ask_user"));
    }
}
