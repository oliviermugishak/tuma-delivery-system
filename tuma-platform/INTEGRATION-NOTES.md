# INTEGRATION NOTES — server surface ↔ platform UI (2026-09-01)

Read-only audit of `tuma-server/src/routes/*` against the platform UI, per
the founder's end-to-end directive: "you will not touch the backend code
but you will read it to rewrite up every functionality with the platform."
Vision reference: `tuma-docs/Tuma_Master_Product_Design_Engineering_Blueprint.md`
(§8 merchant system, §21 admin).

## The complete server surface, and where the UI stands

### Admin wing (`require_role(Admin)`)

| Endpoint | Server truth | UI status |
|---|---|---|
| `POST /v1/admin/merchants` | Provisions business + owner account + owner membership in ONE transaction (name, business_email?, owner email+password) | Wired — Add merchant page |
| `GET /v1/admin/merchants` | All businesses, oldest first | Wired — Merchants table |
| `GET /v1/admin/merchants/{id}` | Business + stores (per-store `product_count`, delivery_fee, is_open) + members (email, role, store_id) — menu deliberately excluded ("admin sees facts; merchant manages the menu") | Wired — detail page + drawer |
| `PATCH /v1/admin/merchants/{id}` | Optional name / business_email / business_phone (E.164) / **status `active`\|`suspended`**; empty string clears contacts; suspension blocks the merchant wing | **Was partially wired** — suspend existed, activation missing → now full kebab set (Activate instant, Suspend guarded, Edit page, Delete guarded) |
| `DELETE /v1/admin/merchants/{id}` | Hard delete; memberships/stores/assortments cascade; member ACCOUNTS survive (identities ≠ business parts) | Wired — guarded dialog |
| `GET /v1/admin/summary` | Counts: merchants, customers, stores, open_stores, products, store_products, orders_in_progress | Wired — Overview KPIs (orders count intentionally unused: admins don't see orders) |
| `GET /v1/admin/customers` | Customer accounts oldest first | Wired |
| `PATCH /v1/admin/customers/{id}` | Optional **name** (empty clears), **phone** (E.164, 409 taken; fixes a typo'd OTP signup), **is_active** (locks out immediately) | **Was is_active-only** → drawer now edits name/phone too |
| `DELETE /v1/admin/customers/{id}` | Hard delete; profile/tokens/order history cascade; customer-scoped (any other identity = 404) | **Was missing** → added, guarded |
| `POST /v1/admin/riders` | OTP account + rider profile + server-generated rider number, one tx; phone = OTP anchor | Wired — Add rider page |
| `GET /v1/admin/riders` | All riders with `rider_number`, `is_active` | Wired |
| `PATCH /v1/admin/riders/{id}` | Optional name / phone (edits OTP anchor together with display phone, 409 taken) / is_active (false = not assignable at handoff, can still sign in) | **Was is_active-only** → drawer now edits name/phone + deactivate/reactivate |
| `DELETE /v1/admin/riders/{id}` | Hard delete | **Was missing** → added, guarded |

### Merchant wing (`require_merchant` + membership grants)

| Endpoint | Server truth | UI status |
|---|---|---|
| `GET /v1/merchant/orders` | Board feed across grants, newest first | Wired — Overview strip + board |
| `GET /v1/merchant/store-orders/{id}` | Fulfillment sheet: items, address+coords, customer name/phone, money, payment_status. **No rider identity on the response** (see findings) | Wired — drawer |
| `PATCH /v1/merchant/store-orders/{id}` | Advance `{status}`; illegal transitions 400; `picked_up` REFUSED (400) — handoff is the only door | Wired — board verbs + cancel |
| `POST /v1/merchant/store-orders/{id}/handoff` | `{rider_number}` — validates an ACTIVE rider, assigns, stamps, caches route → `picked_up`; re-assignable until delivered | Wired — drawer handoff |
| `POST /v1/merchant/stores` | Owner-only, single-business accounts; **new stores start CLOSED** | Wired — New store page |
| `GET/PATCH/DELETE /v1/merchant/stores/{id}` | PATCH = provided overwrites, empty string clears, absent keeps; is_open toggle | Wired — Stores + detail (dirty-save) |
| `POST/DELETE /v1/merchant/stores/{id}/banner` | Multipart ≤5MB | Wired |
| `POST /v1/merchant/products` | **Owner-only** catalog create (name/description/image_url) | Was inline-only → **Catalog page restored** |
| `GET /v1/merchant/products` | Business catalog, gallery cover wins over legacy URL | Was missing → **Catalog page** |
| `PATCH/DELETE /v1/merchant/products/{id}` | Owner-only; delete cascades to every store selling it | Was missing → **Catalog edit + guarded delete** |
| `POST /v1/merchant/products/{id}/images` (+ delete, cover) | Multipart gallery, cover-first ordering | Wired in product edit |
| `POST /v1/merchant/store-products` | Attach product to store: price, stock?, is_available, sku; **409 if already attached** | Wired — Add-product flow |
| `GET /v1/merchant/store-products` | The assortment across reachable stores (store-scoped managers see only their store) | Wired — Menu table |
| `PATCH /v1/merchant/store-products/{id}` | Price, **stock tri-state** (absent=keep, null=untracked, number=set), is_available, sku | Price+availability were wired → **stock/sku now in edit dialog** |
| `DELETE /v1/merchant/store-products/{id}` | Detach from store; catalog identity survives | Wired — guarded |

### Shared
`GET /v1/me` + `PATCH /v1/me` (name editable only for customer/admin
profiles — merchant operators have no editable name, server 400s),
`POST /v1/auth/password`, `GET /v1/search` (products+stores), files.
All wired (Settings, login, ⌘K).

## Findings (read-only; each needs a founder-approved slice)

1. **Rider identity is missing from the merchant fulfillment sheet.**
   `MerchantStoreOrderDetailResponse` carries no rider name/number, so the
   UI cannot show "assigned to Billie Jean (#2)" after handoff — the exact
   re-assignment blind spot the founder flagged. The delivery row has
   `rider_id`; the fix is one additive JOIN + two optional fields on the
   detail response (`rider_name`, `rider_number`, null until picked_up).
   The UI drawer is already shaped to receive them.
2. **No list endpoint for a single order's delivery state** — re-assign
   UX ("Hand off again" while `picked_up`) works but has nothing to show
   the current rider. Same fix as (1).
3. **Merchant order cancel takes no reason.** The customer cancel route
   accepts `reason`; the merchant advance accepts `{status}` only. The UI
   collects a reason it cannot send (noted in the dialog).
4. **Catalog vs assortment is a real permission boundary**: only the
   business OWNER manages the catalog (403 otherwise); store-scoped
   managers manage their store's assortment. The nav therefore splits
   Catalog (owner) from Menu (per-store selling).
5. **Attach is 409-guarded** — attaching the same product to the same
   store twice is a typed conflict; the Add-product flow filters stores
   accordingly.
6. **New stores start closed** — the New store page should say so (the
   consequence line lives on store detail's open toggle).
7. **Empty-string-clears PATCH semantics** on merchant/customer/store text
   fields — the edit surfaces must send `""` to clear, not omit.
8. **E.164 everywhere** — every phone (rider, customer, business) must
   carry the country code; typed 409 on duplicates.
9. **Merchant operators have no editable profile name** (`PATCH /v1/me`
   400s) — Settings says so honestly.
10. **Stock is a tri-state** on PATCH; the UI edit dialog offers
    "untracked" explicitly rather than treating 0 as untracked (0 = sold
    out, null = made to order).

## Seed policy (founder, 2026-09-01)

`features/demo/seed.ts` stays — it visualizes the BACKEND-GAPS.md gaps —
but stale features were removed: the admin payouts rows and the
pending-review merchant (no such server state) are gone from the admin
Overview; retained seeds back only the gap-shower pages (Disputes G3,
Earnings G4, Fees G5, Audit G6, Reviews G7, charts G9, rider stats G10).
