import 'package:tuma_app/core/api/api_client.dart';
import 'package:tuma_app/core/api/models/store.dart';

/// Typed wrapper around the customer-facing store endpoints. Hand-written
/// per the "no codegen for mobile" rule (see AGENTS.md).
class StoreApi {
  StoreApi(this._client);

  final ApiClient _client;

  /// Open stores. With [lat]/[lng] the server attaches distance + ETA to
  /// every located store and sorts nearest-first; without them the feed
  /// is creation-ordered and the fields come back absent. Customer role
  /// only — the server 403s anyone else.
  Future<List<Store>> listStores({double? lat, double? lng}) async {
    final response = await _client.get(
      '/stores',
      query: {
        if (lat != null && lng != null) 'lat': lat.toString(),
        if (lat != null && lng != null) 'lng': lng.toString(),
      },
    );
    return (response as List)
        .whereType<Map<String, dynamic>>()
        .map(Store.fromJson)
        .toList();
  }

  /// The store and its available products. A closed store is a 404 by
  /// design — callers turn [ApiNotFound] into "not taking orders".
  Future<StoreDetail> getStore(String id) async {
    final response = await _client.get('/stores/$id');
    return StoreDetail.fromJson(response as Map<String, dynamic>);
  }
}
