-- Make embedding storage dimension-agnostic and store active profile separately.
DROP INDEX IF EXISTS idx_bookmark_chunk_embedding;

ALTER TABLE bookmark_chunk
    ALTER COLUMN embedding TYPE vector
    USING embedding::vector;

CREATE TABLE IF NOT EXISTS embedding_config (
    embedding_config_id BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (embedding_config_id),
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    dimensions INTEGER NOT NULL CHECK (dimensions > 0),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO schema_version (version) VALUES (8);
