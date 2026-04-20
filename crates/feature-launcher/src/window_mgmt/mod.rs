#[cfg(target_os = "macos")]
pub mod accessibility;
pub mod actions;
pub mod presets;

pub use actions::WindowManager;
pub use presets::{lookup as lookup_preset, Preset, PresetFrame, PRESETS};
