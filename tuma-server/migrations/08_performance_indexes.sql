-- Performance pass (deep review, 2026-08-31): the discovery query behind
-- "Popular near you" aggregates order_items by store_product_id on every
-- load — this index turns that full-table GROUP BY into an index scan.
-- The merchant index on store_orders is dead: every board query resolves
-- membership through the joined stores table, so the column never
-- appears in a WHERE clause.

CREATE INDEX IF NOT EXISTS order_items_product_idx
    ON commerce.order_items (store_product_id);

DROP INDEX IF EXISTS commerce.store_orders_merchant_idx;
