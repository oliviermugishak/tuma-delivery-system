# AGENTS.md — Tuma monorepo

Read this before doing anything in this repository.

## Source of truth

1. `tuma-docs/Tuma_V1_Brief.md` — **the working source of truth for V1.** Six tables, ~10 endpoints, six order statuses, real tracking. If code and brief disagree, stop and surface the conflict.
2. `MEMORY.md` — living project memory: locked decisions, current state, environment notes. Read it, and **update it after every durable change or decision**.
3. `tuma-docs/Tuma_Master_Product_Design_Engineering_Blueprint.md` — long-term reference only. The brief wins for V1.
4. `tuma-docs/Tuma_Flutter_Mobile_App_Engineering_Spec.md` — Flutter design/system reference (evolve-emerald identity lives here).

## Repository map

| Path | What |
|---|---|
| `tuma-server/` | Rust backend: Axum + SQLx + PostgreSQL. House style mirrors `~/Work/projects/kanombe-sda` (layered YAML config, compile-time-checked SQLx, numbered SQL migrations, bunyan tracing, utoipa OpenAPI, integration test harness). |
| `tuma-app/` | Flutter customer app. Design tokens in `lib/core/theme/`. |
| `tuma-platform/` | Web platform: admin + merchant wings. React + TypeScript + Vite + Tailwind + shadcn/ui + TanStack Query; API client Orval-generated from the server's OpenAPI. |
| `tuma-docs/` | Documentation. Doc changes are founder decisions. |

## The hand-to-hand contract

This project is built by the founder and AI agents working **together, line by line**:

1. **Every feature is reviewed by the founder.** No feature is merged autonomously.
2. **Every line of Flutter is written hand-to-hand with the founder.** Agents propose; the founder approves and reviews. Do not generate screens in bulk.
3. **Design before code.** Every slice starts as a few lines of design (tables, endpoints, or screen behavior) that the founder approves. Then code. Never the reverse.
4. **Simple beats complete.** Every table, endpoint, dependency, and abstraction must earn its place. When in doubt, leave it out.
5. **The server owns truth.** Prices, order status, delivery location — the apps never invent or recompute them.
6. **Tracking must be real.** Real GPS from a real phone on a real map. Never simulated, never faked — tracking is the killer feature.
7. **Money is integer RWF.** Never floats. Orders snapshot name + price at order time.
8. **Order status is six values:** `placed, accepted, preparing, picked_up, delivered, cancelled`. Do not add states until a real operation forces it.
9. **Do not rewrite unrelated code during a slice.** Stay in the slice.
10. **Surface conflicts, don't resolve them silently.** Docs vs code, brief vs blueprint — ask.

## Server endpoint checklist (kanombe-sda house style)

Every new endpoint lands as one reviewed slice containing:

```text
1. migration          tuma-server/migrations/NN_name.sql  (UUID PKs, created_at/updated_at + trigger, BIGINT RWF)
2. domain function    domain crate (or module) — typed errors, takes &mut PgConnection
3. handler            tuma-server/src/routes/<area>.rs — #[utoipa::path] + #[tracing::instrument], ValidatedJson inputs
4. registration       tuma-server/src/app.rs — route + middleware
5. openapi            list in tuma-server/src/api_doc.rs
6. integration test   tuma-server/tests/<area>.rs via #[sqlx::test(migrator = "MIGRATOR")] + tests/common harness
```

Server conventions: layered config (`configuration/*.yml` + `APP_*__*` env, `DATABASE_URL` wins) · rustls-only · `AppError` → `ApiErrorResponse` JSON · handlers return `AppResult<impl IntoResponse>` · 201 for creates, 204 for auth mutations, `Json` for reads.

## Flutter conventions

- Tokens in `lib/core/theme/` — raw hex values appear nowhere else.
- Structure: `lib/core/{theme,router,constants,utils}`, `lib/features/<feature>/`, `lib/shared/widgets/`.
- One dominant action per screen; price legible before commitment; every screen handles loading/empty/error.
- New dependencies only with the slice that needs them, named in the slice design.
- The API client in `lib/core/api/` is **hand-written** — typed per slice against the OpenAPI contract, never code-generated.

## Platform conventions

- Stack: React + TypeScript + Vite + Tailwind + shadcn/ui + TanStack Query + TanStack Router, pnpm.
- `src/api/generated/` is **Orval output — never hand-edited**. After any server API change: `./run.sh openapi` then `pnpm generate:api`, and review the generated diff as part of the slice.
- Auth is cookie-based (httpOnly session) — no tokens in JS state. Dev server proxies `/api` to the local server.
- Wings by role: `/admin/*`, `/merchant/*`, shared login. See `tuma-docs/Tuma_API_Architecture.md`.

## Commands

```bash
# dev runner (root) — day-to-day commands live here
./run.sh mobile                      # flutter run
./run.sh api                         # cargo run the server
./run.sh openapi                     # export OpenAPI spec to tuma-platform/openapi.json

# server
cd tuma-server
docker compose up -d                     # dev postgres:18
source init_db.fish                      # DATABASE_URL
cargo sqlx migrate run
cargo run                                # :8080, health at /api/health
cargo fmt && cargo clippy --workspace --all-targets && cargo test
cargo sqlx prepare --workspace           # refresh .sqlx offline cache after query changes

# app
cd tuma-app
flutter pub get && flutter analyze && flutter test
flutter run
```

**Test discipline (founder rule, 2026-08-30):** agents NEVER run the full
suite — `cargo test --workspace` and whole-file `flutter test` runs can
crash under load and waste time. Run ONLY your slice's new tests
(`cargo test --test <area>` / `flutter test --plain-name "<name>"`); when
they pass, move on. The founder runs the global suite himself.

## Founder-review required (never autonomous)

Payments and money math · order/payment/delivery state · migrations ·
auth · pricing · anything in `tuma-docs/` · new dependencies · deleting or
restructuring folders.
