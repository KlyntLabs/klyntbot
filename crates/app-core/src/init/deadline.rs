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
use scheduling::{DeadlineAction, DeadlineScheduler};
use tracing::{debug, info, warn};

/// Convenience: publish a deadline notification through the dispatcher pipeline
/// (quiet hours, idempotency, retry) by emitting `AlarmFired`.
fn notify_alarm(
    bus: &DomainEventBus,
    task_id: Option<String>,
    title: impl Into<String>,
    body: impl Into<String>,
) {
    let payload = serde_json::json!({
        "title": title.into(),
        "body": body.into(),
        "channel_mask": 0,
        "priority_override": null,
    })
    .to_string();
    bus.publish(DomainEvent::AlarmFired {
        fire_id: uuid::Uuid::new_v4().to_string(),
        kind: "deadline_legacy".to_string(),
        ref_id: task_id,
        payload_json: payload,
        fired_at_ms: jiff::Timestamp::now().as_millisecond(),
    });
}

/// Create, start, and populate the deadline scheduler.
///
/// Returns `Arc<DeadlineScheduler>` to be stored in `AppCore`.
pub async fn init_deadline_scheduler(
    repos: &storage::Repos,
    domain_event_bus: &Arc<DomainEventBus>,
    config: &config::Config,
    shutdown_token: &tokio_util::sync::CancellationToken,
) -> Arc<DeadlineScheduler> {
    let scheduler = Arc::new(DeadlineScheduler::new(build_handler(
        repos.tasks.clone(),
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
    deadline_hours: u64,
    timezone: String,
    domain_event_bus: Arc<DomainEventBus>,
) -> scheduling::DeadlineHandler {
    let rt = tokio::runtime::Handle::current();

    Arc::new(move |action: DeadlineAction| {
        let todo_repo = todo_repo.clone();
        let timezone = timezone.clone();
        let domain_event_bus = Arc::clone(&domain_event_bus);

        tokio::task::block_in_place(|| {
            rt.block_on(async move {
                match action {
                    DeadlineAction::TaskReminder { task_id, label } => {
                        handle_task_reminder(&todo_repo, &domain_event_bus, &task_id, &label).await;
                    }
                    DeadlineAction::FocusWarning {
                        task_id,
                        hours_left,
                    } => {
                        handle_focus_warning(&todo_repo, &domain_event_bus, &task_id, hours_left)
                            .await;
                    }
                    DeadlineAction::FocusExpire { task_id } => {
                        handle_focus_expire(
                            &todo_repo,
                            &domain_event_bus,
                            &task_id,
                            deadline_hours,
                        )
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
    bus: &DomainEventBus,
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
    notify_alarm(bus, Some(task_id.to_string()), "Task due soon", label);

    // Mark as reminded.
    let patch = storage::TaskPatch {
        id: task_id.to_string(),
        last_reminded_at: Some(Some(jiff::Timestamp::now())),
        ..Default::default()
    };
    if let Err(e) = repo.update(&patch).await {
        warn!("deadline: failed to mark task {task_id} as reminded: {e}");
    }
}

async fn handle_focus_warning(
    repo: &storage::TaskRepo,
    bus: &DomainEventBus,
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
    notify_alarm(
        bus,
        Some(task_id.to_string()),
        "Focus deadline approaching",
        &msg,
    );
}

async fn handle_focus_expire(
    repo: &storage::TaskRepo,
    bus: &DomainEventBus,
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
        Some(deadline) if *deadline <= jiff::Timestamp::now() => {
            // Unfocus the task.
            if let Err(e) = repo.unfocus(task_id).await {
                warn!("deadline: failed to unfocus task {task_id}: {e}");
                return;
            }

            let msg = format!("Focus expired after {}h: {}", deadline_hours, task.title);
            notify_alarm(bus, Some(task_id.to_string()), "Focus expired", &msg);
        }
        _ => {
            debug!("deadline: task {task_id} focus deadline not yet past, skipping expire");
        }
    }
}

async fn handle_spawn_recurring(
    repo: &storage::TaskRepo,
    timezone: &str,
    template_id: &str,
    domain_event_bus: &DomainEventBus,
) {
    match agent::RecurringTaskSpawner::check_and_spawn_static(repo, timezone).await {
        Ok(()) => {
            debug!("deadline: recurring spawn check complete for template {template_id}");

            // Re-read the template to get the updated next_instance_date and re-schedule.
            if let Ok(Some(tpl)) = repo.get(template_id).await {
                let next_date = tpl.next_instance_date.map(|d| d.to_string());
                domain_event_bus.publish(DomainEvent::RecurringTemplateAdvanced {
                    template_id: template_id.to_string(),
                    next_instance_date: next_date,
                });
            }
        }
        Err(e) => {
            warn!("deadline: recurring spawn failed for template {template_id}: {e}");
        }
    }
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
                if let Ok(due) = due_str.parse::<jiff::Timestamp>() {
                    // Fetch the task title for the notification label.
                    let label = match todo_repo.get(&task_id).await {
                        Ok(Some(t)) => t.title,
                        _ => task_id.clone(),
                    };

                    // Schedule reminder 2 hours before due date.
                    let remind_at = due
                        .checked_sub(jiff::SignedDuration::from_secs(7200))
                        .unwrap_or(due);
                    if remind_at > jiff::Timestamp::now() {
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
                if let Ok(deadline) = dl_str.parse::<jiff::Timestamp>() {
                    let now = jiff::Timestamp::now();

                    // Schedule warnings at 6h, 3h, 1h before expiry.
                    for hours in [6u32, 3, 1] {
                        let warn_at = deadline
                            .checked_sub(jiff::SignedDuration::from_secs(hours as i64 * 3600))
                            .unwrap_or(deadline);
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
                if let Ok(next_date) = date_str.parse::<jiff::Timestamp>() {
                    if next_date > jiff::Timestamp::now() {
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
            let now = jiff::Timestamp::now();
            for task in &tasks {
                if let Some(due) = task.due_date {
                    // Skip already-reminded tasks.
                    if task.last_reminded_at.is_some() {
                        continue;
                    }
                    let remind_at = due
                        .checked_sub(jiff::SignedDuration::from_secs(7200))
                        .unwrap_or(*due);
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
            let now = jiff::Timestamp::now();
            for task in &focused {
                if let Some(deadline) = task.focus_deadline {
                    // Schedule warnings at 6h, 3h, 1h before expiry.
                    for hours in [6u32, 3, 1] {
                        let warn_at = deadline
                            .checked_sub(jiff::SignedDuration::from_secs(hours as i64 * 3600))
                            .unwrap_or(*deadline);
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
                    if *deadline > now {
                        scheduler
                            .schedule(
                                *deadline,
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
            let now = jiff::Timestamp::now();
            let _ = deadline_hours; // available if needed for future use
            for tpl in &templates {
                if let Some(next) = tpl.next_instance_date {
                    if *next > now {
                        scheduler
                            .schedule(
                                *next,
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
