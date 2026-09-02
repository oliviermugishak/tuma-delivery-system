import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

import 'package:tuma_app/core/api/api_client.dart';
import 'package:tuma_app/core/api/models/authenticated_user.dart';
import 'package:tuma_app/core/api/models/order.dart';
import 'package:tuma_app/core/auth/auth_controller.dart';
import 'package:tuma_app/features/orders/active_orders_provider.dart';

/// Review P18: the 5s poll used to write the state on EVERY tick, even
/// when the list was byte-for-byte the same — every write rebuilds every
/// watcher's card. The tick must compare ids+statuses+etaTargets first
/// and skip the write when nothing changed.
void main() {
  ProviderContainer containerWith(ApiClient client) {
    return ProviderContainer(overrides: [
      apiClientProvider.overrideWithValue(client),
      sessionProvider.overrideWith(_SignedInSession.new),
    ]);
  }

  String listBody({
    String status = 'in_progress',
    String? etaTarget,
  }) =>
      '['
      '{"id":"group-1","number":1043,"grand_total":8500,"status":"$status",'
      '"stores":["Aline\'s Kitchen"],"items_count":2,'
      '"created_at":"2026-08-31T08:00:00Z"'
      '${etaTarget == null ? '' : ',"eta_target":"$etaTarget"'}'
      '}]';

  testWidgets('an unchanged poll does not rewrite the state', (tester) async {
    final eta = DateTime.now().toUtc().add(const Duration(minutes: 15));
    final etaIso = eta.toIso8601String();
    var fetches = 0;
    final client = MockClient((request) async {
      // ApiClient prefixes /api/v1.
      if (request.url.path.endsWith('/orders')) {
        fetches++;
        // ALWAYS the same list — identical ids, statuses, etaTarget.
        return http.Response(listBody(etaTarget: etaIso), 200,
            headers: {'content-type': 'application/json'});
      }
      return http.Response('{"error":"not_found"}', 404);
    });

    final container = containerWith(
        ApiClient(httpClient: client, tokenProvider: () => 'test-token'));

    // Warm the session first — the notifier watches it as the account
    // boundary, and an initialized session keeps the build chain's first
    // await deterministic.
    await container.read(sessionProvider.future);

    final states = <List<GroupSummary>>[];
    final sub = container.listen(activeOrdersProvider, (_, next) {
      next.whenData(states.add);
    });

    // First read resolves the initial fetch.
    await container.read(activeOrdersProvider.future);

    // Two 5s poll ticks with identical answers.
    await tester.pump(const Duration(seconds: 5));
    await tester.pump(const Duration(milliseconds: 100));
    await tester.pump(const Duration(seconds: 5));
    await tester.pump(const Duration(milliseconds: 100));

    expect(fetches, 3, reason: 'initial fetch + two ticks');
    expect(
      states.length,
      1,
      reason: 'identical answers must NOT produce state writes — one '
          'initial data state only',
    );

    // Tear down INSIDE the body: the periodic poll timer must die before
    // the framework's pending-timer invariant check.
    sub.close();
    container.dispose();
  }, timeout: const Timeout(Duration(seconds: 15)));

  testWidgets('a changed poll still writes the state', (tester) async {
    var fetches = 0;
    final client = MockClient((request) async {
      if (request.url.path.endsWith('/orders')) {
        fetches++;
        // The status flips on the second answer — the tick MUST write.
        return http.Response(
            listBody(
                status: fetches == 1 ? 'in_progress' : 'partially_fulfilled'),
            200,
            headers: {'content-type': 'application/json'});
      }
      return http.Response('{"error":"not_found"}', 404);
    });

    final container = containerWith(
        ApiClient(httpClient: client, tokenProvider: () => 'test-token'));

    await container.read(sessionProvider.future);

    final states = <List<GroupSummary>>[];
    final sub = container.listen(activeOrdersProvider, (_, next) {
      next.whenData(states.add);
    });

    await container.read(activeOrdersProvider.future);
    await tester.pump(const Duration(seconds: 5));
    await tester.pump(const Duration(milliseconds: 100));

    expect(fetches, 2);
    expect(states.length, 2, reason: 'a real change lands as a new state');
    expect(states.last.single.status, 'partially_fulfilled');

    sub.close();
    container.dispose();
  }, timeout: const Timeout(Duration(seconds: 15)));
}

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
