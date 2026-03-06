//! macOS-specific window and idle detection using native APIs.

use common::Result;

#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub app_name: String,
    pub bundle_id: Option<String>,
    pub window_title: Option<String>,
}

/// Get the currently focused window's app name, bundle ID, and window title.
///
/// Tries two approaches for the window title:
/// 1. **Accessibility API** (`AXUIElement`) — works if Accessibility permission is granted.
/// 2. **CoreGraphics** (`CGWindowListCopyWindowInfo`) — works if Screen Recording permission is granted.
///
/// Accessibility is preferred because Screen Recording permission resets on every
/// recompile during development (unsigned binaries), while Accessibility persists.
#[cfg(target_os = "macos")]
pub fn get_frontmost_window() -> Result<Option<WindowInfo>> {
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

            Ok(Some(WindowInfo {
                app_name: name,
                bundle_id: bundle,
                window_title: title,
            }))
        }
        None => Ok(None),
    }
}

/// Extract the focused window title using the Accessibility API.
///
/// Creates an AXUIElement for the given PID, reads its focused window, and
/// extracts the `AXTitle` attribute. Requires Accessibility permission.
#[cfg(target_os = "macos")]
fn get_window_title_ax(pid: i32) -> Option<String> {
    use core_foundation::base::TCFType;
    use core_foundation::string::CFString;
    use std::ptr;

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXUIElementCreateApplication(pid: i32) -> *mut std::ffi::c_void;
        fn AXUIElementCopyAttributeValue(
            element: *mut std::ffi::c_void,
            attribute: *const std::ffi::c_void,
            value: *mut *mut std::ffi::c_void,
        ) -> i32;
    }

    const AX_ERROR_SUCCESS: i32 = 0;

    unsafe {
        let app_element = AXUIElementCreateApplication(pid);
        if app_element.is_null() {
            return None;
        }

        // Get the focused window
        let focused_window_attr = CFString::new("AXFocusedWindow");
        let mut focused_window: *mut std::ffi::c_void = ptr::null_mut();
        let err = AXUIElementCopyAttributeValue(
            app_element,
            focused_window_attr.as_CFTypeRef() as *const _,
            &mut focused_window,
        );
        core_foundation::base::CFRelease(app_element as _);

        if err != AX_ERROR_SUCCESS || focused_window.is_null() {
            return None;
        }

        // Get AXTitle from the focused window
        let title_attr = CFString::new("AXTitle");
        let mut title_value: *mut std::ffi::c_void = ptr::null_mut();
        let err = AXUIElementCopyAttributeValue(
            focused_window,
            title_attr.as_CFTypeRef() as *const _,
            &mut title_value,
        );
        core_foundation::base::CFRelease(focused_window as _);

        if err != AX_ERROR_SUCCESS || title_value.is_null() {
            return None;
        }

        let cf_title: CFString = CFString::wrap_under_create_rule(title_value as _);
        let title = cf_title.to_string();
        if title.is_empty() {
            None
        } else {
            Some(title)
        }
    }
}

/// Extract the window title for the given PID using CoreGraphics.
///
/// Uses CGWindowListCopyWindowInfo to enumerate on-screen windows, filtering
/// for the given PID and normal window layer (0).
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

#[cfg(target_os = "macos")]
extern "C" {
    fn CGEventSourceSecondsSinceLastEventType(source_state_id: u32, event_type: u32) -> f64;
}

const CG_EVENT_SOURCE_COMBINED_SESSION_STATE: u32 = 0;
const KCG_ANY_INPUT_EVENT_TYPE: u32 = u32::MAX;

/// Get seconds since last user input (mouse/keyboard).
#[cfg(target_os = "macos")]
pub fn seconds_since_last_input() -> f64 {
    unsafe {
        CGEventSourceSecondsSinceLastEventType(
            CG_EVENT_SOURCE_COMBINED_SESSION_STATE,
            KCG_ANY_INPUT_EVENT_TYPE,
        )
    }
}

/// Get the URL of the active browser tab via AppleScript.
///
/// Works for Chrome-family browsers (Chrome, Brave, Vivaldi, Edge, Opera, Arc)
/// and Safari. Returns `None` for non-browser apps or if the script fails.
#[cfg(target_os = "macos")]
pub fn get_browser_url(app_name: &str, bundle_id: Option<&str>) -> Option<String> {
    use super::categorizer::Categorizer;

    if !Categorizer::is_browser(app_name, bundle_id) {
        return None;
    }

    let name_lower = app_name.to_lowercase();

    // Chrome-family browsers all support the same AppleScript API
    let script = if name_lower == "safari" {
        r#"tell application "Safari" to get URL of front document"#.to_string()
    } else {
        // Chrome, Brave, Vivaldi, Edge, Opera, Arc, Chromium all use Chrome's scripting API
        format!(
            r#"tell application "{}" to get URL of active tab of front window"#,
            app_name
        )
    };

    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .ok()?;

    if output.status.success() {
        let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if url.is_empty() || url == "missing value" {
            None
        } else {
            Some(url)
        }
    } else {
        None
    }
}

#[cfg(not(target_os = "macos"))]
pub fn get_browser_url(_app_name: &str, _bundle_id: Option<&str>) -> Option<String> {
    None
}

// Stubs for non-macOS (for compilation on CI/Linux)
#[cfg(not(target_os = "macos"))]
pub fn get_frontmost_window() -> Result<Option<WindowInfo>> {
    Ok(None)
}

#[cfg(not(target_os = "macos"))]
pub fn seconds_since_last_input() -> f64 {
    0.0
}
