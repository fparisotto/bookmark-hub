-- Migration: Update embedding dimension from 768 to 1024 for mxbai-embed-large
-- Deletes chunks first (required before dimension change), then alters column

-- Delete all chunks first - pgvector cannot convert existing 768-dim vectors to 1024-dim
DELETE FROM bookmark_chunk;

DROP INDEX IF EXISTS idx_bookmark_chunk_embedding;

ALTER TABLE bookmark_chunk
    ALTER COLUMN embedding TYPE VECTOR(1024);

CREATE INDEX idx_bookmark_chunk_embedding ON bookmark_chunk
    USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);

INSERT INTO schema_version (version) VALUES (2);
