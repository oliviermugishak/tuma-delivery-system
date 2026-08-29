import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import 'package:tuma_app/core/api/models/product.dart';
import 'package:tuma_app/core/theme/app_colors.dart';
import 'package:tuma_app/core/utils/format_rwf.dart';
import 'package:tuma_app/features/cart/cart_notifier.dart';
import 'package:tuma_app/shared/widgets/remote_image.dart';

/// The cart UI, usable both as the bottom-nav Cart tab and as the full
/// `/cart` route (entered from the store screen). Lines are grouped under
/// their store's header — one bucket per store, because checkout splits
/// into one store order per store. Per-store subtotals and delivery fees,
/// then the grand total, and a sticky "Proceed to checkout" button. Empty
/// carts get an honest empty state.
///
/// State is watched live, so (un)checking items anywhere updates here.
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
              style: Theme.of(context).textTheme.titleMedium?.copyWith(
                    color: AppColors.onSurfaceMuted,
                  ),
              textAlign: TextAlign.center,
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
    final storeCount = state.storeCount;

    return Column(
      children: [
        Expanded(
          child: ListView(
            padding: const EdgeInsets.only(bottom: 16),
            children: [
              if (onBack != null)
                Padding(
                  padding: const EdgeInsets.fromLTRB(12, 8, 20, 8),
                  child: Row(
                    children: [
                      IconButton(
                        icon: const Icon(Icons.arrow_back_ios_new_rounded, size: 18),
                        onPressed: onBack,
                      ),
                      const SizedBox(width: 4),
                      Text(
                        'Your cart',
                        style: textTheme.titleLarge?.copyWith(
                          fontWeight: FontWeight.w700,
                        ),
                      ),
                    ],
                  ),
                ),
              // Multi-store hint: checkout will split this cart per store.
              if (storeCount > 1)
                Padding(
                  padding: const EdgeInsets.fromLTRB(20, 4, 20, 12),
                  child: Text(
                    '$storeCount stores are in this cart — they\'ll arrive as separate deliveries.',
                    style: textTheme.bodySmall?.copyWith(
                      color: AppColors.onSurfaceMuted,
                    ),
                  ),
                ),
              // One grouped section per store, in the order they were added.
              for (final bucket in state.buckets) ...[
                _StoreHeader(bucket: bucket),
                for (final item in bucket.items) ...[
                  _CartItemRow(
                    bucket: bucket,
                    item: item,
                  ),
                  const Divider(
                    height: 1,
                    indent: 20,
                    endIndent: 20,
                    color: AppColors.surfaceBorder,
                  ),
                ],
                Padding(
                  padding: const EdgeInsets.fromLTRB(20, 6, 20, 4),
                  child: Row(
                    mainAxisAlignment: MainAxisAlignment.spaceBetween,
                    children: [
                      Text(
                        'Store subtotal',
                        style: textTheme.labelSmall?.copyWith(
                          color: AppColors.onSurfaceMuted,
                        ),
                      ),
                      Text(
                        formatRwf(bucket.subtotal),
                        style: textTheme.labelMedium?.copyWith(
                          fontWeight: FontWeight.w700,
                        ),
                      ),
                    ],
                  ),
                ),
                const SizedBox(height: 12),
              ],
              // Grand totals.
              Padding(
                padding: const EdgeInsets.fromLTRB(20, 8, 20, 0),
                child: Column(
                  children: [
                    _TotalsRow(label: 'Subtotal', value: state.subtotal),
                    _TotalsRow(
                      label: storeCount > 1
                          ? 'Delivery (all stores)'
                          : 'Delivery fee',
                      value: state.deliveryTotal,
                    ),
                    const Divider(
                      height: 20,
                      color: AppColors.surfaceBorder,
                    ),
                    _TotalsRow(label: 'Total', value: state.total, bold: true),
                    const SizedBox(height: 4),
                    Text(
                      'Prices are confirmed at checkout.',
                      style: textTheme.labelSmall?.copyWith(
                        color: AppColors.onSurfaceMuted,
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
        // Sticky checkout CTA.
        SafeArea(
          top: false,
          child: Padding(
            padding: const EdgeInsets.fromLTRB(20, 10, 20, 12),
            child: FilledButton(
              onPressed: () => context.push('/checkout'),
              style: FilledButton.styleFrom(
                minimumSize: const Size.fromHeight(52),
              ),
              child: Text(
                storeCount > 1
                    ? 'Checkout all $storeCount stores'
                    : 'Proceed to checkout',
                style: const TextStyle(fontSize: 16, fontWeight: FontWeight.w700),
              ),
            ),
          ),
        ),
      ],
    );
  }
}

/// The store's header: who fulfills this bucket and what delivery costs.
class _StoreHeader extends StatelessWidget {
  const _StoreHeader({required this.bucket});

  final StoreBucket bucket;

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    return Padding(
      padding: const EdgeInsets.fromLTRB(20, 8, 20, 4),
      child: Row(
        children: [
          const Icon(Icons.storefront_rounded,
              size: 16, color: AppColors.primary),
          const SizedBox(width: 6),
          Expanded(
            child: Text(
              bucket.storeName,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: textTheme.titleSmall?.copyWith(
                fontWeight: FontWeight.w700,
              ),
            ),
          ),
          Text(
            'Delivery: ${formatRwf(bucket.deliveryFee)}',
            style: textTheme.bodySmall?.copyWith(
              color: AppColors.onSurfaceMuted,
            ),
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

class _CartItemRow extends ConsumerWidget {
  const _CartItemRow({
    required this.bucket,
    required this.item,
  });

  final StoreBucket bucket;
  final CartItem item;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final textTheme = Theme.of(context).textTheme;
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 10),
      child: Row(
        children: [
          ClipRRect(
            borderRadius: BorderRadius.circular(12),
            child: SizedBox(
              width: 56,
              height: 56,
              child: RemoteImage(
                url: item.imageUrl,
                seed: item.name,
                width: 56,
                height: 56,
                borderRadius: 12,
              ),
            ),
          ),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  item.name,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: textTheme.bodyMedium?.copyWith(
                    fontWeight: FontWeight.w600,
                  ),
                ),
                const SizedBox(height: 2),
                Text(
                  formatRwf(item.unitPrice),
                  style: textTheme.labelSmall?.copyWith(
                    color: AppColors.onSurfaceMuted,
                  ),
                ),
              ],
            ),
          ),
          Text(
            formatRwf(item.lineTotal),
            style: textTheme.bodyMedium?.copyWith(
              fontWeight: FontWeight.w700,
              color: AppColors.primary,
            ),
          ),
          const SizedBox(width: 12),
          // Quantity stepper.
          Row(
            children: [
              _StepperButton(
                icon: Icons.remove_rounded,
                onTap: () => unawaited(
                  ref.read(cartProvider.notifier).remove(item.storeProductId),
                ),
              ),
              Padding(
                padding: const EdgeInsets.symmetric(horizontal: 10),
                child: Text(
                  '${item.quantity}',
                  style: textTheme.bodyMedium?.copyWith(
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
      color: AppColors.surfaceBorder.withValues(alpha: 0.35),
      shape: const CircleBorder(),
      child: InkWell(
        customBorder: const CircleBorder(),
        onTap: onTap,
        child: SizedBox(
          width: 28,
          height: 28,
          child: Icon(icon, size: 16, color: AppColors.onSurface),
        ),
      ),
    );
  }
}

class _TotalsRow extends StatelessWidget {
  const _TotalsRow({
    required this.label,
    required this.value,
    this.bold = false,
  });

  final String label;
  final int value;
  final bool bold;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 4),
      child: Row(
        mainAxisAlignment: MainAxisAlignment.spaceBetween,
        children: [
          Text(
            label,
            style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                  fontWeight: bold ? FontWeight.w700 : FontWeight.w500,
                  color: bold ? AppColors.onSurface : AppColors.onSurfaceMuted,
                ),
          ),
          Text(
            formatRwf(value),
            style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                  fontWeight: bold ? FontWeight.w700 : FontWeight.w600,
                  color: bold ? AppColors.primary : AppColors.onSurface,
                ),
          ),
        ],
      ),
    );
  }
}

/// Honest empty state, with one way back to shopping.
class CartEmptyState extends StatelessWidget {
  const CartEmptyState({super.key, this.onBack});

  final VoidCallback? onBack;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Container(
            width: 56,
            height: 56,
            decoration: BoxDecoration(
              color: AppColors.primary.withValues(alpha: 0.1),
              borderRadius: BorderRadius.circular(16),
            ),
            child: const Icon(
              Icons.shopping_cart_outlined,
              size: 28,
              color: AppColors.primary,
            ),
          ),
          const SizedBox(height: 20),
          Text(
            'Your cart is empty',
            style: Theme.of(context).textTheme.titleMedium?.copyWith(
                  fontWeight: FontWeight.w700,
                ),
          ),
          const SizedBox(height: 8),
          Text(
            'Browse stores and add items to get started.',
            style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                  color: AppColors.onSurfaceMuted,
                ),
            textAlign: TextAlign.center,
          ),
          const SizedBox(height: 24),
          // Just-fit button — the global FilledButton style stretches full
          // width, which looks wrong for a secondary empty-state action.
          Center(
            child: FilledButton(
              onPressed: onBack ?? () => context.go('/home'),
              style: FilledButton.styleFrom(
                minimumSize: const Size(0, 52),
                padding: const EdgeInsets.symmetric(horizontal: 28),
              ),
              child: const Text('Browse stores'),
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
          onBack: () => context.pop(),
        ),
      ),
    );
  }
}
