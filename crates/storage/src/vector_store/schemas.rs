//! Arrow schema definitions for all LanceDB tables.
//!
//! Column ordering convention (must match upsert_embedding column construction):
//!   [0]   id         Utf8
//!   [1]   vector     FixedSizeList<Float32, 384>
//!   [2..] extra      Utf8   (table-specific string fields)
//!   last  timestamp  Utf8   (updated_at / created_at)

use std::sync::Arc;

use arrow_schema::{DataType, Field, Schema};

pub(crate) fn vector_field() -> Field {
    Field::new(
        "vector",
        DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Float32, true)), 384),
        false,
    )
}

pub(crate) fn todo_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        vector_field(),
        Field::new("model", DataType::Utf8, false),
        Field::new("updated_at", DataType::Utf8, false),
    ])
}

pub(crate) fn conv_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        vector_field(),
        Field::new("session_key", DataType::Utf8, false),
        Field::new("role", DataType::Utf8, false),
        Field::new("content_preview", DataType::Utf8, false),
        Field::new("created_at", DataType::Utf8, false),
    ])
}

pub(crate) fn activity_embedding_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        vector_field(),
        Field::new("source", DataType::Utf8, false),
        Field::new("work_context_id", DataType::Utf8, false),
        Field::new("timestamp", DataType::Utf8, false),
        Field::new("updated_at", DataType::Utf8, false),
    ])
}

pub(crate) fn work_context_embedding_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        vector_field(),
        Field::new("updated_at", DataType::Utf8, false),
    ])
}

pub(crate) fn task_embedding_schema() -> Schema {
    todo_schema() // Structurally identical to todo_embeddings
}

pub(crate) fn note_embedding_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        vector_field(),
        Field::new("model", DataType::Utf8, false),
        Field::new("updated_at", DataType::Utf8, false),
    ])
}

pub(crate) fn insight_embedding_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        vector_field(),
        Field::new("updated_at", DataType::Utf8, false),
    ])
}

pub(crate) fn entity_embedding_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        vector_field(),
        Field::new("name", DataType::Utf8, false),
        Field::new("entity_type", DataType::Utf8, false),
        Field::new("description", DataType::Utf8, true),
        Field::new("updated_at", DataType::Utf8, false),
    ])
}

pub(crate) fn flashcard_embedding_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        vector_field(),
        Field::new("card_id", DataType::Utf8, false),
        Field::new("side", DataType::Utf8, false),
        Field::new("timestamp", DataType::Utf8, false),
    ])
}

pub(crate) fn cognitive_fact_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        vector_field(),
        Field::new("domain", DataType::Utf8, false),
        Field::new("text", DataType::Utf8, false),
        Field::new("importance", DataType::Utf8, false),
        Field::new("stability", DataType::Utf8, false),
        Field::new("confidence", DataType::Utf8, false),
        Field::new("updated_at", DataType::Utf8, false),
    ])
}

pub(crate) fn tree_node_embedding_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        vector_field(),
        Field::new("note_id", DataType::Utf8, false),
        Field::new("level", DataType::Utf8, false),
        Field::new("source_type", DataType::Utf8, false),
        Field::new("updated_at", DataType::Utf8, false),
    ])
}

pub(crate) fn community_embedding_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        vector_field(),
        Field::new("member_count", DataType::Utf8, false),
        Field::new("source_note_count", DataType::Utf8, false),
        Field::new("updated_at", DataType::Utf8, false),
    ])
}

pub(crate) fn database_entity_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        vector_field(),
        Field::new("database_id", DataType::Utf8, false),
        Field::new("field_context", DataType::Utf8, false),
        Field::new("updated_at", DataType::Utf8, false),
    ])
}

/// Map a table name to its Arrow schema.
///
/// Returns `None` if the table name is not recognized.
pub(crate) fn schema_for_table(name: &str) -> Option<Schema> {
    if name.starts_with("db_") && name.ends_with("_embeddings") {
        return Some(database_entity_schema());
    }
    match name {
        "todo_embeddings" => Some(todo_schema()),
        "task_embeddings" => Some(task_embedding_schema()),
        "note_embeddings" => Some(note_embedding_schema()),
        "conv_embeddings" => Some(conv_schema()),
        "cognitive_fact_embeddings" => Some(cognitive_fact_schema()),
        "activity_embeddings" => Some(activity_embedding_schema()),
        "work_context_embeddings" => Some(work_context_embedding_schema()),
        "insight_embeddings" => Some(insight_embedding_schema()),
        "entity_embeddings" => Some(entity_embedding_schema()),
        "flashcard_embeddings" => Some(flashcard_embedding_schema()),
        "tree_node_embeddings" => Some(tree_node_embedding_schema()),
        "community_embeddings" => Some(community_embedding_schema()),
        _ => None,
    }
}
