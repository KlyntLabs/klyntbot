//! Field actions: add_field, remove_field.

use common::Result;
use entity_store::store::EntityStore;
use entity_store::{CreateFieldInput, FieldType};
use serde_json::Value;
use tools_core::params::ParamExtractor;

pub async fn add_field(
    store: &EntityStore,
    p: &ParamExtractor<'_>,
    args: &Value,
) -> Result<String> {
    let db_id = p.required_str("database_id")?;
    let name = p.required_str("name")?;
    let ft_str = p.required_str("field_type")?;
    let field_type: FieldType =
        serde_json::from_value(serde_json::Value::String(ft_str.to_string()))
            .map_err(|e| common::ToolError::InvalidParams(format!("Invalid field_type: {e}")))?;

    let options = args.get("options").cloned();
    let required = p.optional_bool("required")?;

    let input = CreateFieldInput {
        name: name.to_string(),
        slug: p.optional_str("slug")?.map(String::from),
        field_type,
        options,
        required,
        default_value: None,
        position: None,
    };
    let field = store.add_field(db_id, input).await?;
    Ok(format!(
        "Added field '{}' ({:?}) to database {db_id} (field id: {})",
        field.name, field.field_type, field.id
    ))
}

pub async fn remove_field(store: &EntityStore, p: &ParamExtractor<'_>) -> Result<String> {
    let db_id = p.required_str("database_id")?;
    let field_id = p.required_str("field_id")?;
    store.remove_field(db_id, field_id, true).await?;
    Ok(format!("Removed field {field_id} from database {db_id}."))
}
