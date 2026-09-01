# BACKEND GAPS — the platform UI's awaited endpoints

The Phase-2 dashboard redesign (see `redesigns/dashboard_redesign.html` and
`DESIGN PRINCIPLES.md` at the repo root) ships every screen of the coverage
mandate. Where the server has no endpoint yet, the screen is still built —
wired end-to-end on the client with clearly-marked **demo seed rows**
(`src/features/demo/seed.ts`) and its mutations held in client state — so the
founder can review the complete flow today. **Nothing on those screens is
server truth.** This file catalogs every gap with the exact contract the
server needs, so each one is a normal, small, reviewed slice later.

Rule for the swap: when an endpoint below lands, the page replaces its demo
hook with the generated query (`./run.sh openapi && pnpm generate:api`), the
`BACKEND GAP` comment and the demo rows are deleted, and the seed file
shrinks. No UI changes should be needed — that was the point.

---

## G1 · ~~Admin orders feed~~ — RETIRED (founder decision, 2026-09-01)

**Admins never see orders.** Merchants own the order lifecycle end to end;
the admin wing manages the network (merchants, riders, customers) and the
platform. The admin Orders page, its route, the G1/G2 demo rows, and the
Overview's orders KPI were removed. Refund surfaces through disputes (G3).
Customer order history lives in the customer's own app, not the admin wing.

## G2 · Refunds — folded into G3

Refunds exist only as dispute outcomes (`POST /v1/admin/disputes/{id}/resolve`
below). There is no admin orders surface to hang a standalone refund
endpoint on anymore.

## G3 · Disputes & refunds queue

**Needed:**
- `GET /v1/admin/disputes?status=open|resolved` → dispute rows
- `GET /v1/admin/disputes/{id}` → full trail (events + messages)
- `POST /v1/admin/disputes/{id}/resolve` with
  `{ outcome: 'refund' | 'partial' | 'reject', amount?, reason }`

Used by: **Admin → Disputes & Refunds** queue, trail drawer, and the
Resolve page (a page per P18). Resolutions must land in the audit log (G6).

## G4 · Payouts (admin approval + merchant earnings)

**Needed:**
- `GET /v1/admin/payouts?status=pending` → approval queue
  ("Needs attention" on the admin Overview; approve CTA carries the amount)
- `POST /v1/admin/payouts/{id}/approve` → processing → paid
- `GET /v1/merchant/earnings/balance` → `{ available, held_for_open_orders }`
- `GET /v1/merchant/payouts` → payout history (failed row needs
  `failure_reason` for the Retry affordance)
- `POST /v1/merchant/payouts` (request) · `POST /v1/merchant/payouts/{id}/retry`

Used by: Admin Overview needs-attention, Merchant → Earnings & Payouts
(hero balance, request guard, payout table, failed-retry).

## G5 · Platform fees & categories

**Needed:**
- `GET /v1/admin/platform-settings` → `{ platform_fee_pct, categories[]:
  { name, product_count, delivery_fee_default }, last_edited_by,
  last_edited_at }`
- `PUT /v1/admin/platform-settings` (dirty-save writes)

Used by: **Admin → Fees & Categories**. The page renders real structure
with Save honestly refusing until this lands — never fake persistence.

## G6 · Audit log

**Needed:** `GET /v1/admin/audit?type=&actor=&from=&to=&limit=&offset=` →
append-only rows `{ actor, action, target_type, target_id, at }`. No write
endpoint — it's a record (P1: no row actions). Refunds, suspensions,
payout approvals, and settings writes should append here server-side.

## G7 · Reviews

**Needed:**
- `GET /v1/merchant/reviews?rating=` → `{ order_number, customer, rating,
  comment, replied_at?, reply? , at }`
- `POST /v1/merchant/reviews/{id}/reply` with `{ text }`

Used by: **Merchant → Reviews** (KPIs, list, reply drawer). There is no
reviews feature anywhere in V1 yet — the mobile app doesn't send them
either; this is a product milestone, not just an endpoint.

## G8 · Merchant onboarding states

The server's merchant lifecycle is exactly `active | suspended`
(`02_marketplace.sql`). The constitution's "pending review (amber) →
active (teal) → suspended (red)" needs a third state
(`pending_review`) on `marketplace.merchant_status`, set at
`POST /v1/admin/merchants`, with `PATCH .../status` transitions.
Until then the admin Overview's "1 merchant pending review" row and the
Merchants page's amber state are demo-only.

## G9 · Metrics (both wings' KPIs + charts)

**Needed:**
- `GET /v1/admin/metrics?range=today|7d|30d` → `{ orders_today, gmv_today,
  avg_delivery_min, cancel_rate, deltas_vs, revenue_series[] }`
- `GET /v1/merchant/metrics?range=…` → `{ orders_today, revenue_today,
  items_sold, avg_prep_min, deltas_vs, orders_series[] }`

Used by: both Overviews. Today the admin cards show the real summary
counts, the merchant cards show live in-progress counts, and both charts
render their designed "No data for this range" state with demo series
clearly marked. Deltas/sparklines appear only when the endpoint supplies
baselines — the UI never invents a percentage.

## G10 · Rider operational stats

**Needed:** `GET /v1/admin/riders/stats` → per rider:
`{ deliveries_today, on_time_pct, on_delivery }`
(maybe `GET /v1/admin/riders/{id}/current-delivery` for the drawer).

Used by: **Admin → Riders** (Deliveries today, On-time %, "On delivery"
status) and the rider drawer's current-order section. `is_active` alone
can't tell "on a delivery right now" from "waiting".

## G11 · Merchant members management

`GET /v1/admin/merchants/{id}` already returns `members[]`. Missing:
- `POST /v1/admin/merchants/{id}/members` (invite by email)
- `POST /v1/admin/merchants/{id}/members/{user_id}/resend`
- `DELETE /v1/admin/merchants/{id}/members/{user_id}` (guarded in the UI)

## G12 · Global search

`/v1/search` covers products + stores only. The ⌘K palette searches the
client-cached lists (merchants, riders, customers, own stores, merchant
orders) — fine at V1 scale. A real
`GET /v1/search?q=&kinds=orders,merchants,riders,customers` (name / ID /
phone) is the honest fix once volume grows; the palette's item-shaping
already matches.

## G13 · Notifications

`GET /v1/notifications` (+ `POST /v1/notifications/read-all`, deep-link
targets). The bell currently opens the panel's designed empty state
("You're all caught up") and never fakes an unread dot (P2).

## G14 · Merchant order cancellation reason

The merchant board's cancel guard collects a reason, but
`PATCH /v1/merchant/store-orders/{id}` accepts only `{ status }` — the
customer cancel route's `reason` field has no merchant counterpart. Either
accept `{ status, reason? }` on advance, or route merchant cancels through
a dedicated endpoint that records the reason (audit trail wants it too).

---

## Known vocabulary mapping (not a gap, recorded for reviewers)

The board presents four lifecycle columns over the server's six locked
statuses: To accept = `placed` · Preparing = `accepted` · Ready for
hand-off = `preparing` · Handed off = `picked_up`; `delivered`/`cancelled`
live in History. Handoff stays the only door into `picked_up` (rider
number typed on the drawer — no directory picker), exactly as
`commerce/src/orders.rs` enforces.

## Demo rows (features/demo/seed.ts)

| Constant | Backs | Gap |
|---|---|---|
| `demoAdminOrders` | Admin Orders + drawer | G1/G2 |
| `demoDisputes` | Disputes queue/resolve | G3 |
| `demoAdminPayouts`, `demoMerchantPayouts` | Needs attention, Earnings | G4 |
| `demoFeeSettings` | Fees & Categories | G5 |
| `demoAudit` | Audit log | G6 |
| `demoReviews` | Reviews | G7 |
| `demoPendingMerchant` | Overview onboarding row | G8 |
| `demoRevenue7d`, `demoMerchantOrders7d` | Overview charts | G9 |
| `demoRiderStats` | Riders table/drawer | G10 |
