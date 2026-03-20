-- Trim whitespace around tags, drop empty values, and deduplicate normalized tags
UPDATE bookmark
SET tags = (
        SELECT ARRAY(
            SELECT DISTINCT LOWER(BTRIM(tag))
            FROM unnest(tags) AS tag
            WHERE NULLIF(BTRIM(tag), '') IS NOT NULL
        )
    ),
    updated_at = now()
WHERE tags IS NOT NULL AND array_length(tags, 1) > 0;

UPDATE bookmark_task
SET tags = (
        SELECT ARRAY(
            SELECT DISTINCT LOWER(BTRIM(tag))
            FROM unnest(tags) AS tag
            WHERE NULLIF(BTRIM(tag), '') IS NOT NULL
        )
    )
WHERE tags IS NOT NULL AND array_length(tags, 1) > 0;

INSERT INTO schema_version (version) VALUES (4);
