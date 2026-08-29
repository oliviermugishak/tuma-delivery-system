import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import 'package:tuma_app/core/api/api_client.dart';
import 'package:tuma_app/core/api/models/store.dart';
import 'package:tuma_app/core/auth/auth_controller.dart';
import 'package:tuma_app/core/theme/app_colors.dart';
import 'package:tuma_app/features/location/customer_location.dart';
import 'package:tuma_app/shared/widgets/error_state.dart';
import 'package:tuma_app/shared/widgets/fee_chip.dart';
import 'package:tuma_app/shared/widgets/remote_image.dart';

/// Home tab: greeting, then sections — "Stores near you" first; more
/// sections slot in below it as they earn their place.
class HomeScreen extends ConsumerStatefulWidget {
  const HomeScreen({super.key});

  @override
  ConsumerState<HomeScreen> createState() => _HomeScreenState();
}

class _HomeScreenState extends ConsumerState<HomeScreen> {
  List<Store>? _stores;
  String? _error;

  @override
  void initState() {
    super.initState();
    unawaited(_load());
  }

  Future<void> _load() async {
    setState(() => _error = null);
    try {
      final located = await _locate();
      final stores = await ref
          .read(storeApiProvider)
          .listStores(lat: located?.lat, lng: located?.lng);
      if (!mounted) return;
      setState(() => _stores = stores);
    } on ApiError catch (error) {
      if (!mounted) return;
      // A refresh failure with a list on screen keeps the list; the error
      // state is for first loads with nothing to show.
      if (_stores == null) setState(() => _error = error.message);
    }
  }

  /// Locate once per load, the founder-chosen launch flow: a fresh GPS
  /// fix when the device can (the OS prompt on first run), else the
  /// persisted pin, else null — the bare feed with no distances. GPS is
  /// best-effort by design: it must never block or break the feed.
  Future<CustomerLocation?> _locate() async {
    try {
      final acquire = ref.read(acquireLocationProvider);
      final fix = await acquire();
      await ref.read(customerLocationProvider.notifier).setPin(fix);
      return fix;
    } on Object {
      // No GPS (desktop dev), services off, permission denied — fall
      // back to the pin the checkout map persisted, if there is one.
      return await ref.read(customerLocationProvider.future);
    }
  }

  @override
  Widget build(BuildContext context) {
    final session = ref.watch(sessionProvider).asData?.value;
    final user = switch (session) {
      SessionUser s => s.user,
      _ => null,
    };
    final textTheme = Theme.of(context).textTheme;
    final stores = _stores;

    return Scaffold(
      body: SafeArea(
        child: RefreshIndicator(
          onRefresh: _load,
          color: AppColors.primary,
          backgroundColor: AppColors.surfaceAlt,
          child: ListView(
            physics: const AlwaysScrollableScrollPhysics(),
            padding: const EdgeInsets.fromLTRB(24, 24, 24, 16),
            children: [
              Text(
                user?.displayName != null
                    ? 'Hi, ${user!.displayName} 👋'
                    : 'Welcome to Tuma 👋',
                style: textTheme.headlineSmall?.copyWith(
                  color: AppColors.onSurface,
                  fontWeight: FontWeight.w800,
                ),
              ),
              const SizedBox(height: 24),
              if (stores == null && _error != null)
                ErrorState(message: _error!, onRetry: _load)
              else if (stores == null)
                const _FeedSkeleton()
              else if (stores.isEmpty)
                const _EmptyState()
              else ...[
                Text(
                  'Stores near you',
                  style: textTheme.titleLarge?.copyWith(
                    fontWeight: FontWeight.w700,
                  ),
                ),
                const SizedBox(height: 14),
                LayoutBuilder(
                  builder: (context, constraints) {
                    final metrics = _cardMetrics(constraints.maxWidth);
                    return Wrap(
                      spacing: _cardSpacing,
                      runSpacing: _cardSpacing,
                      children: [
                        for (final store in stores)
                          SizedBox(
                            width: metrics.cardWidth,
                            child: _StoreCard(
                              store: store,
                              onTap: () => unawaited(
                                context.push('/stores/${store.id}'),
                              ),
                            ),
                          ),
                      ],
                    );
                  },
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}

const double _cardSpacing = 16;

/// The store grid is flex-like: as many ~300px columns as the width
/// allows — normally at least two so customers see more, up to four on
/// a wide desktop window. Narrow screens are the backup: under ~400px
/// of width two cards would be narrower than the ETA + fee-chip line
/// needs, so the grid falls back to one full-width column there.
({int columns, double cardWidth}) _cardMetrics(double width) {
  if (width < 400) {
    return (columns: 1, cardWidth: width);
  }
  var columns = (width / 300).floor();
  if (columns < 2) columns = 2;
  if (columns > 4) columns = 4;
  final cardWidth = (width - _cardSpacing * (columns - 1)) / columns;
  return (columns: columns, cardWidth: cardWidth);
}

/// One open store: a picture across the top — its top-right corner
/// stays clear for the merchant star rating later — then the name, the
/// gray category with the distance on its line, and the ETA with the
/// delivery-fee badge on the far right. Every fact is server-owned:
/// category hides when the merchant set none, and distance/ETA only
/// render when the request carried the customer's location.
class _StoreCard extends StatelessWidget {
  const _StoreCard({required this.store, required this.onTap});

  final Store store;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final category = (store.category != null && store.category!.isNotEmpty)
        ? store.category
        : null;
    return Container(
      decoration: BoxDecoration(
        color: AppColors.surfaceAlt,
        borderRadius: BorderRadius.circular(20),
        border: Border.all(color: AppColors.surfaceBorder),
      ),
      // The picture fills the card's top edge to edge; the card's own
      // rounding clips its corners.
      clipBehavior: Clip.antiAlias,
      child: Material(
        color: Colors.transparent,
        child: InkWell(
          onTap: onTap,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              AspectRatio(
                aspectRatio: 16 / 9,
                child: RemoteImage(
                  url: store.imageUrl,
                  seed: store.name,
                  borderRadius: 0,
                ),
              ),
              Padding(
                padding: const EdgeInsets.fromLTRB(16, 12, 16, 14),
                // Fixed height so cards line up in the grid.
                child: SizedBox(
                  height: 84,
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        store.name,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: textTheme.titleMedium?.copyWith(
                          fontWeight: FontWeight.w700,
                        ),
                      ),
                      const SizedBox(height: 4),
                      if (category != null || store.distanceM != null)
                        Row(
                          children: [
                            if (category != null)
                              Expanded(
                                child: Text(
                                  category,
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                  style: textTheme.labelMedium?.copyWith(
                                    color: AppColors.onSurfaceMuted,
                                    fontWeight: FontWeight.w600,
                                  ),
                                ),
                              )
                            else
                              const Spacer(),
                            if (store.distanceM != null) ...[
                              const SizedBox(width: 8),
                              const Icon(
                                Icons.route_rounded,
                                size: 13,
                                color: AppColors.onSurfaceMuted,
                              ),
                              const SizedBox(width: 4),
                              Text(
                                '${(store.distanceM! / 1000).toStringAsFixed(1)} km',
                                style: textTheme.bodySmall?.copyWith(
                                  color: AppColors.onSurfaceMuted,
                                ),
                              ),
                            ],
                          ],
                        ),
                      const SizedBox(height: 8),
                      Row(
                        children: [
                          if (store.etaMin != null) ...[
                            const Icon(
                              Icons.schedule_rounded,
                              size: 13,
                              color: AppColors.onSurfaceMuted,
                            ),
                            const SizedBox(width: 4),
                            Text(
                              '~${store.etaMin} min',
                              style: textTheme.bodySmall?.copyWith(
                                color: AppColors.onSurfaceMuted,
                              ),
                            ),
                          ],
                          const Spacer(),
                          FeeChip(fee: store.deliveryFee),
                        ],
                      ),
                    ],
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// Quiet card shapes in the same grid while the feed loads. Static by
/// design — no shimmer machinery until it earns its place.
class _FeedSkeleton extends StatelessWidget {
  const _FeedSkeleton();

  @override
  Widget build(BuildContext context) {
    final block = AppColors.onSurface.withValues(alpha: 0.07);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Container(
          width: 140,
          height: 14,
          decoration: BoxDecoration(
            color: block,
            borderRadius: BorderRadius.circular(6),
          ),
        ),
        const SizedBox(height: 14),
        LayoutBuilder(
          builder: (context, constraints) {
            final metrics = _cardMetrics(constraints.maxWidth);
            return Wrap(
              spacing: _cardSpacing,
              runSpacing: _cardSpacing,
              children: [
                for (var i = 0; i < 4; i++)
                  SizedBox(
                    width: metrics.cardWidth,
                    child: Container(
                      decoration: BoxDecoration(
                        color: AppColors.surfaceAlt,
                        borderRadius: BorderRadius.circular(20),
                        border: Border.all(color: AppColors.surfaceBorder),
                      ),
                      clipBehavior: Clip.antiAlias,
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          AspectRatio(
                            aspectRatio: 16 / 9,
                            child: ColoredBox(color: block),
                          ),
                          Padding(
                            padding: const EdgeInsets.fromLTRB(16, 12, 16, 14),
                            child: SizedBox(
                              height: 84,
                              child: Column(
                                crossAxisAlignment: CrossAxisAlignment.start,
                                children: [
                                  Container(
                                    width: 120,
                                    height: 12,
                                    decoration: BoxDecoration(
                                      color: block,
                                      borderRadius: BorderRadius.circular(6),
                                    ),
                                  ),
                                  const SizedBox(height: 10),
                                  Row(
                                    children: [
                                      Container(
                                        width: 56,
                                        height: 10,
                                        decoration: BoxDecoration(
                                          color: block,
                                          borderRadius: BorderRadius.circular(
                                            6,
                                          ),
                                        ),
                                      ),
                                      const Spacer(),
                                      Container(
                                        width: 44,
                                        height: 10,
                                        decoration: BoxDecoration(
                                          color: block,
                                          borderRadius: BorderRadius.circular(
                                            6,
                                          ),
                                        ),
                                      ),
                                    ],
                                  ),
                                  const SizedBox(height: 12),
                                  Row(
                                    children: [
                                      Container(
                                        width: 48,
                                        height: 10,
                                        decoration: BoxDecoration(
                                          color: block,
                                          borderRadius: BorderRadius.circular(
                                            6,
                                          ),
                                        ),
                                      ),
                                      const Spacer(),
                                      Container(
                                        width: 64,
                                        height: 26,
                                        decoration: BoxDecoration(
                                          color: block,
                                          borderRadius: BorderRadius.circular(
                                            999,
                                          ),
                                        ),
                                      ),
                                    ],
                                  ),
                                ],
                              ),
                            ),
                          ),
                        ],
                      ),
                    ),
                  ),
              ],
            );
          },
        ),
      ],
    );
  }
}

class _EmptyState extends StatelessWidget {
  const _EmptyState();

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 48),
      child: Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Container(
              width: 56,
              height: 56,
              decoration: BoxDecoration(
                color: AppColors.surfaceAlt,
                borderRadius: BorderRadius.circular(18),
                border: Border.all(color: AppColors.surfaceBorder),
              ),
              child: const Icon(
                Icons.storefront_rounded,
                color: AppColors.onSurfaceMuted,
              ),
            ),
            const SizedBox(height: 14),
            Text(
              'No stores are open right now.',
              style: textTheme.titleMedium?.copyWith(
                fontWeight: FontWeight.w700,
              ),
            ),
            const SizedBox(height: 4),
            Text(
              'Check back a little later.',
              style: textTheme.bodySmall?.copyWith(
                color: AppColors.onSurfaceMuted,
              ),
            ),
          ],
        ),
      ),
    );
  }
}
