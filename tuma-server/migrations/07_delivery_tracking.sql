-- Slice: delivery tracking D1 — schema + Tuma-owned riders.
-- Design: tuma-docs/Tuma_Delivery_Tracking_Architecture.md (§7 data model).

-- Tuma-owned riders: platform entities with an OTP account and a unique,
-- memorizable rider number. Merchants assign deliveries by this number.
CREATE TABLE commerce.riders (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    rider_number BIGINT NOT NULL GENERATED ALWAYS AS IDENTITY,
    account_id   UUID NOT NULL UNIQUE REFERENCES accounts.users(id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    phone        TEXT NOT NULL,
    is_active    BOOLEAN NOT NULL DEFAULT true,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT riders_rider_number_key UNIQUE (rider_number)
);

SELECT trigger_updated_at('commerce.riders');

-- Insert-only GPS breadcrumbs (the ≥25m/15s insert rule is enforced by the
-- D2 push endpoint, not the database). No updated_at, no trigger — rows
-- are never updated, old ones are pruned.
CREATE TABLE commerce.delivery_locations (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    delivery_id UUID NOT NULL REFERENCES commerce.deliveries(id) ON DELETE CASCADE,
    lat         DOUBLE PRECISION NOT NULL,
    lng         DOUBLE PRECISION NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX delivery_locations_delivery_idx
    ON commerce.delivery_locations (delivery_id, recorded_at);

-- Tracking columns: assignment at handoff, cached route, server-owned ETA
-- target, the rider's last real position. No writer until D2.
ALTER TABLE commerce.deliveries
    ADD COLUMN rider_id          UUID REFERENCES commerce.riders(id),
    ADD COLUMN handoff_at        TIMESTAMPTZ,
    ADD COLUMN route_polyline    TEXT,
    ADD COLUMN eta_target        TIMESTAMPTZ,
    ADD COLUMN last_lat          DOUBLE PRECISION,
    ADD COLUMN last_lng          DOUBLE PRECISION,
    ADD COLUMN last_location_at TIMESTAMPTZ;

CREATE INDEX deliveries_rider_idx ON commerce.deliveries (rider_id);

-- Migration 03's placeholder rider/coordinator columns are replaced by the
-- tracking picture above: riders are referenced by id, positions live in
-- delivery_locations and the last-* columns. Nothing ever wrote them.
ALTER TABLE commerce.deliveries
    DROP COLUMN rider_name,
    DROP COLUMN rider_phone,
    DROP COLUMN lat,
    DROP COLUMN lng;

-- The customer's overdue "call the store" action (doc §6). Read by the
-- tracking surfaces in D3/D5.
ALTER TABLE marketplace.stores ADD COLUMN contact_phone TEXT;
