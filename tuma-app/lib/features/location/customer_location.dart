import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:geolocator/geolocator.dart';
import 'package:shared_preferences/shared_preferences.dart';

// ---------------------------------------------------------------------------
// Customer location — one pin, persisted
// ---------------------------------------------------------------------------

/// The customer's known delivery location. Home uses it for nearest-first
/// distances; checkout seeds its map with it; every successful GPS fix or
/// map pick rewrites it. It follows the cart's persistence pattern:
/// one key, one JSON document, corruption-proof.
class CustomerLocation {
  const CustomerLocation({required this.lat, required this.lng});

  final double lat;
  final double lng;

  Map<String, dynamic> toJson() => {'lat': lat, 'lng': lng};

  factory CustomerLocation.fromJson(Map<String, dynamic> json) =>
      CustomerLocation(
        lat: (json['lat'] as num).toDouble(),
        lng: (json['lng'] as num).toDouble(),
      );
}

const _kLocation = 'tuma_location_v1';

Future<CustomerLocation?> _load() async {
  final prefs = await SharedPreferences.getInstance();
  final raw = prefs.getString(_kLocation);
  if (raw == null || raw.isEmpty) return null;
  try {
    return CustomerLocation.fromJson(jsonDecode(raw) as Map<String, dynamic>);
  } on Object {
    // Corrupted local data must never crash the app — start fresh.
    await prefs.remove(_kLocation);
    return null;
  }
}

Future<void> _save(CustomerLocation? location) async {
  final prefs = await SharedPreferences.getInstance();
  if (location == null) {
    await prefs.remove(_kLocation);
    return;
  }
  await prefs.setString(_kLocation, jsonEncode(location.toJson()));
}

/// The customer's persisted pin — null until a GPS fix or a checkout-map
/// tap establishes it. Cleared on sign-out like the cart.
final customerLocationProvider =
    AsyncNotifierProvider<CustomerLocationNotifier, CustomerLocation?>(
  CustomerLocationNotifier.new,
);

class CustomerLocationNotifier extends AsyncNotifier<CustomerLocation?> {
  @override
  Future<CustomerLocation?> build() => _load();

  /// Pin from the checkout map (or any manual choice).
  Future<void> setPin(CustomerLocation location) => _emit(location);

  Future<void> clear() => _emit(null);

  Future<void> _emit(CustomerLocation? location) async {
    // Persistence is best-effort: a storage failure must never break the
    // flow that produced the pin.
    try {
      await _save(location);
    } on Object {
      // keep the in-memory state; it just won't survive this restart.
    }
    state = AsyncData(location);
  }
}

// ---------------------------------------------------------------------------
// GPS acquisition
// ---------------------------------------------------------------------------

/// The GPS acquisition call, as a provider so tests can override it —
/// a real platform-channel call inside a widget test doesn't fail, it
/// hangs forever. Production value is [acquireLocation].
final acquireLocationProvider =
    Provider<Future<CustomerLocation> Function()>((ref) => acquireLocation);

/// One GPS acquisition: service check → the OS permission prompt (the
/// launch prompt the founder chose) → current position. Every failure
/// throws so callers fall back cleanly — to the persisted pin, then to
/// the bare feed. There is no geolocator Linux implementation, so
/// desktop development always takes the fallback path.
Future<CustomerLocation> acquireLocation() async {
  if (!await Geolocator.isLocationServiceEnabled()) {
    throw StateError('location services are off');
  }
  var permission = await Geolocator.checkPermission();
  if (permission == LocationPermission.denied) {
    permission = await Geolocator.requestPermission();
  }
  if (permission == LocationPermission.denied ||
      permission == LocationPermission.deniedForever) {
    throw StateError('location permission not granted');
  }
  final position = await Geolocator.getCurrentPosition(
    locationSettings: const LocationSettings(
      accuracy: LocationAccuracy.high,
      timeLimit: Duration(seconds: 10),
    ),
  );
  return CustomerLocation(lat: position.latitude, lng: position.longitude);
}
