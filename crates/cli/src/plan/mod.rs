//! Plan management CLI commands.

use anyhow::Result;
use common::utils::terminal::*;
use plan::store::PlanStore;
use plan::types::{Plan, PlanStatus};
use uuid::Uuid;

use crate::commands::PlanCommands;

/// Handle plan subcommands.
pub async fn handle_plan(cmd: PlanCommands) -> Result<()> {
    match cmd {
        PlanCommands::Create {
            title,
            description,
            goal_id,
        } => handle_create(title, description, goal_id).await,
        PlanCommands::Show { id } => handle_show(&id).await,
        PlanCommands::Approve { id } => handle_approve(&id).await,
        PlanCommands::Abandon { id } => handle_abandon(&id).await,
        PlanCommands::Status => handle_status().await,
    }
}

async fn handle_create(
    title: String,
    description: Option<String>,
    goal_id: Option<String>,
) -> Result<()> {
    use chrono::Utc;
    use plan::types::PlanStatus;

    let config = config::load().await?;
    let store_path = config.plan_store_path();
    let mut store = PlanStore::new(store_path);

    let parsed_goal_id = if let Some(gid) = goal_id {
        Some(Uuid::parse_str(&gid)?)
    } else {
        None
    };

    let now = Utc::now();
    let plan = Plan {
        id: Uuid::new_v4(),
        session_key: "cli:default".to_string(),
        goal_id: parsed_goal_id,
        title: title.clone(),
        description: description.unwrap_or_default(),
        status: PlanStatus::Draft,
        steps: vec![],
        current_step_index: 0,
        iteration_limit: 50,
        backtrack_history: vec![],
        created_at: now,
        updated_at: now,
        completed_at: None,
    };

    let saved = store.upsert(plan).await?;

    println!(
        "{} Plan created: {} ({})",
        status_success(),
        colorize(&saved.title, BOLD),
        colorize(&saved.id.to_string()[..8], TOOL)
    );
    println!("  Status: {:?}", saved.status);

    Ok(())
}

async fn handle_show(id: &str) -> Result<()> {
    let config = config::load().await?;
    let store_path = config.plan_store_path();
    let mut store = PlanStore::new(store_path);

    let uuid = find_plan_by_prefix(&mut store, id).await?;
    let plan: Plan = store.get(&uuid).await?.unwrap();

    println!("{}", colorize(&plan.title, BOLD));
    println!("  ID:       {}", colorize(&plan.id.to_string(), TOOL));
    println!("  Status:   {:?}", plan.status);
    println!("  Created:  {}", plan.created_at.format("%Y-%m-%d %H:%M"));

    if let Some(goal_id) = plan.goal_id {
        println!("  Goal:     {}", colorize(&goal_id.to_string()[..8], TOOL));
    }

    if !plan.description.is_empty() {
        println!("  Desc:     {}", plan.description);
    }

    println!("  Steps:    {}", plan.steps.len());
    println!(
        "  Progress: {}/{}",
        plan.current_step_index,
        plan.steps.len()
    );

    if !plan.steps.is_empty() {
        println!("\n  Steps:");
        for (idx, step) in plan.steps.iter().enumerate() {
            let marker = if idx == plan.current_step_index {
                ">>>"
            } else {
                "   "
            };
            println!(
                "    {} [{}] {}",
                marker,
                colorize(&format!("{:?}", step.status), TOOL),
                step.description
            );
        }
    }

    Ok(())
}

async fn handle_approve(id: &str) -> Result<()> {
    let config = config::load().await?;
    let store_path = config.plan_store_path();
    let mut store = PlanStore::new(store_path);

    let uuid = find_plan_by_prefix(&mut store, id).await?;
    let mut plan: Plan = store
        .get(&uuid)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Plan not found"))?;

    if plan.status != PlanStatus::Draft {
        println!(
            "{} Plan is not in Draft status (current: {:?})",
            status_warning(),
            plan.status
        );
        return Ok(());
    }

    plan.status = PlanStatus::Approved;
    store.upsert(plan.clone()).await?;

    println!(
        "{} Plan approved: {}",
        status_success(),
        colorize(&plan.title, BOLD)
    );

    Ok(())
}

async fn handle_abandon(id: &str) -> Result<()> {
    let config = config::load().await?;
    let store_path = config.plan_store_path();
    let mut store = PlanStore::new(store_path);

    let uuid = find_plan_by_prefix(&mut store, id).await?;
    let mut plan: Plan = store
        .get(&uuid)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Plan not found"))?;

    plan.status = PlanStatus::Abandoned;
    store.upsert(plan.clone()).await?;

    println!(
        "{} Plan abandoned: {}",
        status_success(),
        colorize(&plan.title, BOLD)
    );

    Ok(())
}

async fn handle_status() -> Result<()> {
    let config = config::load().await?;
    let store_path = config.plan_store_path();
    let mut store = PlanStore::new(store_path);

    let session_key = "cli:default";
    match store.get_active_plan(session_key).await? {
        Some(plan) => {
            println!(
                "{} Active plan: {}",
                status_active(),
                colorize(&plan.title, BOLD)
            );
            println!("  ID:       {}", colorize(&plan.id.to_string()[..8], TOOL));
            println!("  Status:   {:?}", plan.status);
            println!(
                "  Progress: {}/{}",
                plan.current_step_index,
                plan.steps.len()
            );
        }
        None => {
            println!("{} No active plan for this session.", status_active());
        }
    }

    Ok(())
}

/// Find a plan by ID (requires exact UUID for now).
/// TODO: Add prefix matching once PlanStore has a list/all method.
async fn find_plan_by_prefix(_store: &mut PlanStore, id_str: &str) -> Result<Uuid> {
    Uuid::parse_str(id_str)
        .map_err(|_| anyhow::anyhow!("Invalid plan ID: '{}'. Please provide full UUID.", id_str))
}
