# Tuma V1 — Final Review & Blueprint Verdict

**Date:** 2026-09-01 · **Scope:** the whole monorepo as shipped in V1 (prototype; payments/MoMo excluded by the founder's scope note) · **Method:** three parallel deep reviews (server code, app code, blueprint vision) with the highest-impact findings re-verified by hand against the actual files, not taken on faith.

---

## Part 0 — The one-paragraph verdict

**The prototype is genuinely good, and the blueprint's spine was honored — but V1 is not "everything is good."** The server's money and status core is textbook (atomic checkout, anti-probe reads, a real transition machine), and the customer journey the blueprint promised (browse → cart → checkout → live GPS tracking) is complete and real. However, this review found **4 high-severity defects that must be fixed before this system touches real merchants or real stock** — chief among them that **cancelling an order permanently burns the reserved stock and strands the group's payment ledger** — plus a handful of user-facing app bugs (a hardcoded avatar on Home, the product-sheet quantity stepper silently adding 1 instead of the chosen quantity, and a boot path that destroys a valid token when the network hiccups). None of these are hard to fix; all of them are listed below with exact locations. Against the blueprint itself: we executed its architecture faster and stricter than it asked, cut its breadth deliberately (payments, notifications, dispatch, promotions), and made one big bet against its phasing (multi-store checkout) that mostly paid. Details in Part 3.

---

## Part 1 — Server review (tuma-server)

**No CRITICAL findings.** The checkout — the single most dangerous transaction in the system — was verified clean end to end: one transaction including stock decrements, deadlock-ordered updates, idempotency pre-check + `UNIQUE(user_id, idempotency_key)` with a proper race-recovery path, and price/fee recomputation that ignores whatever the client sends. Anti-probe scoping (foreign order = 404) is real on every read I traced. The six-status machine is enforced, with `picked_up` reachable only through the rider-handoff endpoint. `advance` + `settle_delivery_cash` confirmed as one atomic transaction, and the `eta_target` MIN query for group summaries is correct (the LEFT JOIN can't fan out — `deliveries.store_order_id` is UNIQUE).

### HIGH severity (fix before "done")

**S1. Cancellation never restores stock nor settles the ledger.**
`commerce/src/orders.rs:1071-1097` — `cancel_own_store_order` just flips status. Nothing anywhere adds stock back, and the store's `payment_allocations` row stays `pending` forever. Two consequences:
- Every customer cancel **permanently burns the reserved stock** (a cancelled "Rice 5KG" is gone from inventory).
- `settle_delivery_cash`'s group-completion rule ("collected when nothing is pending", `commerce/src/deliveries.rs:415-418`) can never fire for a group with one cancelled sibling → the group payment is stuck `pending` even after every remaining delivery settles, so the merchant ledger is wrong.
**Fix:** inside the cancel path's transaction — restore `store_products.stock += quantity` for the order's items, and mark that store order's allocation `refunded`.

**S2. Status transitions have no compare-and-swap.**
`commerce/src/orders.rs:1034-1047` — the UPDATE is `WHERE id = $1` with no `AND status = $expected`; same shape in handoff (`deliveries.rs:206-226`) and rider delivered (`deliveries.rs:460-469`). Read-check-write races mean, e.g., a customer cancel and a merchant accept can interleave — last write wins, and a cancelled order can be overwritten to `picked_up` with a rider attached to a dead delivery.
**Fix:** `UPDATE … WHERE id = $1 AND status = $expected` + treat `rows_affected == 0` as `Illegal`. Same CAS on the handoff/delivered UPDATEs (also closes S12 below).

**S3. The live Google Directions API key sits in plaintext in `tuma-server/.env`.**
`.env` is correctly gitignored and is NOT in git history (verified) — but it's a billable key that leaks on the first repo zip or machine share.
**Fix:** rotate the key, add referrer/IP restrictions in Cloud console, treat it as runtime-only.

**S4. OTP attempt cap is racy.**
`accounts/src/otp.rs:123-153` — SELECT attempts → check → separate UPDATE. N parallel verifies all read `attempts=0` and each get a guess, so the 5-attempt cap is a soft cap under brute force.
**Fix:** one atomic `UPDATE … SET attempts = attempts + 1 WHERE phone = $1 AND attempts < 5 AND code_hash = $2 AND expires_at > now() RETURNING …` (rows_affected 0 = failed).

### MEDIUM severity

| # | Finding | Where | Fix shape |
|---|---|---|---|
| S5 | Re-armed ETAs are invisible to the 204 poll — `changed_at` omits `d.updated_at`, so a stray-rider re-route (new ETA) still answers 204 | `deliveries.rs:766-771, 794-811` | include `d.updated_at` in both max() computations |
| S6 | Checkout + saved addresses accept `lat=9999` (stores/search validate, these don't) → garbage distances | `routes/orders.rs:176`, `routes/addresses.rs:61` | reuse the shared range validators |
| S7 | Money overflow: no price ceiling + unchecked `unit_price * quantity` → 500/wrap on absurd inputs | `routes/catalog.rs:308`, `commerce/src/orders.rs:533,606` | `checked_mul` or price cap |
| S8 | `delivery_locations` never pruned (migration comment promises pruning that doesn't exist); `refresh_tokens` never cleaned | `07_delivery_tracking.sql:22` | scheduled DELETE by age |
| S9 | Refresh tokens never rotate — a stolen 30-day mobile token lives its whole TTL | `accounts/src/refresh_tokens.rs:47-64` | rotate on silent refresh, revoke lineage |
| S10 | Checkout line count is unbounded (a 100k-line cart = 100k inserts in one tx) | `routes/orders.rs:188` | cap items (e.g. 50) |
| S11 | Merchant board has no status filter — delivered rows pollute paging forever | `routes/orders.rs:571` | `?status=` param, default in-flight |
| S12 | Concurrent handoffs silently replace the rider (first rider's card vanishes, no signal) | `deliveries.rs:206-240` | CAS the delivery UPDATE (same as S2) |

### LOW severity (hygiene)

- Dead code: `store_orders_for_merchant_scoped` (zero callers), `GoogleKeys.directions` hardcoded `None`, `static_dir` parsed-but-unserved, `Route.distance_m` parsed-but-unread.
- Unpaginated admin lists + unbounded `open_stores`/`search_stores` — fine at pilot scale, noted for later.
- A **third, undocumented** haversine copy lives inline in SQL (`rider_distance_m`, `deliveries.rs:710-717`) — the other two duplicates are deliberate and documented; add a comment here.
- `files.rs` claims "unguessable content-UUID" keys, but keys derive from entity UUIDs — images are public by design; fix the comment.
- OpenAPI drift: health declared as `/health`, served at `/api/health`. All other 65 paths match.
- Address PATCH silently ignores `is_default=false` (probably intended; document it), and two `map_err` matches in `routes/addresses.rs` violate the house "one From impl in error.rs" rule.

**Server clean areas (verified):** checkout atomicity + idempotency · anti-probe on every read · the six-status machine incl. the handoff gate · centralized error mapping, no secrets in logs/responses · hashed OTPs/refresh tokens + timing equalization · production.yml fails fast, no fixed OTP in prod, `cookie_secure: true`.

---

## Part 2 — App review (tuma-app)

**No CRITICAL findings on the normal mobile path.** Timer/observer hygiene is genuinely good (every timer cancelled, every observer removed, poll ticks guarded), there is zero logging of tokens, the 401 sign-out loop is bounded, the router's role matrix has no holes (except F4 below), and every screen has honest loading/empty/error states.

### HIGH severity

**A1. Home shows hardcoded `'MO'` avatar initials for every user.**
`lib/features/home/home_screen.dart:312` — `String identityText(dynamic user) => 'MO';`. The only screen in the app with fake initials (Profile, rider, order cards all use real names).
**Fix:** return the user's displayName/phone, or delete the function and pass the name to `AccentAvatar`.

**A2. Booting with no network silently signs the user out — a valid 30-day token is destroyed.**
`lib/core/auth/auth_controller.dart:157-163` — in `bootstrap()`, any failure of `me()` (including `ApiNetwork`: airplane mode, dead Wi-Fi) clears the token and lands anon. The user must re-do SMS OTP because the elevator had no signal.
**Fix:** only clear on `ApiUnauthorized`; on network errors keep the token and retry on next foreground.

**A3. The name screen overflows under the keyboard.**
`lib/features/auth/name_screen.dart` — plain Column + `autofocus: true` + Spacer, no scroll view. The phone and OTP screens were fixed for exactly this; the name screen was missed. First-run flow on a small phone = yellow stripes.
**Fix:** same `SingleChildScrollView` + `ConstrainedBox` treatment as its two siblings.

**A4. `/success` crashes on cold start without `extra`.**
`lib/core/router/app_router.dart:84` — `state.extra!` with no redirect guard (OTP/name have guards; `/success` doesn't). Unreachable on the phone today, but it's the same deep-link class.
**Fix:** redirect `/success` with null extra → `/home`.

**A5. The product-sheet quantity stepper silently adds 1 instead of the chosen quantity.**
`lib/features/cart/cart_notifier.dart:247-254 and 258-272` — both new-item branches hardcode `quantity: 1`, ignoring the `quantity` parameter (it only works when the line already exists). The sheet's "Add to cart · 3×" CTA adds ONE item while showing 3×price. **Verified by hand in the file.**
**Fix:** `quantity: quantity` in both `CartItem(...)` constructors.

### MEDIUM severity

| # | Finding | Where | Fix shape |
|---|---|---|---|
| A6 | Cart lost-update race: `_emit` awaits persistence BEFORE setting state — two rapid `+` taps both read the same old state, one increment is lost | `cart_notifier.dart:183-197` | set state synchronously, persist after |
| A7 | The 5s poller rebuilds Home + Orders fully every tick even when nothing changed (fresh list object each time) | `active_orders_provider.dart:78-88` | compare with previous list, skip the write when equal |
| A8 | Money fields parse with `as int` — a float from the server throws a `TypeError` that escapes the detail screen's catch → eternal spinner | `models/order.dart` (many) | `(json[...] as num).toInt()` + catch-all in `_fetch` |
| A9 | Rider "Mark delivered": `setState` after the dialog await without a `mounted` check — crash if a 401 lands while the dialog is open | `rider_screen.dart:361-363` | add the guard |
| A10 | Checkout/Cart/Location back buttons `context.pop()` with no stack (cold-start crash class) | `checkout_screen.dart:358`, `cart_view.dart:604`, `location_screen.dart:344` | the `canPop()` pattern order-detail already uses |
| A11 | ~12 identical copy-pasted floating snackbars that fight the theme | across screens | one `showAppSnack()` helper |

### LOW severity

- Dead code: `FeeChip`, `InitialsTile`, `_SheetGallery`, `DeliveryPinMap` (all zero usages) — deletable.
- `_rideSpeedEtaMinutes` always returns null while its comment advertises an ETA preview; `cart_view.dart:117` has an if-branch where both arms are identical.
- Cancelled orders render an all-grey "Placed" stepper instead of a dedicated cancelled state; the stepper also marks done-by-any-store on multi-store groups.
- `tel:`/`mailto:` launched with unsanitized server strings (a crafted phone could dial extra digits) + some launch sites don't handle `launchUrl` returning false.
- Search-added cart items carry `deliveryFee: 0` (total understates until checkout; the "prices confirmed at checkout" line covers it, but it's a lie in the preview).
- No `Hero` animations anywhere; no pull-to-refresh on store menu or Search.
- Kiosk auto-stops location sharing on a single empty poll — a server hiccup silently ends the run (consider two consecutive empties).

**App clean areas (verified):** token storage in flutter_secure_storage, zero debugPrint of secrets · timer hygiene as above · 401 loop bounded · idempotency key lifecycle correct · tracking merge logic (group header vs per-store sections, 204 economy) correct · image decode budgets + lazy grids deliberate.

---

## Part 3 — Blueprint verdict: how close did we get?

*(Blueprint = `Tuma_Master_Product_Design_Engineering_Blueprint.md`, 48 chapters. The V1 Brief supersedes it for V1; where we diverged, I name the decision and judge whether it aged well.)*

### The headline

The blueprint's core bet — *"the customer experience is the visible product; the operational system underneath is the defensibility"* — was honored, and the system today genuinely answers the blueprint's "first battle": an order moves from a real customer to a real merchant to a real rider and back, on real GPS, with server-owned truth at every step. **We diverged on sequencing and surface, almost never on principle.**

### Pillar by pillar

| Blueprint pillar | Verdict |
|---|---|
| **"The clients render state. The server owns business truth"** (its self-declared most important rule) | **MATCHED — the strongest match in the repo.** Server-side recomputation, immutable snapshots, anti-probe reads, the handoff gate. This rule is why the deep review found one contained bug (S1) rather than systemic money corruption. |
| **Modular monolith, real module boundaries** | **MATCHED, exceeded** — the founder's crates-only rule is stricter than the blueprint's modules; it made ~170 integration tests and the deep-review fixes possible without a microservices tax. |
| **Canonical data model** | **PARTIAL, deliberately reshaped** — the core is there (accounts + memberships instead of a role enum: a *better* mechanism; store_products per-store pricing: a refinement the blueprint lacked; order_groups → store_orders → items; deliveries; allocations). Missing: order-status-events, favorites, reviews, promotions, notifications, audit — all later-phase chapters. |
| **12+8-state order machine** | **DIVERGED-ON-PURPOSE, aged well** — V1's 6-status machine is the collapsed view Ch 10.3 anticipated; the *mechanism* the blueprint actually cared about (explicit, tested transition service) exists. |
| **Payments (MoMo-first)** | **DIVERGED (out of scope here)** — COD shipped; the allocations ledger means MoMo is an adapter, not a migration. |
| **Dispatch engine** (Ch 14) | **MISSING — the biggest untouched blueprint idea.** Handoff is manual by rider number. Its "defensibility" thesis (§5.4) rides on this. |
| **Realtime (Redis + WebSocket)** | **DIVERGED-ON-PURPOSE, aged well** — 5s polling with `since`→204, truth ladder, polling decay. Ops-free, battery-aware, honest; the canonical-state contract survives, so WS graduation later touches transport, not truth. |
| **API philosophy (`/api/v1` by capability, OpenAPI as contract)** | **MATCHED nearly one-for-one.** |
| **Design system (emerald + Poppins)** | **DIVERGED-ON-PURPOSE** — the palette was superseded by the founder's own brand the same week; the *discipline* (tokens in one place, "motion explains state", layout-stable animation) got stronger, not weaker. |
| **Auth & security** | **PARTIAL** — strong core (Argon2 + timing equalization, refresh cookies, CSRF, capabilities); thin edges (no rate limiting, no audit trail, refresh tokens never rotate — S9). |
| **Observability** | **PARTIAL** — structured JSON logs + trace/error layers; no metrics/SLOs/backups. |
| **Testing ("a feature is not complete because the screen exists")** | **MATCHED, exceeded** — ~170 integration tests + domain units. **But CI was promised in the blueprint's V0 and never built** — the one foundation actually missing. |

### The journeys

- **Customer: browse → cart → checkout → live tracking — COMPLETE.** Every beat real, including the parts that were hard (per-store sections, per-rider maps, closeness line, cancel-while-reversible, contact sheets, reorder). Missing beats: rating after delivery, and any notification when the rider is near.
- **Merchant ops — PARTIAL.** The operational loop is complete (board → accept → prepare → handoff by number → cash recorded via allocations). The hole that will fail first in a real pilot: **no new-order notification — a restaurant won't watch a board.**
- **Rider ops — COMPLETE for the loop, thin for economics.** Kiosk, stages, navigate, tel:, GPS push, cash confirm, history, tally. Missing: earnings ledger beyond the tally, and the offer/accept flow (riders are assigned, never asked).
- **Admin ops — PARTIAL BY DESIGN.** Network administration is real (merchants/riders/customers CRUD + summary). Order control, audit log, and fees were removed by founder decree — justified for V1, but it means **no server-side record of sensitive actions exists anywhere**; that blueprint safety net deserves to come back before real staff or real money.

### Where "today" sits

Between **V1.5 and V2** on the blueprint's own timeline. V0 done except CI. V1 done except payments (COD variant) and notifications. V1.5 (merchant ops) ~70% real — finance/analytics screens are demo rows awaiting endpoints. **V2's customer-facing half (real tracking) was pulled forward and is DONE**, while its automation half (dispatch, offers, earnings) is untouched. Honored restraint: nearly everything on the "do NOT build early" list stayed unbuilt — with one exception:

### The one expensive divergence

**Multi-store checkout, built now instead of later** (the 2026-08-29 re-architecture superseding Ch 33's "one cart → one merchant"). It forced the genuinely superior model (store = fulfillment boundary, per-store price/stock, explicit allocation money, per-store tracking) — and it multiplied complexity everywhere: one delivery per store, two riders on one purchase, the founder having to simplify the presentation. It's the one place we built ahead of the blueprint's phasing and paid a complexity tax; the ledger design is what kept it from rotting. Verdict: **mostly aged well.**

### Five blueprint ideas still worth building next (ranked, payments excluded)

1. **Order event timeline** (Ch 10.4) — `order_status_events` written by every transition. Cheapest high-value item in the blueprint: timestamped customer timelines, merchant history, future support. Purely additive.
2. **Merchant new-order notification** (Ch 17, minimal form) — the first beat that fails in a real pilot.
3. **Dispatch v0 — rider suggestion at handoff** (Ch 14) — show available riders ranked by distance when the merchant hands off; the blueprint's defensibility thesis, without an engine.
4. **Merchant earnings from the existing ledger** (Ch 8.7 / G4) — balance + history are read-only endpoints over money that's already recorded.
5. **CI + observability basics** (Ch 39 V0 / Ch 29) — the one promised foundation that never landed; a GitHub Actions workflow over the existing `.sqlx` offline build protects everything above.

---

## Part 4 — The fix list, in priority order

**Before calling V1 done (2 days of work, all small):**
1. S1 — cancel restores stock + refunds the allocation (server, one tx + tests).
2. S2/S12 — compare-and-swap on all status/handoff/delivered UPDATEs (server).
3. A5 — cart quantity actually applied (app, 2 lines + test).
4. A2 — boot keeps the token on network errors (app, ~5 lines).
5. A1 — real avatar initials (app, 1 line).
6. A3 — name screen scroll-safety (app, same pattern as siblings).
7. A4 — `/success` redirect guard (app, 1 line).
8. S4 — atomic OTP attempt cap (server, 1 query).

**Before real merchants (the pilot batch):** S3 rotate the Directions key · S5 ETA bump visible · S6/S7/S10 input validation + caps · A6/A8/A9/A10 app hardening · merchant new-order notification · CI.

**Before real money/staff:** S8/S9 pruning + rotation · the audit-trail decision revisited · observability basics.

**Cleanup whenever:** the dead-code lists (server LOW 13, app LOW §dead) · A11's snackbar helper · OpenAPI health-path drift.

---

*Three closing notes: (1) the reviews verified the test suites exist and are behavioral, but the server reviewer couldn't execute them in its sandbox — the founder's own serial runs remain the gate; (2) `tuma-platform` was reviewed structurally (wings, gaps, generated client) but not line-by-line this pass — its BACKEND-GAPS.md is accurate and current; (3) everything in Part 1/Part 2 is fixable without schema changes except S1's allocation `refunded` write, which uses the existing enum value — no migration needed.*
