-- Migration: Update embedding dimension from 768 to 1024 for mxbai-embed-large
-- pgvector does not allow changing vector dimensions with ALTER, so we must drop and recreate the column

-- Delete all chunks first
DELETE FROM bookmark_chunk;

-- Drop the index
DROP INDEX IF EXISTS idx_bookmark_chunk_embedding;

-- Drop and recreate the embedding column with correct dimensions
ALTER TABLE bookmark_chunk DROP COLUMN embedding;
ALTER TABLE bookmark_chunk ADD COLUMN embedding VECTOR(1024);

-- Recreate the index
CREATE INDEX idx_bookmark_chunk_embedding ON bookmark_chunk
    USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);

INSERT INTO schema_version (version) VALUES (2);
