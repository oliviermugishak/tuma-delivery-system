import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'package:tuma_app/core/api/models/discovery.dart';
import 'package:tuma_app/core/api/models/product.dart';

// ---------------------------------------------------------------------------
// Cart state — a multi-store cart
// ---------------------------------------------------------------------------

/// One line in the cart. The identity is the store_product (the sellable
/// item of one store); name + price are snapshotted from the menu at add
/// time so the cart remembers what was added even if the menu changes.
class CartItem {
  CartItem({
    required this.storeProductId,
    required this.productId,
    required this.name,
    required this.unitPrice,
    required this.imageUrl,
    required this.quantity,
  });

  final String storeProductId;
  final String productId;
  final String name;
  final int unitPrice; // integer RWF
  final String? imageUrl;
  final int quantity;

  /// Line total = quantity × unitPrice. The server recomputes totals at
  /// checkout; this is just the cart preview.
  int get lineTotal => unitPrice * quantity;

  CartItem copyWith({int? quantity}) => CartItem(
        storeProductId: storeProductId,
        productId: productId,
        name: name,
        unitPrice: unitPrice,
        imageUrl: imageUrl,
        quantity: quantity ?? this.quantity,
      );

  Map<String, dynamic> toJson() => {
        'storeProductId': storeProductId,
        'productId': productId,
        'name': name,
        'unitPrice': unitPrice,
        'imageUrl': imageUrl,
        'quantity': quantity,
      };

  factory CartItem.fromJson(Map<String, dynamic> json) => CartItem(
        storeProductId: json['storeProductId'] as String,
        productId: json['productId'] as String? ?? json['storeProductId'] as String,
        name: json['name'] as String,
        // Same num-safe parse as the wire models (review A42): the cart
        // JSON is app-written, but a num re-encode on restore must not
        // blow up the loader — it degrades to a cart reset instead.
        unitPrice: (json['unitPrice'] as num).toInt(),
        imageUrl: json['imageUrl'] as String?,
        quantity: (json['quantity'] as num).toInt(),
      );
}

/// Everything the cart holds from one store. A multi-store cart is one
/// bucket per store — checkout splits into one store order per bucket.
class StoreBucket {
  StoreBucket({
    required this.storeId,
    required this.storeName,
    required this.deliveryFee,
    required this.items,
  });

  final String storeId;
  final String storeName;
  final int deliveryFee; // integer RWF, displayed only
  final List<CartItem> items;

  int get subtotal => items.fold(0, (sum, item) => sum + item.lineTotal);

  StoreBucket copyWith({List<CartItem>? items}) => StoreBucket(
        storeId: storeId,
        storeName: storeName,
        deliveryFee: deliveryFee,
        items: items ?? this.items,
      );

  Map<String, dynamic> toJson() => {
        'storeId': storeId,
        'storeName': storeName,
        'deliveryFee': deliveryFee,
        'items': items.map((e) => e.toJson()).toList(),
      };

  factory StoreBucket.fromJson(Map<String, dynamic> json) => StoreBucket(
        storeId: json['storeId'] as String,
        storeName: json['storeName'] as String,
        deliveryFee: (json['deliveryFee'] as num).toInt(),
        items: (json['items'] as List? ?? [])
            .whereType<Map<String, dynamic>>()
            .map(CartItem.fromJson)
            .toList(),
      );
}

/// The full cart: per-store buckets, in the order the customer added them.
/// Persisted across restarts via [SharedPreferences].
class CartState {
  const CartState({this.buckets = const []});

  final List<StoreBucket> buckets;

  /// Preview totals (client-side; the server recomputes at checkout).
  int get subtotal =>
      buckets.fold(0, (sum, bucket) => sum + bucket.subtotal);
  int get deliveryTotal =>
      buckets.fold(0, (sum, bucket) => sum + bucket.deliveryFee);
  int get total => subtotal + deliveryTotal;

  bool get isEmpty => buckets.every((bucket) => bucket.items.isEmpty);
  int get itemCount =>
      buckets.fold(0, (sum, bucket) => sum + bucket.items.fold(0, (s, item) => s + item.quantity));

  int get storeCount => buckets.length;

  /// The bucket for one store, or null when the cart doesn't touch it.
  StoreBucket? bucketFor(String storeId) {
    for (final bucket in buckets) {
      if (bucket.storeId == storeId) return bucket;
    }
    return null;
  }
}

// ---------------------------------------------------------------------------
// Persistence helper — one key, one JSON document, corruption-proof
// ---------------------------------------------------------------------------

const _kCart = 'tuma_cart_v2';

Future<CartState> _loadCart() async {
  final prefs = await SharedPreferences.getInstance();
  final raw = prefs.getString(_kCart);
  if (raw == null || raw.isEmpty) return const CartState();
  try {
    final decoded = jsonDecode(raw) as List;
    return CartState(
      buckets: decoded
          .whereType<Map<String, dynamic>>()
          .map(StoreBucket.fromJson)
          .where((bucket) => bucket.items.isNotEmpty)
          .toList(),
    );
  } on Object {
    // Corrupted local data must never crash the app — start fresh.
    await prefs.remove(_kCart);
    return const CartState();
  }
}

Future<void> _saveCart(CartState state) async {
  final prefs = await SharedPreferences.getInstance();
  final live = state.buckets.where((bucket) => bucket.items.isNotEmpty).toList();
  if (live.isEmpty) {
    await prefs.remove(_kCart);
    return;
  }
  await prefs.setString(_kCart, jsonEncode([for (final b in live) b.toJson()]));
}

// ---------------------------------------------------------------------------
// Notifier
// ---------------------------------------------------------------------------

/// Riverpod [AsyncNotifier] that owns the cart. Adding from another store
/// opens a second bucket — nothing is ever wiped behind the customer's
/// back. Persists across restarts; clears on sign-out.
class CartNotifier extends AsyncNotifier<CartState> {
  @override
  Future<CartState> build() async => _loadCart();

  CartState get _current => state.maybeWhen(
        data: (v) => v,
        orElse: () => const CartState(),
      );

  Future<void> _emit(CartState next) async {
    // The in-memory state moves FIRST, synchronously (review P14): the
    // previous order — persist, then assign — let two rapid `add`s race,
    // the second reading the pre-first cart while the first awaited
    // storage, and one of the two lines landing lost. Persistence is
    // still best-effort: a storage failure (disk, plugin) must never
    // break the in-memory cart or crash the add flow.
    state = AsyncData(next);
    try {
      await _saveCart(next);
    } on Object {
      // keep the in-memory state; it just won't survive this restart.
    }
  }

  /// Add a menu item to its store's bucket (opening the bucket when this
  /// is the first item from that store).
  /// Add a search/popular hit straight to the cart. The hit carries the
  /// store id/name and the per-store price — exactly what a line needs.
  Future<void> addFromHit(ProductHit hit) async {
    await add(
      MenuItem(
        id: hit.storeProductId,
        name: hit.name,
        price: hit.price,
        isAvailable: true,
        imageUrl: hit.imageUrl,
      ),
      storeId: hit.storeId,
      storeName: hit.storeName,
      deliveryFee: 0,
    );
  }

  Future<void> add(
    MenuItem item, {
    required String storeId,
    required String storeName,
    required int deliveryFee,
    int quantity = 1,
  }) async {
    assert(quantity >= 1);
    if (quantity <= 0) return;
    final current = _current;
    final buckets = [...current.buckets];
    final index = buckets.indexWhere((bucket) => bucket.storeId == storeId);

    if (index >= 0) {
      final bucket = buckets[index];
      final existing =
          bucket.items.indexWhere((line) => line.storeProductId == item.id);
      final items = existing >= 0
          ? [
              for (var i = 0; i < bucket.items.length; i++)
                i == existing
                    ? bucket.items[i]
                        .copyWith(
                          quantity: bucket.items[i].quantity + quantity,
                        )
                    : bucket.items[i],
            ]
          : [
              ...bucket.items,
              CartItem(
                storeProductId: item.id,
                productId: item.id,
                name: item.name,
                unitPrice: item.price,
                imageUrl: item.displayImage,
                quantity: quantity,
              ),
            ];
      buckets[index] = bucket.copyWith(items: items);
    } else {
      buckets.add(StoreBucket(
        storeId: storeId,
        storeName: storeName,
        deliveryFee: deliveryFee,
        items: [
          CartItem(
            storeProductId: item.id,
            productId: item.id,
            name: item.name,
            unitPrice: item.price,
            imageUrl: item.displayImage,
            quantity: quantity,
          ),
        ],
      ));
    }

    await _emit(CartState(buckets: buckets));
  }

  /// Decrement a line; drop it entirely at zero. A bucket that empties
  /// disappears with its store's delivery fee.
  Future<void> remove(String storeProductId) async {
    final current = _current;
    final buckets = <StoreBucket>[];
    for (final bucket in current.buckets) {
      final index = bucket.items.indexWhere((item) => item.storeProductId == storeProductId);
      if (index < 0) {
        buckets.add(bucket);
        continue;
      }
      final item = bucket.items[index];
      if (item.quantity > 1) {
        final items = [
          for (var i = 0; i < bucket.items.length; i++)
            i == index
                ? item.copyWith(quantity: item.quantity - 1)
                : bucket.items[i],
        ];
        buckets.add(bucket.copyWith(items: items));
      } else {
        final items = [
          for (var i = 0; i < bucket.items.length; i++)
            if (i != index) bucket.items[i],
        ];
        if (items.isNotEmpty) buckets.add(bucket.copyWith(items: items));
      }
    }
    await _emit(CartState(buckets: buckets));
  }

  /// Empty the cart entirely (also used on sign-out).
  Future<void> clear() async {
    await _emit(const CartState());
  }
}

final cartProvider =
    AsyncNotifierProvider<CartNotifier, CartState>(CartNotifier.new);
