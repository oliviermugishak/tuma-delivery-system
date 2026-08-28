-- Stores + products: the commerce core (S9).
-- Design: tuma-docs/Tuma_V1_Brief.md (stores/products lines), plus the
-- founder-approved additions — stores.merchant_id (one store per merchant,
-- ownership resolved server-side) and stores.address_text.
-- Money is integer RWF: BIGINT, never negative, never floats.

CREATE TABLE tuma.stores (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    merchant_id  UUID NOT NULL UNIQUE REFERENCES tuma.users(id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    description  TEXT,
    image_url    TEXT,
    address_text TEXT,
    lat          DOUBLE PRECISION,
    lng          DOUBLE PRECISION,
    delivery_fee BIGINT NOT NULL DEFAULT 0 CHECK (delivery_fee >= 0),
    is_open      BOOLEAN NOT NULL DEFAULT false,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

SELECT tuma.trigger_updated_at('tuma.stores');

CREATE TABLE tuma.products (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    store_id     UUID NOT NULL REFERENCES tuma.stores(id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    description  TEXT,
    price        BIGINT NOT NULL CHECK (price >= 0),
    image_url    TEXT,
    is_available BOOLEAN NOT NULL DEFAULT true,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

SELECT tuma.trigger_updated_at('tuma.products');

CREATE INDEX products_store_id_idx ON tuma.products (store_id);
