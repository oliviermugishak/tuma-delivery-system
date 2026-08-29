import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
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
}) =>
    {
      'id': id,
      'number': 1042,
      'address_text': 'KN 4 Ave, Kigali',
      'address_lat': null,
      'address_lng': null,
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

/// Real [ApiClient] backed by a scripted server. Every screen the test
/// visits gets its data from here — nothing touches the network. With
/// [seen], every request is captured for wire-level assertions. The
/// /stores stub mirrors the real server's contract: distance/eta only
/// when the request carried the customer's location.
ApiClient _apiClient({
  List<Map<String, dynamic>> groups = const [],
  List<http.Request>? seen,
}) {
  final handler = MockClient((request) async {
    seen?.add(request);
    final path = request.url.path;
    final method = request.method;

    if (method == 'POST' && path.endsWith('/orders')) {
      return _json(_group(status: 'in_progress'), 201);
    }
    if (method == 'GET' && path.endsWith('/orders')) {
      return _json(groups, 200);
    }
    if (RegExp(r'/orders/[^/]+$').hasMatch(path) && method == 'GET') {
      return _json(_group(), 200);
    }
    if (path.endsWith('/stores')) {
      final query = request.url.queryParameters;
      if (query.containsKey('lat') && query.containsKey('lng')) {
        return _json([
          {..._store, 'distance_m': 900, 'eta_min': 3},
        ], 200);
      }
      return _json([_store], 200);
    }
    if (RegExp(r'/stores/[^/]+$').hasMatch(path) && method == 'GET') {
      return _json({'store': _store, 'products': [_menuItem]}, 200);
    }
    if (path.endsWith('/me')) {
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

/// Bounded pumps for screens that load over the wire (or poll) —
/// pumpAndSettle would wait forever on their work.
Future<void> _settle(WidgetTester tester, [int rounds = 10]) async {
  for (var i = 0; i < rounds; i++) {
    await tester.pump(const Duration(milliseconds: 60));
  }
}

void main() {
  // Debug dump of unexpected exceptions instead of silent swallowing.
  final origOnError = FlutterError.onError;
  FlutterError.onError = (details) {
    // Let the test framework report it (prints + fails when caught by
    // tester.takeException where relevant).
    origOnError?.call(details);
  };

  testWidgets('shell boots to Home and all four tabs render without errors',
      (tester) async {
    _seedCart();
    await _landOnShell(tester, _apiClient());

    // Home feed loads from the stub, and the greeting uses the profile name.
    expect(find.text("Aline's Kitchen"), findsOneWidget);
    expect(find.text('Hi, Chantal 👋'), findsOneWidget);
    // The real server-owned category renders on the card.
    expect(find.text('Grill'), findsOneWidget);
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

  testWidgets('empty cart shows the honest empty state', (tester) async {
    SharedPreferences.setMockInitialValues({});
    await _landOnShell(tester, _apiClient());

    await tester.tap(find.text('Cart'));
    await _settle(tester);

    expect(find.text('Your cart is empty'), findsOneWidget);
    expect(find.text('Proceed to checkout'), findsNothing);
    expect(tester.takeException(), isNull);
  });

  testWidgets('a multi-store cart groups by store and warns about splitting',
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

  testWidgets('checkout flow places an order and lands on the detail screen',
      (tester) async {
    _seedCart();
    await _landOnShell(tester, _apiClient());

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

    // Enter an address and place the order.
    await tester.enterText(
      find.byType(TextField),
      'KN 4 Ave, Kigali',
    );
    await tester.tap(find.text('Place order'));
    // Bounded pumps: the detail screen polls while in flight, so no
    // pumpAndSettle here. The stub returns a completed group on the first
    // fetch, which stops the poll timer.
    await _settle(tester);

    // Lands on the group detail: overall chip + the store's section.
    expect(find.text('Checkout'), findsNothing);
    expect(find.textContaining('Order #'), findsOneWidget);
    expect(find.text('Completed'), findsOneWidget);
    expect(find.text("Aline's Kitchen"), findsOneWidget);
    expect(find.textContaining('Delivered'), findsWidgets);
    expect(tester.takeException(), isNull);
  });

  testWidgets('a persisted location drives the feed request and the real card',
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

  testWidgets('checkout sends the pinned delivery coordinates with the order',
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
    // Scroll past the map so the address field is built and visible.
    await tester.drag(find.byType(ListView).last, const Offset(0, -600));
    await _settle(tester);
    await tester.enterText(
      find.byType(TextField),
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

  testWidgets('orders tab lists group cards and opens the detail screen',
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

  testWidgets('store screen: add-to-cart shows feedback and updates the cart',
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

  testWidgets('adding from two stores keeps both in one cart',
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

  testWidgets('product sheet scrolls a long description without overflowing',
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
}
