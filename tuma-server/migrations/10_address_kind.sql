-- The geocoding proxy rides the same routing config; the map's search
-- needs a Places/Geocoding key with no new table. The address label
-- (Home/Work/Other) replaces a separate table column.
ALTER TABLE commerce.addresses
    ADD COLUMN IF NOT EXISTS kind TEXT NOT NULL DEFAULT 'other',
    ADD COLUMN IF NOT EXISTS note TEXT;
