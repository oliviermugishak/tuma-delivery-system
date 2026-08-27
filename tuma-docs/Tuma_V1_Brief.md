# TUMA V1 — THE BRIEF

**The product in one sentence:** a mobile app to view merchants and their products, add to cart, order, checkout, and watch the delivery move on a real map.

This is the working source of truth for V1. The big blueprint (`Tuma_Master_Product_Design_Engineering_Blueprint.md`) is long-term reference only — if the two disagree, **this brief wins** until the founder changes it.

## Rules of engagement

1. Simple beats complete. Every table and endpoint must earn its place.
2. Nothing gets built until the founder has approved the design in a few lines.
3. The server owns truth: prices, order status, delivery location. The app never invents them.
4. Delivery tracking is the killer feature — it must be **real** (real GPS, real map), never simulated.

## Stack

- **Backend: Rust** — Axum (HTTP), SQLx (PostgreSQL), serde. Chosen because the founder thinks in Rust.
- **Database:** PostgreSQL.
- **Mobile:** the existing Flutter app, living wherever the founder wants — the only contract between app and backend is the HTTP API.
- **Map:** flutter_map (already in the app) + real GPS from the rider's phone.

## The entire database (6 tables)

```text
users        (id, phone, name)
stores       (id, name, description, image_url, lat, lng, delivery_fee, is_open)
products     (id, store_id, name, description, price, image_url, is_available)
orders       (id, user_id, store_id, status, subtotal, delivery_fee, total,
              address_text, address_lat, address_lng, created_at)
order_items  (id, order_id, product_name, unit_price, quantity)   ← name+price snapshotted at order time
deliveries   (id, order_id, rider_name, rider_phone, lat, lng, updated_at)   ← the killer feature
```

Money is integer RWF, never floats.
Order status is one enum, six values: `placed, accepted, preparing, picked_up, delivered, cancelled`.

## The entire API (~10 endpoints)

```text
customer:  GET  /stores
           GET  /stores/:id            (with its products)
           POST /orders
           GET  /orders
           GET  /orders/:id
           GET  /orders/:id/tracking   (real lat/lng + status)
rider:     POST /deliveries/:id/location   (phone pushes real GPS every ~5s)
ops:       POST /orders/:id/status     (advance the six statuses)
auth:      POST /auth/login            (phone + OTP, minimal)
```

## How real tracking works (simple and real)

1. Order placed → a `deliveries` row is created for it.
2. Whoever delivers (at pilot scale: one rider with the app in "rider mode") pushes real GPS to `POST /deliveries/:id/location` every ~5 seconds.
3. The customer app polls `GET /orders/:id/tracking` every ~3 seconds → real lat/lng + status.
4. The map draws the rider marker at the real coordinates and moves it. ETA = remaining distance ÷ average speed, shown as `~X min`.

No simulation, no dispatch engine, no WebSocket yet — polling is enough for V1. Upgrade to a stream later only if polling hurts.

## Payments

Cash on delivery first (zero integration). MTN MoMo once the loop works.

## Not building in V1 (on purpose)

Merchant web app, admin console, dispatch engine, promotions, refunds, rider earnings, Redis, Kafka, microservices — until the loop is real and something actually hurts.

## Build order

1. Rust backend: schema + `/stores` + `/orders` (cash checkout).
2. Point the Flutter app at the real API instead of seeds.
3. Rider mode in the app + real GPS tracking on the real map.
4. Polish: OTP login, order history, reorder.
