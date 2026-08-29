import 'package:flutter/material.dart';

import 'package:tuma_app/core/theme/app_colors.dart';
import 'package:tuma_app/core/utils/format_rwf.dart';

/// The delivery-fee chip: a moto with the price on solid gold, or the
/// word "Free" on solid teal when delivery costs nothing. It sits on
/// card pictures as well as beside store headers, so it's solid and
/// lightly shadowed to stay readable on any photo. Formats only — the
/// fee itself is server truth.
class FeeChip extends StatelessWidget {
  const FeeChip({super.key, required this.fee});

  final int fee;

  @override
  Widget build(BuildContext context) {
    final free = fee == 0;
    final color = free ? AppColors.success : AppColors.primary;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
      decoration: BoxDecoration(
        color: color,
        borderRadius: BorderRadius.circular(999),
        boxShadow: [
          BoxShadow(
            color: Colors.black.withValues(alpha: 0.25),
            blurRadius: 8,
            offset: const Offset(0, 2),
          ),
        ],
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(
            Icons.motorcycle_rounded,
            size: 14,
            color: AppColors.onPrimary,
          ),
          const SizedBox(width: 5),
          Text(
            free ? 'Free' : formatRwf(fee),
            style: Theme.of(context).textTheme.labelSmall?.copyWith(
                  color: AppColors.onPrimary,
                  fontWeight: FontWeight.w700,
                ),
          ),
        ],
      ),
    );
  }
}
