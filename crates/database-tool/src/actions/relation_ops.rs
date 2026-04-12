//! Relation actions: link, unlink, list_relations.

use common::Result;
use entity_store::relations;
use entity_store::store::EntityStore;
use tools_core::params::ParamExtractor;

pub async fn link(store: &EntityStore, p: &ParamExtractor<'_>) -> Result<String> {
    let db_id = p.required_str("database_id")?;
    let entity_id = p.required_str("entity_id")?;
    let target_id = p.required_str("target_id")?;
    let target_db_id = p.required_str("target_db_id")?;
    let relation_type = p.str_or("relation_type", "related")?;

    let rel = relations::create_relation(
        store.pool(),
        relations::CreateRelationInput {
            source_id: entity_id,
            source_db_id: db_id,
            target_id,
            target_db_id,
            relation_type,
            inferred: false,
            confidence: None,
        },
    )
    .await?;

    Ok(format!(
        "Created relation {} ({}) from {entity_id} -> {target_id}.",
        rel.id, rel.relation_type
    ))
}

pub async fn unlink(store: &EntityStore, p: &ParamExtractor<'_>) -> Result<String> {
    let relation_id = p.required_str("entity_id")?;
    relations::delete_relation(store.pool(), relation_id).await?;
    Ok(format!("Deleted relation {relation_id}."))
}

pub async fn list_relations(store: &EntityStore, p: &ParamExtractor<'_>) -> Result<String> {
    let db_id = p.required_str("database_id")?;
    let entity_id = p.required_str("entity_id")?;
    let rels = relations::list_relations_for_entity(store.pool(), entity_id, db_id).await?;

    if rels.is_empty() {
        return Ok(format!("No relations found for entity {entity_id}."));
    }

    let mut out = format!("Relations for entity {entity_id}:\n");
    for r in &rels {
        out.push_str(&format!(
            "  [{}] {} -- {} -> {} (db {})\n",
            r.id, r.relation_type, r.source_id, r.target_id, r.target_db_id
        ));
    }
    Ok(out)
}
