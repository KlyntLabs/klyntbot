//! Search action: keyword search across text fields.

use common::Result;
use entity_store::store::EntityStore;
use tools_core::params::ParamExtractor;

pub async fn search(store: &EntityStore, p: &ParamExtractor<'_>) -> Result<String> {
    let db_id = p.required_str("database_id")?;
    let query_str = p.required_str("query")?;
    let limit = p.i64_or("limit", 20)? as usize;

    let matched = store.search_entities(db_id, query_str, limit).await?;

    if matched.is_empty() {
        return Ok(format!(
            "No results found for '{query_str}' in database {db_id}."
        ));
    }

    let mut out = format!("Found {} results for '{query_str}':\n", matched.len());
    for (i, e) in matched.iter().enumerate() {
        let summary: Vec<String> = e
            .fields
            .iter()
            .take(3)
            .map(|(k, v)| {
                let display = match v {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                format!("{k}: {display}")
            })
            .collect();
        out.push_str(&format!("{}. [{}] {}\n", i + 1, e.id, summary.join(" | ")));
    }
    Ok(out)
}
