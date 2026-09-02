import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';

/// In-flight pushes, keyed by route location (review A32): a fast
/// double-tap on a CTA must not stack two identical routes. The key is
/// removed when the route pops, so the same destination can be visited
/// again later — it just can't be pushed twice at once.
final Set<String> _inFlightPushes = {};

/// The app's one guarded navigation: use this for CTA-grade pushes
/// (checkout, track order, store cards) where an eager double-tap would
/// double-navigate. Lesser taps can stay plain `context.push`.
Future<T?> pushOnce<T extends Object?>(
  BuildContext context,
  String location, {
  Object? extra,
}) {
  if (_inFlightPushes.contains(location)) return Future.value(null);
  _inFlightPushes.add(location);
  return context
      .push<T>(location, extra: extra)
      .whenComplete(() => _inFlightPushes.remove(location));
}
