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
      body: Stack(
        children: [
          if (_notFound)
            SafeArea(child: _ClosedState(onBack: () => context.pop()))
          else if (detail != null)
            _content(context, detail)
          else
            _loadingOrError(),
          // The back button floats over the banner and stays reachable
          // while scrolling — the pattern delivery apps established.
          SafeArea(
            child: Padding(
              padding: const EdgeInsets.all(16),
              child: Align(
                alignment: Alignment.topLeft,
                child: _BackButton(onTap: () => context.pop()),
              ),
            ),
          ),
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
              // The star badge joins this row when ratings exist — no
              // fake stars before then.
              Row(
                children: [FeeChip(fee: store.deliveryFee)],
              ),
              if (address != null && address.isNotEmpty) ...[
                const SizedBox(height: 12),
                Row(
                  children: [
                    const Icon(
                      Icons.place_rounded,
                      size: 16,
                      color: AppColors.onSurfaceMuted,
                    ),
                    const SizedBox(width: 6),
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
                    onAdd: _announceCart,
                  ),
                  if (i < products.length - 1)
                    Container(height: 1, color: AppColors.surfaceBorder),
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

  /// The cart doesn't exist yet — ordering is build order #3. The button
  /// is present and honest instead of dead; the wiring lands with the cart.
  void _announceCart() {
    final textTheme = Theme.of(context).textTheme;
    ScaffoldMessenger.of(context)
      ..hideCurrentSnackBar()
      ..showSnackBar(
        SnackBar(
          content: Text(
            'Cart is coming soon — ordering is next.',
            style: textTheme.bodyMedium?.copyWith(
              color: AppColors.onSurface,
            ),
          ),
          behavior: SnackBarBehavior.floating,
          duration: const Duration(seconds: 2),
          backgroundColor: AppColors.surfaceAlt,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(12),
            side: const BorderSide(color: AppColors.surfaceBorder),
          ),
        ),
      );
  }

  /// Product tap → bottom sheet: picture, name, price, full description.
  /// Nothing else — quantity and checkout arrive with the orders iteration.
  void _showProduct(BuildContext context, Product product) {
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

class _ProductRow extends StatelessWidget {
  const _ProductRow({
    required this.product,
    required this.onTap,
    required this.onAdd,
  });

  final Product product;
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
          padding: const EdgeInsets.symmetric(vertical: 12),
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
              Container(
                width: 110,
                height: 26,
                decoration: BoxDecoration(
                  color: block,
                  borderRadius: BorderRadius.circular(999),
                ),
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
                  padding: const EdgeInsets.symmetric(vertical: 12),
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
