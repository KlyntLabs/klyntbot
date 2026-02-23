# Browser Automation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a `BrowserTool` to klyntbot that wraps the `agent-browser` CLI, enabling the agent to navigate pages, interact with elements, fill forms, and execute real-world tasks with a configurable trust/confirmation model.

**Architecture:** `BrowserTool` lives in `crates/tools/src/browser.rs` and calls `agent-browser` as a subprocess (same pattern as `ExecTool`). A `TrustLevel` enum controls write-action guarding. The tool is feature-gated behind `config.tools.browser.enabled` and registered in `AgentLoopBuilder`.

**Tech Stack:** `tokio::process::Command`, `serde_json`, `agent-browser` CLI (external binary), Cargo feature flag `browser-integration` for integration tests.

**Design doc:** `docs/plans/2026-02-23-browser-automation-design.md`

---

## Task 1: Add `BrowserConfig` to the config crate

**Files:**
- Modify: `crates/config/src/schema/tools.rs`

### Step 1: Add `BrowserConfig` struct and field to `ToolsConfig`

In `crates/config/src/schema/tools.rs`, add after the `ExecToolConfig` block:

```rust
/// Browser automation tool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserConfig {
    #[serde(default)]
    pub enabled: bool,

    #[serde(default = "default_trust_level")]
    pub trust_level: String,

    #[serde(default = "default_session_timeout_secs")]
    pub session_timeout_secs: u64,
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            trust_level: default_trust_level(),
            session_timeout_secs: default_session_timeout_secs(),
        }
    }
}

fn default_trust_level() -> String {
    "autonomous".to_string()
}

fn default_session_timeout_secs() -> u64 {
    300
}
```

Then add `browser` field to `ToolsConfig`:

```rust
pub struct ToolsConfig {
    #[serde(default)]
    pub web: WebToolsConfig,

    #[serde(default)]
    pub exec: ExecToolConfig,

    #[serde(default)]
    pub browser: BrowserConfig,      // ← add this

    #[serde(default)]
    pub restrict_to_workspace: bool,

    #[serde(default)]
    pub permissions: Option<PermissionsConfig>,
}
```

### Step 2: Build to verify

```bash
cargo build -p config
```
Expected: compiles with zero warnings.

### Step 3: Verify default serde roundtrip

Add to the bottom of `crates/config/src/schema/tools.rs` in the existing `#[cfg(test)]` block (or create one):

```rust
#[cfg(test)]
mod browser_config_tests {
    use super::*;

    #[test]
    fn test_browser_config_defaults() {
        let cfg = BrowserConfig::default();
        assert!(!cfg.enabled);
        assert_eq!(cfg.trust_level, "autonomous");
        assert_eq!(cfg.session_timeout_secs, 300);
    }

    #[test]
    fn test_browser_config_serde_roundtrip() {
        let json = r#"{"enabled": true, "trustLevel": "strict", "sessionTimeoutSecs": 60}"#;
        let cfg: BrowserConfig = serde_json::from_str(json).unwrap();
        assert!(cfg.enabled);
        assert_eq!(cfg.trust_level, "strict");
        assert_eq!(cfg.session_timeout_secs, 60);
    }

    #[test]
    fn test_tools_config_has_browser_field() {
        let cfg = ToolsConfig::default();
        assert!(!cfg.browser.enabled);
    }
}
```

### Step 4: Run tests

```bash
cargo nextest run -p config
```
Expected: all tests pass, zero warnings.

### Step 5: Commit

```bash
git add crates/config/src/schema/tools.rs
git commit -m "feat(config): add BrowserConfig to ToolsConfig"
```

---

## Task 2: `TrustLevel` enum and write-action guard

**Files:**
- Create: `crates/tools/src/browser.rs`
- Modify: `crates/tools/src/lib.rs`

### Step 1: Create `browser.rs` with `TrustLevel` and write guard — tests first

Create `crates/tools/src/browser.rs`:

```rust
//! Browser automation tool using the agent-browser CLI.

use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};
use tokio::process::Command;
use tracing::{debug, warn};

use super::{PermissionLevel, RoutingContext, Tool};
use crate::params::ParamExtractor;
use common::{Result, ToolError};

// ── Trust level ───────────────────────────────────────────────────────────────

/// Controls how write actions are guarded.
#[derive(Debug, Clone, PartialEq)]
pub enum TrustLevel {
    /// Ask the LLM to confirm via ask_user before every write action.
    Strict,
    /// (Default) Detect dangerous actions and block them until the LLM confirms.
    Autonomous,
    /// Execute all actions immediately without any confirmation gate.
    Full,
}

impl TrustLevel {
    pub fn from_str(s: &str) -> Self {
        match s {
            "strict" => Self::Strict,
            "full"   => Self::Full,
            _        => Self::Autonomous,
        }
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    // ── TrustLevel ────────────────────────────────────────────────────────────

    #[test]
    fn test_trust_level_from_str_strict() {
        assert_eq!(TrustLevel::from_str("strict"), TrustLevel::Strict);
    }

    #[test]
    fn test_trust_level_from_str_full() {
        assert_eq!(TrustLevel::from_str("full"), TrustLevel::Full);
    }

    #[test]
    fn test_trust_level_from_str_autonomous_default() {
        assert_eq!(TrustLevel::from_str("autonomous"), TrustLevel::Autonomous);
        assert_eq!(TrustLevel::from_str("anything_else"), TrustLevel::Autonomous);
        assert_eq!(TrustLevel::from_str(""), TrustLevel::Autonomous);
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
}
```

### Step 2: Run tests (expect pass — pure logic only)

```bash
cargo nextest run -p tools browser::tests
```
Expected: all 15 tests pass.

### Step 3: Expose `browser` module in `lib.rs`

In `crates/tools/src/lib.rs`, add after `pub mod web;`:

```rust
pub mod browser;
pub use browser::{BrowserTool, TrustLevel};
```

### Step 4: Build to verify

```bash
cargo build -p tools
```
Expected: compiles with zero warnings.

### Step 5: Commit

```bash
git add crates/tools/src/browser.rs crates/tools/src/lib.rs
git commit -m "feat(tools): add TrustLevel enum and write-action guard"
```

---

## Task 3: `BrowserTool` struct, binary check, and `Tool` trait skeleton

**Files:**
- Modify: `crates/tools/src/browser.rs`

### Step 1: Write tests for binary detection and tool construction

Add to `#[cfg(test)]` in `browser.rs`:

```rust
    #[test]
    fn test_trust_level_full_never_guards() {
        // Full trust: write actions are not blocked
        // (tested via BrowserTool::should_guard below)
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
        // Strict: also guards type/fill even on non-payment fields
        assert!(tool.should_guard("click", "Search")); // any click in strict
        assert!(tool.should_guard("fill", "Email"));   // any fill in strict
    }
```

### Step 2: Run tests (expect fail — `BrowserTool` not defined yet)

```bash
cargo nextest run -p tools browser::tests 2>&1 | head -20
```
Expected: compile error — `BrowserTool` not found.

### Step 3: Implement `BrowserTool` struct and constructors

Add above the `#[cfg(test)]` block in `browser.rs`:

```rust
// ── BrowserTool ───────────────────────────────────────────────────────────────

/// Tool for browser automation via the agent-browser CLI.
pub struct BrowserTool {
    trust_level: TrustLevel,
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
```

### Step 4: Add `Tool` trait skeleton (parameters + empty execute)

Add after the `BrowserTool` impl block:

```rust
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
            "navigate"          => self.act_navigate(&p).await,
            "snapshot"          => self.act_snapshot().await,
            "click"             => self.act_click(&p).await,
            "type"              => self.act_type(&p).await,
            "fill"              => self.act_fill(&p).await,
            "press"             => self.act_press(&p).await,
            "scroll"            => self.act_scroll(&p).await,
            "wait"              => self.act_wait(&p).await,
            "get_text"          => self.act_get_text(&p).await,
            "screenshot"        => self.act_screenshot().await,
            "eval"              => self.act_eval(&p).await,
            "fill_form"         => self.act_fill_form(&p).await,
            "login_flow"        => self.act_login_flow(&p).await,
            "submit_and_confirm" => self.act_submit_and_confirm(&p).await,
            unknown => Err(ToolError::InvalidParams(
                format!("Unknown browser action: {}", unknown)
            ).into()),
        }
    }
}
```

### Step 5: Run tests

```bash
cargo nextest run -p tools browser::tests
```
Expected: all tests pass.

### Step 6: Commit

```bash
git add crates/tools/src/browser.rs
git commit -m "feat(tools): add BrowserTool struct, binary check, Tool trait skeleton"
```

---

## Task 4: Core subprocess helper + primitive actions

**Files:**
- Modify: `crates/tools/src/browser.rs`

### Step 1: Write tests for argument construction (pure, no subprocess)

Add to `#[cfg(test)]` in `browser.rs`:

```rust
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
```

### Step 2: Run tests (expect fail)

```bash
cargo nextest run -p tools browser::tests 2>&1 | head -20
```
Expected: compile errors — methods not defined yet.

### Step 3: Implement helpers and all primitive actions

Add to `BrowserTool` impl block (before the `#[async_trait]` impl):

```rust
impl BrowserTool {
    // ... (existing new / new_unchecked / should_guard) ...

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

    /// A single element reference parsed from snapshot output.
    pub struct SnapshotElement {
        pub ref_id: String,
        pub kind:   String,
        pub label:  String,
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

        let output = timeout(
            Duration::from_secs(30),
            Command::new(&self.binary_path).args(args).output(),
        )
        .await
        .map_err(|_| ToolError::ExecutionFailed(
            "Browser action timed out after 30s".to_string()
        ))?
        .map_err(|e| {
            // Retry hint on connection failure
            warn!("agent-browser connection error: {}", e);
            ToolError::ExecutionFailed(
                "Browser session unavailable. Try navigating again.".to_string()
            )
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(ToolError::ExecutionFailed(stderr).into());
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    // ── Primitive action handlers ─────────────────────────────────────────────

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
        let mut cmd_args = vec![direction];
        let amount_str;
        if let Ok(amount) = p.i64_or("amount", 0) {
            if amount > 0 {
                amount_str = amount.to_string();
                cmd_args.push(&amount_str);
            }
        }
        let args = Self::build_args("scroll", &cmd_args);
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
}
```

**Note:** `SnapshotElement` is a struct defined inside the `impl` block. Move it outside the `impl` before the `BrowserTool` struct:

```rust
/// A single element reference parsed from a snapshot.
pub struct SnapshotElement {
    pub ref_id: String,
    pub kind:   String,
    pub label:  String,
}
```

### Step 4: Run tests

```bash
cargo nextest run -p tools browser::tests
```
Expected: all tests pass. Build:
```bash
cargo build -p tools
```
Expected: zero warnings.

### Step 5: Commit

```bash
git add crates/tools/src/browser.rs
git commit -m "feat(tools): add primitive browser actions (navigate, snapshot, click, etc.)"
```

---

## Task 5: Composite action helpers

**Files:**
- Modify: `crates/tools/src/browser.rs`

### Step 1: Write tests for composite helpers

Add to `#[cfg(test)]` in `browser.rs`:

```rust
    #[test]
    fn test_find_element_by_label_exact() {
        let elements = vec![
            SnapshotElement { ref_id: "@e1".into(), kind: "input".into(), label: "Email".into() },
            SnapshotElement { ref_id: "@e2".into(), kind: "input".into(), label: "Password".into() },
            SnapshotElement { ref_id: "@e3".into(), kind: "button".into(), label: "Sign In".into() },
        ];
        let found = BrowserTool::find_element_by_label(&elements, "Email");
        assert!(found.is_some());
        assert_eq!(found.unwrap().ref_id, "@e1");
    }

    #[test]
    fn test_find_element_by_label_case_insensitive() {
        let elements = vec![
            SnapshotElement { ref_id: "@e1".into(), kind: "input".into(), label: "Email Address".into() },
        ];
        let found = BrowserTool::find_element_by_label(&elements, "email address");
        assert!(found.is_some());
    }

    #[test]
    fn test_find_element_by_label_partial_match() {
        let elements = vec![
            SnapshotElement { ref_id: "@e1".into(), kind: "input".into(), label: "Card Number".into() },
        ];
        let found = BrowserTool::find_element_by_label(&elements, "card");
        assert!(found.is_some());
    }

    #[test]
    fn test_find_element_by_label_not_found() {
        let elements = vec![
            SnapshotElement { ref_id: "@e1".into(), kind: "input".into(), label: "Email".into() },
        ];
        let found = BrowserTool::find_element_by_label(&elements, "nonexistent");
        assert!(found.is_none());
    }

    #[test]
    fn test_submit_and_confirm_full_trust_not_guarded() {
        let tool = BrowserTool::new_unchecked(TrustLevel::Full);
        assert!(!tool.should_guard("submit_and_confirm", ""));
    }

    #[test]
    fn test_submit_and_confirm_autonomous_always_guarded() {
        let tool = BrowserTool::new_unchecked(TrustLevel::Autonomous);
        assert!(tool.should_guard("submit_and_confirm", ""));
    }
```

### Step 2: Run tests (expect fail)

```bash
cargo nextest run -p tools browser::tests 2>&1 | head -20
```
Expected: compile error — `find_element_by_label` not defined.

### Step 3: Implement composite helpers

Add to `BrowserTool` impl block:

```rust
    /// Find the first element whose label contains `query` (case-insensitive).
    pub fn find_element_by_label<'a>(
        elements: &'a [SnapshotElement],
        query: &str,
    ) -> Option<&'a SnapshotElement> {
        let q = query.to_lowercase();
        elements.iter().find(|e| e.label.to_lowercase().contains(&q))
    }

    // ── Composite helpers ─────────────────────────────────────────────────────

    /// fill_form: snapshot → match labels → fill each field.
    async fn act_fill_form(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let fields = p
            .args
            .get("fields")
            .and_then(|v| v.as_object())
            .ok_or_else(|| ToolError::InvalidParams("fill_form requires 'fields' object".into()))?
            .clone();

        let snapshot_raw = self.act_snapshot().await?;
        let elements = Self::parse_snapshot(&snapshot_raw);

        let mut filled = Vec::new();
        let mut not_found = Vec::new();

        for (label, value) in &fields {
            let val = value.as_str().unwrap_or_default();
            if self.should_guard("fill", label) {
                return Ok(Self::guard_message("fill", label));
            }
            match Self::find_element_by_label(&elements, label) {
                Some(elem) => {
                    let args = Self::build_args("fill", &[&elem.ref_id, val]);
                    self.run(&args).await?;
                    filled.push(label.as_str());
                }
                None => not_found.push(label.as_str()),
            }
        }

        let mut result = format!("Filled: {}", filled.join(", "));
        if !not_found.is_empty() {
            result.push_str(&format!(". Not found: {}", not_found.join(", ")));
        }
        Ok(result)
    }

    /// login_flow: navigate → snapshot → fill credentials → Enter → wait.
    async fn act_login_flow(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let url      = p.required_str("url")?;
        let username = p.required_str("username")?;
        let password = p.required_str("password")?;

        // 1. Navigate
        self.run(&Self::build_args("navigate", &[url])).await?;

        // 2. Snapshot
        let snapshot_raw = self.act_snapshot().await?;
        let elements = Self::parse_snapshot(&snapshot_raw);

        // 3. Fill username
        let username_labels = ["email", "username", "user", "login"];
        let user_elem = elements.iter()
            .find(|e| {
                let l = e.label.to_lowercase();
                username_labels.iter().any(|k| l.contains(k))
            })
            .ok_or_else(|| ToolError::ExecutionFailed(
                "Could not find username/email field on page".into()
            ))?;

        self.run(&Self::build_args("fill", &[&user_elem.ref_id, username])).await?;

        // 4. Fill password
        let pass_elem = elements.iter()
            .find(|e| e.label.to_lowercase().contains("password"))
            .ok_or_else(|| ToolError::ExecutionFailed(
                "Could not find password field on page".into()
            ))?;

        self.run(&Self::build_args("fill", &[&pass_elem.ref_id, password])).await?;

        // 5. Press Enter to submit
        self.run(&["press".to_string(), "Enter".to_string()]).await?;

        // 6. Wait for navigation
        self.run(&["wait".to_string(), "load".to_string()]).await?;

        Ok(format!("Login flow completed for {}", url))
    }

    /// submit_and_confirm: always routes through write guard, then clicks element.
    async fn act_submit_and_confirm(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let element = p.required_str("element")?;

        // submit_and_confirm is always a write action regardless of trust level
        if self.should_guard("submit_and_confirm", element) {
            return Ok(Self::guard_message("submit_and_confirm", element));
        }

        // TrustLevel::Full reaches here
        let args = Self::build_args("click", &[element]);
        self.run(&args).await
    }
```

**Note:** `p.args` is the raw `&Value`. Adjust to use `p.args` or add a method to `ParamExtractor` if it doesn't expose the raw value. Check `crates/tools/src/params.rs` — if `ParamExtractor` doesn't expose the inner `Value`, use `args.get("fields")` directly by accepting `args: &Value` in `act_fill_form`, or add `pub fn raw(&self) -> &Value` to `ParamExtractor`.

Look at how other tools access object params — if needed, change `p.args.get("fields")` to `args.get("fields")` and pass `args: &Value` directly into the helper.

### Step 4: Run tests

```bash
cargo nextest run -p tools browser::tests
cargo build -p tools
```
Expected: all tests pass, zero warnings.

### Step 5: Commit

```bash
git add crates/tools/src/browser.rs
git commit -m "feat(tools): add fill_form, login_flow, submit_and_confirm composite helpers"
```

---

## Task 6: Register `BrowserTool` in `AgentLoopBuilder`

**Files:**
- Modify: `crates/agent/src/agent_loop/builder.rs`
- Modify: `crates/tools/src/lib.rs` (ensure `BrowserTool` is re-exported)

### Step 1: Add import and conditional registration

In `crates/agent/src/agent_loop/builder.rs`, add to the `use tools::{...}` block:

```rust
use tools::browser::{BrowserTool, TrustLevel};
```

Then after the web tools registration (after `tool_registry.register(WebFetchTool::new());`), add:

```rust
// Browser tool (optional — requires agent-browser binary)
if config.tools.browser.enabled {
    let trust_level = TrustLevel::from_str(&config.tools.browser.trust_level);
    match BrowserTool::new(trust_level) {
        Ok(tool) => {
            tool_registry.register(tool);
            info!("Browser tool registered (trust_level={})", config.tools.browser.trust_level);
        }
        Err(e) => {
            warn!("Browser tool disabled: {}", e);
        }
    }
}
```

### Step 2: Build the full workspace

```bash
cargo build --workspace
```
Expected: compiles with zero warnings.

### Step 3: Run agent tests

```bash
cargo nextest run -p agent
```
Expected: all tests pass.

### Step 4: Commit

```bash
git add crates/agent/src/agent_loop/builder.rs
git commit -m "feat(agent): register BrowserTool when config.tools.browser.enabled"
```

---

## Task 7: Add browser pack to the init wizard

**Files:**
- Modify: `crates/cli/src/wizard/packs/registry.rs`
- Modify: `crates/cli/src/wizard/pack_selection.rs`

### Step 1: Update registry tests first (they will fail after adding the pack)

In `registry.rs`, find the test `test_all_packs_returns_7` and update:

```rust
fn test_all_packs_returns_7() {
    // Renamed: now 8 packs
    let packs = PackRegistry::all();
    assert_eq!(packs.len(), 8);
}

fn test_optional_packs() {
    let opt = PackRegistry::by_tier(PackTier::Optional);
    assert_eq!(opt.len(), 4);  // was 3
}
```

### Step 2: Run tests (expect fail — still 7 packs)

```bash
cargo nextest run -p cli registry
```
Expected: `assertion failed: 8 == 7`.

### Step 3: Add the browser pack to `PACKS`

In `registry.rs`, add to the `PACKS` static array:

```rust
    Pack {
        id: "browser",
        name: "Browser Automation",
        description: "Real-world task execution: booking, shopping, account management",
        tier: PackTier::Optional,
        skills: &["browser"],
    },
```

### Step 4: Run registry tests

```bash
cargo nextest run -p cli
```
Expected: all tests pass.

### Step 5: Add `apply_pack_config` mutation for browser

In `pack_selection.rs`, inside `apply_pack_config`, add after the finance block:

```rust
    // --- browser ---
    config.tools.browser.enabled = has("browser");
```

### Step 6: Add pack_selection test for browser

In `pack_selection.rs` `#[cfg(test)]` block, add:

```rust
    #[test]
    fn test_apply_pack_config_enables_browser() {
        let mut config = Config::default();
        assert!(!config.tools.browser.enabled);

        let selection = vec!["task-management".to_string(), "browser".to_string()];
        apply_pack_config(&mut config, &selection);

        assert!(config.tools.browser.enabled);
    }

    #[test]
    fn test_apply_pack_config_disables_browser_when_not_selected() {
        let mut config = Config::default();
        config.tools.browser.enabled = true;  // pre-enabled

        let selection = vec!["task-management".to_string()];
        apply_pack_config(&mut config, &selection);

        assert!(!config.tools.browser.enabled);
    }
```

### Step 7: Run all CLI tests

```bash
cargo nextest run -p cli
```
Expected: all tests pass.

### Step 8: Add `run_pack_selection` install detection for browser pack

In `pack_selection.rs`, modify the `Some(selected)` arm inside `run_pack_selection` to add a post-selection install check. After `apply_pack_config(&mut state.config, &selected);`, add:

```rust
            // If browser pack was just enabled, check for agent-browser binary
            if selected.iter().any(|id| id == "browser") {
                offer_agent_browser_install()?;
            }
```

Then add the `offer_agent_browser_install` function outside `run_pack_selection`:

```rust
/// Check for agent-browser binary and offer to install it if missing.
fn offer_agent_browser_install() -> anyhow::Result<()> {
    use common::utils::terminal::*;

    let which_cmd = if cfg!(target_os = "windows") { "where" } else { "which" };
    let found = std::process::Command::new(which_cmd)
        .arg("agent-browser")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if found {
        println!("  {} agent-browser already installed.", colorize("✓", SUCCESS));
        return Ok(());
    }

    println!("  {} agent-browser not found.", colorize("!", WARN));
    println!("  Install it to enable browser automation.\n");

    // Offer npm or brew
    let choices = if cfg!(target_os = "macos") {
        vec!["npm install -g agent-browser", "brew install agent-browser", "Skip (install later)"]
    } else {
        vec!["npm install -g agent-browser", "Skip (install later)"]
    };

    println!("  Choose install method:");
    for (i, choice) in choices.iter().enumerate() {
        println!("    {}. {}", i + 1, choice);
    }

    // Read single char from stdin (raw mode already disabled at this point)
    use std::io::{BufRead, Write};
    print!("  Choice [1]: ");
    std::io::stdout().flush()?;

    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line)?;
    let choice = line.trim().parse::<usize>().unwrap_or(1);

    if choice == choices.len() || choices.get(choice - 1).map(|c| c.contains("Skip")).unwrap_or(true) {
        println!("  Skipped. Run manually: npm install -g agent-browser\n");
        return Ok(());
    }

    let cmd = choices.get(choice - 1).unwrap_or(&choices[0]);
    println!("  Running: {}", colorize(cmd, BOLD));

    let parts: Vec<&str> = cmd.split_whitespace().collect();
    let status = std::process::Command::new(parts[0])
        .args(&parts[1..])
        .status()?;

    if status.success() {
        println!("  {} agent-browser installed successfully.\n", colorize("✓", SUCCESS));
    } else {
        println!("  {} Install failed. Run manually: {}\n", colorize("✗", WARN), cmd);
    }

    Ok(())
}
```

**Note:** `WARN` color constant may need to be added to `common::utils::terminal` if it doesn't exist — check and use `DIM` or `BOLD` as fallback.

### Step 9: Build and run all CLI tests

```bash
cargo build -p cli
cargo nextest run -p cli
```
Expected: compiles, all tests pass.

### Step 10: Commit

```bash
git add crates/cli/src/wizard/packs/registry.rs crates/cli/src/wizard/pack_selection.rs
git commit -m "feat(cli): add browser automation pack to init wizard"
```

---

## Task 8: Add `skills/browser/SKILL.md`

**Files:**
- Create: `skills/browser/SKILL.md`

### Step 1: Create the skill file

```bash
mkdir -p skills/browser
```

Create `skills/browser/SKILL.md`:

```markdown
# Browser Automation

Use the `browser` tool to navigate web pages and perform real-world tasks like booking tickets, shopping, and managing accounts.

## Core workflow

Always follow this sequence:
1. `navigate` to the target URL
2. `snapshot` to see all interactive elements as `@e1`, `@e2`, etc.
3. Interact using the `@e` references from the snapshot
4. `snapshot` again after navigation to refresh element references

## Action reference

| Action | When to use |
|---|---|
| `navigate` | Load a new URL |
| `snapshot` | Get current page elements (always do this before clicking) |
| `fill_form` | Fill multiple fields at once using label names |
| `login_flow` | Authenticate on a login page |
| `click` | Click a button or link by `@e` ref or label |
| `fill` | Fill a single input field |
| `type` | Type text character by character |
| `press` | Send a keyboard key (Enter, Tab, Escape) |
| `wait` | Wait for a page element or URL change |
| `get_text` | Extract text from an element |
| `screenshot` | Capture the current page state |
| `submit_and_confirm` | Click a submit/checkout button (always requires confirmation) |

## Write action confirmation

When you receive a `[CONFIRMATION_REQUIRED]` response from the browser tool:
1. Use `ask_user` to show the user what action is about to happen
2. Wait for their confirmation
3. If confirmed, call the same browser action again

Example:
```
browser: {"action": "click", "element": "@e5 Place Order"}
→ [CONFIRMATION_REQUIRED] About to click 'Place Order'. Use ask_user to confirm...

ask_user: "I'm about to click 'Place Order' on checkout.amazon.com. Shall I proceed?"
→ User: "Yes"

browser: {"action": "click", "element": "@e5"}  ← repeat the call
```

## Common task patterns

### Shopping
```
navigate → snapshot → fill_form (search) → snapshot → click (product) →
snapshot → click "Add to Cart" → snapshot → submit_and_confirm (checkout)
```

### Booking
```
navigate → snapshot → fill_form (dates/passengers) → snapshot →
click (search/available option) → snapshot → fill_form (passenger details) →
submit_and_confirm (book)
```

### Login
```
login_flow (url + username + password)
```

## Tips
- `@e` references expire after navigation — always snapshot again after a page change
- Use `fill_form` instead of individual `fill` calls for multi-field forms
- Use `screenshot` to verify state after complex interactions
- If an element is not found by label, try `snapshot` and inspect the raw output
```

### Step 2: Verify skill is loaded

Skill loading is file-based — verify the file exists in the right location:

```bash
ls skills/browser/SKILL.md
```
Expected: file exists.

### Step 3: Commit

```bash
git add skills/browser/SKILL.md
git commit -m "docs(skills): add browser automation skill"
```

---

## Task 9: Feature-gated integration tests

**Files:**
- Modify: `Cargo.toml` (workspace root — add `browser-integration` feature)
- Modify: `crates/tools/Cargo.toml` (add feature)
- Create: `tests/browser_integration_tests.rs`

### Step 1: Add feature flag to `crates/tools/Cargo.toml`

```toml
[features]
default = []
browser-integration = []
```

### Step 2: Write integration test (gated behind feature flag)

Create `tests/browser_integration_tests.rs`:

```rust
//! Browser integration tests — require a running agent-browser daemon.
//!
//! Run with: cargo nextest run --features browser-integration --test browser_integration_tests

#[cfg(feature = "browser-integration")]
mod browser_integration {
    use tools::browser::{BrowserTool, TrustLevel};
    use tools::RoutingContext;
    use tools::Tool;

    fn ctx() -> RoutingContext {
        RoutingContext::new("cli".into(), "test".into())
    }

    /// Requires: agent-browser daemon running, internet access.
    #[tokio::test]
    async fn test_navigate_and_snapshot() {
        let tool = BrowserTool::new(TrustLevel::Full)
            .expect("agent-browser must be installed");

        let args = serde_json::json!({"action": "navigate", "url": "https://example.com"});
        let result = tool.execute(args, &ctx()).await;
        assert!(result.is_ok(), "navigate failed: {:?}", result);

        let args = serde_json::json!({"action": "snapshot"});
        let snapshot = tool.execute(args, &ctx()).await.unwrap();
        assert!(!snapshot.is_empty(), "snapshot returned empty");
        // example.com should have at least one element
        assert!(snapshot.contains("@e"), "snapshot has no @e refs: {}", snapshot);
    }

    #[tokio::test]
    async fn test_write_guard_blocks_in_autonomous_mode() {
        let tool = BrowserTool::new(TrustLevel::Autonomous)
            .expect("agent-browser must be installed");

        // Navigate somewhere first
        let args = serde_json::json!({"action": "navigate", "url": "https://example.com"});
        tool.execute(args, &ctx()).await.unwrap();

        // A click on a "submit" label should return the guard message
        let args = serde_json::json!({"action": "click", "element": "@e1 button Submit"});
        let result = tool.execute(args, &ctx()).await.unwrap();
        assert!(
            result.contains("[CONFIRMATION_REQUIRED]"),
            "Expected guard message, got: {}",
            result
        );
    }

    #[tokio::test]
    async fn test_fill_form_fills_fields() {
        let tool = BrowserTool::new(TrustLevel::Full)
            .expect("agent-browser must be installed");

        // Use a simple form page
        let navigate = serde_json::json!({
            "action": "navigate",
            "url": "https://httpbin.org/forms/post"
        });
        tool.execute(navigate, &ctx()).await.unwrap();

        let fill = serde_json::json!({
            "action": "fill_form",
            "fields": {
                "Customer name": "Test User",
                "Telephone": "555-1234"
            }
        });
        let result = tool.execute(fill, &ctx()).await.unwrap();
        assert!(result.contains("Filled"), "fill_form result: {}", result);
    }
}

// Provide a compile-time stub so the file compiles without the feature
#[cfg(not(feature = "browser-integration"))]
#[test]
fn browser_integration_tests_require_feature_flag() {
    // Run with: cargo nextest run --features browser-integration --test browser_integration_tests
    println!("Skipped: compile with --features browser-integration to run browser tests");
}
```

### Step 3: Run stub test (always passes)

```bash
cargo nextest run --test browser_integration_tests
```
Expected: 1 test passes (`browser_integration_tests_require_feature_flag`).

### Step 4: Confirm feature-gated tests compile

```bash
cargo build --features browser-integration -p tools
```
Expected: compiles. (Tests won't run without a live daemon.)

### Step 5: Commit

```bash
git add tests/browser_integration_tests.rs crates/tools/Cargo.toml
git commit -m "test(browser): add feature-gated integration tests"
```

---

## Task 10: Final verification

### Step 1: Full workspace build

```bash
cargo build --workspace
```
Expected: zero errors, zero warnings.

### Step 2: Full test suite

```bash
cargo nextest run --workspace
```
Expected: all tests pass.

### Step 3: Clippy

```bash
cargo clippy --workspace --all-targets --all-features
```
Expected: zero warnings.

### Step 4: Verify config schema end-to-end

```bash
cargo run --bin klyntbot -- status
```
Expected: status output shows browser tool as disabled (default). No crashes.

### Step 5: Final commit

```bash
git add -p  # review any remaining changes
git commit -m "feat(browser): complete browser automation tool via agent-browser"
```

---

## Summary of all files changed

| File | Change |
|---|---|
| `crates/config/src/schema/tools.rs` | Add `BrowserConfig`, `browser` field on `ToolsConfig` |
| `crates/tools/src/browser.rs` | New: `TrustLevel`, `SnapshotElement`, `BrowserTool`, all actions |
| `crates/tools/src/lib.rs` | Add `pub mod browser; pub use browser::{BrowserTool, TrustLevel}` |
| `crates/agent/src/agent_loop/builder.rs` | Conditional `BrowserTool` registration |
| `crates/cli/src/wizard/packs/registry.rs` | Add browser pack, update count tests |
| `crates/cli/src/wizard/pack_selection.rs` | Add `config.tools.browser.enabled`, install detection |
| `skills/browser/SKILL.md` | New: LLM guidance for browser tool usage |
| `tests/browser_integration_tests.rs` | New: feature-gated integration tests |
| `crates/tools/Cargo.toml` | Add `browser-integration` feature |
