DELETE FROM bookmark_chunk;
DROP INDEX IF EXISTS idx_bookmark_chunk_embedding;
CREATE INDEX idx_bookmark_chunk_embedding ON bookmark_chunk
    USING hnsw (embedding vector_cosine_ops) WITH (m = 16, ef_construction = 64);
INSERT INTO schema_version (version) VALUES (5);
