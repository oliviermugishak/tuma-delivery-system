import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

import 'package:tuma_app/core/api/api_client.dart';
import 'package:tuma_app/core/api/models/order.dart';
import 'package:tuma_app/core/auth/auth_controller.dart';
import 'package:tuma_app/features/orders/order_detail_screen.dart';

/// Review P17: money and counts arrive as JSON numbers. Dart decodes an
/// integer literal as `int`, but anything upstream that re-encodes through
/// `num` (a caching proxy, a desktop port, a debugger echo) can hand the
/// app `1043.0` — and a hard `as int` cast is a TypeError that used to
/// crash the order screens. Every integer field parses through
/// `(num).toInt()`, and the detail screen's fetch has a catch-all so a
/// payload the parser can't survive lands on the error state, not a crash.
void main() {
  final doubleEncodedGroup = {
    'id': 'group-1',
    'number': 1043.0,
    'address_text': 'KG 7 Ave, Remera',
    'address_lat': -1.9499,
    'address_lng': 30.0622,
    'subtotal': 7000.0,
    'delivery_total': 1500.0,
    'grand_total': 8500.0,
    'status': 'in_progress',
    'payment_status': 'pending',
    'created_at': '2026-08-31T08:00:00Z',
    'store_orders': [
      {
        'id': 'so-1',
        'number': 1043.0,
        'store_id': 'store-1',
        'store_name': "Aline's Kitchen",
        'status': 'preparing',
        'subtotal': 7000.0,
        'delivery_fee': 1500.0,
        'total': 8500.0,
        'items': [
          {
            'store_product_id': 'menu-1',
            'product_id': 'product-1',
            'product_name': 'Ibirazi',
            'unit_price': 3500.0,
            'quantity': 2.0,
          },
        ],
      },
    ],
  };

  test('a double-encoded group parses with exact integer money', () {
    final group = OrderGroup.fromJson(doubleEncodedGroup);

    expect(group.number, 1043);
    expect(group.number, isA<int>());
    expect(group.subtotal, 7000);
    expect(group.deliveryTotal, 1500);
    expect(group.grandTotal, 8500);
    final order = group.storeOrders.single;
    expect(order.number, 1043);
    expect(order.subtotal, 7000);
    expect(order.deliveryFee, 1500);
    expect(order.total, 8500);
    expect(order.items.single.unitPrice, 3500);
    expect(order.items.single.lineTotal, 7000);
    expect(order.items.single.quantity, 2);
  });

  test('a double-encoded summary parses too', () {
    final summary = GroupSummary.fromJson({
      'id': 'group-1',
      'number': 1043.0,
      'grand_total': 8500.0,
      'status': 'in_progress',
      'stores': ["Aline's Kitchen"],
      'created_at': '2026-08-31T08:00:00Z',
    });
    expect(summary.number, 1043);
    expect(summary.grandTotal, 8500);
  });

  test('plain integer payloads still parse exactly as before', () {
    final group = OrderGroup.fromJson({
      ...doubleEncodedGroup,
      'number': 1043,
      'subtotal': 7000,
      'delivery_total': 1500,
      'grand_total': 8500,
      'store_orders': [
        {
          ...(doubleEncodedGroup['store_orders'] as List).first
              as Map<String, dynamic>,
          'number': 1043,
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
    });
    expect(group.grandTotal, 8500);
    expect(group.storeOrders.single.total, 8500);
  });

  testWidgets('a malformed group payload lands on the error state',
      (tester) async {
    final client = MockClient((request) async {
      // A 200 whose body breaks the model contract — the old hard casts
      // threw a TypeError straight out of the fetch.
      return http.Response('{"id":"group-1","number":"not-a-number"}', 200,
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

    // Bounded pumps — the fetch resolves into the catch-all.
    for (var i = 0; i < 12; i++) {
      await tester.pump(const Duration(milliseconds: 60));
    }

    expect(find.text('Something went wrong loading this order.'),
        findsOneWidget);
    expect(tester.takeException(), isNull);
  });
}
