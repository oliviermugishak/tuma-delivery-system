import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'package:tuma_app/core/api/models/product.dart';
import 'package:tuma_app/features/cart/cart_notifier.dart';

/// The cart notifier's quantity contract (review P05): a NEW line must
/// carry the requested quantity — the stepper's "Add to cart · 3×" once
/// added a single item while the CTA said 3×price.
void main() {
  test('a new line takes the requested quantity', () async {
    SharedPreferences.setMockInitialValues({});
    final container = ProviderContainer();
    addTearDown(container.dispose);

    final item = MenuItem(id: 'sp-1', name: 'Rice', price: 3500, isAvailable: true);
    await container
        .read(cartProvider.notifier)
        .add(item, storeId: 'store-1', storeName: 'Aline', deliveryFee: 1500, quantity: 3);

    final cart = container.read(cartProvider).requireValue;
    expect(cart.itemCount, 3);
    expect(cart.buckets.single.items.single.quantity, 3);
  });

  test('a new bucket takes the requested quantity too', () async {
    SharedPreferences.setMockInitialValues({});
    final container = ProviderContainer();
    addTearDown(container.dispose);

    final rice = MenuItem(id: 'sp-1', name: 'Rice', price: 3500, isAvailable: true);
    final burger = MenuItem(id: 'sp-2', name: 'Burger', price: 4000, isAvailable: true);
    final notifier = container.read(cartProvider.notifier);
    await notifier.add(rice, storeId: 's1', storeName: 'A', deliveryFee: 0, quantity: 1);
    await notifier.add(burger, storeId: 's2', storeName: 'B', deliveryFee: 0, quantity: 2);

    final cart = container.read(cartProvider).requireValue;
    expect(cart.storeCount, 2);
    expect(
      cart.buckets.firstWhere((b) => b.storeId == 's2').items.single.quantity,
      2,
    );
  });
}
