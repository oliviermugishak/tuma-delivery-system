import 'dart:async';
import 'dart:ui' show PointerDeviceKind;

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

/// A store and its menu, the way customers see it: a fixed full-bleed
/// banner behind everything, then the identity block (name, description,
/// badges, location) and the menu on a rounded sheet that slides up over
/// the picture — scrolling moves only the sheet, never the image, with
/// ghost back/cart buttons on a top scrim. The server only serves open
/// stores with available products; a 404 means "closed or gone" and
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

  /// Way out of any state — even a cold start on this route with an
  /// empty navigation stack.
  void _goBack() {
    if (context.canPop()) {
      context.pop();
    } else {
      context.go('/home');
    }
  }

  @override
  Widget build(BuildContext context) {
    final detail = _detail;
    final topInset = MediaQuery.paddingOf(context).top;
    return Scaffold(
      // No AppBar: the backdrop, sheet, and floating buttons are one Stack.
      // extendBodyBehindAppBar keeps the banner clear of the system bar.
      extendBodyBehindAppBar: true,
      body: Stack(
        children: [
          if (_notFound)
            SafeArea(child: _ClosedState(onBack: _goBack))
          else if (detail != null)
            _content(context, detail)
          else
            _loadingOrError(),
          // Layer 3 — ghost back + cart on the scrim, over every state
          // (content, skeleton, error) so there is always a way out.
          if (!_notFound)
            Positioned(
              top: topInset,
              left: 8,
              right: 12,
              child: Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  _BackButton(onTap: _goBack),
                  const _CartButton(),
                ],
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
    final topInset = MediaQuery.paddingOf(context).top;
    // How much of the photo stays visible above the sheet on entry — the
    // framed first impression the founder asked for.
    const imagePeek = 24.0;
    final bannerHeight = topInset + _bannerHeight;

    return Stack(
      children: [
        // Layer 1 — the fixed backdrop: the banner never scrolls; the
        // content sheet slides over it. Extends up under the status bar,
        // with a top scrim so the ghost buttons read on any picture.
        Positioned(
          top: 0,
          left: 0,
          right: 0,
          child: SizedBox(
            height: bannerHeight,
            child: Stack(
              fit: StackFit.expand,
              children: [
                RemoteImage(
                  url: store.imageUrl,
                  seed: store.name,
                  borderRadius: 0,
                  memCacheSize: 1080,
                  fallbackIcon: Icons.storefront_rounded,
                ),
                const _TopScrim(),
              ],
            ),
          ),
        ),
        // Layer 2 — the ONLY scrollable: a transparent gap down to the
        // peek point, then the rounded sheet with everything in it.
        ListView(
          padding: EdgeInsets.zero,
          children: [
            SizedBox(height: bannerHeight - imagePeek),
            Container(
              decoration: BoxDecoration(
                color: AppColors.surface,
                borderRadius:
                    const BorderRadius.vertical(top: Radius.circular(24)),
                boxShadow: [
                  BoxShadow(
                    color: AppColors.surface.withValues(alpha: 0.6),
                    blurRadius: 16,
                    offset: const Offset(0, -4),
                  ),
                ],
              ),
              child: Padding(
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
                    onTap: () => _showProduct(context, store, products[i]),
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
            ),
          ],
        ),
      ],
    );
  }

  Widget _loadingOrError() {
    final error = _error;
    if (error != null) {
      // Top padding clears the floating back button above it.
      return SafeArea(
        child: ListView(
          physics: const AlwaysScrollableScrollPhysics(),
          padding: const EdgeInsets.fromLTRB(24, 72, 24, 24),
          children: [ErrorState(message: error, onRetry: _load)],
        ),
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

  /// Product tap → bottom sheet: picture, name, price, full description,
  /// and the add button — the sheet is where the buying decision happens,
  /// so the add lives here too (no hunt back to the row's small circle).
  ///
  /// The sheet's content scrolls: a Column sized to its children inside a
  /// height-capped sheet overflows (yellow-black stripes) the moment a
  /// merchant writes a long description. `isScrollControlled` lifts the
  /// default half-screen cap, the box bounds the sheet at ~85% of the
  /// screen, and the scroll view carries anything taller.
  void _showProduct(
    BuildContext context,
    Store store,
    MenuItem product,
  ) {
    final textTheme = Theme.of(context).textTheme;
    final description = product.description;
    showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      backgroundColor: AppColors.surfaceAlt,
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(top: Radius.circular(20)),
      ),
      builder: (sheetContext) => ConstrainedBox(
        constraints: BoxConstraints(
          maxHeight: MediaQuery.of(sheetContext).size.height * 0.85,
        ),
        child: SingleChildScrollView(
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
                _SheetGallery(product: product),
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
                const SizedBox(height: 24),
                FilledButton(
                  // Close first, then add — the snackbar lands on the
                  // store screen, confirming the fresh count.
                  onPressed: () {
                    Navigator.of(sheetContext).pop();
                    unawaited(_addToCart(context, store, product));
                  },
                  style: FilledButton.styleFrom(
                    minimumSize: const Size.fromHeight(52),
                  ),
                  child: Text(
                    'Add to cart · ${formatRwf(product.price)}',
                    style: const TextStyle(
                      fontSize: 16,
                      fontWeight: FontWeight.w700,
                    ),
                  ),
                ),
              ],
            ),
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
                url: product.displayImage,
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

/// Ghost button over the banner: no chrome, just the icon with a soft
/// shadow — the scrim above it does the legibility work.
class _BackButton extends StatelessWidget {
  const _BackButton({required this.onTap});

  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return Material(
      color: Colors.transparent,
      shape: const CircleBorder(),
      child: InkWell(
        customBorder: const CircleBorder(),
        onTap: onTap,
        child: SizedBox(
          width: 40,
          height: 40,
          child: Icon(
            Icons.arrow_back_rounded,
            size: 22,
            color: AppColors.onSurface,
            shadows: [
              Shadow(
                color: AppColors.surface.withValues(alpha: 0.8),
                blurRadius: 8,
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// Ghost cart button over the banner — same treatment as the back
/// button. Shows a badge with the item count when the cart is non-empty;
/// tapping navigates to /cart.
class _CartButton extends ConsumerWidget {
  const _CartButton();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final cart = ref.watch(cartProvider);
    final count = cart.maybeWhen(
      data: (v) => v.itemCount,
      orElse: () => 0,
    );
    return Material(
      color: Colors.transparent,
      shape: const CircleBorder(),
      child: InkWell(
        customBorder: const CircleBorder(),
        onTap: () => context.push('/cart'),
        child: SizedBox(
          width: 40,
          height: 40,
          child: Stack(
            clipBehavior: Clip.none,
            alignment: Alignment.center,
            children: [
              Icon(
                Icons.shopping_cart_rounded,
                size: 22,
                color: AppColors.onSurface,
                shadows: [
                  Shadow(
                    color: AppColors.surface.withValues(alpha: 0.8),
                    blurRadius: 8,
                  ),
                ],
              ),
              if (count > 0)
                Positioned(
                  right: -4,
                  top: -2,
                  child: Container(
                    constraints:
                        const BoxConstraints(minWidth: 18, minHeight: 18),
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

/// Banner height below the status bar — shared by the real content and
/// the skeleton so the reveal doesn't jump.
const double _bannerHeight = 210;

/// Dark fade across the banner's top so the ghost icons read on any
/// photo, bright or dark.
class _TopScrim extends StatelessWidget {
  const _TopScrim();

  @override
  Widget build(BuildContext context) {
    return DecoratedBox(
      decoration: BoxDecoration(
        gradient: LinearGradient(
          begin: Alignment.topCenter,
          end: Alignment.bottomCenter,
          stops: const [0, 1],
          colors: [
            AppColors.surface.withValues(alpha: 0.72),
            AppColors.surface.withValues(alpha: 0),
          ],
        ),
      ),
      child: const SizedBox(height: 96),
    );
  }
}

/// Loading skeleton mirroring the real layout — fixed banner block, then
/// the sheet's identity lines and a few rows, quiet, while the store
/// loads.
class _StoreSkeleton extends StatelessWidget {
  const _StoreSkeleton();

  @override
  Widget build(BuildContext context) {
    final block = AppColors.onSurface.withValues(alpha: 0.07);
    final topInset = MediaQuery.paddingOf(context).top;
    const imagePeek = 24.0;
    return Stack(
      children: [
        Positioned(
          top: 0,
          left: 0,
          right: 0,
          child: SizedBox(
            height: topInset + _bannerHeight,
            child: const ColoredBox(color: AppColors.surfaceAlt),
          ),
        ),
        ListView(
          padding: EdgeInsets.zero,
          children: [
            SizedBox(height: topInset + _bannerHeight - imagePeek),
            Container(
              decoration: const BoxDecoration(
                color: AppColors.surface,
                borderRadius: BorderRadius.vertical(top: Radius.circular(24)),
              ),
              child: Padding(
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
                        padding: const EdgeInsets.symmetric(
                            horizontal: 12, vertical: 12),
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
                              decoration: const BoxDecoration(
                                color: AppColors.surfaceAlt,
                                shape: BoxShape.circle,
                              ),
                            ),
                          ],
                        ),
                      ),
                  ],
                ),
              ),
            ),
          ],
        ),
      ],
    );
  }
}

/// The product sheet's gallery: swipes through the product's images
/// (cover first) with tappable position dots when there is more than
/// one; a lone image renders without the machinery. Mouse drags page the
/// view too — desktop users are first-class here. Honest icon fallback
/// via [RemoteImage] — no fake photos, ever.
class _SheetGallery extends StatefulWidget {
  const _SheetGallery({required this.product});

  final MenuItem product;

  @override
  State<_SheetGallery> createState() => _SheetGalleryState();
}

class _SheetGalleryState extends State<_SheetGallery> {
  final _controller = PageController();
  int _page = 0;

  // Flutter's scrollables ignore mouse drags by default (dragDevices
  // covers touch, stylus, trackpad) — which made the gallery dead on a
  // desktop window. The mouse joins the list for this view only.
  static const _dragDevices = {
    PointerDeviceKind.touch,
    PointerDeviceKind.mouse,
  };

  void _goTo(int page) {
    _controller.animateToPage(
      page,
      duration: const Duration(milliseconds: 250),
      curve: Curves.easeOut,
    );
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final images = widget.product.images;
    final single = ClipRRect(
      borderRadius: BorderRadius.circular(20),
      child: AspectRatio(
        aspectRatio: 16 / 10,
        child: RemoteImage(
          url: widget.product.displayImage,
          seed: widget.product.name,
          memCacheSize: 1080,
        ),
      ),
    );
    if (images.length < 2) return single;

    return Column(
      children: [
        ClipRRect(
          borderRadius: BorderRadius.circular(20),
          child: AspectRatio(
            aspectRatio: 16 / 10,
            child: ScrollConfiguration(
              behavior: ScrollConfiguration.of(context)
                  .copyWith(dragDevices: _dragDevices),
              child: PageView.builder(
                controller: _controller,
                itemCount: images.length,
                onPageChanged: (page) => setState(() => _page = page),
                itemBuilder: (context, index) => RemoteImage(
                  url: images[index],
                  seed: '${widget.product.name}:$index',
                  borderRadius: 0,
                  memCacheSize: 1080,
                ),
              ),
            ),
          ),
        ),
        const SizedBox(height: 6),
        Row(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            for (var i = 0; i < images.length; i++)
              // A real tap target around a small dot — the indicator is
              // a control, not a decoration.
              GestureDetector(
                onTap: () => _goTo(i),
                behavior: HitTestBehavior.opaque,
                child: Padding(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 6,
                    vertical: 10,
                  ),
                  child: Container(
                    width: _page == i ? 18 : 6,
                    height: 6,
                    decoration: BoxDecoration(
                      color: _page == i
                          ? AppColors.primary
                          : AppColors.onSurfaceMuted.withValues(alpha: 0.35),
                      borderRadius: BorderRadius.circular(999),
                    ),
                  ),
                ),
              ),
          ],
        ),
      ],
    );
  }
}
