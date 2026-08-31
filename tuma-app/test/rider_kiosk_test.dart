import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

import 'package:tuma_app/core/api/api_client.dart';
import 'package:tuma_app/core/api/models/authenticated_user.dart';
import 'package:tuma_app/core/auth/auth_controller.dart';
import 'package:tuma_app/core/auth/token_storage.dart';
import 'package:tuma_app/core/router/app_router.dart';
import 'package:tuma_app/core/theme/app_theme.dart';
import 'package:tuma_app/features/location/customer_location.dart';
import 'package:tuma_app/features/rider/rider_screen.dart';

import 'test_desktop.dart';

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

/// Signed in as a rider (admin-created, no customer profile) — the router
/// lands on rider mode.
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

Map<String, dynamic> _job({
  String deliveryId = 'delivery-1',
  String storeName = "Aline's Kitchen",
  String destination = 'KN 4 Ave, Kigali',
  String? customerPhone = '+250783002002',
}) =>
    {
      'delivery_id': deliveryId,
      'store_order_id': 'so-1',
      'store_name': storeName,
      'store_address': 'KG 7 Ave, Remera',
      'store_lat': -1.9512,
      'store_lng': 30.0623,
      'destination_address': destination,
      'destination_lat': -1.9499,
      'destination_lng': 30.0622,
      'customer_name': 'Chantal',
      'customer_phone': customerPhone,
      'status': 'picked_up',
      'handoff_at': '2026-08-31T10:00:00Z',
      'route_polyline': '_p~iF~ps|U_ulLnnqC',
      'eta_target': '2026-08-31T10:20:00Z',
      'last_lat': null,
      'last_lng': null,
      'last_location_at': null,
    };

http.Response _json(Object body, [int status = 200]) => http.Response(
      jsonEncode(body),
      status,
      headers: {'content-type': 'application/json'},
    );

/// The scripted server: the work list (mutable so tests can clear it),
/// location pushes and delivered calls captured for wire assertions.
class _Script {
  final requests = <http.Request>[];
  List<Map<String, dynamic>> jobs = [];
  bool deliveredCalled = false;

  MockClient client() => MockClient((request) async {
        requests.add(request);
        final path = request.url.path;
        if (request.method == 'GET' && path.endsWith('/deliveries')) {
          return _json(jobs);
        }
        if (RegExp(r'/deliveries/[^/]+/location$').hasMatch(path) &&
            request.method == 'POST') {
          return http.Response('', 204);
        }
        if (RegExp(r'/deliveries/[^/]+/delivered$').hasMatch(path) &&
            request.method == 'POST') {
          deliveredCalled = true;
          jobs = []; // the delivered job leaves the work list.
          final id = _deliveryIdOf(path);
          return _json({
            'store_order_id': 'so-1',
            'status': 'delivered',
            'delivered_at': '2026-08-31T10:15:00Z',
            'delivery_id': id,
          });
        }
        return _json({'error': 'not_found', 'message': 'no stub'}, 404);
      });
}

Widget _harness(_Script script) {
  return ProviderScope(
    overrides: [
      tokenStorageProvider.overrideWithValue(_FakeTokenStorage()),
      apiClientProvider.overrideWithValue(
        ApiClient(httpClient: script.client(), tokenProvider: () => 'token'),
      ),
      sessionProvider.overrideWith(_RiderSession.new),
      // The kiosk's Start needs a GPS fix; in tests the platform channels
      // don't exist, so the seam hands a fixed position instead.
      acquireLocationProvider.overrideWithValue(
        () async => const CustomerLocation(lat: -1.9550, lng: 30.0623),
      ),
    ],
    child: MaterialApp.router(
      theme: AppTheme.dark(),
      routerConfig: buildRouter(),
    ),
  );
}

Future<void> _landOnKiosk(WidgetTester tester, _Script script) async {
  await tester.pumpWidget(_harness(script));
  await tester.pump(const Duration(milliseconds: 50));
  sessionRouterRefresher.notify();
  for (var i = 0; i < 20 && find.byType(RiderScreen).evaluate().isEmpty; i++) {
    await tester.pump(const Duration(milliseconds: 50));
  }
  for (var i = 0; i < 10; i++) {
    await tester.pump(const Duration(milliseconds: 60));
  }
}

/// The delivery id in a path like /api/v1/deliveries/{id}/location.
String _deliveryIdOf(String path) {
  final segments = Uri.parse(path).pathSegments;
  return segments[segments.indexOf('deliveries') + 1];
}

void main() {
  testDesktop('the kiosk renders the assigned job with contact and actions',
      (tester) async {
    final script = _Script()..jobs = [_job()];
    await _landOnKiosk(tester, script);

    // Identity + the handoff interface.
    expect(find.text('#7'), findsOneWidget);
    // The job card: store, destination, receiver, actions.
    expect(find.text("Aline's Kitchen"), findsOneWidget);
    expect(find.text('KN 4 Ave, Kigali'), findsOneWidget);
    expect(find.text('For Chantal'), findsOneWidget);
    expect(find.text('Call'), findsOneWidget);
    expect(find.text('Navigate'), findsOneWidget);
    expect(find.text('Delivered'), findsOneWidget);
    expect(find.text('Start delivering'), findsOneWidget);
    // Desktop: the honest map placeholder, not a fake map.
    expect(find.textContaining('the map renders on your phone'),
        findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testDesktop('no jobs shows the honest empty state', (tester) async {
    final script = _Script();
    await _landOnKiosk(tester, script);

    expect(find.textContaining('No deliveries yet'), findsOneWidget);
    expect(find.text('Start delivering'), findsNothing,
        reason: 'nothing to push — the action only exists with a job');
    expect(tester.takeException(), isNull);
  });

  testDesktop('start delivering pushes the position to every active delivery',
      (tester) async {
    final script = _Script()
      ..jobs = [
        _job(deliveryId: 'delivery-1'),
        _job(deliveryId: 'delivery-2', storeName: 'Simba Supermarket'),
      ];
    await _landOnKiosk(tester, script);

    await tester.tap(find.text('Start delivering'));
    await tester.pump(const Duration(milliseconds: 100));
    await tester.pump(const Duration(seconds: 5));
    await tester.pump(const Duration(milliseconds: 100));

    // The rider is in ONE place — each delivery got the same real fix.
    final pushes = script.requests
        .where((r) => r.url.path.endsWith('/location'))
        .toList();
    expect(pushes.length, 4,
        reason: 'immediate push + the 5s tick, for both deliveries');
    final pushed = {
      for (final r in pushes)
        _deliveryIdOf(r.url.path): jsonDecode(r.body) as Map<String, dynamic>,
    };
    expect(pushed.keys, {'delivery-1', 'delivery-2'});
    expect(pushed['delivery-1']?['lat'], -1.9550);
    expect(pushed['delivery-1']?['lng'], 30.0623);
    expect(tester.takeException(), isNull);
  });

  testDesktop('delivered confirms, settles, and the job leaves the list',
      (tester) async {
    final script = _Script()..jobs = [_job()];
    await _landOnKiosk(tester, script);

    // The job card extends below the fold — scroll the list so the
    // Delivered action is actually on screen (built is not visible).
    await tester.ensureVisible(find.text('Delivered'));
    await tester.pump();
    await tester.tap(find.text('Delivered'));
    await tester.pump();
    // The confirmation names the event: food handed over, cash received.
    expect(find.textContaining('cash is in your hand'), findsOneWidget);
    await tester.tap(find.widgetWithText(FilledButton, 'Delivered').last);
    for (var i = 0; i < 10; i++) {
      await tester.pump(const Duration(milliseconds: 60));
    }

    expect(script.deliveredCalled, isTrue);
    expect(
      script.requests.any((r) => r.url.path.endsWith('/delivered')),
      isTrue,
    );
    // The work list came back empty — the honest idle state returns.
    expect(find.textContaining('No deliveries yet'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });
}
