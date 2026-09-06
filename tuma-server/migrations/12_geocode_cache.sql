-- The geocode cache (geocoding-rebuild slice): the pin names its place.
--
-- Reverse geocoding is now a SAVE-TIME enrichment: when a customer saves
-- an address (or a merchant saves a store) with a pin, the server derives
-- the display text from the pin and stores it as `address_text`. The
-- derivation is cached here, keyed by the pin rounded to 4 decimals
-- (~11 m grid) — the same place never bills twice, and cache entries
-- survive restarts. Rows are insert-only; the table grows one row per
-- distinct ~11 m cell that was ever saved, which is bounded by real usage.

CREATE SCHEMA IF NOT EXISTS geocoding;

CREATE TABLE geocoding.cache (
    pin_key    TEXT PRIMARY KEY,
    display    TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
