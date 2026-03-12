use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "klyntbot-mcp", version, about = "Klyntbot MCP server")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Start the MCP server
    Serve {
        /// Use stdio transport (for Claude Code / IDE integration)
        #[arg(long, group = "transport")]
        stdio: bool,

        /// Use HTTP transport
        #[arg(long, group = "transport")]
        http: bool,

        /// HTTP port (default: from config or 3100)
        #[arg(long)]
        port: Option<u16>,

        /// HTTP host (default: from config or 127.0.0.1)
        #[arg(long)]
        host: Option<String>,
    },
    /// Inspect available tools
    Tools {
        /// List all available tools
        #[arg(long)]
        list: bool,

        /// Show schema for a specific tool
        #[arg(long)]
        schema: Option<String>,
    },
}
