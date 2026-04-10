-- Add retry-aware processing state for AI-generated summaries and tags.

ALTER TABLE bookmark
    ADD COLUMN IF NOT EXISTS summary_status task_status NOT NULL DEFAULT 'pending';
ALTER TABLE bookmark
    ADD COLUMN IF NOT EXISTS summary_attempts SMALLINT NOT NULL DEFAULT 0;
ALTER TABLE bookmark
    ADD COLUMN IF NOT EXISTS summary_next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now();
ALTER TABLE bookmark
    ADD COLUMN IF NOT EXISTS summary_fail_reason TEXT;
ALTER TABLE bookmark
    ADD COLUMN IF NOT EXISTS tag_status task_status NOT NULL DEFAULT 'pending';
ALTER TABLE bookmark
    ADD COLUMN IF NOT EXISTS tag_attempts SMALLINT NOT NULL DEFAULT 0;
ALTER TABLE bookmark
    ADD COLUMN IF NOT EXISTS tag_next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now();
ALTER TABLE bookmark
    ADD COLUMN IF NOT EXISTS tag_fail_reason TEXT;

UPDATE bookmark
SET summary_status = CASE
        WHEN NULLIF(BTRIM(summary), '') IS NOT NULL THEN 'done'::task_status
        ELSE 'pending'::task_status
    END,
    summary_attempts = 0,
    summary_next_attempt_at = now(),
    summary_fail_reason = NULL,
    tag_status = CASE
        WHEN COALESCE(array_length(tags, 1), 0) > 0 THEN 'done'::task_status
        ELSE 'pending'::task_status
    END,
    tag_attempts = 0,
    tag_next_attempt_at = now(),
    tag_fail_reason = NULL;

CREATE INDEX IF NOT EXISTS idx_bookmark_summary_pending ON bookmark (summary_next_attempt_at, created_at)
    WHERE summary_status = 'pending';

CREATE INDEX IF NOT EXISTS idx_bookmark_tag_pending ON bookmark (tag_next_attempt_at, created_at)
    WHERE tag_status = 'pending';

INSERT INTO schema_version (version) VALUES (6);
