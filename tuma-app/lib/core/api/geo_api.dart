import 'package:tuma_app/core/api/api_client.dart';

/// One geocode hit — what the location editor renders and saves.
class GeoHit {
  const GeoHit({
    required this.addressText,
    required this.lat,
    required this.lng,
  });

  final String addressText;
  final double lat;
  final double lng;

  factory GeoHit.fromJson(Map<String, dynamic> json) => GeoHit(
        addressText: json['address_text'] as String,
        lat: (json['lat'] as num).toDouble(),
        lng: (json['lng'] as num).toDouble(),
      );
}

/// The server's geocoding proxy (the Google key never reaches a client).
/// Both calls are customer-guarded; a missing/failed backend surfaces as
/// ApiError and the screen degrades to map-tap (P2).
class GeoApi {
  GeoApi(this._client);

  final ApiClient _client;

  /// Addresses for a text query ("Kk 40 Street Kigali"), best first.
  Future<List<GeoHit>> search(String q) async {
    final response = await _client.getList(
      '/geo/search',
      query: {'q': q},
    );
    return response
        .whereType<Map<String, dynamic>>()
        .map(GeoHit.fromJson)
        .toList();
  }

  /// Addresses at a point (the map pin's drop), best first.
  Future<List<GeoHit>> reverse({required double lat, required double lng}) async {
    final response = await _client.getList(
      '/geo/reverse',
      query: {'lat': '$lat', 'lng': '$lng'},
    );
    return response
        .whereType<Map<String, dynamic>>()
        .map(GeoHit.fromJson)
        .toList();
  }
}
