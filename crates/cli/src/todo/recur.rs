//! CLI handlers for `klyntbot todo recur` — recurring task template management.

use anyhow::Result;
use chrono::Utc;
use common::utils::terminal::*;
use tools::rrule_utils;
use tools::todo_store::TodoStore;
use tools::todo_types::{Todo, TodoStatus};

use crate::commands::RecurCommands;

/// Route recur subcommands to their handlers.
pub async fn handle_recur(cmd: RecurCommands) -> Result<()> {
    match cmd {
        RecurCommands::Add {
            title,
            rule,
            description,
            priority,
            tags,
            project,
        } => handle_recur_add(title, rule, description, priority, tags, project).await,
        RecurCommands::List { project } => handle_recur_list(project).await,
        RecurCommands::Delete { id } => handle_recur_delete(&id).await,
    }
}

async fn handle_recur_add(
    title: String,
    rule: String,
    description: Option<String>,
    priority: Option<u8>,
    tags: Option<Vec<String>>,
    project: Option<String>,
) -> Result<()> {
    // Validate RRULE
    if let Err(e) = rrule_utils::validate_rrule(&rule) {
        println!("{} Invalid RRULE: {}", status_error(), e);
        return Ok(());
    }

    // Calculate first occurrence
    let now = Utc::now();
    let next_instance_date = rrule_utils::next_occurrence(&rule, now)?;

    let config = config::load().await?;
    let store_path = config.todo_store_path();
    let mut store = TodoStore::new(store_path);

    let todo = Todo {
        id: Todo::generate_id(),
        title,
        description,
        priority,
        due_date: None,
        tags: tags.unwrap_or_default(),
        status: TodoStatus::Todo,
        focused_at: None,
        focus_deadline: None,
        focus_expired_count: 0,
        created_at: now,
        updated_at: now,
        completed_at: None,
        parent_id: None,
        project_id: project,
        attachments: Vec::new(),
        time_entries: Vec::new(),
        total_tracked_secs: 0,
        estimated_minutes: None,
        calendar_event_uid: None,
        last_reminded_at: None,
        recurrence_rule: Some(rule.clone()),
        recurrence_parent_id: None,
        is_template: true,
        next_instance_date,
        blocked_by: Vec::new(),
        blocks: Vec::new(),
    };

    let created = store.add(todo).await?;
    let human = rrule_utils::humanize_rrule(&rule);

    println!(
        "{} Recurring template created: {}",
        status_success(),
        colorize(&created.title, BOLD),
    );
    println!("  ID:       {}", colorize(&created.id, TOOL));
    println!("  Schedule: {}", colorize(&human, BRAND));
    println!("  Rule:     {}", colorize(&rule, DIM));

    if let Some(next) = next_instance_date {
        println!(
            "  Next:     {}",
            colorize(&next.format("%Y-%m-%d %H:%M UTC").to_string(), BRAND)
        );
    }

    Ok(())
}

async fn handle_recur_list(project: Option<String>) -> Result<()> {
    let config = config::load().await?;
    let store_path = config.todo_store_path();
    let mut store = TodoStore::new(store_path);

    let templates = store.list_templates().await?;

    // Apply optional project filter
    let filtered: Vec<&Todo> = templates
        .iter()
        .filter(|t| {
            if let Some(ref pid) = project {
                t.project_id.as_ref() == Some(pid)
            } else {
                true
            }
        })
        .collect();

    if filtered.is_empty() {
        println!(
            "{} No recurring task templates found.",
            colorize("ℹ", BRAND)
        );
        return Ok(());
    }

    println!(
        "{} {} recurring template(s):\n",
        colorize("⟳", BRAND),
        filtered.len()
    );

    for t in &filtered {
        let rule = t.recurrence_rule.as_deref().unwrap_or("(no rule)");
        let human = rrule_utils::humanize_rrule(rule);
        let next = t
            .next_instance_date
            .map(|d| d.format("%Y-%m-%d %H:%M UTC").to_string())
            .unwrap_or_else(|| "(not scheduled)".to_string());

        println!(
            "  {} {} — {}",
            colorize(&t.id, TOOL),
            colorize(&t.title, BOLD),
            colorize(&human, BRAND),
        );
        println!(
            "    Next: {}  Rule: {}",
            colorize(&next, DIM),
            colorize(rule, DIM),
        );

        if let Some(ref desc) = t.description {
            println!("    {}", colorize(desc, DIM));
        }
    }

    Ok(())
}

async fn handle_recur_delete(id: &str) -> Result<()> {
    let config = config::load().await?;
    let store_path = config.todo_store_path();
    let mut store = TodoStore::new(store_path);

    let template = match store.get(id).await? {
        Some(t) => t,
        None => {
            println!("{} Template not found: {}", status_error(), id);
            return Ok(());
        }
    };

    if !template.is_template {
        println!(
            "{} Task {} is not a recurring template. Use `todo delete` for regular tasks.",
            status_error(),
            colorize(id, TOOL),
        );
        return Ok(());
    }

    let title = template.title.clone();
    store.delete(id).await?;

    println!(
        "{} Recurring template deleted: {} ({})",
        status_success(),
        colorize(&title, BOLD),
        colorize(id, TOOL),
    );
    println!(
        "  {}",
        colorize("Existing instances are now standalone tasks.", DIM)
    );

    Ok(())
}
