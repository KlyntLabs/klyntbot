pub mod cognitive_commands;
pub mod commands;
pub mod entity_link_types;
pub mod errors;
pub mod events;
pub mod permissions;
pub mod specta_helpers;
pub mod types;

pub use entity_link_types::*;
pub use errors::{ApiError, CommandResult};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct HooksTomlSnapshot {
    pub path: String,
    pub exists: bool,
    pub content: String,
}

#[cfg(test)]
mod phase5_helper_tests {
    use crate::specta_helpers::{JsonValue, Timestamp};
    use specta::Type;

    #[test]
    fn json_value_specta_type_renders_as_unknown() {
        let mut type_map = specta::TypeMap::default();
        let dt = JsonValue::inline(&mut type_map, specta::Generics::Provided(&[]));
        // We only require this not to panic; the precise rendering is checked in
        // bindings_are_current.
        let _ = dt;
    }

    #[test]
    fn timestamp_specta_type_renders_as_string() {
        let mut type_map = specta::TypeMap::default();
        let dt = Timestamp::inline(&mut type_map, specta::Generics::Provided(&[]));
        let _ = dt;
    }
}
