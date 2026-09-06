-- Popularity counters (Wave 2): "Popular near you" used to GROUP BY over
-- ALL of commerce.order_items (joined to store_orders to exclude
-- cancelled) on every empty-q discovery load. This table freezes that
-- aggregate and keeps it live: +1 per checkout LINE at placement, -1 per
-- line on cancel — so the counter keeps the old query's exact semantics
-- (checkout LINES, cancelled orders excluded), read by a straight indexed
-- scan. A line is a row of order_items, not a quantity: 2×Amstel on one
-- checkout line counts once.
--
-- Migration 08's order_items_product_idx was built for the old GROUP BY
-- and becomes vestigial here (the checkout decrement is a PK point-read);
-- it stays — a harmless index the next aggregate might want.
--
-- The backfill reproduces the old query one last time, so the shelf never
-- starts from zero.

CREATE TABLE commerce.product_popularity (
    store_product_id UUID PRIMARY KEY REFERENCES marketplace.store_products(id) ON DELETE CASCADE,
    order_lines      BIGINT NOT NULL DEFAULT 0
);

INSERT INTO commerce.product_popularity (store_product_id, order_lines)
SELECT store_product_id, COUNT(*)
FROM commerce.order_items oi
JOIN commerce.store_orders so ON so.id = oi.store_order_id
WHERE so.status <> 'cancelled'
GROUP BY store_product_id
ON CONFLICT (store_product_id) DO NOTHING;
