---
name: klyntbot-new-tool
description: >
  Use when creating a new tool, feature crate, or feature package for klyntbot.
  Triggers on "new tool", "add a tool", "create feature", "scaffold",
  "new crate", or when the user wants to add a new capability to the agent.
license: MIT
metadata:
  author: klyntbot
  version: "1.0.0"
  updated-on: "2026-03-20"
  source: official
  tags: "scaffold,tool,feature,create,new"
---

# New Tool Scaffolding

Guide the user through creating a new klyntbot tool end-to-end. This is a 7-file process with strict conventions — missing any step causes test failures.

## Before Starting

1. Ask the user: **What does this tool do?** Get a name, description, and list of actions.
2. Decide the implementation style:
   - **`#[tool_actions]` macro** (preferred for multi-action tools) — generates `Tool` impl from annotated methods
   - **Manual `impl Tool`** — for tools with complex parameter schemas or dynamic behavior
   - **`#[derive(Tool)]` + `ToolExecute`** — for single-action typed tools

3. Read `references/checklist.md` for the exact file-by-file wiring guide.

## Tool Naming

- Tool name = singular noun or verb-noun: `finance`, `tasks`, `notes`, `cron`
- Crate name = `feature-{name}`: `feature-finance`, `feature-tasks`
- Registry key = tool's `name()` return value — this MUST match the MCP whitelist entry

## Quick Checklist

1. Create `crates/feature-{name}/` with Cargo.toml, src/lib.rs, src/tool.rs
2. Implement `Tool` trait (or use derive macros)
3. Implement `FeaturePackage` in lib.rs
4. Add migration SQL if needed
5. Register tool in `crates/agent/src/agent_loop/builder.rs`
6. Add to `default_exposed_tools()` in `crates/config/src/schema/mcp.rs`
7. Add Tauri commands in `crates/desktop/src/commands/{name}.rs`
8. Wire into DEV_COMMANDS + dispatch_dev + dev_server test list
9. Create Claude Code skill in `.claude/skills/klyntbot-{name}/`

## Red Flags — STOP

- Using a tool name that doesn't match the registry key
- Forgetting DEV_COMMANDS — the `dev_server_covers_all_tauri_commands` test will fail
- Forgetting to add to `default_exposed_tools()` — MCP clients won't see the tool
- Skipping the Claude Code skill — Claude Code won't know how to use the tool
