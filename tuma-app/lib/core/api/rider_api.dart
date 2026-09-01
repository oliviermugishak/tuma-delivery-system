import 'package:tuma_app/core/api/api_client.dart';
import 'package:tuma_app/core/api/models/rider.dart';

/// The rider kiosk's typed client (hand-written per the no-codegen rule):
/// the work list, the location push (204 → void), and the Delivered
/// action that settles the cash. All three ride on the same bearer token
/// as every other client.
class RiderApi {
  RiderApi(this._client);

  final ApiClient _client;

  /// The rider's active deliveries — every store order handed to them and
  /// still `picked_up`, freshest handoff first.
  Future<List<RiderDelivery>> listActiveDeliveries() async {
    final response = await _client.get('/deliveries');
    return (response as List)
        .whereType<Map<String, dynamic>>()
        .map(RiderDelivery.fromJson)
        .toList();
  }

  /// One breadcrumb: the rider's real position for one delivery. The
  /// server answers 204 whether it recorded or throttled (≥25m/≥15s) —
  /// both are successes and carry no body.
  Future<void> pushLocation(String deliveryId, double lat, double lng) async {
    await _client.post(
      '/deliveries/$deliveryId/location',
      body: {'lat': lat, 'lng': lng},
    );
  }

  /// The day so far — deliveries + cash collected (the Waiting card).
  Future<RiderTally> today() async {
    final response = await _client.get('/deliveries/today');
    return RiderTally.fromJson(response as Map<String, dynamic>);
  }

  /// Every delivery the rider completed, newest first (the profile
  /// page's history list — the tally shows the money, this shows the
  /// runs). Latest 50, server-capped.
  Future<List<RiderHistoryEntry>> history() async {
    final response = await _client.get('/deliveries/history');
    return (response as List)
        .whereType<Map<String, dynamic>>()
        .map(RiderHistoryEntry.fromJson)
        .toList();
  }

  /// The handover: food given, cash received, one real event. The server
  /// advances the store order and settles the delivery's payment
  /// allocation; the delivery leaves the work list.
  Future<DeliveredResult> markDelivered(String deliveryId) async {
    final response =
        await _client.post('/deliveries/$deliveryId/delivered');
    return DeliveredResult.fromJson(response as Map<String, dynamic>);
  }
}
