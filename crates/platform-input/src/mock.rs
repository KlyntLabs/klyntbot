//! `MockInput` — records actions for testing without invoking the OS.

use crate::{ComputerUseAction, PlatformInput, Point, Result};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Test-only `PlatformInput` implementation. Records every action it
/// receives in an internal `Vec` and exposes them via `recorded()`.
#[derive(Debug, Default, Clone)]
pub struct MockInput {
    recorded: Arc<Mutex<Vec<ComputerUseAction>>>,
    cursor: Arc<Mutex<Point>>,
}

impl MockInput {
    pub fn new() -> Self {
        Self::default()
    }

    /// Snapshot of recorded actions in arrival order.
    pub async fn recorded(&self) -> Vec<ComputerUseAction> {
        self.recorded.lock().await.clone()
    }

    /// Clear the recorded action log.
    pub async fn clear(&self) {
        self.recorded.lock().await.clear();
    }
}

#[async_trait]
impl PlatformInput for MockInput {
    async fn perform_action(&self, action: ComputerUseAction) -> Result<()> {
        // Update the simulated cursor for movement actions so
        // `get_cursor_position` returns a sensible value.
        match &action {
            ComputerUseAction::MouseMove { x, y }
            | ComputerUseAction::LeftClick { x, y, .. }
            | ComputerUseAction::DoubleClick { x, y, .. }
            | ComputerUseAction::TripleClick { x, y, .. }
            | ComputerUseAction::RightClick { x, y }
            | ComputerUseAction::MiddleClick { x, y }
            | ComputerUseAction::LeftMouseDown { x, y }
            | ComputerUseAction::LeftMouseUp { x, y } => {
                let mut cursor = self.cursor.lock().await;
                cursor.x = *x as f64;
                cursor.y = *y as f64;
            }
            ComputerUseAction::LeftClickDrag { to, .. } => {
                let mut cursor = self.cursor.lock().await;
                cursor.x = to.x;
                cursor.y = to.y;
            }
            _ => {}
        }
        self.recorded.lock().await.push(action);
        Ok(())
    }

    async fn get_cursor_position(&self) -> Result<Point> {
        Ok(*self.cursor.lock().await)
    }

    async fn release_all(&self) -> Result<()> {
        // No-op for mock — does not record.
        Ok(())
    }
}
