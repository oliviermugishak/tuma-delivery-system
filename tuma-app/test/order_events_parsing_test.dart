import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

import 'package:tuma_app/core/api/api_client.dart';
import 'package:tuma_app/core/api/models/order.dart';
import 'package:tuma_app/core/auth/auth_controller.dart';
import 'package:tuma_app/core/utils/format_time.dart';
import 'package:tuma_app/features/orders/order_detail_screen.dart';

/// The status trail (migration 15): the detail response carries each
/// store order's `events` — the ledger's memory, rendered as the
/// timeline's milestone line. Mutation responses (checkout, the cancel
/// call) leave it out, and orders from before the migration have none,
/// so absent must parse to an honest empty list — never a crash.
void main() {
  final groupWithEvents = {
    'id': 'group-1',
    'number': 1043,
    'address_text': 'KG 7 Ave, Remera',
    'address_lat': -1.9499,
    'address_lng': 30.0622,
    'subtotal': 7000,
    'delivery_total': 1500,
    'grand_total': 8500,
    'status': 'completed',
    'payment_status': 'collected',
    'created_at': '2026-08-31T08:00:00Z',
    'store_orders': [
      {
        'id': 'so-1',
        'number': 1043,
        'store_id': 'store-1',
        'store_name': "Aline's Kitchen",
        'status': 'delivered',
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
        'events': [
          {
            'status': 'placed',
            'at': '2026-08-31T08:00:00Z',
            'actor': 'customer',
          },
          {
            'status': 'delivered',
            'at': '2026-08-31T09:30:00Z',
            'actor': 'rider',
          },
        ],
      },
    ],
  };

  test('events present parse into the trail', () {
    final group = OrderGroup.fromJson(groupWithEvents);
    final events = group.storeOrders.single.events;
    expect(events.length, 2);
    expect(events[0].status, 'placed');
    expect(events[0].actor, 'customer');
    expect(events[0].at, DateTime.parse('2026-08-31T08:00:00Z'));
    expect(events[1].status, 'delivered');
    expect(events[1].actor, 'rider');
  });

  test('absent events parse to an empty list', () {
    final storeOrderJson = {
      ...(groupWithEvents['store_orders'] as List).first
          as Map<String, dynamic>,
    }..remove('events');
    final order = StoreOrder.fromJson(storeOrderJson);
    expect(order.events, isEmpty);
  });

  test('a partial event parses with null at and actor', () {
    final event = OrderEvent.fromJson({'status': 'picked_up'});
    expect(event.status, 'picked_up');
    expect(event.at, isNull);
    expect(event.actor, isNull);
  });

  testWidgets('the detail screen renders the milestone line from the trail',
      (tester) async {
    final client = MockClient((request) async {
      // The tracking snapshot is optional: a miss keeps the group's own
      // facts on screen (the poll never starts — the group is settled).
      if (request.url.path.endsWith('/tracking')) {
        return http.Response('{"error":"not found"}', 404,
            headers: {'content-type': 'application/json'});
      }
      return http.Response(jsonEncode(groupWithEvents), 200,
          headers: {'content-type': 'application/json'});
    });
    await tester.pumpWidget(ProviderScope(
      overrides: [
        apiClientProvider.overrideWithValue(
          ApiClient(httpClient: client, tokenProvider: () => 'test-token'),
        ),
      ],
      child: const MaterialApp(home: OrderDetailScreen(orderId: 'group-1')),
    ));

    // Bounded pumps — the fetch resolves and the section paints.
    for (var i = 0; i < 12; i++) {
      await tester.pump(const Duration(milliseconds: 60));
    }

    // The milestone line, in trail order, each label clocked in local
    // time — built by the same clockTime the screen uses.
    final placed = DateTime.parse('2026-08-31T08:00:00Z');
    final delivered = DateTime.parse('2026-08-31T09:30:00Z');
    expect(
      find.text('Placed ${clockTime(placed)} · Delivered ${clockTime(delivered)}'),
      findsOneWidget,
    );
    expect(tester.takeException(), isNull);
  });
}
