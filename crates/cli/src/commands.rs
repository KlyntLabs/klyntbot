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

        /// Show detailed thinking trace (tool args, token counts, timing)
        #[arg(short = 'V', long)]
        verbose: bool,
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
}
