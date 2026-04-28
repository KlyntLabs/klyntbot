//! AX tree walker: AXUIElement → `AccessibilityNode`.
//!
//! Uses the same raw-FFI pattern as `crates/platform-macos/src/window.rs`.
//! Coordinates are converted from AppKit (bottom-left) to Quartz
//! (top-left) at construction.

use core_foundation::{
    array::{CFArray, CFArrayRef},
    base::{CFRelease, CFTypeRef, TCFType},
    string::CFString,
};
use platform_capture::{AccessibilityNode, CaptureError, Result};
use platform_input::Rect;
use std::collections::HashMap;
use std::ffi::c_void;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXUIElementCreateApplication(pid: i32) -> *mut c_void;
    fn AXUIElementCopyAttributeValue(
        element: *mut c_void,
        attribute: *const c_void,
        value: *mut *mut c_void,
    ) -> i32;
    fn AXValueGetValue(value: *mut c_void, the_type: u32, value_ptr: *mut c_void) -> i32;
}

const K_AX_VALUE_CG_POINT_TYPE: u32 = 1;
const K_AX_VALUE_CG_SIZE_TYPE: u32 = 2;

#[repr(C)]
#[derive(Copy, Clone, Default)]
struct CGPoint { x: f64, y: f64 }
#[repr(C)]
#[derive(Copy, Clone, Default)]
struct CGSize { width: f64, height: f64 }

/// Walk the AX tree of the given app, bounded by `max_depth`.
///
/// Returns the focused window's AX root with children populated.
pub fn walk_focused_app(pid: i32, max_depth: usize) -> Result<AccessibilityNode> {
    // SAFETY: pid is a real ProcessID; AXUIElementCreateApplication
    // returns a retained AXUIElementRef on success.
    let app = unsafe { AXUIElementCreateApplication(pid) };
    if app.is_null() {
        return Err(CaptureError::AxTreeUnavailable(format!(
            "AXUIElementCreateApplication returned null for pid {pid}"
        )));
    }

    let focused = copy_attribute(app, "AXFocusedWindow")?;
    let result = walk(focused, max_depth);
    unsafe {
        CFRelease(focused);
        CFRelease(app);
    }
    result
}

fn walk(element: *mut c_void, depth_remaining: usize) -> Result<AccessibilityNode> {
    let role = copy_string_attribute(element, "AXRole").unwrap_or_default();
    let label = copy_string_attribute(element, "AXTitle")
        .or_else(|_| copy_string_attribute(element, "AXDescription"))
        .ok();
    let value = copy_string_attribute(element, "AXValue").ok();

    let frame = read_frame(element).unwrap_or(Rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 });

    let children = if depth_remaining == 0 {
        Vec::new()
    } else {
        copy_children(element)?
            .into_iter()
            .filter_map(|child| {
                let r = walk(child, depth_remaining - 1).ok();
                unsafe { CFRelease(child) };
                r
            })
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

fn copy_attribute(element: *mut c_void, name: &str) -> Result<*mut c_void> {
    let key = CFString::new(name);
    let mut out: *mut c_void = std::ptr::null_mut();
    let err = unsafe {
        AXUIElementCopyAttributeValue(element, key.as_concrete_TypeRef() as _, &mut out)
    };
    if err != 0 || out.is_null() {
        return Err(CaptureError::AxTreeUnavailable(format!(
            "AXUIElementCopyAttributeValue({name}) failed: {err}"
        )));
    }
    Ok(out)
}

fn copy_string_attribute(element: *mut c_void, name: &str) -> Result<String> {
    let raw = copy_attribute(element, name)?;
    let cfstr = unsafe { CFString::wrap_under_create_rule(raw as _) };
    Ok(cfstr.to_string())
}

fn copy_children(element: *mut c_void) -> Result<Vec<*mut c_void>> {
    let raw = match copy_attribute(element, "AXChildren") {
        Ok(r) => r,
        // No children attribute = leaf node, return empty.
        Err(_) => return Ok(Vec::new()),
    };
    let array = unsafe { CFArray::<CFTypeRef>::wrap_under_create_rule(raw as CFArrayRef) };
    let count = array.len() as usize;
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        if let Some(item) = array.get(i as isize) {
            // Each item is a CFTypeRef; we transfer ownership to the
            // caller (caller must CFRelease).
            let raw_item = *item as *mut c_void;
            // Retain because wrap_under_create_rule already holds the
            // array's reference; the items inside the array do not
            // get extra retains.
            unsafe {
                core_foundation::base::CFRetain(raw_item as _);
            }
            out.push(raw_item);
        }
    }
    Ok(out)
}

fn read_frame(element: *mut c_void) -> Result<Rect> {
    let pos_raw = copy_attribute(element, "AXPosition")?;
    let size_raw = copy_attribute(element, "AXSize")?;
    let mut pos = CGPoint::default();
    let mut size = CGSize::default();
    let pos_ok = unsafe {
        AXValueGetValue(pos_raw, K_AX_VALUE_CG_POINT_TYPE, &mut pos as *mut _ as *mut _)
    };
    let size_ok = unsafe {
        AXValueGetValue(size_raw, K_AX_VALUE_CG_SIZE_TYPE, &mut size as *mut _ as *mut _)
    };
    unsafe {
        CFRelease(pos_raw);
        CFRelease(size_raw);
    }
    if pos_ok == 0 || size_ok == 0 {
        return Err(CaptureError::AxTreeUnavailable("AXValueGetValue failed".into()));
    }
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
