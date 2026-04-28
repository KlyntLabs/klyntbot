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
    use core_foundation::base::TCFType;
    use core_foundation::string::CFString;
    use core_graphics::geometry::{CGPoint, CGSize};
    use std::ptr;

    const AX_VALUE_TYPE_CGPOINT: u32 = 1;
    const AX_VALUE_TYPE_CGSIZE: u32 = 2;
    const AX_ERROR_SUCCESS: i32 = 0;

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXUIElementCreateApplication(pid: i32) -> *mut std::ffi::c_void;
        fn AXUIElementCopyAttributeValue(
            element: *mut std::ffi::c_void,
            attribute: *const std::ffi::c_void,
            value: *mut *mut std::ffi::c_void,
        ) -> i32;
        fn AXUIElementSetAttributeValue(
            element: *mut std::ffi::c_void,
            attribute: *const std::ffi::c_void,
            value: *const std::ffi::c_void,
        ) -> i32;
        fn AXValueCreate(value_type: u32, value: *const std::ffi::c_void) -> *mut std::ffi::c_void;
    }

    unsafe {
        let app_element = AXUIElementCreateApplication(pid);
        if app_element.is_null() {
            return false;
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
            return false;
        }

        // Set position
        let position = CGPoint::new(x, y);
        let position_value = AXValueCreate(
            AX_VALUE_TYPE_CGPOINT,
            &position as *const CGPoint as *const std::ffi::c_void,
        );
        if position_value.is_null() {
            core_foundation::base::CFRelease(focused_window as _);
            return false;
        }
        let position_attr = CFString::new("AXPosition");
        let err = AXUIElementSetAttributeValue(
            focused_window,
            position_attr.as_CFTypeRef() as *const _,
            position_value as *const _,
        );
        core_foundation::base::CFRelease(position_value as _);
        if err != AX_ERROR_SUCCESS {
            core_foundation::base::CFRelease(focused_window as _);
            return false;
        }

        // Set size
        let size = CGSize::new(w, h);
        let size_value = AXValueCreate(
            AX_VALUE_TYPE_CGSIZE,
            &size as *const CGSize as *const std::ffi::c_void,
        );
        if size_value.is_null() {
            core_foundation::base::CFRelease(focused_window as _);
            return false;
        }
        let size_attr = CFString::new("AXSize");
        let err = AXUIElementSetAttributeValue(
            focused_window,
            size_attr.as_CFTypeRef() as *const _,
            size_value as *const _,
        );
        core_foundation::base::CFRelease(size_value as _);
        core_foundation::base::CFRelease(focused_window as _);

        err == AX_ERROR_SUCCESS
    }
}

/// Read the current position and size of the focused window for the given PID.
/// Returns (x, y, w, h) on success, or None if the operation failed.
#[cfg(target_os = "macos")]
pub fn get_frontmost_window_frame(pid: i32) -> Option<(f64, f64, f64, f64)> {
    use core_foundation::base::TCFType;
    use core_foundation::string::CFString;
    use core_graphics::geometry::{CGPoint, CGSize};
    use std::ptr;

    const AX_VALUE_TYPE_CGPOINT: u32 = 1;
    const AX_VALUE_TYPE_CGSIZE: u32 = 2;
    const AX_ERROR_SUCCESS: i32 = 0;

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXUIElementCreateApplication(pid: i32) -> *mut std::ffi::c_void;
        fn AXUIElementCopyAttributeValue(
            element: *mut std::ffi::c_void,
            attribute: *const std::ffi::c_void,
            value: *mut *mut std::ffi::c_void,
        ) -> i32;
        fn AXValueGetValue(
            value: *mut std::ffi::c_void,
            value_type: u32,
            out: *mut std::ffi::c_void,
        ) -> i32;
    }

    unsafe {
        let app_element = AXUIElementCreateApplication(pid);
        if app_element.is_null() {
            return None;
        }

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

        // Read position
        let position_attr = CFString::new("AXPosition");
        let mut position_value: *mut std::ffi::c_void = ptr::null_mut();
        let err = AXUIElementCopyAttributeValue(
            focused_window,
            position_attr.as_CFTypeRef() as *const _,
            &mut position_value,
        );
        if err != AX_ERROR_SUCCESS || position_value.is_null() {
            core_foundation::base::CFRelease(focused_window as _);
            return None;
        }
        let mut position = CGPoint::new(0.0, 0.0);
        AXValueGetValue(
            position_value,
            AX_VALUE_TYPE_CGPOINT,
            &mut position as *mut CGPoint as *mut std::ffi::c_void,
        );
        core_foundation::base::CFRelease(position_value as _);

        // Read size
        let size_attr = CFString::new("AXSize");
        let mut size_value: *mut std::ffi::c_void = ptr::null_mut();
        let err = AXUIElementCopyAttributeValue(
            focused_window,
            size_attr.as_CFTypeRef() as *const _,
            &mut size_value,
        );
        if err != AX_ERROR_SUCCESS || size_value.is_null() {
            core_foundation::base::CFRelease(focused_window as _);
            return None;
        }
        let mut size = CGSize::new(0.0, 0.0);
        AXValueGetValue(
            size_value,
            AX_VALUE_TYPE_CGSIZE,
            &mut size as *mut CGSize as *mut std::ffi::c_void,
        );
        core_foundation::base::CFRelease(size_value as _);
        core_foundation::base::CFRelease(focused_window as _);

        Some((position.x, position.y, size.width, size.height))
    }
}

#[cfg(not(target_os = "macos"))]
pub fn get_frontmost_window_frame(_pid: i32) -> Option<(f64, f64, f64, f64)> {
    None
}

#[cfg(not(target_os = "macos"))]
pub fn set_window_frame(_pid: i32, _x: f64, _y: f64, _w: f64, _h: f64) -> bool {
    false
}
