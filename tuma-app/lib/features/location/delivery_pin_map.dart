import 'package:flutter/material.dart';

import 'package:tuma_app/core/theme/app_colors.dart';
import 'package:tuma_app/features/location/customer_location.dart';

/// Kigali center — where the checkout map sits when nothing is known yet.
const kigaliCenter = CustomerLocation(lat: -1.9449, lng: 30.0619);

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
