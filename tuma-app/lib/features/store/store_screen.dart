import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import 'package:tuma_app/core/api/api_client.dart';
import 'package:tuma_app/core/api/models/product.dart';
import 'package:tuma_app/core/api/models/store.dart';
import 'package:tuma_app/core/auth/auth_controller.dart';
import 'package:tuma_app/core/theme/app_colors.dart';
import 'package:tuma_app/core/utils/format_rwf.dart';
import 'package:tuma_app/features/cart/cart_notifier.dart';
import 'package:tuma_app/shared/widgets/error_state.dart';
import 'package:tuma_app/shared/widgets/fee_chip.dart';
import 'package:tuma_app/shared/widgets/remote_image.dart';

/// A store and its menu, the way customers see it: full-bleed banner, then
/// the identity block (name, description, badges, location), then the menu
/// — the delivery-app layout people already know. The server only serves
/// open stores with available products; a 404 means "closed or gone" and
/// gets a clean terminal state — never a retry loop.
class StoreScreen extends ConsumerStatefulWidget {
  const StoreScreen({super.key, required this.storeId});

  final String storeId;

  @override
  ConsumerState<StoreScreen> createState() => _StoreScreenState();
}

class _StoreScreenState extends ConsumerState<StoreScreen> {
  StoreDetail? _detail;
  String? _error;
  bool _notFound = false;

  @override
  void initState() {
    super.initState();
    unawaited(_load());
  }

  Future<void> _load() async {
    setState(() {
      _error = null;
      _notFound = false;
    });
    try {
      final detail =
          await ref.read(storeApiProvider).getStore(widget.storeId);
      if (!mounted) return;
      setState(() => _detail = detail);
    } on ApiNotFound {
      // Closed stores 404 by design.
      if (!mounted) return;
      setState(() => _notFound = true);
    } on ApiError catch (error) {
      if (!mounted) return;
      setState(() => _error = error.message);
    }
  }

  @override
  Widget build(BuildContext context) {
    final detail = _detail;
    return Scaffold(
      // Back and cart sit together in one transparent top bar over the
      // banner — the delivery-app pattern, with no floating orphans.
      appBar: AppBar(
        backgroundColor: Colors.transparent,
        elevation: 0,
        scrolledUnderElevation: 0,
        automaticallyImplyLeading: false,
        leading: Padding(
          padding: const EdgeInsets.only(left: 8),
          child: _BackButton(onTap: () => context.pop()),
        ),
        actions: const [
          _CartButton(),
          SizedBox(width: 12),
        ],
      ),
      body: Stack(
        children: [
          if (_notFound)
            SafeArea(child: _ClosedState(onBack: () => context.pop()))
          else if (detail != null)
            _content(context, detail)
          else
            _loadingOrError(),
        ],
      ),
    );
  }

  Widget _content(BuildContext context, StoreDetail detail) {
    final textTheme = Theme.of(context).textTheme;
    final store = detail.store;
    final products = detail.products;
    final address = store.addressText;
    final description = store.description;

    return ListView(
      padding: EdgeInsets.zero,
      children: [
        // Full-bleed banner, edge to edge — no corner cuts.
        RemoteImage(
          url: store.imageUrl,
          seed: store.name,
          height: 210,
          borderRadius: 0,
        ),
        Padding(
          padding: const EdgeInsets.fromLTRB(24, 20, 24, 24),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                store.name,
                style: textTheme.headlineSmall?.copyWith(
                  color: AppColors.onSurface,
                  fontWeight: FontWeight.w800,
                ),
              ),
              if (description != null && description.isNotEmpty) ...[
                const SizedBox(height: 10),
                Text(
                  description,
                  style: textTheme.bodyMedium?.copyWith(
                    color: AppColors.onSurfaceMuted,
                    height: 1.5,
                  ),
                ),
              ],
              const SizedBox(height: 14),
              // Badges sit together on one row. The star badge joins when
              // ratings exist — no fake stars before then.
              Row(
                children: [
                  FeeChip(fee: store.deliveryFee),
                  if (address != null && address.isNotEmpty) ...[
                    const SizedBox(width: 8),
                    Flexible(child: _LocationBadge(address: address)),
                  ],
                ],
              ),
              const SizedBox(height: 28),
              Text(
                'Menu',
                style: textTheme.titleMedium?.copyWith(
                  fontWeight: FontWeight.w700,
                ),
              ),
              const SizedBox(height: 8),
              if (products.isEmpty)
                Padding(
                  padding: const EdgeInsets.symmetric(vertical: 24),
                  child: Text(
                    'Nothing on the menu right now.',
                    style: textTheme.bodyMedium?.copyWith(
                      color: AppColors.onSurfaceMuted,
                    ),
                  ),
                )
              else
                for (var i = 0; i < products.length; i++) ...[
                  _ProductRow(
                    product: products[i],
                    onTap: () => _showProduct(context, products[i]),
                    onAdd: () => unawaited(
                      _addToCart(context, store, products[i]),
                    ),
                  ),
                  if (i < products.length - 1)
                    Container(
                      height: 1,
                      margin: const EdgeInsets.symmetric(horizontal: 12),
                      color: AppColors.surfaceBorder,
                    ),
                ],
            ],
          ),
        ),
      ],
    );
  }

  Widget _loadingOrError() {
    final error = _error;
    if (error != null) {
      return ListView(
        physics: const AlwaysScrollableScrollPhysics(),
        padding: const EdgeInsets.all(24),
        children: [ErrorState(message: error, onRetry: _load)],
      );
    }
    return const _StoreSkeleton();
  }

  /// Add the menu item to its store's cart bucket. Carts span stores now —
  /// nothing is ever cleared to make room; the snackbar confirms the fresh
  /// item count.
  Future<void> _addToCart(
    BuildContext context,
    Store store,
    MenuItem item,
  ) async {
    final messenger = ScaffoldMessenger.of(context);
    messenger.hideCurrentSnackBar();
    await ref.read(cartProvider.notifier).add(
          item,
          storeId: store.id,
          storeName: store.name,
          deliveryFee: store.deliveryFee,
        );
    if (!mounted) return;
    final count = ref.read(cartProvider).maybeWhen(
          data: (v) => v.itemCount,
          orElse: () => 0,
        );
    messenger.showSnackBar(
      SnackBar(
        // Explicit light text — the theme's snackbar default is dark text
        // (onInverseSurface), invisible on our navy background.
        content: Text(
          'Added to cart ($count item${count > 1 ? 's' : ''})',
          style: TextStyle(color: AppColors.onSurface),
        ),
        behavior: SnackBarBehavior.floating,
        duration: const Duration(seconds: 1),
        backgroundColor: AppColors.surfaceAlt,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(12),
          side: const BorderSide(color: AppColors.primary),
        ),
      ),
    );
  }

  /// Product tap → bottom sheet: picture, name, price, full description.
  void _showProduct(BuildContext context, MenuItem product) {
    final textTheme = Theme.of(context).textTheme;
    final description = product.description;
    showModalBottomSheet<void>(
      context: context,
      backgroundColor: AppColors.surfaceAlt,
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(top: Radius.circular(20)),
      ),
      builder: (sheetContext) => SafeArea(
        child: Padding(
          padding: const EdgeInsets.fromLTRB(24, 12, 24, 28),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Center(
                child: Container(
                  width: 36,
                  height: 4,
                  decoration: BoxDecoration(
                    color: AppColors.primary,
                    borderRadius: BorderRadius.circular(999),
                  ),
                ),
              ),
              const SizedBox(height: 20),
              ClipRRect(
                borderRadius: BorderRadius.circular(20),
                child: AspectRatio(
                  aspectRatio: 16 / 10,
                  child: RemoteImage(
                    url: product.imageUrl,
                    seed: product.name,
                  ),
                ),
              ),
              const SizedBox(height: 20),
              Text(
                product.name,
                style: textTheme.titleLarge?.copyWith(
                  fontWeight: FontWeight.w700,
                ),
              ),
              const SizedBox(height: 6),
              Text(
                formatRwf(product.price),
                style: textTheme.titleMedium?.copyWith(
                  color: AppColors.primary,
                  fontWeight: FontWeight.w700,
                ),
              ),
              if (description != null && description.isNotEmpty) ...[
                const SizedBox(height: 14),
                Text(
                  description,
                  style: textTheme.bodyMedium?.copyWith(
                    color: AppColors.onSurfaceMuted,
                    height: 1.5,
                  ),
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}

/// The store's address as a quiet pill beside the fee badge.
class _LocationBadge extends StatelessWidget {
  const _LocationBadge({required this.address});

  final String address;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
      decoration: BoxDecoration(
        color: AppColors.onSurface.withValues(alpha: 0.06),
        borderRadius: BorderRadius.circular(999),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          const Icon(
            Icons.place_rounded,
            size: 14,
            color: AppColors.onSurfaceMuted,
          ),
          const SizedBox(width: 5),
          Flexible(
            child: Text(
              address,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: Theme.of(context).textTheme.labelSmall?.copyWith(
                    color: AppColors.onSurfaceMuted,
                    fontWeight: FontWeight.w600,
                  ),
            ),
          ),
        ],
      ),
    );
  }
}

class _ProductRow extends StatelessWidget {
  const _ProductRow({
    required this.product,
    required this.onTap,
    required this.onAdd,
  });

  final MenuItem product;
  final VoidCallback onTap;
  final VoidCallback onAdd;

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final description = product.description;
    return Material(
      color: Colors.transparent,
      child: InkWell(
        borderRadius: BorderRadius.circular(14),
        onTap: onTap,
        child: Padding(
          // Horizontal breathing room so the hover/splash highlight
          // doesn't hug the content edge to edge.
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 12),
          child: Row(
            children: [
              RemoteImage(
                url: product.imageUrl,
                seed: product.name,
                width: 56,
                height: 56,
                borderRadius: 14,
              ),
              const SizedBox(width: 14),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      product.name,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: textTheme.bodyLarge?.copyWith(
                        fontWeight: FontWeight.w600,
                      ),
                    ),
                    if (description != null && description.isNotEmpty) ...[
                      const SizedBox(height: 2),
                      Text(
                        description,
                        maxLines: 2,
                        overflow: TextOverflow.ellipsis,
                        style: textTheme.bodySmall?.copyWith(
                          color: AppColors.onSurfaceMuted,
                        ),
                      ),
                    ],
                    const SizedBox(height: 8),
                    Text(
                      formatRwf(product.price),
                      style: textTheme.labelLarge?.copyWith(
                        color: AppColors.primary,
                        fontWeight: FontWeight.w700,
                      ),
                    ),
                  ],
                ),
              ),
              const SizedBox(width: 12),
              _AddToCartButton(onTap: onAdd),
            ],
          ),
        ),
      ),
    );
  }
}

/// Gold circle where the price used to sit — the row's invitation to add.
class _AddToCartButton extends StatelessWidget {
  const _AddToCartButton({required this.onTap});

  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return Material(
      color: AppColors.primary,
      shape: const CircleBorder(),
      child: InkWell(
        customBorder: const CircleBorder(),
        onTap: onTap,
        child: const SizedBox(
          width: 36,
          height: 36,
          child: Icon(
            Icons.add_shopping_cart_rounded,
            size: 17,
            color: AppColors.onPrimary,
          ),
        ),
      ),
    );
  }
}

/// Floats over the banner: a dark translucent circle that reads on any
/// picture.
class _BackButton extends StatelessWidget {
  const _BackButton({required this.onTap});

  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return Material(
      color: AppColors.surface.withValues(alpha: 0.55),
      shape: const CircleBorder(
        side: BorderSide(color: AppColors.surfaceBorder),
      ),
      child: InkWell(
        customBorder: const CircleBorder(),
        onTap: onTap,
        child: const SizedBox(
          width: 40,
          height: 40,
          child: Icon(
            Icons.arrow_back_rounded,
            size: 20,
            color: AppColors.onSurface,
          ),
        ),
      ),
    );
  }
}

/// Cart button in the store's top bar. Shows a badge with the item count
/// when the cart is non-empty; tapping navigates to /cart.
class _CartButton extends ConsumerWidget {
  const _CartButton();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final cart = ref.watch(cartProvider);
    final count = cart.maybeWhen(
      data: (v) => v.itemCount,
      orElse: () => 0,
    );
    return GestureDetector(
      onTap: () => context.push('/cart'),
      child: Padding(
        padding: const EdgeInsets.all(4),
        child: Stack(
          clipBehavior: Clip.none,
          children: [
            Container(
              width: 40,
              height: 40,
              alignment: Alignment.center,
              decoration: BoxDecoration(
                color: AppColors.surface.withValues(alpha: 0.55),
                shape: BoxShape.circle,
                border: Border.all(color: AppColors.surfaceBorder),
              ),
              child: Icon(
                Icons.shopping_cart_rounded,
                size: 20,
                color: AppColors.primary,
              ),
            ),
            if (count > 0)
              Positioned(
                right: -4,
                top: -4,
                child: Container(
                  constraints: const BoxConstraints(minWidth: 18, minHeight: 18),
                  decoration: const BoxDecoration(
                    color: AppColors.primary,
                    shape: BoxShape.circle,
                  ),
                  alignment: Alignment.center,
                  padding: const EdgeInsets.all(2),
                  child: Text(
                    count > 99 ? '99+' : '$count',
                    style: const TextStyle(
                      color: AppColors.onPrimary,
                      fontSize: 9,
                      fontWeight: FontWeight.w700,
                    ),
                    textAlign: TextAlign.center,
                  ),
                ),
              ),
          ],
        ),
      ),
    );
  }
}

/// A closed (or gone) store is a fact, not a failure: one clean state and
/// a way back.
class _ClosedState extends StatelessWidget {
  const _ClosedState({required this.onBack});

  final VoidCallback onBack;

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Container(
              width: 64,
              height: 64,
              decoration: BoxDecoration(
                color: AppColors.surfaceAlt,
                borderRadius: BorderRadius.circular(20),
                border: Border.all(color: AppColors.surfaceBorder),
              ),
              child: const Icon(
                Icons.storefront_rounded,
                size: 28,
                color: AppColors.onSurfaceMuted,
              ),
            ),
            const SizedBox(height: 16),
            Text(
              'This store isn\'t taking orders right now.',
              textAlign: TextAlign.center,
              style: textTheme.titleMedium?.copyWith(
                fontWeight: FontWeight.w700,
              ),
            ),
            const SizedBox(height: 6),
            Text(
              'It may have closed for the day — check back later.',
              textAlign: TextAlign.center,
              style: textTheme.bodySmall?.copyWith(
                color: AppColors.onSurfaceMuted,
              ),
            ),
            const SizedBox(height: 20),
            TextButton(
              onPressed: onBack,
              style: TextButton.styleFrom(foregroundColor: AppColors.primary),
              child: const Text('Go back'),
            ),
          ],
        ),
      ),
    );
  }
}

/// Banner block + identity lines + a few rows, quiet, while the store
/// loads. Mirrors the real layout so the reveal doesn't jump.
class _StoreSkeleton extends StatelessWidget {
  const _StoreSkeleton();

  @override
  Widget build(BuildContext context) {
    final block = AppColors.onSurface.withValues(alpha: 0.07);
    return ListView(
      padding: EdgeInsets.zero,
      children: [
        Container(
          height: 210,
          color: block,
        ),
        Padding(
          padding: const EdgeInsets.fromLTRB(24, 20, 24, 24),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Container(
                width: 180,
                height: 18,
                decoration: BoxDecoration(
                  color: block,
                  borderRadius: BorderRadius.circular(6),
                ),
              ),
              const SizedBox(height: 12),
              Container(
                width: 240,
                height: 12,
                decoration: BoxDecoration(
                  color: block,
                  borderRadius: BorderRadius.circular(6),
                ),
              ),
              const SizedBox(height: 16),
              Row(
                children: [
                  Container(
                    width: 96,
                    height: 26,
                    decoration: BoxDecoration(
                      color: block,
                      borderRadius: BorderRadius.circular(999),
                    ),
                  ),
                  const SizedBox(width: 8),
                  Container(
                    width: 120,
                    height: 26,
                    decoration: BoxDecoration(
                      color: block,
                      borderRadius: BorderRadius.circular(999),
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 28),
              Container(
                width: 70,
                height: 14,
                decoration: BoxDecoration(
                  color: block,
                  borderRadius: BorderRadius.circular(6),
                ),
              ),
              const SizedBox(height: 16),
              for (var i = 0; i < 4; i++)
                Padding(
                  padding:
                      const EdgeInsets.symmetric(horizontal: 12, vertical: 12),
                  child: Row(
                    children: [
                      Container(
                        width: 56,
                        height: 56,
                        decoration: BoxDecoration(
                          color: block,
                          borderRadius: BorderRadius.circular(14),
                        ),
                      ),
                      const SizedBox(width: 14),
                      Expanded(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Container(
                              width: 140,
                              height: 12,
                              decoration: BoxDecoration(
                                color: block,
                                borderRadius: BorderRadius.circular(6),
                              ),
                            ),
                            const SizedBox(height: 8),
                            Container(
                              width: 90,
                              height: 10,
                              decoration: BoxDecoration(
                                color: block,
                                borderRadius: BorderRadius.circular(6),
                              ),
                            ),
                          ],
                        ),
                      ),
                      const SizedBox(width: 12),
                      Container(
                        width: 36,
                        height: 36,
                        decoration: BoxDecoration(
                          color: block,
                          shape: BoxShape.circle,
                        ),
                      ),
                    ],
                  ),
                ),
            ],
          ),
        ),
      ],
    );
  }
}
