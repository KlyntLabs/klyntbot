//! Idle detection via macOS CoreGraphics event source.

/// Get the number of seconds since the last user input event (mouse/keyboard).
#[cfg(target_os = "macos")]
pub fn seconds_since_last_input() -> f64 {
    extern "C" {
        fn CGEventSourceSecondsSinceLastEventType(source_state_id: u32, event_type: u32) -> f64;
    }
    const COMBINED_SESSION: u32 = 0;
    const ANY_INPUT: u32 = u32::MAX;
    unsafe { CGEventSourceSecondsSinceLastEventType(COMBINED_SESSION, ANY_INPUT) }
}

#[cfg(not(target_os = "macos"))]
pub fn seconds_since_last_input() -> f64 {
    0.0
}
