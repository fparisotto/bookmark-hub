-- Reset derived AI data and migrate to the unified text/embedding pipeline state.

ALTER TABLE bookmark_task
    DROP COLUMN IF EXISTS summary;

DROP INDEX IF EXISTS idx_bookmark_summary_pending;
DROP INDEX IF EXISTS idx_bookmark_tag_pending;

ALTER TABLE bookmark
    DROP COLUMN IF EXISTS summary_attempts,
    DROP COLUMN IF EXISTS summary_next_attempt_at,
    DROP COLUMN IF EXISTS summary_fail_reason,
    DROP COLUMN IF EXISTS tag_attempts,
    DROP COLUMN IF EXISTS tag_next_attempt_at,
    DROP COLUMN IF EXISTS tag_fail_reason;

ALTER TABLE bookmark
    ADD COLUMN IF NOT EXISTS text_ai_status task_status NOT NULL DEFAULT 'pending',
    ADD COLUMN IF NOT EXISTS text_ai_attempts SMALLINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS text_ai_next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    ADD COLUMN IF NOT EXISTS text_ai_fail_reason TEXT,
    ADD COLUMN IF NOT EXISTS text_ai_pipeline_version INTEGER NOT NULL DEFAULT 1,
    ADD COLUMN IF NOT EXISTS embedding_status task_status NOT NULL DEFAULT 'pending',
    ADD COLUMN IF NOT EXISTS embedding_attempts SMALLINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS embedding_next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    ADD COLUMN IF NOT EXISTS embedding_fail_reason TEXT,
    ADD COLUMN IF NOT EXISTS embedding_pipeline_version INTEGER NOT NULL DEFAULT 1;

CREATE TABLE IF NOT EXISTS bookmark_ai_chunk (
    bookmark_id VARCHAR(512) NOT NULL,
    user_id UUID NOT NULL,
    chunk_index INTEGER NOT NULL,
    chunk_hash TEXT NOT NULL,
    pipeline_version INTEGER NOT NULL,
    summary TEXT NOT NULL,
    tags TEXT[] NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (bookmark_id, user_id, chunk_index),
    CONSTRAINT fk_bookmark_ai_chunk FOREIGN KEY (bookmark_id, user_id)
        REFERENCES bookmark(bookmark_id, user_id) ON DELETE CASCADE
);

TRUNCATE TABLE bookmark_ai_chunk;
TRUNCATE TABLE bookmark_chunk;

UPDATE bookmark
SET summary = NULL,
    tags = NULL,
    summary_status = 'pending'::task_status,
    tag_status = 'pending'::task_status,
    text_ai_status = 'pending'::task_status,
    text_ai_attempts = 0,
    text_ai_next_attempt_at = now(),
    text_ai_fail_reason = NULL,
    text_ai_pipeline_version = 1,
    embedding_status = CASE
        WHEN LENGTH(text_content) >= 200 THEN 'pending'::task_status
        ELSE 'done'::task_status
    END,
    embedding_attempts = 0,
    embedding_next_attempt_at = now(),
    embedding_fail_reason = NULL,
    embedding_pipeline_version = 1,
    updated_at = now();

CREATE INDEX IF NOT EXISTS idx_bookmark_text_ai_pending
    ON bookmark (text_ai_next_attempt_at, created_at)
    WHERE text_ai_status = 'pending';

CREATE INDEX IF NOT EXISTS idx_bookmark_embedding_pending
    ON bookmark (embedding_next_attempt_at, created_at)
    WHERE embedding_status = 'pending';

INSERT INTO schema_version (version) VALUES (9);
