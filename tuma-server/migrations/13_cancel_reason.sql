-- Why an order died is a fact worth keeping: the merchant's reject and
-- the customer's change-of-mind both carry an optional reason now —
-- stored at the cancel moment, rendered on the fulfillment sheet, the
-- receipt drawer, and the customer's order detail.

ALTER TABLE commerce.store_orders ADD COLUMN IF NOT EXISTS cancel_reason TEXT;
