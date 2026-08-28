import 'dart:math' as math;

import 'package:flutter/material.dart';

import 'package:tuma_app/core/constants/placeholder_images.dart';
import 'package:tuma_app/core/theme/app_colors.dart';

/// The image pipeline for stores and products, built before uploading
/// exists: renders [url] when the server has one, otherwise a
/// deterministic Unsplash placeholder picked from [seed]. When real
/// uploading lands, `image_url` values flow through this same widget —
/// no UI changes.
///
/// Fallback chain: real URL → placeholder URL → quiet gradient block, so
/// a dead network never shows a broken-image icon.
class RemoteImage extends StatelessWidget {
  const RemoteImage({
    super.key,
    required this.seed,
    this.url,
    this.width,
    this.height,
    this.borderRadius = 16,
    this.fit = BoxFit.cover,
  });

  final String? url;
  final String seed;
  final double? width;
  final double? height;
  final double borderRadius;
  final BoxFit fit;

  @override
  Widget build(BuildContext context) {
    final image = Image.network(
      url ?? placeholderImageFor(seed),
      width: width,
      height: height,
      fit: fit,
      loadingBuilder: (context, child, progress) {
        if (progress == null) return child;
        return Container(
          width: width,
          height: height,
          color: AppColors.onSurface.withValues(alpha: 0.06),
        );
      },
      errorBuilder: (context, error, stackTrace) => _fallbackBlock(),
    );
    return ClipRRect(
      borderRadius: BorderRadius.circular(borderRadius),
      child: width == null && height == null
          ? SizedBox.expand(child: image)
          : image,
    );
  }

  Widget _fallbackBlock() {
    return LayoutBuilder(
      builder: (context, constraints) {
        final side = math.min(
          constraints.maxWidth.isFinite ? constraints.maxWidth : (width ?? 56),
          constraints.maxHeight.isFinite
              ? constraints.maxHeight
              : (height ?? 56),
        );
        return Container(
          width: width,
          height: height,
          decoration: const BoxDecoration(
            gradient: LinearGradient(
              begin: Alignment.topLeft,
              end: Alignment.bottomRight,
              colors: [AppColors.primary, AppColors.primaryDeep],
            ),
          ),
          child: Icon(
            Icons.restaurant,
            color: AppColors.onPrimary,
            size: side * 0.32,
          ),
        );
      },
    );
  }
}
