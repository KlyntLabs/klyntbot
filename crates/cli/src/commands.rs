//! CLI command definitions using clap.

use clap::{Parser, Subcommand};

use crate::plugin_cmd::PluginCommand;

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

    /// Manage WASM plugins (install, list, remove, search, update, new, publish)
    #[command(subcommand)]
    Plugin(PluginCommand),
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

    #[test]
    fn plugin_install_local() {
        let cli = Cli::parse_from(["klyntbot", "plugin", "install", "./my-plugin.wasm"]);
        match cli.command {
            Some(Commands::Plugin(PluginCommand::Install { source })) => {
                assert_eq!(source, "./my-plugin.wasm");
            }
            other => panic!("expected Plugin Install, got {other:?}"),
        }
    }

    #[test]
    fn plugin_list() {
        let cli = Cli::parse_from(["klyntbot", "plugin", "list"]);
        assert!(matches!(
            cli.command,
            Some(Commands::Plugin(PluginCommand::List))
        ));
    }

    #[test]
    fn plugin_remove() {
        let cli = Cli::parse_from(["klyntbot", "plugin", "remove", "notion-connector"]);
        match cli.command {
            Some(Commands::Plugin(PluginCommand::Remove { id })) => {
                assert_eq!(id, "notion-connector");
            }
            other => panic!("expected Plugin Remove, got {other:?}"),
        }
    }

    #[test]
    fn plugin_search() {
        let cli = Cli::parse_from(["klyntbot", "plugin", "search", "notion"]);
        match cli.command {
            Some(Commands::Plugin(PluginCommand::Search { query })) => {
                assert_eq!(query, "notion");
            }
            other => panic!("expected Plugin Search, got {other:?}"),
        }
    }

    #[test]
    fn plugin_update_specific() {
        let cli = Cli::parse_from(["klyntbot", "plugin", "update", "my-plugin"]);
        match cli.command {
            Some(Commands::Plugin(PluginCommand::Update { id })) => {
                assert_eq!(id.as_deref(), Some("my-plugin"));
            }
            other => panic!("expected Plugin Update, got {other:?}"),
        }
    }

    #[test]
    fn plugin_update_all() {
        let cli = Cli::parse_from(["klyntbot", "plugin", "update"]);
        match cli.command {
            Some(Commands::Plugin(PluginCommand::Update { id })) => {
                assert!(id.is_none());
            }
            other => panic!("expected Plugin Update, got {other:?}"),
        }
    }

    #[test]
    fn plugin_new_default_lang() {
        let cli = Cli::parse_from(["klyntbot", "plugin", "new", "my-plugin"]);
        match cli.command {
            Some(Commands::Plugin(PluginCommand::New { name, lang })) => {
                assert_eq!(name, "my-plugin");
                assert_eq!(lang, "rust");
            }
            other => panic!("expected Plugin New, got {other:?}"),
        }
    }

    #[test]
    fn plugin_new_typescript() {
        let cli = Cli::parse_from([
            "klyntbot",
            "plugin",
            "new",
            "my-plugin",
            "--lang",
            "typescript",
        ]);
        match cli.command {
            Some(Commands::Plugin(PluginCommand::New { name, lang })) => {
                assert_eq!(name, "my-plugin");
                assert_eq!(lang, "typescript");
            }
            other => panic!("expected Plugin New, got {other:?}"),
        }
    }

    #[test]
    fn plugin_publish() {
        let cli = Cli::parse_from(["klyntbot", "plugin", "publish"]);
        assert!(matches!(
            cli.command,
            Some(Commands::Plugin(PluginCommand::Publish))
        ));
    }
}
