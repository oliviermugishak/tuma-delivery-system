import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';

import 'package:tuma_app/core/theme/app_colors.dart';

/// App router.
///
/// Only the root exists today. Real routes land slice by slice
/// (A1 navigation shell first) — each designed and approved before code.
final GoRouter appRouter = GoRouter(
  initialLocation: '/',
  routes: [
    GoRoute(
      path: '/',
      builder: (context, state) => const _RootPlaceholder(),
    ),
  ],
);

/// Temporary shell screen proving the app boots with the Tuma identity.
/// Replaced by the real splash + navigation shell in slice A1.
class _RootPlaceholder extends StatelessWidget {
  const _RootPlaceholder();

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    return Scaffold(
      backgroundColor: AppColors.primary,
      body: Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Container(
              width: 112,
              height: 112,
              decoration: const BoxDecoration(
                color: AppColors.onPrimary,
                shape: BoxShape.circle,
              ),
              child: const Icon(
                Icons.delivery_dining_rounded,
                size: 64,
                color: AppColors.primary,
              ),
            ),
            const SizedBox(height: 24),
            Text(
              'Tuma',
              style: textTheme.displaySmall?.copyWith(
                color: AppColors.onPrimary,
                fontWeight: FontWeight.w800,
              ),
            ),
            const SizedBox(height: 8),
            Text(
              'Everything you crave, delivered.',
              style: textTheme.bodyMedium?.copyWith(
                color: AppColors.onPrimary.withValues(alpha: 0.85),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
