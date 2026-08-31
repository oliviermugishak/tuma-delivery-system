import 'package:flutter/material.dart';
import 'package:google_maps_flutter/google_maps_flutter.dart';

import 'package:tuma_app/core/theme/app_colors.dart';
import 'package:tuma_app/features/location/customer_location.dart';
import 'package:tuma_app/features/tracking/delivery_map.dart'
    show isMobilePlatform;

/// Kigali center — where the checkout map sits when nothing is known yet.
const kigaliCenter = CustomerLocation(lat: -1.9449, lng: 30.0619);

/// The checkout's delivery-pin picker: a real Google map the customer taps
/// to drop the pin, with a "Use my location" shortcut for a GPS fix. The
/// chosen pin flows into the checkout request AND the persisted customer
/// location — so Home's distances sharpen after the first pin.
///
/// Linux desktop has no Google Maps target, so dev drops pins via
/// paste-in coordinates (a Google Maps share-link) — the tracking doc's
/// dev caveat, which is genuinely useful on phones too.
class DeliveryPinMap extends StatefulWidget {
  const DeliveryPinMap({
    super.key,
    required this.pin,
    required this.onPin,
    required this.locate,
  });

  /// The current pin — rendered as the marker; also seeds the map center.
  final CustomerLocation? pin;

  /// Called when the customer taps the map, accepts a GPS fix, or commits
  /// a pasted coordinate.
  final ValueChanged<CustomerLocation> onPin;

  /// The GPS call behind "Use my location", injected (usually
  /// [acquireLocationProvider]) so tests never touch platform channels.
  final Future<CustomerLocation> Function() locate;

  @override
  State<DeliveryPinMap> createState() => _DeliveryPinMapState();
}

class _DeliveryPinMapState extends State<DeliveryPinMap> {
  GoogleMapController? _mapController;
  bool _locating = false;

  Future<void> _useMyLocation() async {
    if (_locating) return;
    setState(() => _locating = true);
    try {
      final fix = await widget.locate();
      widget.onPin(fix);
      await _mapController?.animateCamera(
        CameraUpdate.newLatLngZoom(LatLng(fix.lat, fix.lng), 16),
      );
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
    // Linux desktop dev: no Google Maps target — paste-in coordinates
    // carry pin-dropping instead of a map.
    if (!isMobilePlatform) {
      return PasteCoordinatesField(
        pin: widget.pin,
        onPin: widget.onPin,
      );
    }
    final center = widget.pin ?? kigaliCenter;
    return ClipRRect(
      borderRadius: BorderRadius.circular(12),
      child: Stack(
        children: [
          GoogleMap(
            initialCameraPosition: CameraPosition(
              target: LatLng(center.lat, center.lng),
              zoom: 15,
            ),
            onMapCreated: (controller) => _mapController = controller,
            onTap: (latLng) => widget.onPin(
              CustomerLocation(lat: latLng.latitude, lng: latLng.longitude),
            ),
            markers: {
              if (widget.pin != null)
                Marker(
                  markerId: const MarkerId('delivery-pin'),
                  position: LatLng(widget.pin!.lat, widget.pin!.lng),
                  icon: BitmapDescriptor.defaultMarkerWithHue(
                      BitmapDescriptor.hueOrange),
                ),
            },
            myLocationButtonEnabled: false,
            zoomControlsEnabled: false,
            mapToolbarEnabled: false,
            compassEnabled: false,
          ),
          Positioned(
            right: 10,
            top: 10,
            child:
                _LocateButton(locating: _locating, onPressed: _useMyLocation),
          ),
        ],
      ),
    );
  }
}

/// The desktop dev pin flow: paste a Google Maps share-link or a raw
/// "lat, lng" pair, preview the parse, commit the pin. A pure parser
/// ([parsePastedCoordinates]) does the recognizing — the field only
/// presents it.
class PasteCoordinatesField extends StatefulWidget {
  const PasteCoordinatesField({
    super.key,
    required this.onPin,
    this.pin,
  });

  final ValueChanged<CustomerLocation> onPin;
  final CustomerLocation? pin;

  @override
  State<PasteCoordinatesField> createState() => _PasteCoordinatesFieldState();
}

class _PasteCoordinatesFieldState extends State<PasteCoordinatesField> {
  final _controller = TextEditingController();
  CustomerLocation? _parsed;

  @override
  void initState() {
    super.initState();
    _controller.text = widget.pin == null
        ? ''
        : '${widget.pin!.lat.toStringAsFixed(6)}, ${widget.pin!.lng.toStringAsFixed(6)}';
    _parsed = widget.pin;
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  void _reparse(String text) {
    setState(() => _parsed = parsePastedCoordinates(text));
  }

  @override
  Widget build(BuildContext context) {
    final parsed = _parsed;
    final textTheme = Theme.of(context).textTheme;
    return Container(
      padding: const EdgeInsets.all(14),
      decoration: BoxDecoration(
        color: AppColors.surfaceAlt,
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: AppColors.surfaceBorder),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text('Drop your delivery pin', style: textTheme.titleSmall?.copyWith(
                fontWeight: FontWeight.w700,
              )),
          const SizedBox(height: 4),
          Text(
            'Paste a Google Maps share-link or "lat, lng" — maps render on your phone.',
            style: textTheme.bodySmall?.copyWith(
              color: AppColors.onSurfaceMuted,
            ),
          ),
          const SizedBox(height: 10),
          TextField(
            controller: _controller,
            onChanged: _reparse,
            decoration: InputDecoration(
              hintText: 'Paste a Google Maps link or lat, lng',
              isDense: true,
              suffixIcon: parsed == null
                  ? null
                  : const Icon(Icons.check_circle_rounded,
                      color: AppColors.success, size: 20),
            ),
          ),
          const SizedBox(height: 10),
          SizedBox(
            width: double.infinity,
            child: FilledButton.icon(
              onPressed:
                  parsed == null ? null : () => widget.onPin(parsed),
              style: FilledButton.styleFrom(
                minimumSize: const Size.fromHeight(44),
              ),
              icon: const Icon(Icons.location_on_rounded, size: 18),
              label: Text(
                parsed == null
                    ? 'Use this pin'
                    : 'Use (${parsed.lat.toStringAsFixed(4)}, ${parsed.lng.toStringAsFixed(4)})',
                style: const TextStyle(fontWeight: FontWeight.w700),
              ),
            ),
          ),
        ],
      ),
    );
  }
}

/// Recognizes a pasted location: Google Maps share-links (the precise
/// `!3dlat!4dlng` place marker wins over the `@lat,lng` viewport center),
/// `?q=lat,lng` search links, or a raw `lat, lng` pair. `null` when
/// nothing honest is in there — coordinates out of range are rejected.
CustomerLocation? parsePastedCoordinates(String input) {
  final text = input.trim();
  if (text.isEmpty) return null;

  CustomerLocation? loc(String? lat, String? lng) {
    final latValue = double.tryParse(lat ?? '');
    final lngValue = double.tryParse(lng ?? '');
    if (latValue == null || lngValue == null) return null;
    if (latValue < -90 || latValue > 90 || lngValue < -180 || lngValue > 180) {
      return null;
    }
    return CustomerLocation(lat: latValue, lng: lngValue);
  }

  final number = r'([-+]?\d+(?:\.\d+)?)';
  final precise = RegExp('!3d$number!4d$number').firstMatch(text);
  if (precise != null) return loc(precise.group(1), precise.group(2));
  final query = RegExp(r'[?&]q=' '$number,\\s*' '$number').firstMatch(text);
  if (query != null) return loc(query.group(1), query.group(2));
  final at = RegExp('@$number,\\s*$number').firstMatch(text);
  if (at != null) return loc(at.group(1), at.group(2));
  final pair = RegExp('$number\\s*,\\s*$number').firstMatch(text);
  if (pair != null) return loc(pair.group(1), pair.group(2));
  return null;
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
