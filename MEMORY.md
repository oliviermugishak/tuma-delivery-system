# MEMORY.md — Tuma project memory

Living memory for humans and agents. Read it right after `AGENTS.md`.
**Update this file after every durable change or decision** — append to the
log, rewrite "Current state", never silently delete history.

## Identity

Tuma is a commerce + delivery platform starting with food in Kigali.

The product in one sentence: *a mobile app to view merchants and their
products, add to cart, order, checkout, and watch the delivery move on a
real map.*

The killer feature is **real delivery tracking** — real GPS from a real
phone, drawn on a real map. Never simulated.

## Locked decisions (founder)

- **Backend is Rust** (Axum + SQLx + PostgreSQL). The founder thinks in Rust; TypeScript and Python ways were explicitly rejected. (2026-08-27)
- **Monorepo layout:** `tuma-server/`, `tuma-app/`, `tuma-platform/`, `tuma-docs/`. (2026-08-27)
- **Flutter app is a fresh start** from `flutter create` — nothing reused from the old prototype, not even seed data. (2026-08-27)
- **Visual identity = evolve-emerald:** keep emerald `#0B6E4F` / orange `#FF7A1A` / greige `#F7F7F5` + Poppins/Inter, rebuild every component from the roots at a higher polish level. (2026-08-27)
- **tuma-platform stays a template** until V1.5 (merchant web) / V2.5 (admin). Stack deliberately undecided. (2026-08-27)
- **Server house style mirrors `~/Work/projects/kanombe-sda`**: layered YAML config (`config` + dotenvy + secrecy), compile-time-checked SQLx with offline `.sqlx` caches, numbered `NN_name.sql` migrations in a `tuma` schema, UUID PKs, `updated_at` triggers, BIGINT RWF money, bunyan tracing, utoipa OpenAPI, `#[sqlx::test]` integration harness, postgres-only dev compose. (2026-08-27)
- **Dev database = `postgres:18`** via docker compose. `redis:7` exists on the machine and joins only when a real need appears. (2026-08-27)
- **Hand-to-hand contract:** every feature reviewed by the founder; every Flutter line written with the founder; every slice starts as a few approved lines of design before code. (2026-08-27)
- **Simplicity budget for V1:** 6 tables, ~10 endpoints, 6 order statuses (`placed, accepted, preparing, picked_up, delivered, cancelled`). Cash-on-delivery first; MTN MoMo after the loop works. (2026-08-27)
- **The TypeScript scaffold** (pnpm workspace, Fastify, packages/*) **was removed** — superseded by the Rust decision. The v2.0 blueprint's Fastify chapters are reference-only. (2026-08-27)
- **Foundations before features:** auth + RBAC are built before stores/products. The iteration order is: identity on the API → mobile splash/login/home on the real API → platform admin + merchant wings → then stores/products. (2026-08-27)
- **Three roles in V1:** `customer`, `merchant`, `admin` (of the blueprint's nine; mapping + promotion path documented in the auth doc). Merchant accounts are **created by admin** — no self-signup in V1. (2026-08-27)
- **Auth shape:** 2 credential methods (phone+OTP for customers — register and login are one flow; email+password for merchant/admin), 1 token format (HS256 JWT), 2 transports (httpOnly cookies + silent refresh for web — kanombe-sda pattern; Bearer + 30-day token for mobile). Dev OTP is a fixed code from `local.yml`. (2026-08-27)
- **Platform stack decided:** React + TypeScript + Vite + Tailwind + shadcn/ui + TanStack Query + TanStack Router + Orval (pnpm). This **overrides** the earlier "tuma-platform stays a template until V1.5/V2.5" decision — the platform joins the Foundations iteration. (2026-08-27)
- **OpenAPI is the contract:** served at `/api/v1/openapi.json`, exported via `run.sh openapi`; platform client is Orval-generated (never hand-edited), mobile client is hand-written Dart. (2026-08-27)
- **API versioning:** everything business under `/api/v1`, additive-only within v1, `/api/health` unversioned. (2026-08-27)
- **Logo:** the founder has logo files — they are provided when the mobile splash is built (slice S3). (2026-08-27)

## Current state

**Iteration: Foundations (auth + RBAC + first real clients).** Slice map S0–S8 lives in the approved plan and the two architecture docs.

- `tuma-docs/` — V1 Brief (updated: foundations-first build order, evolved users table, auth endpoints) + **Tuma_Auth_and_RBAC_Architecture.md** + **Tuma_API_Architecture.md** (both approved 2026-08-27) + blueprint/Flutter spec as long-term references.
- `tuma-server/` — **S0–S2 done, 22 tests green.** `/api/v1` nesting, OpenAPI at `/api/v1/openapi.json` + `export_openapi` bin; `accounts` crate (users/roles/jwt/password/otp), migrations 00–02, auth-context middleware (Bearer or cookie, fresh user lookup per request), `required_auth` + `require_role` guards, CSRF origin check, `GET /v1/me`, customer OTP (`otp/request` + `otp/verify`, register+login in one flow, dev fixed code `123456` from local.yml), Bearer `logout`, `seed_admin` bin, `.sqlx` offline cache committed. **Next: S3 mobile foundation** (api client, secure storage, splash + founder's logo, auth bootstrap).
- `tuma-app/` — V0 skeleton (emerald tokens, router shell). Awaiting S3 (api client, splash with founder's logo, auth bootstrap).
- `tuma-platform/` — template README + exported `openapi.json` until S6 scaffolds the shadcn app.

## Environment notes

- Arch Linux. Flutter 3.47.1 / Dart 3.13.1 · cargo/rustc 1.98.0-nightly · sqlx-cli 0.9.0 · Docker Compose 5.5.0.
- Docker images on this machine: `postgres:18`, `redis:7`, `mysql:8.0`.
- **Dev Postgres:** container `tuma-postgres-dev` (compose in `tuma-server/`, postgres/password123, db `tuma`). Volume mounts at `/var/lib/postgresql` — **never `/var/lib/postgresql/data`** (founder rule, new postgres paradigm).
- **SQLx offline cache:** `.sqlx/` is committed (CI builds against it); after changing any query, run `cargo sqlx prepare --workspace` with `DATABASE_URL` set.
- SQLx custom enums need schema-qualified `type_name` (e.g. `tuma.user_role`) or runtime lookups fail outside the schema's search_path.
- Reference project (pattern book, do not modify): `~/Work/projects/kanombe-sda`.

## Decision log

- **2026-08-27** — S2 customer OTP landed: `otp/request` + `otp/verify` (register and login are one flow), Bearer `logout`, dev fixed code from `local.yml`, 11 new tests (cooldown, attempt cap, code consumption, CSRF origin, validation) — 22 total green.
- **2026-08-27** — S1 identity core landed: `accounts` crate, migration 02, auth middleware with fresh user lookup (deactivated users lose access immediately), `/v1/me`, seed_admin, 11 tests green. Old squatter containers removed by founder; dev container is `tuma-postgres-dev`. Founder: tests passing is the verification bar — no extra smoke theater.
- **2026-08-27** — Foundations iteration approved: auth + RBAC before stores/products. Two architecture docs written and adopted (auth/RBAC, API). Brief updated (build order flipped, users table evolved, platform in scope). Platform stack locked: React + Vite + Tailwind + shadcn/ui + TanStack Query + Orval. Merchant accounts admin-created. Logo files to be provided by founder at S3.
- **2026-08-27** — `run.sh` dev runner added at repo root: `./run.sh mobile` (flutter run) and `./run.sh api` (cargo run). New day-to-day commands get added there as slices land.
- **2026-08-27** — Production vision docs written: Master Blueprint v2.0, Flutter spec v1.1, then the one-page V1 Brief superseded both as working truth. TS monorepo scaffolded, then removed after the Rust decision. Monorepo restructured: `tuma-server` (Rust skeleton) + `tuma-app` (Flutter skeleton) + `tuma-platform` (template) + `tuma-docs`. Hand-to-hand contract adopted.
