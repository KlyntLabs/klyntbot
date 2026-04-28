//! Window detection and management using macOS native APIs.

#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub app_name: String,
    pub bundle_id: Option<String>,
    pub window_title: Option<String>,
    pub pid: i32,
}

/// Get the currently focused window's app name, bundle ID, and window title.
///
/// Tries two approaches for the window title:
/// 1. **Accessibility API** (`AXUIElement`) -- works if Accessibility permission is granted.
/// 2. **CoreGraphics** (`CGWindowListCopyWindowInfo`) -- works if Screen Recording permission is granted.
///
/// Accessibility is preferred because Screen Recording permission resets on every
/// recompile during development (unsigned binaries), while Accessibility persists.
#[cfg(target_os = "macos")]
pub fn get_frontmost_window() -> Option<WindowInfo> {
    use objc2_app_kit::NSWorkspace;

    let workspace = NSWorkspace::sharedWorkspace();
    let app = workspace.frontmostApplication();

    match app {
        Some(app) => {
            let name = app
                .localizedName()
                .map(|n| n.to_string())
                .unwrap_or_else(|| "Unknown".to_string());
            let bundle = app.bundleIdentifier().map(|b| b.to_string());

            let pid = app.processIdentifier();
            // Try AX first (needs Accessibility), fall back to CG (needs Screen Recording).
            let title = get_window_title_ax(pid).or_else(|| get_window_title_cg(pid));
            tracing::debug!(app = %name, ?title, pid, "frontmost window detected");

            Some(WindowInfo {
                app_name: name,
                bundle_id: bundle,
                window_title: title,
                pid,
            })
        }
        None => None,
    }
}

#[cfg(not(target_os = "macos"))]
pub fn get_frontmost_window() -> Option<WindowInfo> {
    None
}

/// Lightweight: get only the frontmost app's name without AX/CG title lookups.
/// Use this when you only need the app name (e.g., clipboard source tracking).
#[cfg(target_os = "macos")]
pub fn get_frontmost_app_name() -> Option<String> {
    use objc2_app_kit::NSWorkspace;

    let workspace = NSWorkspace::sharedWorkspace();
    let app = workspace.frontmostApplication()?;
    let name = app.localizedName()?;
    Some(name.to_string())
}

#[cfg(not(target_os = "macos"))]
pub fn get_frontmost_app_name() -> Option<String> {
    None
}

/// Extract the focused window title using the Accessibility API.
#[cfg(target_os = "macos")]
fn get_window_title_ax(pid: i32) -> Option<String> {
    use crate::ax::AXUIElement;

    let app = AXUIElement::create_application(pid)?;
    let focused: AXUIElement = app
        .copy_attribute("AXFocusedWindow")
        .ok()?
        .downcast_into()?;
    let title = focused.copy_string_attribute("AXTitle").ok()?;
    if title.is_empty() {
        None
    } else {
        Some(title)
    }
}

/// Extract the window title for the given PID using CoreGraphics.
#[cfg(target_os = "macos")]
fn get_window_title_cg(pid: i32) -> Option<String> {
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::dictionary::CFDictionaryRef;
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;
    use core_graphics::display::{
        kCGNullWindowID, kCGWindowListExcludeDesktopElements, kCGWindowListOptionOnScreenOnly,
        CGWindowListCopyWindowInfo,
    };

    let options = kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements;
    let window_list = unsafe { CGWindowListCopyWindowInfo(options, kCGNullWindowID) };
    if window_list.is_null() {
        return None;
    }

    let windows: core_foundation::array::CFArray<CFType> =
        unsafe { core_foundation::array::CFArray::wrap_under_create_rule(window_list as _) };

    let key_pid = CFString::new("kCGWindowOwnerPID");
    let key_name = CFString::new("kCGWindowName");
    let key_layer = CFString::new("kCGWindowLayer");

    for i in 0..windows.len() {
        let Some(dict) = windows.get(i) else {
            continue;
        };
        let dict_ref: CFDictionaryRef = dict.as_CFTypeRef() as CFDictionaryRef;

        // Get window owner PID
        let mut pid_val: *const core_foundation::base::CFTypeRef = std::ptr::null();
        if unsafe {
            core_foundation::dictionary::CFDictionaryGetValueIfPresent(
                dict_ref,
                key_pid.as_CFTypeRef() as *const _,
                &mut pid_val as *mut *const _ as *mut *const _,
            )
        } == 0
        {
            continue;
        }
        let window_pid: CFNumber = unsafe { CFNumber::wrap_under_get_rule(pid_val as _) };
        let window_pid_i64 = window_pid.to_i64()?;
        if window_pid_i64 != pid as i64 {
            continue;
        }

        // Check layer == 0 (normal windows)
        let mut layer_val: *const core_foundation::base::CFTypeRef = std::ptr::null();
        if unsafe {
            core_foundation::dictionary::CFDictionaryGetValueIfPresent(
                dict_ref,
                key_layer.as_CFTypeRef() as *const _,
                &mut layer_val as *mut *const _ as *mut *const _,
            )
        } != 0
        {
            let layer: CFNumber = unsafe { CFNumber::wrap_under_get_rule(layer_val as _) };
            if let Some(l) = layer.to_i32() {
                if l != 0 {
                    continue;
                }
            }
        }

        // Extract window name
        let mut name_val: *const core_foundation::base::CFTypeRef = std::ptr::null();
        if unsafe {
            core_foundation::dictionary::CFDictionaryGetValueIfPresent(
                dict_ref,
                key_name.as_CFTypeRef() as *const _,
                &mut name_val as *mut *const _ as *mut *const _,
            )
        } != 0
        {
            let name: CFString = unsafe { CFString::wrap_under_get_rule(name_val as _) };
            let title = name.to_string();
            if !title.is_empty() {
                return Some(title);
            }
        }
    }

    None
}

/// Get the main display's screen frame as (x, y, width, height).
#[cfg(target_os = "macos")]
pub fn get_screen_frame() -> (f64, f64, f64, f64) {
    use core_graphics::display::CGDisplay;
    let display = CGDisplay::main();
    let bounds = display.bounds();
    (
        bounds.origin.x,
        bounds.origin.y,
        bounds.size.width,
        bounds.size.height,
    )
}

#[cfg(not(target_os = "macos"))]
pub fn get_screen_frame() -> (f64, f64, f64, f64) {
    (0.0, 0.0, 0.0, 0.0)
}

/// Move and resize the focused window of the application with the given PID.
///
/// Returns `true` on success, `false` if the operation failed (e.g., no
/// Accessibility permission or no focused window).
#[cfg(target_os = "macos")]
pub fn set_window_frame(pid: i32, x: f64, y: f64, w: f64, h: f64) -> bool {
    use crate::ax::{AXUIElement, AXValue};
    use core_foundation::base::TCFType;
    use core_graphics::geometry::{CGPoint, CGSize};

    let app = match AXUIElement::create_application(pid) {
        Some(a) => a,
        None => return false,
    };

    let focused: AXUIElement = match app
        .copy_attribute("AXFocusedWindow")
        .ok()
        .and_then(|v| v.downcast_into())
    {
        Some(f) => f,
        None => return false,
    };

    let position = CGPoint::new(x, y);
    let position_value = match AXValue::from_point(position) {
        Some(v) => v,
        None => return false,
    };
    if focused
        .set_attribute("AXPosition", position_value.as_CFTypeRef())
        .is_err()
    {
        return false;
    }

    let size = CGSize::new(w, h);
    let size_value = match AXValue::from_size(size) {
        Some(v) => v,
        None => return false,
    };
    if focused
        .set_attribute("AXSize", size_value.as_CFTypeRef())
        .is_err()
    {
        return false;
    }

    true
}

/// Read the current position and size of the focused window for the given PID.
/// Returns (x, y, w, h) on success, or None if the operation failed.
#[cfg(target_os = "macos")]
pub fn get_frontmost_window_frame(pid: i32) -> Option<(f64, f64, f64, f64)> {
    use crate::ax::{AXUIElement, AXValue};

    let app = AXUIElement::create_application(pid)?;
    let focused: AXUIElement = app
        .copy_attribute("AXFocusedWindow")
        .ok()?
        .downcast_into()?;

    let position: AXValue = focused.copy_attribute("AXPosition").ok()?.downcast_into()?;
    let size: AXValue = focused.copy_attribute("AXSize").ok()?.downcast_into()?;

    let position = position.to_point()?;
    let size = size.to_size()?;

    Some((position.x, position.y, size.width, size.height))
}

#[cfg(not(target_os = "macos"))]
pub fn get_frontmost_window_frame(_pid: i32) -> Option<(f64, f64, f64, f64)> {
    None
}

#[cfg(not(target_os = "macos"))]
pub fn set_window_frame(_pid: i32, _x: f64, _y: f64, _w: f64, _h: f64) -> bool {
    false
}
