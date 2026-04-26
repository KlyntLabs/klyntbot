use std::path::PathBuf;

use desktop_shared::commands::{AiToolInfo, AiToolInstallResult, AiToolsInstallParams};
use desktop_shared::errors::ApiError;

use crate::state::AppCore;

// ── Supported AI tools ───────────────────────────────────────────────

struct AiToolDef {
    id: &'static str,
    name: &'static str,
    icon: &'static str,
    detect: fn() -> (bool, String),
    install: fn() -> Result<Vec<String>, String>,
}

fn home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("~"))
}

fn command_exists(name: &str) -> bool {
    std::process::Command::new("which")
        .arg(name)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

static TOOLS: &[AiToolDef] = &[
    AiToolDef {
        id: "claude-code",
        name: "Claude Code",
        icon: "claude",
        detect: detect_claude_code,
        install: install_claude_code,
    },
    AiToolDef {
        id: "codex",
        name: "OpenAI Codex",
        icon: "openai",
        detect: detect_codex,
        install: install_codex,
    },
    AiToolDef {
        id: "cursor",
        name: "Cursor",
        icon: "cursor",
        detect: detect_cursor,
        install: install_cursor,
    },
    AiToolDef {
        id: "windsurf",
        name: "Windsurf",
        icon: "windsurf",
        detect: detect_windsurf,
        install: install_windsurf,
    },
    AiToolDef {
        id: "copilot",
        name: "GitHub Copilot",
        icon: "github",
        detect: detect_copilot,
        install: install_copilot,
    },
    AiToolDef {
        id: "gemini-cli",
        name: "Gemini CLI",
        icon: "gemini",
        detect: detect_gemini_cli,
        install: install_gemini_cli,
    },
];

// ── Detection ────────────────────────────────────────────────────────

fn detect_claude_code() -> (bool, String) {
    let config = home_dir().join(".claude.json");
    if config.exists() {
        return (true, "Found ~/.claude.json".into());
    }
    if command_exists("claude") {
        return (true, "Found claude CLI in PATH".into());
    }
    (false, "Install: npm i -g @anthropic-ai/claude-code".into())
}

fn detect_codex() -> (bool, String) {
    if command_exists("codex") {
        return (true, "Found codex CLI in PATH".into());
    }
    let config = home_dir().join(".codex");
    if config.exists() {
        return (true, "Found ~/.codex config".into());
    }
    (false, "Install: npm i -g @openai/codex".into())
}

fn detect_cursor() -> (bool, String) {
    // macOS app
    if PathBuf::from("/Applications/Cursor.app").exists() {
        return (true, "Found Cursor.app".into());
    }
    let config = home_dir().join(".cursor");
    if config.exists() {
        return (true, "Found ~/.cursor config".into());
    }
    if command_exists("cursor") {
        return (true, "Found cursor in PATH".into());
    }
    (false, "Install from cursor.com".into())
}

fn detect_windsurf() -> (bool, String) {
    if PathBuf::from("/Applications/Windsurf.app").exists() {
        return (true, "Found Windsurf.app".into());
    }
    let config = home_dir().join(".windsurf");
    if config.exists() {
        return (true, "Found ~/.windsurf config".into());
    }
    (false, "Install from windsurf.com".into())
}

fn detect_copilot() -> (bool, String) {
    // Check VS Code with Copilot extension
    if PathBuf::from("/Applications/Visual Studio Code.app").exists() {
        return (true, "Found VS Code (Copilot compatible)".into());
    }
    if command_exists("gh") {
        return (true, "Found GitHub CLI (Copilot compatible)".into());
    }
    (false, "Install VS Code + GitHub Copilot extension".into())
}

fn detect_gemini_cli() -> (bool, String) {
    if command_exists("gemini") {
        return (true, "Found gemini CLI in PATH".into());
    }
    (false, "Install: npm i -g @anthropic-ai/gemini-cli".into())
}

// ── Skill definitions ────────────────────────────────────────────────

struct SkillDef {
    name: &'static str,
    description: &'static str,
    body: &'static str,
}

static SKILLS: &[SkillDef] = &[
    SkillDef {
        name: "klyntbot-tasks",
        description: "Create, organize, and track tasks using Klyntbot. Use when the user mentions todos, tasks, projects, areas, planning, goals, focus, decompose, subtasks, or time tracking.",
        body: r#"Use the `klyntbot` MCP tools (`tasks`, `area`, `project`) for task management.

## Quick Reference

| Action | Tool | Key params |
|--------|------|-----------|
| Create task | `tasks` | action: "create", title, area_id, priority |
| List tasks | `tasks` | action: "list", status, area_id |
| Complete task | `tasks` | action: "complete", id |
| Focus on task | `tasks` | action: "focus", id |
| Search | `tasks` | action: "search", query |
| List areas | `area` | action: "list" |
| List projects | `project` | action: "list" |

## Creating a Task

1. `area(action: "list")` — get available areas
2. Confirm details with user before creating
3. `tasks(action: "create", ...)` with confirmed values"#,
    },
    SkillDef {
        name: "klyntbot-finance",
        description: "Track expenses, budgets, and financial goals using Klyntbot. Use when the user mentions budget, spending, money, accounts, transactions, investments, savings, net worth, portfolio, or FIRE.",
        body: r#"Use the `klyntbot - finance` MCP tool for financial management.

## Quick Reference

| User says | Action | Key params |
|-----------|--------|-----------|
| "add expense" | `tx_add` | amount (in cents!), category, type: "expense" |
| "check budget" | `budget_status` | budget_id (optional) |
| "spending report" | `report_spending` | period (default: "monthly") |
| "net worth" | `net_worth` | — |
| "list accounts" | `account_list` | — |

## Critical Rules

- **Amounts in smallest unit**: $50 = 5000 cents
- **Never guess IDs** — use list actions first
- **Default currency** from `settings_get`"#,
    },
    SkillDef {
        name: "klyntbot-automation",
        description: "Schedule reminders and recurring tasks using Klyntbot. Use when the user mentions reminders, schedules, cron, recurring, every day, every hour, automate, or automation.",
        body: r#"Use the `klyntbot - cron` MCP tool for scheduling.

## Quick Reference

| User says | Params |
|-----------|--------|
| "remind me in 20min" | `action: "add", message: "...", every_seconds: 1200` |
| "every day at 8am" | `action: "add", message: "...", cron_expr: "0 8 * * *"` |
| "weekdays at 5pm" | `action: "add", message: "...", cron_expr: "0 17 * * 1-5"` |
| "list my reminders" | `action: "list"` |
| "cancel that reminder" | `action: "remove", job_id: "..."` |"#,
    },
    SkillDef {
        name: "klyntbot-notes",
        description: "Create, search, and organize notes and notebooks using Klyntbot. Use when the user mentions notes, notebooks, writing something down, jotting down, or linking information together.",
        body: r#"Use the `klyntbot - notes` MCP tool for note management.

## Quick Reference

| Action | Key params |
|--------|-----------|
| `create_note` | title, body, notebook_id, tags |
| `list_notes` | notebook_id (optional) |
| `search_notes` | query |
| `update_note` | id, title/body/tags |
| `link_notes` | id, target_ids[] |
| `create_notebook` | title, icon |
| `list_notebooks` | — |"#,
    },
    SkillDef {
        name: "klyntbot-memory",
        description: "Search past conversations and stored facts using Klyntbot's memory system. Use when the user asks 'do you remember', references past conversations, or wants to find something they discussed before.",
        body: r#"Use the `klyntbot - memory` MCP tool for memory search.

## Actions

| Action | Key params | When to use |
|--------|-----------|-------------|
| `search_conversations` | query, limit | Search past chat history |
| `search_all` | query, limit | Search across everything |
| `status` | — | Check memory system health |"#,
    },
    SkillDef {
        name: "klyntbot-okr",
        description: "Manage objectives and key results (OKRs) using Klyntbot. Use when the user mentions objectives, key results, OKRs, goals, metrics, tracking progress, or quarterly planning.",
        body: r#"Use the `klyntbot - okr` MCP tool. Actions use dotted namespace.

## Quick Reference

| Action | Key params |
|--------|-----------|
| `objective.create` | project_id, title, description, priority, due_date |
| `objective.list` | project_id, status |
| `kr.create` | objective_id, title, tracking_mode, target_value, unit |
| `kr.update_metric` | id, current_value |

## Workflow

1. `project(action: "list")` — get project_id
2. `objective.create` — under a project
3. `kr.create` — 2-4 key results per objective
4. `kr.update_metric` — track progress"#,
    },
    SkillDef {
        name: "klyntbot-productivity",
        description: "Track focus sessions, activity, and productivity goals using Klyntbot. Use when the user mentions focus, pomodoro, productivity, activity tracking, time logging, productivity score, or work goals.",
        body: r#"Use the `klyntbot - productivity` MCP tool for focus and activity tracking.

## Quick Reference

| Action | Key params | When to use |
|--------|-----------|-------------|
| `focus_start` | duration_mins, project_id | Start a focus session |
| `focus_end` | notes | Finished working |
| `focus_status` | — | Check active focus session |
| `pomodoro_start` | work_mins, break_mins | Pomodoro technique |
| `activity_today` | — | "What did I do today?" |
| `activity_week` | — | Weekly activity overview |
| `activity_score` | — | Current productivity score |
| `set_goal` | metric, target_value, goal_type | Set productivity targets |
| `check_goals` | — | Check goal progress |
| `log_time` | description, duration_mins | Manually log work time |"#,
    },
    SkillDef {
        name: "klyntbot-work-context",
        description: "View and manage work context sessions using Klyntbot. Use when the user mentions work context, what am I working on, current context, or project context linking.",
        body: r#"Use the `klyntbot - work_context` MCP tool for work context management.

## Quick Reference

| Action | Key params | When to use |
|--------|-----------|-------------|
| `list` | — | Show all work context sessions |
| `show` | id | View a specific context session |
| `rename` | id, title | Rename a context session |
| `link_project` | id, project_id | Link context to a project |
| `search` | query | Search across work contexts |"#,
    },
    SkillDef {
        name: "klyntbot-agent",
        description: "Send natural language requests to Klyntbot's AI agent for complex multi-step tasks. Use when a request spans multiple tools or needs AI reasoning to complete.",
        body: r#"Use the `klyntbot - agent` MCP tool to delegate complex requests to Klyntbot's AI pipeline.

## When to Use

- Request needs **multiple tools** (e.g., "check my budget then create a task")
- Request needs **AI reasoning** (e.g., "plan my week based on what's overdue")
- Request is **natural language** that doesn't map to a single tool action

## How to Use

```json
{"query": "natural language request here"}
```

## When NOT to Use

If you know the exact tool and action, call it directly — it's faster."#,
    },
];

// ── Format adapters ──────────────────────────────────────────────────

/// Agent Skills spec format (Claude Code, Codex).
fn format_skill_md(skill: &SkillDef) -> String {
    format!(
        r#"---
name: {}
description: >
  {}
license: MIT
---

{}
"#,
        skill.name, skill.description, skill.body
    )
}

/// Cursor MDC format.
fn format_cursor_mdc(skill: &SkillDef) -> String {
    format!(
        r#"---
description: "{}"
alwaysApply: false
---

# {}

{}
"#,
        skill.description.replace('"', r#"\""#),
        skill.name,
        skill.body
    )
}

/// Windsurf markdown format.
fn format_windsurf_md(skill: &SkillDef) -> String {
    format!(
        "# {}\n\n{}\n\n{}\n",
        skill.name, skill.description, skill.body
    )
}

/// GitHub Copilot .instructions.md format.
fn format_copilot_instructions(skill: &SkillDef) -> String {
    format!(
        r#"---
name: {}
description: "{}"
---

{}
"#,
        skill.name,
        skill.description.replace('"', r#"\""#),
        skill.body
    )
}

/// Gemini CLI markdown (appended to GEMINI.md).
fn format_gemini_section(skill: &SkillDef) -> String {
    format!(
        "\n## {} — {}\n\n{}\n",
        skill.name, skill.description, skill.body
    )
}

// ── MCP config snippets ──────────────────────────────────────────────

/// MCP server config JSON for klyntbot. Uses the absolute path of the currently
/// running binary so the registered command works even when `klyntbot` isn't on `$PATH`
/// (e.g., when launched from a `.app` bundle).
fn mcp_server_json() -> serde_json::Value {
    let command = std::env::current_exe()
        .ok()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "klyntbot".to_string());
    serde_json::json!({
        "command": command,
        "args": ["mcp", "serve", "--stdio"]
    })
}

fn write_mcp_config_claude_code() -> Result<Vec<String>, String> {
    let config_path = home_dir().join(".claude.json");
    let mut config: serde_json::Value = if config_path.exists() {
        let content =
            std::fs::read_to_string(&config_path).map_err(|e| format!("Read error: {e}"))?;
        serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    // Ensure mcpServers object exists
    if config.get("mcpServers").is_none() {
        config["mcpServers"] = serde_json::json!({});
    }
    config["mcpServers"]["klyntbot"] = mcp_server_json();

    let content = serde_json::to_string_pretty(&config).map_err(|e| format!("JSON error: {e}"))?;
    std::fs::write(&config_path, content).map_err(|e| format!("Write error: {e}"))?;

    Ok(vec![config_path.display().to_string()])
}

fn write_mcp_config_cursor() -> Result<Vec<String>, String> {
    let dir = home_dir().join(".cursor");
    std::fs::create_dir_all(&dir).map_err(|e| format!("Mkdir error: {e}"))?;

    let config_path = dir.join("mcp.json");
    let mut config: serde_json::Value = if config_path.exists() {
        let content =
            std::fs::read_to_string(&config_path).map_err(|e| format!("Read error: {e}"))?;
        serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    if config.get("mcpServers").is_none() {
        config["mcpServers"] = serde_json::json!({});
    }
    config["mcpServers"]["klyntbot"] = mcp_server_json();

    let content = serde_json::to_string_pretty(&config).map_err(|e| format!("JSON error: {e}"))?;
    std::fs::write(&config_path, content).map_err(|e| format!("Write error: {e}"))?;

    Ok(vec![config_path.display().to_string()])
}

fn write_mcp_config_windsurf() -> Result<Vec<String>, String> {
    let dir = home_dir().join(".codeium").join("windsurf");
    std::fs::create_dir_all(&dir).map_err(|e| format!("Mkdir error: {e}"))?;

    let config_path = dir.join("mcp_config.json");
    let mut config: serde_json::Value = if config_path.exists() {
        let content =
            std::fs::read_to_string(&config_path).map_err(|e| format!("Read error: {e}"))?;
        serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    if config.get("mcpServers").is_none() {
        config["mcpServers"] = serde_json::json!({});
    }
    config["mcpServers"]["klyntbot"] = mcp_server_json();

    let content = serde_json::to_string_pretty(&config).map_err(|e| format!("JSON error: {e}"))?;
    std::fs::write(&config_path, content).map_err(|e| format!("Write error: {e}"))?;

    Ok(vec![config_path.display().to_string()])
}

// ── Installation per platform ────────────────────────────────────────

fn install_claude_code() -> Result<Vec<String>, String> {
    let mut files = Vec::new();

    // Write skills to user-global location
    let skills_dir = home_dir().join(".config").join("claude").join("skills");
    for skill in SKILLS {
        let dir = skills_dir.join(skill.name);
        std::fs::create_dir_all(&dir).map_err(|e| format!("Mkdir error: {e}"))?;
        let path = dir.join("SKILL.md");
        std::fs::write(&path, format_skill_md(skill)).map_err(|e| format!("Write error: {e}"))?;
        files.push(path.display().to_string());
    }

    // Write MCP config
    files.extend(write_mcp_config_claude_code()?);

    Ok(files)
}

fn install_codex() -> Result<Vec<String>, String> {
    let mut files = Vec::new();

    // Codex uses the same Agent Skills spec — write to ~/.codex/skills/
    let skills_dir = home_dir().join(".codex").join("skills");
    for skill in SKILLS {
        let dir = skills_dir.join(skill.name);
        std::fs::create_dir_all(&dir).map_err(|e| format!("Mkdir error: {e}"))?;
        let path = dir.join("SKILL.md");
        std::fs::write(&path, format_skill_md(skill)).map_err(|e| format!("Write error: {e}"))?;
        files.push(path.display().to_string());
    }

    // Codex MCP config lives in ~/.codex/config.json
    let config_dir = home_dir().join(".codex");
    std::fs::create_dir_all(&config_dir).map_err(|e| format!("Mkdir error: {e}"))?;
    let config_path = config_dir.join("config.json");
    let mut config: serde_json::Value = if config_path.exists() {
        let content =
            std::fs::read_to_string(&config_path).map_err(|e| format!("Read error: {e}"))?;
        serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };
    if config.get("mcpServers").is_none() {
        config["mcpServers"] = serde_json::json!({});
    }
    config["mcpServers"]["klyntbot"] = mcp_server_json();
    let content = serde_json::to_string_pretty(&config).map_err(|e| format!("JSON error: {e}"))?;
    std::fs::write(&config_path, content).map_err(|e| format!("Write error: {e}"))?;
    files.push(config_path.display().to_string());

    Ok(files)
}

fn install_cursor() -> Result<Vec<String>, String> {
    let mut files = Vec::new();

    // Write rules to user-global ~/.cursor/rules/
    let rules_dir = home_dir().join(".cursor").join("rules");
    std::fs::create_dir_all(&rules_dir).map_err(|e| format!("Mkdir error: {e}"))?;
    for skill in SKILLS {
        let path = rules_dir.join(format!("{}.mdc", skill.name));
        std::fs::write(&path, format_cursor_mdc(skill)).map_err(|e| format!("Write error: {e}"))?;
        files.push(path.display().to_string());
    }

    // Write MCP config
    files.extend(write_mcp_config_cursor()?);

    Ok(files)
}

fn install_windsurf() -> Result<Vec<String>, String> {
    let mut files = Vec::new();

    // Write rules to ~/.windsurf/rules/
    let rules_dir = home_dir().join(".windsurf").join("rules");
    std::fs::create_dir_all(&rules_dir).map_err(|e| format!("Mkdir error: {e}"))?;
    for skill in SKILLS {
        let path = rules_dir.join(format!("{}.md", skill.name));
        std::fs::write(&path, format_windsurf_md(skill))
            .map_err(|e| format!("Write error: {e}"))?;
        files.push(path.display().to_string());
    }

    // Write MCP config
    files.extend(write_mcp_config_windsurf()?);

    Ok(files)
}

fn install_copilot() -> Result<Vec<String>, String> {
    let mut files = Vec::new();

    // Write to user's VS Code settings — global instructions
    // Copilot instructions go per-project, but we write a global one
    let instructions_dir = home_dir().join(".github").join("instructions");
    std::fs::create_dir_all(&instructions_dir).map_err(|e| format!("Mkdir error: {e}"))?;
    for skill in SKILLS {
        let path = instructions_dir.join(format!("{}.instructions.md", skill.name));
        std::fs::write(&path, format_copilot_instructions(skill))
            .map_err(|e| format!("Write error: {e}"))?;
        files.push(path.display().to_string());
    }

    // VS Code MCP config in user settings
    let vscode_dir = home_dir()
        .join("Library")
        .join("Application Support")
        .join("Code")
        .join("User");
    if vscode_dir.exists() {
        let mcp_path = vscode_dir.join("mcp.json");
        let mut config: serde_json::Value = if mcp_path.exists() {
            let content =
                std::fs::read_to_string(&mcp_path).map_err(|e| format!("Read error: {e}"))?;
            serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
        } else {
            serde_json::json!({})
        };
        if config.get("servers").is_none() {
            config["servers"] = serde_json::json!({});
        }
        let command = std::env::current_exe()
            .ok()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "klyntbot".to_string());
        config["servers"]["klyntbot"] = serde_json::json!({
            "type": "stdio",
            "command": command,
            "args": ["mcp", "serve", "--stdio"]
        });
        let content =
            serde_json::to_string_pretty(&config).map_err(|e| format!("JSON error: {e}"))?;
        std::fs::write(&mcp_path, content).map_err(|e| format!("Write error: {e}"))?;
        files.push(mcp_path.display().to_string());
    }

    Ok(files)
}

fn install_gemini_cli() -> Result<Vec<String>, String> {
    let mut files = Vec::new();

    // Append to global GEMINI.md
    let gemini_dir = home_dir().join(".gemini");
    std::fs::create_dir_all(&gemini_dir).map_err(|e| format!("Mkdir error: {e}"))?;
    let gemini_md = gemini_dir.join("GEMINI.md");

    let existing = if gemini_md.exists() {
        std::fs::read_to_string(&gemini_md).unwrap_or_default()
    } else {
        String::new()
    };

    // Only append skills that aren't already there
    let mut content = existing;
    if !content.contains("# Klyntbot Skills") {
        content.push_str("\n# Klyntbot Skills\n");
        content.push_str("\nKlyntbot is a personal AI agent available via MCP. ");
        content.push_str("Use the `klyntbot` MCP server to access these tools.\n");
    }
    for skill in SKILLS {
        if !content.contains(&format!("## {}", skill.name)) {
            content.push_str(&format_gemini_section(skill));
        }
    }
    std::fs::write(&gemini_md, content).map_err(|e| format!("Write error: {e}"))?;
    files.push(gemini_md.display().to_string());

    // Gemini MCP config in settings.json
    let settings_path = gemini_dir.join("settings.json");
    let mut config: serde_json::Value = if settings_path.exists() {
        let content =
            std::fs::read_to_string(&settings_path).map_err(|e| format!("Read error: {e}"))?;
        serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };
    if config.get("mcpServers").is_none() {
        config["mcpServers"] = serde_json::json!({});
    }
    config["mcpServers"]["klyntbot"] = mcp_server_json();
    let content = serde_json::to_string_pretty(&config).map_err(|e| format!("JSON error: {e}"))?;
    std::fs::write(&settings_path, content).map_err(|e| format!("Write error: {e}"))?;
    files.push(settings_path.display().to_string());

    Ok(files)
}

// ── Handler impl ─────────────────────────────────────────────────────

impl AppCore {
    /// Detect which AI coding tools are installed on this machine.
    pub async fn ai_tools_detect(&self) -> Result<Vec<AiToolInfo>, ApiError> {
        Ok(TOOLS
            .iter()
            .map(|t| {
                let (detected, hint) = (t.detect)();
                AiToolInfo {
                    id: t.id.to_string(),
                    name: t.name.to_string(),
                    detected,
                    detection_hint: hint,
                    icon: t.icon.to_string(),
                }
            })
            .collect())
    }

    /// Install klyntbot skills and MCP config for the selected AI tools.
    pub async fn ai_tools_install(
        &self,
        params: AiToolsInstallParams,
    ) -> Result<Vec<AiToolInstallResult>, ApiError> {
        let mut results = Vec::new();

        for tool_id in &params.tools {
            let tool = TOOLS.iter().find(|t| t.id == tool_id.as_str());
            let result = match tool {
                Some(t) => match (t.install)() {
                    Ok(files) => AiToolInstallResult {
                        tool_id: t.id.to_string(),
                        tool_name: t.name.to_string(),
                        success: true,
                        files_written: files,
                        message: format!("{} skills installed successfully", t.name),
                    },
                    Err(e) => AiToolInstallResult {
                        tool_id: t.id.to_string(),
                        tool_name: t.name.to_string(),
                        success: false,
                        files_written: vec![],
                        message: format!("Failed to install: {e}"),
                    },
                },
                None => AiToolInstallResult {
                    tool_id: tool_id.clone(),
                    tool_name: tool_id.clone(),
                    success: false,
                    files_written: vec![],
                    message: format!("Unknown tool: {tool_id}"),
                },
            };
            results.push(result);
        }

        // Also save selected tools to config
        let tools_list: Vec<String> = params.tools.clone();
        {
            let mut cfg = self.config.write().await;
            cfg.integrations.ai_tools = tools_list;
            config::save(&cfg)
                .await
                .map_err(|e| ApiError::new("CONFIG_ERROR", e.to_string()))?;
        }

        Ok(results)
    }
}
