# Tuma — Delivery Tracking Architecture

**Status: FINAL — founder-approved fused architecture (2026-08-30). Supersedes
the earlier handoff-token design and every intermediate proposal.** This is
the design authority for build order #4. Companion docs: `Tuma_V1_Brief.md`
(working source of truth), `Tuma_API_Architecture.md`, `MEMORY.md`.

The system in one sentence: **three truth-gated worlds (timeline → map →
settled), a four-rung honesty ladder, Google Maps everywhere with zero keys
in clients, Tuma-owned riders assigned by their unique rider number, and
every animation either celebrating a real event or expressing real
uncertainty — never simulating motion.**

---

## 0. The wall principle

**Animation must encode truth, never manufacture it.** Every animation does
exactly one of three things:

1. **Celebrates a real event that just happened** (a status the poll just
   caught: connector fills gold, check pops — one-time, ~400ms).
2. **Communicates freshness** (the gentle pulse on the current step + the
   Live badge — the screen is watching something actually moving).
3. **Presents real data smoothly** (ETA decaying against a server-provided
   target timestamp; the marker gliding along the real segment between the
   last two GPS points; camera easing to keep both in frame).

Uncertainty is expressed with **pulse / fade / label** — never with motion
beyond real data. "We don't know" has an animation vocabulary, and it is
never a physics engine. (The anti-fake rule: same sin as the placeholder
photos and placeholder distances we already deleted.)

**Marker interpolation** — the efficiency/UX trick this system adopts:
poll and push at ~5s, but animate the marker along the real segment
between the last two points over the interval. The map feels continuously
live with zero extra network calls. The moto only ever moves between two
real positions.

---

## 1. The screen grammar — three worlds

The customer order detail (and each per-store delivery card in a
multi-store group) is hard-gated on real status:

| World | Statuses | What leads |
|---|---|---|
| **Waiting** | `placed → accepted → preparing` | **The animated timeline is the experience** — nothing is on a road yet. Event animations on each poll-caught transition; gentle pulse on the current step. |
| **Moving** | `picked_up` ("Out for delivery") | **The map takes over**: real rider marker on a real road polyline (interpolated between real points), camera follow, ETA decaying. The timeline collapses to a slim progress strip. |
| **Settled** | `delivered` / `cancelled` | **The map folds away.** Delivered: settled confirmation, timeline returns completed and quiet. Cancelled: the red terminal state. |

Rules:

- **Multi-store groups:** one delivery per store order (the schema is
  already this) — each is its own card with its own story. Tapping an
  in-flight delivery opens that delivery's map. The group chip animates
  only on derived-group-state *changes*.
- **Accessibility floor:** every animation has its text state beside it;
  reduce-motion swaps transitions for instant snaps.
- Continuous-animation budget: one pulse (current step) + one badge (Live).
  Nothing else breathes.

## 2. The truth ladder — liveness, polling decay, expiry

Real failure mode: rider's phone dies, nobody marks delivered, the group
sits in-flight forever. The system tells the truth about the *pipeline*:

| Rung | Condition | Badge | Poll cadence |
|---|---|---|---|
| **Live** | fresh check-in | green **Live** | ~5s |
| **Lagging** | overdue check-in, or past ETA | amber, "last signal — N min ago", reconnect pulse | 15s past ETA → 60s past ETA+1h |
| **Ended** | ~24h past ETA, still not delivered | grey, "contact the store" action | **stopped** — re-fetch on pull-to-refresh / foreground |

- Past ETA + grace (~15 min): the ETA line becomes **"Taking longer than
  expected"** + **call the store** (`stores.contact_phone`).
- **Hard rule: expiry changes presentation and polling, never data.** No
  auto-`delivered`, ever. Statuses move only by real actors.
- Polling is the V1 transport; the tracking poll answers **204 when nothing
  changed** (client sends its last timestamp — battery + payload win). No
  WebSocket until a real server-event system exists (MoMo webhooks era).

## 3. Fallbacks — the founder's rule, made mechanical

- **The pin is the expected path but never a blocker.** Checkout presents
  the map picker prominently ("Use my location" first, tap-to-adjust,
  seeded from the persisted pin) and asks for it; an order CAN be placed
  without one. (Founder: "pin required, but optional.")
- **No pin → no map. Honest text:** "Your delivery is on the way" + status.
  Never a map pointing at a guessed geocode of the street string.
- **Delivering with a pin but no rider position yet:** static real route
  (store → customer pin) + "You'll see the rider the moment their phone
  checks in." No marker.
- **Stale during delivery:** the last **real** point stays pinned + "last
  signal — N min ago"; reconnecting pulse. The cached polyline underneath
  remains — real geography, not a claim about the rider.

## 4. Maps & routing — Google everywhere

**Decision (founder, final): real Google Maps on mobile AND platform.**
OpenStreetMap/flutter_map is retired from the product surfaces.

| Surface | Technology |
|---|---|
| Customer app (Android/iOS) | **`google_maps_flutter`** — rider marker (interpolated), road polyline, camera follow (user pan pauses follow until re-center). |
| Platform (merchant/admin) | **Google Maps JS SDK** — the fulfillment-sheet destination pin (where the order goes — **not tracking**). |
| Rider mode (in the app) | Same `google_maps_flutter` — destination, route, own position. |
| Road routes / ETA | **Google Directions, called by our server** — key never in any client. Behind the `RoutingProvider` adapter (OSRM = one-file swap if ever needed). |
| Rider turn-by-turn | **Deep-link into the installed Google Maps app** — Google's real strength, no SDK cost. |

- **Route caching:** fetched once at handoff (the destination never moves
  mid-delivery), stored on the delivery; **re-fetched only when the rider
  strays ≥~200m off the remaining polyline.** Polls replay the cache.
- **ETA = a server-provided `eta_target` timestamp** (now + Directions
  duration, recomputed on stray). The client decays against the absolute
  time — smooth without lying.
- **Keys — three, one Cloud project, all restricted:** Android (SHA-1
  fingerprint), Web (domains), Directions (server-only, in server
  config/secrets). **Timing: keys are configured when we reach the map
  surfaces** — the plumbing slice (schema, endpoints, simulator) needs none,
  and dev runs the simulator + placeholder until then. (Founder: "when we
  get there we will look at it.")
- **Linux dev caveat (honest):** `google_maps_flutter` has no Linux target.
  Desktop shows a real-data placeholder for map panels (timeline, statuses,
  ladder all work); maps are verified on a phone. The pin picker on desktop
  dev accepts paste-in coordinates (a Google Maps share-link), which is
  genuinely useful on phones too.

## 5. Riders — Tuma-owned, assigned by rider number

**Decision (founder, final): Tuma owns the drivers — that is the platform's
whole purpose. Merchants do not manage staff.** Deliberately kept light:

- **`riders` are platform entities.** A rider = an account (phone + OTP
  sign-in, the exact customer flow) + a rider profile with a **unique
  rider number** (a short numeric code the rider can memorize/share).
- **Admin creates riders** (minimal tooling — an admin endpoint now, a thin
  page later; the founder: "not something to stress about deeply").
- **Merchants assign by rider number:** the fulfillment sheet's "Handed to
  rider" action = **enter the rider's unique number** → server validates an
  active rider → sets `deliveries.rider_id` + advances to `picked_up` →
  route cached at handoff. No directory picker, no merchant staff concept.
- **Rider mode** — role-derived inside the existing app: a rider signs in
  with OTP and lands on a kiosk screen. Nothing else:
  - **Active delivery card** — store name/address, destination pin + cached
    route on Google Maps, **customer name + `tel:` link** ("call when
    you're close" is the Kigali protocol — the data already flows),
    deep-link button into Google Maps navigation.
  - **Start delivering** — pushes real GPS every ~5s to the location
    endpoint, screen-on on the bike mount, no background service (honest
    battery note on the screen).
  - **Delivered** — rider received the cash → advances the status. The
    merchant keeps the same power from the fulfillment sheet; both are
    real actors.
- One rider can carry **multiple deliveries on one run** — the account
  model handles naturally what a per-delivery token could not.
- The rider's first name may surface on the customer's tracking screen
  ("Jean is on the way") — identity without exposure; the customer's own
  phone number is what the rider calls.
- **Re-assignment:** a wrong rider number at handoff isn't a support
  ticket — the handoff action simply runs again while the delivery isn't
  delivered (sets `rider_id` again); after `delivered` the assignment is
  frozen.
- **Rider auth mechanics** (the profile-row pattern, same as customers and
  admins): the rider is an account + a `commerce.riders` profile row; the
  JWT carries only the account id; a rider guard resolves the profile
  fresh per request; the push/delivered endpoints verify **ownership** —
  the `rider_id` on the delivery must match, and another rider's delivery
  is indistinguishable from a missing one (404).

### The cash state — money moves when the customer pays

**Founder rule: money is not a thing to prototype over.** The only mechanic
is the one reality already forces: in cash-on-delivery, the moment of
delivery IS the moment of payment. The rider's **Delivered** action = food
handed over + cash received — one real event, and the ledger follows as a
consequence (never a separate collection action):

- that delivery's `payment_allocation` moves `pending → settled` (the
  store's slice: its subtotal + its delivery fee);
- the group's `payment_status` becomes `collected` when **every**
  allocation in the group is settled;
- nothing else in the money ledger moves. Refunds, MoMo, change-making,
  who-collects-what edge cases — all explicitly deferred to the real
  payment system, where they will be decided properly. (The multi-store
  checkout copy gets aligned to this rule when the slice builds.)

## 6. Contact channels — both directions

- **`stores.contact_phone`** (migration 07) + surfaced on the store and in
  the tracking overdue state — the customer can call the store.
- **The customer's phone already flows** (checkout → order group →
  fulfillment sheet) — the rider mode adds the `tel:` link so the rider can
  call the customer. A user might need to call back; the merchant too.
- No in-app chat until something measurably hurts.

## 7. Data model (migration `07_delivery_tracking.sql`, additive)

- **`commerce.riders`** — `id UUID PK`, `rider_number` (unique, short
  numeric code), `account_id` (unique FK → accounts; the OTP account),
  `name`, `phone`, `is_active`, timestamps + trigger.
- **`commerce.delivery_locations`** — **insert-only breadcrumbs**:
  `(id, delivery_id FK, lat, lng, recorded_at)` + index
  `(delivery_id, recorded_at)`. Inserted **only when the rider moves ≥25m
  or 15s** (GPS-noise killer). Powers the live marker, the **trail** drawn
  behind the rider, and `last_location_at` for the ladder. Old rows pruned.
- **`commerce.deliveries` gains:** `rider_id` (FK, nullable until handoff),
  `handoff_at`, `route_polyline` (cached at handoff), `eta_target`,
  `last_lat`, `last_lng`, `last_location_at`.
- **`marketplace.stores` gains** `contact_phone`.

## 8. Endpoints

| Endpoint | Auth | Notes |
|---|---|---|
| `POST /v1/deliveries/:id/location` | rider (own delivery) | throttled; breadcrumb insert (≥25m/15s); updates last-position columns |
| `POST /v1/deliveries/:id/delivered` | rider (own delivery) | advances the store order; merchant advance stays equivalent |
| `GET /v1/orders/:id/tracking` | customer | status, last position, trail tail, cached polyline, `eta_target`, freshness; **204 when nothing changed** |
| `POST /v1/merchant/store-orders/:id/handoff` (or PATCH extension) | merchant | body = `rider_number`; validates an active rider, sets `rider_id`, advances to `picked_up`, caches the route |
| Admin rider creation | admin | minimal — create rider (name + phone → account + unique rider number); thin tooling now |

Notes:

- **Anti-probe rule everywhere:** another customer's order (or another
  rider's delivery) is indistinguishable from a missing one — 404.

## 9. Dev simulator

A server bin that walks a **real Google route at moto speed** through the
same `POST /deliveries/:id/location` endpoint (or simulates a rider
account). The founder watches a delivery move on his phone before any real
rider exists; the production path is byte-identical. Simulation lives only
in the bin — never in app code.

## 10. Deliberately NOT building (named, not forgotten)

- **Fake moto animations** beyond real-point interpolation (no motion
  beyond real data, ever).
- WebSocket/streaming push (polling + 204 is honest and simple; push comes
  with MoMo webhooks).
- Rider chat, proof-of-delivery photos, multi-rider batching/dispatch
  optimization.
- Background GPS service (screen-on while actively delivering only).
- Merchant rider-staff management, deep rider ops tooling (assign-by-number
  is the whole interface).
- **Merchant live tracking** — the tracking system is for the CUSTOMER. The
  merchant's job ends at handoff (assign by rider number → `picked_up`);
  operational visibility lives on the rider's screen.
- Map licensing beyond the free tier (money goes to MoMo first).

## 11. Now vs Then

| | Now (V1 — this build) | Then (post-V1 graduations) |
|---|---|---|
| Rider identity | Tuma-owned accounts, OTP sign-in, unique rider numbers | Same core; richer rider ops tooling as operations demand |
| Assignment | Merchant enters the rider number at handoff | Dispatch/batching if volumes demand |
| Transport | ~5s polling with 204-unchanged | Server-event push (MoMo webhooks era) |
| Routes | Google Directions via server adapter | Same; OSRM swap available behind the adapter |
| Map | Google Maps SDK on every surface | Same |
| Expiry | Presentational ladder only | Ops tooling to chase stuck deliveries |
| Battery | Screen-on push while actively delivering | Background service if operations demand |
| Contact | `stores.contact_phone` + customer `tel:` links | In-app messaging if ever earned |

## 12. Decision log within this document

- **Animation encodes truth** — wall principle. Uncertainty = pulse/fade/
  label; motion only between real points. (Anti-fake directive applied.)
- **Marker interpolation between real points** — adopted from the fused
  review; feels live at 5s pushes, zero extra calls.
- **Three truth-gated worlds** — map only while truly delivering. (Founder
  directive.)
- **The honesty ladder** — Live/Lagging/Ended + polling decay +
  presentational expiry; never auto-delivered.
- **Google Maps everywhere + server-owned Directions key + deep-link
  navigation** — founder directive, final. Keys configured when we reach
  the map surfaces.
- **Riders are TUMA's** (founder: "we own the drivers, that is its whole
  purpose") — platform entities with unique rider numbers; merchants assign
  by number; no merchant-staff concept; management kept deliberately light.
  This supersedes BOTH earlier proposals (the handoff-token page AND the
  admin-managed rider loop with a picker).
- **Pin expected-but-not-blocking; missing pin → "on the way" text** —
  founder call on the required/optional tension.
- **Contact both ways** — `stores.contact_phone` + customer `tel:` on the
  rider screen (founder: "a user might need to call back, so as the
  customer might be called back too").
- **Breadcrumbs, route cache at handoff, stray-refetch, ETA target
  timestamps, 204-unchanged** — plumbing decisions; thresholds (25m/15s,
  200m, 15-min grace, 24h stop) are founder-tunable defaults.
- **The cash state (founder):** money moves when the customer pays —
  `delivered` (cash in hand) settles that delivery's allocation; the group
  becomes `collected` when all are settled; everything deeper is deferred
  to the real payment system.
- **Completeness audit (founder-requested):** rider auth mechanics,
  re-assignment rule, the anti-probe 404 rule, and the cash state folded
  into §5/§8 — every decision made anywhere is now written down here.
- **No merchant tracking (founder):** the tracking system is for the
  CUSTOMER; the merchant's job ends at handoff (assign by rider number).
  The earlier "merchant live view" is removed.

## 13. Slice shape (build order #4)

1. **D1 — Schema + riders:** migration 07; `riders` domain (accounts +
   rider number); admin rider creation (minimal); OTP role-routing to
   rider mode. No UI beyond stubs.
2. **D2 — Handoff + tracking plumbing:** merchant assign-by-rider-number +
   `picked_up`; location push + `delivered` endpoints (rider auth);
   tracking endpoint with 204; Directions adapter (config ready); dev
   simulator. Watchable via the simulator.
3. **D3 — Customer map world:** the Google map world in the order detail,
   interpolated marker, ETA decay, fallback ladder. **The moment a moto
   moves on the founder's phone.**
4. **D4 — Rider mode:** kiosk screen (card, Start/Delivered, `tel:` link,
   deep-link nav) + merchant fulfillment handoff UI.
5. **D5 — Liveness + polish:** badge ladder, polling decay, overdue
   "call the store", settled states, animations.
