import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import 'package:tuma_app/core/api/api_client.dart';
import 'package:tuma_app/core/api/models/store.dart';
import 'package:tuma_app/core/auth/auth_controller.dart';
import 'package:tuma_app/core/theme/app_colors.dart';
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
      final stores = await ref.read(storeApiProvider).listStores();
      if (!mounted) return;
      setState(() => _stores = stores);
    } on ApiError catch (error) {
      if (!mounted) return;
      // A refresh failure with a list on screen keeps the list; the error
      // state is for first loads with nothing to show.
      if (_stores == null) setState(() => _error = error.message);
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
                user?.name != null && user!.name!.isNotEmpty
                    ? 'Hi, ${user.name} 👋'
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
/// allows — one on a phone, up to four on a wide desktop window.
({int columns, double cardWidth}) _cardMetrics(double width) {
  var columns = (width / 300).floor();
  if (columns < 1) columns = 1;
  if (columns > 4) columns = 4;
  final cardWidth = (width - _cardSpacing * (columns - 1)) / columns;
  return (columns: columns, cardWidth: cardWidth);
}

/// One open store: a picture across the top, then name, fee and address
/// below — the card shape commerce apps settled on.
class _StoreCard extends StatelessWidget {
  const _StoreCard({required this.store, required this.onTap});

  final Store store;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final address = store.addressText;
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
                padding: const EdgeInsets.fromLTRB(16, 14, 16, 16),
                // Fixed height so cards line up in the grid whether or
                // not the store has an address.
                child: SizedBox(
                  height: 50,
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Row(
                        children: [
                          Expanded(
                            child: Text(
                              store.name,
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: textTheme.titleMedium?.copyWith(
                                fontWeight: FontWeight.w700,
                              ),
                            ),
                          ),
                          const SizedBox(width: 12),
                          FeeChip(fee: store.deliveryFee),
                        ],
                      ),
                      if (address != null && address.isNotEmpty) ...[
                        const SizedBox(height: 6),
                        Row(
                          children: [
                            const Icon(
                              Icons.place_rounded,
                              size: 14,
                              color: AppColors.onSurfaceMuted,
                            ),
                            const SizedBox(width: 4),
                            Expanded(
                              child: Text(
                                address,
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                style: textTheme.bodySmall?.copyWith(
                                  color: AppColors.onSurfaceMuted,
                                ),
                              ),
                            ),
                          ],
                        ),
                      ],
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
                            padding: const EdgeInsets.fromLTRB(16, 14, 16, 16),
                            child: SizedBox(
                              height: 50,
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
                                  Container(
                                    width: 85,
                                    height: 10,
                                    decoration: BoxDecoration(
                                      color: block,
                                      borderRadius: BorderRadius.circular(6),
                                    ),
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
