# TUMA
## Everything you crave, delivered.

# FLUTTER MOBILE APP — PRODUCT, DESIGN & ENGINEERING SPECIFICATION

**Customer ordering app • Rider app foundation • production UI for the Tuma platform**

| This is the source of truth for building the Tuma Flutter apps. Engineers and coding agents must follow this spec to build a pixel-perfect, production-grade app. **The Master Blueprint (v2.0, production system vision) is the parent document** — this spec describes how the Flutter client implements it. The seeded/simulated mode described here is the **development and test-harness mode**; production mode speaks the real platform API (auth, catalog, orders, payments, realtime tracking). The UI is built once and must never need rewriting when the data source switches. |
|---|

**Version 1.1 • August 2026 — aligned with Master Blueprint v2.0 (production vision: real backend, real payments, real riders/dispatch). Full Flutter spec revision (rider app, API client, realtime tracking integration) to follow.**

> **V1 note:** the backend the app speaks to is built in **Rust** (Axum + SQLx + PostgreSQL), and the API contract is the small one defined in **`Tuma_V1_Brief.md`** — that one-page brief is the working source of truth for V1. Real delivery tracking (real GPS from the rider's phone, drawn on the real map — never simulated) is a **V1 feature**. The `TrackingState` shape below is what the backend's `GET /orders/:id/tracking` returns.
**Stack: Flutter stable + Dart 3.13 + Riverpod + GoRouter + flutter_map (+ Hive as client cache only in production mode)**

---

# 0. How to use this document

This spec is a build manual. Tuma is a food marketplace with last-mile delivery. The hard part is not page count — it is keeping product decisions, UX flow, domain logic, and state aligned while running fully offline.

**Build order:**

1. Product thesis + strategy (Ch 1-2) — why we build this way
2. UX principles + navigation (Ch 3) — what the user feels
3. Visual system + components (Ch 4) — how it looks (exact tokens from code)
4. Flutter architecture + structure (Ch 5-6) — how it is built
5. Domain model + tracking engine (Ch 7-9) — what it computes (minimal, as shipped)
6. Screen-by-screen + seed data (Ch 10-11) — what to ship
7. Roadmap + quality gates + agent rules (Ch 12-14) — when you are done

### Non-negotiable product truth

```
CUSTOMER EXPERIENCE
        ↓
DISCOVERY → DECISION → ORDER → FULFILLMENT → TRUST → RETURN
        ↓
DOMAIN MODEL (minimal)
        ↓
PROVIDERS + LocalStore + seed → mock now → API later
        ↓
UI NEVER KNOWS THE SOURCE
```

> Every technical decision must protect that chain. Do not add complexity because it is interesting.

### What the two modes mean

**Development / test-harness mode** (how the app ships today, used for demos, QA, and agent work):

- `flutter run` with no env vars, no API keys, no server.
- All catalog comes from `lib/data/seed/` via Riverpod providers.
- Cart, orders, favorites, addresses persist via `LocalStore` (Hive).
- Delivery is a deterministic simulation (`TrackingSimulation` + `flutter_map`) with the same `TrackingState` shape the real backend emits.

**Production mode** (target — Master Blueprint v2.0):

- Same screens and providers; the provider seam switches from seed/Hive to the platform API (REST + WebSocket).
- Hive becomes a **cache/draft store only** — the server is the database of record for prices, order state, payment state, and delivery truth.
- `TrackingState` arrives over WebSocket from the delivery service (real rider GPS, server-computed ETA); the client never computes truth from raw coordinates.
- Payment is real (MTN MoMo first, then Airtel/card/cash workflow) — the client only initiates and renders provider state; it never declares payment success.
- The simulation remains available **only as a test harness**, gated out of production builds.

---

# 1. Product Definition

## 1.1 What Tuma is

A customer-facing commerce and delivery platform beginning with food ordering, built toward the full production system in Master Blueprint v2.0: real backend (Rust + PostgreSQL), real payments (cash first, MTN MoMo next), real merchants, real riders, real GPS tracking. This spec covers the **Flutter customer app** (and later the Flutter rider app). In the current milestone the app runs in development/test-harness mode (seeded catalog + simulated fulfillment) so the UX loop can be built and verified before the API lands; Merchant, Rider, Admin are seams, not screens yet.

## 1.2 Competitive reality

Vuba Vuba (Rwanda/Uganda) is food + retail/essentials + courier, 600+ merchants, live tracking, positioned as last-mile infrastructure. Tuma wins with clarity, visual quality, transparent status and trust — not by copying a restaurant list — while staying able to become a broader platform.

| Question | Answer |
|---|---|
| Launch with? | Food ordering + live tracking (simulated) |
| Architect for? | Food first; marketplace + logistics later |
| First user? | Mobile customer wanting fast, trustworthy meal |
| Hero experience? | Ordering effortless; delivery visible and believable |
| Feels different? | Clarity, food-forward polish, explicit pricing, smooth motion |
| Must not happen? | Broad features that make core ordering mediocre |

## 1.3 Product promise

At every moment the customer can answer: What can I order? What will it cost? When will it arrive? Where is it now? What happens next?

## 1.4 Brand personality

| Trait | Implication |
|---|---|
| Fast | Short flows, clear CTA, motion 200–350ms |
| Warm | Food photography, friendly copy |
| Trustworthy | Explicit fees, honest ETA with `~` |
| Modern | Strong type, whitespace, restrained surfaces |
| Local | RWF, Kigali stores and addresses |
| Ambitious | Premium polish, not a form |

---

# 2. Product Strategy

```
            TUMA
               │
  ┌────────────┼────────────┐
  │            │            │
CUSTOMER   OPERATIONS    PLATFORM
  │            │            │
Ordering   Merchant      Admin
Tracking   Rider         Finance
  │        Dispatch      Support
  └──── Phase 1 focus ────┘
```

| Level | Scope | Status |
|---|---|---|
| P0 | Customer app UX loop (dev/test-harness mode: seed + simulation) | **BUILD NOW** |
| P0.5 | Backend foundation + API client swap (real auth/catalog/orders/payments) | Master BP V0–V1 |
| P1 | Merchant web order lifecycle (not Flutter) | After customer commerce |
| P2 | Rider Flutter app + real GPS + dispatch | Master BP V2 |
| P3 | Admin/support/finance/analytics (web) | After operations exist |
| P4 | Retail/essentials/courier/multi-city | After PMF |

**Scope rule:** No rider network, dispatch AI, or merchant analytics before the customer ordering loop is excellent. The seeded/simulated mode exists to prove the UX loop fast — it is a harness, not the destination.

---

# 3. UX Principles

## 3.1 Journey is the architecture

```
DISCOVER → SELECT → UNDERSTAND → CUSTOMIZE → COMMIT → CONFIRM → TRACK → RECEIVE → REMEMBER
```

| Stage | Question | Goal |
|---|---|---|
| Discovery | What should I order? | Inspire, reduce search cost |
| Selection | Where from? | Compare stores quickly |
| Understanding | What am I buying? | Price/contents obvious |
| Customization | Can I make it mine? | Options without overwhelm |
| Commit | What will I pay? | Remove uncertainty |
| Confirmation | Did it work? | Immediate confidence (tracking starts) |
| Tracking | Where is it? | Time/progress tangible |
| Receipt | Complete? | Close the loop |
| Return | Order again? | Reorder/favorites effortless |

## 3.2 UX Laws

1. One dominant `PrimaryButton` per view (`minimumSize 52h`, `Radii.md`).
2. Price legible before CTA — `formatPrice` + breakdown rows before button.
3. Progressive disclosure — common path first; extras in sheet / `ExpansionTile`.
4. Sheets for context (add-ons, pickers); full screens for journeys (checkout, tracking).
5. No dead ends — every empty = `EmptyState` with icon + CTA.
6. Loading is a state — `CachedNetworkImage` placeholder + `LoadingShimmer`.
7. Errors guide action — `SnackBar` + `errorText` + `AlertDialog` confirms.
8. Animation = causality — add → `CartBar moveY elasticOut 450ms` + `Badge`.
9. Location/status trustworthy — `~ 14 min`, `1.2 km`, never fake seconds.
10. Accessibility — ≥40–48dp targets, tooltips, contrast 4.5:1.

## 3.3 Navigation

```
BOTTOM NAV (StatefulShellRoute.indexedStack — 4 branches, 5 destinations)
[Home] [Search] [Orders] [Profile] [Cart★]

FLOATING CartBar above nav (Stack positioned bottom 0)
Visible when cartItemCount > 0 — same data as Cart tab Badge
```

Dual affordance is intentional — keep both.

**Router (`lib/core/router/app_router.dart:26`):**

- `go_router: ^18.0.0`, `StatefulShellRoute.indexedStack` with `AppScaffold(navigationShell)` for `/home`, `/search`, `/orders`, `/profile`.
- Top-level routes (outside shell): `/`, `/onboarding`, `/login`, `/register`, `/verify-otp`, `/store/:storeId`, `/cart`, `/checkout`, `/orders/:orderId`, `/orders/:orderId/tracking`, `/profile/*`.
- Splash (`/`) decides: `authProvider != null ? /home : onboarding.done=='true' ? /login : /onboarding` (`splash_screen.dart:34`).

## 3.4 Screen Taxonomy

| Area | Route | File |
|---|---|---|
| Auth | `/`, `/onboarding`, `/login`, `/register`, `/verify-otp` | `features/splash`, `onboarding`, `auth` |
| Discovery | `/home`, `/search` | `features/home`, `search` |
| Commerce | `/store/:storeId` (sheet is not a route) | `features/store_detail` |
| Purchase | `/cart`, `/checkout` | `features/cart` |
| Fulfillment | `/orders`, `/orders/:orderId`, `/orders/:orderId/tracking` | `features/orders` |
| Account | `/profile`, `/profile/edit`, `/profile/addresses`, `/profile/payment-methods`, `/profile/favorites`, `/profile/settings`, `/profile/support` | `features/profile` |

---

# 4. Visual Design System

## 4.1 Color Tokens (`lib/core/theme/app_theme.dart:6`)

| Token | Value | Purpose | Dart |
|---|---|---|---|
| Primary (emerald) | `#0B6E4F` | Trust, nav, filled CTAs, selected chips | `AppColors.primary` |
| Accent (orange) | `#FF7A1A` | Energy, promo gradient, popular badge | `AppColors.accent` |
| Deep emerald | `#08412F` | CartBar bg (darker so white pops) | `_darkEmerald` in `cart_bar.dart:106` |
| Surface / Bg | `#F7F7F5` | Scaffold warm greige | `AppColors.surface` |
| Card | `#FFFFFF` | Cards, sheets, inputs | `Colors.white` |
| Text | `#1B1B1F` | Primary copy | `AppColors.neutralDark` |
| Muted | `#6B6B70` | Secondary, hint | `AppColors.mutedText` |
| Error | `#D64545` | Validation | `AppColors.error` |
| Border | `#ECECE8` | Card/divider/input | `AppColors.cardBorder` |
| OnPrimary | `#FFFFFF` | On emerald | `AppColors.onPrimary` |

Promo gradients (`promo_banner.dart:42`): `[accent #FFB25E]`, `[primary #14976F]`, `[#7A3B00 accent]` from top-rated stores.

**Theme wiring (`app_theme.dart:21`):**

```dart
ThemeData light = ThemeData(
  useMaterial3: true,
  colorScheme: ColorScheme.fromSeed(
    seedColor: AppColors.primary, primary: AppColors.primary,
    secondary: AppColors.accent, error: AppColors.error, surface: AppColors.surface),
  scaffoldBackgroundColor: AppColors.surface);
final poppins = GoogleFonts.poppinsTextTheme(base.textTheme);
final inter = GoogleFonts.interTextTheme(base.textTheme);
final textTheme = poppins.copyWith(
  bodyLarge: inter.bodyLarge, bodyMedium: inter.bodyMedium,
  bodySmall: inter.bodySmall, labelLarge: inter.labelLarge,
  labelMedium: inter.labelMedium, labelSmall: inter.labelSmall);
```

## 4.2 Typography (`app_theme.dart:34`)

| Role | Font | Size | Weight | Use |
|---|---|---|---|---|
| Display | Poppins | 32–34 | 800–900 | Splash "Tuma" |
| ScreenTitle | Poppins | 22–26 | 700–800 | Page headers |
| Body | Inter | 14–16 | 400–500 | Descriptions, rows |
| Meta | Inter | 12–13 | 500 | ETA, distance |
| Price | Poppins/Inter | 16–22 | 800–900 | Product/total |

## 4.3 Spacing & Shape (`app_constants.dart:3`, `app_theme.dart:58`)

| Token | Value | Use |
|---|---|---|
| Spacing | `xs4 sm8 md12 lg16 xl24 xxl32` | Padding/margin |
| Radii | `sm8 md12 lg16 xl24` | Cards 16, inputs 12, sheets 24, chips stadium |
| Elevation | Card 0 + border, CartBar 6 | `CardTheme elevation 0` |
| Durations | `fast200 normal350 slow600 searchDebounce300 otp30s` | Motion/debounce |
| Touch | `52h FilledButton`, `40×40 add`, `46 avatar` | Targets |
| Curves | `easeOutCubic`, `easeOutBack` (splash), `elasticOut` (cartBar) | — |

**Imagery:** `cached_network_image: ^3.4.1`, `placeholder ColoredBox(surface)`, `errorWidget storefront/fastfood primary`; promo watermark `local_offer 130 alpha 0.18`.

## 4.4 Components (`lib/shared/widgets/` + `features/*/widgets/`)

| Component | File | Contract |
|---|---|---|
| `PrimaryButton` | `shared/widgets/primary_button.dart:6` | `FilledButton.icon` `label onPressed icon color expand` → `52h Radii.md` (accent override in sheet) |
| `QuantityStepper` | `shared/widgets/primary_button.dart:37` | `quantity onChanged min max size32` → `_StepButton 40×40` |
| `AppScaffold` | `shared/widgets/app_scaffold.dart:8` | `Stack(navigationShell + CartBar)` + `NavigationBar` 5 dest inc. Cart `Badge` |
| `CartBar` | `features/home/widgets/cart_bar.dart:16` | Animated `darkEmerald Radii.xl elevation6 ValueKey(itemCount) moveY elasticOut 450ms`, `cartItemCountProvider + cartSubtotalProvider` → `/cart` |
| `StoreCard` | `features/home/widgets/store_card.dart:12` | Grid: `16/9` image, rating badge `neutralDark 0.85`, `w800` name, `category·km` muted, bolt ETA + fee → `haversineDistanceKm` → `/store/:id` |
| `PromoBanner` | `features/home/widgets/promo_banner.dart:52` | `PageView 0.92` 5s auto, `176h`, dots `24×6 primary`; tap → store |
| `CategoryChip` | `features/home/widgets/category_chip.dart:7` | `StadiumBorder 36h min64` `primary/white` selected |
| `ProductTile` | `features/store_detail/widgets/product_tile.dart:13` | `84×84 Radii.md` thumb, `POPULAR` accent 0.12, price `primary w900`, `filledTonal 40 primary add` + snackbar |
| `ProductDetailSheet` | `features/store_detail/widgets/product_detail_sheet.dart:27` | `maxHeight 0.88 surface Radii.xl` 210h hero + close `black54`, add-ons checklist, stepper, `accent Add to basket · RWF` `scale elasticOut` |
| `TrackingMap` | `features/orders/tracking/widgets/tracking_map.dart:17` | `FlutterMap` OSM `rw.tuma.demo`, route `generateRoute`, polylines full `0.25/3` + traveled `1.0/5`, pins `40` store `primary`/home `accent`, driver `48 white→primary rotated bearing` + recenter FAB |
| `EmptyState` | `shared/widgets/empty_state.dart` | Icon + title + message + CTA → `go('/home')` |
| `AuthTextField` | `features/auth/widgets/auth_text_field.dart` | Email/password fields |

**Rules:** `StatelessWidget`/`ConsumerWidget`; `Stateful` only for controllers; use `AppColors/Spacing/Radii/AppDurations`; cards `elevation 0 + border + antiAlias`; inputs `filled white border Radii.md`.

---

# 5. Flutter Stack

| Layer | Choice | Reason |
|---|---|---|
| Flutter | stable | Impeller + Material 3 |
| Dart | `^3.13.1` | Null safety, records |
| State | `flutter_riverpod: ^3.4.2` + `riverpod_annotation: ^4.0.6` + `riverpod_generator` | Compile-safe, codegen |
| Navigation | `go_router: ^18.0.0` | ShellRoute, deep links |
| Persistence | `hive_flutter: ^1.1.0` via `LocalStore` | Offline-first, zero setup |
| Maps | `flutter_map: ^8.3.1` + `latlong2: ^0.10.1` | No API key, OSM tiles |
| Location | `geolocator: ^14.0.3` | Future GPS (mock now) |
| Immutability | `freezed: ^4.0.0` + `json_annotation: ^4.9.0` + `build_runner` | Models |
| Fonts | `google_fonts: ^8.2.1` | Poppins + Inter |
| Images | `cached_network_image: ^3.4.1` | Cache + placeholder |
| Anim | `flutter_animate: ^4.5.2` | Declarative |
| Format | `intl: ^0.20.3` | RWF |
| Test | `flutter_test` + `fake_async: ^1.3.1` | Deterministic time |

---

# 6. Architecture

```
Flutter App
  Presentation (ConsumerWidget pages + go_router)
        ↓ watches
  Application (Riverpod Notifiers: cartProvider, ordersProvider, trackingSimulationProvider, authProvider, addresses/favorites)
        ↓ reads
  Data (LocalStore Hive + seed kSeed* + utils: route/formatters/distance)
```

**Principle:** Screens `ref.watch(provider)`; providers own logic and persistence. Seed is read by providers only (widgets never import `seed_*` except via provider seam — direct `kSeed*` read in current home/search/store screens is grandfathered but new code must go through providers).

### Project Structure (`lib/`)

```
lib/
├── main.dart                         # LocalStore.init(), ProviderScope(override), TumaApp router
├── core/
│   ├── constants/app_constants.dart  # Spacing Radii AppDurations TrackingConfig(1s/80/4/5/1/22kmh/0.15) PromoCodes(TUMA10:10&WELCOME20:20)
│   ├── router/app_router.dart        # GoRouter + StatefulShellRoute
│   ├── storage/local_store.dart      # Hive boxes
│   ├── theme/app_theme.dart          # AppColors + AppTheme.light
│   └── utils/{formatters,distance_utils,route_utils}.dart
├── data/
│   ├── models/{store,product,order,cart_item,address,user,driver}.dart  # Freezed + json
│   └── seed/{seed_stores,seed_products,seed_addresses,seed_user}.dart
├── features/
│   ├── splash/splash_screen.dart
│   ├── onboarding/onboarding_screen.dart
│   ├── auth/{login,register,otp,providers/auth_provider}
│   ├── home/{home_screen,widgets/{store_card,promo_banner,category_chip,cart_bar}}
│   ├── search/{search_screen}
│   ├── store_detail/{store_detail_screen,widgets/{product_tile,product_detail_sheet}}
│   ├── cart/{cart_screen,checkout_screen,providers/cart_provider,widgets/{cart_item_tile,address_picker_sheet,payment_method_sheet}}
│   ├── orders/{orders_list_screen,order_detail_screen,providers/orders_provider,tracking/{tracking_screen,widgets/{tracking_map,eta_card,status_timeline},services/tracking_simulation_service}}
│   └── profile/{profile_screen,addresses,payment_methods,favorites,settings,support,edit,providers/profile_providers}
└── shared/widgets/{app_scaffold,primary_button,empty_state,loading_shimmer}
```

### State Ownership

| State | Owner | Persist | Single source |
|---|---|---|---|
| Auth | `authProvider` | LocalStore | `User` |
| Cart | `cartProvider: List<CartItem>` | LocalStore | `cartItemCountProvider`, `cartSubtotalProvider` derived |
| Orders | `ordersProvider: List<Order>` | LocalStore | `ordersProvider` |
| Favorites | `favoritesProvider: Set<String>` | LocalStore | ids |
| Addresses | `addressesProvider: List<Address>` | LocalStore | default = first `isDefault` |
| Tracking | `trackingSimulationProvider: TrackingState?` | memory | `TrackingSimulation` |
| Search | `recentSearchesProvider + local _query + debounce 300` | LocalStore | recents |

---

# 7. Domain Model (minimal — no extra schemas)

Only what is shipped. Fewer files, fewer states.

```
User ──< Address
Store ──< Product (addons inline)

Cart = List<CartItem {product, quantity, addons, lineTotal}>
Order {storeId, storeName, items: List<CartItem>, subtotal, deliveryFee, discount, total,
       address, paymentMethod, promoCode?, status, createdAt}
Favorite = Set<storeId>
```

| Entity | File | Key fields |
|---|---|---|
| `User` | `data/models/user.dart` | `id, name, email, phone, avatarUrl?` — demo `kDemoUser` |
| `Address` | `data/models/address.dart` | `id, label, street, details?, city, latitude, longitude, isDefault` |
| `Store` | `data/models/store.dart` | `id, name, description, imageUrl, category, rating, ratingCount, deliveryFee double, minDeliveryMinutes, maxDeliveryMinutes, latitude, longitude` |
| `Product` | `data/models/product.dart:19` | `id, storeId, name, description, imageUrl, price double, category (section), addons: List<ProductAddon>, isPopular` |
| `ProductAddon` | `product.dart:6` | `id, name, price double` |
| `CartItem` | `data/models/cart_item.dart` | `product, quantity, addons, lineTotal = (price+add-ons)*qty` |
| `Order` | `data/models/order.dart:41` | `id, storeId, storeName, items, subtotal, deliveryFee, discount, total, address, paymentMethod, promoCode?, status, createdAt` |
| `PaymentMethod` | `order.dart:26` | `id, type {cash, card}, displayName, last4?` — default `cash "Cash on delivery"`. **Production:** provider references only (MTN MoMo, Airtel Money, card token, cash) — never sensitive credentials; see Master BP Ch 11 |
| `Favorite` | `profile_providers` | `Set<String> storeIds` |

Helpers: `formatPrice(double)`, `haversineDistanceKm(LatLng,LatLng)`, `generateRoute/remainingDistanceKm/bearingDegrees/etaMinutes`.

### Order Status — 5 states only (`data/models/order.dart:9`)

```dart
enum OrderStatus { placed, preparing, pickedUp, onTheWay, arrived }
// label: Order Placed, Preparing, Picked Up, On the Way, Arrived
// isActive = status != arrived
```

```
placed (4 ticks parked) → preparing (5 ticks parked) → pickedUp (1 tick) → onTheWay → arrived
```

No `confirmed`, `readyForPickup`, `nearby`, `cancelled`, `paymentFailed` **in harness mode**. Keep 5 states until real operations require more — then extend behind provider, not by adding tables.

**Production note:** the full order state machine (Master BP Ch 10) is `DRAFT → PENDING_PAYMENT → PAID → PLACED → MERCHANT_ACCEPTED → PREPARING → READY_FOR_PICKUP → RIDER_ASSIGNED → PICKED_UP → IN_TRANSIT → ARRIVED → DELIVERED` plus exception states (`REJECTED`, `CANCELLED`, `PAYMENT_FAILED`, `REFUND_PENDING`, `REFUNDED`, `DELIVERY_FAILED`, `CUSTOMER_UNREACHABLE`). The 5-state enum here is the collapsed customer-timeline view; when the API lands, map server states onto this timeline behind the provider rather than rewriting screens.

**Immutability:** `Order` snapshots `storeName` + `items`; totals `subtotal - discount + deliveryFee` are recomputed at `placeOrder` and never derived later from catalog. PromoCodes `TUMA10=10%`, `WELCOME20=20%` case-insensitive.

---

# 8. Data & Persistence

- `LocalStore` (`core/storage/local_store.dart`) wraps Hive boxes: auth, cart, orders, addresses, favorites, recents, `onboarding.done`.
- `cartProvider` is `List<CartItem>`; helpers `cartItemCountProvider`, `cartSubtotalProvider`, `deliveryFeeFor(max fee)`, `discountFor(subtotal, code)`. Mixed stores (`storeIds >1`) show banner; checkout orders only first store's items.
- `ordersProvider.placeOrder(store, address, paymentMethod, items, promo)` builds `Order(id, ... status=placed, createdAt=now, driverName='Eric M.')` and persists; also `updateStatus(id, status)` called by tracking.
- `addressesProvider.defaultAddress` + `favoritesProvider.toggle(id)` + `recentSearchesProvider.record/clear` all Hive-backed.
- No flavor file (`main_mock.dart`/`main_prod.dart`) — `main.dart` is the single entry; API is future.

**Production positioning:** in production mode Hive is a **cache/draft store only** (cart draft, UI preferences, cached catalog for offline tolerance). The server is the database of record for prices, order state, payment state, and delivery truth. Providers read the API client first and fall back to cache; they never treat cached values as authoritative for money or status.

---

# 9. Delivery Tracking — hero system (as built)

> **Production positioning (Master BP Ch 15):** this simulation is the **test-harness** implementation of tracking. In production, the rider app publishes real GPS to the realtime gateway, the delivery service validates and computes ETA, and the customer app receives the **canonical server tracking state** over WebSocket — the same `TrackingState` shape below. `TrackingMap`, `EtaCard`, and `StatusTimeline` render either source unchanged; only the provider's data source switches. The client never computes delivery truth from raw coordinates in production.

## 9.1 Goal

User perceives a rider moving toward them. One `TrackingState` drives map, ETA, timeline, order status.

```
Timer 1s + generateRoute(80 Bézier) → TrackingState{driverPosition, bearingDegrees, remainingDistanceKm, etaMinutes, progress, status}
                                       ↓
                                   TrackingMap + EtaCard + StatusTimeline + ordersProvider sync
```

## 9.2 Route

`route_utils.generateRoute(origin, destination)` — quadratic Bézier offset `0.15` perpendicular to straight line, sampled `routeSteps 80`. Origin = store `LatLng`, dest = address `LatLng` (`tracking_screen.dart:73`). Kigali stores lie `-1.93..-1.96 / 30.05..30.10`. No separate `DeliveryRoute` model — just `List<LatLng>`.

## 9.3 Simulation (`features/orders/tracking/services/tracking_simulation_service.dart:14`)

```dart
@freezed abstract class TrackingState with _$TrackingState {
  const factory TrackingState({
    required LatLng driverPosition, required double bearingDegrees,
    required double remainingDistanceKm, required int etaMinutes,
    required double progress, required OrderStatus status}) = _TrackingState;
}
@riverpod class TrackingSimulation extends _$TrackingSimulation {
  Timer? _timer; List<LatLng> _route=[]; String? _orderId; int _waypointIndex=0;
  TrackingState? build(){ ref.onDispose(_cancelTimer); return null; }
  void start(Order order){
    _route = generateRoute(storeLatLng, addressLatLng); _waypointIndex=0; var tick=0;
    _timer = Timer.periodic(TrackingConfig.tickEvery, (t){
      tick++;
      if (tick <= 4) _emit(0, placed);
      else if (tick <= 9) _emit(0, preparing);
      else _advance();
    });
  }
  void _advance(){ _waypointIndex++; if (_waypointIndex >= _route.length-1){ _emit(last, arrived); _cancelTimer(); return;} _emit(_waypointIndex, _waypointIndex<=1? pickedUp : onTheWay); }
  void _emit(int i, OrderStatus s){ state=TrackingState(driverPosition:_route[i], bearingDegrees: bearing(i-1,i), remainingDistanceKm: remaining(_route,i), etaMinutes: eta(...), progress:i/79, status:s); if (s != _synced) ref.read(ordersProvider.notifier).updateStatus(_orderId!, s); }
}
```

Config (`app_constants.dart:34`): `tickEvery 1s`, `routeSteps 80`, `placedTicks 4`, `preparingTicks 5`, `pickedUpTicks 1`, `averageSpeed 22 km/h`, `bezier 0.15`.

**Provider contract:** `trackingSimulationProvider` is `TrackingState?`. `TrackingScreen.initState` calls `start(order)` once if `state==null && order.isActive`; for delivered orders emits terminal `arrived`. `ref.watch` drives map/cards. Camera follows driver only while `pickedUp/onTheWay` (`tracking_map.dart:45`).

## 9.4 Map Composition (`tracking_screen.dart:19`, `tracking_map.dart:17`)

```
AppBar: storeName
flutter_map flex:5
  TileLayer OSM userAgent rw.tuma.demo
  PolylineLayer: full route primary 0.25 /3 + traveled primary /5 cut at progress*80
  MarkerLayer: store pin 40 primary, home pin 40 accent, driver 48 white ring → primary with delivery_dining, rotated bearing
  FAB small white recenter → move(driver,15)
  EtaCard overlay centered top
Bottom white container Radii.xl top:
  StatusTimeline(current: order.status) fast/normal
  DriverRow: CircleAvatar primary 0.12 "Eric M. · RA 123 AB" + star 4.9 + disabled call/chat
  Divider + OrderSummary ExpansionTile (items qty× name + price, subtotal/fee/discount/total)
```

`FlutterMap` uses `initialCameraFit: CameraFit.coordinates(_route, padding xl)` and midpoint `initialCenter`.

---

# 10. Screen-by-Screen Spec

Every screen handles `loading` (placeholder), `empty` (`EmptyState`), `error` (snackbar/errorWidget).

## 10.1 Splash `/` (`splash/splash_screen.dart:15`)

`AppColors.primary` bg, `112×112 white Circle delivery_dining 64 primary`, "Tuma" `displaySmall w800 white`, "Hot food, fast." `bodyMedium white 0.85`, `fadeIn slow600 + scale 0.8→1 easeOutBack`, `Timer 1200` → routing above.

## 10.2 Onboarding `/onboarding` (`onboarding_screen.dart:13`)

3 pages: Order in few taps `restaurant_menu_rounded`, Track live `location_on_rounded`, Fast delivery `bolt_rounded`; `200×200` circle `primary 0.10 even / accent 0.12 odd` + icon 96; dots `24×8 primary / 8 cardBorder`; `PrimaryButton Next/getStarted arrow_forward`; persists `onboarding.done`; Skip top-right → `/login`.

## 10.3 Login `/login` (`auth/login_screen.dart:13`)

AppBar "Welcome back", `headlineMedium w800 "Sign in to order"`, muted body, `AuthTextField.email/password`, demo box `primary 0.08 + 0.2 border "Demo — kDemoUser.email / kDemoPassword"` fade fast, `PrimaryButton Sign In` → `validateLogin` snackbar error or `auth.login(kDemoUser)→go('/home')`, row "New here? Create an account" → `/register`.

**Demo:** `kDemoUser` / `$kDemoPassword` in `seed_user.dart`.

## 10.4 Home `/home` (`home/home_screen.dart:23`)

`surface SafeArea(bottom false) CustomScrollView`:
- Header Row: `46 circle primary avatar` + "Deliver to" `labelSmall muted` + `label·street titleMedium w800` + `Badge accent cartCount` bag → `/cart`
- Search `InkWell → /search` white `Radii.xl border cardBorder` search icon + "Search restaurants and dishes" muted
- Chips `SizedBox40 ListView.horizontal` `['All', ...categories]` `CategoryChip Stadium 36h`
- `PromoBanner` `PageView 0.92 5s 176h` gradients, dots `20 primary`
- "Stores near you" `titleLarge w900`
- Grid `maxCrossAxisExtent360 spacing lg aspect0.78` `StoreCard(distance = haversine defaultAddress)` `animate delay60 fade+slideY0.12`, pad bottom 96

## 10.5 Search `/search` (`search/search_screen.dart:20`)

AppBar TextField `autofocus search_rounded hint "Search stores or dishes…" clear close_rounded`, debounce `300ms` → `_query` + `recentSearches.record`. `AnimatedSwitcher fast` between idle (recents Wrap `history` ActionChip + Clear + popular 8 `local_fire accent border`) and results (filter stores `name/category` + products `name/description`; empty → `EmptyState search_off`; else "Stores" `_StoreHitCard 64×64 Radii.sm` → `/store/:id` + "Dishes" `SearchResultTile` → sheet). Chip `_searchNow` bypasses debounce.

## 10.6 Store Detail `/store/:storeId` (`store_detail/store_detail_screen.dart:19`)

`Stack CustomScrollView + CartBar`. `storeById` else not found. `SliverAppBar 220 pinned FlexibleSpaceBar CachedNetworkImage + gradient` actions favorite `favorite/_border error/white` + `Badge cart + bag white`. Header pad lg: `headlineSmall w900` name + `bodyMedium muted` desc + `Wrap _InfoChip star/rating access_time ETA delivery_dining fee restaurant category`. Per `product.category` group: header `titleLarge w800` + `SliverList ProductTile pad lg/xs onOpen→showProductDetailSheet`. Bottom 96.

## 10.7 Product presentation

- `ProductTile:13`: `84×84 Radii.md` thumb, `POPULAR accent 0.12 Radii.sm`, desc 2 lines muted, `price primary w900`, `IconButton.filledTonal 40 primary add` → snackbar.
- `ProductDetailSheet:27` `showModalBottomSheet isScrollControlled useSafeArea transparent maxHeight0.88 surface Radii.xl 210h hero + CloseButton black54`, pad xl: `headlineSmall w900` name + muted desc + "Add extras" `titleMedium w800` add-on checkboxes (22 circle check accent) + `Quantity row QuantityStepper32` + `accent Add to basket · RWF` `scale ValueKey(total) elasticOut`.

## 10.8 Cart `/cart` (`cart/cart_screen.dart:40`)

`promoCodeProvider keepAlive ''`. Empty → `EmptyState → go('/home')`. List: mixed banner accent 0.12 + AlertDialog clear if `storeIds>1`? Items `CartItemTile`. Promo `TextField characters "Promo code" hint "Try TUMA10 or WELCOME20" invalid→"Invalid promo code"`. Summary `Subtotal / Discount(UPPER) primary / Delivery fee / Divider / Total primary bold w800` + `PrimaryButton Checkout · RWF arrow_forward → /checkout`.

## 10.9 Checkout `/checkout` (`cart/checkout_screen.dart:21`)

Empty → `EmptyState → go('/home')`. Picks `defaultAddress` + `orderItems` first store only + `subtotal/discount/deliveryFee` total. `_SectionCard surfaceContainerLowest Radii.md cardBorder`: icon 20 primary + `titleSmall w800` + Change button. Sections: Deliver to (`label·street,details\ncity` or error + choose), Pay with (`payments/credit_card displayName w600` default cash), Order summary (rows `qty × name` + Divider + _Row). Dropped banner accent if mixed. Bottom `PrimaryButton placing?"Placing order…":"Place Order · RWF"` disabled `_placing||address==null` → `ordersProvider.placeOrder(... validPromo upper? null) → cart clear promo '' → pushReplacement /orders/:id/tracking`. Pickers `AddressPickerSheet.show`/`PaymentMethodSheet.show`.

**Production note:** checkout is where server authority takes over — the server recalculates the basket and locks prices before payment; payment runs through the provider layer (MTN MoMo first, with a `REQUIRES_ACTION` "check your phone" UI), and the client never declares payment success. Cash-on-delivery remains a supported method (Master BP Ch 11).

## 10.10 Orders `/orders`, `OrderDetail`

List `ordersProvider`. Detail shows `StatusTimeline + address + payment + items + totals`. Empty after first real order only. `Tracking` entry from list goes to `/orders/:id/tracking`.

## 10.11 Tracking `/orders/:orderId/tracking` (`tracking/tracking_screen.dart:19`)

`AppBar storeName`. `Column Expanded flex5 Stack(TrackingMap origin/destination + Center EtaCard)` + bottom white `Radii.xl top` container: `StatusTimeline` + `_DriverRow Eric M. RA123AB 4.9 disabled icons` + `OrderSummary ExpansionTile #id`. `initState` `start(order)` if `isActive`. Map follows driver when moving, recenter FAB.

## 10.12 Profile `/profile` (`profile/profile_screen.dart:10`)

`surface AppBar Profile ListView lg`: header card `surface Radii.lg cardBorder Row CircleAvatar 32 primary initials w800 + name w800 + email muted + edit_outlined primary → /profile/edit`, menu card `location_on Addresses, credit_card PaymentMethods, favorite_outline Favorites, settings_outlined, help_outline Support` dividers `xl+lg`, logout card `ListTile logout error → auth.logout + go('/login')`. Other screens `addresses/payment_methods/favorites/settings/support/edit` are simple lists + EmptyState.

---

# 11. Seed Data

No hard-coded orders/notifications. Catalog + route are derived.

| Dataset | Value | File | Notes |
|---|---|---|---|
| Users | 1 demo | `seed_user.dart` | `kDemoUser` |
| Stores | 8 | `seed_stores.dart:4` | -1.93..-1.96 / 30.05..30.10 |
| Categories | 8 derived | `store.category` | Pizza Burgers FriedChicken Sushi Coffee&Dessert HealthyBowls AfricanGrill Shawarma |
| Products | 40+ | `seed_products.dart` | `isPopular` subset for search popular chips |
| Promo codes | 2 | `app_constants.dart:59` | `TUMA10 10% WELCOME20 20%` case-insensitive, `discountFor` |
| Promos UI | 3 derived | `promo_banner.dart:31` | Top-rated stores + copy + gradients |
| Routes | 80/step order | `route_utils.generateRoute` | Bézier 0.15, 22 km/h |

**Stores (`seed_stores.dart:4`):**

| Store | Category | ETA | Rating | Fee | Lat,Lng |
|---|---|---|---|---|---|
| Stone Oven Kigali | Pizza | 20–35 | 4.7 (1284) | 1500 | -1.9412,30.0619 |
| Nyamirambo Burger Lab | Burgers | 15–30 | 4.5 (962) | 1200 | -1.9558,30.0561 |
| Crispy Nest Chicken | Fried Chicken | 20–40 | 4.3 (751) | 1000 | -1.9487,30.0723 |
| Umuco Sushi Bar | Sushi | 35–50 | 4.8 (418) | 2500 | -1.9364,30.0944 |
| Kivu Bean Coffee House | Coffee & Dessert | 15–25 | 4.6 (1533) | 1000 | -1.9441,30.0878 |
| Green Hills Bowl Co. | Healthy Bowls | 25–40 | 4.4 (389) | 1800 | -1.9521,30.0665 |
| Inyama African Grill | African Grill | 30–45 | 4.9 (2107) | 2000 | -1.9589,30.0812 |
| Shawarma Corner KG Ave | Shawarma | 20–35 | 4.2 (654) | 1200 | -1.9395,30.0534 |

Images: Unsplash `w=640&h=480&fit=crop` with `errorWidget`.

---

# 12. Roadmap

Flutter milestones, mapped to the Master Blueprint production roadmap (V0–V5):

| Version | Goal | Included | Not included |
|---|---|---|---|
| v0.1 | Base | Theme, router shell, LocalStore, seeds, PrimaryButton etc. | Real data |
| v0.2 | Discovery | Home, Search, StoreDetail, sheet | — |
| v0.3 | Commerce | Cart, Checkout, Orders create | Dispatch |
| v0.4 | Tracking hero | flutter_map + TrackingSimulation + ETA/timeline | Real riders |
| v0.5 | Polish | Reorder, persist all, empty/loading/error states | Merchant |
| v0.6 | **API swap (Master BP V0–V1)** | Typed API client + auth (JWT) + catalog/cart/orders/payments over same providers; Hive demoted to cache; MTN MoMo payment flow (`REQUIRES_ACTION` UI) | Merchant ops |
| v0.7 | **Realtime tracking (Master BP V2)** | WebSocket `TrackingState` replaces simulation; rider app skeleton + `geolocator` publish | Advanced dispatch |
| v1.0 | Pilot | Real payments live, push notifications, monitoring, real rider tracking | Expansion |

**Done for v0.4 (harness):** User can cold-start → demo login → browse/search → open store → customize → add to cart (CartBar bounce) → checkout with address → place → watch rider on map with ETA/distance/timeline → arrive → history.

**Done for v1.0 (production):** the same journey runs against the real platform — real merchant catalog, server-recomputed totals, MTN MoMo payment verified by webhook, real rider GPS on the map, and help/refund paths reachable.

---

# 13. Quality Gates

| Gate | Pass | Verify |
|---|---|---|
| Visual | No clip, spacing ok, Poppins/Inter hierarchy, AppColors only | Golden on 360×800, 412×915 |
| Interaction | Every primary button routes/updates | Widget tap → expect |
| State | Cart/orders/tracking coherent | Add → cart → checkout → order → tracking `placed→arrived` |
| Perf | Scroll + map 60fps | `RepaintBoundary` map, `ListView.builder`, `const` |
| A11y | 40–48dp, semantics, contrast | TextScale 1.3 no overflow |
| Error | Image/form/empty graceful | Placeholder/errorWidget/EmptyState |
| Arch | No price calc outside providers | Grep no `lineTotal` in widgets |
| Test | Journey e2e | `integration_test/app_test.dart` |

**Critical e2e:**

```
Splash → Login(kDemoUser) → Home → /search → /store/:id → sheet add → CartBar → /cart → /checkout (pick address) → place → /orders/:id/tracking (tick 1s, progress→1, status→arrived) → /orders/detail → reorder
```

Unit: cart totals, promo discount, `generateRoute/remaining/bearing/eta`. Widget: StoreCard, ProductTile, EmptyState. Golden: Home, StoreDetail, Cart, Tracking.

# 14. Agent Rules

**May decide:** impl inside architecture, small refactors, which existing widget/provider to use, tests to keep behavior.

**May not:** new navigation shape, new product concepts beyond version, colors/typography change, moving state ownership, adding heavy dep for small feature, replacing Riverpod/GoRouter/Hive/flutter_map.

**Every task:**

```
CONTEXT — problem / user need
SCOPE   — screens + states (loading/empty/error/success)
DESIGN  — components used
STATE   — providers involved, inputs/outputs
ACCEPTANCE — testable behaviors (see Ch 10)
NON-GOALS — explicitly not to build
```

# 15. Design Review Checklist (per PR)

- [ ] Next action obvious in 2s? (single PrimaryButton)
- [ ] Total before CTA?
- [ ] Hierarchy CTA > price > desc > meta?
- [ ] ≤1 accent highlight? (emerald primary, orange promos only)
- [ ] Emerald/greige + food imagery + Poppins/Inter?
- [ ] States: loading placeholder, empty EmptyState, error snackbar/border, selected Stadium primary, disabled null?
- [ ] Shared widget reused?
- [ ] Confidence: explicit fee, `~` ETA, polyline + Timeline?
- [ ] `const` where possible? `Stateful` only for controllers?
- [ ] Card `elevation 0 + border Radii.lg`?

# 16. Future Platform Seams

```
Seed(Hive harness) → API(Dio + typed client + same TrackingState) → MTN MoMo payments
→ Rider Flutter app (geolocator publish) → WS delivery events → push + observability
```

This mirrors the Master Blueprint production roadmap: V1 real customer commerce (API + payments), V2 real riders and dispatch, V2.5 operational intelligence. Events to preserve shape of: `delivery.position_changed`, `status_changed`, `eta_changed`, `completed`. Rider sends `geolocator` samples only while on an active delivery; the server validates ownership/state/plausibility, smooths, computes ETA, and broadcasts the canonical `TrackingState`. The harness already emits that exact shape via `TrackingState.status/progress` — that is the contract.

# 17. Build

```bash
flutter pub get
dart run build_runner build --delete-conflicting-outputs
flutter run
flutter analyze && dart format . && flutter test
flutter test --update-goldens
flutter build apk && flutter build ios
```

No `.env`. Map tiles OSM public `tile.openstreetmap.org` no key. To style tiles, swap `TileLayer` URL only.

---

# 18. Final Brief

> A customer opens Tuma and thinks: "I know what is available. I understand what I am buying. I know what I will pay. I know when it should arrive. And I can see what is happening."
>
> Engineering: "I can replace seeds with APIs, replace simulation with real WebSocket tracking, add real payments and a rider app — without rewriting the customer experience."

| BUILD THE CUSTOMER LOOP FIRST. KEEP THE DOMAIN SMALL. HARNESS WHAT DOES NOT EXIST YET. SHIP THE REAL NETWORK NEXT. |
|---|

