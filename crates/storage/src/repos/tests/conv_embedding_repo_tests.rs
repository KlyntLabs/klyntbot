//! Unit tests for ConvEmbeddingRepo (conversation embeddings with pgvector).

#[cfg(test)]
mod tests {
    // TODO: use super::super::fixtures::*;

    #[tokio::test]
    async fn insert_conversation_embedding() {
        // Given: empty DB
        // When: insert(session_key, role="user", content_preview="hello", 384-dim vector)
        // Then: row persisted with UUID PK
    }

    #[tokio::test]
    async fn search_by_vector_similarity() {
        // Given: 10 conversation embeddings
        // When: search(query_vector, limit=5)
        // Then: returns 5 closest by cosine distance
    }

    #[tokio::test]
    async fn filter_by_session_key() {
        // Given: embeddings from session "a" and session "b"
        // When: search with session_key="a"
        // Then: returns only session "a" embeddings
    }
}
