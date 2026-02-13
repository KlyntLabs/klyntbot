//! CLI command definitions using clap.

use clap::{Parser, Subcommand};

/// klyntbot - Personal AI Assistant
#[derive(Parser, Debug)]
#[command(name = "klyntbot")]
#[command(
    about = "🐈 klyntbot - Personal AI Assistant",
    long_about = "A versatile AI assistant that connects to multiple chat platforms\nand provides agent-driven automation with skills, cron jobs, and more."
)]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Chat with the AI assistant (interactive or single message)
    Chat {
        /// Message to send (omit for interactive mode)
        message: Option<String>,

        /// Session ID for conversation continuity
        #[arg(short, long, default_value = "cli:default")]
        session: String,
    },

    /// Start the gateway daemon to enable channel integrations
    Serve {
        /// Port for the gateway service
        #[arg(short, long, default_value = "18790")]
        port: u16,

        /// Enable verbose debug logging
        #[arg(short, long)]
        verbose: bool,
    },

    /// Initialize klyntbot configuration and workspace
    Init,

    /// Display system status and configuration
    Status {
        /// Show detailed status including channel states
        #[arg(short, long)]
        verbose: bool,
    },

    /// Manage chat platform channels (Telegram, Discord, Slack, etc.)
    #[command(subcommand)]
    Channels(ChannelCommands),

    /// Schedule and manage automated jobs
    #[command(subcommand)]
    Cron(CronCommands),

    /// View and modify configuration settings
    #[command(subcommand)]
    Config(ConfigCommands),

    /// Manage and view available skills
    #[command(subcommand)]
    Skills(SkillsCommands),
}

#[derive(Subcommand, Debug)]
pub enum ChannelCommands {
    /// List all available channels and their configuration status
    List,

    /// Show setup instructions for a specific channel
    Login {
        /// Channel name: telegram, discord, whatsapp, slack, email, qq
        channel: String,
    },

    /// Test if a channel is properly configured and can connect
    Test {
        /// Channel name to test
        channel: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum CronCommands {
    /// List scheduled jobs
    List {
        /// Include disabled jobs
        #[arg(short, long)]
        all: bool,
    },

    /// Add a new scheduled job
    Add {
        /// Job name
        #[arg(short, long)]
        name: String,

        /// Message for the agent
        #[arg(short, long)]
        message: String,

        /// Run every N seconds
        #[arg(short, long)]
        every: Option<u64>,

        /// Cron expression (e.g., "0 9 * * *")
        #[arg(short, long)]
        cron: Option<String>,

        /// Run once at specified time (ISO format)
        #[arg(short, long)]
        at: Option<String>,

        /// Deliver response to channel
        #[arg(short, long)]
        deliver: bool,

        /// Recipient for delivery
        #[arg(short, long)]
        to: Option<String>,

        /// Channel for delivery
        #[arg(long)]
        channel: Option<String>,
    },

    /// Remove a scheduled job
    Remove {
        /// Job ID
        job_id: String,
    },

    /// Manually run a job
    Run {
        /// Job ID
        job_id: String,

        /// Force run even if disabled
        #[arg(short, long)]
        force: bool,
    },

    /// Enable a job
    Enable {
        /// Job ID
        job_id: String,
    },

    /// Disable a job
    Disable {
        /// Job ID
        job_id: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum ConfigCommands {
    /// Show current configuration
    Show,

    /// Get a configuration value
    Get {
        /// Configuration key (e.g., "agents.defaults.model")
        key: String,
    },

    /// Set a configuration value
    Set {
        /// Configuration key
        key: String,

        /// Value to set
        value: String,
    },

    /// Edit configuration in $EDITOR
    Edit,

    /// Reset to default configuration
    Reset {
        /// Skip confirmation
        #[arg(short, long)]
        force: bool,
    },

    /// Validate configuration file
    Validate,
}

#[derive(Subcommand, Debug)]
pub enum SkillsCommands {
    /// List all available skills with their status
    List,

    /// Display detailed information about a specific skill
    Info {
        /// Name of the skill to inspect
        name: String,
    },

    /// Show the filesystem path to the skills directory
    Path,
}
