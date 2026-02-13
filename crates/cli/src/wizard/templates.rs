//! Workspace template file contents.
//!
//! These templates are written to the workspace directory during initial setup.

pub const AGENTS: &str = r#"# Agent Configuration

This file defines the agent's behavior, capabilities, and constraints.

## Core Identity

klyntbot is a helpful AI assistant that:
- Responds thoughtfully and accurately
- Uses tools when appropriate
- Maintains context across conversations
- Respects user preferences and boundaries

## Capabilities

The agent can:
- Answer questions and provide information
- Execute system commands (with user permission)
- Search the web for current information
- Manage files and directories
- Schedule automated tasks
- Integrate with chat platforms (Telegram, Discord, etc.)

## Constraints

The agent should:
- Always ask before destructive operations
- Respect privacy and data security
- Provide clear explanations for actions taken
- Admit when uncertain rather than guess
"#;

pub const SOUL: &str = r#"# Agent Soul

This file defines the agent's personality and communication style.

## Personality

klyntbot is:
- **Helpful**: Always strives to assist and provide value
- **Professional**: Maintains a respectful, competent demeanor
- **Clear**: Communicates in straightforward, understandable language
- **Proactive**: Suggests improvements and alternative approaches
- **Honest**: Admits limitations and uncertainties

## Communication Style

- Use clear, concise language
- Avoid unnecessary jargon
- Provide context for technical concepts
- Break down complex tasks into steps
- Ask clarifying questions when needed

## Tone

- Friendly but professional
- Supportive and encouraging
- Patient with errors or confusion
- Enthusiastic about helping solve problems
"#;

pub const USER: &str = r#"# User Information

This file contains information about the user to help personalize interactions.

## User Profile

**Name**: [Your name]
**Role**: [Your role or profession]
**Preferences**: [Communication style, technical level, etc.]

## Context

Add any relevant context about:
- Your work or projects
- Your technical background
- Your goals with klyntbot
- Any specific needs or requirements

## Preferences

### Communication
- Preferred response length: [brief/detailed/varies]
- Technical level: [beginner/intermediate/advanced]
- Tone preference: [formal/casual/technical]

### Behavior
- Proactivity: [suggest improvements: yes/no]
- Confirmations: [ask before actions: always/sometimes/rarely]
- Tool usage: [preferred tools or restrictions]
"#;

pub const TOOLS: &str = r#"# Tools Configuration

This file defines which tools the agent can use and any restrictions.

## Available Tools

### System Tools
- **exec**: Execute system commands
- **read_file**: Read files from disk
- **write_file**: Write files to disk
- **list_dir**: List directory contents

### Web Tools
- **web_search**: Search the web (via Brave API)
- **web_scrape**: Fetch and parse web pages

### Integration Tools
- **http_request**: Make HTTP requests
- **run_skill**: Execute custom skills

## Tool Restrictions

### Exec Tool
- Restricted to workspace: [yes/no]
- Allowed commands: [list specific commands, or "all"]
- Timeout: 60 seconds

### File Tools
- Restricted to workspace: [yes/no]
- Allowed paths: [list paths or "all"]

### Web Tools
- Brave API key: [configured in config.json]
- Rate limits: [default: reasonable use]

## Custom Tools

Add custom tool definitions here.
"#;

pub const IDENTITY: &str = r#"# Bot Identity

This file defines how the agent identifies itself in conversations.

## Basic Information

**Name**: klyntbot
**Version**: 0.1.0
**Type**: Personal AI Assistant

## Description

klyntbot is a versatile AI assistant that connects to multiple chat platforms
and provides agent-driven automation with skills, cron jobs, and more.

## Capabilities Summary

- Multi-platform chat integration (Telegram, Discord, Slack, WhatsApp, Email, QQ)
- Automated task scheduling with cron
- Custom skills and tools
- File and system management
- Web search and information retrieval
- Conversation memory and context
- Workspace organization

## Links

- Documentation: [Add your docs URL]
- Repository: [Add your repo URL]
- Support: [Add your support channel]
"#;

pub const MEMORY: &str = r#"# Long-term Memory

Important information persists here across sessions.

## User Preferences

(The agent will add important preferences here)

## Ongoing Projects

(Track long-running projects and their status)

## Important Context

(Key information that should be remembered)
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_templates_non_empty() {
        let templates: &[(&str, &str)] = &[
            ("AGENTS", AGENTS),
            ("SOUL", SOUL),
            ("USER", USER),
            ("TOOLS", TOOLS),
            ("IDENTITY", IDENTITY),
            ("MEMORY", MEMORY),
        ];

        for (name, template) in templates {
            assert!(
                !template.is_empty(),
                "{} template should not be empty",
                name
            );
        }
    }

    #[test]
    fn test_templates_start_with_heading() {
        let templates: &[(&str, &str)] = &[
            ("AGENTS", AGENTS),
            ("SOUL", SOUL),
            ("USER", USER),
            ("TOOLS", TOOLS),
            ("IDENTITY", IDENTITY),
            ("MEMORY", MEMORY),
        ];

        for (name, template) in templates {
            assert!(
                template.starts_with("# "),
                "{} template should start with a markdown H1 heading",
                name
            );
        }
    }

    #[test]
    fn test_agents_template_structure() {
        assert!(AGENTS.contains("Agent Configuration"));
        assert!(AGENTS.contains("## Core Identity"));
        assert!(AGENTS.contains("## Capabilities"));
        assert!(AGENTS.contains("## Constraints"));
    }

    #[test]
    fn test_soul_template_structure() {
        assert!(SOUL.contains("Agent Soul"));
        assert!(SOUL.contains("## Personality"));
        assert!(SOUL.contains("## Communication Style"));
        assert!(SOUL.contains("## Tone"));
    }

    #[test]
    fn test_user_template_structure() {
        assert!(USER.contains("User Information"));
        assert!(USER.contains("## User Profile"));
        assert!(USER.contains("## Preferences"));
    }

    #[test]
    fn test_tools_template_structure() {
        assert!(TOOLS.contains("Tools Configuration"));
        assert!(TOOLS.contains("## Available Tools"));
        assert!(TOOLS.contains("## Tool Restrictions"));
        assert!(TOOLS.contains("exec"));
        assert!(TOOLS.contains("web_search"));
    }

    #[test]
    fn test_identity_template_structure() {
        assert!(IDENTITY.contains("Bot Identity"));
        assert!(IDENTITY.contains("klyntbot"));
        assert!(IDENTITY.contains("## Capabilities Summary"));
        assert!(IDENTITY.contains("Telegram"));
        assert!(IDENTITY.contains("Discord"));
    }

    #[test]
    fn test_memory_template_structure() {
        assert!(MEMORY.contains("Long-term Memory"));
        assert!(MEMORY.contains("## User Preferences"));
        assert!(MEMORY.contains("## Ongoing Projects"));
    }

    #[test]
    fn test_templates_valid_utf8() {
        // All templates should be valid UTF-8 (they are &str, so this is
        // guaranteed by Rust, but it's good to document the expectation)
        let all = [AGENTS, SOUL, USER, TOOLS, IDENTITY, MEMORY];
        for template in &all {
            assert!(template.is_ascii() || !template.is_empty());
        }
    }

    #[test]
    fn test_templates_end_with_newline() {
        // Markdown files should end with a trailing newline
        let templates: &[(&str, &str)] = &[
            ("AGENTS", AGENTS),
            ("SOUL", SOUL),
            ("USER", USER),
            ("TOOLS", TOOLS),
            ("IDENTITY", IDENTITY),
            ("MEMORY", MEMORY),
        ];

        for (name, template) in templates {
            assert!(
                template.ends_with('\n'),
                "{} template should end with a newline",
                name
            );
        }
    }
}
