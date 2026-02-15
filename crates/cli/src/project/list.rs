//! Project list and show commands.

use anyhow::Result;
use common::utils::terminal::*;
use tools::project_store::ProjectStore;
use tools::project_types::{Project, ProjectColor, ProjectFilter, ProjectStatus};

pub async fn handle_list(
    status: Option<String>,
    tag: Option<String>,
    limit: Option<usize>,
) -> Result<()> {
    let config = config::load().await?;
    let store_path = config.project_store_path();
    let mut store = ProjectStore::new(store_path);

    let filter = ProjectFilter {
        status: status.as_deref().and_then(parse_project_status),
        tag,
        limit,
    };

    let projects = store.list(&filter).await?;

    if projects.is_empty() {
        println!("{} No projects found.", status_warning());
        return Ok(());
    }

    // Header
    let filter_desc = match status.as_deref() {
        Some(s) => format!(" (status={})", s),
        None => String::new(),
    };
    println!(
        "{} {} project(s){}",
        colorize("Projects", BRAND),
        projects.len(),
        colorize(&filter_desc, DIM)
    );
    println!();

    for project in &projects {
        let color_dot = color_dot(project.color);
        let status_str = format!("{:?}", project.status);
        let tags_str = if project.tags.is_empty() {
            String::new()
        } else {
            format!("  {}", colorize(&project.tags.join(", "), DIM))
        };

        println!(
            "  {} {}  {}  {}{}",
            color_dot,
            colorize(&project.id, TOOL),
            colorize(&project.name, BOLD),
            colorize(&status_str, DIM),
            tags_str,
        );
    }

    Ok(())
}

pub async fn handle_show(id: &str) -> Result<()> {
    let config = config::load().await?;
    let store_path = config.project_store_path();
    let mut store = ProjectStore::new(store_path);

    let project = match store.get(id).await? {
        Some(p) => p,
        None => {
            println!("{} Project not found: {}", status_error(), id);
            return Ok(());
        }
    };

    show_project_details(&project);

    Ok(())
}

fn show_project_details(project: &Project) {
    println!(
        "{}",
        colorize("┌─ Project ──────────────────────────────────────┐", BRAND)
    );
    println!(
        "{}",
        colorize("│                                                │", BRAND)
    );
    println!(
        "{}  {}{}",
        colorize("│", BRAND),
        colorize(&project.name, BOLD),
        colorize("  │", BRAND)
    );
    println!(
        "{}  {}  ·  Status: {:?}  ·  Color: {}{}",
        colorize("│", BRAND),
        colorize(&project.id, TOOL),
        project.status,
        color_dot(project.color),
        colorize("  │", BRAND)
    );
    println!(
        "{}",
        colorize("│                                                │", BRAND)
    );

    if let Some(desc) = &project.description {
        println!(
            "{}  Description:{}",
            colorize("│", BRAND),
            colorize("  │", BRAND)
        );
        println!(
            "{}  {}{}",
            colorize("│", BRAND),
            desc,
            colorize("  │", BRAND)
        );
        println!(
            "{}",
            colorize("│                                                │", BRAND)
        );
    }

    if !project.tags.is_empty() {
        println!(
            "{}  Tags: {}{}",
            colorize("│", BRAND),
            colorize(&project.tags.join(", "), DIM),
            colorize("  │", BRAND)
        );
    }

    println!(
        "{}  Created: {}{}",
        colorize("│", BRAND),
        colorize(
            &project.created_at.format("%Y-%m-%d %H:%M").to_string(),
            DIM
        ),
        colorize("  │", BRAND)
    );

    println!(
        "{}",
        colorize("│                                                │", BRAND)
    );
    println!(
        "{}",
        colorize("└─────────────────────────────────────────────────┘", BRAND)
    );
}

fn parse_project_status(s: &str) -> Option<ProjectStatus> {
    match s {
        "active" => Some(ProjectStatus::Active),
        "paused" => Some(ProjectStatus::Paused),
        "completed" => Some(ProjectStatus::Completed),
        "archived" => Some(ProjectStatus::Archived),
        _ => None,
    }
}

fn color_dot(color: ProjectColor) -> String {
    let dot = "●";
    match color {
        ProjectColor::Red => colorize(dot, ERROR),
        ProjectColor::Orange => colorize(dot, BRAND),
        ProjectColor::Yellow => colorize(dot, WARNING),
        ProjectColor::Green => colorize(dot, SUCCESS),
        ProjectColor::Blue => colorize(dot, TOOL),
        ProjectColor::Purple => colorize(dot, BOLD),
        ProjectColor::Gray => colorize(dot, DIM),
    }
}
