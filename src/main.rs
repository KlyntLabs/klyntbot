use clap::Parser;

use cli::{Cli, Commands};

// Import CLI handlers
mod cli_handlers {
    pub use cli::calendar::handle_calendar;
    pub use cli::channels::handle_channels;
    pub use cli::chat::handle_chat;
    pub use cli::config_cmd::handle_config;
    pub use cli::cron::handle_cron;
    pub use cli::goal::handle_goal;
    pub use cli::learning_cmd::handle_learning;
    pub use cli::plan::handle_plan;
    pub use cli::project::handle_project;
    pub use cli::serve::handle_serve;
    pub use cli::skills::handle_skills;
    pub use cli::status::{handle_brief_status, handle_status};
    pub use cli::todo::handle_todo;
    pub use cli::provider_cmd::handle_provider;
    pub use cli::usage_cmd::handle_usage;
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Initialize tracing based on command context
    // Chat mode suppresses INFO logs for a clean REPL experience
    let log_level = match &cli.command {
        Some(Commands::Serve { verbose: true, .. }) => "debug",
        Some(Commands::Chat { .. }) => "warn",
        _ => "info",
    };
    init_tracing(log_level);

    let result = match cli.command {
        Some(Commands::Chat { message, session }) => {
            cli_handlers::handle_chat(message, session).await
        }

        Some(Commands::Serve { port, .. }) => cli_handlers::handle_serve(port).await,

        Some(Commands::Init) => handle_init().await,

        Some(Commands::Status { verbose }) => cli_handlers::handle_status(verbose).await,

        Some(Commands::Channels(cmd)) => cli_handlers::handle_channels(cmd).await,

        Some(Commands::Cron(cmd)) => cli_handlers::handle_cron(cmd).await,

        Some(Commands::Config(cmd)) => cli_handlers::handle_config(cmd).await,

        Some(Commands::Skills(cmd)) => cli_handlers::handle_skills(cmd).await,

        Some(Commands::Todo(cmd)) => cli_handlers::handle_todo(cmd).await,

        Some(Commands::Project(cmd)) => cli_handlers::handle_project(cmd).await,

        Some(Commands::Calendar(cmd)) => cli_handlers::handle_calendar(cmd).await,

        Some(Commands::Goal(cmd)) => cli_handlers::handle_goal(cmd).await,

        Some(Commands::Plan(cmd)) => cli_handlers::handle_plan(cmd).await,

        Some(Commands::Usage(cmd)) => cli_handlers::handle_usage(cmd).await,

        Some(Commands::Learning(cmd)) => cli_handlers::handle_learning(cmd).await,

        Some(Commands::Provider(cmd)) => cli_handlers::handle_provider(cmd).await,

        None => {
            // No command specified, show brief status
            cli_handlers::handle_brief_status().await
        }
    };

    if let Err(e) = result {
        use common::utils::terminal::display_error;

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

/// Initialize tracing/logging with the given level (e.g. "info", "warn", "debug")
fn init_tracing(level: &str) {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

    let filter = EnvFilter::new(format!("klyntbot={}", level));

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().with_target(false))
        .init();
}

/// Handle init command
async fn handle_init() -> anyhow::Result<()> {
    use cli::run_wizard;

    // Run the interactive wizard
    run_wizard().await?;

    Ok(())
}
