CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE "user" (
    user_id UUID DEFAULT uuid_generate_v4(),
    username TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT null DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id)
);
CREATE UNIQUE INDEX user_username_unique ON "user" (LOWER(username));

CREATE TABLE bookmark (
    bookmark_id VARCHAR(512) UNIQUE NOT NULL,
    user_id UUID NOT NULL,
    url TEXT NOT NULL,
    domain text NOT NULL,
    title TEXT NOT NULL,
    text_content TEXT NOT NULL,
    tags TEXT[],
    summary TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (bookmark_id, user_id),
    CONSTRAINT fk_user FOREIGN KEY(user_id) REFERENCES "user"(user_id) ON DELETE CASCADE,
    CONSTRAINT bookmark_url_user_unique UNIQUE (url, user_id)
);
ALTER TABLE bookmark ADD COLUMN search_tokens TSVECTOR GENERATED ALWAYS AS (
    setweight(to_tsvector('english', coalesce(title, '')), 'A') ||
    setweight(to_tsvector('english', coalesce(text_content, '')), 'B') ||
    setweight(to_tsvector('english', coalesce(summary, '')), 'C')
) STORED;
CREATE INDEX bookmark_search_index ON bookmark USING GIN (search_tokens);

CREATE TYPE task_status AS ENUM ('done', 'pending', 'fail');
CREATE TABLE bookmark_task (
    task_id UUID DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL,
    url TEXT NOT NULL,
    status task_status NOT NULL DEFAULT 'pending',
    tags TEXT[],
    summary TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    next_delivery TIMESTAMPTZ NOT NULL DEFAULT now(),
    retries SMALLINT,
    fail_reason TEXT,
    PRIMARY KEY (task_id),
    CONSTRAINT fk_user FOREIGN KEY(user_id) REFERENCES "user"(user_id) ON DELETE CASCADE
);

CREATE TABLE schema_version (
  version INTEGER NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (version)
);
INSERT INTO schema_version (version, updated_at) VALUES ('1', NOW());
