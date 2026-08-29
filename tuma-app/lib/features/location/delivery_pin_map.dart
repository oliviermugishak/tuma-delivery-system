import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:latlong2/latlong.dart';

import 'package:tuma_app/core/theme/app_colors.dart';
import 'package:tuma_app/features/location/customer_location.dart';

/// Kigali center — where the checkout map sits when nothing is known yet.
const kigaliCenter = CustomerLocation(lat: -1.9449, lng: 30.0619);

/// The checkout's delivery-pin picker: a real OpenStreetMap the customer
/// taps to drop the pin, with a "Use my location" shortcut for a GPS fix.
/// The chosen pin flows into the checkout request AND the persisted
/// customer location — so Home's distances sharpen after the first pin.
///
/// Tiles are OSM standard (fine for dev; a commercial provider swaps in
/// behind this one widget later, per the blueprint's boundary).
class DeliveryPinMap extends StatefulWidget {
  const DeliveryPinMap({
    super.key,
    required this.pin,
    required this.onPin,
    required this.locate,
  });

  /// The current pin — rendered as the marker; also seeds the map center.
  final CustomerLocation? pin;

  /// Called when the customer taps the map or accepts a GPS fix.
  final ValueChanged<CustomerLocation> onPin;

  /// The GPS call behind "Use my location", injected (usually
  /// [acquireLocationProvider]) so tests never touch platform channels.
  final Future<CustomerLocation> Function() locate;

  @override
  State<DeliveryPinMap> createState() => _DeliveryPinMapState();
}

class _DeliveryPinMapState extends State<DeliveryPinMap> {
  final _mapController = MapController();
  bool _locating = false;

  Future<void> _useMyLocation() async {
    if (_locating) return;
    setState(() => _locating = true);
    try {
      final fix = await widget.locate();
      widget.onPin(fix);
      _mapController.move(LatLng(fix.lat, fix.lng), 16);
    } on Object {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Could not get your location.')),
        );
      }
    } finally {
      if (mounted) setState(() => _locating = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final center = widget.pin ?? kigaliCenter;
    return ClipRRect(
      borderRadius: BorderRadius.circular(12),
      child: Stack(
        children: [
          FlutterMap(
            mapController: _mapController,
            options: MapOptions(
              initialCenter: LatLng(center.lat, center.lng),
              initialZoom: 15,
              interactionOptions: const InteractionOptions(
                flags: InteractiveFlag.drag | InteractiveFlag.pinchZoom,
              ),
              onTap: (_, latLng) => widget.onPin(
                CustomerLocation(lat: latLng.latitude, lng: latLng.longitude),
              ),
            ),
            children: [
              TileLayer(
                urlTemplate: 'https://tile.openstreetmap.org/{z}/{x}/{y}.png',
                userAgentPackageName: 'rw.tuma.app',
              ),
              if (widget.pin != null)
                MarkerLayer(
                  markers: [
                    Marker(
                      point: LatLng(widget.pin!.lat, widget.pin!.lng),
                      width: 40,
                      height: 40,
                      child: const Icon(
                        Icons.location_on_rounded,
                        size: 40,
                        color: AppColors.primary,
                      ),
                    ),
                  ],
                ),
              const RichAttributionWidget(
                attributions: [
                  TextSourceAttribution('© OpenStreetMap contributors'),
                ],
              ),
            ],
          ),
          Positioned(
            right: 10,
            top: 10,
            child: _LocateButton(locating: _locating, onPressed: _useMyLocation),
          ),
        ],
      ),
    );
  }
}

class _LocateButton extends StatelessWidget {
  const _LocateButton({required this.locating, required this.onPressed});

  final bool locating;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    return Material(
      color: AppColors.surfaceAlt,
      borderRadius: BorderRadius.circular(10),
      child: InkWell(
        onTap: onPressed,
        borderRadius: BorderRadius.circular(10),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              locating
                  ? const SizedBox(
                      width: 14,
                      height: 14,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Icon(
                      Icons.my_location_rounded,
                      size: 14,
                      color: AppColors.primary,
                    ),
              const SizedBox(width: 6),
              Text(
                'Use my location',
                style: Theme.of(context).textTheme.labelMedium?.copyWith(
                      fontWeight: FontWeight.w600,
                    ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
