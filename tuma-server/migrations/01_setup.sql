CREATE OR REPLACE FUNCTION tuma.trigger_updated_at()
    RETURNS trigger AS
$$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$
LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION tuma.trigger_updated_at(tablename regclass)
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
            EXECUTE FUNCTION tuma.trigger_updated_at();',
        tablename,
        tablename
    );
END;
$$
LANGUAGE plpgsql;
