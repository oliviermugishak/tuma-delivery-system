import 'dart:math' as math;

import 'package:flutter/material.dart';

import 'package:tuma_app/core/theme/app_colors.dart';

/// The image pipeline for stores and products: renders [url] when the
/// server has one — an uploaded banner or gallery cover, served from the
/// storage base with immutable caching — otherwise an honest icon block.
///
/// There is deliberately NO stock-photo fallback: a hashed photo of some
/// other food on a store that never uploaded anything is a lie. When
/// nothing real exists (or the fetch fails), the customer sees exactly
/// that — a quiet block with [fallbackIcon].
class RemoteImage extends StatelessWidget {
  const RemoteImage({
    super.key,
    required this.seed,
    this.url,
    this.width,
    this.height,
    this.borderRadius = 16,
    this.fit = BoxFit.cover,
    this.fallbackIcon = Icons.shopping_basket_rounded,
  });

  final String? url;

  /// Kept for call-site symmetry (cards pass the name); no longer drives
  /// any placeholder — imagery is either real or an icon.
  final String seed;
  final double? width;
  final double? height;
  final double borderRadius;
  final BoxFit fit;

  /// The honest stand-in: what this thing is, in one glyph.
  final IconData fallbackIcon;

  @override
  Widget build(BuildContext context) {
    if (url == null) {
      return _buildIconBlock();
    }
    final image = Image.network(
      url!,
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
      errorBuilder: (context, error, stackTrace) => _buildIconBlock(),
    );
    return ClipRRect(
      borderRadius: BorderRadius.circular(borderRadius),
      child: width == null && height == null
          ? SizedBox.expand(child: image)
          : image,
    );
  }

  /// The honest fallback: a quiet block and the thing's glyph — the same
  /// recipe as the closed-store state, never a stock photo.
  Widget _buildIconBlock() {
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
          decoration: BoxDecoration(
            color: AppColors.onSurface.withValues(alpha: 0.06),
            border: Border.all(color: AppColors.surfaceBorder),
          ),
          child: Icon(
            fallbackIcon,
            color: AppColors.onSurfaceMuted,
            size: side * 0.32,
          ),
        );
      },
    );
  }
}
