-- Identity: the central ACCOUNT, its authorization profiles, OTP codes,
-- and web refresh sessions.
-- Design: Tuma_V1_Brief.md (marketplace re-architecture, 2026-08-29).
-- An account authenticates; it is never a business role. Customer and
-- admin profiles are separate rows; the merchant business and its
-- memberships live in 02_marketplace.sql.

CREATE TABLE accounts.users (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    phone         TEXT UNIQUE,
    email         TEXT UNIQUE,
    password_hash TEXT,
    is_active     BOOLEAN NOT NULL DEFAULT true,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Every account carries at least one credential anchor.
    CONSTRAINT credential_check CHECK (phone IS NOT NULL OR email IS NOT NULL)
);

SELECT trigger_updated_at('accounts.users');

-- Customer profile: one per account, created with the account by the OTP
-- flow (register and login are one). The display name lives here — the
-- account itself has no business role and no profile data.
CREATE TABLE accounts.customers (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id    UUID NOT NULL UNIQUE REFERENCES accounts.users(id) ON DELETE CASCADE,
    name       TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

SELECT trigger_updated_at('accounts.customers');

-- Platform-admin profile: one per account, created by seed_admin.
CREATE TABLE accounts.admins (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id    UUID NOT NULL UNIQUE REFERENCES accounts.users(id) ON DELETE CASCADE,
    name       TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

SELECT trigger_updated_at('accounts.admins');

-- One live code per phone: a new request replaces the previous code.
CREATE TABLE accounts.auth_otps (
    phone      TEXT PRIMARY KEY,
    code_hash  TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    attempts   INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Opaque refresh tokens for web sessions; only their hash is stored.
CREATE TABLE accounts.refresh_tokens (
    token_hash TEXT PRIMARY KEY,
    user_id    UUID NOT NULL REFERENCES accounts.users(id) ON DELETE CASCADE,
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX refresh_tokens_user_idx ON accounts.refresh_tokens (user_id);
