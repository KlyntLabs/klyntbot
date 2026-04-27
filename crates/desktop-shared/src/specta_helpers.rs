//! Specta type overrides for non-trivial Rust types we cross the IPC boundary.
//!
//! `serde_json::Value` and `jiff::Timestamp` don't implement `specta::Type`
//! directly. Use these markers in `#[specta(type = ...)]` attributes:
//!
//! ```ignore
//! #[derive(serde::Serialize, serde::Deserialize, specta::Type)]
//! pub struct Cron {
//!     #[specta(type = crate::specta_helpers::JsonValue)]
//!     pub schedule: serde_json::Value,
//!     #[specta(type = crate::specta_helpers::Timestamp)]
//!     pub when: jiff::Timestamp,
//! }
//! ```

use serde::{Deserialize, Serialize};
use specta::{datatype::DataType, Generics, Type, TypeMap};

/// Renders to TypeScript `unknown`. Use as `#[specta(type = JsonValue)]` on
/// every field whose Rust type is `serde_json::Value`.
pub struct JsonValue;

impl Type for JsonValue {
    fn inline(_type_map: &mut TypeMap, _generics: Generics) -> DataType {
        DataType::Unknown
    }
}

/// Newtype wrapper around `serde_json::Value` that implements `specta::Type`.
///
/// Use this for command parameters or return types that need to pass arbitrary
/// JSON when the underlying type is `serde_json::Value`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonValueWrapper(pub serde_json::Value);

impl Type for JsonValueWrapper {
    fn inline(_type_map: &mut TypeMap, _generics: Generics) -> DataType {
        DataType::Unknown
    }
}

/// Renders to TypeScript `string` (RFC 3339 timestamp). Use as
/// `#[specta(type = Timestamp)]` on every field whose Rust type is
/// `jiff::Timestamp` — jiff serialises timestamps as RFC 3339 strings already.
pub struct Timestamp;

impl Type for Timestamp {
    fn inline(_type_map: &mut TypeMap, _generics: Generics) -> DataType {
        DataType::Primitive(specta::datatype::PrimitiveType::String)
    }
}
