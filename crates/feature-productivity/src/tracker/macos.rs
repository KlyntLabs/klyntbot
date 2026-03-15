//! macOS-specific window and idle detection — delegates to platform-macos crate.

pub use platform_macos::window::WindowInfo;

pub fn get_frontmost_window() -> common::Result<Option<WindowInfo>> {
    Ok(platform_macos::window::get_frontmost_window())
}

pub fn seconds_since_last_input() -> f64 {
    platform_macos::input::seconds_since_last_input()
}

pub fn get_browser_url(app_name: &str, bundle_id: Option<&str>) -> Option<String> {
    platform_macos::browser::get_browser_url(app_name, bundle_id)
}
