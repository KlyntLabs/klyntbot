//! macOS-specific window and idle detection using native APIs.

use common::Result;

#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub app_name: String,
    pub bundle_id: Option<String>,
    pub window_title: Option<String>,
}

/// Get the currently focused window's app name and bundle ID.
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
            let title = get_window_title_cg(pid);

            Ok(Some(WindowInfo {
                app_name: name,
                bundle_id: bundle,
                window_title: title,
            }))
        }
        None => Ok(None),
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

// Stubs for non-macOS (for compilation on CI/Linux)
#[cfg(not(target_os = "macos"))]
pub fn get_frontmost_window() -> Result<Option<WindowInfo>> {
    Ok(None)
}

#[cfg(not(target_os = "macos"))]
pub fn seconds_since_last_input() -> f64 {
    0.0
}
