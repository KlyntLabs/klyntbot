//! Deadline scheduler initialization — wires the event-driven timer into AppCore.
//!
//! Three responsibilities:
//!
//! 1. **Handler closure** — dispatches fired [`DeadlineAction`] variants:
//!    sends OS notifications, marks tasks reminded, unfocuses expired tasks,
//!    and spawns recurring task instances.
//!
//! 2. **Event subscriber** — listens to [`DomainEventBus`] and feeds the scheduler
//!    whenever a task's due date, focus deadline, or recurring template changes.
//!
//! 3. **Startup population** — scans existing data to seed the scheduler on boot.

use std::sync::Arc;

use bus::{DomainEvent, DomainEventBus};
use chrono::{DateTime, Duration, Utc};
use scheduling::{DeadlineAction, DeadlineScheduler};
use tracing::{debug, info, warn};

/// Create, start, and populate the deadline scheduler.
///
/// Returns `Arc<DeadlineScheduler>` to be stored in `AppCore`.
pub async fn init_deadline_scheduler(
    repos: &storage::Repos,
    notification_dispatcher: &Arc<agent::NotificationDispatcher>,
    domain_event_bus: &Arc<DomainEventBus>,
    config: &config::Config,
    shutdown_token: &tokio_util::sync::CancellationToken,
) -> Arc<DeadlineScheduler> {
    let scheduler = Arc::new(DeadlineScheduler::new(build_handler(
        repos.tasks.clone(),
        Arc::clone(notification_dispatcher),
        config.todo.focus.deadline_hours,
        config.timezone.clone(),
        Arc::clone(domain_event_bus),
    )));

    scheduler.start().await;

    // Spawn event subscriber.
    spawn_event_subscriber(
        Arc::clone(&scheduler),
        Arc::clone(domain_event_bus),
        repos.tasks.clone(),
        shutdown_token.clone(),
    );

    // Spawn startup population (background, non-blocking).
    spawn_startup_population(
        Arc::clone(&scheduler),
        repos.tasks.clone(),
        config.todo.focus.deadline_hours,
    );

    info!(
        "DeadlineScheduler initialized and started (pending: {})",
        scheduler.pending_count().await
    );

    scheduler
}

// ---------------------------------------------------------------------------
// 1. Handler closure
// ---------------------------------------------------------------------------

fn build_handler(
    todo_repo: storage::TaskRepo,
    dispatcher: Arc<agent::NotificationDispatcher>,
    deadline_hours: u64,
    timezone: String,
    domain_event_bus: Arc<DomainEventBus>,
) -> scheduling::DeadlineHandler {
    let rt = tokio::runtime::Handle::current();

    Arc::new(move |action: DeadlineAction| {
        let todo_repo = todo_repo.clone();
        let dispatcher = Arc::clone(&dispatcher);
        let timezone = timezone.clone();
        let domain_event_bus = Arc::clone(&domain_event_bus);

        tokio::task::block_in_place(|| {
            rt.block_on(async move {
                match action {
                    DeadlineAction::TaskReminder { task_id, label } => {
                        handle_task_reminder(&todo_repo, &dispatcher, &task_id, &label).await;
                    }
                    DeadlineAction::FocusWarning {
                        task_id,
                        hours_left,
                    } => {
                        handle_focus_warning(&todo_repo, &dispatcher, &task_id, hours_left).await;
                    }
                    DeadlineAction::FocusExpire { task_id } => {
                        handle_focus_expire(&todo_repo, &dispatcher, &task_id, deadline_hours)
                            .await;
                    }
                    DeadlineAction::SpawnRecurring { template_id } => {
                        handle_spawn_recurring(
                            &todo_repo,
                            &timezone,
                            &template_id,
                            &domain_event_bus,
                        )
                        .await;
                    }
                }
            })
        });
    })
}

async fn handle_task_reminder(
    repo: &storage::TaskRepo,
    dispatcher: &agent::NotificationDispatcher,
    task_id: &str,
    label: &str,
) {
    // Check task still exists, not done, and not already reminded.
    let task = match repo.get(task_id).await {
        Ok(Some(t)) => t,
        Ok(None) => {
            debug!("deadline: task {task_id} not found, skipping reminder");
            return;
        }
        Err(e) => {
            warn!("deadline: failed to fetch task {task_id}: {e}");
            return;
        }
    };

    if task.completed || task.status == "done" {
        debug!("deadline: task {task_id} already completed, skipping reminder");
        return;
    }

    if task.last_reminded_at.is_some() {
        debug!("deadline: task {task_id} already reminded, skipping");
        return;
    }

    // Send notification.
    if let Err(e) = dispatcher.notify("Task due soon", label).await {
        warn!("deadline: notification failed for task {task_id}: {e}");
    }

    // Mark as reminded.
    let patch = storage::TaskPatch {
        id: task_id.to_string(),
        last_reminded_at: Some(Some(Utc::now())),
        ..Default::default()
    };
    if let Err(e) = repo.update(&patch).await {
        warn!("deadline: failed to mark task {task_id} as reminded: {e}");
    }
}

async fn handle_focus_warning(
    repo: &storage::TaskRepo,
    dispatcher: &agent::NotificationDispatcher,
    task_id: &str,
    hours_left: u32,
) {
    // Check task is still focused.
    let task = match repo.get(task_id).await {
        Ok(Some(t)) => t,
        Ok(None) => return,
        Err(e) => {
            warn!("deadline: failed to fetch task {task_id}: {e}");
            return;
        }
    };

    if task.focused_at.is_none() {
        debug!("deadline: task {task_id} no longer focused, skipping warning");
        return;
    }

    let msg = format!("Focus deadline in {}h: {}", hours_left, task.title);
    if let Err(e) = dispatcher.notify("Focus deadline approaching", &msg).await {
        warn!("deadline: focus warning notification failed for {task_id}: {e}");
    }
}

async fn handle_focus_expire(
    repo: &storage::TaskRepo,
    dispatcher: &agent::NotificationDispatcher,
    task_id: &str,
    deadline_hours: u64,
) {
    // Check focus_deadline is actually past.
    let task = match repo.get(task_id).await {
        Ok(Some(t)) => t,
        Ok(None) => return,
        Err(e) => {
            warn!("deadline: failed to fetch task {task_id}: {e}");
            return;
        }
    };

    match task.focus_deadline {
        Some(deadline) if deadline <= Utc::now() => {
            // Unfocus the task.
            if let Err(e) = repo.unfocus(task_id).await {
                warn!("deadline: failed to unfocus task {task_id}: {e}");
                return;
            }

            let msg = format!("Focus expired after {}h: {}", deadline_hours, task.title);
            if let Err(e) = dispatcher.notify("Focus expired", &msg).await {
                warn!("deadline: expire notification failed for {task_id}: {e}");
            }
        }
        _ => {
            debug!("deadline: task {task_id} focus deadline not yet past, skipping expire");
        }
    }
}

async fn handle_spawn_recurring(
    _repo: &storage::TaskRepo,
    _timezone: &str,
    template_id: &str,
    _domain_event_bus: &DomainEventBus,
) {
    // Recurring task spawning removed with feature-tasks crate.
    debug!("deadline: recurring spawn skipped for template {template_id} (feature-tasks removed)");
}

// ---------------------------------------------------------------------------
// 2. Event subscriber
// ---------------------------------------------------------------------------

fn spawn_event_subscriber(
    scheduler: Arc<DeadlineScheduler>,
    domain_event_bus: Arc<DomainEventBus>,
    todo_repo: storage::TaskRepo,
    shutdown_token: tokio_util::sync::CancellationToken,
) {
    let mut rx = domain_event_bus.subscribe();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = shutdown_token.cancelled() => {
                    debug!("deadline event subscriber shutting down");
                    break;
                }
                result = rx.recv() => {
                    match result {
                        Ok(event) => {
                            handle_domain_event(&scheduler, &todo_repo, event).await;
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            warn!("deadline event subscriber lagged by {n} events");
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            debug!("deadline event subscriber: bus closed");
                            break;
                        }
                    }
                }
            }
        }
    });
}

async fn handle_domain_event(
    scheduler: &DeadlineScheduler,
    todo_repo: &storage::TaskRepo,
    event: DomainEvent,
) {
    match event {
        DomainEvent::TaskDueDateChanged { task_id, due_date } => {
            // Cancel any existing reminder for this task.
            scheduler
                .cancel_by_prefix(&format!("task_reminder:{task_id}"))
                .await;

            if let Some(due_str) = due_date {
                if let Ok(due) = due_str.parse::<DateTime<Utc>>() {
                    // Fetch the task title for the notification label.
                    let label = match todo_repo.get(&task_id).await {
                        Ok(Some(t)) => t.title,
                        _ => task_id.clone(),
                    };

                    // Schedule reminder 2 hours before due date.
                    let remind_at = due - Duration::hours(2);
                    if remind_at > Utc::now() {
                        scheduler
                            .schedule(remind_at, DeadlineAction::TaskReminder { task_id, label })
                            .await;
                    }
                }
            }
        }

        DomainEvent::TaskFocusChanged {
            task_id,
            focus_deadline,
        } => {
            // Cancel all existing focus timers for this task.
            scheduler
                .cancel_by_prefix(&format!("focus_warning:{task_id}:"))
                .await;
            scheduler
                .cancel_by_key(&format!("focus_expire:{task_id}"))
                .await;

            if let Some(dl_str) = focus_deadline {
                if let Ok(deadline) = dl_str.parse::<DateTime<Utc>>() {
                    let now = Utc::now();

                    // Schedule warnings at 6h, 3h, 1h before expiry.
                    for hours in [6u32, 3, 1] {
                        let warn_at = deadline - Duration::hours(hours as i64);
                        if warn_at > now {
                            scheduler
                                .schedule(
                                    warn_at,
                                    DeadlineAction::FocusWarning {
                                        task_id: task_id.clone(),
                                        hours_left: hours,
                                    },
                                )
                                .await;
                        }
                    }

                    // Schedule the expire action.
                    scheduler
                        .schedule(deadline, DeadlineAction::FocusExpire { task_id })
                        .await;
                }
            }
        }

        DomainEvent::RecurringTemplateAdvanced {
            template_id,
            next_instance_date,
        } => {
            // Cancel existing spawn timer for this template.
            scheduler
                .cancel_by_key(&format!("spawn_recurring:{template_id}"))
                .await;

            if let Some(date_str) = next_instance_date {
                if let Ok(next_date) = date_str.parse::<DateTime<Utc>>() {
                    if next_date > Utc::now() {
                        scheduler
                            .schedule(next_date, DeadlineAction::SpawnRecurring { template_id })
                            .await;
                    }
                }
            }
        }

        // Ignore all other events.
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// 3. Startup population
// ---------------------------------------------------------------------------

fn spawn_startup_population(
    scheduler: Arc<DeadlineScheduler>,
    todo_repo: storage::TaskRepo,
    deadline_hours: u64,
) {
    tokio::spawn(async move {
        // Small delay to let the app finish starting.
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        let mut scheduled = 0u32;

        // ── Tasks with due dates (active, not completed) ──
        if let Ok(tasks) = todo_repo
            .list(&storage::TaskFilter {
                completed: Some(false),
                ..Default::default()
            })
            .await
        {
            let now = Utc::now();
            for task in &tasks {
                if let Some(due) = task.due_date {
                    // Skip already-reminded tasks.
                    if task.last_reminded_at.is_some() {
                        continue;
                    }
                    let remind_at = due - Duration::hours(2);
                    if remind_at > now {
                        scheduler
                            .schedule(
                                remind_at,
                                DeadlineAction::TaskReminder {
                                    task_id: task.id.clone(),
                                    label: task.title.clone(),
                                },
                            )
                            .await;
                        scheduled += 1;
                    }
                }
            }
        }

        // ── Focused tasks with deadlines ──
        if let Ok(focused) = todo_repo.list_focused().await {
            let now = Utc::now();
            for task in &focused {
                if let Some(deadline) = task.focus_deadline {
                    // Schedule warnings at 6h, 3h, 1h before expiry.
                    for hours in [6u32, 3, 1] {
                        let warn_at = deadline - Duration::hours(hours as i64);
                        if warn_at > now {
                            scheduler
                                .schedule(
                                    warn_at,
                                    DeadlineAction::FocusWarning {
                                        task_id: task.id.clone(),
                                        hours_left: hours,
                                    },
                                )
                                .await;
                            scheduled += 1;
                        }
                    }

                    // Schedule expire action.
                    if deadline > now {
                        scheduler
                            .schedule(
                                deadline,
                                DeadlineAction::FocusExpire {
                                    task_id: task.id.clone(),
                                },
                            )
                            .await;
                        scheduled += 1;
                    }
                }
            }
        }

        // ── Recurring templates with next_instance_date ──
        if let Ok(templates) = todo_repo.list_templates().await {
            let now = Utc::now();
            let _ = deadline_hours; // available if needed for future use
            for tpl in &templates {
                if let Some(next) = tpl.next_instance_date {
                    if next > now {
                        scheduler
                            .schedule(
                                next,
                                DeadlineAction::SpawnRecurring {
                                    template_id: tpl.id.clone(),
                                },
                            )
                            .await;
                        scheduled += 1;
                    }
                }
            }
        }

        info!("DeadlineScheduler startup population: {scheduled} deadlines scheduled");
    });
}
