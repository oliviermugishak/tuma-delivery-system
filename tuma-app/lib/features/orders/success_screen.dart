import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';

import 'package:tuma_app/core/theme/app_colors.dart';
import 'package:tuma_app/core/theme/app_theme.dart';
import 'package:tuma_app/core/utils/format_rwf.dart';

/// The checkout's success screen — the redesign's screen 05: celebration,
/// the order's facts (number · store), the ETA + cash card, and a
/// one-tap path to tracking (P12, P17). One screen, one job: convert
/// anxiety into orientation. Routed as `/success` with the placed
/// group's id as an extra.
class SuccessScreen extends StatelessWidget {
  const SuccessScreen({
    super.key,
    required this.groupId,
    required this.orderNumber,
    required this.storeName,
    required this.total,
    this.etaMinutes,
  });

  final String groupId;
  final int orderNumber;
  final String storeName;
  final int total;
  final int? etaMinutes;

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    return Scaffold(
      body: SafeArea(
        child: Padding(
          padding: const EdgeInsets.fromLTRB(16, 0, 16, 24),
          child: Column(
            children: [
              const Spacer(),
              Container(
                width: 96,
                height: 96,
                decoration: BoxDecoration(
                  color: AppColors.success.withValues(alpha: 0.14),
                  shape: BoxShape.circle,
                ),
                child: const Icon(
                  Icons.check_circle_rounded,
                  size: 52,
                  color: AppColors.success,
                ),
              ),
              const SizedBox(height: 22),
              Text('Order placed!', style: AppTheme.d2(textTheme)),
              const SizedBox(height: 6),
              Text(
                '#$orderNumber · $storeName',
                style: AppTheme.bd(textTheme)
                    .copyWith(color: AppColors.onSurfaceMuted),
              ),
              const Spacer(),
              Container(
                padding:
                    const EdgeInsets.symmetric(horizontal: 14, vertical: 14),
                margin: const EdgeInsets.symmetric(horizontal: 8),
                decoration: BoxDecoration(
                  color: AppColors.surfaceAlt,
                  borderRadius: BorderRadius.circular(16),
                  border: Border.all(color: AppColors.surfaceBorder),
                ),
                child: Row(
                  children: [
                    const Icon(Icons.schedule_rounded,
                        size: 20, color: AppColors.success),
                    const SizedBox(width: 10),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            // P2: an ETA the server didn't give renders as
                            // nothing — never a guessed "~99 min".
                            etaMinutes != null
                                ? 'Arriving in about $etaMinutes min'
                                : 'The store is preparing your order.',
                            style: AppTheme.bd(textTheme)
                                .copyWith(fontWeight: FontWeight.w600),
                          ),
                          const SizedBox(height: 2),
                          Text(
                            'Pay ${formatRwf(total)} cash on delivery',
                            style: AppTheme.sub(textTheme),
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
              ),
              const SizedBox(height: 20),
              Padding(
                padding: const EdgeInsets.symmetric(horizontal: 8),
                child: FilledButton(
                  onPressed: () => context.push('/orders/$groupId'),
                  child: const Text(
                    'Track order',
                    style: TextStyle(
                      fontSize: 15,
                      fontWeight: FontWeight.w600,
                    ),
                  ),
                ),
              ),
              const SizedBox(height: 14),
              TextButton(
                onPressed: () => context.go('/home'),
                style: TextButton.styleFrom(
                  foregroundColor: AppColors.onSurfaceMuted,
                ),
                child: const Text('Back to home'),
              ),
            ],
          ),
        ),
      ),
    );
  }
}


/// The success route's payload.
class SuccessScreenArgs {
  const SuccessScreenArgs({
    required this.groupId,
    required this.orderNumber,
    required this.storeName,
    required this.total,
    this.etaMinutes,
  });

  final String groupId;
  final int orderNumber;
  final String storeName;
  final int total;
  final int? etaMinutes;
}
