# TUMA V1 — THE BRIEF

**The product in one sentence:** a mobile app to view merchants and their products, add to cart, order, checkout, and watch the delivery move on a real map.

This is the working source of truth for V1. The big blueprint (`Tuma_Master_Product_Design_Engineering_Blueprint.md`) is long-term reference only — if the two disagree, **this brief wins** until the founder changes it.

> **2026-08-29 — Marketplace re-architecture (founder directive).** The domain was re-engineered around a real multi-merchant, multi-store marketplace: identity split from business entities, merchants are businesses (not logins), stores are the fulfillment boundary, products sell through per-store `store_products`, and one checkout becomes one `order_group` containing one `store_order` per participating store, funded by one payment with explicit allocations. This supersedes the earlier 6-table budget and the one-order-per-checkout model. The re-architecture plan in the session record is the design authority for the migration.

## Rules of engagement

1. Simple beats complete. Every table and endpoint must earn its place.
2. Nothing gets built until the founder has approved the design in a few lines.
3. The server owns truth: prices, stock, order status, delivery location. The apps never invent them.
4. Delivery tracking is the killer feature — it must be **real** (real GPS, real map), never simulated.

## Stack

- **Backend: Rust** — Axum (HTTP), SQLx (PostgreSQL), serde. Chosen because the founder thinks in Rust.
- **Database:** PostgreSQL.
- **Mobile:** the Flutter app in `tuma-app/` — the contract between app and backend is the HTTP API. **Android is the only V1 target** (2026-09-05): the iOS Maps key is not wired; making iOS real later is a small key-wiring slice, nothing structural.
- **Web platform:** `tuma-platform/` — React + TypeScript + Vite + Tailwind + shadcn/ui + TanStack Query; API client generated from the server's OpenAPI (hey-api). Admin + merchant wings.
- **Map:** Google Maps — `google_maps_flutter` on mobile (JS SDK on the platform in D4); road routes + ETA from Google Directions called by the server (tracking doc §4 supersedes the earlier flutter_map choice).
- **Architecture docs:** `Tuma_Auth_and_RBAC_Architecture.md` and `Tuma_API_Architecture.md` — the foundations (auth sections predate the 2026-08-29 identity split; the split below wins where they disagree).

## The database (marketplace domain + auth plumbing)

```text
IDENTITY — authentication only; a business role is never stored here
users          (id, phone?, email?, password_hash?, is_active)     ← the central ACCOUNT
auth_otps      (phone, code_hash, expires_at, attempts, created_at)
refresh_tokens (token_hash, user_id, expires_at, revoked_at, created_at)

BUSINESS ENTITIES — authorization and domain, separate from identity
customers      (id, user_id UNIQUE, name)                          ← customer profile
admins         (id, user_id UNIQUE, name)                          ← platform-admin profile
merchants      (id, name, business_email?, business_phone?, status) ← the BUSINESS; no login
merchant_memberships (id, user_id, merchant_id, role owner|manager, store_id?, status)
                   ← store_id NULL = all stores of the merchant (owner);
                     set = scoped to that one store

CATALOG — the store is the fulfillment boundary
stores         (id, merchant_id → merchants, name, description, image_url, address_text,
                lat, lng, category, delivery_fee, is_open, contact_phone?)
products       (id, merchant_id → merchants, name, description, image_url)  ← merchant catalog
store_products (id, store_id, product_id, price, stock? (NULL = untracked, ≥ 0),
                is_available, sku?)    UNIQUE(store_id, product_id)
                   ← the customer buys a store_product: per-store price, stock, availability

COMMERCE — one checkout, one store order per store
order_groups   (id, user_id, number, address_text, address_lat?, address_lng?,
                subtotal, delivery_total, grand_total, idempotency_key?)
                   UNIQUE(user_id, idempotency_key)   ← NO status column; derived from children
store_orders   (id, order_group_id, merchant_id, store_id, number,
                status, subtotal, delivery_fee, total)
order_items    (id, store_order_id, store_product_id, product_id,
                product_name_snapshot, unit_price, quantity)   ← snapshots at order time
deliveries     (id, store_order_id UNIQUE, rider_id → riders, handoff_at?,
                route_polyline?, eta_target?, last_lat?, last_lng?,
                last_location_at?)   ← tracking lives here (build order #4)
riders         (id, rider_number UNIQUE, account_id UNIQUE → users, name,
                phone, is_active)    ← Tuma-owned drivers; OTP accounts;
                                         merchants assign by rider number
delivery_locations (id, delivery_id → deliveries, lat, lng, recorded_at)
                   ← insert-only GPS breadcrumbs (≥25m/15s), never updated

PAYMENTS — one customer payment, explicit allocations
payments            (id, order_group_id UNIQUE, provider cash_on_delivery, amount, currency,
                     status pending|collected|refunded, provider_reference?)
payment_allocations (id, payment_id, store_order_id, merchant_id, store_id, amount,
                     status pending|settled|refunded)
```

Money is integer RWF, never floats. Order status is one enum, six values, living at **store-order** level: `placed, accepted, preparing, picked_up, delivered, cancelled`. The order group's overall state is **derived** from its store orders (`in_progress | partially_fulfilled | completed | cancelled`) — never a second state machine.

## The API (all business routes under `/api/v1`)

```text
auth:      POST /auth/otp/request    (phone → 6-digit code; creates account + customer)
           POST /auth/otp/verify     (code → account + token; register & login are one flow)
           POST /auth/login          (email + password → web session cookies)
           POST /auth/logout
           POST /auth/password       (change password)
           GET  /me                  (account + customer/admin profiles + merchant memberships)
           PATCH /me                 (update own profile name)

admin:     POST  /admin/merchants    (create business + owner account + owner membership)
           GET   /admin/merchants    / GET /admin/merchants/:id
           PATCH /admin/merchants/:id  / DELETE /admin/merchants/:id
           GET   /admin/customers    / PATCH /admin/customers/:id  / DELETE /admin/customers/:id
           POST  /admin/riders       (create rider: OTP account + unique rider number)
           GET   /admin/riders       / PATCH /admin/riders/:id  / DELETE /admin/riders/:id
           GET   /admin/summary      (platform counts; orders_in_progress is read-only visibility)

merchant:  POST /merchant/stores     / GET /merchant/stores  / GET|PATCH|DELETE /merchant/stores/:id
           POST /merchant/products   / GET /merchant/products  (merchant CATALOG)
           PATCH|DELETE /merchant/products/:id
           POST /merchant/store-products   (attach catalog product to a store: price, stock, availability)
           GET  /merchant/store-products       (across own stores)
           PATCH|DELETE /merchant/store-products/:id
           GET  /merchant/orders         (store orders across authorized stores)
           GET  /merchant/store-orders/:id     (fulfillment sheet: items, address, customer contact)
           PATCH /merchant/store-orders/:id (advance status)
           POST|DELETE /merchant/stores/:id/banner    (upload/replace | clear the store banner)
           GET|POST /merchant/products/:id/images     (gallery, cover first | append an image
                                                       — 409 when the 8-image cap is full)
           DELETE /merchant/products/:pid/images/:iid (remove one image + its object)
           POST /merchant/products/:pid/images/:iid/cover (make it the cover)

customer:  GET  /stores              (open stores only; ?lat&lng → server-computed
                                       distance_m/eta_min, nearest first — nulls without;
                                       ?q → server-side search on name/category,
                                       composes with lat&lng; absent q = unchanged feed)
           GET  /search              (discovery: ?q → matched products (catalog name) +
                                       matched stores; absent q → popular products by
                                       real order counts + all open stores; ?lat&lng
                                       attach distance/eta to both sections)
           GET  /stores/:id          (store + its available store_products; each item
                                       carries its full gallery `images[]`, cover first —
                                       max 8 per product, server-enforced)
           POST /orders               (CHECKOUT: items from many stores → one order group)
           GET  /orders               (order groups, newest first)
           GET  /orders/:id           (group detail: store orders, items, payment)
           POST /orders/:id/store-orders/:sid/cancel   (customer cancels one store order)
           GET  /orders/:id/tracking   (real lat/lng + status — build order #4)

rider:     POST /deliveries/:id/location   (rider's phone pushes real GPS — ≥25m/15s breadcrumbs)
           POST /deliveries/:id/delivered  (rider confirms at the door → allocation settles; the
                                            group's payment collects when every allocation has)
           GET  /deliveries                (the rider's active runs — D2)
           POST /merchant/store-orders/:id/handoff  (merchant enters the rider number →
                                            rider_id + picked_up + route/ETA cached — D2)
           GET  /orders/:id/tracking       (customer: snapshot per delivery + 204-when-unchanged — D2)
           Tuma-owned riders (OTP accounts + unique rider numbers); merchants
           assign at handoff by entering the rider number. Full design:
           tuma-docs/Tuma_Delivery_Tracking_Architecture.md

public:    GET  /files/{key}         (stored objects — dev/LAN read path; prod reads
                                       go straight to the R2/CDN URL, immutable cache)
```

**Store orders are the merchants' monopoly.** The admin namespace has no
order endpoints — no advancing, no collecting. Order logic lives with the
business that fulfills it; the admin's only visibility is the passive
`orders_in_progress` count in the summary. Payment allocations are created
automatically at checkout as passive records (the per-merchant separation
of the one customer payment); there is no collection action anywhere until
a real money system exists, at which point collection becomes automatic
(e.g. a MoMo webhook), never a manual button.
```

## How checkout works (the core redesign)

1. The cart is a client-side session grouped by store; the customer adds `store_products` from many stores.
2. One checkout → `POST /orders` with `store_product_id + quantity` lines, one address, an idempotency key. The client never sends totals.
3. The server validates everything (stores open, store_products available, stock reserved atomically), groups lines by store, computes every number, and creates **in one transaction**: the order group, one store order per store, item snapshots, one delivery row per store order, the cash payment, and its per-store allocations.
4. All-or-nothing: any closed store, unavailable product, or insufficient stock → 409 naming the offender; nothing is created. A retry with the same idempotency key returns the existing group.
5. Each store fulfills only its own store order. The customer sees one purchase with per-store sections.

## How real tracking works (simple and real — unchanged)

1. Store order placed → a `deliveries` row exists for it.
2. Whoever delivers (pilot scale: one rider with the app in "rider mode") pushes real GPS every ~5 seconds.
3. The customer app polls the group tracking every ~5 seconds with a `since` echo — a 204 when nothing changed means zero work (battery is the contract).
4. The map draws the cached road route (Google Directions, called by the server at handoff) and the rider marker at the real coordinates; the ETA is the server's `eta_target`, which the client decays.

No simulation, no dispatch engine, no WebSocket yet — polling is enough for V1.

## Payments

Cash on delivery first (zero integration). One payment per checkout, with its per-store allocation rows created automatically at checkout — the record of how the customer's money separates per merchant. There is **no collection action anywhere in V1**: no money system exists yet, so nothing reconciles manually. When MoMo lands, collection becomes automatic (provider webhook), never a button. The provider reference and idempotency keys are already reserved in the schema.

## Not building in V1 (on purpose)

Dispatch engine, promotions/discounts, refunds workflow, settlements/payouts, saved-address book, staff-management UI (memberships are enforced server-side; inviting staff comes later), SMS gateway (dev OTP is a fixed code until then), MTN MoMo, Redis, Kafka, microservices — until the loop is real and something actually hurts. (Riders were "name+phone on the delivery" before D1; they are now Tuma-owned OTP accounts — the tracking doc §5 supersedes.)

## Build order (re-architecture program)

1. **S1 Docs** (this rewrite) → **S2** fresh migration chain + identity/authorization split → **S3** catalog + store_products + inventory → **S4** checkout, order groups, payments, allocations → **S5** mobile cart/checkout/orders on the new model → **S6** mobile store screen on store_products → **S7** platform (merchant catalog/assortment/orders, admin rework) → **S8** closeout + end-to-end verification.
2. Then build order #4: rider mode + real GPS tracking on the real map (per store order).
