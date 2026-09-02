import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import 'package:tuma_app/core/api/api_client.dart';
import 'package:tuma_app/core/api/models/discovery.dart';
import 'package:tuma_app/core/api/models/product.dart';
import 'package:tuma_app/core/auth/auth_controller.dart';
import 'package:tuma_app/core/theme/app_colors.dart';
import 'package:tuma_app/core/theme/app_theme.dart';
import 'package:tuma_app/core/utils/format_rwf.dart';
import 'package:tuma_app/features/cart/cart_notifier.dart';
import 'package:tuma_app/shared/widgets/design_system.dart';
import 'package:tuma_app/shared/widgets/remote_image.dart';

/// The cart — the redesign's screens 12/13: one store card per bucket
/// (with "+ Add items" back to the store), tinted line thumbs, the
/// quantity stepper whose minus becomes a DELETE icon exactly at qty 1
/// (P10), ONE summary block (P1), the honest "Prices are confirmed at
/// checkout" line, and the CTA carrying the total (P8). The empty state
/// is a designed dead end: message + Browse stores + popular re-entry
/// items (P12, P17).
class CartView extends ConsumerWidget {
  const CartView({super.key, this.onBack});

  /// Optional back navigation — passed by the full-screen `/cart` route.
  final VoidCallback? onBack;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final cart = ref.watch(cartProvider);
    final state = cart.maybeWhen(
      data: (v) => v,
      orElse: () => const CartState(),
    );

    if (cart.isLoading) {
      return const Center(child: CircularProgressIndicator());
    }
    if (cart.hasError) {
      return Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(
              'Couldn\'t load your cart. Try again.',
              style: AppTheme.bd(context.textTheme).copyWith(
                color: AppColors.onSurfaceMuted,
              ),
            ),
            const SizedBox(height: 16),
            FilledButton(
              onPressed: () => ref.refresh(cartProvider),
              child: const Text('Retry'),
            ),
          ],
        ),
      );
    }
    if (state.isEmpty) {
      return CartEmptyState(onBack: onBack);
    }

    final textTheme = Theme.of(context).textTheme;

    return Column(
      children: [
        Expanded(
          child: ListView(
            padding: const EdgeInsets.fromLTRB(16, 8, 16, 16),
            children: [
              Row(
                children: [
                  if (onBack != null)
                    IconButton(
                      onPressed: onBack,
                      icon: const Icon(Icons.arrow_back_rounded, size: 22),
                    ),
                  if (onBack == null) ...[
                    const SizedBox(
                        width: 40,
                        child: Icon(
                          Icons.shopping_cart_rounded,
                          size: 22,
                          color: AppColors.onSurfaceMuted,
                        )),
                  ],
                  const SizedBox(width: 8),
                  Text('Your cart', style: AppTheme.d1(textTheme).copyWith(fontSize: 21)),
                ],
              ),
              const SizedBox(height: 14),
              // One card per store bucket: who fulfills it + "+ Add items"
              // (P17: dead space becomes a next step), then its lines.
              for (final bucket in state.buckets) ...[
                _StoreHeaderCard(bucket: bucket),
                const SizedBox(height: 10),
                ...[
                  for (final item in bucket.items)
                    Padding(
                      padding: const EdgeInsets.only(bottom: 10),
                      child: _CartItemCard(
                        bucket: bucket,
                        item: item,
                      ),
                    ),
                ],
              ],
              const SizedBox(height: 2),
              // ONE summary block (P1).
              _SummaryCard(
                lines: [
                  (label: 'Subtotal', value: state.subtotal),
                  (
                    label: state.buckets.length > 1
                        ? 'Delivery fee'
                        : 'Delivery fee',
                    value: state.deliveryTotal,
                  ),
                ],
                total: state.total,
              ),
              const SizedBox(height: 10),
              Text(
                'Prices are confirmed at checkout.',
                textAlign: TextAlign.center,
                style: AppTheme.sub(textTheme),
              ),
            ],
          ),
        ),
        // Bottom-pinned CTA carrying the total (P7, P8).
        SafeArea(
          top: false,
          child: Padding(
            padding: const EdgeInsets.fromLTRB(16, 10, 16, 16),
            child: FilledButton(
              onPressed: () => context.push('/checkout'),
              child: Text(
                'Checkout · ${formatRwf(state.total)}',
                style: const TextStyle(
                  fontSize: 15,
                  fontWeight: FontWeight.w600,
                ),
              ),
            ),
          ),
        ),
      ],
    );
  }
}

extension on BuildContext {
  TextTheme get textTheme => Theme.of(this).textTheme;
}

/// The store card: tinted tile + name + the "+ Add items" text action
/// back into that store (P17).
class _StoreHeaderCard extends StatelessWidget {
  const _StoreHeaderCard({required this.bucket});

  final StoreBucket bucket;

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final tint = CategoryTint.forCategory(bucket.storeName);
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 12),
      decoration: BoxDecoration(
        color: AppColors.surfaceAlt,
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: AppColors.surfaceBorder),
      ),
      child: Row(
        children: [
          TintedTile(tint: tint, size: 40, iconSize: 18),
          const SizedBox(width: 12),
          Expanded(
            child: Text(
              bucket.storeName,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: textTheme.titleSmall?.copyWith(fontSize: 15),
            ),
          ),
          TextButton(
            onPressed: () => context.push('/stores/${bucket.storeId}'),
            child: const Text('+ Add items'),
          ),
        ],
      ),
    );
  }
}

/// Rebuild a [MenuItem] so the notifier can step the line up.
MenuItem _menuLine(StoreBucket bucket, CartItem item) => MenuItem(
      id: item.storeProductId,
      name: item.name,
      price: item.unitPrice,
      isAvailable: true,
      imageUrl: item.imageUrl,
    );

class _CartItemCard extends ConsumerWidget {
  const _CartItemCard({required this.bucket, required this.item});

  final StoreBucket bucket;
  final CartItem item;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final textTheme = Theme.of(context).textTheme;
    final tint = CategoryTint.forCategory(item.name);
    return Container(
      padding: const EdgeInsets.all(13),
      decoration: BoxDecoration(
        color: AppColors.surfaceAlt,
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: AppColors.surfaceBorder),
      ),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Stack(
            children: [
              TintedTile(tint: tint, size: 52, iconSize: 22),
              if (item.imageUrl != null)
                RemoteImage(
                  url: item.imageUrl,
                  seed: item.name,
                  width: 52,
                  height: 52,
                  borderRadius: 12,
                  memCacheSize: 120,
                  fallbackIcon: Icons.lunch_dining_rounded,
                ),
            ],
          ),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  item.name,
                  maxLines: 2,
                  overflow: TextOverflow.ellipsis,
                  style: textTheme.titleSmall?.copyWith(fontSize: 14.5),
                ),
                const SizedBox(height: 2),
                Text(
                  '${formatRwf(item.unitPrice)} each',
                  style: AppTheme.sub(textTheme),
                ),
              ],
            ),
          ),
          Column(
            crossAxisAlignment: CrossAxisAlignment.end,
            children: [
              Text(
                formatRwf(item.lineTotal),
                style: textTheme.titleSmall?.copyWith(
                  fontSize: 14,
                  color: AppColors.primary,
                  fontWeight: FontWeight.w700,
                ),
              ),
              const SizedBox(height: 7),
              Row(
                children: [
                  // P10: minus becomes DELETE exactly at qty 1 — the
                  // removal affordance exists when it's needed.
                  _StepperButton(
                    icon: item.quantity == 1
                        ? Icons.delete_outline_rounded
                        : Icons.remove_rounded,
                    onTap: () => unawaited(
                      ref
                          .read(cartProvider.notifier)
                          .remove(item.storeProductId),
                    ),
                  ),
                  Padding(
                    padding: const EdgeInsets.symmetric(horizontal: 10),
                    child: Text(
                      '${item.quantity}',
                      style: textTheme.titleSmall?.copyWith(
                        fontWeight: FontWeight.w700,
                      ),
                    ),
                  ),
                  _StepperButton(
                    icon: Icons.add_rounded,
                    onTap: () => unawaited(
                      ref.read(cartProvider.notifier).add(
                            _menuLine(bucket, item),
                            storeId: bucket.storeId,
                            storeName: bucket.storeName,
                            deliveryFee: bucket.deliveryFee,
                          ),
                    ),
                  ),
                ],
              ),
            ],
          ),
        ],
      ),
    );
  }
}

class _StepperButton extends StatelessWidget {
  const _StepperButton({required this.icon, required this.onTap});

  final IconData icon;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return Material(
      color: AppColors.surfaceHigh,
      shape: const CircleBorder(),
      child: InkWell(
        customBorder: const CircleBorder(),
        onTap: onTap,
        child: SizedBox(
          width: 28,
          height: 28,
          child: Icon(icon, size: 15, color: AppColors.onSurface),
        ),
      ),
    );
  }
}

/// The one summary block (P1): lines + hairline + accent total.
class _SummaryCard extends StatelessWidget {
  const _SummaryCard({required this.lines, required this.total});

  final List<({String label, int value})> lines;
  final int total;

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    return Container(
      padding: const EdgeInsets.all(14),
      decoration: BoxDecoration(
        color: AppColors.surfaceAlt,
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: AppColors.surfaceBorder),
      ),
      child: Column(
        children: [
          for (final line in lines)
            Padding(
              padding: const EdgeInsets.only(bottom: 5),
              child: Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  Text(line.label, style: AppTheme.bd(textTheme).copyWith(
                    color: AppColors.onSurfaceMuted,
                  )),
                  Text(formatRwf(line.value), style: AppTheme.bd(textTheme)),
                ],
              ),
            ),
          const Divider(color: AppColors.surfaceBorder),
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              Text('Total',
                  style: AppTheme.bd(textTheme).copyWith(fontWeight: FontWeight.w600)),
              Text(
                formatRwf(total),
                style: textTheme.titleSmall?.copyWith(
                  fontSize: 15,
                  color: AppColors.primary,
                  fontWeight: FontWeight.w700,
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

/// Honest empty state (screen 13): message + Browse stores + two popular
/// items so recovery is one tap (P12, P17).
class CartEmptyState extends ConsumerStatefulWidget {
  const CartEmptyState({super.key, this.onBack});

  final VoidCallback? onBack;

  @override
  ConsumerState<CartEmptyState> createState() => _CartEmptyStateState();
}

class _CartEmptyStateState extends ConsumerState<CartEmptyState> {
  List<ProductHit>? _popular;
  bool _failed = false;

  @override
  void initState() {
    super.initState();
    unawaited(_loadPopular());
  }

  Future<void> _loadPopular() async {
    try {
      final result = await ref.read(storeApiProvider).search();
      if (!mounted) return;
      setState(() => _popular = result.products.take(2).toList());
    } on ApiError {
      if (!mounted) return;
      // The empty state stands alone without the shelf (P2: never fake).
      setState(() => _failed = true);
    }
  }

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final popular = _popular;
    return ListView(
      padding: const EdgeInsets.fromLTRB(16, 8, 16, 24),
      children: [
        Row(
          children: [
            if (widget.onBack != null)
              IconButton(
                onPressed: widget.onBack,
                icon: const Icon(Icons.arrow_back_rounded, size: 22),
              )
            else
              const SizedBox(width: 40),
            const SizedBox(width: 8),
            Text('Your cart',
                style: AppTheme.d1(textTheme).copyWith(fontSize: 21)),
          ],
        ),
        SizedBox(
          height: MediaQuery.of(context).size.height * 0.42,
          child: Center(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                Container(
                  width: 96,
                  height: 96,
                  decoration: const BoxDecoration(
                    color: AppColors.surfaceHigh,
                    shape: BoxShape.circle,
                  ),
                  child: const Icon(
                    Icons.shopping_cart_rounded,
                    size: 40,
                    color: AppColors.onSurfaceMuted,
                  ),
                ),
                const SizedBox(height: 18),
                Text('Your cart is empty',
                    style: AppTheme.hd(textTheme).copyWith(fontSize: 16)),
                const SizedBox(height: 4),
                Text('Browse stores and add something tasty.',
                    style: AppTheme.sub(textTheme)),
                const SizedBox(height: 20),
                FilledButton(
                  onPressed: widget.onBack ?? () => context.go('/home'),
                  style: FilledButton.styleFrom(
                    minimumSize: const Size(0, 50),
                    padding: const EdgeInsets.symmetric(horizontal: 28),
                  ),
                  child: const Text('Browse stores'),
                ),
              ],
            ),
          ),
        ),
        // Popular re-entry: only real orders light this shelf (P13).
        if (popular != null && popular.isNotEmpty) ...[
          Text('Popular near you', style: AppTheme.sec(textTheme)),
          const SizedBox(height: 10),
          Row(
            children: [
              for (var i = 0; i < popular.length; i++) ...[
                if (i > 0) const SizedBox(width: 12),
                Expanded(
                  child: _PopularMiniCard(hit: popular[i]),
                ),
              ],
            ],
          ),
        ] else if (_failed) ...[
          Text('Popular near you', style: AppTheme.sec(textTheme)),
          const SizedBox(height: 10),
          Text('Orders will light this shelf up.',
              style: AppTheme.sub(textTheme)),
        ],
      ],
    );
  }
}

/// The empty cart's mini product card: tinted thumb, name, price, and a
/// 30dp add button that drops it straight into the cart.
class _PopularMiniCard extends ConsumerWidget {
  const _PopularMiniCard({required this.hit});

  final ProductHit hit;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final textTheme = Theme.of(context).textTheme;
    final tint = CategoryTint.forCategory(hit.storeName, hint: hit.name);
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: AppColors.surfaceAlt,
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: AppColors.surfaceBorder),
      ),
      child: Row(
        children: [
          Stack(
            children: [
              TintedTile(tint: tint, size: 44, iconSize: 18),
              if (hit.imageUrl != null)
                RemoteImage(
                  url: hit.imageUrl,
                  seed: hit.name,
                  width: 44,
                  height: 44,
                  borderRadius: 12,
                  memCacheSize: 120,
                  fallbackIcon: Icons.lunch_dining_rounded,
                ),
            ],
          ),
          const SizedBox(width: 10),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  hit.name,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: textTheme.titleSmall?.copyWith(fontSize: 13),
                ),
                const SizedBox(height: 2),
                Text(
                  formatRwf(hit.price),
                  style: textTheme.titleSmall?.copyWith(
                    fontSize: 13,
                    color: AppColors.primary,
                    fontWeight: FontWeight.w700,
                  ),
                ),
              ],
            ),
          ),
          Material(
            color: AppColors.primary,
            shape: const CircleBorder(),
            child: InkWell(
              customBorder: const CircleBorder(),
              onTap: () => unawaited(
                ref.read(cartProvider.notifier).addFromHit(hit),
              ),
              child: const SizedBox(
                width: 30,
                height: 30,
                child: Icon(Icons.add_rounded, size: 16, color: AppColors.onPrimary),
              ),
            ),
          ),
        ],
      ),
    );
  }
}

/// Thin route shell for the store screen's cart entry: full screen with a
/// back button (the bottom-nav Cart tab renders [CartView] directly).
class CartScreen extends StatelessWidget {
  const CartScreen({super.key});

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: AppColors.surface,
      body: SafeArea(
        child: CartView(
          // A cold start on /cart has nothing to pop — fall home instead
          // of throwing (review P16).
          onBack: () {
            if (context.canPop()) {
              context.pop();
            } else {
              context.go('/home');
            }
          },
        ),
      ),
    );
  }
}
