import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'package:tuma_app/core/api/api_client.dart';
import 'package:tuma_app/core/api/models/discovery.dart';
import 'package:tuma_app/core/api/models/store.dart';
import 'package:tuma_app/core/auth/auth_controller.dart';
import 'package:tuma_app/core/theme/app_colors.dart';
import 'package:tuma_app/core/theme/app_theme.dart';
import 'package:tuma_app/core/utils/format_rwf.dart';
import 'package:tuma_app/features/location/customer_location.dart';
import 'package:tuma_app/shared/widgets/error_state.dart';
import 'package:tuma_app/shared/widgets/design_system.dart';
import 'package:tuma_app/shared/widgets/sliver_row_grid.dart';
import 'package:tuma_app/shared/widgets/remote_image.dart';

/// Where the customer's recent searches live, newest first, five deep.
const _kRecentSearches = 'tuma_recent_searches_v1';
const _maxRecentSearches = 5;

/// Sign-out wipes the recents with the rest of the customer's data —
/// the clear lives beside the key so the two can't drift apart.
Future<void> clearRecentSearches() async {
  try {
    final prefs = await SharedPreferences.getInstance();
    await prefs.remove(_kRecentSearches);
  } on Object {
    // Best-effort, like every end-of-session write: a failure must not
    // break the sign-out flow. The in-memory list drops with the screen.
  }
}

/// The Search tab — discovery. One field, two tabs: **Products**
/// (default: the 🔥 Popular near you shelf — real order counts, never
/// fake) and **Stores** (default: every open store). A search term
/// narrows both sections server-side; this screen never does geo math or
/// name matching of its own.
class SearchScreen extends ConsumerStatefulWidget {
  const SearchScreen({super.key});

  @override
  ConsumerState<SearchScreen> createState() => _SearchScreenState();
}

class _SearchScreenState extends ConsumerState<SearchScreen>
    with SingleTickerProviderStateMixin {
  late final TabController _tab = TabController(length: 2, vsync: this);
  final _searchController = TextEditingController();
  Timer? _debounce;
  int _seq = 0;

  SearchResult? _result;
  String? _error;
  /// The committed term the server is answering; the raw text lives in
  /// the controller.
  String _query = '';
  List<String> _recents = const [];

  @override
  void initState() {
    super.initState();
    _loadRecents();
    unawaited(_runSearch());
  }

  @override
  void dispose() {
    _debounce?.cancel();
    _searchController.dispose();
    _tab.dispose();
    super.dispose();
  }

  Future<void> _loadRecents() async {
    final prefs = await SharedPreferences.getInstance();
    if (!mounted) return;
    setState(() {
      _recents = prefs.getStringList(_kRecentSearches) ?? const [];
    });
  }

  /// Best-effort persistence: a failed write never breaks searching.
  Future<void> _saveRecents(List<String> recents) async {
    setState(() => _recents = recents);
    try {
      final prefs = await SharedPreferences.getInstance();
      await prefs.setStringList(_kRecentSearches, recents);
    } on Object {
      // In-memory recents survive; persistence tried again next time.
    }
  }

  Future<void> _rememberSearch(String term) => _saveRecents([
        term,
        ..._recents.where((r) => r.toLowerCase() != term.toLowerCase()),
      ].take(_maxRecentSearches).toList());

  /// Removes one recent (its ✕) — or all of them (the row's "Clear").
  Future<void> _forgetSearch(String? term) async {
    final remaining =
        term == null ? const <String>[] : _recents.where((r) => r != term);
    await _saveRecents(remaining.toList());
  }

  /// Keystrokes debounce; only the settled term reaches the server, and
  /// a reply overtaken by a newer keystroke is dropped.
  void _onSearchChanged(String raw) {
    _debounce?.cancel();
    _debounce = Timer(const Duration(milliseconds: 250), () {
      final term = raw.trim();
      if (term == _query) return;
      _query = term;
      if (term.isNotEmpty) unawaited(_rememberSearch(term));
      unawaited(_runSearch());
    });
  }

  Future<void> _runSearch() async {
    final seq = ++_seq;
    setState(() => _error = null);
    // The persisted pin (set by checkout, the profile map, or a GPS fix)
    // sharpens both sections — searches never re-run the GPS fix.
    final pin = await ref.read(customerLocationProvider.future);
    if (!mounted || seq != _seq) return;
    try {
      final result = await ref
          .read(storeApiProvider)
          .search(q: _query, lat: pin?.lat, lng: pin?.lng);
      if (!mounted || seq != _seq) return;
      setState(() => _result = result);
    } on ApiError catch (error) {
      if (!mounted || seq != _seq) return;
      // A failed refresh keeps the last good discovery state; the error
      // state is for first loads with nothing on screen.
      if (_result == null) setState(() => _error = error.message);
    }
  }

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final result = _result;

    return Scaffold(
      body: SafeArea(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(24, 24, 24, 0),
              child: Text(
                'Search',
                style: textTheme.headlineSmall?.copyWith(
                  color: AppColors.onSurface,
                  fontWeight: FontWeight.w800,
                ),
              ),
            ),
            Padding(
              padding: const EdgeInsets.fromLTRB(24, 16, 24, 12),
              child: TextField(
                controller: _searchController,
                onChanged: _onSearchChanged,
                decoration: InputDecoration(
                  hintText: 'Search stores or food…',
                  hintStyle: textTheme.bodyMedium?.copyWith(
                    color: AppColors.onSurfaceMuted,
                  ),
                  prefixIcon: const Icon(
                    Icons.search_rounded,
                    size: 20,
                    color: AppColors.onSurfaceMuted,
                  ),
                  suffixIcon: _searchController.text.isEmpty
                      ? null
                      : IconButton(
                          icon: const Icon(
                            Icons.close_rounded,
                            size: 18,
                            color: AppColors.onSurfaceMuted,
                          ),
                          onPressed: () {
                            _searchController.clear();
                            _onSearchChanged('');
                          },
                        ),
                  filled: true,
                  fillColor: AppColors.surfaceAlt,
                  contentPadding: const EdgeInsets.symmetric(vertical: 12),
                  border: OutlineInputBorder(
                    borderRadius: BorderRadius.circular(14),
                    borderSide: BorderSide(color: AppColors.surfaceBorder),
                  ),
                  enabledBorder: OutlineInputBorder(
                    borderRadius: BorderRadius.circular(14),
                    borderSide: BorderSide(color: AppColors.surfaceBorder),
                  ),
                  focusedBorder: OutlineInputBorder(
                    borderRadius: BorderRadius.circular(14),
                    borderSide: const BorderSide(
                      color: AppColors.primary,
                      width: 1.5,
                    ),
                  ),
                ),
                style: textTheme.bodyMedium,
              ),
            ),
            // Recents belong to the EMPTY-FIELD state — a visible rule,
            // not a focus rule: empty field = shortcuts on screen; a
            // committed term = results. Removing them is always one
            // clear-tap away.
            if (_query.isEmpty && _recents.isNotEmpty)
              Padding(
                padding: const EdgeInsets.fromLTRB(24, 0, 24, 8),
                child: _RecentSearches(
                  recents: _recents,
                  onPick: (term) {
                    _searchController.text = term;
                    _query = term;
                    unawaited(_runSearch());
                  },
                  onRemove: (term) => unawaited(_forgetSearch(term)),
                  onClearAll: () => unawaited(_forgetSearch(null)),
                ),
              ),
            TabBar(
              controller: _tab,
              labelColor: AppColors.primary,
              unselectedLabelColor: AppColors.onSurfaceMuted,
              indicatorColor: AppColors.primary,
              dividerColor: AppColors.surfaceBorder,
              tabs: const [
                Tab(text: 'Products'),
                Tab(text: 'Stores'),
              ],
            ),
            Expanded(
              child: result == null && _error != null
                  ? ErrorState(message: _error!, onRetry: _runSearch)
                  : result == null
                      ? const Center(child: CircularProgressIndicator())
                      : TabBarView(
                          controller: _tab,
                          children: [
                            _ProductsTab(result: result, query: _query),
                            _StoresTab(result: result, query: _query),
                          ],
                        ),
            ),
          ],
        ),
      ),
    );
  }
}

/// The persisted recent searches, as quiet chips — each tappable to
/// re-run, each removable with its ✕, the whole row clearable.
class _RecentSearches extends StatelessWidget {
  const _RecentSearches({
    required this.recents,
    required this.onPick,
    required this.onRemove,
    required this.onClearAll,
  });

  final List<String> recents;
  final ValueChanged<String> onPick;
  final ValueChanged<String> onRemove;
  final VoidCallback onClearAll;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            const Icon(
              Icons.history_rounded,
              size: 14,
              color: AppColors.onSurfaceMuted,
            ),
            const SizedBox(width: 6),
            Text(
              'Recent searches',
              style: Theme.of(context).textTheme.labelSmall?.copyWith(
                    color: AppColors.onSurfaceMuted,
                    fontWeight: FontWeight.w600,
                  ),
            ),
            const Spacer(),
            GestureDetector(
              onTap: onClearAll,
              behavior: HitTestBehavior.opaque,
              child: Padding(
                padding: const EdgeInsets.symmetric(
                  horizontal: 6,
                  vertical: 4,
                ),
                child: Text(
                  'Clear',
                  style: Theme.of(context).textTheme.labelSmall?.copyWith(
                        color: AppColors.primary,
                        fontWeight: FontWeight.w600,
                      ),
                ),
              ),
            ),
          ],
        ),
        const SizedBox(height: 6),
        SizedBox(
          height: 36,
          child: ListView(
            scrollDirection: Axis.horizontal,
            children: [
              for (final term in recents)
                Padding(
                  padding: const EdgeInsets.only(right: 8),
                  child: Material(
                    color: AppColors.surfaceAlt,
                    shape: StadiumBorder(
                      side: BorderSide(color: AppColors.surfaceBorder),
                    ),
                    child: InkWell(
                      customBorder: const StadiumBorder(),
                      onTap: () => onPick(term),
                      child: Padding(
                        padding: const EdgeInsets.fromLTRB(12, 6, 6, 6),
                        child: Row(
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            Text(
                              term,
                              style: Theme.of(context)
                                  .textTheme
                                  .labelMedium
                                  ?.copyWith(
                                    color: AppColors.onSurfaceMuted,
                                    fontWeight: FontWeight.w600,
                                  ),
                            ),
                            const SizedBox(width: 4),
                            // The chip's own ✕ — removing one recent
                            // never disturbs the others.
                            GestureDetector(
                              onTap: () => onRemove(term),
                              behavior: HitTestBehavior.opaque,
                              child: const Padding(
                                padding: EdgeInsets.all(4),
                                child: Icon(
                                  Icons.close_rounded,
                                  size: 14,
                                  color: AppColors.onSurfaceMuted,
                                ),
                              ),
                            ),
                          ],
                        ),
                      ),
                    ),
                  ),
                ),
            ],
          ),
        ),
      ],
    );
  }
}

/// The Products tab: the 🔥 shelf when browsing, matched products when
/// searching. Every fact is server-owned; the fire means real orders.
class _ProductsTab extends StatelessWidget {
  const _ProductsTab({required this.result, required this.query});

  final SearchResult result;
  final String query;

  @override
  Widget build(BuildContext context) {
    final products = result.products;
    if (products.isEmpty) {
      return _DiscoveryEmpty(
        icon: Icons.search_off_rounded,
        title: query.isEmpty
            ? 'Nothing is popular yet.'
            : "No products match '$query'.",
        subtitle: query.isEmpty
            ? 'Orders will light this shelf up.'
            : 'Try another name.',
      );
    }
    return CustomScrollView(
      slivers: [
        SliverPadding(
          padding: const EdgeInsets.fromLTRB(24, 16, 24, 0),
          sliver: SliverToBoxAdapter(
            child: query.isEmpty
                ? Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: const [
                      _PopularHeader(),
                      SizedBox(height: 12),
                    ],
                  )
                : const SizedBox.shrink(),
          ),
        ),
        SliverPadding(
          padding: const EdgeInsets.fromLTRB(24, 0, 24, 24),
          sliver: SliverLayoutBuilder(
            builder: (context, constraints) {
              // Two tiles per row on a phone, three on wide windows — the
              // shelf is a grid, scrolling vertically. Rows build lazily
              // (the eager Wrap resolved every tile up front).
              final columns =
                  constraints.crossAxisExtent < 400 ? 2 : (constraints.crossAxisExtent ~/ 220).clamp(2, 3);
              const spacing = 12.0;
              final tileWidth =
                  (constraints.crossAxisExtent - spacing * (columns - 1)) / columns;
              return SliverRowGrid(
                itemCount: products.length,
                columns: columns,
                cellWidth: tileWidth,
                spacing: spacing,
                itemBuilder: (context, index) => _ProductTile(
                  hit: products[index],
                  popular: query.isEmpty,
                ),
              );
            },
          ),
        ),
      ],
    );
  }
}

/// The fire line: what the shelf is, in brand orange.
class _PopularHeader extends StatelessWidget {
  const _PopularHeader();

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        const Icon(
          Icons.local_fire_department_rounded,
          size: 18,
          color: AppColors.warning,
        ),
        const SizedBox(width: 8),
        Text('Popular near you',
            style: AppTheme.sec(Theme.of(context).textTheme)),
      ],
    );
  }
}

/// One hot (or matched) product: picture with the fire badge on top,
/// name, the store fulfilling it, price in gold. Tapping goes to the
/// store — the product lives there.
class _ProductTile extends StatelessWidget {
  const _ProductTile({required this.hit, required this.popular});

  final ProductHit hit;
  final bool popular;

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    // P4: distance is deleted from product cards — the useful question
    // for a product is "where from", answered by the store name below.
    final tint = CategoryTint.forCategory(hit.storeName, hint: hit.name);
    return Material(
      color: AppColors.surfaceAlt,
      borderRadius: BorderRadius.circular(16),
      clipBehavior: Clip.antiAlias,
      child: InkWell(
        onTap: () => context.push('/stores/${hit.storeId}'),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            SizedBox(
              height: 86,
              child: Stack(
                fit: StackFit.expand,
                children: [
                  TintedTile(tint: tint, borderRadius: 0, iconSize: 30),
                  if (hit.imageUrl != null)
                    RemoteImage(
                      url: hit.imageUrl,
                      seed: hit.name,
                      borderRadius: 0,
                      memCacheSize: 720,
                      fallbackIcon: Icons.lunch_dining_rounded,
                    ),
                  // Scarcity is what makes "popular" mean anything: the
                  // flame rides only where it's earned (top 2 of the
                  // shelf, decided by the caller).
                  if (popular)
                    Positioned(
                      top: 8,
                      left: 8,
                      child: Container(
                        width: 22,
                        height: 22,
                        decoration: BoxDecoration(
                          color: AppColors.surface.withValues(alpha: 0.75),
                          shape: BoxShape.circle,
                        ),
                        child: const Icon(
                          Icons.local_fire_department_rounded,
                          size: 14,
                          color: AppColors.warning,
                        ),
                      ),
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
                    hit.name,
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(
                      fontSize: 13.5,
                      fontWeight: FontWeight.w600,
                      color: AppColors.onSurface,
                      height: 1.25,
                    ),
                  ),
                  const SizedBox(height: 2),
                  Text(
                    hit.storeName,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(
                      fontSize: 11.5,
                      fontWeight: FontWeight.w500,
                      color: AppColors.onSurfaceMuted,
                    ),
                  ),
                  const SizedBox(height: 8),
                  Text(
                    formatRwf(hit.price),
                    style: textTheme.titleSmall?.copyWith(
                      fontSize: 14,
                      color: AppColors.primary,
                      fontWeight: FontWeight.w700,
                    ),
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// The Stores tab: every open store when browsing, matches when
/// searching — nearest first when the pin exists.
class _StoresTab extends StatelessWidget {
  const _StoresTab({required this.result, required this.query});

  final SearchResult result;
  final String query;

  @override
  Widget build(BuildContext context) {
    final stores = result.stores;
    if (stores.isEmpty) {
      return _DiscoveryEmpty(
        icon: Icons.storefront_rounded,
        title: query.isEmpty
            ? 'No stores are open right now.'
            : "No stores match '$query'.",
        subtitle: query.isEmpty
            ? 'Check back a little later.'
            : 'Try another name or category.',
      );
    }
    return ListView.separated(
      padding: const EdgeInsets.fromLTRB(24, 8, 24, 24),
      itemCount: stores.length,
      // The tiny gap: the rounded rows sat flush against each other.
      separatorBuilder: (_, _) => const SizedBox(height: 8),
      itemBuilder: (context, index) =>
          _StoreRow(store: stores[index], onTap: () {
        context.push('/stores/${stores[index].id}');
      }),
    );
  }
}

/// One store as a results row — quieter than the home cards, the shape
/// of a list you scan.
class _StoreRow extends StatelessWidget {
  const _StoreRow({required this.store, required this.onTap});

  final Store store;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final category = (store.category != null && store.category!.isNotEmpty)
        ? store.category
        : null;
    final distance = store.distanceM;
    final tint = CategoryTint.forCategory(category ?? '', hint: store.name);
    return Material(
      color: AppColors.surfaceAlt,
      borderRadius: BorderRadius.circular(16),
      child: InkWell(
        borderRadius: BorderRadius.circular(16),
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.symmetric(vertical: 12, horizontal: 14),
          child: Row(
            children: [
              Stack(
                children: [
                  TintedTile(tint: tint, size: 44, iconSize: 20),
                  if (store.imageUrl != null)
                    RemoteImage(
                      url: store.imageUrl,
                      seed: store.name,
                      width: 44,
                      height: 44,
                      borderRadius: 12,
                      memCacheSize: 120,
                      fallbackIcon: Icons.storefront_rounded,
                    ),
                ],
              ),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      store.name,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: textTheme.titleSmall?.copyWith(
                        fontSize: 15,
                        fontWeight: FontWeight.w600,
                      ),
                    ),
                    const SizedBox(height: 2),
                    // Distance is KEPT here, deliberately: for stores
                    // it's decision-relevant (P4's complement).
                    Text(
                      [
                        ?category,
                        if (distance != null)
                          '${(distance / 1000).toStringAsFixed(1)} km',
                      ].join(' · '),
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: AppTheme.sub(textTheme),
                    ),
                  ],
                ),
              ),
              const Icon(
                Icons.chevron_right_rounded,
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

/// Honest empty states — browsing with an empty market is different from
/// a search matching nothing, in words as well as icons.
class _DiscoveryEmpty extends StatelessWidget {
  const _DiscoveryEmpty({
    required this.icon,
    required this.title,
    required this.subtitle,
  });

  final IconData icon;
  final String title;
  final String subtitle;

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
              width: 56,
              height: 56,
              decoration: BoxDecoration(
                color: AppColors.surfaceAlt,
                borderRadius: BorderRadius.circular(18),
                border: Border.all(color: AppColors.surfaceBorder),
              ),
              child: Icon(icon, color: AppColors.onSurfaceMuted),
            ),
            const SizedBox(height: 14),
            Text(
              title,
              textAlign: TextAlign.center,
              style: textTheme.titleMedium?.copyWith(
                fontWeight: FontWeight.w700,
              ),
            ),
            const SizedBox(height: 4),
            Text(
              subtitle,
              textAlign: TextAlign.center,
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
