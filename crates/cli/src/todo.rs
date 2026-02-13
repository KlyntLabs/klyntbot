//! Todo task management CLI commands.
//!
//! Implements all todo subcommands including task creation with confidence
//! scoring, Phase 1/2 enrichment flow, focus mode, and dashboard rendering.

use anyhow::Result;
use chrono::Utc;
use common::utils::terminal::*;
use tools::todo_store::TodoStore;
use tools::todo_types::{Todo, TodoFilter, TodoPatch, TodoStatus};

use crate::commands::TodoCommands;
use crate::wizard::prompts::{prompt_optional, prompt_select_with_input, SelectWithInputOption};

/// Handle todo subcommands.
///
/// Routes to the appropriate handler based on the subcommand variant.
pub async fn handle_todo(cmd: TodoCommands) -> Result<()> {
    match cmd {
        TodoCommands::Add {
            title,
            description,
            priority,
            due,
            tags,
        } => handle_add(title, description, priority, due, tags).await,
        TodoCommands::List {
            status,
            tag,
            priority_min,
            limit,
        } => handle_list(status, tag, priority_min, limit).await,
        TodoCommands::Show { id } => handle_show(&id).await,
        TodoCommands::Complete { id } => handle_complete(&id).await,
        TodoCommands::Delete { id } => handle_delete(&id).await,
        TodoCommands::Focus { id } => handle_focus(id).await,
        TodoCommands::Unfocus { id } => handle_unfocus(&id).await,
        TodoCommands::Summary => handle_summary().await,
    }
}

async fn handle_add(
    title: String,
    description: Option<String>,
    priority: Option<u8>,
    due: Option<String>,
    tags: Option<String>,
) -> Result<()> {
    // Load config and create store
    let config = config::load()?;
    let store_path = config.todo_store_path();
    let mut store = TodoStore::new(store_path);

    // Create initial todo from args
    let mut todo = Todo {
        id: Todo::generate_id(),
        title: title.clone(),
        description: description.clone(),
        priority,
        due_date: due.as_ref().and_then(|d| parse_due_date(d).ok()),
        tags: tags
            .as_ref()
            .map(|t| t.split(',').map(|s| s.trim().to_string()).collect())
            .unwrap_or_default(),
        status: TodoStatus::Todo,
        confidence: 0.0, // Will be calculated
        focused_at: None,
        focus_deadline: None,
        focus_expired_count: 0,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        completed_at: None,
    };

    // Calculate initial confidence
    let initial_confidence = todo.calculate_confidence();
    todo.confidence = initial_confidence;

    // Phase 1: Pre-creation enrichment (if confidence < threshold)
    if initial_confidence < config.todo.confidence_threshold {
        println!("Creating task: \"{}\"", colorize(&title, BOLD));
        println!();

        // Show confidence bar
        println!(
            "Confidence: {}% {}",
            (initial_confidence * 100.0) as u8,
            render_confidence_bar(initial_confidence)
        );
        println!(
            "             {} title only, no description/priority/date/tags",
            colorize("↑", DIM)
        );
        println!();
        println!("How would you like to improve it?");
        println!();

        let options = vec![
            SelectWithInputOption {
                label: "Yes — looks good",
                description: "",
                expandable: true,
                input_hint: Some("Add more detail to help the task"),
            },
            SelectWithInputOption {
                label: "Manual — I'll fill the details",
                description: "",
                expandable: true,
                input_hint: Some("Description"),
            },
            SelectWithInputOption {
                label: "YOLO — auto-enrich from context & memory",
                description: "",
                expandable: false,
                input_hint: None,
            },
            SelectWithInputOption {
                label: "Party — let's brainstorm together",
                description: "",
                expandable: false,
                input_hint: None,
            },
            SelectWithInputOption {
                label: "Skip — create as-is (low confidence)",
                description: "",
                expandable: false,
                input_hint: None,
            },
        ];

        let result = prompt_select_with_input("", &options, 0)?;

        match result.index {
            0 => {
                // "Yes — looks good" (with optional hint)
                if let Some(hint) = result.text {
                    if !hint.is_empty() {
                        todo = enrich_with_ai_hint(&todo, &hint).await?;
                    }
                }
            }
            1 => {
                // "Manual" — either with description from expanded input or full form
                if let Some(desc) = result.text {
                    if !desc.is_empty() {
                        todo.description = Some(desc);
                    }
                }

                // Always prompt for remaining fields in manual mode
                let priority_str = prompt_optional("Priority (1-5)")?;
                if let Some(p) = priority_str {
                    if let Ok(pri) = p.parse::<u8>() {
                        if (1..=5).contains(&pri) {
                            todo.priority = Some(pri);
                        }
                    }
                }

                let due_str = prompt_optional("Due date (YYYY-MM-DD)")?;
                if let Some(d) = due_str {
                    if let Ok(due_date) = parse_due_date(&d) {
                        todo.due_date = Some(due_date);
                    }
                }

                let tags_str = prompt_optional("Tags (comma-separated)")?;
                if let Some(t) = tags_str {
                    todo.tags = t.split(',').map(|s| s.trim().to_string()).collect();
                }
            }
            2 => {
                // "YOLO" — AI auto-enrichment
                println!("{} Auto-enriching task...", colorize("⣾", BRAND));
                todo = enrich_with_ai_auto(&todo).await?;
            }
            3 => {
                // "Party" — conversational brainstorm
                todo = enrich_with_ai_party(&todo).await?;
            }
            4 => {
                // "Skip" — create as-is
                // No enrichment
            }
            _ => unreachable!(),
        }

        // Recalculate confidence after enrichment
        todo.confidence = todo.calculate_confidence();
    }

    // Save to store
    let saved_todo = store.add(todo.clone()).await?;

    // Phase 2: Post-creation review
    show_task_created_box(&saved_todo);

    // TODO: Implement Phase 2 review loop (improve/confirm)

    Ok(())
}

async fn handle_list(
    status: Option<String>,
    tag: Option<String>,
    priority_min: Option<u8>,
    limit: Option<usize>,
) -> Result<()> {
    // Load config and create store
    let config = config::load()?;
    let store_path = config.todo_store_path();
    let mut store = TodoStore::new(store_path);

    // Parse status filter
    let status_filter = status
        .as_ref()
        .and_then(|s| match s.to_lowercase().as_str() {
            "todo" => Some(TodoStatus::Todo),
            "doing" => Some(TodoStatus::Doing),
            "done" => Some(TodoStatus::Done),
            "archived" => Some(TodoStatus::Archived),
            _ => None,
        });

    // Build filter
    let filter = TodoFilter {
        status: status_filter,
        tag: tag.clone(),
        priority_min,
        limit,
    };

    // List todos
    let todos = store.list(&filter).await?;

    if todos.is_empty() {
        println!("No tasks found.");
        return Ok(());
    }

    // P2: Count header with filter description
    let header = if status.is_some() || tag.is_some() || priority_min.is_some() {
        let mut filters = Vec::new();
        if let Some(s) = &status {
            filters.push(format!("status={}", s));
        }
        if let Some(t) = &tag {
            filters.push(format!("tag={}", t));
        }
        if let Some(p) = priority_min {
            filters.push(format!("priority≥{}", p));
        }
        format!("{} tasks matching: {}", todos.len(), filters.join(", "))
    } else {
        format!("{} tasks:", todos.len())
    };
    println!("{}", header);
    println!();

    // Render list
    for todo in todos {
        print!("{} ", colorize(&todo.id, TOOL));

        // P1: Focus indicator
        if todo.focused_at.is_some() {
            print!("{} ", colorize("●", BRAND));
        } else {
            print!("  ");
        }

        print!("{}", colorize(&todo.title, BOLD));

        if let Some(pri) = todo.priority {
            print!("  {}", render_priority(pri));
        }

        if !todo.tags.is_empty() {
            print!("  {}", colorize(&todo.tags.join(", "), DIM));
        }

        if let Some(due) = &todo.due_date {
            let now = Utc::now();
            if *due < now {
                print!("  {}", colorize("OVERDUE", ERROR));
            } else {
                print!("  {}", colorize(&due.format("%b %d").to_string(), DIM));
            }
        }

        // P1: Confidence column
        let conf_pct = (todo.confidence * 100.0) as u8;
        let conf_color = if todo.confidence >= 0.8 {
            SUCCESS
        } else if todo.confidence >= 0.5 {
            BRAND
        } else {
            WARNING
        };
        print!("  {}", colorize(&format!("{}%", conf_pct), conf_color));

        println!();
    }

    Ok(())
}

async fn handle_show(id: &str) -> Result<()> {
    // Load config and create store
    let config = config::load()?;
    let store_path = config.todo_store_path();
    let mut store = TodoStore::new(store_path);

    // Get the todo
    let todo = store.get(id).await?;

    match todo {
        Some(t) => {
            show_task_details(&t);
            Ok(())
        }
        None => {
            println!("{} Task not found: {}", status_error(), id);
            Ok(())
        }
    }
}

async fn handle_complete(id: &str) -> Result<()> {
    // Load config and create store
    let config = config::load()?;
    let store_path = config.todo_store_path();
    let mut store = TodoStore::new(store_path);

    // Update status to Done
    let patch = TodoPatch {
        status: Some(TodoStatus::Done),
        ..Default::default()
    };

    let result = store.update(id, patch).await?;

    match result {
        Some(todo) => {
            println!(
                "{} Completed: {} — {}",
                status_success(),
                colorize(&todo.id, TOOL),
                todo.title
            );
            Ok(())
        }
        None => {
            println!("{} Task not found: {}", status_error(), id);
            Ok(())
        }
    }
}

async fn handle_delete(id: &str) -> Result<()> {
    // Load config and create store
    let config = config::load()?;
    let store_path = config.todo_store_path();
    let mut store = TodoStore::new(store_path);

    // Delete the task
    let deleted = store.delete(id).await?;

    if deleted {
        println!("{} Deleted task: {}", status_success(), colorize(id, TOOL));
    } else {
        println!("{} Task not found: {}", status_error(), id);
    }

    Ok(())
}

async fn handle_focus(id: Option<String>) -> Result<()> {
    // Load config and create store
    let config = config::load()?;
    let store_path = config.todo_store_path();
    let mut store = TodoStore::new(store_path);

    if let Some(task_id) = id {
        // Focus on a specific task
        let focused = store
            .focus(
                &task_id,
                config.todo.focus.max_slots,
                config.todo.focus.deadline_hours,
            )
            .await?;

        if focused {
            // Get the todo to show success message
            if let Some(todo) = store.get(&task_id).await? {
                println!(
                    "{} Focused: {} — {}",
                    status_success(),
                    colorize(&todo.id, TOOL),
                    todo.title
                );
            }
            Ok(())
        } else {
            // Focus failed - either not found or slots full
            println!("{} Task not found or focus slots full", status_error());
            Ok(())
        }
    } else {
        // Show focus board
        let focused = store.focused().await?;

        if focused.is_empty() {
            println!("No tasks in focus.");
            println!();
            println!("Focus on a task: klyntbot todo focus <id>");
            return Ok(());
        }

        // Render focus board
        println!("{}", colorize("Focus Board", BRAND));
        println!();

        for todo in focused {
            show_focused_task(&todo, config.todo.focus.deadline_hours);
            println!();
        }

        Ok(())
    }
}

async fn handle_unfocus(id: &str) -> Result<()> {
    // Load config and create store
    let config = config::load()?;
    let store_path = config.todo_store_path();
    let mut store = TodoStore::new(store_path);

    // Unfocus the task
    let unfocused = store.unfocus(id).await?;

    if unfocused {
        println!("{} Unfocused: {}", status_success(), colorize(id, TOOL));
    } else {
        println!("{} Task not found: {}", status_error(), id);
    }

    Ok(())
}

async fn handle_summary() -> Result<()> {
    // Load config and create store
    let config = config::load()?;
    let store_path = config.todo_store_path();
    let mut store = TodoStore::new(store_path);

    // Get summary
    let summary = store.summary().await?;

    println!("{}", colorize("Todo Summary", BRAND));
    println!();

    println!("Total tasks: {}", summary.total);
    println!();

    // By status
    println!("By Status:");
    for (status, count) in &summary.by_status {
        println!("  {:?}: {}", status, count);
    }
    println!();

    // Overdue
    if !summary.overdue.is_empty() {
        println!(
            "{} {} overdue tasks:",
            status_warning(),
            summary.overdue.len()
        );
        for id in &summary.overdue {
            println!("  {}", colorize(id, TOOL));
        }
        println!();
    }

    // Low confidence
    if !summary.low_confidence.is_empty() {
        println!(
            "{} {} low confidence tasks:",
            status_warning(),
            summary.low_confidence.len()
        );
        for id in &summary.low_confidence {
            println!("  {}", colorize(id, TOOL));
        }
        println!();
    }

    // Upcoming
    if !summary.upcoming_week.is_empty() {
        println!("Due this week: {}", summary.upcoming_week.len());
        for id in &summary.upcoming_week {
            println!("  {}", colorize(id, TOOL));
        }
    }

    Ok(())
}

// Helper functions

/// Parse a due date string (YYYY-MM-DD) to DateTime<Utc>
fn parse_due_date(date_str: &str) -> Result<chrono::DateTime<Utc>> {
    use chrono::NaiveDate;

    let naive = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")?;
    let naive_datetime = naive
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| anyhow::anyhow!("Invalid date"))?;

    Ok(chrono::DateTime::<Utc>::from_naive_utc_and_offset(
        naive_datetime,
        Utc,
    ))
}

/// Render a confidence bar (10 chars)
fn render_confidence_bar(confidence: f32) -> String {
    let filled = (confidence * 10.0) as usize;
    let empty = 10 - filled;

    let color = if confidence >= 0.8 {
        SUCCESS
    } else if confidence >= 0.5 {
        BRAND
    } else {
        WARNING
    };

    let bar = format!("{}{}", "■".repeat(filled), "░".repeat(empty));
    colorize(&bar, color)
}

/// Show task created box with details
fn show_task_created_box(todo: &Todo) {
    println!(
        "{}",
        colorize("┌─ ✓ Task Created ──────────────────────────────┐", SUCCESS)
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
        colorize(&todo.title, BOLD),
        colorize("  │", SUCCESS)
    );
    println!(
        "{}  ID: {}{}",
        colorize("│", SUCCESS),
        colorize(&todo.id, TOOL),
        colorize("                                      │", SUCCESS)
    );
    println!(
        "{}",
        colorize(
            "│                                                │",
            SUCCESS
        )
    );

    // Priority
    if let Some(pri) = todo.priority {
        println!(
            "{}  Priority:    {}{}",
            colorize("│", SUCCESS),
            render_priority(pri),
            colorize("                          │", SUCCESS)
        );
    }

    // Due date
    if let Some(due) = &todo.due_date {
        println!(
            "{}  Due:         {}{}",
            colorize("│", SUCCESS),
            due.format("%Y-%m-%d"),
            colorize("                              │", SUCCESS)
        );
    }

    // Tags
    if !todo.tags.is_empty() {
        println!(
            "{}  Tags:        {}{}",
            colorize("│", SUCCESS),
            colorize(&todo.tags.join(", "), DIM),
            colorize("                    │", SUCCESS)
        );
    }

    // Description
    if let Some(desc) = &todo.description {
        println!(
            "{}  Description: {}{}",
            colorize("│", SUCCESS),
            &desc.chars().take(36).collect::<String>(),
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
        "{}  Confidence:  {}% {}{}",
        colorize("│", SUCCESS),
        (todo.confidence * 100.0) as u8,
        render_confidence_bar(todo.confidence),
        colorize("                    │", SUCCESS)
    );
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

/// Render priority badge
fn render_priority(priority: u8) -> String {
    let (color, blocks) = match priority {
        5 => (ERROR, "■■■■■"),
        4 => (ERROR, "■■■■○"),
        3 => (WARNING, "■■■○○"),
        2 => (DIM, "■■○○○"),
        1 => (DIM, "■○○○○"),
        _ => (DIM, "○○○○○"),
    };

    let badge = format!("{} P{}", blocks, priority);
    colorize(&badge, color)
}

// AI enrichment stubs (to be implemented with actual AI integration)

async fn enrich_with_ai_hint(todo: &Todo, hint: &str) -> Result<Todo> {
    // TODO: Call AI with hint to enrich the task
    // For now, just return the todo unchanged
    println!("TODO: Implement AI enrichment with hint: {}", hint);
    Ok(todo.clone())
}

async fn enrich_with_ai_auto(todo: &Todo) -> Result<Todo> {
    // TODO: Call AI to auto-generate all fields from context
    println!("TODO: Implement AI auto-enrichment");
    Ok(todo.clone())
}

async fn enrich_with_ai_party(todo: &Todo) -> Result<Todo> {
    // TODO: Call AI with persona to ask questions
    println!("TODO: Implement AI party mode");
    Ok(todo.clone())
}

/// Show focused task in focus board
fn show_focused_task(todo: &Todo, deadline_hours: u64) {
    println!(
        "{}",
        colorize(
            "┌─ ● Focused ──────────────────────────────────────────┐",
            BRAND
        )
    );
    println!(
        "{}",
        colorize(
            "│                                                       │",
            BRAND
        )
    );
    println!(
        "{}  {}  {}{}",
        colorize("│", BRAND),
        colorize(&todo.id, TOOL),
        colorize(&todo.title, BOLD),
        colorize("  │", BRAND)
    );

    // Priority, tags, due
    print!("{}  ", colorize("│", BRAND));
    if let Some(pri) = todo.priority {
        print!("{} ", render_priority(pri));
    }
    if !todo.tags.is_empty() {
        print!(" · {} ", colorize(&todo.tags.join(", "), DIM));
    }
    if let Some(due) = &todo.due_date {
        print!(
            " · Due: {}",
            colorize(&due.format("%b %d").to_string(), DIM)
        );
    }
    println!("{}", colorize("  │", BRAND));

    println!(
        "{}",
        colorize(
            "│                                                       │",
            BRAND
        )
    );

    // Focus timer
    if let (Some(focused_at), Some(deadline)) = (&todo.focused_at, &todo.focus_deadline) {
        let now = Utc::now();
        let elapsed = now.signed_duration_since(*focused_at);
        let remaining = deadline.signed_duration_since(now);

        let total_secs = deadline_hours * 3600;
        let elapsed_secs = elapsed.num_seconds();
        let progress = (elapsed_secs as f32 / total_secs as f32).min(1.0);

        let bar_width = 20;
        let filled = (progress * bar_width as f32) as usize;
        let empty = bar_width - filled;

        let color = if remaining.num_hours() > 6 {
            BRAND
        } else if remaining.num_hours() > 1 {
            WARNING
        } else {
            ERROR
        };

        let bar = format!("{}{}", "█".repeat(filled), "░".repeat(empty));

        println!(
            "{}  Time: {} {}{}",
            colorize("│", BRAND),
            colorize(&bar, color),
            if remaining.num_seconds() > 0 {
                format!(
                    "{}h {}m remaining",
                    remaining.num_hours(),
                    remaining.num_minutes() % 60
                )
            } else {
                colorize("EXPIRED", ERROR)
            },
            colorize("  │", BRAND)
        );
    }

    // Description
    if let Some(desc) = &todo.description {
        println!(
            "{}",
            colorize(
                "│                                                       │",
                BRAND
            )
        );
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
    }

    println!(
        "{}",
        colorize(
            "│                                                       │",
            BRAND
        )
    );
    println!(
        "{}",
        colorize(
            "└───────────────────────────────────────────────────────┘",
            BRAND
        )
    );
}

/// Render confidence breakdown with field-by-field analysis
fn render_confidence_breakdown(todo: &Todo) {
    println!(
        "{}",
        colorize(
            "│                                                         │",
            BRAND
        )
    );

    // Title quality (0.25)
    let title_words = todo.title.split_whitespace().count();
    let has_title = title_words > 3;
    let title_icon = if has_title {
        colorize("✓", SUCCESS)
    } else {
        colorize("○", DIM)
    };
    println!(
        "{}    {} Title quality    (25%)  — {} words{}",
        colorize("│", BRAND),
        title_icon,
        title_words,
        colorize("  │", BRAND)
    );

    // Description (0.25)
    let has_desc = todo.description.as_ref().is_some_and(|d| d.len() > 10);
    let desc_icon = if has_desc {
        colorize("✓", SUCCESS)
    } else {
        colorize("○", DIM)
    };
    let desc_text = if let Some(desc) = &todo.description {
        format!("{} chars", desc.len())
    } else {
        "not set".to_string()
    };
    println!(
        "{}    {} Description      (25%)  — {}{}",
        colorize("│", BRAND),
        desc_icon,
        desc_text,
        colorize("  │", BRAND)
    );

    // Priority (0.15)
    let has_priority = todo.priority.is_some();
    let priority_icon = if has_priority {
        colorize("✓", SUCCESS)
    } else {
        colorize("○", DIM)
    };
    let priority_text = if let Some(p) = todo.priority {
        format!("P{}", p)
    } else {
        "not set".to_string()
    };
    println!(
        "{}    {} Priority set     (15%)  — {}{}",
        colorize("│", BRAND),
        priority_icon,
        priority_text,
        colorize("  │", BRAND)
    );

    // Due date (0.20)
    let has_due = todo.due_date.is_some();
    let due_icon = if has_due {
        colorize("✓", SUCCESS)
    } else {
        colorize("○", DIM)
    };
    let due_text = if let Some(due) = &todo.due_date {
        due.format("%Y-%m-%d").to_string()
    } else {
        "not set".to_string()
    };
    println!(
        "{}    {} Due date         (20%)  — {}{}",
        colorize("│", BRAND),
        due_icon,
        due_text,
        colorize("  │", BRAND)
    );

    // Tags (0.15)
    let has_tags = !todo.tags.is_empty();
    let tags_icon = if has_tags {
        colorize("✓", SUCCESS)
    } else {
        colorize("○", DIM)
    };
    let tags_text = if has_tags {
        todo.tags.join(", ")
    } else {
        "none".to_string()
    };
    println!(
        "{}    {} Tags             (15%)  — {}{}",
        colorize("│", BRAND),
        tags_icon,
        tags_text,
        colorize("  │", BRAND)
    );

    // Suggestions (only if confidence < 80%)
    if todo.confidence < 0.8 {
        println!(
            "{}",
            colorize(
                "│                                                         │",
                BRAND
            )
        );
        println!(
            "{}    {}{}",
            colorize("│", BRAND),
            colorize("Suggestions:", DIM),
            colorize("  │", BRAND)
        );

        if !has_title {
            println!(
                "{}      {} Add more detail to title (+25%){}",
                colorize("│", BRAND),
                colorize("→", BRAND),
                colorize("  │", BRAND)
            );
        }
        if !has_desc {
            println!(
                "{}      {} Add a description (+25%){}",
                colorize("│", BRAND),
                colorize("→", BRAND),
                colorize("  │", BRAND)
            );
        }
        if !has_priority {
            println!(
                "{}      {} Set a priority level (+15%){}",
                colorize("│", BRAND),
                colorize("→", BRAND),
                colorize("  │", BRAND)
            );
        }
        if !has_due {
            println!(
                "{}      {} Add a due date (+20%){}",
                colorize("│", BRAND),
                colorize("→", BRAND),
                colorize("  │", BRAND)
            );
        }
        if !has_tags {
            println!(
                "{}      {} Add tags (+15%){}",
                colorize("│", BRAND),
                colorize("→", BRAND),
                colorize("  │", BRAND)
            );
        }
    }
}

/// Show detailed task view
fn show_task_details(todo: &Todo) {
    println!(
        "{}",
        colorize(
            "┌─ Task ─────────────────────────────────────────────────┐",
            BRAND
        )
    );
    println!(
        "{}",
        colorize(
            "│                                                         │",
            BRAND
        )
    );
    println!(
        "{}  {}{}",
        colorize("│", BRAND),
        colorize(&todo.title, BOLD),
        colorize("  │", BRAND)
    );
    println!(
        "{}  {}  ·  Status: {:?}  ·  Created: {}{}",
        colorize("│", BRAND),
        colorize(&todo.id, TOOL),
        todo.status,
        todo.created_at.format("%b %d"),
        colorize("  │", BRAND)
    );
    println!(
        "{}",
        colorize(
            "│                                                         │",
            BRAND
        )
    );

    // Description
    if let Some(desc) = &todo.description {
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
            colorize(
                "│                                                         │",
                BRAND
            )
        );
    }

    // Priority
    if let Some(pri) = todo.priority {
        println!(
            "{}  Priority:    {}{}",
            colorize("│", BRAND),
            render_priority(pri),
            colorize("  │", BRAND)
        );
    }

    // Due date
    if let Some(due) = &todo.due_date {
        let now = Utc::now();
        let due_str = if *due < now {
            colorize(&format!("{} (OVERDUE)", due.format("%Y-%m-%d")), ERROR)
        } else {
            colorize(&due.format("%Y-%m-%d").to_string(), DIM)
        };
        println!(
            "{}  Due:         {}{}",
            colorize("│", BRAND),
            due_str,
            colorize("  │", BRAND)
        );
    }

    // Tags
    if !todo.tags.is_empty() {
        println!(
            "{}  Tags:        {}{}",
            colorize("│", BRAND),
            colorize(&todo.tags.join(", "), DIM),
            colorize("  │", BRAND)
        );
    }

    // Confidence
    println!(
        "{}  Confidence:  {}% {}{}",
        colorize("│", BRAND),
        (todo.confidence * 100.0) as u8,
        render_confidence_bar(todo.confidence),
        colorize("  │", BRAND)
    );

    // P3: Confidence breakdown
    render_confidence_breakdown(todo);

    println!(
        "{}",
        colorize(
            "│                                                         │",
            BRAND
        )
    );
    println!(
        "{}",
        colorize(
            "└─────────────────────────────────────────────────────────┘",
            BRAND
        )
    );
}
