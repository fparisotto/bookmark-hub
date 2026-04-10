ALTER TABLE bookmark
ADD COLUMN IF NOT EXISTS canonical_url TEXT;

ALTER TABLE bookmark
DROP CONSTRAINT IF EXISTS bookmark_url_user_unique;

ALTER TABLE bookmark
DROP CONSTRAINT IF EXISTS bookmark_bookmark_id_key;

INSERT INTO schema_version (version) VALUES (7);
