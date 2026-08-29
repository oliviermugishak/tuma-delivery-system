-- Commerce: order groups (one checkout), store orders (one per store),
-- item snapshots, deliveries, payments, and payment allocations.
-- Design: Tuma_V1_Brief.md (marketplace re-architecture, 2026-08-29).

CREATE TYPE commerce.order_status AS ENUM (
    'placed',
    'accepted',
    'preparing',
    'picked_up',
    'delivered',
    'cancelled'
);

CREATE TYPE commerce.payment_provider AS ENUM ('cash_on_delivery');
CREATE TYPE commerce.payment_status AS ENUM ('pending', 'collected', 'refunded');
CREATE TYPE commerce.allocation_status AS ENUM ('pending', 'settled', 'refunded');

-- The customer-facing purchase: one checkout = one group. No status column —
-- the group's state is derived from its store orders, never a second
-- state machine. Money is integer RWF, computed server-side.
CREATE TABLE commerce.order_groups (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES accounts.users(id) ON DELETE CASCADE,
    number          BIGINT NOT NULL GENERATED ALWAYS AS IDENTITY,
    address_text    TEXT NOT NULL,
    address_lat     DOUBLE PRECISION,
    address_lng     DOUBLE PRECISION,
    subtotal        BIGINT NOT NULL CHECK (subtotal >= 0),
    delivery_total  BIGINT NOT NULL CHECK (delivery_total >= 0),
    grand_total     BIGINT NOT NULL CHECK (grand_total >= 0),
    -- A retried checkout returns the group it already created.
    idempotency_key TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT order_groups_number_key UNIQUE (number),
    CONSTRAINT order_groups_idempotency_key_key UNIQUE (user_id, idempotency_key)
);

SELECT trigger_updated_at('commerce.order_groups');

CREATE INDEX order_groups_user_id_idx ON commerce.order_groups (user_id);

-- The operational order: what a store fulfills, independently. merchant_id
-- is stored redundantly (the checkout derives it from the store) so merchant
-- scoping and reporting never join to learn who owns an order.
CREATE TABLE commerce.store_orders (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    order_group_id UUID NOT NULL REFERENCES commerce.order_groups(id) ON DELETE CASCADE,
    merchant_id    UUID NOT NULL REFERENCES marketplace.merchants(id),
    store_id       UUID NOT NULL REFERENCES marketplace.stores(id),
    number         BIGINT NOT NULL GENERATED ALWAYS AS IDENTITY,
    status         commerce.order_status NOT NULL DEFAULT 'placed',
    subtotal       BIGINT NOT NULL CHECK (subtotal >= 0),
    delivery_fee   BIGINT NOT NULL CHECK (delivery_fee >= 0),
    total          BIGINT NOT NULL CHECK (total >= 0),
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT store_orders_number_key UNIQUE (number)
);

SELECT trigger_updated_at('commerce.store_orders');

CREATE INDEX store_orders_group_idx ON commerce.store_orders (order_group_id);
CREATE INDEX store_orders_store_idx ON commerce.store_orders (store_id);
CREATE INDEX store_orders_merchant_idx ON commerce.store_orders (merchant_id);

-- Name and price are snapshotted at order time: later catalog edits never
-- rewrite history. store_product_id + product_id keep reorder possible even
-- after an item leaves a store's assortment.
CREATE TABLE commerce.order_items (
    id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    store_order_id        UUID NOT NULL REFERENCES commerce.store_orders(id) ON DELETE CASCADE,
    store_product_id      UUID NOT NULL REFERENCES marketplace.store_products(id),
    product_id            UUID NOT NULL REFERENCES marketplace.products(id),
    product_name_snapshot TEXT NOT NULL,
    unit_price            BIGINT NOT NULL CHECK (unit_price >= 0),
    quantity              INT NOT NULL CHECK (quantity > 0)
);

CREATE INDEX order_items_order_idx ON commerce.order_items (store_order_id);

-- Created empty at order time (one per store order — pickup originates at
-- the store). Rider fields and coordinates stay null until build order #4
-- assigns a real rider.
CREATE TABLE commerce.deliveries (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    store_order_id UUID NOT NULL UNIQUE REFERENCES commerce.store_orders(id) ON DELETE CASCADE,
    rider_name     TEXT,
    rider_phone    TEXT,
    lat            DOUBLE PRECISION,
    lng            DOUBLE PRECISION,
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

SELECT trigger_updated_at('commerce.deliveries');

-- One payment per checkout. Cash first; the provider reference waits for MoMo.
CREATE TABLE commerce.payments (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    order_group_id     UUID NOT NULL UNIQUE REFERENCES commerce.order_groups(id) ON DELETE CASCADE,
    provider           commerce.payment_provider NOT NULL DEFAULT 'cash_on_delivery',
    amount             BIGINT NOT NULL CHECK (amount >= 0),
    currency           TEXT NOT NULL DEFAULT 'RWF',
    status             commerce.payment_status NOT NULL DEFAULT 'pending',
    provider_reference TEXT,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);

SELECT trigger_updated_at('commerce.payments');

-- The ledger: how the customer's one payment belongs to each store order.
-- Merchant money is explicitly allocated here, never inferred from the
-- group total later.
CREATE TABLE commerce.payment_allocations (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    payment_id     UUID NOT NULL REFERENCES commerce.payments(id) ON DELETE CASCADE,
    store_order_id UUID NOT NULL UNIQUE REFERENCES commerce.store_orders(id) ON DELETE CASCADE,
    merchant_id    UUID NOT NULL REFERENCES marketplace.merchants(id),
    store_id       UUID NOT NULL REFERENCES marketplace.stores(id),
    amount         BIGINT NOT NULL CHECK (amount >= 0),
    status         commerce.allocation_status NOT NULL DEFAULT 'pending',
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

SELECT trigger_updated_at('commerce.payment_allocations');

CREATE INDEX payment_allocations_payment_idx ON commerce.payment_allocations (payment_id);
CREATE INDEX payment_allocations_merchant_idx ON commerce.payment_allocations (merchant_id);
