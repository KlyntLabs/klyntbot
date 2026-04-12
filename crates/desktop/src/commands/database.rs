use desktop_shared::errors::ApiError;
use entity_store::{DatabaseSchema, Entity, FieldDefinition};
use serde_json::Value;
use std::sync::Arc;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn db_list(state: State<'_, Arc<AppCore>>) -> Result<Vec<DatabaseSchema>, ApiError> {
    state.db_list().await
}

#[tauri::command]
pub async fn db_get_schema(
    state: State<'_, Arc<AppCore>>,
    database_id: String,
) -> Result<DatabaseSchema, ApiError> {
    state.db_get_schema(database_id).await
}

#[tauri::command]
pub async fn db_create_database(
    state: State<'_, Arc<AppCore>>,
    name: String,
    slug: Option<String>,
    icon: Option<String>,
    description: Option<String>,
) -> Result<DatabaseSchema, ApiError> {
    state
        .db_create_database(name, slug, icon, description)
        .await
}

#[tauri::command]
pub async fn db_delete_database(
    state: State<'_, Arc<AppCore>>,
    database_id: String,
) -> Result<(), ApiError> {
    state.db_delete_database(database_id).await
}

#[tauri::command]
pub async fn db_query(
    state: State<'_, Arc<AppCore>>,
    database_id: String,
    filters: Option<Value>,
    sorts: Option<Value>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Value, ApiError> {
    state
        .db_query(database_id, filters, sorts, limit, offset)
        .await
}

#[tauri::command]
pub async fn db_create_entity(
    state: State<'_, Arc<AppCore>>,
    database_id: String,
    fields: Value,
) -> Result<Entity, ApiError> {
    state.db_create_entity(database_id, fields).await
}

#[tauri::command]
pub async fn db_update_entity(
    state: State<'_, Arc<AppCore>>,
    database_id: String,
    entity_id: String,
    fields: Value,
) -> Result<Entity, ApiError> {
    state.db_update_entity(database_id, entity_id, fields).await
}

#[tauri::command]
pub async fn db_delete_entity(
    state: State<'_, Arc<AppCore>>,
    database_id: String,
    entity_id: String,
) -> Result<(), ApiError> {
    state.db_delete_entity(database_id, entity_id).await
}

#[tauri::command]
pub async fn db_add_field(
    state: State<'_, Arc<AppCore>>,
    database_id: String,
    name: String,
    field_type: String,
    options: Option<Value>,
) -> Result<FieldDefinition, ApiError> {
    state
        .db_add_field(database_id, name, field_type, options)
        .await
}

#[tauri::command]
pub async fn db_remove_field(
    state: State<'_, Arc<AppCore>>,
    database_id: String,
    field_id: String,
) -> Result<(), ApiError> {
    state.db_remove_field(database_id, field_id).await
}

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "db_list",
    "db_get_schema",
    "db_create_database",
    "db_delete_database",
    "db_query",
    "db_create_entity",
    "db_update_entity",
    "db_delete_entity",
    "db_add_field",
    "db_remove_field",
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "db_list" => dev::val(core.db_list().await),
        "db_get_schema" => {
            let id = try_field!(dev::get_str(body, "databaseId"));
            dev::val(core.db_get_schema(id).await)
        }
        "db_create_database" => {
            let name = try_field!(dev::get_str(body, "name"));
            let slug: Option<String> = dev::get(body, "slug");
            let icon: Option<String> = dev::get(body, "icon");
            let description: Option<String> = dev::get(body, "description");
            dev::val(core.db_create_database(name, slug, icon, description).await)
        }
        "db_delete_database" => {
            let id = try_field!(dev::get_str(body, "databaseId"));
            dev::val(core.db_delete_database(id).await)
        }
        "db_query" => {
            let id = try_field!(dev::get_str(body, "databaseId"));
            let filters: Option<serde_json::Value> = dev::get(body, "filters");
            let sorts: Option<serde_json::Value> = dev::get(body, "sorts");
            let limit: Option<i64> = dev::get(body, "limit");
            let offset: Option<i64> = dev::get(body, "offset");
            dev::val(core.db_query(id, filters, sorts, limit, offset).await)
        }
        "db_create_entity" => {
            let db_id = try_field!(dev::get_str(body, "databaseId"));
            let fields: serde_json::Value = try_field!(dev::require(body, "fields"));
            dev::val(core.db_create_entity(db_id, fields).await)
        }
        "db_update_entity" => {
            let db_id = try_field!(dev::get_str(body, "databaseId"));
            let entity_id = try_field!(dev::get_str(body, "entityId"));
            let fields: serde_json::Value = try_field!(dev::require(body, "fields"));
            dev::val(core.db_update_entity(db_id, entity_id, fields).await)
        }
        "db_delete_entity" => {
            let db_id = try_field!(dev::get_str(body, "databaseId"));
            let entity_id = try_field!(dev::get_str(body, "entityId"));
            dev::val(core.db_delete_entity(db_id, entity_id).await)
        }
        "db_add_field" => {
            let db_id = try_field!(dev::get_str(body, "databaseId"));
            let name = try_field!(dev::get_str(body, "name"));
            let field_type = try_field!(dev::get_str(body, "fieldType"));
            let options: Option<serde_json::Value> = dev::get(body, "options");
            dev::val(core.db_add_field(db_id, name, field_type, options).await)
        }
        "db_remove_field" => {
            let db_id = try_field!(dev::get_str(body, "databaseId"));
            let field_id = try_field!(dev::get_str(body, "fieldId"));
            dev::val(core.db_remove_field(db_id, field_id).await)
        }
        _ => return None,
    })
}
