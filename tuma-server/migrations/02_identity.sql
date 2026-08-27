-- Identity: users + roles, OTP codes, and web refresh sessions.
-- Design: tuma-docs/Tuma_Auth_and_RBAC_Architecture.md (§10).

CREATE TYPE tuma.user_role AS ENUM ('customer', 'merchant', 'admin');

CREATE TABLE tuma.users (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    role          tuma.user_role NOT NULL,
    name          TEXT,
    phone         TEXT UNIQUE,
    email         TEXT UNIQUE,
    password_hash TEXT,
    is_active     BOOLEAN NOT NULL DEFAULT true,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT identity_check CHECK (
        (role = 'customer' AND phone IS NOT NULL)
        OR (role IN ('merchant', 'admin') AND email IS NOT NULL AND password_hash IS NOT NULL)
    )
);

SELECT tuma.trigger_updated_at('tuma.users');

-- One live code per phone: a new request replaces the previous code.
CREATE TABLE tuma.auth_otps (
    phone      TEXT PRIMARY KEY,
    code_hash  TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    attempts   INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Opaque refresh tokens for web sessions; only their hash is stored.
CREATE TABLE tuma.refresh_tokens (
    token_hash TEXT PRIMARY KEY,
    user_id    UUID NOT NULL REFERENCES tuma.users(id) ON DELETE CASCADE,
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX refresh_tokens_user_idx ON tuma.refresh_tokens (user_id);
