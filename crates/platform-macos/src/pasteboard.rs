//! macOS pasteboard (clipboard) access.

/// Get the current change count of the general pasteboard.
///
/// Each time the pasteboard content changes, the count increments.
/// Useful for detecting clipboard changes without polling content.
#[cfg(target_os = "macos")]
pub fn pasteboard_change_count() -> i64 {
    use objc2_app_kit::NSPasteboard;
    NSPasteboard::generalPasteboard().changeCount() as i64
}

#[cfg(not(target_os = "macos"))]
pub fn pasteboard_change_count() -> i64 {
    0
}

/// Read the current string content from the general pasteboard.
#[cfg(target_os = "macos")]
pub fn read_pasteboard_string() -> Option<String> {
    use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
    let pasteboard = NSPasteboard::generalPasteboard();
    let string = unsafe { pasteboard.stringForType(NSPasteboardTypeString) }?;
    Some(string.to_string())
}

#[cfg(not(target_os = "macos"))]
pub fn read_pasteboard_string() -> Option<String> {
    None
}
