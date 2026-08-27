**TUMA**
**Everything you crave, delivered.**

**MASTER PRODUCT, UX/UI, SYSTEM & ENGINEERING BLUEPRINT**

Customer marketplace • Merchant operating system • Rider network • Dispatch engine • Payment system • Platform control plane

| **This is the source of truth. Engineers and coding agents must read this document before making architectural decisions. It defines what Tuma is, why it is designed this way, what belongs in each version, how every subsystem behaves, and what must not be built too early. This document supersedes all earlier prototype-era documentation.** |
|---|

**Version 2.0 • August 2026 — Production system vision.** This replaces the v1.x prototype blueprint (seed data, Hive-only persistence, simulated delivery, mock payments). The prototype remains useful as a validated UX reference and as a test harness, but the product is now built as a **real delivery network**: real backend, real payments, real merchants, real riders, real GPS, real money settlement.

> **V1 implementation note:** the V1 backend is built in **Rust** (Axum + SQLx + PostgreSQL) per founder decision — the Fastify/TypeScript stack described in this document is superseded for V1, and real delivery tracking (real GPS on a real map) is a V1 feature, not a later phase. Day-to-day work follows **`Tuma_V1_Brief.md`** (one page); this blueprint remains the long-term reference for the full platform. The ideas in this document (server authority, immutable order snapshots, provider-confirmed payments) are language-independent and still apply.

---

# 0. Executive decision

Tuma should **not** be built as a collection of screens that imitates an existing delivery app.

It should be built as a **customer marketplace, merchant operating system, rider network, dispatch engine, payment system, and platform control plane** that happen to surface through different interfaces.

```text
                        TUMA
                          |
         +----------------+----------------+
         |                |                |
     CUSTOMER         MERCHANT          PLATFORM
       APP               WEB              ADMIN
   (Flutter)          (Web app)       (Web console)
         |                |                |
         +----------------+----------------+
                          |
                   DELIVERY PLATFORM
                          |
                        RIDERS
                     (Flutter app)
```

The customer experience is the visible product. The operational system underneath is the defensibility.

## Core strategic thesis

Existing delivery companies can copy colors, cards, promotions, or tracking screens. Tuma's competitive advantage must therefore come from **system quality**:

1. Faster and clearer customer journeys.
2. Better merchant tooling and operational visibility.
3. Better dispatch and delivery reliability.
4. More transparent pricing and order states.
5. Superior local payment and address handling.
6. A delivery network that gets more efficient as volume grows.
7. Data and workflows that make merchants more successful, not merely more dependent on the marketplace.

There is no guarantee any feature set will cause Tuma to win. The goal is to create a product and operating model with a credible path to winning through better execution, density, trust, and merchant/rider economics.

## What changed from the prototype era

| Prototype (v1.x docs) | Production (this document) |
|---|---|
| Seed catalog in `lib/data/seed/` | Real merchants onboarded through admin, catalog served by API |
| Hive as the database of record | Server (PostgreSQL) is the database of record; Hive/cache is a client cache only |
| `TrackingSimulation` Bézier 80-step timer | Real rider GPS → realtime gateway → canonical tracking state |
| 5-state order lifecycle | Full order state machine with payment, rejection, cancellation, refund states |
| Mock cash/card payment | MTN MoMo (first), Airtel Money, card, cash-on-delivery via provider adapters |
| Client-computed totals | Server-authoritative pricing engine with immutable snapshots |
| Solo customer app | Customer + merchant web + rider app + admin console on one platform API |

The prototype's UX language (emerald system, journey-first screens, honest ETA) is retained. Its data and logistics model is retired from production code paths and kept only as a **test harness** (see Ch 37).

---

# 1. The competitive reality

Vuba Vuba currently presents itself as a delivery platform spanning food, retail/essentials, and courier services across Rwanda and Uganda. Its public materials emphasize a broad delivery proposition, merchants, riders, multiple payment methods, and real-time delivery tracking. Its Rwanda service emphasizes seeing the rider's movement and receiving nearby notifications. [Vuba Vuba Rwanda; Vuba Vuba Africa; Vuba Vuba Services]

This means Tuma must **not** define the competitor as a simple food-ordering UI.

The real competitor is:

```text
marketplace + merchant acquisition + rider supply + dispatch
+ payments + customer trust + operations
```

The same is true of global platforms: merchant products increasingly include live orders, menu management, availability, issue resolution, reporting, campaigns, and operational communication. DoorDash, for example, exposes active order management, menu management, out-of-stock handling, store availability, reporting, campaigns, and communication capabilities to merchants. [DoorDash Merchant Business Manager / Merchant Portal]

Therefore the design target is not "make a prettier delivery app." It is:

> **Build the most coherent local delivery operating system we can create, with a customer experience that makes the underlying operational quality visible.**

---

# 2. What Tuma is

## 2.1 One-sentence definition

> Tuma connects customers, merchants, riders, and payment providers into one real-time commerce and last-mile delivery network, starting in Kigali.

## 2.2 What customers buy

Initially:

- restaurant meals
- drinks
- desserts
- selected convenience/essential items

Later:

- groceries
- pharmacy/health products where legally and operationally appropriate
- retail
- parcels
- scheduled deliveries
- business deliveries

## 2.3 What merchants buy

Merchants are not merely "listed on Tuma." They receive:

- customer acquisition
- order intake
- digital menu/storefront
- delivery fulfillment
- real-time order visibility
- promotions
- operational reporting
- customer communication tools
- payout visibility
- store performance analytics

## 2.4 What riders receive

- delivery opportunities
- dispatching
- navigation support
- order/pickup information
- earnings visibility
- incentives
- performance history
- support workflows

## 2.5 What Tuma itself operates

- marketplace rules
- dispatch
- pricing
- delivery zones
- commissions
- payment routing
- refunds
- support
- fraud/risk controls
- merchant onboarding
- rider onboarding
- data/analytics
- platform configuration

## 2.6 Product promise

At every important moment the customer should be able to answer:

```text
What am I buying?
How much will it cost?
When will it arrive?
Where is it now?
What happens next?
```

## 2.7 Brand personality

| Trait | Design implication |
|---|---|
| Fast | Short flows, one dominant CTA per view, decisive motion 200–350ms |
| Warm | Food-forward photography, friendly copy, human rider states |
| Trustworthy | Explicit fees early, full order state machine surfaced honestly, ETA as ranges (`~ 14 min`), proactive delay explanations |
| Modern | Poppins headlines + Inter body, intentional whitespace, restrained surfaces |
| Local | RWF pricing, mobile money first, Kigali addresses/landmarks, multilingual potential |
| Ambitious | Polish closer to a premium global consumer product than a utility form |

---

# 3. Product architecture: five surfaces, one platform

```text
                              TUMA PLATFORM
                                    |
      +-----------------------------+-----------------------------+
      |                             |                             |
CUSTOMER EXPERIENCE         OPERATIONS EXPERIENCE           CONTROL PLANE
      |                             |                             |
Customer app (Flutter)      Merchant web app              Admin web console
Rider app (Flutter)                                         
      |                             |                             |
      +-----------------------------+-----------------------------+
                                    |
                            Platform API / BFF
                                    |
      +--------------+--------------+--------------+--------------+
      |              |              |              |              |
  Commerce      Fulfillment      Payments       Identity      Messaging
      |              |              |              |              |
      +--------------+--------------+--------------+--------------+
                                    |
                        PostgreSQL / PostGIS / Redis
                                    |
                       Events / Jobs / Observability
                                    |
                            External providers
                 (MTN MoMo, Airtel, maps, routing, push)
```

## The critical architectural principle: server authority

The mobile apps and web apps are never the ultimate source of truth for:

- prices
- discounts
- delivery fees
- order totals
- order status
- payment status
- refund status
- merchant availability
- rider assignment
- delivery completion
- earnings

**The clients render state. The server owns business truth.**

This is the single most important rule in this document. Every design and engineering decision must protect it.

---

# 4. Product principles

## 4.1 Customer principles

### Principle 1 — Remove uncertainty

At every important moment the customer knows what they are buying, what it costs, when it arrives, where it is, and what happens next.

### Principle 2 — Make the common path extremely short

A returning customer should be able to reorder in seconds. A previous order is nearly one gesture away from repeating.

### Principle 3 — Show price truth early

Delivery and service fees must not appear as a surprise immediately before payment. Fees are visible on the store page and in the cart, not only at checkout.

### Principle 4 — Progress must be tangible

Orders have clear states and timestamps. Delivery tracking communicates actual progress, not decorative animation.

### Principle 5 — Recovery is part of the product

The product must gracefully handle:

- merchant rejecting an order
- item unavailable
- rider unavailable
- payment failure
- delayed preparation
- delivery delay
- customer unreachable
- cancellation
- refund

Every one of these states has a designed customer experience, not just a backend state.

### Principle 6 — Local reality is a first-class design input

Do not design as though Kigali is a copy of London or San Francisco. Account for:

- mobile money as the primary payment behavior
- local addresses and landmarks over street numbers
- multilingual potential (English first, Kinyarwanda later)
- cash-on-delivery realities where supported
- variable road conditions
- dense neighborhood names without conventional street addressing
- customer phone-call behavior (riders often call)
- local merchant operating habits

---

# 5. Why Tuma can beat incumbents

These are **strategic advantages to pursue**, not guaranteed outcomes.

## 5.1 Better customer trust

Competitors can say "real-time tracking." Tuma makes trust visible through:

- accurate order states
- clear timestamps
- transparent fees
- explicit ETA ranges
- rider identity once assigned
- meaningful delivery progress
- proactive delay communication
- simple support recovery

The goal is for the customer to feel that Tuma is telling the truth about the order.

## 5.2 Better merchant economics

Many marketplaces optimize primarily for consumer volume. Tuma should make merchants ask:

> "Does this platform make my restaurant run better?"

Provide:

- conversion metrics
- repeat customer metrics
- preparation-time analytics
- cancellation reasons
- out-of-stock impact
- peak-hour demand
- product performance
- promotion ROI
- delivery quality

## 5.3 Better merchant control

Merchants manage their operation without begging support for basic changes. Real-time controls over:

- open/closed state
- pause orders ("we are overwhelmed; pause new orders for 20 minutes" — without contacting Tuma staff)
- preparation time
- item availability
- modifier availability
- menu price
- special hours
- order acceptance
- cancellations
- refunds where permitted

## 5.4 Better delivery intelligence

Build dispatch as a real system instead of manually assigning riders forever. A dispatch engine considers rider availability, rider location, route distance, restaurant prep time, rider capacity, current workload, vehicle type, delivery zone, promised ETA, and — later — batch opportunities.

## 5.5 Local payments as infrastructure

MTN's developer platform exposes payment collection and disbursement APIs and explicitly supports Rwanda. Payment integration is designed as a **provider-agnostic platform layer**, with MTN MoMo as the first adapter and additional providers (Airtel Money, card, cash workflow) added through the same contract. [MTN MoMo API / MTN Payments V1]

## 5.6 Address intelligence

Do not treat delivery addresses as a single text box. The platform stores:

```text
latitude, longitude, label, building / place name,
street / road if available, sector, cell, landmark,
instructions, phone contact
```

Over time, Tuma builds an internal address/landmark intelligence layer for better delivery success. Better addresses improve rider efficiency and customer satisfaction — this is strategically important.

## 5.7 Merchant-owned relationship

A long-term differentiator: giving merchants more control over their customer relationship through first-party order pages, direct reorder links, loyalty, customer insights, marketing campaigns, QR ordering, web ordering, and embeddable ordering. This moves Tuma from "listing marketplace" toward "merchant commerce infrastructure."

---

# 6. Business model architecture

The backend supports multiple revenue models without hard-coding one model everywhere.

| Revenue source | Description | When |
|---|---|---|
| Commission | Percentage of item subtotal charged to the merchant | From launch |
| Delivery fee | Customer-facing fee from the delivery pricing engine | From launch |
| Service/platform fee | Platform fee displayed transparently where applicable | From launch |
| Promotion / sponsored placement | Merchants pay for promoted placements | Growth phase |
| Subscription / loyalty | Customer membership for reduced fees/benefits | Later |
| Merchant SaaS | Advanced merchant tooling subscription | Later |
| B2B delivery | Delivery-as-a-service API and business accounts | Later |

---

# 7. The customer product

## 7.1 Customer navigation

```text
Home • Search • Orders • Profile
```

Cart remains contextual and persistent when populated (floating CartBar + Cart tab badge — the shipped dual affordance is retained).

## 7.2 Discovery journey

```text
HOME → Search / Category / Recommendation → Store comparison → Store → Product
```

Home answers: Where am I delivering? What can I order now? What is fast? What is popular? What are the best deals? What did I order recently?

## 7.3 Store experience

The store page communicates: store identity, open/closed state, rating, delivery estimate, distance, delivery fee, promotions, menu categories, best sellers, item availability.

## 7.4 Product experience

A product communicates: image, name, description, price, dietary/ingredient information where available, options, add-ons, quantity, special instruction.

## 7.5 Cart experience

Cart owns commitment. The customer sees:

```text
Items, customization, subtotal, delivery fee, service/platform fee,
discount, total, destination, payment, estimated arrival
```

**One cart → one merchant → one delivery.** Multi-store carts are not supported initially (see Ch 33).

## 7.6 Checkout

Checkout is not an enormous form. Progressive sections:

```text
Delivery address → Delivery instructions → Payment method → Order review → Place order
```

At "Place order" the **server recalculates the basket**, locks/validates prices, and only then initiates payment. The client never sends totals the server will trust.

## 7.7 Order tracking

Tracking is the customer-facing window into the logistics system:

```text
Confirmed → Accepted → Preparing → Ready → Rider assigned
→ Picked up → On the way → Nearby → Delivered
```

The UI distinguishes merchant progress, rider progress, and customer proximity. The shipped tracking screen composition (map + EtaCard + StatusTimeline + DriverRow + OrderSummary) is retained; it now renders **canonical server tracking state** instead of a local simulation.

## 7.8 Order history

Orders become a personal commerce memory. Useful actions: reorder, rate, report problem, view receipt, view delivery details.

## 7.9 Customer account system

```text
Profile: Personal information, Addresses, Payment methods, Favorites,
Notifications, Language, Support, Privacy, Security, Sign out
```

- **Saved addresses** — each supports label, coordinates, place name, landmark, instructions, phone, default flag (see Ch 13).
- **Payment methods** — store provider references/tokens, never sensitive payment credentials that Tuma should not own.
- **Favorites** — favorite stores and products.
- **Notification preferences** by category: order updates, promotions, delivery updates, account/security.

---

# 8. Merchant system

The merchant product is a **real operating system**, not a CRUD dashboard. Primary surface: **Merchant Web App**. Optional later: merchant mobile app.

## 8.1 Merchant navigation

```text
Overview • Orders • Menu • Store • Customers • Promotions
Finance • Analytics • Settings • Support
```

## 8.2 Merchant dashboard

The merchant homepage tells an owner what they need in under 10 seconds.

Top-level KPIs:

```text
Today's sales, orders today, average order value,
average prep time, cancellation rate, rating
```

Operational alerts (examples):

```text
3 orders waiting for acceptance
5 orders delayed
Chicken burger is out of stock
Store has been paused
Rider waiting 9 minutes
```

The dashboard prioritizes **action**, not decoration.

## 8.3 Merchant order management

One of the most important platform surfaces. Order board:

```text
NEW → ACCEPTED → PREPARING → READY → PICKED UP → DELIVERED
```

Each order card shows: order number, customer first name, items, item count, total, requested time, prep deadline, rider status, current state, issue warnings.

Actions: accept, reject, adjust prep time, mark item unavailable, mark ready, contact support, view rider, view customer delivery details as appropriate.

## 8.4 Merchant menu management

```text
Catalog
  ├── Categories
  ├── Products (name, description, image, price, availability, options)
  └── Modifier groups
```

Capabilities: drag/reorder categories, reorder products, change prices, upload images, pause item, schedule availability, configure modifiers, set minimum/maximum selections. Maintain an audit trail for meaningful changes.

## 8.5 Merchant store management

Controls: open/closed, temporary pause, normal hours, special hours, holidays, delivery availability, preparation time, minimum order value, delivery radius, store information.

## 8.6 Merchant analytics

The platform helps answer: What sells? When are we busiest? What converts? What causes cancellations? How long does preparation take? Who are our repeat customers? Which promotions work? Which locations generate demand?

Metrics: gross sales, net sales, orders, average order value, conversion, repeat rate, cancellation rate, average prep time, delivery success, rating, product performance, promotion ROI.

## 8.7 Merchant finance

Merchant finance reconciles:

```text
Customer payment → Platform fees → Merchant commission
→ Refunds / adjustments → Net merchant amount → Payout
```

Merchant finance screens: current balance, pending balance, paid out, payout history, order-level earnings, commission, fees, refunds, downloadable statements.

**Never calculate historical financial values dynamically from current mutable menu prices. Store immutable monetary snapshots on orders and order items.**

---

# 9. Rider system

The rider application (Flutter) is the other side of delivery.

## 9.1 Rider navigation

```text
Home • Deliveries • Earnings • Profile
```

Rider home shows: online/offline status, current task, current zone, earnings today, delivery count, performance. Primary action: **GO ONLINE**.

## 9.2 Rider order lifecycle

```text
AVAILABLE → OFFERED → ACCEPTED → NAVIGATING_TO_PICKUP
→ ARRIVED_AT_PICKUP → PICKED_UP → NAVIGATING_TO_CUSTOMER
→ ARRIVED → DELIVERED
```

**Every transition is authorized and timestamped by the server.** The app does not simply change a local status; it requests transitions and the server validates them.

## 9.3 Rider delivery screen

Shows: restaurant name, pickup address, customer destination, customer contact policy, order contents where operationally necessary, notes, navigation action, current earnings, status action.

The most important button always corresponds to the next legitimate state:

```text
ARRIVED AT PICKUP → MARK AS PICKED UP → START DELIVERY → ARRIVED → CONFIRM DELIVERY
```

## 9.4 Rider earnings

Shows: delivery earnings, bonuses, incentives, tips if supported, deductions if any, completed delivery count, daily/weekly/monthly totals.

**Earnings never depend on client-side calculations. The server creates immutable earning records.**

## 9.5 Rider offline considerations

Connectivity interruptions can occur during deliveries. Critical rider commands (state transitions, delivery confirmation) must have reliable retry + idempotency behavior so a retried command never double-counts.

---

# 10. Order architecture

An order is a financial and operational record.

## 10.1 Immutable snapshots

At checkout, snapshot:

- product name
- product price
- selected options
- option prices
- tax/fee assumptions where applicable
- discount allocation
- store information needed for the receipt

**Do not recalculate an old order using today's product data.**

## 10.2 Order state machine

```text
DRAFT → PENDING_PAYMENT → PAID → PLACED → MERCHANT_ACCEPTED
→ PREPARING → READY_FOR_PICKUP → RIDER_ASSIGNED → PICKED_UP
→ IN_TRANSIT → ARRIVED → DELIVERED
```

Alternative/exception states:

```text
REJECTED, CANCELLED, PAYMENT_FAILED, REFUND_PENDING,
REFUNDED, DELIVERY_FAILED, CUSTOMER_UNREACHABLE, ITEM_UNAVAILABLE
```

Not every state transition is legal. Implement an **explicit state transition service** instead of allowing arbitrary updates. The exact state graph is encoded and tested.

## 10.3 Transition rules (examples)

```text
PLACED → MERCHANT_ACCEPTED | REJECTED
MERCHANT_ACCEPTED → PREPARING | CANCELLED
PREPARING → READY_FOR_PICKUP | ITEM_UNAVAILABLE
READY_FOR_PICKUP → RIDER_ASSIGNED
RIDER_ASSIGNED → PICKED_UP
PICKED_UP → IN_TRANSIT
IN_TRANSIT → ARRIVED
ARRIVED → DELIVERED
```

The prototype's 5-state lifecycle (`placed → preparing → pickedUp → onTheWay → arrived`) is a collapsed view of this machine used only by the test harness. Production UI maps the full machine onto the customer timeline (Ch 7.7).

## 10.4 Customer-readable event timeline

Every order has a durable event history:

```text
7:03 PM  Order placed
7:04 PM  Restaurant accepted
7:09 PM  Preparation started
7:24 PM  Order ready
7:25 PM  Sarah picked up your order
7:38 PM  Sarah is nearby
7:41 PM  Delivered
```

This creates a durable explanation of what happened — the foundation of trust and of support resolution.

---

# 11. Payments architecture

## 11.1 Provider-agnostic payment layer

Never make the order service depend directly on one payment provider.

```text
PaymentService
     |
     +---- MTN MoMo Adapter      (first — Rwanda-supported collection + disbursement APIs)
     +---- Airtel Money Adapter
     +---- Card Provider Adapter
     +---- Cash-on-delivery workflow
```

MTN currently provides payment collection and disbursement APIs and documents Rwanda support. Treat provider availability, commercial terms, credentials, limits, and production approval as **integration requirements to verify**, not assumptions. [MTN Payments V1]

## 11.2 Normalized payment state

```text
PENDING → REQUIRES_ACTION → PROCESSING → SUCCEEDED
                                       → FAILED
CANCELLED, REFUNDED, PARTIALLY_REFUNDED
```

All providers map into this normalized state. The rest of the platform only ever sees normalized states.

## 11.3 Payment safety rules

The client must never be trusted to declare:

```text
payment successful • order paid • refund successful
```

**Provider callback/webhook or verified provider status is the authoritative confirmation.** Webhook signatures are verified. Every payment operation records:

```text
idempotency key, provider reference, internal payment ID, order ID,
amount, currency, status, timestamps, failure code, audit information
```

## 11.4 Payment flows

- **Mobile money (primary):** customer confirms on their phone (USSD/prompt); Tuma polls/receives webhook; order advances only on verified success. `REQUIRES_ACTION` is a first-class UI state ("Check your phone to approve payment").
- **Cash on delivery:** order proceeds with payment state `PENDING`; rider collects; settlement workflow reconciles cash at handoff/payout.
- **Refunds:** explicit workflow (Ch 24) producing accounting records and audit trails.

## 11.5 Monetary modeling

**Never use floating-point numbers for money.** Use integer minor units. For RWF, determine the provider/accounting convention and keep a canonical currency representation. Every monetary amount carries `amount` + `currency`. Price calculation is deterministic and testable.

---

# 12. Pricing engine

Pricing is a dedicated domain, not scattered `if` statements.

Input:

```text
merchant, customer location, order subtotal, delivery distance,
zone, time, promotions, membership, surge/rules
```

Output:

```text
subtotal, merchant discounts, platform discount, delivery fee,
service fee, other fees, customer total, merchant net, platform revenue
```

**Store the final price breakdown with every order** (immutable snapshot).

---

# 13. Location and address system

## 13.1 Address creation

Supports: map pin, current location, search, saved place, manual entry.

Collected fields:

```text
label, latitude, longitude, placeName, addressText, landmark,
sector, cell, instructions, contactPhone
```

## 13.2 Address confirmation screen

```text
MAP — exact pin
DELIVER TO — Kigali Heights
LANDMARK — Near ...
INSTRUCTIONS — Call when outside the gate.
[CONFIRM ADDRESS]
```

## 13.3 Zones and geography

```text
Country → City → Service Area → Delivery Zone → Merchant → Rider coverage
```

A zone defines: enabled/disabled, delivery pricing rules, supported merchants, expected ETA, rider supply needs, operating hours. Use PostGIS when geographic queries become real requirements.

## 13.4 Maps and routing

Use an abstraction around maps — `MapProvider`, `RoutingProvider`, `GeocodingProvider`, `DistanceProvider`. Do not scatter provider-specific code throughout the apps.

- **Customer/rider mobile:** `flutter_map` with a controlled tile source is the shipped baseline; the tile/routing boundary is the swap point for a styled or commercial tile provider.
- **Web surfaces:** MapLibre GL with vector tiles.
- **Routing:** behind an adapter so Tuma can switch providers later. The prototype's Bézier `generateRoute` is retired from production paths — real routing goes through the adapter; it remains available in the test harness.

---

# 14. Dispatch engine

A major future competitive moat. Initial rule-based dispatch can be simple; later it becomes an optimization system.

## Inputs

```text
New order, restaurant location, customer location, restaurant preparation
estimate, available riders, rider locations, rider workloads, vehicle types,
current traffic estimate, delivery zones
```

## Candidate selection

Filter riders by: online status, valid service zone, distance to pickup, active delivery capacity, rider type, temporary restrictions.

## Ranking

Rank candidates by a weighted score such as:

```text
ETA to pickup + expected total delivery time + current workload
+ rider idle time + zone balancing + reliability score
```

**Do not hard-code business economics into the ranking formula. Make dispatch rules configurable.**

## Delivery lifecycle

```text
Order accepted → Fulfillment expected → Dispatch search begins
→ Rider candidate identified → Rider offer sent
→ Accepted? YES: Assigned / NO: Next candidate
→ Rider heads to merchant → Pickup → Delivery route
→ Customer arrival → Proof of delivery → Completed
```

All important events become durable records. Rider offer timeout is handled by a background job.

---

# 15. Real-time tracking architecture

Production tracking is **not simulated**.

```text
RIDER APP (Flutter)
   | GPS samples (while active delivery in progress)
   v
Realtime Gateway
   |
   +--> Delivery Location Stream
   v
DELIVERY SERVICE
   |
   +--> Redis current state (hot)
   +--> PostgreSQL durable checkpoints
   v
CUSTOMER APP (canonical tracking state over WebSocket)
```

The server validates every sample:

- rider owns the assigned delivery
- rider is in a valid delivery state
- location is plausible
- timestamps are valid
- update rate is within acceptable limits

Storage policy:

```text
hot state              → Redis / realtime layer
important checkpoints  → PostgreSQL
historical telemetry   → specialized storage later if needed
```

Do not permanently write every GPS sample into a relational table at extreme frequency.

## Tracking data flow

```text
Rider GPS → Location validation → (map matching / smoothing later)
→ Current rider position → ETA calculation → Delivery state
→ WebSocket fan-out → Customer tracking UI
```

**The customer UI does not calculate truth from raw coordinates.** The backend delivers a canonical tracking state:

```json
{
  "deliveryId": "...",
  "status": "ON_THE_WAY",
  "rider": { "id": "...", "displayName": "Sarah" },
  "position": { "lat": -1.94, "lng": 30.06, "bearing": 87 },
  "etaSeconds": 540,
  "distanceMeters": 2100,
  "updatedAt": "..."
}
```

This is the production shape the prototype's `TrackingState` was designed to anticipate — the Flutter tracking widgets render this payload unchanged.

---

# 16. ETA engine

ETA is not a static number. Conceptually:

```text
remaining road distance ÷ expected travel speed
+ pickup / handoff uncertainty
+ traffic / congestion adjustment
```

Later, ETA can incorporate learned historical travel times by road segment, time of day, day of week, weather if appropriate, and neighborhood.

**Always expose uncertainty appropriately. Use an ETA range when precision is not reliable** — this matches the shipped `~ 14 min` presentation.

---

# 17. Notifications

Channels:

```text
Push (first) • In-app • SMS later • Email later
```

Events:

```text
Order accepted, order delayed, order ready, rider assigned, rider nearby,
order delivered, payment succeeded, payment failed, refund completed,
promotion, security alert
```

Create a **notification service** rather than triggering device push calls directly from random modules. Notification preferences are per-category (Ch 7.9).

---

# 18. Support system

Support exists as a platform capability, attached to an order/delivery whenever possible.

| Customer issue types | Merchant issue types | Rider issue types |
|---|---|---|
| missing item | rider late | pickup blocked |
| wrong item | order dispute | address issue |
| late order | payment dispute | unreachable customer |
| payment issue | customer issue | accident/emergency |
| delivery problem | | app problem |
| damaged item | | |
| order cancellation | | |

---

# 19. Refunds and disputes

An explicit refund workflow:

```text
Issue reported → Case created → Evidence collected → Decision
→ Refund amount → Payment provider refund → Customer notification
→ Case resolved
```

**Refunds produce accounting records and audit trails.** Partial refunds are supported by the normalized payment state (`PARTIALLY_REFUNDED`).

---

# 20. Cancellation, rejection, out-of-stock

## 20.1 Cancellation architecture

Modeled by **actor and timing**. Actors: customer, merchant, rider, platform, payment provider.

```text
Before merchant accepts → generally easier to cancel
After preparation begins → policy changes
After pickup → stricter policy
```

Always record: actor, reason, state, refund outcome, timestamp.

## 20.2 Merchant rejection handling

```text
Order rejected → Customer notified → Payment reversal/refund workflow
→ Offer alternatives where useful
```

Do not leave an order sitting in limbo.

## 20.3 Out-of-stock handling

```text
Merchant marks unavailable → Customer sees update
→ If order affected: substitute / remove / refund
```

Later, merchants can configure substitution policy.

## 20.4 Delivery proof

Depending on operational policy, delivery completion can include: customer confirmation, OTP/PIN, rider confirmation, photo proof where appropriate, location checkpoint. Do not use a single mechanism blindly — the policy varies by delivery category and risk.

---

# 21. Admin platform

The admin product is the **control plane** for the company, not a database viewer.

## 21.1 Sections

```text
Overview, Orders, Customers, Merchants, Riders, Dispatch, Payments,
Finance, Promotions, Support, Locations / Zones, Analytics, Content,
Risk / Fraud, System, Audit Log
```

## 21.2 Overview dashboard

Live operational health:

```text
Active orders, orders waiting for merchant, orders waiting for rider,
deliveries in transit, delayed deliveries, failed payments, support cases,
online riders, open merchants
```

Alert panels:

```text
10 orders > promised ETA
4 merchants currently paused
3 rider supply shortages in zone
Payment provider degradation detected
```

This dashboard is an **operations cockpit**, not a decorative analytics page.

## 21.3 Order control center

Admin can: search any order, inspect full state history, payment state, merchant state, rider assignment, location events; open support cases; initiate permitted recovery actions. Every sensitive action requires permission and creates an audit event.

## 21.4 Merchant administration

```text
Application → Review → Verification → Approved → Onboarding → Active
```

Merchant profile: legal information, owners, locations, payout information, menu, documents, status, risk flags, commission agreement.

## 21.5 Rider administration

```text
Applicant → Documents submitted → Verification → Approved → Active
→ Suspended / Deactivated
```

Admin sees: documents, vehicle information, operational zone, status, delivery history, earnings, support cases, reliability, customer rating.

## 21.6 Dispatch operations console

A map-first operational screen: live map with riders and active routes; right panel with unassigned/delayed counts and riders online. Operators can inspect delivery/rider, manually reassign where authorized, contact support, flag abnormal movement, override dispatch in exceptional cases. **No unrestricted manual edits without audit logging.**

## 21.7 Content and merchandising

Admin manages homepage banners, categories, promoted stores, featured products, campaigns, announcements — with boundaries so paid placement does not destroy search quality.

---

# 22. Promotions engine

Do not implement discounts as a random set of `if` statements. Model promotions explicitly.

Types: percentage discount, fixed discount, free delivery, first order, store-specific, category-specific, minimum order, time window, customer segment.

Every promotion has:

```text
eligibility rules, benefit, usage limits, start/end, scope, budget
```

A promotion evaluation service computes discounts; results are recorded per order (`promotion_redemptions`). The prototype's hard-coded `TUMA10`/`WELCOME20` codes become real promotions in this engine (retained as the first seeded promotions for staging).

---

# 23. Search and recommendations

## Search

Start PostgreSQL-backed: store name, cuisine, product name, category, popular queries. Later, when traffic justifies: indexing pipeline into Meilisearch/OpenSearch behind an interface.

## Recommendations

Start deterministic: popular nearby, recently ordered, highly rated, fast delivery, active promotion. Later a recommendation service over customer history, merchant quality, product popularity, context, location. **Don't build machine-learning infrastructure before the platform has meaningful behavioral data.**

---

# 24. Ratings and reputation

Customers rate merchant/order and delivery experience. Aggregate ratings are robust against manipulation. Potential future dimensions: food quality, packaging, delivery speed, rider experience, overall.

For riders, ratings are not displayed as a weapon; they feed coaching/support/risk systems carefully.

---

# 25. Anti-fraud / risk system

As money and incentives grow, abuse becomes inevitable. Signals:

- repeated payment failures / chargebacks
- suspicious account creation
- promo abuse
- impossible delivery GPS / rider GPS anomalies
- repeated "order not received" claims
- unusual refund frequency
- merchant manipulation

Start rule-based. Later develop a dedicated risk service.

---

# 26. Backend architecture

## 26.1 Modular monolith first

Do **not** start with microservices.

```text
                  Fastify
                     |
        +------------+------------+
        |            |            |
     Auth        Commerce      Delivery
        |            |            |
     Users        Orders       Dispatch
        |            |            |
        +------------+------------+
                     |
              PostgreSQL + Redis
```

All modules live in one deployable backend at first. This is intentionally a **modular monolith**, not a pile of unrelated code. Extract services only when there is a proven, measured reason.

## 26.2 Module boundaries

```text
modules/
  auth/ users/ merchants/ stores/ catalog/ search/ cart/
  pricing/ promotions/ orders/ payments/ deliveries/ dispatch/
  riders/ notifications/ support/ analytics/ admin/
```

Each module owns: routes, schemas, service, repository, domain types, mappers, policies, unit tests. Avoid circular dependencies.

## 26.3 Technology stack

```text
Backend:  Node.js, Fastify, TypeScript, Prisma, PostgreSQL, PostGIS,
          Redis, Zod, WebSocket
Mobile:   Flutter (customer app + rider app) — see Flutter spec
Web:      TypeScript (merchant web app, admin console)
```

## 26.4 Database strategy

| Concern | Choice |
|---|---|
| Primary database | PostgreSQL |
| Geospatial | PostGIS |
| ORM | Prisma for the majority of application access |
| Cache / ephemeral realtime state | Redis |

The database architecture favors **correctness and traceability** over cleverness.

## 26.5 API architecture

Versioned REST initially, organized around business capabilities — not database-shaped endpoints merely because tables exist.

```text
/api/v1/auth        /api/v1/users      /api/v1/stores
/api/v1/products    /api/v1/search     /api/v1/cart
/api/v1/orders      /api/v1/payments   /api/v1/deliveries
/api/v1/addresses   /api/v1/favorites  /api/v1/notifications
/api/v1/admin/...   /api/v1/merchant/...  /api/v1/rider/...
```

## 26.6 Realtime architecture

REST handles durable commands and queries. WebSocket handles changing state.

```text
REST:      Create order, accept order, place order, update menu, create promotion
WebSocket: Order status, rider location, delivery ETA, merchant live orders, rider offer
```

Do not use WebSocket for everything.

## 26.7 Event model

Internal domain events:

```text
OrderPlaced, PaymentSucceeded, MerchantAcceptedOrder, OrderReadyForPickup,
RiderAssigned, RiderPickedUpOrder, RiderLocationUpdated, DeliveryArrived,
OrderDelivered, PaymentRefunded
```

Events trigger notifications, analytics, workflow jobs, support automation, merchant updates, customer updates. Initially these live inside the modular monolith.

## 26.8 Background jobs

A job system (Redis-backed to start) for: send push notification, payment reconciliation, merchant payout, refund processing, promotion expiry, order timeout, rider offer timeout, audit aggregation, analytics processing. **Do not introduce Kafka simply because Tuma is "real-time."**

---

# 27. Core domain model

```text
User
 ├── UserRole
 ├── Address
 ├── FavoriteStore / FavoriteProduct
 ├── PaymentMethodReference
 └── Order

Merchant
 ├── MerchantLocation
 ├── Store
 ├── Staff
 ├── Menu
 ├── Product
 ├── Promotion
 └── PayoutAccount

Store
 ├── StoreCategory
 ├── Product
 ├── StoreHours / StoreAvailability
 └── Order

Order
 ├── OrderItem → OrderItemOption
 ├── PriceBreakdown
 ├── Payment
 ├── Delivery
 ├── OrderStatusEvent
 ├── SupportCase
 └── Review

Delivery
 ├── RiderAssignment
 ├── DeliveryStatusEvent
 ├── DeliveryRoute
 ├── LocationCheckpoint
 └── ProofOfDelivery

Rider
 ├── RiderProfile
 ├── RiderVehicle
 ├── RiderAvailability
 ├── RiderAssignment
 └── RiderEarning
```

## Recommended schema groups

| Group | Tables |
|---|---|
| Identity | users, user_roles, user_sessions, user_devices, addresses |
| Merchants | merchants, merchant_staff, stores, store_hours, store_special_hours, store_status_events |
| Catalog | categories, products, product_categories, modifier_groups, modifier_options, product_modifier_groups, product_availability |
| Commerce | carts, cart_items, orders, order_items, order_item_options, price_breakdowns, promotions, promotion_redemptions, favorites, reviews |
| Payments | payments, payment_attempts, payment_events, refunds, payouts, payout_items |
| Logistics | riders, rider_vehicles, rider_availability, rider_locations_current, deliveries, delivery_assignments, delivery_status_events, delivery_route_points, delivery_location_events |
| Support | support_cases, support_messages, support_attachments |
| Platform | notifications, notification_deliveries, audit_logs, feature_flags, system_config |

---

# 28. Authentication, authorization, security

## 28.1 Authentication

Email/phone registration, login, session refresh, logout, password reset, device sessions, optional OTP verification.

Roles:

```text
CUSTOMER, MERCHANT_OWNER, MERCHANT_MANAGER, RIDER,
SUPPORT_AGENT, OPERATIONS, FINANCE, ADMIN, SUPER_ADMIN
```

## 28.2 Capability-based authorization

```text
MERCHANT_OWNER     can_manage_menu, can_manage_store, can_view_finance
MERCHANT_MANAGER   can_manage_orders, can_manage_store
SUPPORT_AGENT      can_view_orders, can_create_support_cases
FINANCE            can_view_finance, can_manage_refunds
```

Do not rely only on a single giant `isAdmin` flag. Authorization is server-side, least privilege.

## 28.3 Security model — required from the beginning

Secure password hashing, token/session rotation, rate limiting, validation at every API boundary, authorization checks, audit logs, secret management, encrypted transport, secure file upload handling, webhook signature verification, idempotency, brute-force protection, admin MFA later/where feasible.

Never trust:

```text
client prices • client roles • client order state
client payment state • client location ownership
```

## 28.4 Idempotency

Critical commands are idempotent: create payment, place order, accept order, refund, assign delivery, mark delivered. Use idempotency keys where a request might be retried. This prevents **double order, double payment, double refund, double payout**.

## 28.5 Admin audit system

Sensitive actions create audit records: who, what, where, before, after, reason, timestamp, requestId. Examples: refund issued, rider suspended, merchant deactivated, commission changed, payout adjusted, order manually changed.

---

# 29. Observability and reliability

## 29.1 Logs

Structured JSON logs. Every important request traceable by: `requestId, userId, orderId, deliveryId, merchantId, riderId`.

## 29.2 Metrics

```text
API latency, error rate, payment failures, order conversion,
merchant acceptance rate, prep time, rider assignment time, pickup time,
delivery time, ETA accuracy, cancellation rate, refund rate
```

## 29.3 Traces

Trace critical workflows: place order, payment, dispatch, delivery, refund.

## 29.4 Reliability targets

Define product SLIs/SLOs instead of saying "make it reliable": API availability, order creation success, payment verification latency, realtime connection success, notification delivery, ETA freshness. Exact targets are established after observing real traffic and the cost of meeting them.

## 29.5 Backups and disaster recovery

Database backups, point-in-time recovery, **restore testing**, secret recovery, object storage backups/versioning where useful, incident procedures. A backup nobody has tested is not a reliable recovery strategy.

## 29.6 Privacy and data minimization

Collect only what the product needs. Sensitive categories: phone, address, location history, payments, merchant financial data, rider documents. Policies define retention, access, deletion, export where applicable, operational visibility. Avoid exposing customer location or contact information to parties that do not need it.

---

# 30. File and media system

Media types: merchant logos, store cover images, product images, rider avatars, merchant documents, support attachments.

Use **object storage** rather than storing binary files in PostgreSQL.

```text
Upload → Validation → Virus/content checks where appropriate
→ Resize / optimize → Object storage → CDN
```

Store metadata and URLs/references in PostgreSQL. Food delivery is visually driven — merchant onboarding encourages clear dish names, good photos, concise descriptions, correct pricing, modifiers. Admin can flag low-quality listings.

---

# 31. Design system architecture

The design system exists as a product asset, not an afterthought. The shipped **emerald system** is the locked foundation.

## 31.1 Color tokens

| Token | Value | Purpose |
|---|---|---|
| Primary (emerald) | `#0B6E4F` | Trust, navigation, filled CTAs, selected chips |
| Accent (orange) | `#FF7A1A` | Energy, promo gradient, popular badge, sheet CTA |
| Deep emerald | `#08412F` | CartBar background (darker so white pops) |
| Background / Surface | `#F7F7F5` | Warm greige scaffold |
| Card | `#FFFFFF` | Cards, sheets, inputs |
| Text | `#1B1B1F` | Primary copy |
| Muted | `#6B6B70` | Secondary information |
| Error | `#D64545` | Validation |
| Border | `#ECECE8` | Card/divider/input border |
| OnPrimary | `#FFFFFF` | Text on primary |
| Success | selective green | Delivered/success states only |

## 31.2 Typography

Poppins for headlines/display, Inter for body/label.

| Role | Font | Size | Weight | Use |
|---|---|---|---|---|
| Display | Poppins | 32–34 | 800–900 | Splash "Tuma" |
| Screen title | Poppins | 22–26 | 700–800 | Page headers |
| Section | Poppins | 16–19 | 700–800 | Content groups |
| Body | Inter | 14–16 | 400/500 | Descriptions |
| Meta | Inter | 12–13 | 500 | ETA, distance, category |
| Price | Poppins/Inter | 16–22 | 800–900 | Products/totals |

## 31.3 Spacing, shape, motion tokens

```text
Spacing: xs4 sm8 md12 lg16 xl24 xxl32
Radii:   sm8 md12 lg16 xl24
Durations: fast200 normal350 slow600 searchDebounce300
Touch:   52h primary buttons, 40×40 add buttons, 46 avatar
```

## 31.4 Component layers

```text
Primitives:  Text, Icon, Button, Input, Badge, Avatar, Divider, Sheet, Modal
Commerce:    StoreCard, ProductCard, Price, CartBar, CartItem,
             ModifierSelector, OrderSummary
Logistics:   DeliveryStatus, RiderCard, ETA, OrderTimeline,
             MapOverlay, LocationMarker
```

The visual identity should be recognizable even without the logo. Do not copy Domino's brand identity — borrow ordering clarity, not branding.

## 31.5 Motion language

Use motion to explain state: add to cart → card responds; checkout → confirmation transition; rider assigned → timeline advances; rider moving → map marker moves smoothly; delivered → completion state. **Do not animate things simply because an animation library exists.**

## 31.6 Accessibility

Semantic labels, accessible touch targets (≥40–48dp), readable contrast, scalable text, non-color status communication, screen reader descriptions where practical, reduced-motion behavior where supported.

---

# 32. Customer visual and experience philosophy

Tuma should feel: **premium, warm, fast, confident, local.**

Premium does not mean visual extravagance. It means: fast, predictable, clear, quiet, responsive, forgiving.

A premium interaction:

```text
Tap → Immediate visual response → Server confirmation → Clear updated state
```

Not:

```text
Tap → Spinner → Blank screen → Unexpected state
```

## The "wow" moments

1. **First order** — a beautiful, confident checkout and confirmation.
2. **Rider assignment** — "Sarah is bringing your order."
3. **Live movement** — the rider visibly moves on the map with current ETA.
4. **Nearby moment** — "Your rider is 2 minutes away."
5. **Delivery completion** — a satisfying completion state with instant reorder/rating.
6. **Reorder** — a previous order nearly one gesture away from repeating.

## The "trust" moments (more important than wow)

- **Fee transparency** — no surprise totals.
- **Delay explanation** — tell customers what happened.
- **Payment status** — never leave customers wondering whether they were charged.
- **Rider identity** — show who is handling the delivery when appropriate.
- **Support** — help easy to find when something fails.

## Customer care philosophy

A delivery marketplace creates failures by nature. The advantage is not "nothing ever goes wrong." The advantage is:

> **When something goes wrong, Tuma resolves it faster and more honestly.**

Example delay communication:

```text
Your order is running 8 minutes late.
The restaurant needed extra preparation time.
New arrival estimate: 7:18 PM
[View order] [Get help]
```

---

# 33. Product policies

- **Multi-store cart:** not initially. One cart → one merchant → one delivery. This dramatically simplifies pricing, dispatch, preparation synchronization, refunds, and ETA. Multi-store batching can be considered later as a deliberate feature.
- **Multi-order rider batching:** later optimization only. Must improve unit economics without destroying ETA and food quality. Never introduce it to look sophisticated.
- **Scheduled orders:** not initially unless there is a product reason. When introduced, they enter operations at the correct future time rather than behaving like immediate orders with a delayed notification.

---

# 34. Client app state model

Separate:

```text
Server state:    orders, stores, products, profile, notifications
Client state:    cart draft, selected filters, UI preferences, temporary checkout state
Realtime state:  active rider position, delivery progress
```

Do not duplicate server truth unnecessarily in client state. In the Flutter apps: Riverpod providers backed by the API client hold server state; Hive is a cache/draft store, never the database of record.

**Offline behavior:** the customer app tolerates temporary network interruption (preserve current screen, show connection state, retry safe requests, avoid duplicate commands). The rider app needs stronger offline handling — critical commands retry with idempotency (Ch 9.5).

**API contract strategy:** frontends consume a typed/generated API client derived from the backend schema. Apps do not manually reconstruct API data shapes everywhere.

---

# 35. Analytics and experimentation

## 35.1 Event model

Track events at product boundaries.

```text
Customer: app_opened, store_viewed, product_viewed, product_added_to_cart,
          checkout_started, order_placed, payment_succeeded, tracking_opened,
          order_delivered, reorder_started
Merchant: order_received, order_accepted, prep_time_changed, item_out_of_stock,
          order_ready, store_paused, promotion_created
Rider:    went_online, delivery_offered, delivery_accepted, arrived_pickup,
          picked_up, arrived_customer, delivered
```

## 35.2 Data architecture

PostgreSQL for authoritative transactional data. Redis for ephemeral state, caching, realtime fan-out helpers, locks, rate limits, job queues. Later: PostgreSQL → analytics pipeline → warehouse/BI. **Do not turn PostgreSQL into an analytics warehouse.**

## 35.3 Experimentation

Feature flags and controlled experiments eventually: home ranking, checkout layout, promotion placement, delivery fee presentation, reorder CTA. A/B tests are server-configurable and measurable. Do not embed experimental percentages throughout UI components.

---

# 36. Production flows

## 36.1 Customer order flow

```text
Customer opens app → Location selected → Browse/search → Store opened
→ Product configured → Cart → Checkout → Server recalculates basket
→ Server locks/validates prices → Payment initiated → Payment verified
→ Order created → Merchant notified → Merchant accepts → Preparation begins
→ Dispatch begins → Rider assigned → Rider pickup
→ Customer receives realtime updates → Delivery completed
→ Receipt / rating / reorder
```

## 36.2 Merchant flow

```text
Merchant signs in → Store online → Order arrives → Accept / reject
→ Preparation → Mark ready → Rider arrives → Handoff → Order delivered
→ Revenue recorded → Merchant analytics updated
```

## 36.3 Rider flow

```text
Rider approved → Go online → Dispatch offer → Accept
→ Navigate to merchant → Pickup → Navigate to customer
→ Customer arrival → Proof / confirmation → Delivered → Earnings finalized
```

## 36.4 Admin flow

```text
Monitor platform → Detect exception → Inspect order
→ Inspect merchant / rider / payment → Take controlled action
→ Record audit event → Notify affected user → Resolve case
```

---

# 37. Simulation removal and test harness

The prototype's simulation tooling (seeded catalog, `TrackingSimulation` Bézier timer, mock payments) is **removed from production code paths**. Production supports: real rider GPS, real dispatch, real status transitions, real ETA, real notifications, real payment confirmation, real order state.

Simulation remains available as a **test harness**, never as business logic. A production-quality test harness generates:

```text
synthetic customer, synthetic merchant, synthetic rider, synthetic order,
synthetic payment provider response, synthetic GPS stream
```

without contaminating production data. This lets engineers test the entire delivery lifecycle end-to-end.

---

# 38. Screen maps

## Customer app (Flutter)

```text
AUTH        Splash, Onboarding, Login, Register, OTP verify, Password recovery
DISCOVERY   Home, Search, Search results, Category, Store list
COMMERCE    Store, Product (sheet), Customization, Cart, Checkout
FULFILLMENT Confirmation, Order detail, Live tracking, Delivery details, Rating
ACCOUNT     Profile, Personal info, Addresses, Payments, Favorites,
            Notifications, Settings, Support
```

## Merchant web app

```text
Dashboard, Orders, Order detail, Menu, Product editor, Modifier editor,
Store settings, Hours, Promotions, Customers, Analytics, Finance,
Payouts, Team / staff, Support
```

## Rider app (Flutter)

```text
Home, Delivery offer, Active delivery, Pickup, Navigation,
Customer delivery, Completion, Delivery history, Earnings,
Performance, Profile, Support
```

## Admin console

```text
Overview, Live operations, Orders, Order detail, Customers, Merchants,
Merchant detail, Riders, Rider detail, Dispatch map, Payments, Refunds,
Payouts, Promotions, Zones, Support, Analytics, Content, Risk,
Audit logs, Platform settings
```

---

# 39. Production roadmap

The biggest mistake would be building all surfaces simultaneously.

## V0 — Foundation

> Establish the real architecture.

Monorepo, customer app shell, Fastify API, PostgreSQL, Prisma, auth, design system, merchant/store/product domain, CI, environments, observability basics. No giant feature explosion.

## V1 — Real customer commerce

Customer: auth, home, search, stores, products, cart, checkout, **payment (MTN MoMo first)**, orders. Backend: users, stores, products, cart, orders, payments, notifications. Merchant interface initially narrow but managing **real** orders — no hardcoded demo behavior.

## V1.5 — Merchant operating product

Merchant onboarding, dashboard, order management, menu management, store availability, preparation times, finance visibility, basic analytics. **This is when Tuma stops being a customer app and becomes a real marketplace.**

## V2 — Real riders and dispatch

Rider onboarding, rider app, online/offline state, dispatch, real GPS, realtime customer tracking, delivery proof, rider earnings, support workflows. **The most important operational milestone.**

## V2.5 — Operational intelligence

ETA accuracy metrics, dispatch performance, merchant SLA monitoring, rider supply heatmaps, delivery zones, cancellation analytics, refunds/disputes, stronger admin tools.

## V3 — Growth engine

Promotions, loyalty, referrals, subscriptions, personalized recommendations, merchant campaigns, deeper analytics, sponsored listings.

## V4 — Commerce platform

Expand beyond food carefully: Food → Convenience → Groceries → Retail → Courier → B2B delivery. Expansion driven by demand and operational capability, not ambition alone.

## V5 — Network effects

Route optimization, rider batching, dynamic incentives, merchant demand forecasting, customer loyalty network, merchant CRM, first-party merchant ordering, delivery API, business accounts, multi-city orchestration.

## What NOT to build early

Microservices, AI recommendation engine, custom routing engine, nationwide coverage, complicated loyalty economics, multi-store baskets, advanced batching, fully autonomous dispatch, giant data warehouse, dozens of payment providers.

The first battle is:

```text
Can we consistently move an order from a customer to a merchant to a rider
and back to the customer with excellent UX and reliable operations?
```

---

# 40. Success metrics

## MVP metrics

| Customer | Merchant | Rider | Platform |
|---|---|---|---|
| browse → checkout conversion | acceptance rate | acceptance rate | contribution margin per order |
| order success rate | prep time | pickup wait time | payment success |
| repeat purchase rate | cancellation rate | delivery completion | refund rate |
| support contact rate | repeat customer rate | average delivery time | dispatch success |
| ETA accuracy | merchant retention | rider retention | on-time delivery |

## North-star metrics (eventually)

- **Successful delivered orders** within promised service level
- **Customer repeat rate**
- **Merchant retention** because Tuma works for their business
- **Contribution margin per delivery**
- **ETA reliability** — what Tuma promises is close to what customers experience

---

# 41. Competitive moat and flywheel

```text
                 TUMA MOAT
   CUSTOMER TRUST — MERCHANT — DELIVERY NETWORK
                 — DATA LOOP — OPERATIONS
                 — LOCAL PAYMENTS — LOCAL ADDRESS DATA
```

Each delivered order improves the system:

```text
more orders → more delivery data → better ETA → better dispatch
→ lower delivery cost → better merchant/customer experience → more orders
```

The commerce flywheel:

```text
MORE CUSTOMERS → MORE ORDERS → MORE MERCHANT DATA → BETTER MERCHANT INSIGHTS
→ BETTER MERCHANT RETENTION → MORE QUALITY MERCHANTS → BETTER CUSTOMER
SELECTION → MORE CUSTOMERS
```

The delivery flywheel:

```text
MORE ORDERS → MORE RIDER DENSITY → SHORTER DISPATCH DISTANCE → BETTER ETA
→ BETTER CUSTOMER TRUST → MORE ORDERS
```

This is why operations eventually become a moat.

---

# 42. Testing strategy

## Unit tests

Price calculations, promotion eligibility, order transitions, dispatch scoring, ETA calculations, permissions, refund calculations.

## Integration tests

Create order, payment, merchant acceptance, rider assignment, delivery completion, refund.

## End-to-end

The critical journey:

```text
Customer → store → product → cart → checkout → payment → order
→ merchant → rider → delivery
```

## QA acceptance philosophy

A feature is not complete because the screen exists. It is complete when **UI + state + API + errors + loading + empty states + permissions + analytics + accessibility + tests** are accounted for.

---

# 43. AI agent operating rules

Agents are implementation partners, not product owners.

1. Read this blueprint before making architectural decisions.
2. Inspect the existing code before adding patterns.
3. Do not create new state management patterns without justification.
4. Do not add dependencies without a concrete reason.
5. Do not change domain rules from the UI layer.
6. **Never trust client-provided prices or permissions.**
7. Do not invent APIs when the backend contract already exists.
8. Write tests for critical business rules.
9. Prefer simple code over impressive abstraction.
10. Do not rewrite unrelated areas during feature work.

## Founder control model

The founder personally owns: product vision, UX, visual design, architecture, critical domain logic, security decisions, financial logic, state machines.

Agents accelerate: component implementation, CRUD screens, API plumbing, tests, refactors, boilerplate, type generation, documentation.

The founder should be able to explain every critical workflow even when an agent wrote much of the implementation.

---

# 44. Environments and infrastructure progression

## Environments

```text
dev • staging • production
```

Never point local development at production data. Seed/demo data belongs in development and staging fixtures — **not in the production business model**. Production starts with real merchant/customer/rider data and controlled onboarding.

## Infrastructure progression

**Early production:** mobile apps → CDN/edge → Fastify API → PostgreSQL → Redis → object storage.

**Growing production:** load balancer → API replicas → PostgreSQL primary + backups → Redis → worker processes.

**Larger scale** (only when measured needs justify): modular monolith → extract high-load services → specialized dispatch/realtime/search infrastructure.

---

# 45. Build order for a solo founder

```text
1.  Product/design system          9.  Merchant order operations
2.  Backend foundation             10. Notifications
3.  Identity + users               11. Rider system
4.  Merchant/store/catalog         12. Dispatch
5.  Customer discovery             13. Realtime tracking
6.  Cart + pricing                 14. Admin operations
7.  Orders                         15. Analytics / promotions / growth
8.  Payments
```

At every stage: **Design → implement → integrate → test → observe → refine.** Do not move on simply because a screen "looks finished."

---

# 46. Definition of production readiness

Tuma is not production-ready when all screens exist. It is production-ready when:

**Customer** — can discover real merchants, create a real order, pay through supported providers, receive accurate order state, see delivery progress, get help, receive/refund money correctly.

**Merchant** — can onboard, manage menu, accept/reject orders, manage availability, see earnings, resolve normal operational issues.

**Rider** — can onboard, go online, receive assignments, navigate pickup/delivery, send reliable location, complete deliveries, see earnings.

**Platform** — can monitor everything important, investigate failures, handle refunds, manage merchants/riders, has audit logs, backups, observability, security controls.

**Engineering** — critical flows have automated tests, deployments are repeatable, environments are separated, secrets are protected, data is recoverable, incidents can be investigated.

---

# 47. The ultimate goal

The first release of Tuma should be a **small but real delivery network**.

Not a fake demo. Not a collection of screens. Not a clone.

A real system where:

```text
real customer → real merchant → real payment → real order
→ real rider → real GPS → real delivery → real money settlement
```

The architecture must be capable of becoming a broader East African commerce and logistics platform, but the first operating territory remains narrow enough for a solo founder to understand and control.

The strategic goal is not to out-feature every competitor on day one. It is to build a **better system in one city**, learn from every order, improve the customer/merchant/rider loops, and use that operating knowledge to expand.

## The product loop — the actual product

```text
"I know what I want."        → Tuma makes discovery easy.
"I know what it costs."      → Tuma makes checkout clear.
"I know what is happening."  → Tuma makes fulfillment visible.
"I know where my delivery is." → Tuma makes logistics tangible.
"Something went wrong."      → Tuma explains and resolves it.
"That was easy."             → Customer orders again.
```

---

# 48. Source notes

Competitive positioning was informed by current public materials from Vuba Vuba and merchant-platform references from DoorDash, including Vuba Vuba's positioning around food, retail, courier, real-time tracking, merchants, riders, and multiple payment methods, plus DoorDash's public merchant capabilities around orders, menus, availability, campaigns, reporting, and operational communication.

Technical baseline notes were checked against current official documentation for Flutter, flutter_map, PostgreSQL/PostGIS, and MTN payment APIs that list Rwanda support.

## Source URLs

- Vuba Vuba Rwanda — https://www.vubavuba.rw/
- Vuba Vuba Africa platform — https://vubavuba.africa/vubafrica/platform.php
- Vuba Vuba services — https://vubavuba.africa/vubafrica/services.php
- DoorDash Merchant Business Manager — https://help.doordash.com/en-us/merchants/article/business-manager-app
- DoorDash Merchant Portal — https://merchants.doordash.com/en-us/products/merchant-portal
- DoorDash Customer Analytics — https://help.doordash.com/en-us/merchants/article/customer-analytics
- Flutter — https://docs.flutter.dev/
- flutter_map — https://docs.fleaflet.dev/
- Riverpod — https://riverpod.dev/
- MTN MoMo APIs — https://momo.mtn.com/api/
- MTN Payments V1 — https://developers.mtn.com/products/payments-v1

---

# END OF SOURCE-OF-TRUTH BLUEPRINT
