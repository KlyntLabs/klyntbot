//! macOS shortcut bridge for Focus (DND) activation.
//!
//! The two bundled `.shortcut` assets are exposed as byte-slice consts so they
//! compile on every platform — the bytes are just data; running them requires
//! macOS. The `MacosFocusBridge` impl lives in `macos.rs` (cfg-gated).

/// Bundled "Klyntbot Focus On" shortcut binary.
pub const FOCUS_ON_BYTES: &[u8] =
    include_bytes!("../../assets/Klyntbot Focus On.shortcut");

/// Bundled "Klyntbot Focus Off" shortcut binary.
pub const FOCUS_OFF_BYTES: &[u8] =
    include_bytes!("../../assets/Klyntbot Focus Off.shortcut");

#[cfg(target_os = "macos")]
pub mod macos;
