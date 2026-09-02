import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import 'package:tuma_app/core/api/api_client.dart';
import 'package:tuma_app/core/api/models/product.dart';
import 'package:tuma_app/core/api/models/store.dart';
import 'package:tuma_app/core/auth/auth_controller.dart';
import 'package:tuma_app/core/theme/app_colors.dart';
import 'package:tuma_app/core/theme/app_theme.dart';
import 'package:tuma_app/shared/widgets/design_system.dart';
import 'package:tuma_app/core/utils/format_rwf.dart';
import 'package:tuma_app/features/cart/cart_notifier.dart';
import 'package:tuma_app/shared/widgets/error_state.dart';
import 'package:tuma_app/shared/widgets/remote_image.dart';
import 'package:tuma_app/shared/widgets/show_app_snack.dart';
import 'package:tuma_app/shared/widgets/store_contact_sheet.dart';

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
    } on Object {
      // A TypeError/FormatException from a bad body or model must leave
      // the same honest error state — never a skeleton forever.
      if (!mounted) return;
      setState(() => _error = 'Something went wrong. Please try again.');
    }
  }

  /// The store's details, one tap away: name, address, and the real
  /// Call/Email actions from the server's contact fields.
  Future<void> _showStoreInfo(Store store) {
    return showStoreContactSheet(
      context,
      entries: [
        StoreContactEntry(
          name: store.name,
          address: store.addressText,
          phone: store.contactPhone,
          email: store.contactEmail,
        ),
      ],
    );
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
              // The spec's sheet-card: bg-colored, 26px overlap over the
              // hero, hairline top edge — depth through surface (P6).
              margin: const EdgeInsets.only(top: 8),
              decoration: const BoxDecoration(
                color: AppColors.surface,
                borderRadius: BorderRadius.vertical(top: Radius.circular(26)),
                border: Border(
                  top: BorderSide(color: AppColors.surfaceBorder),
                ),
              ),
              child: Padding(
                padding: const EdgeInsets.fromLTRB(16, 20, 16, 96),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(store.name, style: AppTheme.d2(textTheme)),
                    if (address != null && address.isNotEmpty) ...[
                      const SizedBox(height: 3),
                      Text(address, style: AppTheme.sub(textTheme)),
                    ],
                    const SizedBox(height: 12),
                    // The facts that matter before the menu: the fee in
                    // its accent pill, the ETA in its quiet well — and
                    // the info button that surfaces the store's contact
                    // details (the founder's "?" on every store screen).
                    Row(
                      children: [
                        _FeePill(label: 'Delivery · ${formatRwf(store.deliveryFee)}'),
                        if (store.etaMin != null) ...[
                          const SizedBox(width: 8),
                          Container(
                            height: 30,
                            padding: const EdgeInsets.symmetric(horizontal: 12),
                            alignment: Alignment.center,
                            decoration: BoxDecoration(
                              color: AppColors.surfaceHigh,
                              borderRadius: BorderRadius.circular(999),
                            ),
                            child: Row(
                              mainAxisSize: MainAxisSize.min,
                              children: [
                                const Icon(Icons.schedule_rounded,
                                    size: 14,
                                    color: AppColors.onSurfaceMuted),
                                const SizedBox(width: 4),
                                Text(
                                  '~${store.etaMin} min',
                                  style: AppTheme.sub(textTheme),
                                ),
                              ],
                            ),
                          ),
                        ],
                        const Spacer(),
                        GestureDetector(
                          onTap: () => unawaited(_showStoreInfo(store)),
                          behavior: HitTestBehavior.opaque,
                          child: Container(
                            width: 36,
                            height: 36,
                            decoration: BoxDecoration(
                              shape: BoxShape.circle,
                              border: Border.all(
                                color: AppColors.surfaceBorder,
                              ),
                            ),
                            child: const Icon(
                              Icons.info_outline_rounded,
                              size: 19,
                              color: AppColors.onSurfaceMuted,
                            ),
                          ),
                        ),
                      ],
                    ),
                    const SizedBox(height: 14),
                    Text('Menu', style: AppTheme.sec(textTheme)),
                    const SizedBox(height: 10),
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
                      Container(
                        decoration: BoxDecoration(
                          color: AppColors.surfaceAlt,
                          borderRadius: BorderRadius.circular(16),
                          border: Border.all(color: AppColors.surfaceBorder),
                        ),
                        child: Column(
                          children: [
                            for (var i = 0; i < products.length; i++) ...[
                              _ProductRow(
                                product: products[i],
                                onTap: () =>
                                    _showProduct(context, store, products[i]),
                                onAdd: () => unawaited(
                                  _addToCart(context, store, products[i]),
                                ),
                              ),
                              if (i < products.length - 1)
                                Container(
                                  height: 1,
                                  margin: const EdgeInsets.symmetric(
                                      horizontal: 12),
                                  color: AppColors.surfaceBorder,
                                ),
                            ],
                          ],
                        ),
                      ),
                  ],
                ),
              ),
            ),
          ],
        ),
        // Layer 3 — the floating cart bar LAST in the Stack: paint order
        // is hit-test order, so the bar sits ABOVE the scrolling sheet
        // and every tap reaches it (the old layer order let the sheet
        // paint over the bar and swallow its taps — "View cart does
        // nothing").
        const _FloatingCartBar(),
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
    MenuItem item, {
    int quantity = 1,
  }) async {
    ScaffoldMessenger.of(context).hideCurrentSnackBar();
    await ref.read(cartProvider.notifier).add(
          item,
          storeId: store.id,
          storeName: store.name,
          deliveryFee: store.deliveryFee,
          quantity: quantity,
        );
    if (!mounted) return;
    final count = ref.read(cartProvider).maybeWhen(
          data: (v) => v.itemCount,
          orElse: () => 0,
        );
    showAppSnack(
      this.context,
      'Added to cart ($count item${count > 1 ? 's' : ''})',
      duration: const Duration(seconds: 1),
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
    showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      backgroundColor: AppColors.surface,
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(top: Radius.circular(26)),
      ),
      builder: (sheetContext) => _ProductSheet(
        store: store,
        product: product,
        onAdd: (quantity) {
          Navigator.of(sheetContext).pop();
          unawaited(_addToCart(context, store, product, quantity: quantity));
        },
      ),
    );
  }
}

/// The accent fee pill (P14: accent carries money) — "2,000 delivery".
class _FeePill extends StatelessWidget {
  const _FeePill({required this.label});

  final String label;

  @override
  Widget build(BuildContext context) {
    return Container(
      height: 30,
      padding: const EdgeInsets.symmetric(horizontal: 12),
      alignment: Alignment.center,
      decoration: BoxDecoration(
        color: AppColors.primary,
        borderRadius: BorderRadius.circular(999),
      ),
      child: Text(
        label,
        style: const TextStyle(
          fontSize: 11.5,
          fontWeight: FontWeight.w700,
          color: AppColors.onPrimary,
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
              Stack(
                children: [
                  TintedTile(
                    tint: CategoryTint.forCategory(
                      product.name,
                      hint: product.description ?? '',
                    ),
                    size: 52,
                    iconSize: 22,
                  ),
                  if (product.displayImage != null)
                    RemoteImage(
                      url: product.displayImage,
                      seed: product.name,
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
                      product.name,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: textTheme.titleSmall?.copyWith(fontSize: 15),
                    ),
                    if (description != null && description.isNotEmpty) ...[
                      const SizedBox(height: 2),
                      Text(
                        description,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: AppTheme.sub(textTheme),
                      ),
                    ],
                    const SizedBox(height: 4),
                    Text(
                      formatRwf(product.price),
                      style: textTheme.titleSmall?.copyWith(
                        fontSize: 14,
                        color: AppColors.primary,
                        fontWeight: FontWeight.w700,
                      ),
                    ),
                  ],
                ),
              ),
              const SizedBox(width: 12),
              _AddButton(onTap: onAdd),
            ],
          ),
        ),
      ),
    );
  }
}

/// The spec's addbtn: a 36dp accent circle with a plain "+" — it means
/// add, and it's a smaller, calmer target than a cart icon (P4).
class _AddButton extends StatelessWidget {
  const _AddButton({required this.onTap});

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
          child: Icon(Icons.add_rounded, size: 19, color: AppColors.onPrimary),
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

/// The floating cart bar — the screen's one primary action (P5): items
/// + live total over a shadow strong enough to separate it from
/// scrolling content (P6's single sanctioned shadow).
class _FloatingCartBar extends ConsumerWidget {
  const _FloatingCartBar();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final cart = ref.watch(cartProvider);
    final state = cart.maybeWhen(data: (v) => v, orElse: () => null);
    final count = state?.itemCount ?? 0;
    if (count == 0) return const SizedBox.shrink();
    final total = state?.total ?? 0;
    return Positioned(
      left: 16,
      right: 16,
      bottom: 18,
      child: Container(
        decoration: BoxDecoration(
          borderRadius: BorderRadius.circular(14),
          boxShadow: [
            BoxShadow(
              color: Colors.black.withValues(alpha: 0.45),
              blurRadius: 30,
              offset: const Offset(0, 12),
            ),
          ],
        ),
        child: FilledButton.icon(
          onPressed: () => context.push('/cart'),
          style: FilledButton.styleFrom(
            minimumSize: const Size.fromHeight(50),
          ),
          icon: const Icon(Icons.shopping_bag_rounded, size: 20),
          label: Text(
            'View cart · $count item${count > 1 ? 's' : ''} · ${formatRwf(total)}',
            style: const TextStyle(
              fontSize: 15,
              fontWeight: FontWeight.w600,
            ),
          ),
        ),
      ),
    );
  }
}

/// The product sheet — the redesign's screen 11: store caption + close X
/// above the gallery, elongated dots, the name/price with a quantity
/// stepper, and the CTA total going live with qty (P8).
class _ProductSheet extends StatefulWidget {
  const _ProductSheet({
    required this.store,
    required this.product,
    required this.onAdd,
  });

  final Store store;
  final MenuItem product;
  final ValueChanged<int> onAdd;

  @override
  State<_ProductSheet> createState() => _ProductSheetState();
}

class _ProductSheetState extends State<_ProductSheet> {
  int _quantity = 1;
  int _page = 0;

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final product = widget.product;
    final tint = CategoryTint.forCategory(
      product.name,
      hint: product.description ?? '',
    );
    final images = [
      ?product.displayImage,
      ...product.images,
    ];
    return ConstrainedBox(
      constraints: BoxConstraints(
        maxHeight: MediaQuery.of(context).size.height * 0.9,
      ),
      child: SingleChildScrollView(
        child: Padding(
          padding: const EdgeInsets.fromLTRB(16, 10, 16, 18),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const Center(child: _SheetHandle()),
              const SizedBox(height: 12),
              Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  Flexible(
                    child: Text(
                      widget.store.name,
                      style: AppTheme.cap(textTheme),
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                  _SheetCloseButton(onTap: () => Navigator.of(context).pop()),
                ],
              ),
              const SizedBox(height: 12),
              ClipRRect(
                borderRadius: BorderRadius.circular(16),
                child: SizedBox(
                  height: 172,
                  child: Stack(
                    fit: StackFit.expand,
                    children: [
                      TintedTile(tint: tint, borderRadius: 16, iconSize: 60),
                      if (images.isNotEmpty)
                        PageView.builder(
                          itemCount: images.length,
                          onPageChanged: (page) => setState(() => _page = page),
                          itemBuilder: (context, index) => RemoteImage(
                            url: images[index],
                            seed: '${product.name}:$index',
                            borderRadius: 0,
                            memCacheSize: 1080,
                            fallbackIcon: Icons.restaurant_rounded,
                          ),
                        ),
                    ],
                  ),
                ),
              ),
              if (images.length > 1) ...[
                const SizedBox(height: 10),
                DotIndicators(count: images.length, activeIndex: _page),
              ],
              const SizedBox(height: 14),
              Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          product.name,
                          style: AppTheme.d2(textTheme).copyWith(fontSize: 20),
                        ),
                        const SizedBox(height: 3),
                        Text(
                          formatRwf(product.price),
                          style: textTheme.titleSmall?.copyWith(
                            fontSize: 15,
                            color: AppColors.primary,
                            fontWeight: FontWeight.w700,
                          ),
                        ),
                      ],
                    ),
                  ),
                  // The quantity stepper — the missing half of the
                  // purchase decision.
                  Row(
                    children: [
                      _QtyButton(
                        icon: Icons.remove_rounded,
                        onTap: _quantity > 1
                            ? () => setState(() => _quantity -= 1)
                            : null,
                      ),
                      SizedBox(
                        width: 28,
                        child: Text(
                          '$_quantity',
                          textAlign: TextAlign.center,
                          style: textTheme.titleMedium?.copyWith(
                            fontWeight: FontWeight.w700,
                          ),
                        ),
                      ),
                      _QtyButton(
                        icon: Icons.add_rounded,
                        onTap: () => setState(() => _quantity += 1),
                      ),
                    ],
                  ),
                ],
              ),
              if (product.description != null &&
                  product.description!.isNotEmpty) ...[
                const SizedBox(height: 10),
                Text(
                  product.description!,
                  style: textTheme.bodySmall?.copyWith(
                    color: AppColors.onSurfaceMuted,
                    fontWeight: FontWeight.w400,
                    height: 1.55,
                    fontSize: 13,
                  ),
                ),
              ],
              const SizedBox(height: 16),
              FilledButton(
                onPressed: () => widget.onAdd(_quantity),
                child: Text(
                  'Add to cart · ${formatRwf(product.price * _quantity)}',
                  style: const TextStyle(
                    fontSize: 15,
                    fontWeight: FontWeight.w600,
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

class _SheetHandle extends StatelessWidget {
  const _SheetHandle();

  @override
  Widget build(BuildContext context) {
    return Container(
      width: 36,
      height: 4,
      decoration: BoxDecoration(
        color: AppColors.surfaceHigh,
        borderRadius: BorderRadius.circular(999),
      ),
    );
  }
}

class _SheetCloseButton extends StatelessWidget {
  const _SheetCloseButton({required this.onTap});

  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      onTap: onTap,
      child: Container(
        width: 32,
        height: 32,
        decoration: const BoxDecoration(
          color: AppColors.surfaceHigh,
          shape: BoxShape.circle,
        ),
        child: const Icon(
          Icons.close_rounded,
          size: 17,
          color: AppColors.onSurfaceMuted,
        ),
      ),
    );
  }
}

class _QtyButton extends StatelessWidget {
  const _QtyButton({required this.icon, required this.onTap});

  final IconData icon;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      onTap: onTap,
      child: Container(
        width: 38,
        height: 38,
        decoration: const BoxDecoration(
          color: AppColors.surfaceHigh,
          shape: BoxShape.circle,
        ),
        child: Icon(icon, size: 18, color: AppColors.onSurface),
      ),
    );
  }
}
