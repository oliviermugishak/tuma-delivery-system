import 'package:tuma_app/core/api/api_client.dart';
import 'package:tuma_app/core/api/models/discovery.dart';
import 'package:tuma_app/core/api/models/store.dart';

/// Typed wrapper around the customer-facing store endpoints. Hand-written
/// per the "no codegen for mobile" rule (see AGENTS.md).
class StoreApi {
  StoreApi(this._client);

  final ApiClient _client;

  /// Open stores. With [lat]/[lng] the server attaches distance + ETA to
  /// every located store and sorts nearest-first; without them the feed
  /// is creation-ordered and the fields come back absent. [q] is the
  /// server-side search term — matched against name and category,
  /// case-insensitively; empty/null means the plain feed. Customer role
  /// only — the server 403s anyone else.
  Future<List<Store>> listStores({double? lat, double? lng, String? q}) async {
    final term = q?.trim();
    final response = await _client.get(
      '/stores',
      query: {
        if (lat != null && lng != null) 'lat': lat.toString(),
        if (lat != null && lng != null) 'lng': lng.toString(),
        if (term != null && term.isNotEmpty) 'q': term,
      },
    );
    return (response as List)
        .whereType<Map<String, dynamic>>()
        .map(Store.fromJson)
        .toList();
  }

  /// Discovery: matched-or-popular products and matched-or-open stores in
  /// one call. Empty [q] is the default discovery state (popular shelf +
  /// the open feed); a present [q] narrows both sections server-side.
  Future<SearchResult> search({String? q, double? lat, double? lng}) async {
    final term = q?.trim();
    final response = await _client.get(
      '/search',
      query: {
        if (term != null && term.isNotEmpty) 'q': term,
        if (lat != null && lng != null) 'lat': lat.toString(),
        if (lat != null && lng != null) 'lng': lng.toString(),
      },
    );
    return SearchResult.fromJson(response as Map<String, dynamic>);
  }

  /// The store and its available products. A closed store is a 404 by
  /// design — callers turn [ApiNotFound] into "not taking orders".
  Future<StoreDetail> getStore(String id) async {
    final response = await _client.get('/stores/$id');
    return StoreDetail.fromJson(response as Map<String, dynamic>);
  }
}
