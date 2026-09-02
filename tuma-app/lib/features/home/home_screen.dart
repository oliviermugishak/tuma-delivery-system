import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import 'package:tuma_app/core/api/api_client.dart';
import 'package:tuma_app/core/api/models/address.dart';
import 'package:tuma_app/core/api/models/order.dart';
import 'package:tuma_app/core/api/models/store.dart';
import 'package:tuma_app/core/auth/auth_controller.dart';
import 'package:tuma_app/core/theme/app_colors.dart';
import 'package:tuma_app/core/theme/app_theme.dart';
import 'package:tuma_app/core/utils/format_rwf.dart';
import 'package:tuma_app/features/location/customer_location.dart';
import 'package:tuma_app/features/orders/active_orders_provider.dart';
import 'package:tuma_app/shared/widgets/design_system.dart';
import 'package:tuma_app/shared/widgets/error_state.dart';
import 'package:tuma_app/shared/widgets/push_once.dart';
import 'package:tuma_app/shared/widgets/remote_image.dart';
import 'package:tuma_app/shared/widgets/sliver_row_grid.dart';

/// Home tab — the redesign's screen 01: greeting + avatar, the deliver-to
/// bar (context first), the live order card claiming the top slot when a
/// delivery is moving (an active delivery is the most urgent fact in the
/// user's life), category chips, and the tinted store grid. Pure
/// browsing: discovery lives on the Search tab.
class HomeScreen extends ConsumerStatefulWidget {
  const HomeScreen({super.key});

  @override
  ConsumerState<HomeScreen> createState() => _HomeScreenState();
}

class _HomeScreenState extends ConsumerState<HomeScreen> {
  List<Store>? _stores;
  String? _error;

  List<Address>? _addresses;
  bool _showLocationHint = false;
  String? _category;

  @override
  void initState() {
    super.initState();
    unawaited(_load());
  }

  List<String> _categories(List<Store> stores) {
    final seen = <String>{};
    return [
      for (final store in stores)
        if (store.category != null &&
            store.category!.isNotEmpty &&
            seen.add(store.category!))
          store.category!,
    ];
  }

  List<Store> _filtered(List<Store> stores) {
    if (_category == null) return stores;
    return [
      for (final store in stores)
        if (store.category == _category) store,
    ];
  }

  /// First mount: the feed fires immediately with the persisted pin, and
  /// a fresh GPS fix — when the device can (the OS prompt on first run)
  /// — lands afterwards and re-anchors it. Feed first, GPS refines: the
  /// fix can take 10s or never come, and it must never block or break
  /// the feed. The active orders come from the shared realtime poller
  /// (Home only renders them) — the feed fetch is stores + addresses.
  Future<void> _load() async {
    final pin = await ref.read(customerLocationProvider.future);
    if (!mounted) return;
    await _fetchFeed(pin);
    // GPS refines: best-effort, no spinners — the feed is already on
    // screen; a fix only sharpens the distances.
    unawaited(_locate());
  }

  /// Pull-to-refresh: refresh the FEED only. A fresh GPS fix on every
  /// pull made the user wait on a 10s acquisition — the persisted pin
  /// already carries the distances; the location hint owns the retry.
  Future<void> _refresh() async {
    final pin = await ref.read(customerLocationProvider.future);
    await _fetchFeed(pin);
  }

  Future<void> _fetchFeed(CustomerLocation? located) async {
    if (!mounted) return;
    setState(() => _error = null);
    try {
      final api = ref.read(storeApiProvider);
      final stores = await api.listStores(lat: located?.lat, lng: located?.lng);
      List<Address>? addresses;
      try {
        addresses = await ref.read(addressApiProvider).list();
      } on ApiError {
        // The bar falls back to the invitation (P2).
      }
      if (!mounted) return;
      setState(() {
        _stores = stores;
        _addresses = addresses;
        _showLocationHint = located == null;
        if (_category != null && !_categories(stores).contains(_category)) {
          _category = null;
        }
      });
    } on ApiError catch (error) {
      if (!mounted) return;
      // A refresh failure with a list on screen keeps the list; the error
      // state is for first loads with nothing to show.
      if (_stores == null) setState(() => _error = error.message);
    } on Object {
      // A TypeError/FormatException from a bad body or model must leave
      // the same honest error state — never a skeleton forever.
      if (!mounted) return;
      if (_stores == null) {
        setState(() => _error = 'Something went wrong. Please try again.');
      }
    }
  }

  Future<void> _retryLocation() async {
    setState(() => _showLocationHint = false);
    await _load();
  }

  /// Locate once per launch, the founder-chosen flow: a fresh GPS fix
  /// when the device can (the OS prompt on first run), else the
  /// persisted pin, else null — the bare feed with no distances. GPS is
  /// best-effort by design: it must never block or break the feed — it
  /// runs after the feed is on screen, and a fix re-anchors it via the
  /// same silent refresh pull-to-refresh uses.
  Future<CustomerLocation?> _locate() async {
    try {
      final acquire = ref.read(acquireLocationProvider);
      final fix = await acquire();
      await ref.read(customerLocationProvider.notifier).setPin(fix);
      await _refresh();
      return fix;
    } on Object {
      // No GPS (desktop dev), services off, permission denied — the
      // persisted pin (if there is one) already drove the feed; the
      // location hint owns the retry.
      return null;
    }
  }

  @override
  Widget build(BuildContext context) {
    final session = ref.watch(sessionProvider).asData?.value;
    final user = switch (session) {
      SessionUser s => s.user,
      _ => null,
    };
    final deliverTo = _deliverToLine();
    final textTheme = Theme.of(context).textTheme;
    final stores = _stores;
    final filtered = stores == null ? null : _filtered(stores);
    final categories = stores == null ? null : _categories(stores);
    // The live order card rides the shared realtime poller: its status
    // words and pulse are always the server's truth, and the card
    // disappears the moment the order settles.
    final activeOrders = [
      for (final group in ref.watch(activeOrdersProvider).value ?? const <GroupSummary>[])
        if (groupInFlight(group)) group,
    ];
    // P15's tint system keys off the deliver-to bar's location: the
    // persisted pin is the address line; the hint hides it (P2).

    return Scaffold(
      body: SafeArea(
        child: RefreshIndicator(
          onRefresh: _refresh,
          color: AppColors.primary,
          backgroundColor: AppColors.surfaceAlt,
          child: CustomScrollView(
            physics: const AlwaysScrollableScrollPhysics(),
            slivers: [
              SliverPadding(
                padding: const EdgeInsets.fromLTRB(16, 12, 16, 0),
                sliver: SliverToBoxAdapter(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Row(
                        mainAxisAlignment: MainAxisAlignment.spaceBetween,
                        children: [
                          Text(
                            user?.displayName != null
                                ? 'Hi, ${user!.displayName}'
                                : 'Welcome to Tuma',
                            style: AppTheme.d1(textTheme),
                          ),
                          AccentAvatar(
                            text: user?.displayName ??
                                user?.phone ??
                                'Customer',
                            size: 40,
                          ),
                        ],
                      ),
                      const SizedBox(height: 12),
                      // The deliver-to bar: where this session's orders
                      // will go. Tapping opens the profile locations.
                      _DeliverToBar(
                        line: deliverTo,
                        onTap: () async {
                          await context.push('/profile/location');
                          if (mounted) unawaited(_refresh());
                        },
                      ),
                      // The live order card claims the highest-value slot
                      // when a delivery is moving (P11, P17).
                      if (activeOrders.isNotEmpty) ...[
                        const SizedBox(height: 10),
                        _LiveOrderCard(order: activeOrders.first),
                      ],
                      if (stores != null) ...[
                        if (categories!.isNotEmpty) ...[
                          const SizedBox(height: 14),
                          SingleChildScrollView(
                            scrollDirection: Axis.horizontal,
                            clipBehavior: Clip.none,
                            child: Row(
                              children: [
                                _CategoryChip(
                                  label: 'All',
                                  selected: _category == null,
                                  onTap: () => setState(() => _category = null),
                                ),
                                for (final category in categories)
                                  _CategoryChip(
                                    label: category,
                                    selected: _category == category,
                                    onTap: () =>
                                        setState(() => _category = category),
                                  ),
                              ],
                            ),
                          ),
                        ],
                        if (_showLocationHint) ...[
                          const SizedBox(height: 12),
                          _LocationHint(onTap: _retryLocation),
                        ],
                      ],
                    ],
                  ),
                ),
              ),
              if (stores == null && _error != null)
                SliverPadding(
                  padding: const EdgeInsets.symmetric(horizontal: 16),
                  sliver: SliverToBoxAdapter(
                    child: ErrorState(message: _error!, onRetry: _load),
                  ),
                )
              else if (stores == null)
                const SliverToBoxAdapter(child: _FeedSkeleton())
              else if (stores.isEmpty && _category != null)
                const SliverPadding(
                  padding: EdgeInsets.symmetric(horizontal: 16),
                  sliver: SliverToBoxAdapter(child: _NoMatch()),
                )
              else if (stores.isEmpty)
                const SliverPadding(
                  padding: EdgeInsets.symmetric(horizontal: 16),
                  sliver: SliverToBoxAdapter(child: _EmptyState()),
                )
              else ...[
                SliverPadding(
                  padding: const EdgeInsets.fromLTRB(16, 18, 16, 0),
                  sliver: SliverToBoxAdapter(
                    child: Text('Stores near you', style: AppTheme.sec(textTheme)),
                  ),
                ),
                const SliverToBoxAdapter(child: SizedBox(height: 10)),
                if (filtered!.isEmpty)
                  const SliverPadding(
                    padding: EdgeInsets.symmetric(horizontal: 16),
                    sliver: SliverToBoxAdapter(child: _NoMatch()),
                  )
                else
                  SliverPadding(
                    padding: const EdgeInsets.symmetric(horizontal: 16),
                    sliver: SliverLayoutBuilder(
                      builder: (context, constraints) {
                        final metrics = _cardMetrics(constraints.crossAxisExtent);
                        return SliverRowGrid(
                          itemCount: filtered.length,
                          columns: metrics.columns,
                          cellWidth: metrics.cardWidth,
                          spacing: _cardSpacing,
                          itemBuilder: (BuildContext context, int index) => _StoreCard(
                            store: filtered[index],
                            onTap: () => unawaited(
                              pushOnce(context, '/stores/${filtered[index].id}'),
                            ),
                          ),
                        );
                      },
                    ),
                  ),
              ],
              const SliverToBoxAdapter(child: SizedBox(height: 24)),
            ],
          ),
        ),
      ),
    );
  }

  /// The deliver-to line: the persisted pin's coordinates formatted as a
  /// place-line, or "Set your delivery location" when unknown. The saved
  /// address book replaces this line's data source when the profile
  /// slice lands; the bar itself stays.
  /// The deliver-to line: the DEFAULT SAVED ADDRESS (the address book
  /// is the source of truth), else the persisted pin as a short
  /// place-line, else the invitation. Never the display name — a name
  /// is not a place (P2's sibling: don't render a fact you don't have).
  String _deliverToLine() {
    final addresses = _addresses;
    if (addresses != null && addresses.isNotEmpty) {
      final def = addresses.where((a) => a.isDefault).toList();
      return (def.isNotEmpty ? def.first : addresses.first).addressText;
    }
    return 'Set your delivery location';
  }
}

const double _cardSpacing = 12;

/// Two columns on a phone, up to three on wide windows — the same grid
/// math as the Search shelf, with the spec's 12px gaps.
({int columns, double cardWidth}) _cardMetrics(double width) {
  final columns = width < 400 ? 2 : (width ~/ 220).clamp(2, 3);
  final cardWidth = (width - _cardSpacing * (columns - 1)) / columns;
  return (columns: columns, cardWidth: cardWidth);
}

/// "Deliver to · Kk 40 Street, Kigali" — the context bar (P3: it states
/// the outcome, not the mechanism).
class _DeliverToBar extends StatelessWidget {
  const _DeliverToBar({required this.line, this.onTap});

  final String line;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
      decoration: BoxDecoration(
        color: AppColors.surfaceAlt,
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: AppColors.surfaceBorder),
      ),
      child: InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(16),
        child: Row(
          children: [
            const Icon(Icons.location_on_rounded,
                size: 20, color: AppColors.primary),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                const MicroLabel('Deliver to'),
                const SizedBox(height: 2),
                Text(
                  line,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: AppTheme.bd(Theme.of(context).textTheme)
                      .copyWith(fontWeight: FontWeight.w600),
                ),
              ],
            ),
          ),
            const Icon(Icons.expand_more_rounded,
                size: 20, color: AppColors.onSurfaceMuted),
          ],
        ),
      ),
    );
  }
}

/// The live order card: the most recent in-flight purchase with its
/// pulsing progress — one tap to Track (P11, P13, P17). The status row
/// reads the server's story (never a hardcoded label).
class _LiveOrderCard extends StatelessWidget {
  const _LiveOrderCard({required this.order});

  final GroupSummary order;

  @override
  Widget build(BuildContext context) {
    final store = order.stores.isNotEmpty ? order.stores.first : 'Your order';
    final story = activeOrderStory(order);
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 12),
      decoration: BoxDecoration(
        color: AppColors.surfaceAlt,
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: story.color.withValues(alpha: 0.35)),
      ),
      child: Row(
        children: [
          const Icon(Icons.two_wheeler_rounded,
              size: 24, color: AppColors.primary),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  '$store · ${order.firstItemName ?? '${order.itemsCount} items'}',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: AppTheme.bd(Theme.of(context).textTheme)
                      .copyWith(fontWeight: FontWeight.w600),
                ),
                const SizedBox(height: 3),
                StatusRow(
                  color: story.color,
                  label: story.label,
                  pulsing: story.pulsing,
                ),
              ],
            ),
          ),
          TrackPill(
              onTap: () =>
                  unawaited(pushOnce(context, '/orders/${order.id}'))),
        ],
      ),
    );
  }
}

/// One open store — the redesign's tinted card: 86dp media (a real photo
/// when the merchant uploaded one, else the category tint), name,
/// category, ETA, the accent fee pill.
class _StoreCard extends StatelessWidget {
  const _StoreCard({required this.store, required this.onTap});

  final Store store;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final category = (store.category != null && store.category!.isNotEmpty)
        ? store.category
        : null;
    final tint = CategoryTint.forCategory(category ?? '', hint: store.name);
    return Container(
      decoration: BoxDecoration(
        color: AppColors.surfaceAlt,
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: AppColors.surfaceBorder),
      ),
      clipBehavior: Clip.antiAlias,
      child: Material(
        color: Colors.transparent,
        child: InkWell(
          onTap: onTap,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              SizedBox(
                height: 86,
                child: Stack(
                  fit: StackFit.expand,
                  children: [
                    TintedTile(
                      tint: tint,
                      borderRadius: 0,
                      iconSize: 30,
                    ),
                    if (store.imageUrl != null)
                      RemoteImage(
                        url: store.imageUrl,
                        seed: store.name,
                        borderRadius: 0,
                        memCacheSize: 720,
                        fallbackIcon: Icons.storefront_rounded,
                      ),
                  ],
                ),
              ),
              Padding(
                padding: const EdgeInsets.fromLTRB(12, 10, 12, 12),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      store.name,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        fontSize: 13.5,
                        fontWeight: FontWeight.w600,
                        color: AppColors.onSurface,
                        height: 1.25,
                      ),
                    ),
                    if (category != null) ...[
                      const SizedBox(height: 2),
                      Text(
                        category,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: TextStyle(
                          fontSize: 11.5,
                          fontWeight: FontWeight.w500,
                          color: AppColors.onSurfaceMuted,
                        ),
                      ),
                    ],
                    const SizedBox(height: 9),
                    Row(
                      children: [
                        if (store.etaMin != null) ...[
                          const Icon(Icons.schedule_rounded,
                              size: 14, color: AppColors.onSurfaceMuted),
                          const SizedBox(width: 4),
                          // Expanded, not fixed + Spacer: the fee pill
                          // keeps its full price and the eta absorbs the
                          // squeeze — a Row of fixed children overflows
                          // on a narrow card.
                          Expanded(
                            child: Text(
                              '~${store.etaMin} min',
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: TextStyle(
                                fontSize: 11.5,
                                fontWeight: FontWeight.w500,
                                color: AppColors.onSurfaceMuted,
                              ),
                            ),
                          ),
                        ] else
                          const Spacer(),
                        _FeePill(fee: store.deliveryFee),
                      ],
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// The accent fee pill — the delivery fee in its yellow badge (P14:
/// accent carries money).
class _FeePill extends StatelessWidget {
  const _FeePill({required this.fee});

  final int fee;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 9, vertical: 4),
      decoration: BoxDecoration(
        color: AppColors.primary,
        borderRadius: BorderRadius.circular(999),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          const Icon(Icons.two_wheeler_rounded,
              size: 13, color: AppColors.onPrimary),
          const SizedBox(width: 4),
          Text(
            formatRwf(fee),
            style: const TextStyle(
              fontSize: 11,
              fontWeight: FontWeight.w700,
              color: AppColors.onPrimary,
            ),
          ),
        ],
      ),
    );
  }
}



/// One category filter pill — accent when selected, quiet otherwise.
class _CategoryChip extends StatelessWidget {
  const _CategoryChip({
    required this.label,
    required this.selected,
    required this.onTap,
  });

  final String label;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(right: 8),
      child: Material(
        color: selected ? AppColors.primary : AppColors.surfaceHigh,
        borderRadius: BorderRadius.circular(999),
        child: InkWell(
          borderRadius: BorderRadius.circular(999),
          onTap: onTap,
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
            child: Text(
              label,
              style: TextStyle(
                fontSize: 13,
                fontWeight: FontWeight.w600,
                color: selected ? AppColors.onPrimary : AppColors.onSurfaceMuted,
              ),
            ),
          ),
        ),
      ),
    );
  }
}

/// The bare-feed hint: GPS failed and no pin exists, so distances are
/// hidden. One tap re-runs the launch flow's GPS attempt.
class _LocationHint extends StatelessWidget {
  const _LocationHint({required this.onTap});

  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return Material(
      color: AppColors.surfaceAlt,
      borderRadius: BorderRadius.circular(12),
      child: InkWell(
        borderRadius: BorderRadius.circular(12),
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
          child: Row(
            children: [
              const Icon(
                Icons.location_on_outlined,
                size: 18,
                color: AppColors.primary,
              ),
              const SizedBox(width: 10),
              Expanded(
                child: Text(
                  'Add your location to see what\'s closest.',
                  style: Theme.of(context).textTheme.bodySmall?.copyWith(
                        color: AppColors.onSurfaceMuted,
                      ),
                ),
              ),
              const Icon(
                Icons.refresh_rounded,
                size: 18,
                color: AppColors.onSurfaceMuted,
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _NoMatch extends StatelessWidget {
  const _NoMatch();

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 48),
      child: Column(
        children: [
          const Icon(
            Icons.search_off_rounded,
            size: 40,
            color: AppColors.onSurfaceMuted,
          ),
          const SizedBox(height: 12),
          Text(
            'Nothing matches that filter.',
            style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                  color: AppColors.onSurfaceMuted,
                ),
          ),
        ],
      ),
    );
  }
}

class _EmptyState extends StatelessWidget {
  const _EmptyState();

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 48),
      child: Column(
        children: [
          const Icon(
            Icons.storefront_rounded,
            size: 40,
            color: AppColors.onSurfaceMuted,
          ),
          const SizedBox(height: 12),
          Text(
            'No stores are open right now — check back soon.',
            textAlign: TextAlign.center,
            style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                  color: AppColors.onSurfaceMuted,
                ),
          ),
        ],
      ),
    );
  }
}

/// The feed skeleton shaped like the real layout (P12): greeting bar,
/// deliver-to card, chips row, then the two-column grid of tiles.
class _FeedSkeleton extends StatelessWidget {
  const _FeedSkeleton();

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 0, 16, 0),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          _shimmer(140, 28),
          const SizedBox(height: 12),
          _shimmer(double.infinity, 58),
          const SizedBox(height: 18),
          _shimmer(120, 20),
          const SizedBox(height: 10),
          LayoutBuilder(
            builder: (context, constraints) {
              final metrics = _cardMetrics(constraints.maxWidth);
              return Column(
                children: [
                  for (var i = 0; i < 4; i += metrics.columns)
                    Padding(
                      padding: const EdgeInsets.only(bottom: 12),
                      child: Row(
                        children: [
                          for (var c = 0; c < metrics.columns; c++)
                            Padding(
                              padding: EdgeInsets.only(
                                right: c < metrics.columns - 1
                                    ? _cardSpacing
                                    : 0,
                              ),
                              child: SizedBox(
                                width: metrics.cardWidth,
                                child: Column(
                                  children: [
                                    Container(
                                      height: 86,
                                      decoration: BoxDecoration(
                                        color: AppColors.surfaceHigh,
                                        borderRadius:
                                            BorderRadius.circular(16),
                                      ),
                                    ),
                                    const SizedBox(height: 8),
                                    _shimmer(double.infinity, 14),
                                    const SizedBox(height: 6),
                                    _shimmer(90, 12),
                                  ],
                                ),
                              ),
                            ),
                        ],
                      ),
                    ),
                ],
              );
            },
          ),
        ],
      ),
    );
  }

  Widget _shimmer(double width, double height) {
    return Container(
      width: width,
      height: height,
      decoration: BoxDecoration(
        color: AppColors.surfaceHigh,
        borderRadius: BorderRadius.circular(8),
      ),
    );
  }
}
