# Agent Task: Integrate File/Image Storage Service into E-Commerce Backend

## 0. Read before you write anything

Before touching any code, do the following in order and summarize what you find:

1. Read `memory.md` and `agents.md` at the project root (or wherever they live — search for them if not at root). Treat their contents as binding project context and conventions — coding style, naming rules, architectural decisions already made, and anything explicitly marked as a constraint or "do not change."
2. Walk the project structure (`src/`, `migrations/`, `Cargo.toml`, any `crates/` workspace layout) and identify:
   - The Axum router setup and how routes/handlers are currently organized (single crate vs. modular by domain, e.g. `handlers/`, `routes/`, `services/`).
   - The current sqlx setup: connection pool creation, `sqlx::migrate!` usage, whether migrations are versioned/timestamped, and whether `sqlx-cli` or an embedded migrator is used.
   - The existing schema for **merchants**, **stores**, and **products** (or equivalent entities) — exact table names, column names, primary/foreign key conventions, and existing image-related fields if any (e.g. a stray `image_url TEXT` column already in use).
   - Existing error-handling conventions (custom `AppError` type, `thiserror`, `anyhow`, response mapping into Axum `IntoResponse`).
   - Existing config/env loading pattern (e.g. `dotenvy` + a `Config` struct, `envy`, or manual `std::env::var`).
   - Existing test setup (integration tests against a real/test Postgres, `sqlx::test` macro usage, fixtures).
3. Report back a short summary of these findings **before making changes**, including any ambiguities or missing conventions you'll need to make a judgment call on.

Do not assume any of the above — verify against the actual code. If `memory.md` or `agents.md` conflict with anything in this prompt, the project files win; flag the conflict explicitly rather than silently picking one.

## 1. Objective

Integrate a swappable file/image storage service into the existing Axum + Postgres + sqlx backend, and wire it into the data model for a multi-merchant e-commerce system where:

- Each **merchant** has one **store**, and a store can have a **store image** (logo/banner — treat as two distinct optional images unless the existing schema already models this differently).
- Each **product** can have **multiple images** (a primary/cover image plus a gallery), not just one.

The storage layer must support at minimum: local filesystem (dev), in-memory (tests), and an S3-compatible backend (prod — Cloudflare R2). Backend selection must be config-driven, not hardcoded.

## 2. Storage service requirements

- Implement (or adapt, if a storage abstraction already exists in the project — check first) a `FileStore` trait or use the `object_store` crate directly behind a thin app-level wrapper. Prefer `object_store` unless the project already has its own convention.
- Backend selection driven by an `AppConfig`/env value (e.g. `STORAGE_BACKEND=local|memory|s3`), consistent with however this project already loads config.
- Support: `put`, `get`, `delete`, and `exists` at minimum. Add a `presigned_url` or equivalent if the S3-compatible backend supports it and the product requirement calls for direct client uploads — otherwise proxy uploads through the API.
- Generate storage keys deterministically and collision-safely, e.g. `merchants/{merchant_id}/store/{image_kind}.{ext}` and `products/{product_id}/images/{image_id}.{ext}` — do not use the raw uploaded filename as the key.
- Validate content type and size before accepting an upload (reject anything that isn't an allowed image MIME type; enforce a max size — confirm the limit with existing conventions or default to a sane one, e.g. 5 MB per image, and flag this as an assumption).
- Wire the store into Axum application state (`Arc<FileStore>` or similar) following the project's existing `AppState` pattern — do not introduce a second, parallel state pattern.

## 3. Schema changes

Design and write sqlx migrations (additive, non-destructive, reversible where the project's migration tooling supports down-migrations) for:

- **Store images**: add the necessary column(s)/table to the merchant or store entity to hold a storage key/URL for logo and/or banner image. If the existing schema has one store per merchant, prefer columns on the existing table (`logo_key`, `banner_key`) over a new table unless a store can have many images, in which case model it the same way as product images below.
- **Product images**: since products need *multiple* images, do **not** add an array/JSON column to the `products` table. Instead create a `product_images` table:
  - `id` (PK, uuid or bigserial — match existing PK convention in the project)
  - `product_id` (FK → `products.id`, `ON DELETE CASCADE`)
  - `storage_key` (text, not null — the object store key, not the full URL)
  - `position` or `sort_order` (int, for gallery ordering)
  - `is_primary` (bool, default false) — or derive "primary" from `position = 0` if that better matches project conventions; pick one and be consistent
  - `created_at` (timestamptz, default now())
  - An index on `product_id`, and a partial unique index enforcing at most one `is_primary = true` per product if you use that approach.
- Match whatever primary key type, timestamp conventions, and naming style (snake_case, plural table names, etc.) the existing migrations already use — do not introduce a new convention.
- Each migration must be a separate file following the project's existing migration naming/timestamp scheme. Do not squash unrelated schema changes into one migration.

## 4. Application layer

- Add/extend sqlx query functions (or the project's existing repository/DAO layer, if one exists) for: inserting a product image record, listing images for a product ordered by position, deleting an image record, and setting/updating a store's logo/banner key. Use the same query style already in use (raw `sqlx::query!`/`query_as!` macros vs. a query builder) — don't mix styles.
- Add Axum handlers for upload endpoints (e.g. `POST /merchants/:id/store/logo`, `POST /products/:id/images`, `DELETE /products/:id/images/:image_id`), following the existing route registration and auth/middleware pattern (merchant must own the resource they're uploading to — check how ownership/auth is currently enforced and reuse it, don't invent a new auth check).
- On upload: validate → write bytes to the storage backend → insert/update the DB row → return the resulting resource (including a usable URL or key) in the response. If the DB insert fails after a successful storage write, decide and document a cleanup/compensation strategy (e.g. best-effort delete-on-failure) rather than leaving orphaned objects silently.
- On delete: delete the DB row and the underlying storage object; decide and document ordering (DB first vs. storage first) with the failure mode in mind.

## 5. Testing

- Unit/integration tests for the storage wrapper itself using the in-memory backend — no real network or filesystem dependency.
- Integration tests for the new handlers using the project's existing test-database setup (`sqlx::test` or equivalent) combined with the in-memory storage backend, so tests are hermetic and fast.
- At least one test verifying migration correctness (schema applies cleanly on a fresh test DB, matching the project's existing migration test pattern if one exists).
- Do not write tests that depend on real R2/S3 credentials.

## 6. Constraints — do not violate these

- Do not change unrelated code, rename existing columns/tables, or "clean up" adjacent code as a side effect. Scope is strictly the storage integration described above.
- Do not introduce a new dependency for something the project already has a working solution for (e.g. don't add a second HTTP client, a second error-handling crate, or a second config-loading approach).
- Do not hardcode credentials, bucket names, or endpoints — everything storage-related must come from config/env.
- Do not write a destructive migration (dropping/renaming existing columns in a way that loses data) without calling it out explicitly and pausing for confirmation before proceeding.
- If `memory.md`/`agents.md` specify a preferred crate, pattern, or explicitly forbid something (e.g. "no `unwrap()` in handler code"), follow that over the defaults suggested in this prompt.

## 7. Deliverables

1. The read-first summary from Section 0.
2. A short design note (5–10 sentences) stating: chosen storage crate/pattern, the exact schema you're adding, and any assumptions made where information was missing — before writing code, if the change is non-trivial, so I can confirm the approach.
3. The migrations.
4. The storage service code and its wiring into `AppState`.
5. The new/modified handlers and routes.
6. The new/modified sqlx queries.
7. Tests covering the above.
8. A final summary of every file touched and why.
