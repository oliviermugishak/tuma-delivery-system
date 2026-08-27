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

## Current state

- `tuma-server/` — V0 skeleton: health endpoint at `/api/health`, app-config crate, `configuration/*.yml`, migrations 00–01 (schema + trigger helpers), test harness. `cargo build` + `clippy` clean. **DB-dependent checks pending** (migrate, tests, boot) — need docker.
- `tuma-app/` — V0 skeleton: flutter create (android, ios, linux, web), counter boilerplate stripped, evolve-emerald tokens in `lib/core/theme/`, router shell with temporary root placeholder, smoke test passing, analyzer clean.
- `tuma-platform/` — template README only.
- `tuma-docs/` — Master Blueprint v2.0 (long-term reference), Flutter spec v1.1 (design reference), **V1 Brief (working truth)**.
- **Next slices** (each designed with the founder before code): server S1 stores + products schema + endpoints → app A1 navigation shell. See brief for the full order.

## Environment notes

- Arch Linux. Flutter 3.47.1 / Dart 3.13.1 · cargo/rustc 1.98.0-nightly · sqlx-cli 0.9.0 · Docker Compose 5.5.0.
- Docker images on this machine: `postgres:18`, `redis:7`, `mysql:8.0`.
- **Docker socket is not accessible from agent sandboxes** — the founder runs `docker compose up -d` and friends; agents should not block on docker.
- Reference project (pattern book, do not modify): `~/Work/projects/kanombe-sda`.

## Decision log

- **2026-08-27** — Production vision docs written: Master Blueprint v2.0, Flutter spec v1.1, then the one-page V1 Brief superseded both as working truth. TS monorepo scaffolded, then removed after the Rust decision. Monorepo restructured: `tuma-server` (Rust skeleton) + `tuma-app` (Flutter skeleton) + `tuma-platform` (template) + `tuma-docs`. Hand-to-hand contract adopted.
