# Browser Automation Design

> Date: 2026-02-23
> Feature: Real-world task execution via agent-browser (booking, shopping, account management)
> Status: Approved

---

## Overview

Add browser automation to klyntbot by integrating [Vercel Labs agent-browser](https://github.com/vercel-labs/agent-browser) — a Rust CLI that wraps Chromium via the Playwright protocol. The integration adds a `BrowserTool` to the existing tools system, enabling the agent to navigate pages, interact with elements, fill forms, and execute real-world tasks like ticket booking, shopping, and account management.

**Key properties:**
- 93% token reduction vs Playwright MCP via semantic `@e1` element references
- Configurable trust level (strict / autonomous / full) — default: autonomous with write guards
- Hybrid API: low-level primitives + composite helpers for common task patterns
- Feature-gated: opt-in via `klyntbot init --packs`, init wizard handles binary detection + install

---

## Architecture

### Approach

Approach A: `BrowserTool` lives directly in `crates/tools/src/browser.rs`. No new crate. Follows the same subprocess pattern as `ExecTool` (`tokio::process::Command`). Feature-gated in `AgentLoopBuilder` behind `config.tools.browser.enabled`.

### Files Changed

```
crates/
  tools/src/browser.rs                  ← NEW: BrowserTool + TrustLevel + write guard
  config/src/schema/tools.rs            ← ADD: BrowserConfig { enabled, trust_level, session_timeout_secs }
  agent/src/agent_loop/builder.rs       ← ADD: conditional BrowserTool registration
  cli/src/wizard/packs/registry.rs      ← ADD: browser pack (Optional tier)
  cli/src/wizard/packs/pack_selection.rs ← ADD: browser pack config mutation
  skills/browser/SKILL.md               ← NEW: LLM guidance for chaining browser actions
```

### Data Flow

```
User message → AgentPipeline → LLM calls browser tool
  → BrowserTool::execute(action, params)
      ├─ write-action guard (checks TrustLevel)
      │    └─ Strict / Autonomous → AskUserTool interception
      ├─ tokio::process::Command("agent-browser", [action, args...])
      └─ stdout → parsed result string → back to LLM context
```

### Session State

`BrowserTool` holds an `Arc<Mutex<Option<String>>>` session ID. The session is created on the first `navigate` call and reused across all subsequent tool calls within an agent session. The agent-browser daemon manages the actual browser process lifetime.

### Permission Level

`BrowserTool` returns `PermissionLevel::Elevated` — consistent with `ExecTool`. Channels without elevated permission (e.g. public Telegram bots) cannot invoke it.

---

## Action Set

Single tool, action-dispatch pattern. The LLM calls `browser` with an `action` field.

### Primitives (low-level)

| Action | agent-browser call | Returns |
|---|---|---|
| `navigate` | `open <url>` | Page title + current URL |
| `snapshot` | `snapshot` | Semantic element list (`@e1 button "Add to Cart"`) |
| `click` | `click @e1` | Confirmation string |
| `type` | `type @e1 <text>` | Confirmation string |
| `fill` | `fill @e1 <value>` | Confirmation string |
| `press` | `press <key>` | Confirmation string |
| `scroll` | `scroll <dir> [px]` | Confirmation string |
| `wait` | `wait <condition>` | Resolved state description |
| `get_text` | `get text @e1` | Extracted text |
| `screenshot` | `screenshot` | Saved file path (`~/.klyntbot/screenshots/<ts>.png`) |
| `eval` | `eval <js>` | JavaScript return value |

### Composite Helpers (high-level)

| Action | Behaviour |
|---|---|
| `fill_form` | Takes `fields: {label: value}` map. Calls `snapshot`, matches labels to `@e` refs, fills each. |
| `login_flow` | `navigate` → `snapshot` → fill username/password by label → `press Enter` → `wait` for redirect. |
| `submit_and_confirm` | Always routes through the write guard regardless of label. Click the target element on approval. |

### Example LLM Calls

```json
{ "tool": "browser", "action": "navigate", "url": "https://booking.com" }
{ "tool": "browser", "action": "snapshot" }
{ "tool": "browser", "action": "fill_form", "fields": { "Check-in": "2026-03-01", "Check-out": "2026-03-05" } }
{ "tool": "browser", "action": "submit_and_confirm", "element": "@e12" }
```

---

## Trust Level & Write Guard

### Enum

```rust
pub enum TrustLevel {
    Strict,     // ask_user before every write action
    Autonomous, // (default) ask_user only for detected dangerous actions
    Full,       // never pause
}
```

### Write Action Detection (Autonomous mode)

Fires when any of these conditions match:

| Condition | Trigger keywords |
|---|---|
| `click` element label contains | `submit`, `checkout`, `buy`, `purchase`, `confirm`, `place order`, `delete`, `remove`, `send`, `pay` |
| Action is `submit_and_confirm` | Always guarded |
| `fill` on a payment field label | `card`, `cvv`, `expiry`, `billing` |

### Guard Flow

```
write action detected
  ├─ TrustLevel::Full       → execute immediately
  ├─ TrustLevel::Autonomous → AskUserTool: "About to click 'Place Order' on checkout.amazon.com. Proceed?"
  │                            ✓ approved → execute
  │                            ✗ denied   → return "Action cancelled by user"
  └─ TrustLevel::Strict     → AskUserTool for every click / fill / type
```

The guard calls `AskUserTool` — already in the tool registry — keeping the confirmation flow consistent with the rest of klyntbot's interaction model.

---

## Configuration

### Config Schema (`~/.klyntbot/config.json`)

```json
{
  "tools": {
    "browser": {
      "enabled": false,
      "trustLevel": "autonomous",
      "sessionTimeoutSecs": 300
    }
  }
}
```

### Fields

| Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | bool | `false` | Enable the browser tool |
| `trustLevel` | string | `"autonomous"` | `"strict"` / `"autonomous"` / `"full"` |
| `sessionTimeoutSecs` | u64 | `300` | Idle session timeout before the daemon closes the browser |

### Init Wizard Integration

Browser automation is an **Optional** feature pack (`browser`). When selected in `klyntbot init --packs`:

1. Check for `agent-browser` binary via `which agent-browser` (or `where` on Windows)
2. If missing, offer installation:
   - Option A: `npm install -g agent-browser`
   - Option B: `brew install agent-browser` (macOS only)
3. Execute chosen install with a progress spinner
4. Verify binary post-install before writing config
5. Set `tools.browser.enabled = true` and `trustLevel = "autonomous"` in config

`klyntbot init --packs` re-runs pack selection only, so users can add browser without full re-init.

---

## Error Handling

| Failure | Behaviour |
|---|---|
| Binary not found at construction | Fail with: `"Browser tool requires agent-browser. Run: klyntbot init --packs"` |
| Daemon connection failure | Retry once (100ms delay), then: `"Browser session unavailable. Try navigating again."` |
| Command timeout (30s default) | Kill subprocess, return: `"Browser action timed out after 30s"` |
| Write action denied by user | Return: `"Action cancelled by user"` |
| Non-zero exit from agent-browser | Capture stderr, return as error string — LLM can self-correct |
| Screenshot save failure | Return error, preserve browser session state |

---

## Testing Strategy

### Unit Tests (inline `#[cfg(test)]` in `browser.rs`)

- Write guard classification: does `click @e1 "Place Order"` trigger the guard in Autonomous mode?
- `TrustLevel::from_str` roundtrip
- Command argument construction (no subprocess invoked)
- Output parsing from mocked stdout strings

### Integration Tests (`tests/`, feature-gated)

Gated behind `#[cfg(feature = "browser-integration")]` — CI stays fast without a browser daemon; enabled locally with `cargo nextest run --features browser-integration`.

- Full flow: `navigate` → `snapshot` → `fill_form` → `submit_and_confirm`
- Uses a local test HTTP server (tiny `axum` handler in the test harness)
- Trust level guard tests: Strict blocks, Autonomous prompts, Full passes through

### Init Wizard Tests

- Mock `which` output to test binary detection paths
- Mock install command to test the offer + install + verify flow

---

## Skills

`skills/browser/SKILL.md` — injected into system prompt when the browser pack is enabled. Guides the LLM to:
- Always `snapshot` before interacting with elements
- Use `fill_form` for multi-field forms rather than individual `fill` calls
- Prefer `login_flow` for authentication pages
- Use `screenshot` to verify state after complex interactions
- Understand that `@e` refs are session-scoped and refresh after navigation

---

## Out of Scope (this iteration)

- Multi-session management (one browser session per agent session)
- Screenshot forwarding to chat channels (screenshots saved to disk, path returned)
- Visual feedback loop (no vision model integration — screenshots for debugging only)
- Firefox / WebKit support (Chromium only via agent-browser default)
- Docker sandbox isolation (no SSRF protection in v1 — document that browser runs with user permissions)
