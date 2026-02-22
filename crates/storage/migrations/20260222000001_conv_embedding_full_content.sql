-- Add full message content alongside the 100-char preview.
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'conversation_embeddings') THEN
        ALTER TABLE conversation_embeddings ADD COLUMN IF NOT EXISTS content_full TEXT NOT NULL DEFAULT '';
        RAISE NOTICE 'Added content_full column to conversation_embeddings';
    END IF;
END $$;
