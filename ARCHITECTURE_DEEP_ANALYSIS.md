# Tuma — Architecture Deep Analysis

**A working document of questions, answers, findings, and ambiguities.**
Every section states: *current reality* → *the answer/design* → *what lands
when*. It is the recall file for architectural decisions made in conversation
and the punch-list of gaps between what the backend can do and what the UIs
actually surface.

**Sources:** the 2026-08-29 marketplace re-architecture (see `MEMORY.md`),
the founder's questions from that session, and `STORAGE_GUIDANCE.md`
(integrated in §3).

**Status legend:** ✅ real today · 🟡 designed, not built · 🔴 known bug/gap ·
⏸️ deliberately deferred (named, not forgotten)

---

## 1. Delivery tracking, distances, and the "Temporary deviation"

**The question.** The API doc carries a deviation warning: the customer
app's store cards show distances and arrival times that are *fake*
(deterministic random numbers from the store name). Should stores share
real, verifiable coordinates, and should we compute real distances and
minutes for clients according to their location — which means the app also
needs the customer's location?

**Answer: yes, on every count.** The design:

1. **Stores carry real, map-verifiable coordinates.** ✅ As of the Kigali
   seed, all 7 stores have real lat/lng (e.g. Simba City Center at
   `-1.9499, 30.0622` — drop it into any map app). Going forward the rule
   is: *a store without coordinates is not fully onboarded* — the store
   create/edit UI must capture them (see §7; today it doesn't, which is a
   🔴 gap).
2. **The app asks for the customer's location** (permission-gated GPS fix),
   and sends it with the browse request: `GET /v1/stores?lat=…&lng=…`.
3. **The server computes distance and ETA** — haversine distance (fine at
   Kigali scale; PostGIS only when zone/radius queries become real) and
   `eta_min` = distance ÷ effective moto speed (~25 km/h in Kigali traffic)
   + a preparation buffer. 🟡 Not built yet. The server computes because
   *the server owns truth* — clients render, never calculate business facts.
4. **The placeholder file retires** the day the real fields land; the
   category half of the deviation is already resolved ✅ (stores now carry a
   real server-owned `category`, and the home cards render it).
5. **Build order #4 (tracking)** reuses the same machinery end-to-end: the
   rider's phone pushes real GPS every ~5s to
   `POST /deliveries/:id/location`, the customer polls
   `GET /orders/:id/tracking` (~3s), and the map draws the real marker with
   ETA = *remaining* travel time. Schema is already in place
   (`commerce.deliveries`); nothing about this is simulated in production.

**Open decision (founder):** straight-line haversine distance for ETA in V1
vs. road-distance via a routing provider from day one. Recommendation:
haversine for the browse cards (honest enough for "near you" sorting), road
distance only for the active-delivery ETA in #4 — it needs the routing
provider anyway (§6).

---

## 2. Checkout drop-off: capturing a real delivery location

**The question.** Checkout should pick the user's real location for
delivery, or at least a valid, verifiable one — not just prose.

**Answer.** Agreed; the schema already supports it
(`commerce.order_groups.address_lat / address_lng`, nullable today). The
progression:

1. ✅ **Now:** free-text address only (lat/lng sent as null). Honest but
   weak — a rider would navigate by phone call.
2. 🟡 **Next slice — map-pin picker at checkout:** a flutter_map view,
   drag-the-pin or "use my current location", plus the text field as the
   human-readable label ("Kimironko, past the market, blue gate"). Purely
   additive client work; the API already accepts the coordinates.
3. 🟡 **Saved-addresses book** (the `addresses` table we deliberately
   deferred): label + pin + delivery instructions + a default flag, so
   reordering is one tap. It earns its place the moment checkout has a map.
4. **#4:** the rider's app consumes that exact pin for navigation. Garbage
   coordinates send a real rider to the wrong place — which is why "valid,
   verifiable locations" is the right bar for both stores and customers.

---

## 3. Images, uploads, and object storage (per STORAGE_GUIDANCE.md)

**The question.** Where do images live, and how do uploads work — keeping
in mind `STORAGE_GUIDANCE.md`.

**Answer — synthesized with our actual system** (the guidance document was
written before this codebase was known; two of its assumptions need
adapting, flagged below):

- **Metadata in Postgres, bytes in object storage — never files in the DB.**
  The `image_url` columns that already exist everywhere stay; the pipeline
  around them changes.
- **Use the `object_store` crate behind a thin `FileStore`-style wrapper in
  `AppState`** (the guidance's own preference; matches our
  `Arc<…>`-in-state pattern). Backend selection is config-driven:
  `STORAGE_BACKEND = local | memory | s3` — local disk for dev, **in-memory
  for tests** (hermetic `#[sqlx::test]` runs, no network), S3-compatible
  (Cloudflare R2 first choice) for production. One env var is the whole
  deployment swap; clients only ever see URLs, so zero client changes.
- **Deterministic, collision-safe keys** — never raw filenames:
  `merchants/{merchant_id}/stores/{store_id}/banner.{ext}` and
  `products/{product_id}/images/{position}-{uuid}.{ext}`.
- **Validation at the door:** allow-listed image MIME types, ~5 MB max
  (flagged assumption per the guidance), enforced before the storage write.
- **Schema — one adaptation to the guidance:** it assumed one store per
  merchant and single-image products. Our reality: multi-store merchants,
  and products need galleries. So:
  - store banner: the existing `marketplace.stores.image_url` column holds
    the URL (a logo column is additive later if a design needs one);
  - product gallery: a new `marketplace.product_images` table —
    `id UUID PK`, `product_id FK ON DELETE CASCADE`, `storage_key TEXT NOT
    NULL` (the *key*, not the URL), `position INT`, `is_primary BOOL` with
    a partial unique index (at most one primary per product),
    `created_at`. One numbered migration, matching house style.
- **Handlers** under the merchant namespace, reusing the existing
  capability guards + membership grants exactly (no new auth pattern):
  `POST /v1/merchant/store-products/{id}/images` style + a delete, plus
  store-banner upload. Validate → write to storage → insert row → return
  the resource with a usable URL. **Failure compensation:** if the DB row
  fails after a successful storage write, best-effort delete of the object;
  deletes go DB-row-first, object-second (an orphaned object is sweepable —
  an orphaned URL pointing at nothing is worse), with a cleanup job when
  the volume justifies it.
- **Direct-to-storage uploads via presigned URLs** (client → R2 directly,
  server only validates + records) so images never transit the API server.
  Local/memory backends proxy through the API; the handler boundary hides
  the difference.
- **Testing** per the guidance: in-memory backend unit tests + hermetic
  `#[sqlx::test]` handler tests + a fresh-DB migration check; nothing ever
  touches real R2 credentials.

⏸️ Status: still deferred as a slice, but now fully specified. The seed data
uses real Unsplash CDN URLs as stand-in imagery, which keep working until
real uploads replace them.

---

## 4. ✅ RESOLVED (slice F1, 2026-08-29) — findings and fixes

Every finding in this section was verified in code, then fixed in the same
iteration; the details live in `MEMORY.md`'s decision log. What landed:

- **4.1 Admin dashboard** — the stale "orders land with the orders
  iteration" card is gone, replaced by a live **Orders in progress** count
  (`GET /v1/admin/summary` now carries `orders_in_progress`).
- **4.2 Stranded after checkout** — checkout now `push`es the group detail
  (the shell survives beneath), the detail screen has an explicit back
  button that falls back to `/home`, a Home action, and a "Continue
  shopping" CTA on settled groups. There is no dead end.
- **4.3 Overlapping header** — the `SliverAppBar`/`FlexibleSpaceBar`
  combination was replaced with a plain pinned `AppBar`.
- **4.4 Store location capture** — the create/edit store dialogs now take
  **category + latitude/longitude** with range validation and a
  **"Verify this pin on the map"** link (OpenStreetMap); the store detail
  page shows the coordinates (or warns that they're missing) with the same
  verify link.
- **Bonus (the founder's request): merchants see who ordered** — new
  `GET /v1/merchant/store-orders/{id}` fulfillment sheet: items, totals,
  delivery address + pin link, **customer name + phone** (as a `tel:`
  link), payment status. The board rows open this dialog; advance/reject
  actions live inside it too.

---

## 5. 🔴 The backend↔UI gap audit — what exists but isn't surfaced

Everything below **works on the server today** and has no home in any UI,
plus one gap that runs the other way:

| # | Capability | Server | Platform UI | Mobile UI |
|---|---|---|---|---|
| 1 | ~~Merchant sees WHAT to cook~~ **✅ F1** — `GET /v1/merchant/store-orders/{id}` returns the fulfillment sheet (items + customer contact) | ✅ | ✅ board rows open the sheet | group detail already shows items |
| 2 | ~~Admin advances any store order~~ **✅ REMOVED (founder directive)** — order operations are the merchants' monopoly; the admin route was deleted (404), tested | — | — | — |
| 3 | ~~Payment collection~~ **✅ REMOVED (founder directive)** — allocation rows are created automatically at checkout as passive records; no collection endpoint exists anywhere; when a real money system lands, collection becomes automatic (e.g. MoMo webhook), never a button | — | — | group detail shows payment status |
| 4 | ~~Store lat/lng capture~~ **✅ F1** — in the create/edit dialogs with a verify-on-map link | ✅ | ✅ | — |
| 5 | ~~Store category~~ **✅ F1** — captured in the dialogs, shown on the detail page | ✅ | ✅ | ✅ home cards (when set) |
| 6 | Order-group idempotent retry | ✅ unique key | — | ✅ client sends a retry-safe key |
| 7 | List pagination (`limit`/`offset`) | ✅ orders + merchant board | board renders page 1 only (no "load more") | history fetches page 1 only (no "load more") |
| 8 | Customer cancellation reason | accepted, validated | — | confirm dialog collects nothing (server would store it — no column yet; ⏸️ event-log slice) |
| 9 | Payment ledger (`payment_allocations`) | ✅ passive records, created at checkout (no mutator by design) | ❌ invisible (fine — nothing to reconcile) | amount only |
| 10 | Derived group status | ✅ | ✅ live "Orders in progress" count on the dashboard (read-only visibility) | ✅ group cards + detail |

**Priority from this audit:** gap #1 was the operational blocker — resolved
in F1 with the fulfillment-sheet endpoint. #2/#3 were dissolved by the
founder's merchant-monopoly directive: the admin has no order endpoints,
and collection doesn't exist until a real money system does. #4 is §7.

---

## 6. Order status: who changes what, and the "delivering" question

**The six statuses are a contract between actors, not a UI decoration.**
Ownership today (server-enforced by `OwnershipScope` + transition rules):

| Status | Advanced by | Meaning |
|---|---|---|
| `placed` | *system* (checkout) | money-or-nothing committed; store must act |
| `accepted` | **merchant** | "we took the order" — starts the clock |
| `preparing` | **merchant** | cooking/assembly started |
| `picked_up` | **merchant hands off → rider**. Pre-#4 (riders are name+phone, no accounts): the *merchant* marks it when the courier takes the order. From #4: the **rider app** confirms pickup from the field. | the order has left the premises |
| `delivered` | **rider** confirms at the door (from #4, with proof-of-delivery options). Pre-#4: the merchant marks it. | fulfillment complete; cash has changed hands |
| `cancelled` | **customer** (placed/accepted/preparing only — the server enforces this), or **merchant** rejecting | terminal |

**The admin has no order powers — founder directive, 2026-08-29.** Store
orders are the merchants' monopoly: every advance, cancel, and fulfillment
decision belongs to the business that took the order, scoped by membership
grants server-side. The admin sees a passive `orders_in_progress` count;
that is the extent of platform involvement.

**The missing "delivering" status — answer:** don't add an enum value;
relabel. `picked_up` *is* "out for delivery" — adding a separate
`out_for_delivery` value creates two names for one physical fact and a
second bookkeeping surface, which the brief explicitly forbids until a real
operation forces it. What the clients should do (🟡 small slice): render
`picked_up` as **"Out for delivery"** in customer-facing labels and *color
it as the moving state* — that's the moment the map view activates (§7).
If batching or multi-leg logistics ever genuinely needs "rider holding" vs
"en route" as distinct facts, an `ALTER TYPE … ADD VALUE` is additive and
cheap — but it must be forced by operations, not by vocabulary.

**The delivered→cash tie-in (operational truth, deferred):** cash on
delivery means `delivered` and "money accounted for" are related but
distinct facts. In V1 there is **no collection action anywhere** — the
allocation rows sit as passive records, and that is fine because there is
no money system to reconcile against. When a real one lands (MoMo first),
collection becomes **automatic** (the provider's webhook flips the status —
never a manual button), and if cash stays in the mix, the rider app's
delivery confirmation carries the cash-received flag so the ledger follows
reality without anyone doing bookkeeping by hand.

---

## 7. Map integration and realistic movement (real routes, honest simulation)

**Production tracking (build order #4):**
rider app (foreground service, `flutter_map`) → GPS every ~5s →
`POST /deliveries/:id/location` (authored by the rider identity — the
rider-auth decision named in MEMORY) → customer polls the tracking endpoint
→ canonical server-computed state (position, remaining distance, ETA range)
→ `flutter_map` marker + polyline. The customer's map view **activates when
the order enters `picked_up`** ("Out for delivery") — before that the
detail screen's per-store timeline is the honest view.

**Road routes.** Straight lines lie in Kigali (one-way loops, the
Gishushu curve). So the routing boundary named in the blueprint is real:
a `RoutingProvider` adapter (OSRM's public demo server is the zero-cost
first provider; swap later behind the interface) that turns
store→customer coordinates into a road polyline. Uses:
1. real remaining-distance for the live ETA;
2. drawing the actual route line under the moving marker;
3. the dev simulator below.

**GPS simulation that follows real roads — test harness only.** The
blueprint's rule stands: simulation never ships in production paths. But
for development and demo you need the marker to move *realistically*
without a real rider: a **rider simulator** (dev-only bin or a debug toggle
in the rider app) that:
1. takes an order in `preparing`;
2. asks the RoutingProvider for the store→customer road polyline;
3. walks the polyline at motorcycle speed (~25–35 km/h, slowed at dense
   segments), pushing each point to the same
   `POST /deliveries/:id/location` a real rider uses — no separate
   code path, no fake data in the DB beyond what a real rider would write;
4. completes → `delivered`.

That gives a truthful demo of the whole tracking pipeline (polling, map,
ETA decay) with the only simulation living where the blueprint allows it:
the harness.

---

## 8. Dashboards: reasonably showing what is

**Principle: every number on a dashboard must be live, server-owned, and
actionable — no placeholders, no vanity cards.** With the gap audit (§5)
closed, the surfaces become:

**Admin — platform administration, NOT order operations** (founder
directive: stores and orders are the merchants' monopoly):
- the count cards stay (merchants, customers, stores, open now, catalog,
  assortment) — they are real platform facts;
- a read-only **"Orders in progress"** count — visibility of the
  marketplace's pulse, nothing more; no admin order pages, no actions
  (✅ built in F1);
- suspension/health alerts (suspended businesses, stores closed >24h).

**Merchant — the operating board:**
- "orders needing you" hero (placed orders awaiting acceptance) — the
  single most important number a merchant owns;
- today's numbers that are honest at pilot scale: orders today, revenue
  today (allocated totals), items low on stock (tracked `stock` ≤
  threshold — we have the data);
- the board itself (✅ F1: rows open the fulfillment sheet with items and
  the customer contact);
- keep dashboard cards few: hero action + 3–4 facts + the store grid.

Everything the dashboards show already exists server-side; the remaining
work is surfacing plus the "orders needing you" hero.

---

## 9. Priority order (recommendation)

1. ~~Merchant order items~~ **✅ F1** — fulfillment sheet + board dialog.
2. ~~Mobile wayfinding fixes~~ **✅ F1** — push-after-checkout, explicit
   back/home, plain AppBar header.
3. 🟡 Location capture, phase 2 — checkout gains the delivery-pin picker;
   server browse route gains `lat`/`lng` → server-computed
   `distance_m`/`eta_min`; the placeholder file retires. (Store-side
   capture ✅ F1.)
4. ~~Status relabel~~ **✅ F1** — `picked_up` renders as "Out for delivery"
   on the merchant board; map-view activation gate lands with #4.
5. 🟡 Uploads slice per §3 (object_store + product_images migration +
   presigned R2).
6. #4 — rider mode, real GPS, routing adapter, road-route ETA; the
   simulator (§7) as the harness.
7. Money, when it's real — commission model, MoMo webhook (collection
   becomes automatic), payouts. Not before; the ledger is already shaped
   for it.

Each line is a slice under the house rules: design → approve → build →
verify → record.
