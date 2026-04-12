//! Entity CRUD actions: create, get, list, update, delete.

use std::collections::HashMap;

use common::Result;
use entity_store::query;
use entity_store::store::EntityStore;
use entity_store::{FilterRule, SortRule};
use serde_json::Value;
use tools_core::params::ParamExtractor;

pub fn format_entity_fields(fields: &HashMap<String, serde_json::Value>) -> String {
    let mut out = String::new();
    for (k, v) in fields {
        let display = match v {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        out.push_str(&format!("  {k}: {display}\n"));
    }
    out
}

pub async fn create_entity(
    store: &EntityStore,
    p: &ParamExtractor<'_>,
    args: &Value,
) -> Result<String> {
    let db_id = p.required_str("database_id")?;
    let fields: HashMap<String, serde_json::Value> = match args.get("fields") {
        Some(v) if v.is_object() => serde_json::from_value(v.clone())?,
        _ => HashMap::new(),
    };
    let entity = store.create_entity(db_id, fields).await?;
    let mut out = format!("Created entity {} in database {db_id}:\n", entity.id);
    out.push_str(&format_entity_fields(&entity.fields));
    Ok(out)
}

pub async fn get_entity(store: &EntityStore, p: &ParamExtractor<'_>) -> Result<String> {
    let db_id = p.required_str("database_id")?;
    let entity_id = p.required_str("entity_id")?;
    let entity = store.get_entity(db_id, entity_id).await?;
    let mut out = format!("Entity {} (database {db_id}):\n", entity.id);
    out.push_str(&format_entity_fields(&entity.fields));
    Ok(out)
}

pub async fn list_entities(
    store: &EntityStore,
    p: &ParamExtractor<'_>,
    args: &Value,
) -> Result<String> {
    let db_id = p.required_str("database_id")?;
    let schema = store.get_database(db_id).await?;

    let filters: Vec<FilterRule> = args
        .get("filters")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    let sorts: Vec<SortRule> = args
        .get("sorts")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let params = query::QueryParams {
        filters,
        sorts,
        limit: p.optional_i64("limit")?,
        offset: p.optional_i64("offset")?,
    };

    let result = query::query_entities(store.pool(), &schema, &params).await?;
    if result.entities.is_empty() {
        return Ok(format!("No entities found in database {db_id}."));
    }
    let mut out = format!(
        "Showing {} of {} entities in database {db_id}:\n",
        result.entities.len(),
        result.total
    );
    for e in &result.entities {
        out.push_str(&format!("\n[{}]\n", e.id));
        out.push_str(&format_entity_fields(&e.fields));
    }
    Ok(out)
}

pub async fn update_entity(
    store: &EntityStore,
    p: &ParamExtractor<'_>,
    args: &Value,
) -> Result<String> {
    let db_id = p.required_str("database_id")?;
    let entity_id = p.required_str("entity_id")?;
    let fields: HashMap<String, serde_json::Value> = match args.get("fields") {
        Some(v) if v.is_object() => serde_json::from_value(v.clone())?,
        _ => HashMap::new(),
    };
    let entity = store.update_entity(db_id, entity_id, fields).await?;
    let mut out = format!("Updated entity {} in database {db_id}:\n", entity.id);
    out.push_str(&format_entity_fields(&entity.fields));
    Ok(out)
}

pub async fn delete_entity(store: &EntityStore, p: &ParamExtractor<'_>) -> Result<String> {
    let db_id = p.required_str("database_id")?;
    let entity_id = p.required_str("entity_id")?;
    store.delete_entity(db_id, entity_id).await?;
    Ok(format!("Deleted entity {entity_id} from database {db_id}."))
}
