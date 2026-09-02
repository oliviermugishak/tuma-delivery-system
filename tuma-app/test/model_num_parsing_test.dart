import 'package:flutter_test/flutter_test.dart';

import 'package:tuma_app/core/api/models/discovery.dart';
import 'package:tuma_app/core/api/models/product.dart';
import 'package:tuma_app/core/api/models/rider.dart';
import 'package:tuma_app/core/api/models/store.dart';

/// P17's hard-cast class beyond the order models: Dart's jsonDecode gives
/// back `double` the moment anything upstream re-encodes through `num` (a
/// proxy, a cache, a desktop port). Every integer field must read through
/// the order models' `asInt` pattern — parsed values land as exact ints,
/// never a `type 'double' is not a subtype of type 'int'` crash.
void main() {
  test('double-encoded store and rider payloads parse with exact integers',
      () {
    // Every numeric field arrives as a JSON double.
    final store = Store.fromJson({
      'id': 'store-1',
      'merchant_id': 'merchant-1',
      'name': "Aline's Kitchen",
      'delivery_fee': 1500.0,
      'is_open': true,
      'lat': -1.9512,
      'lng': 30.0623,
      'distance_m': 900.0,
      'eta_min': 3.0,
      'created_at': '2026-08-28T06:55:45Z',
      'updated_at': '2026-08-28T06:55:45Z',
    });
    expect(store.deliveryFee, 1500);
    expect(store.deliveryFee, isA<int>());
    expect(store.distanceM, 900);
    expect(store.distanceM, isA<int>());
    expect(store.etaMin, 3);
    expect(store.etaMin, isA<int>());

    final item = MenuItem.fromJson({
      'id': 'menu-1',
      'name': 'Ibirazi',
      'price': 3500.0,
      'is_available': true,
    });
    expect(item.price, 3500);
    expect(item.price, isA<int>());

    final hit = ProductHit.fromJson({
      'store_product_id': 'menu-1',
      'store_id': 'store-1',
      'store_name': "Aline's Kitchen",
      'name': 'Ibirazi',
      'price': 3500.0,
      'distance_m': 900.0,
      'eta_min': 3.0,
    });
    expect(hit.price, 3500);
    expect(hit.price, isA<int>());
    expect(hit.distanceM, 900);
    expect(hit.distanceM, isA<int>());
    expect(hit.etaMin, 3);
    expect(hit.etaMin, isA<int>());

    final delivery = RiderDelivery.fromJson({
      'delivery_id': 'd-1',
      'store_order_id': 'so-1',
      'number': 1043.0,
      'total': 8500.0,
      'store_name': "Aline's Kitchen",
      'destination_address': 'KN 4 Ave, Kigali',
      'status': 'picked_up',
    });
    expect(delivery.number, 1043);
    expect(delivery.number, isA<int>());
    expect(delivery.total, 8500);
    expect(delivery.total, isA<int>());

    final tally = RiderTally.fromJson({
      'deliveries': 7.0,
      'collected': 24500.0,
    });
    expect(tally.deliveries, 7);
    expect(tally.deliveries, isA<int>());
    expect(tally.collected, 24500);
    expect(tally.collected, isA<int>());

    final history = RiderHistoryEntry.fromJson({
      'delivery_id': 'd-1',
      'number': 1043.0,
      'total': 8500.0,
      'store_name': "Aline's Kitchen",
      'destination_address': 'KN 4 Ave, Kigali',
      'delivered_at': '2026-08-28T07:25:00Z',
    });
    expect(history.number, 1043);
    expect(history.number, isA<int>());
    expect(history.total, 8500);
    expect(history.total, isA<int>());
  });
}
