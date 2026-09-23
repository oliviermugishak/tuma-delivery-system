import 'dart:math' as math;

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:google_maps_flutter/google_maps_flutter.dart';

import 'package:tuma_app/core/api/models/tracking.dart';
import 'package:tuma_app/core/theme/app_colors.dart';

/// The delivery map — the Moving world's lead (tracking doc §1/§4). Real
/// geometry only: the cached road route (gold), the rider's real trail
/// (teal), the rider marker at their last real position (orange), and the
/// store/destination markers. Between two real points the marker glides
/// across the poll interval — motion only between real positions, zero
/// extra calls (the doc's interpolation trick). Linux has no Google Maps
/// target, so desktop development renders an honest data placeholder —
/// real position text, never a fake map.
class DeliveryMap extends StatefulWidget {
  const DeliveryMap({
    super.key,
    required this.tracking,
    this.destinationLat,
    this.destinationLng,
  });

  final DeliveryTracking tracking;
  final double? destinationLat;
  final double? destinationLng;

  @override
  State<DeliveryMap> createState() => _DeliveryMapState();
}

class _DeliveryMapState extends State<DeliveryMap>
    with TickerProviderStateMixin {
  GoogleMapController? _controller;
  bool _followRider = true;

  /// Programmatic camera moves also raise onCameraMoveStarted; counting
  /// them lets the user's own pan still win the follow state.
  int _programmaticMoves = 0;

  /// The marker glides from the previous real point to the newest across
  /// the poll interval (the screen polls every 5 s — the same cadence
  /// here, without knowing about the screen).
  static const _glideDuration = Duration(seconds: 5);

  late final AnimationController _glide = AnimationController(
    vsync: this,
    duration: _glideDuration,
    value: 1,
  );
  _LatLngTween? _glideTween;

  @override
  void initState() {
    super.initState();
    // The glide re-renders the marker as it moves — but the whole widget
    // subtree (including the GoogleMap platform-view widget) rebuilds on
    // every setState, so per-frame 60fps updates were ~300 rebuilds per
    // 5s glide. Throttled to ~10fps: still reads as smooth motion for a
    // cross-town moto, at a tenth of the widget churn.
    _glide.addListener(_throttledGlideRepaint);
  }

  DateTime _lastGlideRepaint = DateTime.fromMillisecondsSinceEpoch(0);

  void _throttledGlideRepaint() {
    if (!mounted || !_glide.isAnimating) return;
    final now = DateTime.now();
    if (now.difference(_lastGlideRepaint) < const Duration(milliseconds: 100)) {
      return;
    }
    _lastGlideRepaint = now;
    setState(() {});
  }

  @override
  void didUpdateWidget(DeliveryMap oldWidget) {
    super.didUpdateWidget(oldWidget);
    final tracking = widget.tracking;
    if (!tracking.hasRiderPosition) return;
    final real = LatLng(tracking.lastLat!, tracking.lastLng!);

    if (!oldWidget.tracking.hasRiderPosition) {
      // First check-in: be there — nothing to glide from.
      setState(() {
        _glideTween = null;
        _glide.value = 1;
      });
      _followIfEnabled(real);
      return;
    }
    final previous = LatLng(
      oldWidget.tracking.lastLat!,
      oldWidget.tracking.lastLng!,
    );
    if (previous == real) return; // Same real point — nothing moved.
    // Glide from wherever the marker is drawn now (possibly mid-glide) to
    // the newest real position.
    final from = _drawnPosition ?? previous;
    setState(() => _glideTween = _LatLngTween(begin: from, end: real));
    _glide.forward(from: 0);
    _followIfEnabled(real);
  }

  @override
  void dispose() {
    _glide.dispose();
    super.dispose();
  }

  /// The marker's drawn position: the gliding interpolation while a glide
  /// runs, the real position otherwise. Never anything else — the drawn
  /// path is always between two real points.
  LatLng? get _drawnPosition {
    final tracking = widget.tracking;
    if (!tracking.hasRiderPosition) return null;
    final real = LatLng(tracking.lastLat!, tracking.lastLng!);
    final tween = _glideTween;
    if (tween == null || _glide.value >= 1) return real;
    return tween.evaluate(_glide);
  }

  void _followIfEnabled(LatLng target) {
    if (!_followRider) return;
    final controller = _controller;
    if (controller == null) return;
    _programmaticMoves += 1;
    controller.animateCamera(CameraUpdate.newLatLng(target)).whenComplete(() {
      _programmaticMoves = math.max(0, _programmaticMoves - 1);
    });
  }

  bool get _hasDestination =>
      widget.destinationLat != null && widget.destinationLng != null;

  @override
  Widget build(BuildContext context) {
    // Linux/desktop: no Google Maps target exists — the honest placeholder
    // presents the same real data as text (verified on a phone).
    if (!isMobilePlatform) {
      return _DesktopMapPlaceholder(tracking: widget.tracking);
    }
    if (!_hasDestination) {
      // No pin, no map: the §3 fallback lives in the card above; this
      // branch only guards the widget itself.
      return const SizedBox.shrink();
    }

    // The store marker is optional — a store without coordinates must
    // not crash the screen (the old `storeLat!` threw here).
    final store = widget.tracking.storeLat != null &&
            widget.tracking.storeLng != null
        ? LatLng(widget.tracking.storeLat!, widget.tracking.storeLng!)
        : null;
    final destination =
        LatLng(widget.destinationLat!, widget.destinationLng!);
    final rider = _drawnPosition;
    final route = widget.tracking.routePolyline == null
        ? null
        : decodeGooglePolyline(widget.tracking.routePolyline!);

    final markers = <Marker>{
      if (store != null)
        Marker(
          markerId: const MarkerId('store'),
          position: store,
          infoWindow: InfoWindow(title: widget.tracking.storeName),
          icon: BitmapDescriptor.defaultMarker,
        ),
      Marker(
        markerId: const MarkerId('destination'),
        position: destination,
        infoWindow: const InfoWindow(title: 'Your address'),
        icon: BitmapDescriptor.defaultMarkerWithHue(BitmapDescriptor.hueGreen),
      ),
      if (rider != null)
        Marker(
          markerId: const MarkerId('rider'),
          position: rider,
          infoWindow: const InfoWindow(title: 'Your rider'),
          icon: BitmapDescriptor.defaultMarkerWithHue(
              BitmapDescriptor.hueOrange),
        ),
    };

    final polylines = <Polyline>{
      // The cached road route — real Google geometry or nothing at all.
      if (route != null && route.length >= 2)
        Polyline(
          polylineId: const PolylineId('route'),
          points: route,
          color: AppColors.primary,
          width: 5,
        ),
      // The trail the rider actually rode behind them.
      if (widget.tracking.trail.length >= 2)
        Polyline(
          polylineId: const PolylineId('trail'),
          points: widget.tracking.trail
              .map((point) => LatLng(point.lat, point.lng))
              .toList(),
          color: AppColors.primary.withValues(alpha: 0.35),
          width: 3,
        ),
    };

    return ClipRRect(
      borderRadius: BorderRadius.circular(16),
      child: Stack(
        children: [
          GoogleMap(
            initialCameraPosition: CameraPosition(
              target: _drawnPosition ?? destination,
              zoom: 14,
            ),
            markers: markers,
            polylines: polylines,
            onMapCreated: (controller) {
              _controller = controller;
              _fitBounds(store: store, destination: destination);
            },
            onCameraMoveStarted: () {
              // The customer's pan wins over the follow — with a way back.
              if (_programmaticMoves > 0) return;
              if (_followRider) setState(() => _followRider = false);
            },
            myLocationButtonEnabled: false,
            zoomControlsEnabled: true,
            mapToolbarEnabled: false,
            compassEnabled: false,
          ),
          if (!_followRider)
            Positioned(
              right: 12,
              bottom: 12,
              child: FloatingActionButton.small(
                heroTag: 'recenter-tracking',
                backgroundColor: AppColors.surface,
                foregroundColor: AppColors.primary,
                onPressed: () {
                  setState(() => _followRider = true);
                  final tracking = widget.tracking;
                  if (tracking.hasRiderPosition) {
                    _followIfEnabled(
                        LatLng(tracking.lastLat!, tracking.lastLng!));
                  }
                },
                child: const Icon(Icons.my_location_rounded),
              ),
            ),
        ],
      ),
    );
  }

  void _fitBounds({required LatLng? store, required LatLng destination}) {
    final rider = _drawnPosition;
    final targets = [
      ?store,
      destination,
      ?rider,
    ];
    if (targets.length == 1) {
      _controller?.moveCamera(CameraUpdate.newLatLngZoom(destination, 14));
      return;
    }
    final south = targets.map((t) => t.latitude).reduce(math.min);
    final north = targets.map((t) => t.latitude).reduce(math.max);
    final west = targets.map((t) => t.longitude).reduce(math.min);
    final east = targets.map((t) => t.longitude).reduce(math.max);
    _controller?.moveCamera(
      CameraUpdate.newLatLngBounds(
        LatLngBounds(
          southwest: LatLng(south, west),
          northeast: LatLng(north, east),
        ),
        32,
      ),
    );
  }
}

/// Linear interpolation between two real points — a moto at steady speed,
/// not an easing.
class _LatLngTween extends Tween<LatLng> {
  _LatLngTween({required super.begin, required super.end});

  @override
  LatLng lerp(double t) => LatLng(
        begin!.latitude + (end!.latitude - begin!.latitude) * t,
        begin!.longitude + (end!.longitude - begin!.longitude) * t,
      );
}

/// Desktop dev honesty (tracking doc §4): no fake map — the same real
/// facts, as text.
class _DesktopMapPlaceholder extends StatelessWidget {
  const _DesktopMapPlaceholder({required this.tracking});

  final DeliveryTracking tracking;

  @override
  Widget build(BuildContext context) {
    return Container(
      height: 180,
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: AppColors.surface,
        borderRadius: BorderRadius.circular(16),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          const Icon(Icons.motorcycle_rounded, color: AppColors.primary),
          const SizedBox(height: 8),
          Text(
            tracking.hasRiderPosition
                ? 'Rider at (${tracking.lastLat!.toStringAsFixed(4)}, ${tracking.lastLng!.toStringAsFixed(4)}) — the map renders on your phone.'
                : 'Out for delivery — the map renders on your phone.',
            style: Theme.of(context).textTheme.bodySmall?.copyWith(
                  color: AppColors.onSurfaceMuted,
                ),
          ),
        ],
      ),
    );
  }
}

/// Maps render on Android/iOS only — Linux desktop (the founder's dev
/// surface) takes the honest data placeholder.
bool get isMobilePlatform =>
    defaultTargetPlatform == TargetPlatform.android ||
    defaultTargetPlatform == TargetPlatform.iOS;

/// A compact Google encoded-polyline decoder (the route travels the wire
/// encoded; the map wants points). The algorithm is fully specified —
/// mirrors the server's commerce-side decoder.
List<LatLng> decodeGooglePolyline(String encoded) {
  final bytes = encoded.codeUnits;
  final points = <LatLng>[];
  var index = 0;
  var lat = 0;
  var lng = 0;

  int? component() {
    var result = 0;
    var shift = 0;
    while (true) {
      if (index >= bytes.length) return null;
      final byte = bytes[index] - 63;
      index += 1;
      result |= (byte & 0x1f) << shift;
      shift += 5;
      if (byte & 0x20 == 0) break;
    }
    return result & 1 == 1 ? ~(result >> 1) : (result >> 1);
  }

  while (index < bytes.length) {
    final dLat = component();
    final dLng = component();
    if (dLat == null || dLng == null) break;
    lat += dLat;
    lng += dLng;
    points.add(LatLng(lat / 1e5, lng / 1e5));
  }
  return points;
}
