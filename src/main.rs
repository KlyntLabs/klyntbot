use clap::Parser;

use klyntbot::cli::{Cli, Commands};

// Import CLI handlers
mod cli_handlers {
    pub use klyntbot::cli::chat::handle_chat;
    pub use klyntbot::cli::serve::handle_serve;
    pub use klyntbot::cli::status::{handle_brief_status, handle_status};
    pub use klyntbot::cli::channels::handle_channels;
    pub use klyntbot::cli::cron::handle_cron;
    pub use klyntbot::cli::config_cmd::handle_config;
    pub use klyntbot::cli::skills::handle_skills;
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    init_tracing(false);

    let cli = Cli::parse();

    let result = match cli.command {
        Some(Commands::Chat {
            message,
            session,
            no_markdown,
        }) => cli_handlers::handle_chat(message, session, !no_markdown).await,

        Some(Commands::Serve { port, verbose }) => {
            if verbose {
                init_tracing(true);
            }
            cli_handlers::handle_serve(port).await
        }

        Some(Commands::Init) => handle_init().await,

        Some(Commands::Status { verbose }) => cli_handlers::handle_status(verbose).await,

        Some(Commands::Channels(cmd)) => cli_handlers::handle_channels(cmd).await,

        Some(Commands::Cron(cmd)) => cli_handlers::handle_cron(cmd).await,

        Some(Commands::Config(cmd)) => cli_handlers::handle_config(cmd).await,

        Some(Commands::Skills(cmd)) => cli_handlers::handle_skills(cmd).await,

        None => {
            // No command specified, show brief status
            cli_handlers::handle_brief_status().await
        }
    };

    if let Err(e) = result {
        use klyntbot::utils::terminal::display_error;

        // Display structured error
        let error_msg = display_error(
            "Command execution failed",
            &e.to_string(),
            &[
                "Check the error message above for specific details",
                "Verify your configuration with: klyntbot config validate",
                "Run 'klyntbot --help' for available commands",
            ],
            None,
        );
        eprintln!("\n{}", error_msg);
        std::process::exit(1);
    }
}

/// Initialize tracing/logging
fn init_tracing(verbose: bool) {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

    let filter = if verbose {
        EnvFilter::new("klyntbot=debug,info")
    } else {
        EnvFilter::new("klyntbot=info")
    };

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().with_target(false))
        .init();
}

/// Handle init command
async fn handle_init() -> anyhow::Result<()> {
    use klyntbot::cli::run_wizard;

    // Run the interactive wizard
    run_wizard().await?;

    Ok(())
}
