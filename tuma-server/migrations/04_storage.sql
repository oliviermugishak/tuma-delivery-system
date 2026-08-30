-- Slice U1: image storage (founder-approved via STORAGE_GUIDANCE.md).
-- Additive: one nullable column on stores for the banner's storage key,
-- one new table for product-image galleries. Bytes live in object
-- storage; these rows hold metadata only — storage keys, never URLs
-- (URLs are composed at response time from the configured public base).

ALTER TABLE marketplace.stores ADD COLUMN banner_key TEXT;

CREATE TABLE marketplace.product_images (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    product_id UUID NOT NULL REFERENCES marketplace.products ON DELETE CASCADE,
    -- Object-storage key (`merchants/{merchant_id}/products/{product_id}/
    -- images/{uuid}.jpg`). Keys are content-UUIDs, never overwritten —
    -- reads cache forever. The extension is the content type.
    storage_key TEXT NOT NULL,
    -- Gallery order; the lowest position is the cover. No unique constraint
    -- on purpose: the cover swap moves two rows in one transaction.
    position INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX product_images_product_id_idx ON marketplace.product_images (product_id);

SELECT trigger_updated_at('marketplace.product_images');
