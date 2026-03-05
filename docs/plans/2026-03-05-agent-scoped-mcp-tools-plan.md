# Agent-Scoped MCP Tool Filtering — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add `mcp_tools` field to agent profiles so each agent controls which MCP servers' tools it can access, replacing the unconditional `mcp_*` bypass.

**Architecture:** New `mcp_tools: Vec<String>` field on `AgentProfile`, parsed from YAML frontmatter. `filter_tools_for_profile()` uses this field to scope MCP tools per agent. Default is empty (no MCP tools). `["*"]` means all servers.

**Tech Stack:** Rust, serde_yaml, serde_json

---

### Task 1: Add `mcp_tools` to AgentProfile

**Files:**
- Modify: `crates/agent/src/agent_profile/types.rs`

**Step 1: Write the failing tests**

Add these tests to the existing `mod tests` block at the bottom of the file:

```rust
#[test]
fn test_parse_agent_md_with_mcp_tools() {
    let content = r#"---
name: communication
description: Communication agent
tools: [message, ask_user]
mcp_tools: [linear, slack]
---

Instructions here.
"#;
    let profile = AgentProfile::parse("communication", content, PathBuf::from("builtin::communication")).unwrap();
    assert_eq!(profile.mcp_tools, vec!["linear", "slack"]);
    assert!(profile.allows_mcp_server("linear"));
    assert!(profile.allows_mcp_server("slack"));
    assert!(!profile.allows_mcp_server("github"));
}

#[test]
fn test_parse_agent_md_mcp_tools_defaults_empty() {
    let content = r#"---
name: task
description: Task agent
tools: [task]
---

Instructions here.
"#;
    let profile = AgentProfile::parse("task", content, PathBuf::from("builtin::task")).unwrap();
    assert!(profile.mcp_tools.is_empty());
    assert!(!profile.allows_mcp_server("linear"));
}

#[test]
fn test_mcp_tools_wildcard_allows_all() {
    let profile = AgentProfile {
        name: "general".into(),
        mcp_tools: vec!["*".into()],
        ..Default::default()
    };
    assert!(profile.allows_mcp_server("linear"));
    assert!(profile.allows_mcp_server("github"));
    assert!(profile.allows_mcp_server("anything"));
}

#[test]
fn test_mcp_tools_empty_denies_all() {
    let profile = AgentProfile {
        name: "task".into(),
        mcp_tools: vec![],
        ..Default::default()
    };
    assert!(!profile.allows_mcp_server("linear"));
    assert!(!profile.allows_mcp_server("github"));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p agent -E 'test(mcp_tools)' --no-capture`
Expected: FAIL — `mcp_tools` field and `allows_mcp_server` method don't exist yet

**Step 3: Implement the changes**

Add `mcp_tools` field to `AgentProfile` struct (after `tools` field, line 12):

```rust
pub mcp_tools: Vec<String>,
```

Add to `Default` impl (after `tools: vec![]`, line 27):

```rust
mcp_tools: vec![],
```

Add `mcp_tools` field to `AgentFrontmatter` (after `tools`, line 54):

```rust
#[serde(default)]
mcp_tools: Vec<String>,
```

Add to `AgentProfile::parse()` inside the `Ok(Self { ... })` block (after `tools: fm.tools`, line 89):

```rust
mcp_tools: fm.mcp_tools,
```

Add new method to `impl AgentProfile` (after `allowed_tool_names`, around line 109):

```rust
/// Check if this profile allows tools from the given MCP server name.
/// Empty `mcp_tools` denies all. `["*"]` allows all.
pub fn allows_mcp_server(&self, server_name: &str) -> bool {
    self.mcp_tools.iter().any(|s| s == "*" || s == server_name)
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo nextest run -p agent -E 'test(mcp_tools)' --no-capture`
Expected: PASS — all 4 new tests pass

**Step 5: Run full agent crate tests to check nothing broke**

Run: `cargo nextest run -p agent`
Expected: PASS — existing tests unaffected

**Step 6: Commit**

```bash
git add crates/agent/src/agent_profile/types.rs
git commit -m "feat(agent): add mcp_tools field to AgentProfile for scoped MCP access"
```

---

### Task 2: Update `filter_tools_for_profile` to scope MCP tools

**Files:**
- Modify: `crates/agent/src/agent_runtime/runtime.rs:700-718`

**Step 1: Write the failing test**

Add to `runtime.rs` test module (or create one if absent). Since `filter_tools_for_profile` is a private function, add tests in the same file:

```rust
#[cfg(test)]
mod filter_tests {
    use super::*;
    use crate::agent_profile::AgentProfile;
    use serde_json::json;
    use std::path::PathBuf;

    fn make_tool_def(name: &str) -> serde_json::Value {
        json!({
            "type": "function",
            "function": {
                "name": name,
                "description": "test tool",
                "parameters": { "type": "object", "properties": {} }
            }
        })
    }

    #[test]
    fn test_filter_blocks_mcp_tools_for_restricted_agent() {
        let profile = AgentProfile {
            name: "task".into(),
            tools: vec!["task".into(), "area".into()],
            mcp_tools: vec![], // No MCP tools
            ..Default::default()
        };

        let tool_defs = vec![
            make_tool_def("task"),
            make_tool_def("area"),
            make_tool_def("mcp_linear_create_issue"),
            make_tool_def("mcp_linear_list_issues"),
            make_tool_def("finance"),
        ];

        let filtered = filter_tools_for_profile(&tool_defs, &profile);
        let names: Vec<&str> = filtered
            .iter()
            .filter_map(|t| common::tool_def_name(t))
            .collect();

        // Should include native allowed tools + ask_user, but NOT mcp tools or finance
        assert!(names.contains(&"task"));
        assert!(names.contains(&"area"));
        assert!(!names.contains(&"mcp_linear_create_issue"));
        assert!(!names.contains(&"mcp_linear_list_issues"));
        assert!(!names.contains(&"finance"));
    }

    #[test]
    fn test_filter_allows_mcp_tools_for_wildcard_agent() {
        let profile = AgentProfile {
            name: "general".into(),
            tools: vec![], // All native tools
            mcp_tools: vec!["*".into()], // All MCP servers
            ..Default::default()
        };

        let tool_defs = vec![
            make_tool_def("task"),
            make_tool_def("mcp_linear_create_issue"),
            make_tool_def("mcp_github_list_repos"),
        ];

        let filtered = filter_tools_for_profile(&tool_defs, &profile);
        let names: Vec<&str> = filtered
            .iter()
            .filter_map(|t| common::tool_def_name(t))
            .collect();

        assert!(names.contains(&"task"));
        assert!(names.contains(&"mcp_linear_create_issue"));
        assert!(names.contains(&"mcp_github_list_repos"));
    }

    #[test]
    fn test_filter_allows_specific_mcp_server() {
        let profile = AgentProfile {
            name: "comms".into(),
            tools: vec!["message".into()],
            mcp_tools: vec!["linear".into()], // Only Linear
            ..Default::default()
        };

        let tool_defs = vec![
            make_tool_def("message"),
            make_tool_def("mcp_linear_create_issue"),
            make_tool_def("mcp_github_list_repos"),
            make_tool_def("task"),
        ];

        let filtered = filter_tools_for_profile(&tool_defs, &profile);
        let names: Vec<&str> = filtered
            .iter()
            .filter_map(|t| common::tool_def_name(t))
            .collect();

        assert!(names.contains(&"message"));
        assert!(names.contains(&"mcp_linear_create_issue"));
        assert!(!names.contains(&"mcp_github_list_repos"));
        assert!(!names.contains(&"task"));
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p agent -E 'test(filter)' --no-capture`
Expected: FAIL — current code unconditionally passes `mcp_*` tools through

**Step 3: Replace `filter_tools_for_profile` implementation**

Replace the function at lines 700-718 with:

```rust
/// Filter tool definitions to only those allowed by the agent profile.
/// Native tools are filtered by `profile.tools` (empty = all allowed).
/// MCP tools are filtered by `profile.mcp_tools` (empty = none allowed, ["*"] = all).
fn filter_tools_for_profile(
    tool_defs: &[serde_json::Value],
    profile: &AgentProfile,
) -> Vec<serde_json::Value> {
    let native_allowlist = profile.allowed_tool_names();

    tool_defs
        .iter()
        .filter(|t| {
            let Some(name) = tool_def_name(t) else {
                return true;
            };

            if name.starts_with("mcp_") {
                // MCP tool: extract server name from "mcp_{server}_{tool}"
                let server_name = name
                    .strip_prefix("mcp_")
                    .and_then(|rest| rest.split('_').next())
                    .unwrap_or("");
                return profile.allows_mcp_server(server_name);
            }

            // Native tool: check against allowlist (None = all allowed)
            match &native_allowlist {
                Some(allowed) => allowed.contains(name),
                None => true,
            }
        })
        .cloned()
        .collect()
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo nextest run -p agent -E 'test(filter)' --no-capture`
Expected: PASS

**Step 5: Run full agent crate tests**

Run: `cargo nextest run -p agent`
Expected: PASS

**Step 6: Commit**

```bash
git add crates/agent/src/agent_runtime/runtime.rs
git commit -m "feat(agent): scope MCP tools per agent profile via mcp_tools field"
```

---

### Task 3: Update agent AGENT.md profiles

**Files:**
- Modify: `agents/general/AGENT.md`
- Modify: `agents/task/AGENT.md`
- Modify: `agents/finance/AGENT.md`
- Modify: `agents/calendar/AGENT.md`
- Modify: `agents/automation/AGENT.md`
- Modify: `agents/communication/AGENT.md`

**Step 1: Add `mcp_tools` to general agent**

In `agents/general/AGENT.md`, add `mcp_tools: ["*"]` to the frontmatter (after `max_iterations`):

```yaml
---
name: general
description: General-purpose assistant and orchestrator
tools: [ask_user, memory, web_search, web_fetch, grep, glob, read_file, list_dir, spawn, learning]
mcp_tools: ["*"]
max_iterations: 15
can_delegate_to: [task, finance, calendar, automation, communication]
always_skills: []
---
```

**Step 2: Add `mcp_tools: []` to all specialist agents**

For each of `task`, `finance`, `calendar`, `automation`, `communication` AGENT.md files, add `mcp_tools: []` to the frontmatter after `tools`:

Example for `agents/task/AGENT.md`:
```yaml
---
name: task
description: Task and project management specialist with planning, reviews, and goal tracking
tools: [task, area, project, okr, calendar, ask_user, memory, grep, glob, read_file, list_dir]
mcp_tools: []
triggers: [...]
...
---
```

Note: `mcp_tools: []` is technically optional since the default is empty, but being explicit improves readability and documents the intent.

**Step 3: Build to verify frontmatter parses correctly**

Run: `cargo build -p agent`
Expected: PASS — no compilation errors. The `include_str!` macros compile the AGENT.md files in.

**Step 4: Run agent profile parsing tests**

Run: `cargo nextest run -p agent -E 'test(parse_agent)' --no-capture`
Expected: PASS

**Step 5: Commit**

```bash
git add agents/
git commit -m "feat(agents): configure mcp_tools for all agent profiles"
```

---

### Task 4: Filter MCP tool names from intent classifier prompt

**Files:**
- Modify: `crates/agent/src/intent_pipeline/analysis.rs`

**Step 1: Understand the current flow**

The `IntentAnalyzer::analyze()` method at line 627 receives `tool_names: &[&str]` which includes all registered tool names (native + MCP). These are passed to `classify_with_llm()` which injects them into the `CLASSIFICATION_PROMPT`. This biases the classifier toward MCP tools even when the matched agent can't use them.

**Step 2: Add profile-aware tool name filtering**

Modify the `analyze` method signature to accept an optional `&AgentProfile`:

In `analysis.rs`, change the `analyze` method signature (line 627):

```rust
pub async fn analyze(
    &self,
    message: &str,
    tool_names: &[&str],
    profile: Option<&AgentProfile>,
) -> IntentAnalysis {
```

Add filtering logic at the top of `analyze`, before the heuristic call:

```rust
// Filter tool names to only those the matched agent can access
let filtered_names: Vec<&str>;
let effective_tool_names = if let Some(profile) = profile {
    filtered_names = tool_names
        .iter()
        .filter(|name| {
            if name.starts_with("mcp_") {
                let server_name = name
                    .strip_prefix("mcp_")
                    .and_then(|rest| rest.split('_').next())
                    .unwrap_or("");
                profile.allows_mcp_server(server_name)
            } else {
                true // Native tools filtered separately by profile.tools
            }
        })
        .copied()
        .collect();
    &filtered_names[..]
} else {
    tool_names
};
```

Then use `effective_tool_names` instead of `tool_names` in the rest of the method (the `classify_with_llm` call and the `references_mcp_tools` call).

**Step 3: Update the call site in `AgentRuntime`**

In `runtime.rs`, find where `analyzer.analyze()` is called and pass the matched profile:

```rust
// Before (approximately):
let analysis = self.intent_analyzer.analyze(message, &tool_names).await;

// After:
let analysis = self.intent_analyzer.analyze(message, &tool_names, Some(&profile)).await;
```

**Step 4: Update existing tests**

Update all existing calls to `analyzer.analyze()` in test code to pass `None` as the third argument (preserving existing behavior for tests that don't care about profile filtering).

**Step 5: Run tests**

Run: `cargo nextest run -p agent --no-capture`
Expected: PASS

**Step 6: Commit**

```bash
git add crates/agent/src/intent_pipeline/analysis.rs crates/agent/src/agent_runtime/runtime.rs
git commit -m "feat(agent): filter MCP tool names from intent classifier based on agent profile"
```

---

### Task 5: Integration verification and clippy

**Files:** None (verification only)

**Step 1: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

**Step 2: Run formatting check**

Run: `cargo fmt --all --check`
Expected: PASS

**Step 3: Run full workspace tests**

Run: `cargo nextest run --workspace`
Expected: PASS

**Step 4: Final commit if any fixups needed**

```bash
git add -A
git commit -m "chore: clippy and format fixes for mcp_tools scoping"
```
