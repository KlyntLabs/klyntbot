//! `MacInput` — CGEvent-based input injection on macOS.
//!
//! Uses `core-graphics 0.24` directly; no `enigo` dependency.
//!
//! Threading: CGEvent is thread-safe per Apple. Methods are async to
//! match the `PlatformInput` trait but the underlying calls are
//! synchronous and may run on any thread; callers should dispatch
//! into a `spawn_blocking` worker if they need to avoid blocking the
//! tokio reactor.

use async_trait::async_trait;
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use platform_input::{
    ComputerUseAction, PlatformError, PlatformInput, Point, Result,
};

pub struct MacInput {
    /// Cached CGEventSource. Apple states a single source can be
    /// reused across all events from the same logical actor.
    source: CGEventSource,
}

// SAFETY: CGEventSource is a reference-counted CoreFoundation object.
// Apple's documentation states it is safe to use from any thread.
unsafe impl Send for MacInput {}
unsafe impl Sync for MacInput {}

impl MacInput {
    /// Construct a new `MacInput`.
    ///
    /// Returns `PlatformCallFailed` if `CGEventSourceCreate` fails
    /// (extremely rare — typically only when the process lacks the
    /// underlying Quartz framework).
    pub fn new() -> Result<Self> {
        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|()| PlatformError::PlatformCallFailed(
                "CGEventSourceCreate failed".into(),
            ))?;
        Ok(Self { source })
    }
}

#[async_trait]
impl PlatformInput for MacInput {
    async fn perform_action(&self, action: ComputerUseAction) -> Result<()> {
        use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton};
        use core_graphics::geometry::CGPoint;

        match action {
            ComputerUseAction::MouseMove { x, y } => {
                let event = CGEvent::new_mouse_event(
                    self.source.clone(),
                    CGEventType::MouseMoved,
                    CGPoint { x: x as f64, y: y as f64 },
                    CGMouseButton::Left,
                )
                .map_err(|()| PlatformError::PlatformCallFailed(
                    "CGEventCreateMouseEvent failed".into(),
                ))?;
                event.post(CGEventTapLocation::HID);
                Ok(())
            }
            _ => Err(PlatformError::NotImplemented),
        }
    }

    async fn get_cursor_position(&self) -> Result<Point> {
        use core_graphics::event::CGEvent;
        let event = CGEvent::new(self.source.clone()).map_err(|()| {
            PlatformError::PlatformCallFailed("CGEventCreate failed".into())
        })?;
        let loc = event.location();
        Ok(Point { x: loc.x, y: loc.y })
    }

    async fn release_all(&self) -> Result<()> {
        Err(PlatformError::NotImplemented)
    }
}
