//! Project update and archive commands.

use anyhow::Result;
use common::utils::terminal::*;
use tools::project_store::ProjectStore;
use tools::project_types::{ProjectColor, ProjectPatch, ProjectStatus};

/// Parse a color string into a ProjectColor enum.
fn parse_color(s: &str) -> Result<ProjectColor> {
    match s.to_lowercase().as_str() {
        "red" => Ok(ProjectColor::Red),
        "orange" => Ok(ProjectColor::Orange),
        "yellow" => Ok(ProjectColor::Yellow),
        "green" => Ok(ProjectColor::Green),
        "blue" => Ok(ProjectColor::Blue),
        "purple" => Ok(ProjectColor::Purple),
        "gray" | "grey" => Ok(ProjectColor::Gray),
        _ => Err(anyhow::anyhow!(
            "Invalid color: {}. Valid: red, orange, yellow, green, blue, purple, gray",
            s
        )),
    }
}

/// Parse a status string into a ProjectStatus enum.
fn parse_status(s: &str) -> Result<ProjectStatus> {
    match s.to_lowercase().as_str() {
        "active" => Ok(ProjectStatus::Active),
        "paused" => Ok(ProjectStatus::Paused),
        "completed" => Ok(ProjectStatus::Completed),
        "archived" => Ok(ProjectStatus::Archived),
        _ => Err(anyhow::anyhow!(
            "Invalid status: {}. Valid: active, paused, completed, archived",
            s
        )),
    }
}

pub async fn handle_update(
    id: String,
    name: Option<String>,
    description: Option<String>,
    color: Option<String>,
    status: Option<String>,
    tags: Option<String>,
) -> Result<()> {
    let config = config::load().await?;
    let store_path = config.project_store_path();
    let mut store = ProjectStore::new(store_path);

    let parsed_color = color.as_ref().map(|c| parse_color(c)).transpose()?;
    let parsed_status = status.as_ref().map(|s| parse_status(s)).transpose()?;

    let patch = ProjectPatch {
        name,
        description: description.map(Some),
        color: parsed_color,
        tags: tags.map(|t| t.split(',').map(|s| s.trim().to_string()).collect()),
        status: parsed_status,
    };

    let result = store.update(&id, patch).await?;

    match result {
        Some(project) => {
            println!(
                "{} Updated project {} — {}",
                status_success(),
                colorize(&project.id, TOOL),
                colorize(&project.name, BOLD),
            );
        }
        None => {
            println!("{} Project not found: {}", status_error(), id);
        }
    }

    Ok(())
}

pub async fn handle_archive(id: &str) -> Result<()> {
    let config = config::load().await?;
    let store_path = config.project_store_path();
    let mut store = ProjectStore::new(store_path);

    let patch = ProjectPatch {
        status: Some(ProjectStatus::Archived),
        ..Default::default()
    };

    let result = store.update(id, patch).await?;

    match result {
        Some(project) => {
            println!(
                "{} Archived project {} — {}",
                status_success(),
                colorize(&project.id, TOOL),
                colorize(&project.name, BOLD),
            );
        }
        None => {
            println!("{} Project not found: {}", status_error(), id);
        }
    }

    Ok(())
}
