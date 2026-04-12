//! Database-level actions: create, list, get_schema, delete.

use common::Result;
use entity_store::store::EntityStore;
use entity_store::CreateDatabaseInput;
use serde_json::Value;
use tools_core::params::ParamExtractor;

pub async fn create_database(
    store: &EntityStore,
    p: &ParamExtractor<'_>,
    _args: &Value,
) -> Result<String> {
    let name = p.required_str("name")?;
    let input = CreateDatabaseInput {
        name: name.to_string(),
        slug: p.optional_str("slug")?.map(String::from),
        icon: p.optional_str("icon")?.map(String::from),
        description: p.optional_str("description")?.map(String::from),
        template_id: None,
    };
    let db = store.create_database(input).await?;
    Ok(format!(
        "Created database '{}' (id: {}) with slug '{}'",
        db.name, db.id, db.slug
    ))
}

pub async fn list_databases(store: &EntityStore) -> Result<String> {
    let dbs = store.list_databases().await?;
    if dbs.is_empty() {
        return Ok("No databases found.".to_string());
    }
    let mut out = String::from("Databases:\n");
    for (i, db) in dbs.iter().enumerate() {
        out.push_str(&format!(
            "{}. {} ({}) -- {} fields, {} views\n",
            i + 1,
            db.name,
            db.id,
            db.fields.len(),
            db.views.len()
        ));
    }
    Ok(out)
}

pub async fn get_schema(store: &EntityStore, p: &ParamExtractor<'_>) -> Result<String> {
    let db_id = p.required_str("database_id")?;
    let db = store.get_database(db_id).await?;
    let mut out = format!("Database: {} ({})\nSlug: {}\n", db.name, db.id, db.slug);
    if let Some(ref desc) = db.description {
        out.push_str(&format!("Description: {desc}\n"));
    }
    if db.fields.is_empty() {
        out.push_str("Fields: (none)\n");
    } else {
        out.push_str("Fields:\n");
        for f in &db.fields {
            let req = if f.required { " [required]" } else { "" };
            out.push_str(&format!(
                "  - {} ({}): {:?}{}\n",
                f.name, f.slug, f.field_type, req
            ));
        }
    }
    if !db.views.is_empty() {
        out.push_str("Views:\n");
        for v in &db.views {
            out.push_str(&format!("  - {} ({:?})\n", v.name, v.view_type));
        }
    }
    Ok(out)
}

pub async fn delete_database(store: &EntityStore, p: &ParamExtractor<'_>) -> Result<String> {
    let db_id = p.required_str("database_id")?;
    let db = store.get_database(db_id).await?;
    let name = db.name.clone();
    store.delete_database(db_id).await?;
    Ok(format!("Deleted database '{name}' ({db_id})."))
}
