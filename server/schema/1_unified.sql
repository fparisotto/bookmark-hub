-- Unified Schema Migration
-- Creates all tables and indexes for the bookmark hub application including RAG support

CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS vector;

-- Create task status enum
CREATE TYPE task_status AS ENUM (
    'pending',
    'done',
    'fail'
);

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
-- Add search index (done via trigger instead of generated column for compatibility)
CREATE OR REPLACE FUNCTION update_bookmark_search_tokens()
RETURNS TRIGGER AS $$
BEGIN
    NEW.search_tokens := setweight(to_tsvector('english', coalesce(NEW.title, '')), 'A') ||
                        setweight(to_tsvector('english', coalesce(NEW.text_content, '')), 'B') ||
                        setweight(to_tsvector('english', coalesce(array_to_string(NEW.tags, ' '), '')), 'C');
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

ALTER TABLE bookmark ADD COLUMN search_tokens TSVECTOR;
CREATE TRIGGER update_bookmark_search_tokens_trigger
    BEFORE INSERT OR UPDATE ON bookmark
    FOR EACH ROW EXECUTE FUNCTION update_bookmark_search_tokens();

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

-- RAG Support Tables
CREATE TABLE bookmark_chunk (
    chunk_id UUID DEFAULT uuid_generate_v4(),
    bookmark_id VARCHAR(512) NOT NULL,
    user_id UUID NOT NULL,
    chunk_text TEXT NOT NULL,
    chunk_index INTEGER NOT NULL,
    embedding VECTOR(768), -- nomic-embed-text:v1.5 produces 768-dimensional embeddings
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (chunk_id),
    CONSTRAINT fk_bookmark_chunk FOREIGN KEY (bookmark_id, user_id) 
        REFERENCES bookmark(bookmark_id, user_id) ON DELETE CASCADE,
    CONSTRAINT bookmark_chunk_unique UNIQUE (bookmark_id, user_id, chunk_index)
);

CREATE TABLE rag_session (
    session_id UUID DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL,
    question TEXT NOT NULL,
    answer TEXT,
    relevant_chunks UUID[] NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (session_id),
    CONSTRAINT fk_rag_session_user FOREIGN KEY (user_id) 
        REFERENCES "user"(user_id) ON DELETE CASCADE
);

-- Indexes
CREATE INDEX idx_bookmark_user_created_at ON bookmark (user_id, created_at DESC);
CREATE INDEX idx_bookmark_user_updated_at ON bookmark (user_id, updated_at DESC);
CREATE INDEX idx_bookmark_search_tokens ON bookmark USING GIN (search_tokens);
CREATE INDEX idx_bookmark_tags ON bookmark USING GIN (tags);
CREATE INDEX idx_bookmark_user_domain ON bookmark (user_id, domain);
CREATE INDEX idx_task_user_id ON bookmark_task (user_id);
CREATE INDEX idx_task_status ON bookmark_task (status);
CREATE INDEX idx_task_next_delivery ON bookmark_task (next_delivery) WHERE status = 'pending';
CREATE INDEX idx_bookmark_chunk_embedding ON bookmark_chunk 
    USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);
CREATE INDEX idx_bookmark_chunk_bookmark ON bookmark_chunk (bookmark_id, user_id);
CREATE INDEX idx_rag_session_user ON rag_session (user_id, created_at DESC);

CREATE TABLE schema_version (
    version INTEGER PRIMARY KEY,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
INSERT INTO schema_version (version) VALUES (1);