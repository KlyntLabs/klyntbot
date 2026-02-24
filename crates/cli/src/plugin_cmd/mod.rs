//! `klyntbot plugin` subcommand — install, list, remove, search, update, new, publish.

pub mod install;
pub mod list;
pub mod new_plugin;
pub mod publish;
pub mod remove;
pub mod search;
pub mod update;

use clap::Subcommand;

#[derive(Subcommand, Debug, Clone)]
pub enum PluginCommand {
    /// Install a plugin from local file, GitHub release, or registry
    Install {
        /// Plugin source: `./path.wasm`, `github:user/repo`, or `plugin-id[@version]`
        source: String,
    },
    /// List installed plugins
    List,
    /// Remove an installed plugin
    Remove {
        /// Plugin ID to remove
        id: String,
    },
    /// Search the plugin registry
    Search {
        /// Search query
        query: String,
    },
    /// Update installed plugin(s) to latest version
    Update {
        /// Plugin ID (omit to update all)
        id: Option<String>,
    },
    /// Scaffold a new plugin project
    New {
        /// Plugin name
        name: String,
        /// Language: rust, typescript, or python
        #[arg(short, long, default_value = "rust")]
        lang: String,
    },
    /// Publish a plugin to the registry (opens PR on registry repo)
    Publish,
}

/// Resolve the plugins directory from config.
pub fn plugins_dir(config: &config::Config) -> std::path::PathBuf {
    config.data_dir_path().join("plugins")
}

/// Run a plugin subcommand.
pub async fn handle_plugin(cmd: PluginCommand, config: &config::Config) -> anyhow::Result<()> {
    let dir = plugins_dir(config);
    let registry_url = &config.plugins.registry_url;

    match cmd {
        PluginCommand::Install { source } => install::run(&source, &dir, registry_url).await,
        PluginCommand::List => list::run(&dir).await,
        PluginCommand::Remove { id } => remove::run(&id, &dir).await,
        PluginCommand::Search { query } => search::run(&query, registry_url).await,
        PluginCommand::Update { id } => update::run(id.as_deref(), &dir, registry_url).await,
        PluginCommand::New { name, lang } => new_plugin::run(&name, &lang).await,
        PluginCommand::Publish => publish::run().await,
    }
}
