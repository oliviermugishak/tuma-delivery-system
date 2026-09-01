-- Grand redesign (v2) — the data the new screens honestly need (P2: the
-- UI may only render what the server knows).
--
-- 1. riders.vehicle_type / riders.plate_number — the customer's tracking
--    screen shows "Amani N. · Moto · MUP 1234". Nullable: the rider
--    admin form fills them when known, and P2 says the UI hides what's
--    absent.
-- 2. order_groups.customer_note — the checkout's "Note for rider ·
--    optional" field; the rider's Delivering card renders it ("Note:
--    blue gate, ring the bell"). One note per purchase, so it lives on
--    the group and every store-order's rider sees it.
-- 3. addresses — the saved-address book behind checkout's
--    saved-address-first design and the profile's plural "Delivery
--    locations". One address is `is_default`; checkout snapshots
--    address_text + lat/lng onto the group as before (the address row
--    is a source, never a live reference).

ALTER TABLE commerce.riders
    ADD COLUMN IF NOT EXISTS vehicle_type TEXT,
    ADD COLUMN IF NOT EXISTS plate_number TEXT;

ALTER TABLE commerce.order_groups
    ADD COLUMN IF NOT EXISTS customer_note TEXT;

CREATE TABLE IF NOT EXISTS commerce.addresses (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES accounts.users(id) ON DELETE CASCADE,
    label TEXT NOT NULL,
    address_text TEXT NOT NULL,
    lat DOUBLE PRECISION,
    lng DOUBLE PRECISION,
    is_default BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS addresses_user_idx
    ON commerce.addresses (user_id);

-- One default per user: a partial unique index enforces it while
-- letting every row be non-default.
CREATE UNIQUE INDEX IF NOT EXISTS addresses_one_default_idx
    ON commerce.addresses (user_id) WHERE is_default;

COMMENT ON TABLE commerce.addresses IS
    'The customer''s saved delivery addresses; checkout snapshots the chosen one onto the order group.';
