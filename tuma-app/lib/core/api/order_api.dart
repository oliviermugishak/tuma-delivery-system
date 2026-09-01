import 'package:tuma_app/core/api/api_client.dart';
import 'package:tuma_app/core/api/models/order.dart';
import 'package:tuma_app/core/api/models/tracking.dart';

/// Typed wrapper around the customer-facing order endpoints. Hand-written
/// per the "no codegen for mobile" rule (see AGENTS.md).
class OrderApi {
  OrderApi(this._client);

  final ApiClient _client;

  /// The tracking snapshot for one group (the customer map world's data).
  /// Returns **null on 204** — the client sent `since` and nothing changed
  /// server-side: keep rendering, spend nothing. A fresh snapshot (or a
  /// first poll without `since`) returns the full [GroupTracking].
  Future<GroupTracking?> trackGroup(String groupId, {DateTime? since}) async {
    final response = await _client.get('/orders/$groupId/tracking', query: {
      if (since != null)
        'since': since.toUtc().toIso8601String(),
    });
    if (response == null) return null;
    return GroupTracking.fromJson(response as Map<String, dynamic>);
  }

  /// Place the checkout: one call, one order group — even when the cart
  /// spans many stores; the server splits it per store. A retried request
  /// with the same [request.idempotencyKey] returns the group it already
  /// created (the server answers 200 instead of 201 — same shape).
  Future<OrderGroup> checkout(CheckoutRequest request) async {
    final response = await _client.post('/orders', body: request.toJson());
    return OrderGroup.fromJson(response as Map<String, dynamic>);
  }

  /// Order groups, newest first. [limit] caps the page (1-200, default
  /// 50); [offset] pages past it.
  Future<List<GroupSummary>> listGroups({int limit = 50, int offset = 0}) async {
    final response = await _client
        .getList('/orders', query: {'limit': '$limit', 'offset': '$offset'});
    return response
        .whereType<Map<String, dynamic>>()
        .map(GroupSummary.fromJson)
        .toList();
  }

  /// One order group with its store orders and item snapshots. Another
  /// customer's group returns [ApiNotFound] — indistinguishable from a
  /// missing one.
  Future<OrderGroup> getGroup(String id) async {
    final response = await _client.get('/orders/$id');
    return OrderGroup.fromJson(response as Map<String, dynamic>);
  }

  /// Cancel one store order of the customer's own group — allowed while
  /// the order is still on the premises (placed, accepted, preparing).
  /// An already-out order is an [ApiBadRequest]; a foreign one a 404.
  Future<StoreOrder> cancelStoreOrder(String groupId, String storeOrderId) async {
    final response = await _client.post(
      '/orders/$groupId/store-orders/$storeOrderId/cancel',
      body: {'reason': null},
    );
    return StoreOrder.fromJson(response as Map<String, dynamic>);
  }
}
