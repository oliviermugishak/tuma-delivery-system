-- Multi-store merchants (founder decision, 2026-08-28): a merchant can have
-- several stores. Drops the one-store-per-merchant UNIQUE constraint from
-- migration 03 and replaces it with a plain index for per-merchant listings.

ALTER TABLE tuma.stores DROP CONSTRAINT stores_merchant_id_key;

CREATE INDEX stores_merchant_id_idx ON tuma.stores (merchant_id);
