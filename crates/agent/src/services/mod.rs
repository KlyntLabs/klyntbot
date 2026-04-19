//! Background lifecycle services — long-running tasks with graceful shutdown.
//!
//! Each service follows the start/stop pattern with `CancellationToken`
//! for cooperative shutdown.

pub mod memory_maintenance;
pub mod recurring_tasks;
pub mod session_cleanup;
