import 'package:flutter/foundation.dart';

/// One active delivery on the rider's work list (GET /v1/deliveries) —
/// the kiosk's job card. Everything the run needs rides along: where it
/// is, where it goes, who receives it (name + `tel:`), the cached road
/// route, and the server's ETA target. Positions the rider pushes come
/// back only as this list refreshes; the rider's own marker is drawn
/// from device GPS, never from the server.
@immutable
class RiderDelivery {
  const RiderDelivery({
    required this.deliveryId,
    required this.storeOrderId,
    required this.number,
    required this.total,
    required this.storeName,
    this.storeAddress,
    this.storeLat,
    this.storeLng,
    this.storeContactPhone,
    required this.destinationAddress,
    this.destinationLat,
    this.destinationLng,
    this.customerName,
    this.customerPhone,
    this.customerNote,
    required this.status,
    this.handoffAt,
    this.routePolyline,
    this.etaTarget,
    this.lastLat,
    this.lastLng,
    this.lastLocationAt,
  });

  final String deliveryId;
  final String storeOrderId;

  /// The customer-facing order number ("Order #8").
  final int number;

  /// The cash this stop collects (the accent strip's fact, P8).
  final int total;
  final String storeName;
  final String? storeAddress;
  final double? storeLat;
  final double? storeLng;
  /// The store's phone — the Pick-up stage's Call-the-store action.
  final String? storeContactPhone;
  final String destinationAddress;
  final double? destinationLat;
  final double? destinationLng;
  final String? customerName;
  final String? customerPhone;
  /// The checkout's rider note ("blue gate, ring the bell").
  final String? customerNote;
  final String status;
  final DateTime? handoffAt;
  final String? routePolyline;
  final DateTime? etaTarget;
  final double? lastLat;
  final double? lastLng;
  final DateTime? lastLocationAt;

  factory RiderDelivery.fromJson(Map<String, dynamic> json) => RiderDelivery(
        deliveryId: json['delivery_id'] as String,
        storeOrderId: json['store_order_id'] as String,
        number: json['number'] as int,
        total: json['total'] as int,
        storeName: json['store_name'] as String,
        storeAddress: json['store_address'] as String?,
        storeLat: (json['store_lat'] as num?)?.toDouble(),
        storeLng: (json['store_lng'] as num?)?.toDouble(),
        storeContactPhone: json['store_contact_phone'] as String?,
        destinationAddress: json['destination_address'] as String,
        destinationLat: (json['destination_lat'] as num?)?.toDouble(),
        destinationLng: (json['destination_lng'] as num?)?.toDouble(),
        customerName: json['customer_name'] as String?,
        customerPhone: json['customer_phone'] as String?,
        customerNote: json['customer_note'] as String?,
        status: json['status'] as String,
        handoffAt: json['handoff_at'] is String
            ? DateTime.parse(json['handoff_at'] as String)
            : null,
        routePolyline: json['route_polyline'] as String?,
        etaTarget: json['eta_target'] is String
            ? DateTime.parse(json['eta_target'] as String)
            : null,
        lastLat: (json['last_lat'] as num?)?.toDouble(),
        lastLng: (json['last_lng'] as num?)?.toDouble(),
        lastLocationAt: json['last_location_at'] is String
            ? DateTime.parse(json['last_location_at'] as String)
            : null,
      );
}

/// The server's answer to the rider's Delivered action
/// (POST /v1/deliveries/{id}/delivered) — the settled event.
@immutable
class DeliveredResult {
  const DeliveredResult({
    required this.storeOrderId,
    required this.status,
    required this.deliveredAt,
  });

  final String storeOrderId;
  final String status;
  final DateTime deliveredAt;

  factory DeliveredResult.fromJson(Map<String, dynamic> json) =>
      DeliveredResult(
        storeOrderId: json['store_order_id'] as String,
        status: json['status'] as String,
        deliveredAt: DateTime.parse(json['delivered_at'] as String),
      );
}


/// The rider's day so far — the Waiting card's motivation line.
class RiderTally {
  const RiderTally({required this.deliveries, required this.collected});

  final int deliveries;
  final int collected;

  factory RiderTally.fromJson(Map<String, dynamic> json) => RiderTally(
        deliveries: json['deliveries'] as int,
        collected: json['collected'] as int,
      );
}

/// One completed stop in the rider's history (GET /v1/deliveries/history)
/// — the profile page's run record: what was delivered, for whom, and
/// the cash it collected.
@immutable
class RiderHistoryEntry {
  const RiderHistoryEntry({
    required this.deliveryId,
    required this.number,
    required this.total,
    required this.storeName,
    required this.destinationAddress,
    this.customerName,
    required this.deliveredAt,
  });

  final String deliveryId;

  /// The customer-facing order number ("Order #8").
  final int number;

  /// The cash this stop collected.
  final int total;
  final String storeName;
  final String destinationAddress;
  final String? customerName;
  final DateTime deliveredAt;

  factory RiderHistoryEntry.fromJson(Map<String, dynamic> json) =>
      RiderHistoryEntry(
        deliveryId: json['delivery_id'] as String,
        number: json['number'] as int,
        total: json['total'] as int,
        storeName: json['store_name'] as String,
        destinationAddress: json['destination_address'] as String,
        customerName: json['customer_name'] as String?,
        deliveredAt: DateTime.parse(json['delivered_at'] as String),
      );
}
