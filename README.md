# Tuma — monorepo

**Everything you crave, delivered.** A commerce + delivery platform starting
with food in Kigali: view merchants and products, add to cart, order,
checkout, and watch the delivery move on a real map.

## Monorepo map

| Folder | What | Status |
|---|---|---|
| [`tuma-server/`](tuma-server/) | Rust backend — Axum + SQLx + PostgreSQL (house style mirrors `kanombe-sda`) | V0 skeleton |
| [`tuma-app/`](tuma-app/) | Flutter customer app (android, ios, linux, web) — evolve-emerald identity | V0 skeleton |
| [`tuma-platform/`](tuma-platform/) | Web platform: admin console + merchant web | Template — V1.5/V2.5 |
| [`tuma-docs/`](tuma-docs/) | Product + engineering documentation | Living |

## Read first

1. [`tuma-docs/Tuma_V1_Brief.md`](tuma-docs/Tuma_V1_Brief.md) — **one page, the working source of truth for V1.**
2. [`AGENTS.md`](AGENTS.md) — how humans and AI agents work in this repo (hand-to-hand contract).
3. [`tuma-docs/Tuma_Master_Product_Design_Engineering_Blueprint.md`](tuma-docs/Tuma_Master_Product_Design_Engineering_Blueprint.md) — long-term reference only.

## Quickstart

### Server

```bash
cd tuma-server
docker compose up -d                 # dev Postgres (postgres:18)
cp .env.example .env
source init_db.fish                  # exports DATABASE_URL
cargo sqlx migrate run
cargo run                            # http://127.0.0.1:8080/api/health
```

### App

```bash
cd tuma-app
flutter pub get
flutter run                          # pick a device (android/ios/linux/web)
```

## The loop we are building

```text
real customer → real merchant → real order → real rider
→ real GPS on a real map → delivered
```

Simple schemas, simple implementations, real everything. No simulation.
