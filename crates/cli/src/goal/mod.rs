//! Goal management CLI commands.

use anyhow::Result;
use chrono::{NaiveDate, Utc};
use common::utils::terminal::*;
use goal::store::GoalStore;
use goal::types::{Goal, GoalStatus};
use std::collections::HashMap;
use uuid::Uuid;

use crate::commands::GoalCommands;

/// Handle goal subcommands.
pub async fn handle_goal(cmd: GoalCommands) -> Result<()> {
    match cmd {
        GoalCommands::Create {
            title,
            description,
            priority,
            target_date,
            tags,
        } => handle_create(title, description, priority, target_date, tags).await,
        GoalCommands::List { status } => handle_list(status).await,
        GoalCommands::Show { id } => handle_show(&id).await,
        GoalCommands::Update {
            id,
            title,
            description,
            priority,
            status,
            target_date,
        } => handle_update(id, title, description, priority, status, target_date).await,
        GoalCommands::Delete { id } => handle_delete(&id).await,
        GoalCommands::Progress { id } => handle_progress(&id).await,
    }
}

async fn handle_create(
    title: String,
    description: Option<String>,
    priority: u8,
    target_date: Option<String>,
    tags: Option<String>,
) -> Result<()> {
    let config = config::load().await?;
    let store_path = config.goal_store_path();
    let mut store = GoalStore::new(store_path);

    let parsed_date = if let Some(date_str) = target_date {
        let naive = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")?;
        Some(naive.and_hms_opt(23, 59, 59).unwrap().and_utc())
    } else {
        None
    };

    let now = Utc::now();
    let goal = Goal {
        id: Uuid::new_v4(),
        title: title.clone(),
        description: description.unwrap_or_default(),
        status: GoalStatus::Active,
        priority,
        target_date: parsed_date,
        created_at: now,
        updated_at: now,
        metrics: vec![],
        linked_project_ids: vec![],
        metadata: tags
            .map(|t| {
                let tag_list: Vec<String> =
                    t.split(',').map(|s| s.trim().to_string()).collect();
                HashMap::from([("tags".to_string(), tag_list.join(","))])
            })
            .unwrap_or_default(),
    };

    let saved = store.add(goal).await?;
    println!(
        "{} Goal created: {} ({})",
        status_success(),
        colorize(&saved.title, BOLD),
        colorize(&saved.id.to_string()[..8], TOOL)
    );

    Ok(())
}

async fn handle_list(status_filter: Option<String>) -> Result<()> {
    let config = config::load().await?;
    let store_path = config.goal_store_path();
    let mut store = GoalStore::new(store_path);

    let filter_status = match status_filter.as_deref() {
        Some("active") => Some(GoalStatus::Active),
        Some("paused") => Some(GoalStatus::Paused),
        Some("achieved") => Some(GoalStatus::Achieved),
        Some("abandoned") => Some(GoalStatus::Abandoned),
        Some(other) => {
            println!(
                "{} Unknown status '{}'. Valid: active, paused, achieved, abandoned",
                status_warning(),
                other
            );
            None
        }
        None => None,
    };

    let goals = store.list(filter_status).await?;

    if goals.is_empty() {
        println!("{} No goals found.", status_active());
        return Ok(());
    }

    println!(
        "{} {} goal(s):\n",
        status_active(),
        goals.len()
    );

    for goal in &goals {
        let status_str = format!("{:?}", goal.status);
        println!(
            "  {} {} [{}] P{}",
            colorize(&goal.id.to_string()[..8], TOOL),
            colorize(&goal.title, BOLD),
            status_str,
            goal.priority
        );
    }

    Ok(())
}

async fn handle_show(id: &str) -> Result<()> {
    let config = config::load().await?;
    let store_path = config.goal_store_path();
    let mut store = GoalStore::new(store_path);

    let uuid = find_goal_by_prefix(&mut store, id).await?;
    let goal = store.get(&uuid).await?.unwrap();

    println!("{}", colorize(&goal.title, BOLD));
    println!("  ID:       {}", colorize(&goal.id.to_string(), TOOL));
    println!("  Status:   {:?}", goal.status);
    println!("  Priority: {}", goal.priority);
    println!("  Created:  {}", goal.created_at.format("%Y-%m-%d %H:%M"));

    if let Some(target) = goal.target_date {
        println!("  Target:   {}", target.format("%Y-%m-%d"));
    }

    if !goal.description.is_empty() {
        println!("  Desc:     {}", goal.description);
    }

    if !goal.metrics.is_empty() {
        println!("\n  Metrics:");
        for m in &goal.metrics {
            let pct = m.progress_percentage();
            println!(
                "    {} {}/{} {} ({:.0}%)",
                m.name, m.current, m.target, m.unit, pct
            );
        }
    }

    Ok(())
}

async fn handle_update(
    id: String,
    title: Option<String>,
    description: Option<String>,
    priority: Option<u8>,
    status: Option<String>,
    target_date: Option<String>,
) -> Result<()> {
    let config = config::load().await?;
    let store_path = config.goal_store_path();
    let mut store = GoalStore::new(store_path);

    let uuid = find_goal_by_prefix(&mut store, &id).await?;
    let mut goal = store
        .get(&uuid)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Goal not found"))?;

    if let Some(t) = title {
        goal.title = t;
    }
    if let Some(d) = description {
        goal.description = d;
    }
    if let Some(p) = priority {
        goal.priority = p;
    }
    if let Some(s) = status {
        goal.status = match s.as_str() {
            "active" => GoalStatus::Active,
            "paused" => GoalStatus::Paused,
            "achieved" => GoalStatus::Achieved,
            "abandoned" => GoalStatus::Abandoned,
            _ => {
                println!(
                    "{} Unknown status '{}'. Valid: active, paused, achieved, abandoned",
                    status_warning(),
                    s
                );
                goal.status
            }
        };
    }
    if let Some(date_str) = target_date {
        let naive = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")?;
        goal.target_date = Some(naive.and_hms_opt(23, 59, 59).unwrap().and_utc());
    }

    store.update(goal.clone()).await?;
    println!(
        "{} Goal updated: {}",
        status_success(),
        colorize(&goal.title, BOLD)
    );

    Ok(())
}

async fn handle_delete(id: &str) -> Result<()> {
    let config = config::load().await?;
    let store_path = config.goal_store_path();
    let mut store = GoalStore::new(store_path);

    let uuid = find_goal_by_prefix(&mut store, id).await?;

    if store.delete(&uuid).await? {
        println!("{} Goal deleted.", status_success());
    } else {
        println!("{} Goal not found.", status_warning());
    }

    Ok(())
}

async fn handle_progress(id: &str) -> Result<()> {
    let config = config::load().await?;
    let store_path = config.goal_store_path();
    let mut store = GoalStore::new(store_path);

    let uuid = find_goal_by_prefix(&mut store, id).await?;

    match store.calculate_progress(&uuid).await? {
        Some(progress) => {
            println!(
                "{} {:.0}% complete",
                status_active(),
                progress.completion_percentage
            );
            println!("  {}", progress.summary);
            for m in &progress.metrics {
                println!(
                    "    {} {}/{} {} ({:.0}%)",
                    m.name,
                    m.current,
                    m.target,
                    m.unit,
                    m.progress_percentage()
                );
            }
        }
        None => {
            println!("{} Goal not found.", status_warning());
        }
    }

    Ok(())
}

/// Find a goal by ID prefix (supports short IDs).
async fn find_goal_by_prefix(store: &mut GoalStore, prefix: &str) -> Result<Uuid> {
    // Try direct UUID parse first
    if let Ok(uuid) = Uuid::parse_str(prefix) {
        return Ok(uuid);
    }

    // Search by prefix
    let all = store.all().await?;
    let matches: Vec<&Goal> = all
        .iter()
        .filter(|g| g.id.to_string().starts_with(prefix))
        .collect();

    match matches.len() {
        0 => Err(anyhow::anyhow!("No goal found with ID prefix '{}'", prefix)),
        1 => Ok(matches[0].id),
        n => Err(anyhow::anyhow!(
            "Ambiguous ID prefix '{}' matches {} goals",
            prefix,
            n
        )),
    }
}
