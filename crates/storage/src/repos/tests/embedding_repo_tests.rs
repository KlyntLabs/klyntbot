//! Unit tests for EmbeddingRepo (pgvector).

#[cfg(test)]
mod tests {
    // TODO: use super::super::fixtures::*;

    #[tokio::test]
    async fn upsert_and_get() {
        // Given: todo exists
        // When: upsert embedding (todo_id, 384-dim vector, model name)
        // Then: get(todo_id) returns embedding record
    }

    #[tokio::test]
    async fn upsert_replaces_existing() {
        // Given: existing embedding for todo
        // When: upsert with new vector
        // Then: old vector replaced (ON CONFLICT UPDATE)
    }

    #[tokio::test]
    async fn delete_embedding() {
        // Given: existing embedding
        // When: delete(todo_id)
        // Then: get returns None
    }

    #[tokio::test]
    async fn nearest_neighbors_by_cosine() {
        // Given: 10 embeddings with known vectors
        // When: search(query_vector, limit=3)
        // Then: returns 3 closest by cosine distance
        //       ORDER BY embedding <=> $query LIMIT 3
        // NOTE: Use deterministic vectors from sample_embedding_384(seed)
    }

    #[tokio::test]
    async fn dimension_mismatch_rejected() {
        // Given: schema expects vector(384)
        // When: insert 128-dim vector
        // Then: Postgres error (dimension mismatch)
    }

    #[tokio::test]
    async fn ids_missing_embeddings() {
        // Given: 5 todos, 3 have embeddings
        // When: ids_missing_embeddings([t1,t2,t3,t4,t5])
        // Then: returns [t4, t5]
        // SQL: SELECT id FROM unnest($ids) WHERE id NOT IN (SELECT todo_id FROM todo_embeddings)
    }
}
