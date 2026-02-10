-- Normalize all tags to lowercase and deduplicate
UPDATE bookmark
SET tags = (SELECT ARRAY(SELECT DISTINCT LOWER(tag) FROM unnest(tags) AS tag)),
    updated_at = now()
WHERE tags IS NOT NULL AND array_length(tags, 1) > 0;

UPDATE bookmark_task
SET tags = (SELECT ARRAY(SELECT DISTINCT LOWER(tag) FROM unnest(tags) AS tag))
WHERE tags IS NOT NULL AND array_length(tags, 1) > 0;

INSERT INTO schema_version (version) VALUES (3);
