-- The store's contact surface: the customer's "Get help" and store-info
-- sheets call and email the store directly. contact_phone landed with the
-- tracking slice (07); email joins it.
ALTER TABLE marketplace.stores ADD COLUMN IF NOT EXISTS contact_email TEXT;
