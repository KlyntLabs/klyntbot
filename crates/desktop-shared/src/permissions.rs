//! Shared macOS permission checks used by both Tauri commands and the dev-API.

/// Check whether Screen Recording permission is granted.
pub fn check_screen_recording() -> bool {
    #[cfg(target_os = "macos")]
    {
        extern "C" {
            fn CGPreflightScreenCaptureAccess() -> bool;
        }
        unsafe { CGPreflightScreenCaptureAccess() }
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// Prompt the user to grant Screen Recording permission.
/// Returns `true` if permission was already granted.
pub fn request_screen_recording() -> bool {
    #[cfg(target_os = "macos")]
    {
        extern "C" {
            fn CGRequestScreenCaptureAccess() -> bool;
        }
        unsafe { CGRequestScreenCaptureAccess() }
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// Open System Settings → Privacy & Security → Screen Recording.
pub fn open_screen_recording_settings() {
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture")
            .spawn();
    }
}

/// Check Accessibility permission.
pub fn check_accessibility() -> bool {
    #[cfg(target_os = "macos")]
    {
        extern "C" {
            fn AXIsProcessTrusted() -> bool;
        }
        unsafe { AXIsProcessTrusted() }
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// Open System Settings → Privacy & Security → Accessibility.
pub fn open_accessibility_settings() {
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
            .spawn();
    }
}

/// Request the macOS Accessibility permission with prompt.
///
/// Calls `AXIsProcessTrustedWithOptions({"AXTrustedCheckOptionPrompt": true})`,
/// which presents the system "wants to control your computer" dialog
/// the *first* time the app needs Accessibility. Returns the current
/// trust state. On subsequent calls (after user grants or denies),
/// the dialog is not re-shown — the function simply returns the trust
/// state.
///
/// Required for CGEvent input injection (see `MacInput`).
#[cfg(target_os = "macos")]
pub fn request_accessibility_for_input() -> bool {
    use core_foundation::base::TCFType;
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::string::CFString;

    extern "C" {
        fn AXIsProcessTrustedWithOptions(options: core_foundation::base::CFTypeRef) -> bool;
    }

    let key = CFString::new("AXTrustedCheckOptionPrompt");
    let value = CFBoolean::true_value();
    let options = CFDictionary::from_CFType_pairs(&[(key, value)]);
    // SAFETY: CFTypeRef from a CFDictionary is valid for the call.
    unsafe { AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef() as _) }
}

#[cfg(not(target_os = "macos"))]
pub fn request_accessibility_for_input() -> bool {
    true
}
