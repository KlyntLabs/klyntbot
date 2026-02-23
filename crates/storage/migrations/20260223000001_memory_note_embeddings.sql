-- Memory note embeddings for semantic relevance filtering
CREATE TABLE IF NOT EXISTS memory_note_embeddings (
    note_key   TEXT PRIMARY KEY REFERENCES memory_notes(note_key) ON DELETE CASCADE,
    embedding  vector(384) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- HNSW index for fast ANN search
CREATE INDEX IF NOT EXISTS idx_memory_note_embeddings_ann
    ON memory_note_embeddings USING hnsw (embedding vector_cosine_ops);
