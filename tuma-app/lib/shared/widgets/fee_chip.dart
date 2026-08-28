import 'package:flutter/material.dart';

import 'package:tuma_app/core/theme/app_colors.dart';
import 'package:tuma_app/core/utils/format_rwf.dart';

/// The delivery-fee chip: gold on tinted gold, "Free delivery" at zero.
/// Formats only — the fee itself is server truth.
class FeeChip extends StatelessWidget {
  const FeeChip({super.key, required this.fee});

  final int fee;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
      decoration: BoxDecoration(
        color: AppColors.primary.withValues(alpha: 0.1),
        borderRadius: BorderRadius.circular(999),
      ),
      child: Text(
        fee > 0 ? formatRwf(fee) : 'Free delivery',
        style: Theme.of(context).textTheme.labelSmall?.copyWith(
              color: AppColors.primary,
              fontWeight: FontWeight.w600,
            ),
      ),
    );
  }
}
