# Tuma — Authentication & RBAC Architecture

**Status:** approved foundation for V1 (iteration "Foundations", 2026-08-27).
**Supersedes:** the brief's old single-line `POST /auth/login (phone + OTP, minimal)`. The brief is updated to match this doc.
**House pattern:** `~/Work/projects/kanombe-sda` — two-cookie web session with silent refresh, argon2 password hashing, CSRF origin check, role middleware. We lift what is proven there and simplify where V1 allows.

---

## 1. Principles

1. **The server owns identity.** Clients never declare who they are, what role they have, or whether a code/password was valid. Every claim is checked server-side against the database.
2. **Passwordless where possible, passwords where practical.** Customers never carry passwords (phone + OTP). People who work (merchants, admin) use email + password — they are created by the platform, not self-signed-up.
3. **One token format, whatever the door.** Every authenticated request ultimately presents the same JWT shape; only the transport differs (cookie vs Bearer header).
4. **Simple now, extensible later.** Three roles, not nine. No permission matrix until a real org chart forces it. Every deferred piece is named in §14 so it has a clear slot to land in.

## 2. Roles

V1 has **three roles**, stored as Postgres enum `tuma.user_role` and carried in the JWT:

| Role | Who | Signs in with | Where |
|---|---|---|---|
| `customer` | app users ordering food | phone + OTP | mobile app |
| `merchant` | store owners | email + password | web platform |
| `admin` | system manager (founder) | email + password | web platform |

**Mapping to the blueprint's nine roles (ch 28):**

| Blueprint role | V1 treatment |
|---|---|
| `CUSTOMER` | `customer` |
| `MERCHANT_OWNER`, `MERCHANT_MANAGER` | collapsed into `merchant` until a store hires staff |
| `RIDER` | not an account yet — V1 riders are `rider_name`/`rider_phone` on the delivery row; the `rider` enum value lands when riders get accounts |
| `ADMIN`, `SUPER_ADMIN` | collapsed into `admin` |
| `SUPPORT_AGENT`, `OPERATIONS`, `FINANCE` | post-V1 |

Promotion path is additive: add an enum value, add a guard arm, add a platform wing. Nothing built today blocks it.

**Who creates whom:**
- Customers create themselves — implicitly: the first successful OTP verify on a phone number creates the account. There is no separate "register" step.
- Admin creates merchants (`POST /v1/admin/merchants`) with a name, email, and temporary password. The merchant signs in and changes the password.
- Admin accounts are seeded out-of-band (`seed-admin` binary), never via API.

## 3. Authentication matrix — the whole picture

By the end of the Foundations iteration Tuma has **2 credential methods, 1 token format, 2 transports, 3 roles**:

| # | Credential method | For | Flow |
|---|---|---|---|
| 1 | **Phone + OTP** (passwordless) | customers | request code → verify code → JWT |
| 2 | **Email + password** (argon2) | merchants, admin | login → session cookies |

| Token format | Transport | Used by | Lifetime |
|---|---|---|---|
| HS256 JWT | `Authorization: Bearer` header | mobile app | 30 days |
| HS256 JWT | `tuma-auth-token` httpOnly cookie | web platform | 15 minutes |
| opaque 256-bit refresh token (hashed in DB) | `tuma-auth-refresh` httpOnly cookie | web platform | 30 days |

## 4. Token design

JWT claims (HS256, `jsonwebtoken` crate, house style):

```json
{
  "iss": "tuma",
  "sub": "<user uuid>",
  "role": "customer | merchant | admin",
  "iat": 1756300000,
  "exp": 1756303600
}
```

- Signing key: `APP_SECRET__JWT_SIGNING_KEY` (config `secret.jwt_signing_key`, secrecy-wrapped, never logged). Local key is dev-only; production must rotate.
- The token carries **only** identity + role. Everything else (merchant's store, user's active state, name) is looked up from the database at request time — tokens never go stale with business data.
- `is_active = false` users are rejected at login and at `/me` re-check; an existing valid JWT of a deactivated user stops working on the next request that re-fetches the user (guards that need freshness check the DB — see §9).

## 5. Flows

### 5.1 Customer — phone + OTP (register and login are one)

```
app                        server                        Postgres
 │ POST /v1/auth/otp/request {phone}                       │
 ├─────────────────────────▶ rate-check (60s cooldown) ───▶│
 │                          generate 6-digit code,         │
 │                          store hash, TTL 5 min ────────▶│ auth_otps
 │                          [dev/local: fixed code]        │
 │◀───────────────────────── 200 {"message": …}            │
 │                                                         │
 │ POST /v1/auth/otp/verify {phone, code, name?}           │
 ├─────────────────────────▶ attempts < 5? not expired? ──▶│
 │                          code matches hash?             │
 │                          upsert user(role=customer) ───▶│ users
 │                          mint JWT (30 days)             │
 │◀───────────────────────── 200 {token, user}             │
 │                                                         │
 │ GET /v1/me   Authorization: Bearer <token>              │
 ├─────────────────────────▶ verify signature+exp ────────▶│ fetch user
 │◀───────────────────────── 200 {user}                    │
```

If `name` is provided at verify time and the user has none, it is set. Otherwise the app shows a one-field name screen and calls `PATCH /v1/me` (lands with S4 if needed; verify-time name covers most cases).

### 5.2 Merchant/admin — email + password with silent refresh (house style)

```
browser                    server
 │ POST /v1/auth/login {email, password}
 ├─────────────────────────▶ argon2 verify (timing-equalized:
 │                           unknown email still burns one hash),
 │                           check is_active,
 │                           insert refresh_tokens row
 │◀───────────────────────── 204
 │    Set-Cookie: tuma-auth-token=<JWT>          (15 min, httpOnly, SameSite=Lax)
 │    Set-Cookie: tuma-auth-refresh=<opaque>     (30 days, httpOnly, SameSite=Lax)
 │
 │ any later request, token cookie expired but refresh cookie valid:
 │    middleware hashes refresh, finds the row, mints a fresh JWT,
 │    sets a new tuma-auth-token cookie, continues the request.   ← silent refresh
 │
 │ POST /v1/auth/logout
 ├─────────────────────────▶ revoke refresh row, remove both cookies
 │◀───────────────────────── 204
```

### 5.3 Admin creates a merchant

```
admin (platform)  → POST /v1/admin/merchants {name, email, password, phone?}
                       → users row (role=merchant)          → 201 {merchant}
                  → GET  /v1/admin/merchants                → 200 [ … ]
                  → PATCH /v1/admin/merchants/:id {is_active}
                       → deactivate blocks login & /me      → 200
```

### 5.4 Logout semantics

- **Web:** logout revokes the refresh row and clears both cookies. The 15-min access JWT dies quickly on its own.
- **Password change** revokes ALL of the account's web refresh tokens in the same transaction — every session (including the one that changed the password) dies at its next silent refresh; already-minted access JWTs fade within their 15-minute TTL.
- **Mobile:** logout deletes the token from secure storage. The stateless JWT remains technically valid until expiry (30 days max) — acceptable for V1; server-side revocation is an extension point (§14).

## 6. OTP rules

| Rule | Value |
|---|---|
| Code | 6 digits, cryptographically random |
| Storage | hashed (never stored in plain text), single row per phone (upsert) |
| TTL | 5 minutes |
| Resend cooldown | 60 seconds per phone |
| Attempts | 5 per code, then the code is dead |
| Response shape | always the same 200 — never reveals whether the phone is known |
| Delivery | **dev/local:** fixed code from `configuration/local.yml` (`auth.dev_otp_code`), logged. **Production:** SMS gateway — a later slice, not built yet |

The fixed dev code exists **only** in local config; `production.yml` has no such key and the server refuses to start an OTP flow without a delivery method in production (guard added with the SMS slice).

## 7. Password policy

- Hashing: **argon2** (house style), run in `spawn_blocking` behind a semaphore to throttle parallel-hash DoS.
- Minimum 8 characters with at least one lowercase letter and one digit (validator rules, same as house style); 422 with field details otherwise.
- Wrong-email and wrong-password return the **same** 401 message ("Invalid email or password") and burn equal CPU time.
- Change password (`POST /v1/auth/password`) requires the current password and revokes all web refresh tokens for the user.

## 8. Cookies & CSRF

- Cookie names: `tuma-auth-token`, `tuma-auth-refresh`. Attributes: `Path=/`, `HttpOnly`, `SameSite=Lax`, `Secure` per config (`cookie_secure: true` in production). Removal uses the same `Path=/`.
- **CSRF:** every state-changing request (POST/PUT/PATCH/DELETE) that carries an `Origin` header must have that origin in `TUMA_CORS_ORIGIN`; mismatch → 403. Requests without an `Origin` header (mobile Bearer clients, curl, server-to-server) pass — Bearer tokens are not auto-attached by browsers, which is what CSRF exploits.
- CORS layer: explicit origin allowlist from `TUMA_CORS_ORIGIN`, `Content-Type` header only, credentials allowed. In dev the platform's Vite proxy (`/api → 127.0.0.1:8080`) sidesteps CORS entirely; the allowlist matters for direct-origin setups and production.

## 9. RBAC enforcement — two tiers

1. **Route-level role guard.** `require_role([Admin])` / `require_role([Merchant, Admin])` middleware wraps the `/v1/admin/*` and `/v1/merchant/*` sub-routers (same wiring shape as kanombe-sda's `require_permissions`). No session → **401**; wrong role → **403**.
2. **Ownership checks in handlers/domain** (lands with stores & orders): merchant ↔ own store, customer ↔ own orders. Ownership is always resolved from the database (`stores.owner_id = <token user>`), never from client input.

The auth context is built once per request by the **auth-context middleware**: it reads the token cookie *or* the `Authorization: Bearer` header, verifies the JWT, and inserts `UserContext { user_id, role }` into request extensions. Handlers extract it; nothing parses tokens anywhere else.

**Graduation path:** if merchant staff or support roles appear, we adopt the blueprint's capability model (ch 28) in kanombe-sda's shape — permissions as compile-time code constants (`grant!`-style), not database rows. Not built in V1.

## 10. Schema — migration `02_identity.sql`

```sql
CREATE TYPE tuma.user_role AS ENUM ('customer', 'merchant', 'admin');

CREATE TABLE tuma.users (
  id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  role          tuma.user_role NOT NULL,
  name          TEXT,
  phone         TEXT UNIQUE,           -- customer identity
  email         TEXT UNIQUE,           -- merchant/admin identity
  password_hash TEXT,                  -- merchant/admin
  is_active     BOOLEAN NOT NULL DEFAULT true,
  created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
  CONSTRAINT identity_check CHECK (
    (role = 'customer' AND phone IS NOT NULL) OR
    (role IN ('merchant', 'admin') AND email IS NOT NULL AND password_hash IS NOT NULL)
  )
);
SELECT tuma.trigger_updated_at('tuma.users');

CREATE TABLE tuma.auth_otps (
  phone       TEXT PRIMARY KEY,
  code_hash   TEXT NOT NULL,
  expires_at  TIMESTAMPTZ NOT NULL,
  attempts    INT NOT NULL DEFAULT 0,
  created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE tuma.refresh_tokens (
  token_hash  TEXT PRIMARY KEY,
  user_id     UUID NOT NULL REFERENCES tuma.users(id) ON DELETE CASCADE,
  expires_at  TIMESTAMPTZ NOT NULL,
  revoked_at  TIMESTAMPTZ,
  created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX refresh_tokens_user_idx ON tuma.refresh_tokens (user_id);
```

This evolves the brief's `users (id, phone, name)` — the brief now shows the evolved shape.

## 11. Middleware stack (order, outermost last)

```
request → TraceLayer → CORS → csrf_origin_check → auth_context → [require_role] → handler
```

- `auth_context` also performs **silent refresh** for cookie clients (re-mint on expired token cookie + valid refresh cookie), and skips re-mint on `/v1/auth/logout` (house-style detail: never resurrect a session mid-logout).
- Bearer clients get no cookie side-effects.

## 12. Dev vs production behavior

| Concern | local (dev) | production |
|---|---|---|
| OTP delivery | fixed code from `local.yml`, logged | SMS gateway (later slice) |
| Cookies `Secure` | false | true |
| JWT key | dev key in `local.yml` | unique, from env/secret store |
| First admin | `seed-admin` binary | `seed-admin` binary, strong password, rotate after |
| Migrations | `cargo sqlx migrate run` by hand | on boot (existing behavior) |

## 13. Security posture (what is built in, by design)

- Timing-equalized password login; opaque identical error messages for unknown email vs wrong password.
- OTP codes hashed; attempt-capped; cooldown-limited; response shape leaks nothing.
- Refresh tokens stored as hashes only; revocable; cascade-deleted with the user.
- Password hashing throttled by semaphore.
- CSRF origin allowlist on all mutations carrying an `Origin` header.
- Secrets via `secrecy`; never in logs; JWT signing key out of config files in production.

## 14. Extension points (named, deferred — not forgotten)

| Extension | Trigger to build | Slot |
|---|---|---|
| SMS gateway for OTP | first real customer outside the office | replaces dev fixed code behind the same endpoints |
| `rider` role + OTP login | riders get accounts (post first pilot) | new enum value + `/v1/rider/*` namespace |
| Mobile refresh tokens / revocation list | if 30-day re-OTP ever feels bad | `/v1/auth/refresh` + `refresh_tokens` reuse |
| Capability RBAC (`grant!`-style) | first merchant staff account | replaces role guard, keeps middleware shape |
| Webhook signature verification | MTN MoMo callbacks | shared secret + signature check middleware |
| Admin MFA | blueprint 28.3, when admin holds money keys | second factor at login |
| Rate limiting beyond DB checks | abuse appears | Redis token bucket (redis:7 is on the machine) |
