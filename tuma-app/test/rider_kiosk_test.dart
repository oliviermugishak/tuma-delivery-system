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
import 'package:tuma_app/features/rider/rider_profile_screen.dart';
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
  String? storePhone = '+250788000001',
}) =>
    {
      'delivery_id': deliveryId,
      'store_order_id': 'so-1',
      'number': 8,
      'total': 8500,
      'store_name': storeName,
      'store_address': 'KG 7 Ave, Remera',
      'store_lat': -1.9512,
      'store_lng': 30.0623,
      'store_contact_phone': storePhone,
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
  List<Map<String, dynamic>> history = [];
  bool deliveredCalled = false;

  MockClient client() => MockClient((request) async {
        requests.add(request);
        final path = request.url.path;
        if (request.method == 'GET' &&
            path.endsWith('/deliveries/history')) {
          return _json(history);
        }
        if (request.method == 'GET' && path.endsWith('/deliveries/today')) {
          return _json({'deliveries': 0, 'collected': 0});
        }
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

    // The stage card: the stat label, the store stage, the destination,
    // and Navigate as the primary action.
    expect(find.textContaining('STOP 1 OF 1 · PICK UP'), findsOneWidget);
    expect(find.text("Aline's Kitchen"), findsOneWidget);
    expect(find.text('KN 4 Ave, Kigali'), findsOneWidget);
    expect(find.text('Navigate to store'), findsOneWidget);
    expect(find.text('Picked up'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testDesktop('no jobs shows the honest empty state', (tester) async {
    final script = _Script();
    await _landOnKiosk(tester, script);

    // The waiting state (screen 07): online, the number explained, and
    // Start as the way onto the clock.
    expect(find.text("You're online"), findsOneWidget);
    expect(find.text('#7'), findsOneWidget);
    expect(find.text('Start delivering'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testDesktop('confirming pickup auto-starts the push loop to every delivery',
      (tester) async {
    // The founder's contract: Picked Up IS going to work — the GPS loop
    // arms itself at the confirm, with no separate Start tap. The stages
    // branch never shows a Start button at all.
    final script = _Script()
      ..jobs = [
        _job(deliveryId: 'delivery-1'),
        _job(deliveryId: 'delivery-2', storeName: 'Simba Supermarket'),
      ];
    await _landOnKiosk(tester, script);

    // No Start button in the stages view — the state change owns it.
    expect(find.text('Start delivering'), findsNothing);

    // Confirm the pickup on stop 1: the immediate fix goes to BOTH
    // active deliveries (the rider is in one place), and the 5s tick
    // keeps the loop alive.
    await tester.ensureVisible(find.text('Picked up').first);
    await tester.pump();
    await tester.tap(find.text('Picked up').first);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 100));
    await tester.pump(const Duration(seconds: 5));
    await tester.pump(const Duration(milliseconds: 100));

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

  testDesktop('the pick-up stage calls the store with its real number',
      (tester) async {
    final script = _Script()..jobs = [_job()];
    await _landOnKiosk(tester, script);

    // The Store chip is a REAL call action now — it used to render with
    // no number behind it (onTap null: a dead button). The fix: the
    // pick-up chip carries the work list's store_contact_phone.
    await tester.ensureVisible(find.text('Store'));
    await tester.pump();
    final chip = tester.widget<GestureDetector>(
      find
          .ancestor(
            of: find.text('Store'),
            matching: find.byType(GestureDetector),
          )
          .first,
    );
    expect(chip.onTap, isNotNull,
        reason: 'the Pick-up Store chip is tappable — a real call action');
    expect(tester.takeException(), isNull);
  });

  testDesktop('one empty poll does not end the run; two do', (tester) async {
    final script = _Script()..jobs = [_job()];
    await _landOnKiosk(tester, script);

    // Go to work: the confirm arms the push loop and the run.
    await tester.ensureVisible(find.text('Picked up').first);
    await tester.pump();
    await tester.tap(find.text('Picked up').first);
    await tester.pump();

    // The server hiccups: ONE empty work-list poll. The run must survive
    // — still delivering (the status pill and Mark delivered stay).
    script.jobs = [];
    await tester.pump(const Duration(seconds: 15));
    await tester.pump(const Duration(milliseconds: 100));
    expect(find.text('Delivering'), findsOneWidget,
        reason: 'one empty poll is a hiccup, not a finished run');

    // A second consecutive empty poll IS proof: the run ends honestly.
    await tester.pump(const Duration(seconds: 15));
    await tester.pump(const Duration(milliseconds: 100));
    expect(find.text('Start delivering'), findsOneWidget,
        reason: 'two consecutive empties end the run');

    // The marker: one more empty poll changes nothing (already stopped).
    expect(tester.takeException(), isNull);
  });

  testDesktop('delivered confirms, settles, and the job leaves the list',
      (tester) async {
    final script = _Script()..jobs = [_job()];
    await _landOnKiosk(tester, script);

    // The designed sequence (P11): confirm PICK UP first — the stage
    // flips to Deliver, the cash strip surfaces, and only then is Mark
    // delivered reachable (P10: the guard sits at the risky instant).
    await tester.ensureVisible(find.text('Picked up'));
    await tester.pump();
    await tester.tap(find.text('Picked up'));
    await tester.pump();
    // The cash strip is the hero of the Deliver stage (P8).
    expect(find.textContaining('Collect 8,500 RWF cash'), findsOneWidget);
    await tester.ensureVisible(find.text('Mark delivered'));
    await tester.pump();
    await tester.tap(find.text('Mark delivered'));
    await tester.pump();
    // The confirmation restates the amount; the confirm verb is the real
    // event (P9, P10).
    // The strip AND the dialog both restate it (P8 — money at the
    // point of decision, twice by design).
    expect(find.textContaining('8,500 RWF cash'), findsWidgets);
    await tester.tap(find.text('Cash received'));
    for (var i = 0; i < 10; i++) {
      await tester.pump(const Duration(milliseconds: 60));
    }

    expect(script.deliveredCalled, isTrue);
    expect(
      script.requests.any((r) => r.url.path.endsWith('/delivered')),
      isTrue,
    );
    // The work list came back empty — the honest idle state returns.
    // Back to the Waiting state — the online pill and the number.
    expect(find.text("You're online"), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testDesktop('the profile page carries identity, history, and sign-out',
      (tester) async {
    final script = _Script()
      ..history = [
        {
          'delivery_id': 'delivery-9',
          'number': 8,
          'total': 8500,
          'store_name': "Aline's Kitchen",
          'destination_address': 'KN 4 Ave, Kigali',
          'customer_name': 'Chantal',
          'delivered_at': '2026-08-31T10:15:00Z',
        },
      ];
    await _landOnKiosk(tester, script);

    // The kiosk's top chrome opens the profile — sign-out LEFT the main
    // screen (the founder's call).
    await tester.tap(find.byIcon(Icons.account_circle_rounded));
    for (var i = 0;
        i < 20 && find.byType(RiderProfileScreen).evaluate().isEmpty;
        i++) {
      await tester.pump(const Duration(milliseconds: 50));
    }
    await tester.pump(const Duration(milliseconds: 300));

    // Identity + the rider number card.
    expect(find.text('Jean'), findsOneWidget);
    expect(find.text('#7'), findsOneWidget);
    // The delivered history: one run with its cash.
    expect(find.text('Order #8 · Aline\'s Kitchen'), findsOneWidget);
    expect(find.text('8,500 RWF'), findsOneWidget);
    // Sign out lives HERE now — the kiosk's main screen has none.
    expect(find.text('Sign out'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });
}
