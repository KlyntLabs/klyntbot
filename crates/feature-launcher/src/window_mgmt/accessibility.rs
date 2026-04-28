//! Window management using platform-macos AXUIElement wrappers.

pub fn get_frontmost_pid() -> Option<i32> {
    platform_macos::window::get_frontmost_window().map(|w| w.pid)
}

pub fn get_screen_frame() -> (f64, f64, f64, f64) {
    platform_macos::window::get_screen_frame()
}

pub fn get_frontmost_window_frame(pid: i32) -> Option<(f64, f64, f64, f64)> {
    platform_macos::window::get_frontmost_window_frame(pid)
}

pub fn set_window_frame(pid: i32, x: f64, y: f64, w: f64, h: f64) -> bool {
    platform_macos::window::set_window_frame(pid, x, y, w, h)
}
