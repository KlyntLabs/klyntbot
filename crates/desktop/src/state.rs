use crate::app_core::AppCore;

/// Wrapper for Tauri managed state.
pub struct ManagedState {
    pub core: AppCore,
}

impl ManagedState {
    pub fn new() -> Self {
        Self {
            core: AppCore::new(),
        }
    }
}
