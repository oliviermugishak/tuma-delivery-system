import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:go_router/go_router.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'package:tuma_app/core/api/api_client.dart';
import 'package:tuma_app/core/api/models/authenticated_user.dart';
import 'package:tuma_app/core/auth/auth_controller.dart';
import 'package:tuma_app/core/auth/token_storage.dart';
import 'package:tuma_app/core/router/app_router.dart';
import 'package:tuma_app/core/theme/app_theme.dart';
import 'package:tuma_app/features/cart/cart_notifier.dart';
import 'package:tuma_app/features/home/app_shell.dart';
import 'package:tuma_app/features/location/customer_location.dart';
import 'package:tuma_app/features/rider/rider_screen.dart';

// ---------------------------------------------------------------------------
// Test doubles
// ---------------------------------------------------------------------------

class _FakeTokenStorage implements TokenStorage {
  @override
  Future<String?> readToken() async => null;
  @override
  Future<void> writeToken(String token) async {}
  @override
  Future<void> clear() async {}
}

/// Session already signed in as a customer, so the router stays on the
/// shell and each tab is reachable without the OTP flow.
class _SignedInSession extends SessionNotifier {
  @override
  Future<SessionState> build() async => SessionState.user(
        AuthenticatedUser(
          id: 'user-1',
          phone: '+250780000001',
          customer: ProfileInfo(id: 'customer-1', name: 'Chantal'),
        ),
      );
}

/// Session already signed in as a rider (admin-created, no customer
/// profile) — the router must land on rider mode, never the customer shell.
class _RiderSession extends SessionNotifier {
  @override
  Future<SessionState> build() async => SessionState.user(
        AuthenticatedUser(
          id: 'user-2',
          phone: '+250780000002',
          rider: RiderInfo(
            id: 'rider-1',
            riderNumber: 7,
            name: 'Jean',
            isActive: true,
          ),
        ),
      );
}

// Canned responses, shaped exactly like the server's OpenAPI contract.
final _store = {
  'id': 'store-1',
  'merchant_id': 'merchant-1',
  'name': "Aline's Kitchen",
  'description': 'Fresh food, fast.',
  'image_url': null,
  'address_text': 'KN 4 Ave, Kigali',
  'lat': -1.9512,
  'lng': 30.0623,
  'category': 'Grill',
  'delivery_fee': 1500,
  'is_open': true,
  'created_at': '2026-08-28T06:55:45Z',
  'updated_at': '2026-08-28T06:55:45Z',
};

final _menuItem = {
  'id': 'menu-1',
  'name': 'Ibirazi',
  // Long enough to overflow a non-scrolling sheet on the test surface —
  // the product-sheet regression test depends on it overflowing.
  'description':
      'Slow-cooked rice and beans the Aline way — bay leaves, a whisper of '
          'palm oil, and a full hour over low heat. ' *
      8,
  'price': 3500,
  'image_url': null,
  'is_available': true,
};

Map<String, dynamic> _group({
  String id = 'group-1',
  String status = 'completed',
  double? addressLat,
  double? addressLng,
}) =>
    {
      'id': id,
      'number': 1042,
      'address_text': 'KN 4 Ave, Kigali',
      'address_lat': addressLat,
      'address_lng': addressLng,
      'subtotal': 7000,
      'delivery_total': 1500,
      'grand_total': 8500,
      'status': status,
      'payment_status': 'pending',
      'created_at': '2026-08-28T07:00:00Z',
      'store_orders': [
        {
          'id': 'so-1',
          'number': 1043,
          'store_id': 'store-1',
          'store_name': "Aline's Kitchen",
          'status': status == 'completed' ? 'delivered' : status,
          'subtotal': 7000,
          'delivery_fee': 1500,
          'total': 8500,
          'items': [
            {
              'store_product_id': 'menu-1',
              'product_id': 'product-1',
              'product_name': 'Ibirazi',
              'unit_price': 3500,
              'quantity': 2,
            },
          ],
        },
      ],
    };

http.Response _json(Object body, [int status = 200]) => http.Response(
      jsonEncode(body),
      status,
      headers: {'content-type': 'application/json'},
    );

/// A tracking snapshot shaped exactly like GET /orders/{id}/tracking —
/// the customer map world's data. One delivery per store order, joined to
/// the detail stub's `so-1`. `changedAt` is what the client echoes back
/// as `since`; `riderLat/riderLng` null means no check-in yet. The
/// run's timestamps default to RELATIVE-TO-NOW (ETA +15 min, last signal
/// 30s ago) so the truth ladder is deterministic: pass explicit values to
/// build lagging/ended worlds.
Map<String, dynamic> _tracking({
  String groupId = 'group-1',
  String groupStatus = 'in_progress',
  String paymentStatus = 'pending',
  String changedAt = '2026-08-28T07:05:00Z',
  String deliveryStatus = 'picked_up',
  double? riderLat = -1.9550,
  double? riderLng = 30.0623,
  String? routePolyline = '_p~iF~ps|U_ulLnnqC',
  DateTime? etaTarget,
  DateTime? lastLocationAt,
}) {
  final eta =
      etaTarget ?? DateTime.now().toUtc().add(const Duration(minutes: 15));
  final signal =
      lastLocationAt ?? DateTime.now().toUtc().subtract(const Duration(seconds: 30));
  return {
    'group_id': groupId,
    'group_status': groupStatus,
    'payment_status': paymentStatus,
    'changed_at': changedAt,
    'deliveries': [
      {
        'store_order_id': 'so-1',
        'store_name': "Aline's Kitchen",
        'store_lat': -1.9512,
        'store_lng': 30.0623,
        'store_contact_phone': '+250788000001',
        'status': deliveryStatus,
        'handoff_at':
            DateTime.now().toUtc().subtract(const Duration(minutes: 5))
                .toIso8601String(),
        'route_polyline': routePolyline,
        'eta_target': eta.toIso8601String(),
        'last_lat': riderLat,
        'last_lng': riderLng,
        'last_location_at':
            riderLat == null ? null : signal.toIso8601String(),
        'updated_at': DateTime.now().toUtc().toIso8601String(),
        'trail': [
          {
            'lat': -1.9580,
            'lng': 30.0930,
            'recorded_at': signal.toIso8601String(),
          },
          if (riderLat != null)
            {
              'lat': riderLat,
              'lng': riderLng,
              'recorded_at': signal.toIso8601String(),
            },
        ],
      },
    ],
  };
}

/// Real [ApiClient] backed by a scripted server. Every screen the test
/// visits gets its data from here — nothing touches the network. With
/// [seen], every request is captured for wire-level assertions. The
/// /stores stub mirrors the real server's contract: distance/eta only
/// when the request carried the customer's location.
ApiClient _apiClient({
  List<Map<String, dynamic>> groups = const [],
  List<http.Request>? seen,
  List<Map<String, dynamic>>? stores,
  List<String>? menuImages,
  Map<String, dynamic>? groupDetail,
  List<Map<String, dynamic>>? riderDeliveries,
  http.Response Function(http.Request request)? tracking,
}) {
  final feed = stores ?? [_store];
  final handler = MockClient((request) async {
    seen?.add(request);
    final path = request.url.path;
    final method = request.method;

    if (method == 'GET' && path.endsWith('/deliveries')) {
      // The rider kiosk's work list: empty by default in this harness.
      return _json(riderDeliveries ?? <Map<String, dynamic>>[], 200);
    }
    if (method == 'POST' && path.endsWith('/orders')) {
      return _json(_group(status: 'in_progress'), 201);
    }
    if (method == 'GET' && path.endsWith('/orders')) {
      return _json(groups, 200);
    }
    if (RegExp(r'/orders/[^/]+/tracking$').hasMatch(path) &&
        method == 'GET') {
      // Absent by default: the detail screen takes its documented
      // "tracking unavailable" fallback (group statuses only).
      if (tracking == null) {
        return _json({'error': 'not_found', 'message': 'no stub'}, 404);
      }
      return tracking(request);
    }
    if (RegExp(r'/orders/[^/]+$').hasMatch(path) && method == 'GET') {
      return _json(groupDetail ?? _group(), 200);
    }
    if (path.endsWith('/search')) {
      final query = request.url.queryParameters;
      // The server owns discovery: the stub matches products/stores the
      // way /v1/search does — the popular shelf when q is absent.
      final term = query['q']?.toLowerCase();
      var matchedStores = [
        for (final store in feed)
          if (term == null ||
              term.isEmpty ||
              (store['name'] as String? ?? '').toLowerCase().contains(term) ||
              (store['category'] as String? ?? '')
                  .toLowerCase()
                  .contains(term))
            store,
      ];
      if (query.containsKey('lat') && query.containsKey('lng')) {
        matchedStores = [
          for (final store in matchedStores)
            if (store['id'] == _store['id'])
              {...store, 'distance_m': 900, 'eta_min': 3}
            else
              store,
        ];
      }
      final productHit = {
        'store_product_id': 'menu-1',
        'store_id': _store['id'],
        'store_name': _store['name'],
        'name': _menuItem['name'],
        'description': _menuItem['description'],
        'price': _menuItem['price'],
        'image_url': null,
      };
      final matchedProducts = [
        if (term == null ||
            term.isEmpty ||
            (_menuItem['name'] as String).toLowerCase().contains(term))
          productHit,
      ];
      return _json(
        {'products': matchedProducts, 'stores': matchedStores},
        200,
      );
    }
    if (path.endsWith('/stores')) {
      final query = request.url.queryParameters;
      // The server owns search now: the stub matches name/category the
      // way /v1/stores does when the request carries q.
      final term = query['q']?.toLowerCase();
      var results = feed;
      if (term != null && term.isNotEmpty) {
        results = [
          for (final store in results)
            if ((store['name'] as String? ?? '').toLowerCase().contains(term) ||
                (store['category'] as String? ?? '')
                    .toLowerCase()
                    .contains(term))
              store,
        ];
      }
      if (query.containsKey('lat') && query.containsKey('lng')) {
        // Distance/eta attach to the located feed the way the real
        // server does — per store, only when coords were sent.
        return _json([
          for (final store in results)
            if (store['id'] == _store['id'])
              {...store, 'distance_m': 900, 'eta_min': 3}
            else
              store,
        ], 200);
      }
      return _json(results, 200);
    }
    if (RegExp(r'/stores/[^/]+$').hasMatch(path) && method == 'GET') {
      final item = menuImages == null
          ? _menuItem
          : {..._menuItem, 'images': menuImages};
      return _json({'store': _store, 'products': [item]}, 200);
    }
    if (path.endsWith('/me')) {
      if (method == 'PATCH') {
        // The profile edit echoes the submitted name back, the way the
        // server returns the updated user.
        final body = jsonDecode(request.body) as Map<String, dynamic>;
        return _json({
          'id': 'user-1',
          'phone': '+250780000001',
          'customer': {'id': 'customer-1', 'name': body['name'] as String?},
          'admin': null,
          'merchant_memberships': <Map<String, dynamic>>[],
        });
      }
      return _json({
        'id': 'user-1',
        'phone': '+250780000001',
        'customer': {'id': 'customer-1', 'name': 'Chantal'},
        'admin': null,
        'merchant_memberships': <Map<String, dynamic>>[],
      });
    }
    return _json({'error': 'not_found', 'message': 'no stub'}, 404);
  });

  return ApiClient(
    httpClient: handler,
    tokenProvider: () => 'test-token',
  );
}

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

/// Seed the persisted cart (v2 format: one JSON document of buckets).
void _seedCart() {
  SharedPreferences.setMockInitialValues({
    'tuma_cart_v2': jsonEncode([
      {
        'storeId': 'store-1',
        'storeName': "Aline's Kitchen",
        'deliveryFee': 1500,
        'items': [
          {
            'storeProductId': 'menu-1',
            'productId': 'product-1',
            'name': 'Ibirazi',
            'unitPrice': 3500,
            'imageUrl': null,
            'quantity': 2,
          },
        ],
      },
    ]),
  });
}

Widget _harness(ApiClient client) {
  return ProviderScope(
    overrides: [
      tokenStorageProvider.overrideWithValue(_FakeTokenStorage()),
      apiClientProvider.overrideWithValue(client),
      sessionProvider.overrideWith(_SignedInSession.new),
      // No GPS in widget tests: a real platform-channel call doesn't
      // fail there, it hangs forever. The stub throws so screens take
      // their documented fallback (the persisted pin).
      acquireLocationProvider.overrideWithValue(
        () async => throw StateError('no GPS in widget tests'),
      ),
    ],
    child: MaterialApp.router(
      theme: AppTheme.dark(),
      routerConfig: buildRouter(), // fresh router per test
    ),
  );
}

/// Wait for the router redirect to settle the signed-in user on /home.
Future<void> _landOnShell(WidgetTester tester, ApiClient client) async {
  await tester.pumpWidget(_harness(client));
  await tester.pump(const Duration(milliseconds: 50)); // session resolves
  // The real SessionNotifier pokes this on every transition; the fake in
  // this harness must too, or the router stays parked on the splash.
  sessionRouterRefresher.notify();
  for (var i = 0; i < 20 && find.byType(AppShell).evaluate().isEmpty; i++) {
    await tester.pump(const Duration(milliseconds: 50));
  }
  // Let the shell's tab data loads (stores, orders) finish.
  for (var i = 0; i < 10; i++) {
    await tester.pump(const Duration(milliseconds: 50));
  }
}

/// Wait for the router redirect to settle the signed-in rider on /rider.
Future<void> _landOnRider(WidgetTester tester) async {
  await tester.pumpWidget(
    ProviderScope(
      overrides: [
        tokenStorageProvider.overrideWithValue(_FakeTokenStorage()),
        apiClientProvider.overrideWithValue(_apiClient()),
        sessionProvider.overrideWith(_RiderSession.new),
        acquireLocationProvider.overrideWithValue(
          () async => throw StateError('no GPS in widget tests'),
        ),
      ],
      child: MaterialApp.router(
        theme: AppTheme.dark(),
        routerConfig: buildRouter(), // fresh router per test
      ),
    ),
  );
  await tester.pump(const Duration(milliseconds: 50)); // session resolves
  sessionRouterRefresher.notify();
  for (var i = 0;
      i < 20 && find.byType(RiderScreen).evaluate().isEmpty;
      i++) {
    await tester.pump(const Duration(milliseconds: 50));
  }
  for (var i = 0; i < 10; i++) {
    await tester.pump(const Duration(milliseconds: 50));
  }
}

/// Bounded pumps for screens that load over the wire (or poll) —
/// pumpAndSettle would wait forever on their work.
Future<void> _settle(WidgetTester tester, [int rounds = 10]) async {
  for (var i = 0; i < rounds; i++) {
    await tester.pump(const Duration(milliseconds: 60));
  }
}

/// A widget test pinned to desktop: `defaultTargetPlatform` reads linux
/// for the body, so map surfaces take their documented honest paths (the
/// data placeholder, the paste-in pin field) — no Google Maps platform
/// channels exist in the test binding. The override is scoped (the
/// framework asserts foundation debug vars are unset after each test)
/// and restored even when the body throws. On the Linux dev host this is
/// a no-op override; on any other host it pins the map surfaces off.
void testDesktop(
    String description, Future<void> Function(WidgetTester) callback) {
  testWidgets(description, (tester) async {
    debugDefaultTargetPlatformOverride = TargetPlatform.linux;
    try {
      await callback(tester);
    } finally {
      debugDefaultTargetPlatformOverride = null;
    }
  });
}

void main() {
  // Debug dump of unexpected exceptions instead of silent swallowing.
  final origOnError = FlutterError.onError;
  FlutterError.onError = (details) {
    // Let the test framework report it (prints + fails when caught by
    // tester.takeException where relevant).
    origOnError?.call(details);
  };

  testDesktop('shell boots to Home and all four tabs render without errors',
      (tester) async {
    _seedCart();
    await _landOnShell(tester, _apiClient());

    // Home feed loads from the stub, and the greeting uses the profile name.
    expect(find.text("Aline's Kitchen"), findsOneWidget);
    expect(find.text('Hi, Chantal 👋'), findsOneWidget);
    // The real server-owned category renders on the card — and on the
    // category chip row above it, both fed by the same server field.
    expect(find.text('Grill'), findsWidgets);
    expect(find.text('Home'), findsWidgets);

    // Orders tab renders its empty state.
    await tester.tap(find.text('Orders'));
    await _settle(tester);
    expect(find.text('No orders yet'), findsOneWidget);

    // Cart tab shows the grouped item, store header, totals, and the CTA.
    await tester.tap(find.text('Cart'));
    await _settle(tester);
    // Home is offstage inside the IndexedStack — only the cart's bucket
    // header is in the visible tree.
    expect(find.text("Aline's Kitchen"), findsOneWidget);
    expect(find.text('Ibirazi'), findsOneWidget);
    expect(find.text('Proceed to checkout'), findsOneWidget);
    expect(find.textContaining('7,000'), findsWidgets);
    expect(find.textContaining('1,500'), findsWidgets); // delivery fee row
    expect(find.textContaining('8,500'), findsOneWidget); // total

    // Profile tab renders.
    await tester.tap(find.text('Profile'));
    await _settle(tester);
    expect(find.text('Sign out'), findsOneWidget);

    expect(tester.takeException(), isNull);
  });

  testDesktop('empty cart shows the honest empty state', (tester) async {
    SharedPreferences.setMockInitialValues({});
    await _landOnShell(tester, _apiClient());

    await tester.tap(find.text('Cart'));
    await _settle(tester);

    expect(find.text('Your cart is empty'), findsOneWidget);
    expect(find.text('Proceed to checkout'), findsNothing);
    expect(tester.takeException(), isNull);
  });

  testDesktop('a multi-store cart groups by store and warns about splitting',
      (tester) async {
    SharedPreferences.setMockInitialValues({
      'tuma_cart_v2': jsonEncode([
        {
          'storeId': 'store-1',
          'storeName': "Aline's Kitchen",
          'deliveryFee': 1500,
          'items': [
            {
              'storeProductId': 'menu-1',
              'productId': 'product-1',
              'name': 'Ibirazi',
              'unitPrice': 3500,
              'imageUrl': null,
              'quantity': 2,
            },
          ],
        },
        {
          'storeId': 'store-2',
          'storeName': "Bruce's Grill",
          'deliveryFee': 1000,
          'items': [
            {
              'storeProductId': 'menu-2',
              'productId': 'product-2',
              'name': 'Burger',
              'unitPrice': 4000,
              'imageUrl': null,
              'quantity': 1,
            },
          ],
        },
      ]),
    });
    await _landOnShell(tester, _apiClient());

    await tester.tap(find.text('Cart'));
    await _settle(tester);

    // Both buckets render under their own headers, with the split hint.
    expect(find.text("Aline's Kitchen"), findsOneWidget);
    expect(find.text("Bruce's Grill"), findsOneWidget);
    expect(
      find.textContaining('separate deliveries'),
      findsOneWidget,
    );
    expect(find.text('Checkout all 2 stores'), findsOneWidget);

    // Grand total = 7,000 + 4,000 items + 1,500 + 1,000 delivery.
    expect(find.textContaining('11,000'), findsOneWidget); // subtotal
    expect(find.textContaining('2,500'), findsOneWidget); // delivery (all)
    expect(find.textContaining('13,500'), findsOneWidget); // total
    expect(tester.takeException(), isNull);
  });

  testDesktop('checkout flow places an order and lands on the orders list',
      (tester) async {
    _seedCart();
    await _landOnShell(
      tester,
      _apiClient(groups: [
        {
          'id': 'group-1',
          'number': 1,
          'grand_total': 16500,
          'status': 'in_progress',
          'stores': ["Aline's Kitchen"],
          'created_at': '2026-08-31T08:00:00Z',
        },
      ]),
    );

    // Cart → checkout.
    await tester.tap(find.text('Cart'));
    await _settle(tester);
    await tester.tap(find.text('Proceed to checkout'));
    await _settle(tester);

    expect(find.text('Checkout'), findsOneWidget);
    // The delivery-pin map pushes address + payment below the fold, and
    // a ListView only builds visible children — scroll down (topmost
    // route's list) before interacting with them.
    await tester.drag(find.byType(ListView).last, const Offset(0, -600));
    await _settle(tester);
    expect(find.text('Cash on delivery'), findsOneWidget);

    // Enter an address and place the order. The desktop pin field is a
    // TextField too — target the address field by its hint.
    await tester.enterText(
      find.ancestor(
        of: find.text('Street, building, landmark…'),
        matching: find.byType(TextField),
      ),
      'KN 4 Ave, Kigali',
    );
    await tester.tap(find.text('Place order'));
    // Bounded pumps: the shell's data loads settle, so no pumpAndSettle.
    await _settle(tester);

    // Lands on the Orders TAB of the shell — never on a dead checkout:
    // the list page title is up, the bottom bar is back, and the order
    // is in history. Back from a detail opened here goes to this list.
    expect(find.text('Your Orders'), findsOneWidget);
    expect(find.byType(AppShell), findsOneWidget);
    expect(find.text('Checkout'), findsNothing);
    expect(find.text("Aline's Kitchen"), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testDesktop('an empty checkout routes back to the cart tab in the shell',
      (tester) async {
    SharedPreferences.setMockInitialValues({});
    await _landOnShell(tester, _apiClient());

    // A dead end: /checkout with nothing in the cart (a deep link, or any
    // history where the cart emptied underneath us).
    GoRouter.of(tester.element(find.byType(AppShell).first)).go('/checkout');
    await _settle(tester);
    expect(find.text('Nothing to checkout.'), findsOneWidget);

    // The just-fit button lands in the shell's Cart tab — app bar back,
    // never stranded on a barless dead end.
    await tester.tap(find.text('Back to cart'));
    await _settle(tester);
    expect(find.byType(AppShell), findsOneWidget);
    expect(find.text('Your cart is empty'), findsOneWidget);
    expect(find.text('Nothing to checkout.'), findsNothing);
    expect(tester.takeException(), isNull);
  });

  testDesktop('a persisted location drives the feed request and the real card',
      (tester) async {
    // A pin the checkout map (or an earlier GPS fix) persisted. In tests
    // acquireLocation has no platform channel — this exercises exactly
    // the desktop fallback: GPS fails, the persisted pin wins.
    SharedPreferences.setMockInitialValues({
      'tuma_location_v1': jsonEncode({'lat': -1.9512, 'lng': 30.0623}),
    });
    final seen = <http.Request>[];
    await _landOnShell(tester, _apiClient(seen: seen));

    // The feed request carried the pin…
    final storesCall =
        seen.singleWhere((r) => r.url.path.endsWith('/stores'));
    expect(storesCall.url.queryParameters['lat'], '-1.9512');
    expect(storesCall.url.queryParameters['lng'], '30.0623');

    // …and the card renders the server-computed facts — no placeholder
    // numbers anywhere.
    expect(find.text('0.9 km'), findsOneWidget);
    expect(find.text('~3 min'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testDesktop('checkout sends the pinned delivery coordinates with the order',
      (tester) async {
    SharedPreferences.setMockInitialValues({
      'tuma_cart_v2': jsonEncode([
        {
          'storeId': 'store-1',
          'storeName': "Aline's Kitchen",
          'deliveryFee': 1500,
          'items': [
            {
              'storeProductId': 'menu-1',
              'productId': 'product-1',
              'name': 'Ibirazi',
              'unitPrice': 3500,
              'imageUrl': null,
              'quantity': 2,
            },
          ],
        },
      ]),
      'tuma_location_v1': jsonEncode({'lat': -1.9449, 'lng': 30.0619}),
    });
    final seen = <http.Request>[];
    await _landOnShell(tester, _apiClient(seen: seen));

    await tester.tap(find.text('Cart'));
    await _settle(tester);
    await tester.tap(find.text('Proceed to checkout'));
    await _settle(tester);
    // Scroll past the map so the address field is built and visible. The
    // desktop pin field is a TextField too — target the address field by
    // its hint.
    await tester.drag(find.byType(ListView).last, const Offset(0, -600));
    await _settle(tester);
    await tester.enterText(
      find.ancestor(
        of: find.text('Street, building, landmark…'),
        matching: find.byType(TextField),
      ),
      'KN 4 Ave, Kigali',
    );
    await tester.tap(find.text('Place order'));
    await _settle(tester);

    // The checkout POST body carries the pin the map was seeded with.
    final checkout = seen.singleWhere(
      (r) => r.method == 'POST' && r.url.path.endsWith('/orders'),
    );
    final body = jsonDecode(checkout.body) as Map<String, dynamic>;
    expect(body['address_lat'], -1.9449);
    expect(body['address_lng'], 30.0619);
    expect(tester.takeException(), isNull);
  });

  testDesktop('orders tab lists group cards and opens the detail screen',
      (tester) async {
    final client = _apiClient(groups: [
      {
        'id': 'group-9',
        'number': 999,
        'grand_total': 8500,
        'status': 'completed',
        'stores': ["Aline's Kitchen"],
        'created_at': '2026-08-28T07:00:00Z',
      },
    ]);
    await _landOnShell(tester, client);

    await tester.tap(find.text('Orders'));
    await _settle(tester);

    // The group card renders the derived state and the store list.
    expect(find.text('Completed'), findsOneWidget);
    expect(find.text("Aline's Kitchen"), findsWidgets);

    await tester.tap(find.text('Completed'));
    await _settle(tester);

    // Detail screen renders the store section's timeline.
    expect(find.textContaining('Order #'), findsOneWidget);
    expect(find.textContaining('Delivered'), findsWidgets);
    expect(tester.takeException(), isNull);
  });

  testDesktop('the picked_up world: map card, decaying ETA, and the strip',
      (tester) async {
    _seedCart();
    final client = _apiClient(
      groups: [
        {
          'id': 'group-1',
          'number': 1042,
          'grand_total': 8500,
          'status': 'in_progress',
          'stores': ["Aline's Kitchen"],
          'created_at': '2026-08-28T07:00:00Z',
        },
      ],
      groupDetail: _group(
        id: 'group-1',
        status: 'in_progress',
        addressLat: -1.9499,
        addressLng: 30.0622,
      ),
      tracking: (_) => _json(_tracking()),
    );
    await _landOnShell(tester, client);

    await tester.tap(find.text('Orders'));
    await _settle(tester);
    await tester.tap(find.text('In progress'));
    await _settle(tester);

    // The Moving world: the strip replaces the timeline, the card shows
    // the decaying ETA, and the map panel is present (desktop: the honest
    // data placeholder — the real map is a phone verification).
    expect(find.text('Out for delivery'), findsOneWidget);
    expect(find.textContaining('Arriving'), findsOneWidget);
    expect(find.textContaining('the map renders on your phone'),
        findsOneWidget);
    expect(find.text('Delivered'), findsNothing,
        reason: 'the six-step timeline is folded away while moving');
    expect(tester.takeException(), isNull);
  });

  testDesktop('no destination pin: no map, the honest text', (tester) async {
    _seedCart();
    final client = _apiClient(
      groups: [
        {
          'id': 'group-1',
          'number': 1042,
          'grand_total': 8500,
          'status': 'in_progress',
          'stores': ["Aline's Kitchen"],
          'created_at': '2026-08-28T07:00:00Z',
        },
      ],
      // The group has no pin — the §3 rule: no guessed geocode, ever.
      groupDetail: _group(id: 'group-1', status: 'in_progress'),
      tracking: (_) => _json(_tracking()),
    );
    await _landOnShell(tester, client);

    await tester.tap(find.text('Orders'));
    await _settle(tester);
    await tester.tap(find.text('In progress'));
    await _settle(tester);

    expect(find.textContaining('add a map pin at checkout next time'),
        findsOneWidget);
    expect(find.textContaining('the map renders on your phone'),
        findsNothing, reason: 'no pin, no map — not even the placeholder');
    expect(tester.takeException(), isNull);
  });

  testDesktop('pin but no rider check-in: the route waits with you',
      (tester) async {
    _seedCart();
    final client = _apiClient(
      groups: [
        {
          'id': 'group-1',
          'number': 1042,
          'grand_total': 8500,
          'status': 'in_progress',
          'stores': ["Aline's Kitchen"],
          'created_at': '2026-08-28T07:00:00Z',
        },
      ],
      groupDetail: _group(
        id: 'group-1',
        status: 'in_progress',
        addressLat: -1.9499,
        addressLng: 30.0622,
      ),
      tracking: (_) => _json(_tracking(riderLat: null, riderLng: null)),
    );
    await _landOnShell(tester, client);

    await tester.tap(find.text('Orders'));
    await _settle(tester);
    await tester.tap(find.text('In progress'));
    await _settle(tester);

    expect(find.textContaining('the moment their phone checks in'),
        findsOneWidget);
    // The map panel IS present — the pin exists; only the rider is away.
    expect(find.textContaining('the map renders on your phone'),
        findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testDesktop('a landing tracking snapshot merges over the group detail',
      (tester) async {
    _seedCart();
    final client = _apiClient(
      groups: [
        {
          'id': 'group-1',
          'number': 1042,
          'grand_total': 8500,
          'status': 'in_progress',
          'stores': ["Aline's Kitchen"],
          'created_at': '2026-08-28T07:00:00Z',
        },
      ],
      groupDetail: _group(id: 'group-1', status: 'in_progress'),
      tracking: (_) => _json(
        _tracking(
          groupStatus: 'completed',
          paymentStatus: 'collected',
          deliveryStatus: 'delivered',
        ),
      ),
    );
    await _landOnShell(tester, client);

    await tester.tap(find.text('Orders'));
    await _settle(tester);
    await tester.tap(find.text('In progress'));
    await _settle(tester);

    // The snapshot's statuses win: the header chip, the payment line, and
    // the settled-state wayfinding — no full refetch needed.
    expect(find.text('Completed'), findsOneWidget);
    expect(find.textContaining('collected'), findsOneWidget);
    // The settled polish: the delivered moment shows on the section.
    expect(find.textContaining('Delivered · '), findsOneWidget);
    // Wayfinding sits below the fold — scroll the detail to it.
    await tester.scrollUntilVisible(
      find.text('Continue shopping'),
      200,
      scrollable: find.byType(Scrollable).last,
    );
    expect(find.text('Continue shopping'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testDesktop('the poll echoes changed_at as since and 204 changes nothing',
      (tester) async {
    _seedCart();
    final seen = <http.Request>[];
    final client = _apiClient(
      groups: [
        {
          'id': 'group-1',
          'number': 1042,
          'grand_total': 8500,
          'status': 'in_progress',
          'stores': ["Aline's Kitchen"],
          'created_at': '2026-08-28T07:00:00Z',
        },
      ],
      groupDetail: _group(id: 'group-1', status: 'in_progress'),
      seen: seen,
      tracking: (request) {
        // First poll: the full snapshot. Every poll after (it echoes
        // since): nothing changed — 204, the battery contract.
        final isEcho = request.url.queryParameters.containsKey('since');
        return isEcho ? http.Response('', 204) : _json(_tracking());
      },
    );
    await _landOnShell(tester, client);

    await tester.tap(find.text('Orders'));
    await _settle(tester);
    await tester.tap(find.text('In progress'));
    await _settle(tester);

    var trackingRequests = seen
        .where((r) => r.url.path.endsWith('/tracking'))
        .toList(growable: false);
    expect(trackingRequests, isNotEmpty);
    expect(trackingRequests.first.url.queryParameters.containsKey('since'),
        isFalse, reason: 'the first fetch seeds changed_at, no since');
    final snapshotCount = trackingRequests.length;

    // Advance past the 5 s poll interval: the echo carries the snapshot's
    // changed_at, the 204 lands, and no rebuild happens.
    await tester.pump(const Duration(seconds: 5));
    await tester.pump(const Duration(milliseconds: 100));
    trackingRequests = seen
        .where((r) => r.url.path.endsWith('/tracking'))
        .toList(growable: false);
    expect(trackingRequests.last.url.queryParameters['since'],
        '2026-08-28T07:05:00.000Z',
        reason: 'changed_at echoed back as since');
    expect(trackingRequests.length, snapshotCount + 1);
    expect(tester.takeException(), isNull);
  });

  testDesktop('a stale signal lags the ladder honestly', (tester) async {
    _seedCart();
    final client = _apiClient(
      groups: [
        {
          'id': 'group-1',
          'number': 1042,
          'grand_total': 8500,
          'status': 'in_progress',
          'stores': ["Aline's Kitchen"],
          'created_at': '2026-08-28T07:00:00Z',
        },
      ],
      groupDetail: _group(id: 'group-1', status: 'in_progress'),
      tracking: (_) => _json(_tracking(
        // Signal 10 min old, but the ETA hasn't passed: lagging badge,
        // and the ETA line is still the honest countdown.
        lastLocationAt:
            DateTime.now().toUtc().subtract(const Duration(minutes: 10)),
      )),
    );
    await _landOnShell(tester, client);

    await tester.tap(find.text('Orders'));
    await _settle(tester);
    await tester.tap(find.text('In progress'));
    await _settle(tester);

    expect(find.textContaining('Lagging'), findsOneWidget);
    expect(find.textContaining('Arriving'), findsOneWidget);
    expect(find.text('Taking longer than expected'), findsNothing);
    expect(tester.takeException(), isNull);
  });

  testDesktop('past the grace: taking longer, with the way to call the store',
      (tester) async {
    _seedCart();
    final client = _apiClient(
      groups: [
        {
          'id': 'group-1',
          'number': 1042,
          'grand_total': 8500,
          'status': 'in_progress',
          'stores': ["Aline's Kitchen"],
          'created_at': '2026-08-28T07:00:00Z',
        },
      ],
      groupDetail: _group(id: 'group-1', status: 'in_progress'),
      tracking: (_) => _json(_tracking(
        // 30 min past the ETA, signal stale: the honest overdue line and
        // the store's contact — presentation only, never auto-delivered.
        etaTarget:
            DateTime.now().toUtc().subtract(const Duration(minutes: 30)),
        lastLocationAt:
            DateTime.now().toUtc().subtract(const Duration(minutes: 10)),
      )),
    );
    await _landOnShell(tester, client);

    await tester.tap(find.text('Orders'));
    await _settle(tester);
    await tester.tap(find.text('In progress'));
    await _settle(tester);

    expect(find.text('Taking longer than expected'), findsOneWidget);
    expect(find.textContaining('call the store'), findsOneWidget);
    expect(find.text('Call'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testDesktop('an ended delivery tells the truth and stops pretending',
      (tester) async {
    _seedCart();
    final client = _apiClient(
      groups: [
        {
          'id': 'group-1',
          'number': 1042,
          'grand_total': 8500,
          'status': 'in_progress',
          'stores': ["Aline's Kitchen"],
          'created_at': '2026-08-28T07:00:00Z',
        },
      ],
      groupDetail: _group(id: 'group-1', status: 'in_progress'),
      tracking: (_) => _json(_tracking(
        // ~25h past the ETA: Ended. No ETA countdown, no check-in
        // promise — the store call is the way out.
        etaTarget:
            DateTime.now().toUtc().subtract(const Duration(hours: 25)),
        lastLocationAt:
            DateTime.now().toUtc().subtract(const Duration(hours: 25)),
      )),
    );
    await _landOnShell(tester, client);

    await tester.tap(find.text('Orders'));
    await _settle(tester);
    await tester.tap(find.text('In progress'));
    await _settle(tester);

    expect(find.textContaining('much longer than expected'), findsOneWidget);
    expect(find.textContaining('Ended'), findsOneWidget);
    expect(find.text('Arriving now'), findsNothing);
    expect(tester.takeException(), isNull);
  });

  testDesktop('the poll decays when the delivery lags', (tester) async {
    _seedCart();
    final seen = <http.Request>[];
    final client = _apiClient(
      groups: [
        {
          'id': 'group-1',
          'number': 1042,
          'grand_total': 8500,
          'status': 'in_progress',
          'stores': ["Aline's Kitchen"],
          'created_at': '2026-08-28T07:00:00Z',
        },
      ],
      groupDetail: _group(id: 'group-1', status: 'in_progress'),
      seen: seen,
      tracking: (request) {
        // The FIRST response is already lagging (stale signal) so the
        // 15s cadence programs from the start; echoes stay lagging too.
        return _json(
          _tracking(
            lastLocationAt:
                DateTime.now().toUtc().subtract(const Duration(minutes: 10)),
          ),
        );
      },
    );
    await _landOnShell(tester, client);

    await tester.tap(find.text('Orders'));
    await _settle(tester);
    await tester.tap(find.text('In progress'));
    await _settle(tester);
    // Let any in-flight initial poll finish before baselining.
    await tester.pump(const Duration(seconds: 5));
    await tester.pump(const Duration(milliseconds: 100));
    var count = seen.where((r) => r.url.path.endsWith('/tracking')).length;

    // Lagging (stale signal): the cadence dropped to 15s — at 5s nothing
    // fires, at the next 15s the poll runs once.
    await tester.pump(const Duration(seconds: 5));
    final afterFive = seen.where((r) => r.url.path.endsWith('/tracking')).length;
    expect(afterFive, count, reason: 'no poll at the old 5s cadence');
    await tester.pump(const Duration(seconds: 15));
    count = seen.where((r) => r.url.path.endsWith('/tracking')).length;
    expect(count, greaterThan(afterFive),
        reason: 'the 15s lagging poll fired');
    expect(tester.takeException(), isNull);
  });

  testDesktop('store screen: add-to-cart shows feedback and updates the cart',
      (tester) async {
    // Empty prefs so the cart's persistence writes resolve in the test.
    SharedPreferences.setMockInitialValues({});
    await _landOnShell(tester, _apiClient());

    // Open the store.
    await tester.tap(find.text("Aline's Kitchen").first);
    await _settle(tester);
    expect(find.text('Menu'), findsOneWidget);

    // Scroll the first add-to-cart button into view, tap it, and expect a
    // confirmation snackbar.
    final addButton = find.byIcon(Icons.add_shopping_cart_rounded).first;
    await tester.ensureVisible(addButton);
    await tester.pump();
    await tester.tap(addButton);
    await tester.pump(const Duration(milliseconds: 300));
    expect(find.textContaining('Added to cart'), findsOneWidget);

    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
  });

  testDesktop('adding from two stores keeps both in one cart',
      (tester) async {
    // The store detail stub only knows store-1; add twice there, then
    // verify the bucket behavior directly through the notifier state.
    SharedPreferences.setMockInitialValues({});
    await _landOnShell(tester, _apiClient());

    await tester.tap(find.text("Aline's Kitchen").first);
    await _settle(tester);

    final addButton = find.byIcon(Icons.add_shopping_cart_rounded).first;
    await tester.ensureVisible(addButton);
    await tester.pump();
    await tester.tap(addButton);
    await tester.pump(const Duration(milliseconds: 300));
    await tester.tap(addButton);
    await tester.pump(const Duration(milliseconds: 300));

    // The second tap increments the same line instead of duplicating it.
    expect(find.textContaining('Added to cart (2 items)'), findsOneWidget);
    final container = ProviderScope.containerOf(
      tester.element(find.text('Menu')),
      listen: false,
    );
    final cart = container.read<AsyncValue<CartState>>(cartProvider).requireValue;
    expect(cart.storeCount, 1);
    expect(cart.itemCount, 2);
    expect(cart.total, 3500 * 2 + 1500);

    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
  });

  testDesktop('signing out clears the cart, the pin, and the recents',
      (tester) async {
    // One customer's session state: a multi-item cart, a delivery pin,
    // and their search recents — everything personal on this phone.
    SharedPreferences.setMockInitialValues({
      'tuma_cart_v2': jsonEncode([
        {
          'storeId': 'store-1',
          'storeName': "Aline's Kitchen",
          'deliveryFee': 1500,
          'items': [
            {
              'storeProductId': 'menu-1',
              'productId': 'product-1',
              'name': 'Ibirazi',
              'unitPrice': 3500,
              'imageUrl': null,
              'quantity': 2,
            },
          ],
        },
      ]),
      'tuma_location_v1': jsonEncode({'lat': -1.9449, 'lng': 30.0619}),
      'tuma_recent_searches_v1': ['brochettes', 'kfc'],
    });
    await _landOnShell(tester, _apiClient());

    await tester.tap(find.text('Profile'));
    await _settle(tester);
    await tester.tap(find.text('Sign out'));
    await _settle(tester);

    // The router redirect lands on the phone screen…
    expect(find.text('Welcome to Tuma'), findsOneWidget);
    // …and everything that belonged to this customer is gone — the next
    // person on this phone sees none of it.
    final prefs = await SharedPreferences.getInstance();
    expect(prefs.getString('tuma_cart_v2'), isNull);
    expect(prefs.getString('tuma_location_v1'), isNull);
    expect(prefs.getString('tuma_recent_searches_v1'), isNull);
    expect(tester.takeException(), isNull);
  });

  testDesktop('product sheet scrolls a long description without overflowing',
      (tester) async {
    SharedPreferences.setMockInitialValues({});
    await _landOnShell(tester, _apiClient());

    // Open the store, then the product sheet — the stub's description is
    // far taller than the sheet's height cap, so the old fixed-Column
    // sheet threw a RenderFlex overflow here.
    await tester.tap(find.text("Aline's Kitchen").first);
    await _settle(tester);
    await tester.tap(find.text('Ibirazi').first);
    await _settle(tester);

    // The sheet renders the identity and the full description (the row
    // behind the sheet shows the same snippet — hence findsWidgets).
    expect(find.text('Ibirazi'), findsWidgets);
    expect(find.textContaining('Slow-cooked'), findsWidgets);

    // …and the content scrolls instead of clipping or throwing. Bounded
    // pumps only: the sheet's route animation and the drag's ballistic
    // settle keep scheduling frames, so pumpAndSettle would never return.
    await tester.drag(
      find.byType(SingleChildScrollView).last,
      const Offset(0, -400),
    );
    for (var i = 0; i < 15; i++) {
      await tester.pump(const Duration(milliseconds: 60));
    }
    expect(tester.takeException(), isNull);
  });

  testDesktop('home shows no search field — the Search tab owns discovery',
      (tester) async {
    SharedPreferences.setMockInitialValues({});
    await _landOnShell(tester, _apiClient());

    // Home is pure browse: no input-shaped button, the feed directly
    // under the greeting.
    expect(find.text('Search stores or food…'), findsNothing);
    expect(find.byType(TextField), findsNothing);
    expect(find.text("Aline's Kitchen"), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testDesktop('search tab: the shelf by default, matches when queried',
      (tester) async {
    SharedPreferences.setMockInitialValues({});
    await _landOnShell(tester, _apiClient());

    await tester.tap(find.text('Search'));
    await _settle(tester);
    // Default state: the popular shelf with the stub's one hit. The
    // Stores tab rides along in the IndexedStack — the store name shows
    // both on the product tile's store line and in the hidden store row.
    expect(find.text('Popular near you'), findsOneWidget);
    expect(find.text('Ibirazi'), findsOneWidget);
    expect(find.text("Aline's Kitchen"), findsWidgets);

    // A search narrows both sections; a matchless term empties them
    // honestly.
    await tester.enterText(find.byType(TextField).first, 'zzz');
    await _settle(tester);
    expect(find.text("No products match 'zzz'."), findsOneWidget);

    await tester.enterText(find.byType(TextField).first, 'ibi');
    await _settle(tester);
    expect(find.text('Ibirazi'), findsOneWidget);

    // The search landed in recents; clearing the field shows them back —
    // visible only while the field is active.
    await tester.enterText(find.byType(TextField).first, '');
    await _settle(tester);
    expect(find.text('ibi'), findsOneWidget, reason: 'recent search chip');
    expect(find.text('zzz'), findsOneWidget, reason: 'the other recent');
    expect(find.text('Recent searches'), findsOneWidget);

    // The chip's own ✕ removes exactly that recent; the others survive.
    final ibiChip =
        find.ancestor(of: find.text('ibi'), matching: find.byType(Material)).first;
    await tester.tap(
      find.descendant(of: ibiChip, matching: find.byIcon(Icons.close_rounded)),
    );
    await _settle(tester);
    expect(find.text('ibi'), findsNothing);
    expect(find.text('zzz'), findsOneWidget, reason: 'siblings untouched');
    expect(find.text('Recent searches'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testDesktop('category chips filter the home feed in place', (tester) async {
    final bakery = {
      ..._store,
      'id': 'store-2',
      'name': "Bruce's Bakery",
      'category': 'Bakery',
    };
    SharedPreferences.setMockInitialValues({});
    await _landOnShell(tester, _apiClient(stores: [_store, bakery]));

    // Both cards render; the chips carry the two real categories.
    expect(find.text("Aline's Kitchen"), findsOneWidget);
    expect(find.text("Bruce's Bakery"), findsOneWidget);

    // The Grill chip filters client-side: only Aline's survives.
    await tester.tap(find.text('Grill').first);
    await _settle(tester);
    expect(find.text("Aline's Kitchen"), findsOneWidget);
    expect(find.text("Bruce's Bakery"), findsNothing);

    // All restores the feed.
    await tester.tap(find.text('All'));
    await _settle(tester);
    expect(find.text("Bruce's Bakery"), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testDesktop('profile: editing the name updates the session greeting',
      (tester) async {
    SharedPreferences.setMockInitialValues({});
    await _landOnShell(tester, _apiClient());

    await tester.tap(find.text('Profile'));
    await _settle(tester);
    // The pencil opens the dialog seeded with the current name.
    await tester.tap(find.byIcon(Icons.edit_rounded));
    await _settle(tester);
    await tester.enterText(find.byType(TextField), 'Mutesi');
    await tester.tap(find.text('Save'));
    await _settle(tester);

    // The identity card shows the new name immediately…
    expect(find.text('Mutesi'), findsOneWidget);
    // …and the Home greeting follows, because the session updated live.
    await tester.tap(find.text('Home'));
    await _settle(tester);
    expect(find.text('Hi, Mutesi 👋'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testDesktop('stores without uploads render the honest icon, never a fake photo',
      (tester) async {
    // The stub's image_url is null: the card shows the storefront glyph
    // block — no Unsplash, no other food's picture.
    SharedPreferences.setMockInitialValues({});
    await _landOnShell(tester, _apiClient());

    expect(find.byIcon(Icons.storefront_rounded), findsWidgets);
    expect(tester.takeException(), isNull);
  });

  testDesktop('the product sheet swipes a multi-image gallery',
      (tester) async {
    SharedPreferences.setMockInitialValues({});
    await _landOnShell(
      tester,
      _apiClient(
        menuImages: ['https://cdn.test/cover.jpg', 'https://cdn.test/second.jpg'],
      ),
    );

    await tester.tap(find.text("Aline's Kitchen").first);
    await _settle(tester);
    await tester.tap(find.text('Ibirazi').first);
    await _settle(tester);

    // Two images = the carousel; a swipe moves it and nothing overflows.
    expect(find.byType(PageView), findsOneWidget);
    await tester.drag(find.byType(PageView), const Offset(-400, 0));
    await _settle(tester);
    expect(find.byType(PageView), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testDesktop('a rider account lands on the rider kiosk', (tester) async {
    SharedPreferences.setMockInitialValues({});
    await _landOnRider(tester);

    // Rider mode renders: identity, the rider number, the honest empty
    // work list (this harness's stub serves no jobs). No customer shell.
    expect(find.text('Rider mode'), findsOneWidget);
    expect(find.text('Jean'), findsOneWidget);
    expect(find.text('#7'), findsOneWidget);
    expect(find.textContaining('No deliveries yet'), findsOneWidget);
    expect(find.byType(AppShell), findsNothing);
    expect(tester.takeException(), isNull);
  });

  testDesktop('a non-rider is bounced off the rider screen', (tester) async {
    SharedPreferences.setMockInitialValues({});
    await _landOnShell(tester, _apiClient());

    // A signed-in customer navigating to /rider lands back on home —
    // rider mode is the rider's surface only.
    GoRouter.of(tester.element(find.byType(AppShell).first)).go('/rider');
    await _settle(tester);

    expect(find.byType(RiderScreen), findsNothing);
    expect(find.byType(AppShell), findsOneWidget);
    expect(tester.takeException(), isNull);
  });
}
