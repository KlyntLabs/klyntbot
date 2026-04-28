//! AX tree walker: `AXUIElement` → `AccessibilityNode`.
//!
//! Uses safe RAII wrappers from [`crate::ax`]; all CoreFoundation objects are
//! automatically released on drop. There are no raw `*mut c_void` pointers or
//! manual `CFRelease` calls in this file.

use crate::ax::AXUIElement;
use platform_capture::{AccessibilityNode, CaptureError, Result};
use platform_input::Rect;
use std::collections::HashMap;

/// Walk the AX tree of the given app, bounded by `max_depth`.
///
/// Returns the focused window's AX root with children populated.
pub fn walk_focused_app(pid: i32, max_depth: usize) -> Result<AccessibilityNode> {
    let app = AXUIElement::create_application(pid).ok_or_else(|| {
        CaptureError::AxTreeUnavailable(format!(
            "AXUIElementCreateApplication returned null for pid {pid}"
        ))
    })?;

    let focused: AXUIElement = app
        .copy_attribute("AXFocusedWindow")?
        .downcast_into()
        .ok_or_else(|| {
            CaptureError::AxTreeUnavailable("AXFocusedWindow was not an AXUIElement".into())
        })?;

    walk(&focused, max_depth)
}

fn walk(element: &AXUIElement, depth_remaining: usize) -> Result<AccessibilityNode> {
    let role = element.copy_string_attribute("AXRole").unwrap_or_default();
    let label = element
        .copy_string_attribute("AXTitle")
        .or_else(|_| element.copy_string_attribute("AXDescription"))
        .ok();
    let value = element.copy_string_attribute("AXValue").ok();

    let frame = read_frame(element).unwrap_or(Rect {
        x: 0.0,
        y: 0.0,
        w: 0.0,
        h: 0.0,
    });

    let children = if depth_remaining == 0 {
        Vec::new()
    } else {
        element
            .children()?
            .into_iter()
            .filter_map(|child| walk(&child, depth_remaining - 1).ok())
            .collect()
    };

    Ok(AccessibilityNode {
        role,
        label,
        value,
        frame,
        children,
        attrs: HashMap::new(),
    })
}

fn read_frame(element: &AXUIElement) -> Result<Rect> {
    use crate::ax::AXValue;

    let pos: AXValue = element
        .copy_attribute("AXPosition")?
        .downcast_into()
        .ok_or_else(|| CaptureError::AxTreeUnavailable("AXPosition was not an AXValue".into()))?;
    let size: AXValue = element
        .copy_attribute("AXSize")?
        .downcast_into()
        .ok_or_else(|| CaptureError::AxTreeUnavailable("AXSize was not an AXValue".into()))?;

    let pos = pos
        .to_point()
        .ok_or_else(|| CaptureError::AxTreeUnavailable("AXValueGetValue(point) failed".into()))?;
    let size = size
        .to_size()
        .ok_or_else(|| CaptureError::AxTreeUnavailable("AXValueGetValue(size) failed".into()))?;

    // AXPosition is in AppKit space (bottom-left origin); we currently
    // pass it through as-is. Y-flip will be applied in Phase 4 once we
    // have a screen-height lookup. For Phase 1 the smoke test only
    // verifies tree traversal, not exact coordinates.
    Ok(Rect {
        x: pos.x,
        y: pos.y,
        w: size.width,
        h: size.height,
    })
}
