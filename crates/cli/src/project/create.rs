//! Project creation command.

use anyhow::Result;
use chrono::Utc;
use common::utils::terminal::*;
use tools::project_store::ProjectStore;
use tools::project_types::{Project, ProjectColor, ProjectStatus};

pub async fn handle_create(
    name: String,
    description: Option<String>,
    color: Option<String>,
    tags: Option<String>,
) -> Result<()> {
    let config = config::load().await?;
    let store_path = config.project_store_path();
    let mut store = ProjectStore::new(store_path);

    let project_color = match color.as_deref() {
        Some("red") => ProjectColor::Red,
        Some("orange") => ProjectColor::Orange,
        Some("yellow") => ProjectColor::Yellow,
        Some("green") => ProjectColor::Green,
        Some("blue") => ProjectColor::Blue,
        Some("purple") => ProjectColor::Purple,
        Some("gray") => ProjectColor::Gray,
        Some(other) => {
            println!(
                "{} Unknown color '{}', using orange. Valid: red, orange, yellow, green, blue, purple, gray",
                status_warning(),
                other
            );
            ProjectColor::Orange
        }
        None => ProjectColor::default(),
    };

    let project = Project {
        id: Project::generate_id(),
        name: name.clone(),
        description,
        color: project_color,
        tags: tags
            .as_ref()
            .map(|t| t.split(',').map(|s| s.trim().to_string()).collect())
            .unwrap_or_default(),
        status: ProjectStatus::Active,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    let saved = store.add(project).await?;

    show_project_created_box(&saved);

    Ok(())
}

fn show_project_created_box(project: &Project) {
    println!(
        "{}",
        colorize(
            "┌─ ✓ Project Created ────────────────────────────┐",
            SUCCESS
        )
    );
    println!(
        "{}",
        colorize(
            "│                                                │",
            SUCCESS
        )
    );
    println!(
        "{}  {}{}",
        colorize("│", SUCCESS),
        colorize(&project.name, BOLD),
        colorize("  │", SUCCESS)
    );
    println!(
        "{}  ID: {}{}",
        colorize("│", SUCCESS),
        colorize(&project.id, TOOL),
        colorize("  │", SUCCESS)
    );
    println!(
        "{}  Color: {:?}  ·  Status: {:?}{}",
        colorize("│", SUCCESS),
        project.color,
        project.status,
        colorize("  │", SUCCESS)
    );

    if let Some(desc) = &project.description {
        println!(
            "{}  {}{}",
            colorize("│", SUCCESS),
            &desc.chars().take(44).collect::<String>(),
            colorize("  │", SUCCESS)
        );
    }

    if !project.tags.is_empty() {
        println!(
            "{}  Tags: {}{}",
            colorize("│", SUCCESS),
            colorize(&project.tags.join(", "), DIM),
            colorize("  │", SUCCESS)
        );
    }

    println!(
        "{}",
        colorize(
            "│                                                │",
            SUCCESS
        )
    );
    println!(
        "{}",
        colorize(
            "└─────────────────────────────────────────────────┘",
            SUCCESS
        )
    );
}
