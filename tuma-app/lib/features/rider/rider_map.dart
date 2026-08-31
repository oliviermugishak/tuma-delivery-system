import 'package:flutter/material.dart';
import 'package:google_maps_flutter/google_maps_flutter.dart';

import 'package:tuma_app/core/api/models/rider.dart';
import 'package:tuma_app/core/theme/app_colors.dart';
import 'package:tuma_app/features/tracking/delivery_map.dart'
    show decodeGooglePolyline, isMobilePlatform;

/// The rider's own map — destination, cached road route, store, and the
/// rider's real device position (client-side truth from the push loop,
/// never re-fetched from the server). Same honesty rules as the
/// customer's map: no destination pin → no destination marker + the
/// honest note; desktop dev → the data placeholder.
class RiderMap extends StatefulWidget {
  const RiderMap({
    super.key,
    required this.delivery,
    this.riderLat,
    this.riderLng,
  });

  final RiderDelivery delivery;
  final double? riderLat;
  final double? riderLng;

  @override
  State<RiderMap> createState() => _RiderMapState();
}

class _RiderMapState extends State<RiderMap> {
  GoogleMapController? _controller;
  bool _followMe = true;

  /// Programmatic camera moves also raise onCameraMoveStarted; counting
  /// them lets the rider's own pan still win the follow state.
  int _programmaticMoves = 0;

  bool get _hasDestination =>
      widget.delivery.destinationLat != null &&
      widget.delivery.destinationLng != null;

  @override
  Widget build(BuildContext context) {
    if (!isMobilePlatform) {
      return _RiderMapPlaceholder(delivery: widget.delivery);
    }

    final store = widget.delivery.storeLat != null &&
            widget.delivery.storeLng != null
        ? LatLng(widget.delivery.storeLat!, widget.delivery.storeLng!)
        : null;
    final destination = _hasDestination
        ? LatLng(
            widget.delivery.destinationLat!, widget.delivery.destinationLng!)
        : null;
    final me = widget.riderLat != null && widget.riderLng != null
        ? LatLng(widget.riderLat!, widget.riderLng!)
        : null;
    final route = widget.delivery.routePolyline == null
        ? null
        : decodeGooglePolyline(widget.delivery.routePolyline!);

    final markers = <Marker>{
      if (destination != null)
        Marker(
          markerId: const MarkerId('destination'),
          position: destination,
          infoWindow: InfoWindow(title: widget.delivery.destinationAddress),
          icon:
              BitmapDescriptor.defaultMarkerWithHue(BitmapDescriptor.hueGreen),
        ),
      if (store != null)
        Marker(
          markerId: const MarkerId('store'),
          position: store,
          infoWindow: InfoWindow(title: widget.delivery.storeName),
          icon:
              BitmapDescriptor.defaultMarkerWithHue(BitmapDescriptor.hueAzure),
        ),
      if (me != null)
        Marker(
          markerId: const MarkerId('me'),
          position: me,
          infoWindow: const InfoWindow(title: 'You'),
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
    };

    return ClipRRect(
      borderRadius: BorderRadius.circular(16),
      child: Stack(
        children: [
          GoogleMap(
            initialCameraPosition: CameraPosition(
              target: me ?? destination ?? store ?? const LatLng(-1.9449, 30.0619),
              zoom: 14,
            ),
            markers: markers,
            polylines: polylines,
            onMapCreated: (controller) {
              _controller = controller;
              if (me != null) {
                _moveTo(me);
              } else if (destination != null) {
                _moveTo(destination);
              }
            },
            onCameraMoveStarted: () {
              if (_programmaticMoves > 0) return;
              if (_followMe) setState(() => _followMe = false);
            },
            myLocationButtonEnabled: false,
            zoomControlsEnabled: false,
            mapToolbarEnabled: false,
            compassEnabled: false,
          ),
          if (!_followMe && me != null)
            Positioned(
              right: 12,
              bottom: 12,
              child: FloatingActionButton.small(
                heroTag: 'recenter-rider',
                backgroundColor: AppColors.surfaceAlt,
                foregroundColor: AppColors.primary,
                onPressed: () {
                  setState(() => _followMe = true);
                  _moveTo(me);
                },
                child: const Icon(Icons.my_location_rounded),
              ),
            ),
        ],
      ),
    );
  }

  void _moveTo(LatLng target) {
    final controller = _controller;
    if (controller == null) return;
    _programmaticMoves += 1;
    controller
        .animateCamera(CameraUpdate.newLatLngZoom(target, 15))
        .whenComplete(() {
      _programmaticMoves = (_programmaticMoves - 1).clamp(0, 1 << 31);
    });
  }
}

/// Desktop dev honesty: no fake map — the run's real facts, as text.
class _RiderMapPlaceholder extends StatelessWidget {
  const _RiderMapPlaceholder({required this.delivery});

  final RiderDelivery delivery;

  @override
  Widget build(BuildContext context) {
    return Container(
      height: 160,
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: AppColors.surfaceAlt,
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: AppColors.surfaceBorder),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          const Icon(Icons.map_rounded, color: AppColors.primaryDeep),
          const SizedBox(height: 8),
          Text(
            delivery.destinationLat != null
                ? 'Destination at (${delivery.destinationLat!.toStringAsFixed(4)}, ${delivery.destinationLng!.toStringAsFixed(4)}) — the map renders on your phone.'
                : 'No delivery pin on this order — the address text is all the customer gave. The map renders on your phone.',
            style: Theme.of(context).textTheme.bodySmall?.copyWith(
                  color: AppColors.onSurfaceMuted,
                ),
          ),
        ],
      ),
    );
  }
}
