-- Slice: server-side store search.
-- pg_trgm + one GIN index turns the plain ILIKE search into an
-- index-accelerated scan, so the same query that serves 7 stores keeps
-- serving 50,000 without a code change. Category stays unindexed —
-- short, low-cardinality, seq-scan cheap at any pilot scale.
CREATE EXTENSION IF NOT EXISTS pg_trgm;

CREATE INDEX stores_name_trgm_idx ON marketplace.stores USING gin (name gin_trgm_ops);
