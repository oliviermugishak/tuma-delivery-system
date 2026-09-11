-- ORDER STATUS EVENTS — the system's memory. Every status transition
-- appends one row inside the same transaction as the change itself:
-- what happened, when, by whom. Append-only: no updated_at, no trigger,
-- no UPDATE/DELETE anywhere (the delivery_locations precedent).
-- No backfill: events start the day this lands.
--
-- Actor identity: 'customer' and 'merchant' events carry accounts.users
-- ids; 'rider' events carry commerce.riders ids (the rider profile).
CREATE TYPE commerce.actor_kind AS ENUM ('customer', 'merchant', 'rider');

CREATE TABLE commerce.order_status_events (
  id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  store_order_id UUID NOT NULL REFERENCES commerce.store_orders(id) ON DELETE CASCADE,
  status        commerce.order_status NOT NULL,
  actor_kind    commerce.actor_kind NOT NULL,
  actor_id      UUID,
  created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX order_status_events_order_idx ON commerce.order_status_events (store_order_id, created_at);
