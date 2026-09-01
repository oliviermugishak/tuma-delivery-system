import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import 'package:tuma_app/core/api/models/order.dart';
import 'package:tuma_app/core/theme/app_colors.dart';
import 'package:tuma_app/core/theme/app_theme.dart';
import 'package:tuma_app/core/utils/format_rwf.dart';
import 'package:tuma_app/features/orders/active_orders_provider.dart';
import 'package:tuma_app/shared/widgets/design_system.dart';
import 'package:tuma_app/shared/widgets/error_state.dart';

/// Orders — the redesign's screen 03: Active and History tabs. Live
/// orders get rich cards (status story + progress + Track, P11/P13);
/// history compresses into grouped tiles with date headers (P1, P4).
/// Status dot rows replace badge pills (P14).
///
/// The list is realtime: the screen watches [activeOrdersProvider], the
/// one 5s heartbeat shared with Home — a status the merchant or rider
/// advances appears here without a pull (and the poll stops the moment
/// nothing is in flight).
class OrdersScreen extends ConsumerStatefulWidget {
  const OrdersScreen({super.key});

  @override
  ConsumerState<OrdersScreen> createState() => _OrdersScreenState();
}

class _OrdersScreenState extends ConsumerState<OrdersScreen> {
  bool _showActive = true;

  List<GroupSummary> _active(List<GroupSummary> orders) => [
        for (final order in orders)
          if (groupInFlight(order)) order,
      ];

  List<GroupSummary> _history(List<GroupSummary> orders) => [
        for (final order in orders)
          if (!groupInFlight(order)) order,
      ];

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final ordersAsync = ref.watch(activeOrdersProvider);
    final orders = ordersAsync.value;
    final activeCount = orders == null ? 0 : _active(orders).length;

    return Scaffold(
      body: SafeArea(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 12, 16, 0),
              child: Text('Your Orders', style: AppTheme.d1(textTheme)),
            ),
            // The underline tabs (the spec's .tab): accent text + accent
            // underline — no gray Material active-block.
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 12, 16, 0),
              child: _Tabs(
                active: _showActive,
                activeCount: activeCount,
                onSelect: (value) => setState(() => _showActive = value),
              ),
            ),
            Expanded(child: _body(ordersAsync)),
          ],
        ),
      ),
    );
  }

  Widget _body(AsyncValue<List<GroupSummary>> ordersAsync) {
    if (ordersAsync.isLoading && !ordersAsync.hasValue) {
      return const Center(child: CircularProgressIndicator());
    }
    if (ordersAsync.hasError && !ordersAsync.hasValue) {
      return ErrorState(
        message: 'Could not load your orders.',
        onRetry: () => ref.invalidate(activeOrdersProvider),
      );
    }
    final list = ordersAsync.value ?? const <GroupSummary>[];
    final shown = _showActive ? _active(list) : _history(list);
    if (shown.isEmpty) {
      return RefreshIndicator(
        onRefresh: _refresh,
        child: ListView(
          children: [
            const SizedBox(height: 80),
            _EmptyOrders(active: _showActive),
          ],
        ),
      );
    }

    return RefreshIndicator(
      onRefresh: _refresh,
      color: AppColors.primary,
      backgroundColor: AppColors.surfaceAlt,
      child: _showActive
          ? ListView.builder(
              padding: const EdgeInsets.fromLTRB(16, 12, 16, 24),
              itemCount: shown.length,
              itemBuilder: (context, index) => Padding(
                padding: const EdgeInsets.only(bottom: 10),
                child: _ActiveOrderCard(
                  group: shown[index],
                  onTap: () => context.push('/orders/${shown[index].id}'),
                ),
              ),
            )
          : _HistoryList(groups: shown),
    );
  }

  /// Pull-to-refresh goes through the poller's silent poke: no loading
  /// state, the new list swaps in place, and a failure keeps the last
  /// good list.
  Future<void> _refresh() => ref.read(activeOrdersProvider.notifier).poke();
}

/// The underline tab pair.
class _Tabs extends StatelessWidget {
  const _Tabs({
    required this.active,
    required this.activeCount,
    required this.onSelect,
  });

  final bool active;
  final int activeCount;
  final ValueChanged<bool> onSelect;

  @override
  Widget build(BuildContext context) {
    return Container(
      decoration: const BoxDecoration(
        border: Border(bottom: BorderSide(color: AppColors.surfaceBorder)),
      ),
      child: Row(
        children: [
          _Tab(
            label: 'Active',
            count: activeCount,
            selected: active,
            onTap: () => onSelect(true),
          ),
          const SizedBox(width: 26),
          _Tab(
            label: 'History',
            selected: !active,
            onTap: () => onSelect(false),
          ),
        ],
      ),
    );
  }
}

class _Tab extends StatelessWidget {
  const _Tab({
    required this.label,
    required this.selected,
    required this.onTap,
    this.count,
  });

  final String label;
  final bool selected;
  final VoidCallback onTap;
  final int? count;

  @override
  Widget build(BuildContext context) {
    final color = selected ? AppColors.primary : AppColors.onSurfaceMuted;
    return InkWell(
      onTap: onTap,
      child: Container(
        padding: const EdgeInsets.fromLTRB(0, 11, 0, 11),
        decoration: BoxDecoration(
          border: Border(
            bottom: BorderSide(
              color: selected ? AppColors.primary : Colors.transparent,
              width: 2,
            ),
          ),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(
              label,
              style: TextStyle(
                fontSize: 14,
                fontWeight: FontWeight.w600,
                color: color,
              ),
            ),
            if (count != null && count! > 0) ...[
              const SizedBox(width: 5),
              Container(
                padding: const EdgeInsets.symmetric(horizontal: 7, vertical: 2),
                decoration: BoxDecoration(
                  color: selected
                      ? AppColors.primary.withValues(alpha: 0.15)
                      : AppColors.surfaceHigh,
                  borderRadius: BorderRadius.circular(999),
                ),
                child: Text(
                  '$count',
                  style: TextStyle(
                    fontSize: 11,
                    fontWeight: FontWeight.w700,
                    color: selected ? AppColors.primary : AppColors.onSurfaceMuted,
                  ),
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }
}

/// One ACTIVE order — the rich card: store name + accent total, the
/// status story on a dot row, the item line, a progress bar, Track.
class _ActiveOrderCard extends StatelessWidget {
  const _ActiveOrderCard({required this.group, required this.onTap});

  final GroupSummary group;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    // Every fulfilling store, joined — a multi-store order shows all of
    // them (one rider per store means several stories on one purchase).
    final store = group.stores.isEmpty
        ? 'Your order'
        : group.stores.join(' · ');
    final story = activeOrderStory(group);
    final color = story.color;
    final label = story.label;
    final progress = story.progress;
    final pulsing = story.pulsing;
    final subline = group.firstItemName != null
        ? group.firstItemName!
        : '${group.itemsCount} item${group.itemsCount == 1 ? '' : 's'}';

    return Container(
      padding: const EdgeInsets.all(14),
      decoration: BoxDecoration(
        color: AppColors.surfaceAlt,
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: AppColors.surfaceBorder),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              Expanded(
                child: Text(
                  store,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: textTheme.titleSmall?.copyWith(
                    fontSize: 14,
                    fontWeight: FontWeight.w700,
                  ),
                ),
              ),
              Text(
                formatRwf(group.grandTotal),
                style: textTheme.titleSmall?.copyWith(
                  fontSize: 14,
                  color: AppColors.primary,
                  fontWeight: FontWeight.w700,
                ),
              ),
            ],
          ),
          const SizedBox(height: 5),
          StatusRow(color: color, label: label, pulsing: pulsing),
          const SizedBox(height: 3),
          Text(subline, style: AppTheme.sub(textTheme)),
          const SizedBox(height: 10),
          Row(
            children: [
              Expanded(
                child: ClipRRect(
                  borderRadius: BorderRadius.circular(999),
                  child: LinearProgressIndicator(
                    value: progress,
                    minHeight: 4,
                    backgroundColor: AppColors.surfaceHigh,
                    valueColor: AlwaysStoppedAnimation<Color>(color),
                  ),
                ),
              ),
              const SizedBox(width: 12),
              TrackPill(onTap: onTap),
            ],
          ),
        ],
      ),
    );
  }
}

/// HISTORY — grouped by calendar day (the spec's hdiv + glabel): tiles
/// with store line, muted total, dot-status, chevron.
class _HistoryList extends StatelessWidget {
  const _HistoryList({required this.groups});

  final List<GroupSummary> groups;

  @override
  Widget build(BuildContext context) {
    // Group by day label, preserving newest-first order.
    final sections = <String, List<GroupSummary>>{};
    final order = <String>[];
    for (final group in groups) {
      final label = _dayLabel(group.createdAt);
      if (!sections.containsKey(label)) {
        sections[label] = [];
        order.add(label);
      }
      sections[label]!.add(group);
    }

    final children = <Widget>[];
    for (final label in order) {
      children
        ..add(Padding(
          padding: const EdgeInsets.fromLTRB(2, 12, 2, 8),
          child: Text(
            label.toUpperCase(),
            style: AppTheme.cap(context.textTheme),
          ),
        ))
        ..addAll([
          for (final group in sections[label]!)
            Padding(
              padding: const EdgeInsets.only(bottom: 8),
              child: _HistoryRow(
                group: group,
                onTap: () => context.push('/orders/${group.id}'),
              ),
            ),
        ]);
    }
    return ListView(
      padding: const EdgeInsets.fromLTRB(16, 0, 16, 24),
      children: children,
    );
  }

  String _dayLabel(String rfc3339) {
    try {
      final dt = DateTime.parse(rfc3339).toLocal();
      final now = DateTime.now();
      final today = DateTime(now.year, now.month, now.day);
      final thatDay = DateTime(dt.year, dt.month, dt.day);
      final diff = today.difference(thatDay).inDays;
      if (diff <= 0) return 'Today';
      if (diff == 1) return 'Yesterday';
      if (diff < 7) return 'This week';
      return 'Earlier';
    } on Object {
      return 'Earlier';
    }
  }
}

/// One HISTORY row: stores (+N), muted total, dot status, chevron.
class _HistoryRow extends StatelessWidget {
  const _HistoryRow({required this.group, required this.onTap});

  final GroupSummary group;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    // All the stores, joined — no redundant "+N store" after the names.
    final name = group.stores.isEmpty ? 'Your order' : group.stores.join(' · ');
    final story = _historyStatus(group.status);
    final color = story.color;
    final label = story.label;

    return Material(
      color: AppColors.surfaceAlt,
      borderRadius: BorderRadius.circular(16),
      child: InkWell(
        borderRadius: BorderRadius.circular(16),
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 13),
          child: Row(
            children: [
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      name,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: textTheme.titleSmall?.copyWith(fontSize: 14),
                    ),
                    const SizedBox(height: 4),
                    Row(
                      children: [
                        Text(
                          formatRwf(group.grandTotal),
                          style: textTheme.titleSmall?.copyWith(
                            fontSize: 13.5,
                            fontWeight: FontWeight.w700,
                          ),
                        ),
                        const SizedBox(width: 12),
                        StatusRow(color: color, label: label),
                      ],
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

  ({Color color, String label}) _historyStatus(String status) {
    switch (status) {
      case 'completed':
        return (color: AppColors.success, label: 'Delivered');
      case 'cancelled':
        return (color: AppColors.error, label: 'Cancelled');
      default:
        return (color: AppColors.onSurfaceMuted, label: status);
    }
  }
}

class _EmptyOrders extends StatelessWidget {
  const _EmptyOrders({required this.active});

  final bool active;

  @override
  Widget build(BuildContext context) {
    return Center(
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
              Icons.receipt_long_rounded,
              size: 40,
              color: AppColors.onSurfaceMuted,
            ),
          ),
          const SizedBox(height: 18),
          Text(
            active ? 'No active orders' : 'No past orders',
            style: AppTheme.hd(context.textTheme).copyWith(fontSize: 16),
          ),
          const SizedBox(height: 4),
          Text(
            active
                ? 'When you place an order, it lives here while it moves.'
                : 'Your order history will appear here.',
            style: AppTheme.sub(context.textTheme),
          ),
          const SizedBox(height: 20),
          FilledButton(
            onPressed: () => context.go('/home'),
            style: FilledButton.styleFrom(
              minimumSize: const Size(0, 50),
              padding: const EdgeInsets.symmetric(horizontal: 28),
            ),
            child: const Text('Browse stores'),
          ),
        ],
      ),
    );
  }
}

extension on BuildContext {
  TextTheme get textTheme => Theme.of(this).textTheme;
}

