//! DashboardEventBus — broadcast event distribution for the web dashboard.
//!
//! Mirrors the `LearningEventBus` pattern from `crates/bus`: a thin
//! `tokio::sync::broadcast` wrapper that fans out events to multiple
//! independent subscribers (WebSocket connections, GraphQL subscriptions).

pub mod store_events;

use tokio::sync::broadcast;

use agent::AgentEvent;

/// Actions that can happen to a store entity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeAction {
    Created,
    Updated,
    Deleted,
}

/// Events distributed by the dashboard event bus.
#[derive(Debug, Clone)]
pub enum DashboardEvent {
    /// Forwarded agent loop event (streaming output, tool calls, etc.)
    AgentEvent(AgentEvent),

    /// A todo item was created, updated, or deleted.
    TodoChanged { action: ChangeAction, id: String },

    /// A project was created, updated, or deleted.
    ProjectChanged { action: ChangeAction, id: String },

    /// A goal was created, updated, or deleted.
    GoalChanged { action: ChangeAction, id: String },

    /// A plan was created, updated, or deleted.
    PlanChanged { action: ChangeAction, id: String },

    /// The config file was reloaded successfully.
    ConfigReloaded,

    /// A notification to display to the user.
    Notification { title: String, body: String },

    /// A cron job executed.
    CronJobRan { name: String, result: Option<String> },
}

/// Broadcast bus for `DashboardEvent`s.
///
/// Clone or pass an `Arc<DashboardEventBus>` to producers and consumers.
/// Each `subscribe()` call returns an independent receiver.
pub struct DashboardEventBus {
    tx: broadcast::Sender<DashboardEvent>,
}

impl DashboardEventBus {
    /// Create a new bus with the specified channel capacity.
    ///
    /// Capacity of 256 handles burst traffic from store mutations and
    /// agent streaming without dropping events under normal load.
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }

    /// Publish an event to all current subscribers.
    ///
    /// Returns the number of active subscribers that received the event.
    /// No-op (does not error) when there are no active subscribers.
    pub fn publish(&self, event: DashboardEvent) -> usize {
        self.tx.send(event).unwrap_or(0)
    }

    /// Subscribe to the event stream.
    ///
    /// Each subscriber gets its own independent copy of every event
    /// published after this call.
    pub fn subscribe(&self) -> broadcast::Receiver<DashboardEvent> {
        self.tx.subscribe()
    }

    /// Return the number of active receivers.
    pub fn receiver_count(&self) -> usize {
        self.tx.receiver_count()
    }
}
