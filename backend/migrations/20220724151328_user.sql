CREATE TABLE "user" (
    user_id UUID DEFAULT uuid_generate_v4(),
    email TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT null DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id)
);
CREATE UNIQUE INDEX user_email_unique ON "user" (LOWER(email));
