//! Entity card data emitted when tools create entities.

use serde::Serialize;
use std::collections::HashMap;

/// Card data for an entity created by a tool.
/// Sent through RoutingContext to the agent event stream.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityCard {
    pub entity_type: String,
    pub entity_id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub route: Option<String>,
    pub icon_hint: String,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}
