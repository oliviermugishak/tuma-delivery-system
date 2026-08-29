// The order-domain models of the OpenAPI contract: one checkout is one
// `OrderGroup` carrying one `StoreOrder` per participating store. Money
// is integer RWF — the server owns every number; timestamps stay strings
// until a screen needs them parsed.

/// A frozen line of a store order: name + price snapshotted at order time.
class OrderItem {
  OrderItem({
    required this.storeProductId,
    required this.productId,
    required this.productName,
    required this.unitPrice,
    required this.quantity,
  });

  final String storeProductId;
  final String productId;
  final String productName;
  final int unitPrice; // integer RWF
  final int quantity;

  /// Line total = quantity × unitPrice. The server recomputed it; this is
  /// a convenience for rendering.
  int get lineTotal => unitPrice * quantity;

  factory OrderItem.fromJson(Map<String, dynamic> json) => OrderItem(
        storeProductId: json['store_product_id'] as String,
        productId: json['product_id'] as String,
        productName: json['product_name'] as String,
        unitPrice: json['unit_price'] as int,
        quantity: json['quantity'] as int,
      );
}

/// One store's slice of the checkout — the unit a store fulfills,
/// independently.
class StoreOrder {
  StoreOrder({
    required this.id,
    required this.number,
    required this.storeId,
    required this.storeName,
    required this.status,
    required this.subtotal,
    required this.deliveryFee,
    required this.total,
    required this.items,
  });

  final String id;
  final int number;
  final String storeId;
  final String storeName;
  /// One of: placed, accepted, preparing, picked_up, delivered, cancelled.
  final String status;
  final int subtotal;
  final int deliveryFee;
  final int total;
  final List<OrderItem> items;

  factory StoreOrder.fromJson(Map<String, dynamic> json) => StoreOrder(
        id: json['id'] as String,
        number: json['number'] as int,
        storeId: json['store_id'] as String,
        storeName: json['store_name'] as String,
        status: json['status'] as String,
        subtotal: json['subtotal'] as int,
        deliveryFee: json['delivery_fee'] as int,
        total: json['total'] as int,
        items: (json['items'] as List)
            .whereType<Map<String, dynamic>>()
            .map(OrderItem.fromJson)
            .toList(),
      );
}

/// The customer-facing purchase: one checkout, N store orders, one payment.
/// `status` is derived server-side from the children — never a second
/// state machine.
class OrderGroup {
  OrderGroup({
    required this.id,
    required this.number,
    required this.addressText,
    required this.subtotal,
    required this.deliveryTotal,
    required this.grandTotal,
    required this.status,
    required this.paymentStatus,
    required this.createdAt,
    required this.storeOrders,
    this.addressLat,
    this.addressLng,
  });

  final String id;
  final int number;
  final String addressText;
  final double? addressLat;
  final double? addressLng;
  final int subtotal;
  final int deliveryTotal;
  final int grandTotal;
  /// One of: in_progress, partially_fulfilled, completed, cancelled.
  final String status;
  /// One of: pending, collected, refunded.
  final String paymentStatus;
  final String createdAt; // RFC-3339
  final List<StoreOrder> storeOrders;

  /// True while anything is still moving — the live-tracking poll runs on
  /// exactly these groups.
  bool get isInFlight =>
      status == 'in_progress' || status == 'partially_fulfilled';

  factory OrderGroup.fromJson(Map<String, dynamic> json) => OrderGroup(
        id: json['id'] as String,
        number: json['number'] as int,
        addressText: json['address_text'] as String,
        addressLat: (json['address_lat'] as num?)?.toDouble(),
        addressLng: (json['address_lng'] as num?)?.toDouble(),
        subtotal: json['subtotal'] as int,
        deliveryTotal: json['delivery_total'] as int,
        grandTotal: json['grand_total'] as int,
        status: json['status'] as String,
        paymentStatus: json['payment_status'] as String,
        createdAt: json['created_at'] as String,
        storeOrders: (json['store_orders'] as List)
            .whereType<Map<String, dynamic>>()
            .map(StoreOrder.fromJson)
            .toList(),
      );
}

/// A compact row of the history list — enough for the cards without the
/// items.
class GroupSummary {
  GroupSummary({
    required this.id,
    required this.number,
    required this.grandTotal,
    required this.status,
    required this.stores,
    required this.createdAt,
  });

  final String id;
  final int number;
  final int grandTotal; // integer RWF
  final String status;
  /// Which stores are fulfilling this purchase.
  final List<String> stores;
  final String createdAt; // RFC-3339

  factory GroupSummary.fromJson(Map<String, dynamic> json) => GroupSummary(
        id: json['id'] as String,
        number: json['number'] as int,
        grandTotal: json['grand_total'] as int,
        status: json['status'] as String,
        stores: (json['stores'] as List).whereType<String>().toList(),
        createdAt: json['created_at'] as String,
      );
}

/// What the checkout sends. Store_product ids and quantities only — the
/// client never sends totals; the server computes them all.
class CheckoutRequest {
  CheckoutRequest({
    required this.addressText,
    required this.items,
    this.addressLat,
    this.addressLng,
    this.idempotencyKey,
  });

  final String addressText;
  final double? addressLat;
  final double? addressLng;
  final String? idempotencyKey;
  final List<CheckoutLine> items;

  Map<String, dynamic> toJson() => {
        'address_text': addressText,
        if (addressLat != null) 'address_lat': addressLat,
        if (addressLng != null) 'address_lng': addressLng,
        if (idempotencyKey != null) 'idempotency_key': idempotencyKey,
        'items': items.map((e) => e.toJson()).toList(),
      };
}

class CheckoutLine {
  CheckoutLine({required this.storeProductId, required this.quantity});

  final String storeProductId;
  final int quantity;

  Map<String, dynamic> toJson() => {
        'store_product_id': storeProductId,
        'quantity': quantity,
      };
}
