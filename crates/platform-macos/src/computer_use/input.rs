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

/// Map a Klynt-canonical key name to a macOS virtual key code.
/// Returns `None` for unknown names; callers may fall back to
/// per-character `CGEventKeyboardSetUnicodeString`.
fn key_name_to_virtual_code(name: &str) -> Option<u16> {
    match name.to_lowercase().as_str() {
        "enter" | "return" => Some(0x24),
        "tab" => Some(0x30),
        "space" => Some(0x31),
        "delete" | "backspace" => Some(0x33),
        "escape" | "esc" => Some(0x35),
        "left" => Some(0x7B),
        "right" => Some(0x7C),
        "down" => Some(0x7D),
        "up" => Some(0x7E),
        "cmd" | "command" => Some(0x37),
        "shift" => Some(0x38),
        "alt" | "option" => Some(0x3A),
        "ctrl" | "control" => Some(0x3B),
        "f1" => Some(0x7A),
        "f2" => Some(0x78),
        "f3" => Some(0x63),
        "f4" => Some(0x76),
        "f5" => Some(0x60),
        "f6" => Some(0x61),
        // a-z map to 0x00..0x1D in macOS QWERTY layout
        c if c.len() == 1 => {
            let ch = c.chars().next()?;
            match ch {
                'a' => Some(0x00), 'b' => Some(0x0B), 'c' => Some(0x08),
                'd' => Some(0x02), 'e' => Some(0x0E), 'f' => Some(0x03),
                'g' => Some(0x05), 'h' => Some(0x04), 'i' => Some(0x22),
                'j' => Some(0x26), 'k' => Some(0x28), 'l' => Some(0x25),
                'm' => Some(0x2E), 'n' => Some(0x2D), 'o' => Some(0x1F),
                'p' => Some(0x23), 'q' => Some(0x0C), 'r' => Some(0x0F),
                's' => Some(0x01), 't' => Some(0x11), 'u' => Some(0x20),
                'v' => Some(0x09), 'w' => Some(0x0D), 'x' => Some(0x07),
                'y' => Some(0x10), 'z' => Some(0x06),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Map a `KeyMods` to CGEvent flag bits.
#[allow(dead_code)]
fn mods_to_flags(m: platform_input::KeyMods) -> core_graphics::event::CGEventFlags {
    use core_graphics::event::CGEventFlags;
    let mut f = CGEventFlags::empty();
    if m.cmd { f |= CGEventFlags::CGEventFlagCommand; }
    if m.shift { f |= CGEventFlags::CGEventFlagShift; }
    if m.alt { f |= CGEventFlags::CGEventFlagAlternate; }
    if m.ctrl { f |= CGEventFlags::CGEventFlagControl; }
    f
}

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

    fn post_click(&self, x: i32, y: i32, button: core_graphics::event::CGMouseButton, count: i64) -> Result<()> {
        use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType};
        use core_graphics::geometry::CGPoint;

        let down_type = match button {
            core_graphics::event::CGMouseButton::Left => CGEventType::LeftMouseDown,
            core_graphics::event::CGMouseButton::Right => CGEventType::RightMouseDown,
            _ => CGEventType::OtherMouseDown,
        };
        let up_type = match button {
            core_graphics::event::CGMouseButton::Left => CGEventType::LeftMouseUp,
            core_graphics::event::CGMouseButton::Right => CGEventType::RightMouseUp,
            _ => CGEventType::OtherMouseUp,
        };
        let point = CGPoint { x: x as f64, y: y as f64 };

        for i in 1..=count {
            let down = CGEvent::new_mouse_event(self.source.clone(), down_type, point, button)
                .map_err(|()| PlatformError::PlatformCallFailed("CGEventCreate down failed".into()))?;
            // CGEventField::MouseEventClickState = 1
            down.set_integer_value_field(core_graphics::event::EventField::MOUSE_EVENT_CLICK_STATE, i);
            down.post(CGEventTapLocation::HID);
            let up = CGEvent::new_mouse_event(self.source.clone(), up_type, point, button)
                .map_err(|()| PlatformError::PlatformCallFailed("CGEventCreate up failed".into()))?;
            up.set_integer_value_field(core_graphics::event::EventField::MOUSE_EVENT_CLICK_STATE, i);
            up.post(CGEventTapLocation::HID);
        }
        Ok(())
    }

    fn move_cursor(&self, x: i32, y: i32) -> Result<()> {
        use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton};
        use core_graphics::geometry::CGPoint;
        let event = CGEvent::new_mouse_event(
            self.source.clone(),
            CGEventType::MouseMoved,
            CGPoint { x: x as f64, y: y as f64 },
            CGMouseButton::Left,
        )
        .map_err(|()| PlatformError::PlatformCallFailed("MouseMoved failed".into()))?;
        event.post(CGEventTapLocation::HID);
        Ok(())
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
            ComputerUseAction::LeftClick { x, y, .. } => {
                self.post_click(x, y, core_graphics::event::CGMouseButton::Left, 1)
            }
            ComputerUseAction::DoubleClick { x, y, .. } => {
                self.post_click(x, y, core_graphics::event::CGMouseButton::Left, 2)
            }
            ComputerUseAction::TripleClick { x, y, .. } => {
                self.post_click(x, y, core_graphics::event::CGMouseButton::Left, 3)
            }
            ComputerUseAction::RightClick { x, y } => {
                self.post_click(x, y, core_graphics::event::CGMouseButton::Right, 1)
            }
            ComputerUseAction::MiddleClick { x, y } => {
                self.post_click(x, y, core_graphics::event::CGMouseButton::Center, 1)
            }
            ComputerUseAction::Type { text } => {
                // Per-character via CGEventKeyboardSetUnicodeString (works for
                // any Unicode without virtual-key resolution).
                for ch in text.chars() {
                    let down = CGEvent::new_keyboard_event(self.source.clone(), 0, true)
                        .map_err(|()| PlatformError::PlatformCallFailed(
                            "CGEventCreateKeyboardEvent down failed".into(),
                        ))?;
                    let s = ch.to_string();
                    let utf16: Vec<u16> = s.encode_utf16().collect();
                    down.set_string_from_utf16_unchecked(&utf16);
                    down.post(CGEventTapLocation::HID);
                    let up = CGEvent::new_keyboard_event(self.source.clone(), 0, false)
                        .map_err(|()| PlatformError::PlatformCallFailed(
                            "CGEventCreateKeyboardEvent up failed".into(),
                        ))?;
                    up.set_string_from_utf16_unchecked(&utf16);
                    up.post(CGEventTapLocation::HID);
                }
                Ok(())
            }
            ComputerUseAction::Key { keys } => {
                use core_graphics::event::CGEventFlags;
                // Resolve modifiers first, then the final key.
                let mut flags = CGEventFlags::empty();
                let mut final_key: Option<u16> = None;
                for k in &keys {
                    match k.to_lowercase().as_str() {
                        "cmd" | "command" => flags |= CGEventFlags::CGEventFlagCommand,
                        "shift"           => flags |= CGEventFlags::CGEventFlagShift,
                        "alt" | "option"  => flags |= CGEventFlags::CGEventFlagAlternate,
                        "ctrl" | "control"=> flags |= CGEventFlags::CGEventFlagControl,
                        other => {
                            final_key = key_name_to_virtual_code(other)
                                .or(final_key); // first non-modifier wins
                        }
                    }
                }
                let vk = final_key.ok_or_else(|| {
                    PlatformError::UnsupportedKey(format!("no terminal key in {:?}", keys))
                })?;
                let down = CGEvent::new_keyboard_event(self.source.clone(), vk, true)
                    .map_err(|()| PlatformError::PlatformCallFailed("keyboard down failed".into()))?;
                down.set_flags(flags);
                down.post(CGEventTapLocation::HID);
                let up = CGEvent::new_keyboard_event(self.source.clone(), vk, false)
                    .map_err(|()| PlatformError::PlatformCallFailed("keyboard up failed".into()))?;
                up.set_flags(flags);
                up.post(CGEventTapLocation::HID);
                Ok(())
            }
            ComputerUseAction::HoldKey { keys, duration_ms } => {
                use core_graphics::event::CGEventFlags;
                let mut flags = CGEventFlags::empty();
                let mut final_key: Option<u16> = None;
                for k in &keys {
                    match k.to_lowercase().as_str() {
                        "cmd" | "command" => flags |= CGEventFlags::CGEventFlagCommand,
                        "shift"           => flags |= CGEventFlags::CGEventFlagShift,
                        "alt" | "option"  => flags |= CGEventFlags::CGEventFlagAlternate,
                        "ctrl" | "control"=> flags |= CGEventFlags::CGEventFlagControl,
                        other => final_key = key_name_to_virtual_code(other).or(final_key),
                    }
                }
                let vk = final_key.ok_or_else(|| {
                    PlatformError::UnsupportedKey(format!("no terminal key in {:?}", keys))
                })?;
                {
                    let down = CGEvent::new_keyboard_event(self.source.clone(), vk, true)
                        .map_err(|()| PlatformError::PlatformCallFailed("keyboard down failed".into()))?;
                    down.set_flags(flags);
                    down.post(CGEventTapLocation::HID);
                }
                tokio::time::sleep(std::time::Duration::from_millis(duration_ms as u64)).await;
                {
                    let up = CGEvent::new_keyboard_event(self.source.clone(), vk, false)
                        .map_err(|()| PlatformError::PlatformCallFailed("keyboard up failed".into()))?;
                    up.set_flags(flags);
                    up.post(CGEventTapLocation::HID);
                }
                Ok(())
            }
            ComputerUseAction::Wait { duration_ms } => {
                tokio::time::sleep(std::time::Duration::from_millis(duration_ms as u64)).await;
                Ok(())
            }
            ComputerUseAction::Scroll { x, y, direction, amount } => {
                use core_graphics::event::{CGEvent, CGEventTapLocation, ScrollEventUnit};
                use platform_input::ScrollDir;
                // Move cursor first so the scroll lands at the right place.
                self.move_cursor(x, y)?;
                let (dy, dx) = match direction {
                    ScrollDir::Up => (amount, 0),
                    ScrollDir::Down => (-amount, 0),
                    ScrollDir::Left => (0, amount),
                    ScrollDir::Right => (0, -amount),
                };
                let event = CGEvent::new_scroll_event(
                    self.source.clone(),
                    ScrollEventUnit::LINE,
                    2,
                    dy,
                    dx,
                    0,
                )
                .map_err(|()| PlatformError::PlatformCallFailed(
                    "CGEventCreateScrollWheelEvent failed".into(),
                ))?;
                event.post(CGEventTapLocation::HID);
                Ok(())
            }
            ComputerUseAction::LeftMouseDown { x, y } => {
                use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton};
                use core_graphics::geometry::CGPoint;
                let event = CGEvent::new_mouse_event(
                    self.source.clone(),
                    CGEventType::LeftMouseDown,
                    CGPoint { x: x as f64, y: y as f64 },
                    CGMouseButton::Left,
                )
                .map_err(|()| PlatformError::PlatformCallFailed("LeftMouseDown failed".into()))?;
                event.post(CGEventTapLocation::HID);
                Ok(())
            }
            ComputerUseAction::LeftMouseUp { x, y } => {
                use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton};
                use core_graphics::geometry::CGPoint;
                let event = CGEvent::new_mouse_event(
                    self.source.clone(),
                    CGEventType::LeftMouseUp,
                    CGPoint { x: x as f64, y: y as f64 },
                    CGMouseButton::Left,
                )
                .map_err(|()| PlatformError::PlatformCallFailed("LeftMouseUp failed".into()))?;
                event.post(CGEventTapLocation::HID);
                Ok(())
            }
            ComputerUseAction::LeftClickDrag { from, to, .. } => {
                use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton};
                use core_graphics::geometry::CGPoint;
                // Press at `from`.
                {
                    let down = CGEvent::new_mouse_event(
                        self.source.clone(),
                        CGEventType::LeftMouseDown,
                        CGPoint { x: from.x, y: from.y },
                        CGMouseButton::Left,
                    )
                    .map_err(|()| PlatformError::PlatformCallFailed("drag down failed".into()))?;
                    down.post(CGEventTapLocation::HID);
                }
                // Drag through several intermediate points (more reliable than
                // a single jump for apps that watch dragged events).
                let steps = 16;
                for i in 1..=steps {
                    let t = (i as f64) / (steps as f64);
                    let p = CGPoint {
                        x: from.x + (to.x - from.x) * t,
                        y: from.y + (to.y - from.y) * t,
                    };
                    {
                        let drag = CGEvent::new_mouse_event(
                            self.source.clone(),
                            CGEventType::LeftMouseDragged,
                            p,
                            CGMouseButton::Left,
                        )
                        .map_err(|()| PlatformError::PlatformCallFailed("drag step failed".into()))?;
                        drag.post(CGEventTapLocation::HID);
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(8)).await;
                }
                // Release at `to`.
                {
                    let up = CGEvent::new_mouse_event(
                        self.source.clone(),
                        CGEventType::LeftMouseUp,
                        CGPoint { x: to.x, y: to.y },
                        CGMouseButton::Left,
                    )
                    .map_err(|()| PlatformError::PlatformCallFailed("drag up failed".into()))?;
                    up.post(CGEventTapLocation::HID);
                }
                Ok(())
            }
            ComputerUseAction::Screenshot { .. } | ComputerUseAction::Zoom { .. } => {
                Err(PlatformError::NotImplemented)
            }
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
        use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton};
        use core_graphics::geometry::CGPoint;
        // Force-release all mouse buttons at current position.
        let pos = self.get_cursor_position().await?;
        for btn in [CGMouseButton::Left, CGMouseButton::Right, CGMouseButton::Center] {
            let up_type = match btn {
                CGMouseButton::Left => CGEventType::LeftMouseUp,
                CGMouseButton::Right => CGEventType::RightMouseUp,
                _ => CGEventType::OtherMouseUp,
            };
            if let Ok(event) = CGEvent::new_mouse_event(
                self.source.clone(),
                up_type,
                CGPoint { x: pos.x, y: pos.y },
                btn,
            ) {
                event.post(CGEventTapLocation::HID);
            }
        }
        Ok(())
    }
}
