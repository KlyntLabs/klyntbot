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
    Init {
        /// Jump directly to pack selection
        #[arg(long)]
        packs: bool,
        /// Reset configuration to defaults before running wizard
        #[arg(long)]
        reset: bool,
    },

    /// Display system status and configuration
    Status {
        /// Show detailed status including channel states
        #[arg(short, long)]
        verbose: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_no_flags() {
        let cli = Cli::parse_from(["klyntbot", "init"]);
        match cli.command {
            Some(Commands::Init { packs, reset }) => {
                assert!(!packs);
                assert!(!reset);
            }
            other => panic!("expected Init, got {other:?}"),
        }
    }

    #[test]
    fn init_packs_flag() {
        let cli = Cli::parse_from(["klyntbot", "init", "--packs"]);
        match cli.command {
            Some(Commands::Init { packs, reset }) => {
                assert!(packs);
                assert!(!reset);
            }
            other => panic!("expected Init, got {other:?}"),
        }
    }

    #[test]
    fn init_reset_flag() {
        let cli = Cli::parse_from(["klyntbot", "init", "--reset"]);
        match cli.command {
            Some(Commands::Init { packs, reset }) => {
                assert!(!packs);
                assert!(reset);
            }
            other => panic!("expected Init, got {other:?}"),
        }
    }

    #[test]
    fn init_both_flags() {
        let cli = Cli::parse_from(["klyntbot", "init", "--packs", "--reset"]);
        match cli.command {
            Some(Commands::Init { packs, reset }) => {
                assert!(packs);
                assert!(reset);
            }
            other => panic!("expected Init, got {other:?}"),
        }
    }
}
