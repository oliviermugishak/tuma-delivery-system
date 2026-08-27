import 'package:flutter/material.dart';

/// Path to the brand mark used in the splash and chrome.
///
/// Sourced from `assets/brand/` and registered in `pubspec.yaml`.
const String brandMarkPath = 'assets/brand/android-chrome-192x192.png';

/// Provider of the brand mark image, cached for the lifetime of the app.
class BrandMark extends StatelessWidget {
  const BrandMark({super.key, this.size = 128, this.borderRadius = 24});

  final double size;
  final double borderRadius;

  @override
  Widget build(BuildContext context) {
    return ClipRRect(
      borderRadius: BorderRadius.circular(borderRadius),
      child: Image.asset(
        brandMarkPath,
        width: size,
        height: size,
        fit: BoxFit.contain,
      ),
    );
  }
}
