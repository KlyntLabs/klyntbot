#[cfg(target_os = "macos")]
pub mod accessibility;
pub mod actions;
pub mod presets;

pub use actions::WindowManager;
pub use presets::{lookup as lookup_preset, Preset, PresetFrame, PRESETS};

use std::sync::OnceLock;

pub fn global() -> &'static WindowManager {
    static INSTANCE: OnceLock<WindowManager> = OnceLock::new();
    INSTANCE.get_or_init(WindowManager::default)
}
