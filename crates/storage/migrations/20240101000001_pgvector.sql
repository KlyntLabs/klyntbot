-- Conditional pgvector setup.
-- Creates embedding tables only when the pgvector extension is available.
-- If pgvector is not installed, this migration is a no-op and the app
-- starts normally with semantic search disabled.

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_available_extensions WHERE name = 'vector') THEN
        CREATE EXTENSION IF NOT EXISTS vector;

        -- Todo Embeddings
        CREATE TABLE IF NOT EXISTS todo_embeddings (
            todo_id    VARCHAR(8) PRIMARY KEY REFERENCES todos(id) ON DELETE CASCADE,
            embedding  vector(384) NOT NULL,
            model      TEXT NOT NULL DEFAULT 'paraphrase-multilingual-MiniLM-L12-v2',
            updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
        );

        -- IVFFlat index for approximate nearest neighbor search.
        IF NOT EXISTS (
            SELECT 1 FROM pg_indexes WHERE indexname = 'idx_todo_embeddings_ann'
        ) THEN
            CREATE INDEX idx_todo_embeddings_ann ON todo_embeddings
                USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);
        END IF;

        -- Conversation Embeddings
        CREATE TABLE IF NOT EXISTS conversation_embeddings (
            id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
            session_key     TEXT NOT NULL,
            embedding       vector(384) NOT NULL,
            role            TEXT NOT NULL,
            content_preview TEXT NOT NULL,
            created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
        );

        IF NOT EXISTS (
            SELECT 1 FROM pg_indexes WHERE indexname = 'idx_conv_embeddings_ann'
        ) THEN
            CREATE INDEX idx_conv_embeddings_ann ON conversation_embeddings
                USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);
        END IF;

        RAISE NOTICE 'pgvector extension enabled — embedding tables created';
    ELSE
        RAISE NOTICE 'pgvector not available — skipping embedding tables (semantic search disabled)';
    END IF;
END $$;
