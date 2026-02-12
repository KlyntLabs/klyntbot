//! Interactive onboarding wizard for klyntbot initialization.

use std::io::{self, Write};

use klyntbot_config::{self as config, Config};
use anyhow::Result;
use klyntbot_core::utils::terminal::*;

/// Provider option for the onboarding wizard
#[derive(Debug, Clone)]
struct ProviderOption {
    name: &'static str,
    key: &'static str,
    description: &'static str,
    api_url: &'static str,
}

const PROVIDERS: &[ProviderOption] = &[
    ProviderOption {
        name: "Anthropic (Claude)",
        key: "anthropic",
        description: "Recommended for best quality",
        api_url: "https://console.anthropic.com",
    },
    ProviderOption {
        name: "OpenAI (GPT)",
        key: "openai",
        description: "Industry standard models",
        api_url: "https://platform.openai.com/api-keys",
    },
    ProviderOption {
        name: "DeepSeek",
        key: "deepseek",
        description: "Cost-effective alternative",
        api_url: "https://platform.deepseek.com",
    },
    ProviderOption {
        name: "Google (Gemini)",
        key: "gemini",
        description: "Multimodal capabilities",
        api_url: "https://makersuite.google.com/app/apikey",
    },
    ProviderOption {
        name: "OpenRouter",
        key: "openrouter",
        description: "Access to many models",
        api_url: "https://openrouter.ai/keys",
    },
];

/// Run the interactive onboarding wizard
pub async fn run_wizard() -> Result<()> {
    // Welcome screen
    print_welcome();

    // Step 1: Choose provider
    println!("\n{}", draw_separator());
    println!(
        "\n{}",
        colorize("Step 1 of 4: Choose Your LLM Provider", BOLD)
    );
    println!();
    let provider_idx = select_provider()?;
    let provider = &PROVIDERS[provider_idx];

    // Step 2: API configuration
    println!("\n{}", draw_separator());
    println!("\n{}", colorize("Step 2 of 4: API Configuration", BOLD));
    println!();
    let (api_key, model) = configure_api(provider)?;

    // Step 3: Enable channels (optional)
    println!("\n{}", draw_separator());
    println!(
        "\n{}",
        colorize("Step 3 of 4: Enable Channels (Optional)", BOLD)
    );
    println!();
    let enable_channels = prompt_yes_no("Would you like to enable chat channels?", false)?;

    let mut config = Config::default();

    // Set provider config
    config.set_provider_key(provider.key, api_key);
    config.agents.defaults.model = model;

    if enable_channels {
        // For now, just show info. Full channel setup can come later.
        println!(
            "\n{}",
            colorize("Channel setup can be done later with:", DIM)
        );
        println!("{}", colorize("  klyntbot channels login <channel>", DIM));
    }

    // Step 4: Workspace setup
    println!("\n{}", draw_separator());
    println!("\n{}", colorize("Step 4 of 4: Workspace Setup", BOLD));
    println!();
    setup_workspace(&config)?;

    // Save configuration
    config::save(&config)?;

    // Completion screen
    print_completion(&config);

    Ok(())
}

/// Print welcome screen
fn print_welcome() {
    let welcome_text =
        "Welcome to klyntbot\n\nLet's set up your AI assistant.\nThis will take about 2 minutes.";
    println!("{}", draw_box(welcome_text, None));
}

/// Print completion screen
fn print_completion(_config: &Config) {
    println!("\n{}", draw_separator());
    println!();

    let config_path_str = config::config_path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "~/.klyntbot/config.json".to_string());

    let completion_text = format!(
        "Your AI assistant is ready to use.\n\nTry it out:\n  klyntbot chat\n  klyntbot chat \"Hello!\"\n\nGet help:\n  klyntbot --help\n  klyntbot status\n\nConfig: {}",
        config_path_str
    );

    println!("{}", draw_box(&completion_text, Some("Setup Complete!")));
}

/// Draw a separator line
fn draw_separator() -> String {
    let chars = BoxChars::get();
    format!(
        "{}{}{}",
        color(SEPARATOR),
        chars.horizontal.repeat(60),
        color(RESET)
    )
}

/// Select a provider from the list
fn select_provider() -> Result<usize> {
    println!("Select your provider (enter number):\n");

    for (idx, provider) in PROVIDERS.iter().enumerate() {
        println!(
            "  {}. {} - {}",
            colorize(&(idx + 1).to_string(), BOLD),
            provider.name,
            colorize(provider.description, DIM)
        );
    }
    println!();

    loop {
        print!("Provider [1]: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        // Default to 1 (Anthropic)
        if input.is_empty() {
            return Ok(0);
        }

        if let Ok(idx) = input.parse::<usize>() {
            if idx > 0 && idx <= PROVIDERS.len() {
                return Ok(idx - 1);
            }
        }

        println!(
            "{}",
            colorize(
                &format!("Please enter a number between 1 and {}", PROVIDERS.len()),
                ERROR
            )
        );
    }
}

/// Configure API key and model for the selected provider
fn configure_api(provider: &ProviderOption) -> Result<(String, String)> {
    println!("{} API Configuration", provider.name);
    println!("Get yours at: {}\n", colorize(provider.api_url, UNDERLINE));

    // API Key input
    let api_key = loop {
        print!("API Key: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim().to_string();

        if input.is_empty() {
            println!("{}", colorize("API key is required", ERROR));
            continue;
        }

        // Basic validation
        if input.len() < 10 {
            println!(
                "{}",
                colorize(
                    "API key seems too short. Please check and try again.",
                    WARNING
                )
            );
            continue;
        }

        break input;
    };

    // Model selection
    let default_model = match provider.key {
        "anthropic" => "anthropic/claude-opus-4-5",
        "openai" => "openai/gpt-4",
        "deepseek" => "deepseek/deepseek-chat",
        "gemini" => "google/gemini-pro",
        "openrouter" => "openrouter/auto",
        _ => "anthropic/claude-opus-4-5",
    };

    print!("Model [{}]: ", default_model);
    io::stdout().flush()?;

    let mut model_input = String::new();
    io::stdin().read_line(&mut model_input)?;
    let model = if model_input.trim().is_empty() {
        default_model.to_string()
    } else {
        model_input.trim().to_string()
    };

    println!(
        "\n{} Configuration validated successfully",
        status_success()
    );

    Ok((api_key, model))
}

/// Setup workspace directories and files
fn setup_workspace(config: &Config) -> Result<()> {
    let workspace = config.workspace_path();

    println!(
        "Creating workspace at {}...\n",
        colorize(&workspace.display().to_string(), DIM)
    );

    // Create directories
    std::fs::create_dir_all(&workspace)?;
    std::fs::create_dir_all(workspace.join("memory"))?;
    std::fs::create_dir_all(workspace.join("skills"))?;
    println!("  {} Created workspace directories", status_success());

    // Create template files
    create_template_file(&workspace.join("AGENTS.md"), AGENTS_TEMPLATE)?;
    create_template_file(&workspace.join("SOUL.md"), SOUL_TEMPLATE)?;
    create_template_file(&workspace.join("USER.md"), USER_TEMPLATE)?;
    create_template_file(&workspace.join("TOOLS.md"), TOOLS_TEMPLATE)?;
    create_template_file(&workspace.join("IDENTITY.md"), IDENTITY_TEMPLATE)?;
    println!("  {} Created workspace templates", status_success());

    // Create memory file
    let memory_file = workspace.join("memory").join("MEMORY.md");
    if !memory_file.exists() {
        std::fs::write(&memory_file, MEMORY_TEMPLATE)?;
    }
    println!("  {} Initialized memory directory", status_success());

    // Create config directories
    let config_dir = config::config_dir()?;
    std::fs::create_dir_all(config_dir.join("sessions"))?;
    std::fs::create_dir_all(config_dir.join("cron"))?;
    std::fs::create_dir_all(config_dir.join("media"))?;
    std::fs::create_dir_all(config_dir.join("history"))?;
    println!("  {} Set up configuration directories", status_success());

    Ok(())
}

/// Create a template file if it doesn't exist
fn create_template_file(path: &std::path::Path, content: &str) -> Result<()> {
    if !path.exists() {
        std::fs::write(path, content)?;
    }
    Ok(())
}

/// Prompt for yes/no input
fn prompt_yes_no(prompt: &str, default: bool) -> Result<bool> {
    let default_str = if default { "Y/n" } else { "y/N" };
    print!("{} [{}]: ", prompt, default_str);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim().to_lowercase();

    if input.is_empty() {
        return Ok(default);
    }

    Ok(input == "y" || input == "yes")
}

// ============================================================================
// Template Files
// ============================================================================

const AGENTS_TEMPLATE: &str = r#"# Agent Configuration

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

const SOUL_TEMPLATE: &str = r#"# Agent Soul

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

const USER_TEMPLATE: &str = r#"# User Information

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

const TOOLS_TEMPLATE: &str = r#"# Tools Configuration

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

const IDENTITY_TEMPLATE: &str = r#"# Bot Identity

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

const MEMORY_TEMPLATE: &str = r#"# Long-term Memory

Important information persists here across sessions.

## User Preferences

(The agent will add important preferences here)

## Ongoing Projects

(Track long-running projects and their status)

## Important Context

(Key information that should be remembered)
"#;
