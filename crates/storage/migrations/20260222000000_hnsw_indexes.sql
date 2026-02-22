-- Upgrade pgvector indexes from IVFFlat to HNSW.
-- HNSW handles continuous inserts without needing VACUUM ANALYZE.

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_indexes WHERE indexname = 'idx_todo_embeddings_ann') THEN
        DROP INDEX idx_todo_embeddings_ann;
        CREATE INDEX idx_todo_embeddings_ann ON todo_embeddings
            USING hnsw (embedding vector_cosine_ops) WITH (m = 16, ef_construction = 64);
        RAISE NOTICE 'Upgraded todo_embeddings to HNSW index';
    END IF;

    IF EXISTS (SELECT 1 FROM pg_indexes WHERE indexname = 'idx_conv_embeddings_ann') THEN
        DROP INDEX idx_conv_embeddings_ann;
        CREATE INDEX idx_conv_embeddings_ann ON conversation_embeddings
            USING hnsw (embedding vector_cosine_ops) WITH (m = 16, ef_construction = 64);
        RAISE NOTICE 'Upgraded conversation_embeddings to HNSW index';
    END IF;
END $$;
