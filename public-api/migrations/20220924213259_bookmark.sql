CREATE TABLE bookmark (
    bookmark_id VARCHAR(512) UNIQUE NOT NULL,
    url TEXT NOT NULL UNIQUE,
    domain text NOT NULL,
    title TEXT NOT NULL,
    text_content TEXT NOT NULL,
    html_content TEXT NOT NULL,
    images TEXT[],
    links TEXT[],
    created_at TIMESTAMPTZ NOT NULL default now(),
    PRIMARY KEY (bookmark_id)
);

ALTER TABLE bookmark ADD COLUMN search_tokens TSVECTOR GENERATED ALWAYS AS (
    setweight(to_tsvector('english', coalesce(title, '')), 'A') ||
    setweight(to_tsvector('english', coalesce(text_content, '')), 'B')
) STORED;

CREATE INDEX bookmark_search_index ON bookmark USING GIN (search_tokens);

CREATE TABLE bookmark_user (
    bookmark_user_id UUID DEFAULT uuid_generate_v4(),
    bookmark_id VARCHAR(512),
    user_id UUID,
    tags TEXT[],
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (bookmark_user_id),
    CONSTRAINT fk_bookmark FOREIGN KEY(bookmark_id) REFERENCES bookmark(bookmark_id) ON DELETE CASCADE,
    CONSTRAINT fk_user FOREIGN KEY(user_id) REFERENCES "user"(user_id) ON DELETE CASCADE,
    CONSTRAINT bookmark_user_unique UNIQUE (bookmark_id, user_id)
);

CREATE TYPE task_status AS ENUM ('done', 'pending', 'fail');

CREATE TABLE bookmark_task (
    task_id UUID DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL,
    url TEXT NOT NULL,
    status task_status NOT NULL DEFAULT 'pending',
    tags TEXT[],
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    next_delivery TIMESTAMPTZ NOT NULL DEFAULT now(),
    retries SMALLINT DEFAULT 0,
    fail_reason TEXT,
    PRIMARY KEY (task_id),
    CONSTRAINT fk_user FOREIGN KEY(user_id) REFERENCES "user"(user_id) ON DELETE CASCADE
);
