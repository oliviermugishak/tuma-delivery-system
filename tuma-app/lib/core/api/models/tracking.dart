import 'package:flutter/foundation.dart';

/// The tracking snapshot for one order group — the customer map world's
/// data (GET /orders/{id}/tracking). One delivery entry per store order,
/// each its own story (the multi-store rule). The snapshot carries
/// `changedAt`: the client echoes it back as `since` on the next poll and
/// the server answers 204 when nothing moved — battery and payload win.
@immutable
class GroupTracking {
  const GroupTracking({
    required this.groupId,
    required this.groupStatus,
    required this.paymentStatus,
    required this.changedAt,
    required this.deliveries,
  });

  final String groupId;
  final String groupStatus;
  final String paymentStatus;
  final DateTime changedAt;
  final List<DeliveryTracking> deliveries;

  factory GroupTracking.fromJson(Map<String, dynamic> json) => GroupTracking(
        groupId: json['group_id'] as String,
        groupStatus: json['group_status'] as String,
        paymentStatus: json['payment_status'] as String,
        changedAt: DateTime.parse(json['changed_at'] as String),
        deliveries: (json['deliveries'] as List? ?? [])
            .whereType<Map<String, dynamic>>()
            .map(DeliveryTracking.fromJson)
            .toList(),
      );

  /// The tracking entry for one store order — the client joins it to the
  /// group detail's store-order card by id.
  DeliveryTracking? forStoreOrder(String storeOrderId) {
    for (final delivery in deliveries) {
      if (delivery.storeOrderId == storeOrderId) return delivery;
    }
    return null;
  }
}

/// One delivery's tracking view: the real positions (or nothing — no
/// geometry is ever invented), the cached road route, and the ETA target
/// the client decays against.
@immutable
class DeliveryTracking {
  const DeliveryTracking({
    required this.storeOrderId,
    required this.storeName,
    this.storeLat,
    this.storeLng,
    this.storeContactPhone,
    required this.status,
    this.handoffAt,
    this.routePolyline,
    this.etaTarget,
    this.lastLat,
    this.lastLng,
    this.lastLocationAt,
    required this.updatedAt,
    required this.trail,
  });

  final String storeOrderId;
  final String storeName;
  final double? storeLat;
  final double? storeLng;
  final String? storeContactPhone;
  final String status;
  final DateTime? handoffAt;
  final String? routePolyline;
  final DateTime? etaTarget;
  final double? lastLat;
  final double? lastLng;
  final DateTime? lastLocationAt;

  /// The delivery's last write — for a settled delivery, the delivered
  /// moment the "Delivered · time" line shows.
  final DateTime updatedAt;
  final List<TrailPoint> trail;

  factory DeliveryTracking.fromJson(Map<String, dynamic> json) =>
      DeliveryTracking(
        storeOrderId: json['store_order_id'] as String,
        storeName: json['store_name'] as String,
        storeLat: (json['store_lat'] as num?)?.toDouble(),
        storeLng: (json['store_lng'] as num?)?.toDouble(),
        storeContactPhone: json['store_contact_phone'] as String?,
        status: json['status'] as String,
        handoffAt: (json['handoff_at'] as String?) is String
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
        updatedAt: DateTime.parse(json['updated_at'] as String),
        trail: (json['trail'] as List? ?? [])
            .whereType<Map<String, dynamic>>()
            .map(TrailPoint.fromJson)
            .toList(),
      );

  /// The rider's freshest real position, or null when their phone hasn't
  /// checked in yet.
  bool get hasRiderPosition => lastLat != null && lastLng != null;

  /// How stale the rider's last signal is, in minutes — the ladder's
  /// honesty line ("last signal — N min ago").
  int? get signalAgeMinutes {
    final at = lastLocationAt;
    if (at == null) return null;
    return DateTime.now().difference(at).inMinutes;
  }

  /// How far past the ETA the delivery is, or null when no ETA is set or
  /// it hasn't arrived yet. The ladder's lagging/ended conditions.
  Duration? get overdueBy {
    final target = etaTarget;
    if (target == null) return null;
    final past = DateTime.now().difference(target);
    return past.isNegative ? null : past;
  }
}

/// One recorded point on the rider's trail — a real breadcrumb, oldest
/// first for drawing.
@immutable
class TrailPoint {
  const TrailPoint({
    required this.lat,
    required this.lng,
    required this.recordedAt,
  });

  final double lat;
  final double lng;
  final DateTime recordedAt;

  factory TrailPoint.fromJson(Map<String, dynamic> json) => TrailPoint(
        lat: (json['lat'] as num).toDouble(),
        lng: (json['lng'] as num).toDouble(),
        recordedAt: DateTime.parse(json['recorded_at'] as String),
      );
}
