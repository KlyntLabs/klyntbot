//! Single source of truth for the tauri-specta Builder.
//!
//! Both `main.rs` (production runtime) and `tests/bindings_are_current.rs`
//! (CI drift check) call `build_specta()` to ensure they see the *same*
//! command + event list. Add new commands and events here.

use tauri_specta::{collect_commands, collect_events, Builder};

pub fn build_specta() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        // Commands and events are added in later phases.
        .commands(collect_commands![])
        .events(collect_events![])
}
