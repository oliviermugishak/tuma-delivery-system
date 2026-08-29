-- Platform plumbing: grouped schemas + the shared updated_at trigger.
-- Design: Tuma_V1_Brief.md (marketplace re-architecture, 2026-08-29).
-- Domain tables are grouped into schemas by concern:
--   accounts    — identity, profiles, OTP, refresh sessions
--   marketplace — merchants, memberships, stores, catalog
--   commerce    — order groups, store orders, deliveries, payments
-- The one shared function lives in `public` so every schema can use it.

CREATE SCHEMA IF NOT EXISTS accounts;
CREATE SCHEMA IF NOT EXISTS marketplace;
CREATE SCHEMA IF NOT EXISTS commerce;

CREATE OR REPLACE FUNCTION trigger_updated_at()
    RETURNS trigger AS
$$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$
LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION trigger_updated_at(tablename regclass)
    RETURNS void AS
$$
BEGIN
    EXECUTE format(
        '
        DROP TRIGGER IF EXISTS set_updated_at ON %s;

        CREATE TRIGGER set_updated_at
            BEFORE UPDATE
            ON %s
            FOR EACH ROW
            WHEN (OLD is distinct from NEW)
            EXECUTE FUNCTION trigger_updated_at();',
        tablename,
        tablename
    );
END;
$$
LANGUAGE plpgsql;
