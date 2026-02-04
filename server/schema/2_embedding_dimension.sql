-- Migration: Update embedding dimension from 768 to 1024 for mxbai-embed-large
-- Drops index, alters column, deletes chunks to trigger re-embedding

DROP INDEX IF EXISTS idx_bookmark_chunk_embedding;

ALTER TABLE bookmark_chunk
    ALTER COLUMN embedding TYPE VECTOR(1024);

-- Delete all chunks so embeddings daemon regenerates them
DELETE FROM bookmark_chunk;

CREATE INDEX idx_bookmark_chunk_embedding ON bookmark_chunk
    USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);

INSERT INTO schema_version (version) VALUES (2);
