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
- **Mobile:** the Flutter app in `tuma-app/` — the contract between app and backend is the HTTP API.
- **Web platform:** `tuma-platform/` — React + TypeScript + Vite + Tailwind + shadcn/ui + TanStack Query; API client generated from the server's OpenAPI (Orval). Admin + merchant wings.
- **Map:** flutter_map + real GPS from the rider's phone.
- **Architecture docs:** `Tuma_Auth_and_RBAC_Architecture.md` and `Tuma_API_Architecture.md` — the foundations, approved 2026-08-27.

## The database (6 domain tables + auth plumbing)

```text
users        (id, role, name, phone, email, password_hash, is_active)
             role: customer | merchant | admin        ← see auth architecture doc
stores       (id, merchant_id, name, description, image_url, address_text,
              lat, lng, delivery_fee, is_open)
               merchant_id: a merchant can have several stores, ownership resolved server-side
products     (id, store_id, name, description, price, image_url, is_available)
orders       (id, user_id, store_id, status, subtotal, delivery_fee, total,
              address_text, address_lat, address_lng, created_at)
order_items  (id, order_id, product_name, unit_price, quantity)   ← name+price snapshotted at order time
deliveries   (id, order_id, rider_name, rider_phone, lat, lng, updated_at)   ← the killer feature

auth plumbing:
auth_otps      (phone, code_hash, expires_at, attempts, created_at)
refresh_tokens (token_hash, user_id, expires_at, revoked_at, created_at)
```

Money is integer RWF, never floats.
Order status is one enum, six values: `placed, accepted, preparing, picked_up, delivered, cancelled`.

## The API (all business routes under `/api/v1`)

```text
auth:      POST /auth/otp/request    (phone → 6-digit code)
           POST /auth/otp/verify     (code → account + token; register & login are one flow)
           POST /auth/login          (email + password → web session cookies)
           POST /auth/logout
           POST /auth/password       (change password)
           GET  /me                  (current user + role)
           PATCH /me                  (update own profile: name)
admin:     POST  /admin/merchants    (admin creates merchant accounts)
           GET   /admin/merchants
           PATCH /admin/merchants/:id  (enable/disable)
merchant:  POST  /merchant/stores    (create a store — merchants can have several)
           GET   /merchant/stores
           GET   /merchant/stores/:id
           PATCH /merchant/stores/:id  (name, description, address, fee, is_open)
           GET   /merchant/products    (across all own stores)
           POST  /merchant/products    (store_id picks which own store)
           PATCH /merchant/products/:id
customer:  GET  /stores              (open stores only)
           GET  /stores/:id            (with its available products)
           POST /orders
           GET  /orders
           GET  /orders/:id
           GET  /orders/:id/tracking   (real lat/lng + status)
rider:     POST /deliveries/:id/location   (phone pushes real GPS every ~5s)
ops:       POST /orders/:id/status     (advance the six statuses)
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

Dispatch engine, promotions, refunds, rider earnings, rider accounts (riders are name+phone on the delivery for now), SMS gateway (dev OTP is a fixed code until then), MTN MoMo, Redis, Kafka, microservices — until the loop is real and something actually hurts.

## Build order

1. **Foundations (current iteration):** auth + RBAC on the API · mobile app splash + register/login + home on the real API · web platform (shadcn) login + admin merchant management. See the two architecture docs.
2. Stores + products: schema, `/stores`, merchant menu management on the platform, home feed in the app.
3. Orders: cart, cash checkout, order flow, status updates.
4. Rider mode in the app + real GPS tracking on the real map.
5. Polish: order history, reorder.
