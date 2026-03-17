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
        Field::new("full_content", DataType::Utf8, false),
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
