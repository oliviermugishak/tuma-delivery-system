-- Marketplace: merchants (businesses), stores (the fulfillment boundary),
-- the two-level catalog, and memberships (authorization).
-- Design: Tuma_V1_Brief.md (marketplace re-architecture, 2026-08-29).

CREATE TYPE marketplace.merchant_status AS ENUM ('active', 'suspended');
CREATE TYPE marketplace.membership_role AS ENUM ('owner', 'manager');

-- The merchant is a BUSINESS, never a login. People act for it through
-- merchant_memberships.
CREATE TABLE marketplace.merchants (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name           TEXT NOT NULL,
    business_email TEXT,
    business_phone TEXT,
    status         marketplace.merchant_status NOT NULL DEFAULT 'active',
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

SELECT trigger_updated_at('marketplace.merchants');

-- Stores belong to exactly one merchant; a merchant can have several.
CREATE TABLE marketplace.stores (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    merchant_id  UUID NOT NULL REFERENCES marketplace.merchants(id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    description  TEXT,
    image_url    TEXT,
    address_text TEXT,
    lat          DOUBLE PRECISION,
    lng          DOUBLE PRECISION,
    category     TEXT,
    delivery_fee BIGINT NOT NULL DEFAULT 0 CHECK (delivery_fee >= 0),
    is_open      BOOLEAN NOT NULL DEFAULT false,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

SELECT trigger_updated_at('marketplace.stores');

CREATE INDEX stores_merchant_id_idx ON marketplace.stores (merchant_id);

-- Merchant-level catalog: the product identity a merchant defines once.
CREATE TABLE marketplace.products (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    merchant_id UUID NOT NULL REFERENCES marketplace.merchants(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    description TEXT,
    image_url   TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

SELECT trigger_updated_at('marketplace.products');

CREATE INDEX products_merchant_id_idx ON marketplace.products (merchant_id);

-- What a store actually sells: per-store price, stock, and availability of
-- a catalog product. The customer buys a store_product, never an abstract
-- product. stock NULL means untracked (made-to-order food); a number means
-- the placement transaction reserves it.
CREATE TABLE marketplace.store_products (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    store_id     UUID NOT NULL REFERENCES marketplace.stores(id) ON DELETE CASCADE,
    product_id   UUID NOT NULL REFERENCES marketplace.products(id) ON DELETE CASCADE,
    price        BIGINT NOT NULL CHECK (price >= 0),
    stock        BIGINT CHECK (stock IS NULL OR stock >= 0),
    is_available BOOLEAN NOT NULL DEFAULT true,
    sku          TEXT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT store_products_store_product_key UNIQUE (store_id, product_id)
);

SELECT trigger_updated_at('marketplace.store_products');

CREATE INDEX store_products_store_id_idx ON marketplace.store_products (store_id);
CREATE INDEX store_products_product_id_idx ON marketplace.store_products (product_id);

-- Authorization: an account acts for a merchant. store_id NULL means the
-- membership covers every store of the merchant (owner, or a merchant-wide
-- manager); a set store_id scopes the member to that one store.
CREATE TABLE marketplace.merchant_memberships (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID NOT NULL REFERENCES accounts.users(id) ON DELETE CASCADE,
    merchant_id UUID NOT NULL REFERENCES marketplace.merchants(id) ON DELETE CASCADE,
    role        marketplace.membership_role NOT NULL,
    store_id    UUID REFERENCES marketplace.stores(id) ON DELETE CASCADE,
    status      marketplace.merchant_status NOT NULL DEFAULT 'active',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Only store-scoped members carry a store id; owners cover the business.
    CONSTRAINT membership_store_scope_check CHECK (store_id IS NULL OR role = 'manager')
);

CREATE INDEX merchant_memberships_user_idx ON marketplace.merchant_memberships (user_id);
CREATE INDEX merchant_memberships_merchant_idx ON marketplace.merchant_memberships (merchant_id);
