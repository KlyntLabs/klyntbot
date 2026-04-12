//! View actions: create_view, update_view, delete_view, list_views.

use common::Result;
use entity_store::store::EntityStore;
use entity_store::{views, ViewConfig, ViewType};
use serde_json::Value;
use tools_core::params::ParamExtractor;

pub async fn create_view(
    store: &EntityStore,
    p: &ParamExtractor<'_>,
    args: &Value,
) -> Result<String> {
    let db_id = p.required_str("database_id")?;
    let name = p.required_str("name")?;
    let vt_str = p.str_or("view_type", "table")?;
    let view_type: ViewType = serde_json::from_value(serde_json::Value::String(vt_str.to_string()))
        .map_err(|e| common::ToolError::InvalidParams(format!("Invalid view_type: {e}")))?;

    let config: ViewConfig = args
        .get("config")
        .and_then(|v| serde_json::from_value::<ViewConfig>(v.clone()).ok())
        .unwrap_or_default();

    let view = views::create_view(store.pool(), db_id, name, view_type, config, false).await?;
    Ok(format!(
        "Created view '{}' ({:?}) for database {db_id} (view id: {}).",
        view.name, view.view_type, view.id
    ))
}

pub async fn update_view(
    store: &EntityStore,
    p: &ParamExtractor<'_>,
    args: &Value,
) -> Result<String> {
    let view_id = p.required_str("view_id")?;
    let name = p.optional_str("name")?;
    let config: Option<ViewConfig> = args
        .get("config")
        .and_then(|v| serde_json::from_value::<ViewConfig>(v.clone()).ok());

    let view = views::update_view(store.pool(), view_id, name, config).await?;
    Ok(format!("Updated view '{}' (id: {}).", view.name, view.id))
}

pub async fn delete_view(store: &EntityStore, p: &ParamExtractor<'_>) -> Result<String> {
    let view_id = p.required_str("view_id")?;
    views::delete_view(store.pool(), view_id).await?;
    Ok(format!("Deleted view {view_id}."))
}

pub async fn list_views(store: &EntityStore, p: &ParamExtractor<'_>) -> Result<String> {
    let db_id = p.required_str("database_id")?;
    let views = store.list_views(db_id).await?;
    if views.is_empty() {
        return Ok(format!("No views found for database {db_id}."));
    }
    let mut out = format!("Views for database {db_id}:\n");
    for v in &views {
        let default_marker = if v.is_default { " [default]" } else { "" };
        out.push_str(&format!(
            "  - {} ({:?}) id: {}{}\n",
            v.name, v.view_type, v.id, default_marker
        ));
    }
    Ok(out)
}
