-- Slice: discovery — cross-store product search.
-- The catalog product carries the name (store_products hold only
-- price/stock/availability), so the trigram index goes on
-- marketplace.products(name) — the same pg_trgm + GIN discipline as the
-- store search: the plain ILIKE stays fast as the catalog grows.
CREATE EXTENSION IF NOT EXISTS pg_trgm;

CREATE INDEX products_name_trgm_idx ON marketplace.products USING gin (name gin_trgm_ops);
