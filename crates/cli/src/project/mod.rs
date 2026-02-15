//! Project management CLI commands.
//!
//! Implements all project subcommands including creation,
//! listing, updating, archiving, and task views.

mod create;
mod list;
mod tasks;
mod update;

use anyhow::Result;

use crate::commands::ProjectCommands;

/// Handle project subcommands.
pub async fn handle_project(cmd: ProjectCommands) -> Result<()> {
    match cmd {
        ProjectCommands::Create {
            name,
            description,
            color,
            tags,
        } => create::handle_create(name, description, color, tags).await,
        ProjectCommands::List { status, tag, limit } => list::handle_list(status, tag, limit).await,
        ProjectCommands::Show { id } => list::handle_show(&id).await,
        ProjectCommands::Update {
            id,
            name,
            description,
            color,
            status,
            tags,
        } => update::handle_update(id, name, description, color, status, tags).await,
        ProjectCommands::Archive { id } => update::handle_archive(&id).await,
        ProjectCommands::Tasks { id, limit, tree } => tasks::handle_tasks(id, limit, tree).await,
    }
}
