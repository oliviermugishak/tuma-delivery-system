# Tuma

**Everything you crave, delivered.** A multi-merchant food marketplace for Kigali: browse real stores, check out in cash, and watch a real rider move on a real map.

The killer feature is **live delivery tracking** — GPS from a phone on a motorbike, drawn on Google Maps. Never simulated.

```text
customer (Flutter)     merchant + admin (web)     rider (same Flutter app)
        \                      |                      /
         \                     |                     /
                    tuma-server  (Rust / Axum)
                              |
                    PostgreSQL 18  ·  object storage (R2)
```

---

## Why this repo looks the way it does

- **The server owns truth.** Prices, stock, order status, and coordinates are computed or stored on the API. Clients render; they do not invent money or ETA.
- **Money is integer RWF.** No floats. Name and unit price are snapshotted onto the order line at checkout.
- **Six order states, no more:** `placed → accepted → preparing → picked_up → delivered`, or `cancelled` before pickup. `picked_up` only happens when a merchant types a real rider number.
- **Simple beats complete.** Every table and endpoint had to earn its place. Cash on delivery in V1 — cards and MoMo are a later adapter behind the same allocations ledger.

Long-form product rules live in [`tuma-docs/Tuma_V1_Brief.md`](tuma-docs/Tuma_V1_Brief.md). How humans and agents work in this tree: [`AGENTS.md`](AGENTS.md). Deploy posture: [`tuma-docs/DEPLOYMENT.md`](tuma-docs/DEPLOYMENT.md).

---

## Monorepo

| Path | Stack | Role |
|---|---|---|
| [`tuma-server/`](tuma-server/) | Rust, Axum, SQLx, PostgreSQL 18 | HTTP API, domain crates, migrations, OpenAPI |
| [`tuma-app/`](tuma-app/) | Flutter (Android first) | Customer + rider. Tokens only in `lib/core/theme/` |
| [`tuma-platform/`](tuma-platform/) | React, Vite, Tailwind, TanStack Query/Router | Admin + merchant console |
| [`tuma-docs/`](tuma-docs/) | Markdown | Brief, tracking architecture, deploy plan |

Identity is one `users` table. A **merchant** is a business, not a login; people attach through `merchant_memberships`. Customers sign in with phone + OTP. Merchants and admins use email + password (httpOnly cookies on the web). Riders are admin-created OTP accounts with a public **rider number** used at handoff.

---

## Architecture

```mermaid
flowchart TB
  subgraph clients [Clients]
    App[Flutter app<br/>customer + rider]
    Web[Platform<br/>admin + merchant]
  end

  subgraph edge [Edge]
    Maps[Google Maps / Directions / Geocoding]
    R2[Cloudflare R2<br/>public image URLs]
  end

  subgraph api [tuma-server]
    Axum[Axum /api/v1]
    Domain[crates: accounts · commerce · marketplace · storage]
    Axum --> Domain
  end

  PG[(PostgreSQL 18)]

  App -->|Bearer JWT| Axum
  Web -->|cookie session| Axum
  Domain --> PG
  Domain --> Maps
  Domain --> R2
  App -->|tiles + SDK| Maps
  Web -->|JS map pin| Maps
```

The API is stateless. Sessions are JWTs (mobile Bearer, web cookies + refresh row). Image bytes never live in Postgres — the API validates an upload and stores a content-UUID key; clients only ever see a URL.

### Domain, in one picture

```mermaid
flowchart LR
  User[users] --> Customer[customers]
  User --> Admin[admins]
  User --> Rider[riders]
  User --> Mem[merchant_memberships]
  Mem --> Merchant[merchants]
  Merchant --> Store[stores]
  Merchant --> Product[products]
  Store --> SP[store_products]
  Product --> SP

  Customer --> OG[order_groups]
  OG --> SO[store_orders]
  Store --> SO
  SO --> Item[order_items]
  SO --> Del[deliveries]
  Rider --> Del
  OG --> Pay[payments]
  Pay --> Alloc[payment_allocations]
  SO --> Alloc
```

A checkout is **one order group**, **one store-order per shop**, **one payment** with an allocation per store. Group status is derived from the children — there is no second state machine.

---

## How a delivery runs

```mermaid
sequenceDiagram
  actor C as Customer
  actor M as Merchant
  actor R as Rider
  participant S as Server

  C->>S: POST /v1/orders (idempotency key)
  S-->>C: group + store-orders (placed)
  M->>S: accept → preparing
  M->>S: handoff rider_number
  S-->>S: picked_up + Directions route
  R->>S: confirm pickup (GPS loop starts)
  loop every ~5s while app is open
    R->>S: POST location
    C->>S: GET tracking (204 if unchanged)
  end
  R->>S: delivered (cash collected)
  S-->>C: delivered
```

Cash only. The rider keeps the app in the foreground; the customer sees a gold road line, a teal GPS trail, and an orange marker that interpolates between real fixes. Two stores can mean two riders and two maps. Either side can cancel only before pickup.

---

## API surface (shape)

All business routes hang under `/api/v1`. Health is unversioned: `GET /api/health`.

| Area | Examples |
|---|---|
| Auth | OTP request/verify (customer + rider), email login (web), logout, `GET/PATCH /me` |
| Catalog | Open stores, store detail, search; merchant CRUD for stores / products / store-products |
| Commerce | Checkout, group detail, cancel, merchant board + status + handoff |
| Tracking | Rider location push, customer poll (`since` → 204), rider active list + delivered |
| Admin | Merchants, riders, customers, summary |
| Files | Authenticated upload; public read by key |

The living contract is the server’s OpenAPI (`/api/v1/openapi.json`). The platform client is generated (hey-api). The Flutter client is hand-written against the same contract.

---

## Local development

```bash
# API — Postgres 18 via compose
cd tuma-server
docker compose up -d
cp .env.example .env
# export DATABASE_URL=postgres://postgres:password123@localhost:5432/tuma
cargo sqlx migrate run
cargo run                          # :8080  →  /api/health

# Seed the Kigali pilot (3 businesses, 7 stores, 44 products) — idempotent
cargo run --bin seed_kigali
# Admin (optional): TUMA_ADMIN_PASSWORD=… cargo run --bin seed_admin

# Web console (proxies /api → :8080)
cd ../tuma-platform && pnpm install && pnpm dev     # :3000

# Customer / rider app
cd ../tuma-app && flutter pub get
# MAPS_API_KEY in tuma-app/.env  (never commit it)
./run.sh mobile
```

Dev OTP is a fixed code from `tuma-server/configuration/local.yml` (default `123456`). Seeded merchant logins are in [`MEMORY.md`](MEMORY.md) (change them before anything real).

**Test discipline:** do not run the full suite from an agent session (`cargo test --workspace` / whole-file `flutter test` can OOM). Run the slice under change. The founder runs the global suite.

After SQL changes: `cargo sqlx prepare --workspace` with `DATABASE_URL` set — `.sqlx/` is committed so CI/Docker compile offline.

---

## Production sketch

Stakeholder demo today: API on Render (Docker), Postgres on Neon, images on Cloudflare R2, Flutter APK pointed at the public API, platform as a static site with `VITE_API_BASE_URL`.

The intended long-term shape (see DEPLOYMENT.md): one VM in Johannesburg, Postgres co-located or Cloud SQL in-region, R2 for media, Android on Play + a later iOS slice. The server stays stateless so a second replica is a load balancer, not a rewrite.

---

## What V1 is not

Push notifications, background rider GPS, card/MoMo, self-serve merchant signup, dispatch auctions, ratings. Named and deferred — not forgotten.

---

## License

Private. All rights reserved.
